use crate::document::{
    CompiledFind, Document, DocumentError, DocumentResult, DocumentSummary, FindOptions,
    LineEnding, MatchSpan, TextEncoding, decode_bytes,
};
use crate::file_watcher::{FileEvent, FileWatcher};
use directories::ProjectDirs;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_MRU_LIMIT: usize = 10;
const MAX_FILE_SIZE_BYTES: u64 = 25 * 1024 * 1024;

/// Public snapshot of a single match expressed in character indices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindMatch {
    pub start: usize,
    pub end: usize,
}

/// Aggregated search state for a document suitable for serialization to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindSnapshot {
    pub query: String,
    pub options: FindOptions,
    pub matches: Vec<FindMatch>,
    pub current_index: Option<usize>,
    pub message: Option<String>,
}

impl FindSnapshot {
    fn empty() -> Self {
        Self {
            query: String::new(),
            options: FindOptions::default(),
            matches: Vec::new(),
            current_index: None,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchDirection {
    Forward,
    Backward,
}

/// Central service responsible for creating, tracking, and persisting documents.
pub struct DocumentManager {
    documents: Vec<Document>,
    active_id: Option<u64>,
    next_id: u64,
    config_path: PathBuf,
    autosave_path: PathBuf,
    config: ManagerConfig,
    mru_limit: usize,
    find_sessions: HashMap<u64, FindSession>,
    file_watcher: Option<FileWatcher>,
}

#[derive(Debug, Clone)]
struct FindSession {
    query: String,
    options: FindOptions,
    regex: Arc<Regex>,
    matches: Vec<StoredMatch>,
    current: Option<usize>,
}

impl FindSession {
    fn from_compiled(query: String, options: FindOptions, compiled: CompiledFind) -> Self {
        let matches = compiled
            .matches
            .into_iter()
            .map(StoredMatch::from)
            .collect::<Vec<_>>();
        let current = if matches.is_empty() { None } else { Some(0) };
        Self {
            query,
            options,
            regex: Arc::new(compiled.regex),
            matches,
            current,
        }
    }

    fn snapshot(&self, message: Option<String>) -> FindSnapshot {
        FindSnapshot {
            query: self.query.clone(),
            options: self.options,
            matches: self.matches.iter().map(StoredMatch::to_public).collect(),
            current_index: self.current,
            message,
        }
    }

    fn ensure_current_within_bounds(&mut self) {
        if self.matches.is_empty() {
            self.current = None;
        } else {
            let idx = self.current.unwrap_or(0);
            let max = self.matches.len() - 1;
            self.current = Some(idx.min(max));
        }
    }
}

#[derive(Debug, Clone)]
struct StoredMatch {
    char_start: usize,
    char_end: usize,
}

impl StoredMatch {
    fn to_public(&self) -> FindMatch {
        FindMatch {
            start: self.char_start,
            end: self.char_end,
        }
    }
}

impl From<MatchSpan> for StoredMatch {
    fn from(span: MatchSpan) -> Self {
        Self {
            char_start: span.char_start,
            char_end: span.char_end,
        }
    }
}

impl DocumentManager {
    /// Construct a manager using the standard configuration location.
    pub fn new() -> DocumentResult<Self> {
        let config_path = default_config_dir().join("config.json");
        Self::with_config_path(config_path)
    }

    /// Construct a manager pointing at a specific configuration file.
    pub fn with_config_path(config_path: PathBuf) -> DocumentResult<Self> {
        let config = load_config(&config_path)?;
        let autosave_path = autosave_path_for(&config_path);
        let file_watcher = FileWatcher::new().ok();
        Ok(Self {
            documents: Vec::new(),
            active_id: None,
            next_id: 1,
            config_path,
            autosave_path,
            config,
            mru_limit: DEFAULT_MRU_LIMIT,
            find_sessions: HashMap::new(),
            file_watcher,
        })
    }

    /// Returns the summaries of all open documents.
    pub fn documents(&self) -> Vec<DocumentSummary> {
        self.documents.iter().map(Document::summary).collect()
    }

    /// Returns the active document summary if one is selected.
    pub fn active_document(&self) -> Option<DocumentSummary> {
        let id = self.active_id?;
        self.documents
            .iter()
            .find(|doc| doc.id() == id)
            .map(Document::summary)
    }

    /// Returns the identifier of the active document.
    pub fn active_document_id(&self) -> Option<u64> {
        self.active_id
    }

    /// Sets the active document to the specified identifier.
    pub fn set_active_document(&mut self, id: u64) -> DocumentResult<()> {
        if self.documents.iter().any(|doc| doc.id() == id) {
            self.active_id = Some(id);
            Ok(())
        } else {
            Err(DocumentError::DocumentNotFound(id))
        }
    }

    /// Returns the list of recently opened documents ordered by most recent first.
    pub fn recent_documents(&self) -> Vec<RecentDocument> {
        self.config.recent_documents.clone()
    }

    /// Creates a new, empty document with the supplied title.
    pub fn new_document(&mut self, title: impl Into<String>) -> DocumentSummary {
        let id = self.next_id();
        let document = Document::new_empty(id, title);
        self.active_id = Some(id);
        self.documents.push(document);
        self.autosave().ok();
        self.documents.last().unwrap().summary()
    }

    /// Opens a document from disk, returning its summary.
    pub fn open_document(&mut self, path: impl AsRef<Path>) -> DocumentResult<DocumentSummary> {
        let path_buf = path.as_ref().to_path_buf();

        if let Some(existing) = self
            .documents
            .iter()
            .find(|doc| doc.path().map(|p| p == path_buf.as_path()).unwrap_or(false))
        {
            self.active_id = Some(existing.id());
            return Ok(existing.summary());
        }

        if let Ok(metadata) = fs::metadata(&path_buf) {
            let size = metadata.len();
            if size > MAX_FILE_SIZE_BYTES {
                return Err(DocumentError::FileTooLarge {
                    path: path_buf,
                    size,
                    limit: MAX_FILE_SIZE_BYTES,
                });
            }
        }

        let id = self.next_id();
        let mut document = Document::open(id, &path_buf)?;
        document.mark_clean();
        let summary = document.summary();
        self.documents.push(document);
        self.active_id = Some(summary.id);

        // Start watching the file for external changes
        if let Some(watcher) = &mut self.file_watcher {
            let _ = watcher.watch(path_buf.clone(), id);
        }

        self.record_recent(&summary, Some(path_buf));
        self.persist_config()?;
        self.autosave()?;
        Ok(summary)
    }

    /// Saves the specified document to its current path.
    pub fn save_document(&mut self, id: u64) -> DocumentResult<DocumentSummary> {
        let summary = {
            let document = self.get_mut(id)?;
            if document.editing_locked() {
                return Err(DocumentError::ReadOnlyDocument(
                    document.title().to_string(),
                ));
            }
            document.save()?;
            document.summary()
        };
        let path = summary.path.clone();
        self.record_recent(&summary, path);
        self.persist_config()?;
        self.autosave()?;
        Ok(summary)
    }

    /// Saves the specified document to a new path.
    pub fn save_document_as(
        &mut self,
        id: u64,
        path: impl AsRef<Path>,
    ) -> DocumentResult<DocumentSummary> {
        let path_buf = path.as_ref();
        let summary = {
            let document = self.get_mut(id)?;
            document.save_as(path_buf)?;
            document.summary()
        };
        let new_path = summary.path.clone();
        self.record_recent(&summary, new_path);
        self.persist_config()?;
        self.autosave()?;
        Ok(summary)
    }

    /// Updates the text of a document and returns its new summary.
    pub fn update_text(
        &mut self,
        id: u64,
        text: impl Into<String>,
    ) -> DocumentResult<DocumentSummary> {
        if let Some(title) = {
            let document = self.get(id)?;
            if document.editing_locked() {
                Some(document.title().to_string())
            } else {
                None
            }
        } {
            return Err(DocumentError::ReadOnlyDocument(title));
        }
        let text = text.into();
        {
            let document = self.get_mut(id)?;
            document.set_text(text);
        }
        self.refresh_find_session(id)?;
        self.autosave()?;
        self.document_summary(id)
    }

    /// Returns the current text buffer for the specified document.
    pub fn document_text(&self, id: u64) -> DocumentResult<String> {
        Ok(self.get(id)?.text().to_string())
    }

    /// Returns the latest summary for the specified document.
    pub fn document_summary(&self, id: u64) -> DocumentResult<DocumentSummary> {
        Ok(self.get(id)?.summary())
    }

    /// Sets the encoding for a document without reloading its contents.
    pub fn set_document_encoding(
        &mut self,
        id: u64,
        encoding: TextEncoding,
    ) -> DocumentResult<DocumentSummary> {
        if let Some(title) = {
            let document = self.get(id)?;
            if document.editing_locked() {
                Some(document.title().to_string())
            } else {
                None
            }
        } {
            return Err(DocumentError::ReadOnlyDocument(title));
        }
        {
            let document = self.get_mut(id)?;
            document.set_encoding(encoding);
        }
        self.autosave()?;
        self.document_summary(id)
    }

    /// Converts the active document buffer to a different line ending style.
    pub fn set_document_line_ending(
        &mut self,
        id: u64,
        line_ending: LineEnding,
    ) -> DocumentResult<DocumentSummary> {
        if let Some(title) = {
            let document = self.get(id)?;
            if document.editing_locked() {
                Some(document.title().to_string())
            } else {
                None
            }
        } {
            return Err(DocumentError::ReadOnlyDocument(title));
        }
        {
            let document = self.get_mut(id)?;
            document.set_line_ending(line_ending);
        }
        self.refresh_find_session(id)?;
        self.autosave()?;
        self.document_summary(id)
    }

    /// Enables or disables the edit override for a read-only document.
    pub fn set_read_only_override(
        &mut self,
        id: u64,
        allow_edit: bool,
    ) -> DocumentResult<DocumentSummary> {
        {
            let document = self.get_mut(id)?;
            document.set_edit_override(allow_edit);
        }
        self.autosave().ok();
        self.document_summary(id)
    }

    /// Reloads the document contents from disk using the specified encoding.
    pub fn reload_document_with_encoding(
        &mut self,
        id: u64,
        encoding: TextEncoding,
    ) -> DocumentResult<DocumentSummary> {
        {
            let document = self.get_mut(id)?;
            document.reload_with_encoding(encoding)?;
        }
        self.refresh_find_session(id)?;
        let summary = self.document_summary(id)?;
        let path = summary.path.clone();
        if path.is_some() {
            self.record_recent(&summary, path);
            self.persist_config()?;
        }
        self.autosave()?;
        Ok(summary)
    }

    /// Update (or initialize) the search session for the specified document.
    pub fn find_update(
        &mut self,
        id: u64,
        query: impl Into<String>,
        options: FindOptions,
    ) -> DocumentResult<FindSnapshot> {
        let query = query.into();
        if query.is_empty() {
            self.find_sessions.remove(&id);
            return Ok(FindSnapshot::empty());
        }

        let compiled = {
            let document = self.get(id)?;
            document.compile_find(&query, &options)?
        };

        let session = FindSession::from_compiled(query, options, compiled);
        let message = if session.matches.is_empty() {
            Some("No matches found".to_string())
        } else {
            None
        };
        let snapshot = session.snapshot(message);
        self.find_sessions.insert(id, session);
        Ok(snapshot)
    }

    /// Step to the next (or previous) match in the active search session.
    pub fn find_step(
        &mut self,
        id: u64,
        wrap: bool,
        backwards: bool,
    ) -> DocumentResult<FindSnapshot> {
        self.refresh_find_session(id)?;
        let direction = if backwards {
            SearchDirection::Backward
        } else {
            SearchDirection::Forward
        };

        let Some(session) = self.find_sessions.get_mut(&id) else {
            return Ok(FindSnapshot::empty());
        };

        if session.matches.is_empty() {
            return Ok(session.snapshot(Some("No matches found".to_string())));
        }

        let message = advance_session(session, direction, wrap);
        Ok(session.snapshot(message))
    }

    /// Replace the current match (if any) and advance according to the provided direction.
    pub fn replace_current(
        &mut self,
        id: u64,
        replacement: impl Into<String>,
        wrap: bool,
        backwards: bool,
    ) -> DocumentResult<FindSnapshot> {
        self.refresh_find_session(id)?;

        let direction = if backwards {
            SearchDirection::Backward
        } else {
            SearchDirection::Forward
        };

        let Some(session_clone) = self.find_sessions.get(&id).cloned() else {
            return Ok(FindSnapshot::empty());
        };

        if session_clone.matches.is_empty() {
            return Ok(session_clone.snapshot(Some("No matches found".to_string())));
        }

        let target_index = session_clone.current.unwrap_or_else(|| match direction {
            SearchDirection::Forward => 0,
            SearchDirection::Backward => session_clone.matches.len() - 1,
        });

        if target_index >= session_clone.matches.len() {
            return Ok(session_clone.snapshot(Some("No matches found".to_string())));
        }

        let regex = Arc::clone(&session_clone.regex);
        let use_regex = session_clone.options.use_regex;
        let replacement = replacement.into();

        let replaced = {
            let document = self.get_mut(id)?;
            let text = document.text().to_string();
            let (new_text, did_replace) =
                replace_target(&regex, &text, &replacement, use_regex, target_index);
            if did_replace {
                document.set_text(new_text);
                true
            } else {
                false
            }
        };

        self.refresh_find_session(id)?;

        if replaced {
            self.autosave()?;
        }

        let Some(session) = self.find_sessions.get_mut(&id) else {
            return Ok(FindSnapshot::empty());
        };

        let mut message = if replaced {
            Some("Replaced 1 occurrence".to_string())
        } else {
            Some("No matching occurrence at cursor".to_string())
        };

        if replaced && !session.matches.is_empty() {
            let step_message = advance_session(session, direction, wrap);
            message = combine_messages(message, step_message);
        }

        if !replaced {
            session.ensure_current_within_bounds();
        }

        Ok(session.snapshot(message))
    }

    /// Replace every match in the current session.
    pub fn replace_all(
        &mut self,
        id: u64,
        replacement: impl Into<String>,
    ) -> DocumentResult<FindSnapshot> {
        self.refresh_find_session(id)?;

        let Some(session_clone) = self.find_sessions.get(&id).cloned() else {
            return Ok(FindSnapshot::empty());
        };

        if session_clone.matches.is_empty() {
            return Ok(session_clone.snapshot(Some("No matches found".to_string())));
        }

        let regex = Arc::clone(&session_clone.regex);
        let use_regex = session_clone.options.use_regex;
        let replacement = replacement.into();

        let replaced_count = {
            let document = self.get_mut(id)?;
            let text = document.text().to_string();
            let (new_text, count) = replace_all_occurrences(&regex, &text, &replacement, use_regex);
            if count > 0 {
                document.set_text(new_text);
            }
            count
        };

        self.refresh_find_session(id)?;

        if replaced_count > 0 {
            self.autosave()?;
        }

        let Some(session) = self.find_sessions.get_mut(&id) else {
            return Ok(FindSnapshot::empty());
        };

        let message = if replaced_count == 0 {
            Some("No matches found".to_string())
        } else {
            Some(format!(
                "Replaced {replaced_count} occurrence{}",
                if replaced_count == 1 { "" } else { "s" }
            ))
        };

        Ok(session.snapshot(message))
    }

    /// Clear any cached search state for the given document.
    pub fn clear_find_state(&mut self, id: u64) {
        self.find_sessions.remove(&id);
    }

    /// Retrieve the current find snapshot for the specified document if it exists.
    pub fn find_snapshot(&self, id: u64) -> Option<FindSnapshot> {
        self.find_sessions
            .get(&id)
            .map(|session| session.snapshot(None))
    }

    /// Retrieve the active document's find snapshot, if any.
    pub fn active_find_snapshot(&self) -> Option<FindSnapshot> {
        self.active_id
            .and_then(|id| self.find_sessions.get(&id))
            .map(|session| session.snapshot(None))
    }

    /// Closes the document, optionally forcing the close even if unsaved.
    pub fn close_document(&mut self, id: u64, discard_unsaved: bool) -> DocumentResult<()> {
        let position = self
            .documents
            .iter()
            .position(|doc| doc.id() == id)
            .ok_or(DocumentError::DocumentNotFound(id))?;
        let is_dirty = self.documents[position].is_dirty();
        if is_dirty && !discard_unsaved {
            return Err(DocumentError::DocumentDirty(
                self.documents[position].summary(),
            ));
        }

        // Stop watching the file before removing the document
        if let Some(path) = self.documents[position].path().map(|p| p.to_path_buf())
            && let Some(watcher) = &mut self.file_watcher
        {
            let _ = watcher.unwatch(&path);
        }

        self.documents.remove(position);
        self.find_sessions.remove(&id);
        if self.active_id == Some(id) {
            self.active_id = self.documents.last().map(Document::id);
        }
        if self.documents.is_empty() {
            self.active_id = None;
        }
        self.autosave()?;
        Ok(())
    }

    /// Loads a document directly from raw bytes—useful for tests.
    pub fn open_from_bytes(
        &mut self,
        path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> DocumentResult<DocumentSummary> {
        let id = self.next_id();
        let (encoding, text) = decode_bytes(bytes)?;
        let mut document = Document::new_empty(id, path.as_ref().to_string_lossy());
        document.set_encoding(encoding);
        document.set_text(text);
        document.set_path(path.as_ref());
        document.mark_clean();
        let summary = document.summary();
        self.documents.push(document);
        self.record_recent(&summary, Some(path.as_ref().to_path_buf()));
        self.persist_config()?;
        self.autosave()?;
        Ok(summary)
    }

    fn refresh_find_session(&mut self, id: u64) -> DocumentResult<()> {
        let Some(existing) = self.find_sessions.get(&id).cloned() else {
            return Ok(());
        };

        if existing.query.is_empty() {
            self.find_sessions.remove(&id);
            return Ok(());
        }

        let compiled = {
            let document = self.get(id)?;
            document.compile_find(&existing.query, &existing.options)?
        };

        let mut session = FindSession::from_compiled(existing.query, existing.options, compiled);
        if let Some(current) = existing.current
            && !session.matches.is_empty()
        {
            let max = session.matches.len() - 1;
            session.current = Some(current.min(max));
        }
        self.find_sessions.insert(id, session);
        Ok(())
    }

    fn get_mut(&mut self, id: u64) -> DocumentResult<&mut Document> {
        self.documents
            .iter_mut()
            .find(|doc| doc.id() == id)
            .ok_or(DocumentError::DocumentNotFound(id))
    }

    fn get(&self, id: u64) -> DocumentResult<&Document> {
        self.documents
            .iter()
            .find(|doc| doc.id() == id)
            .ok_or(DocumentError::DocumentNotFound(id))
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn record_recent(&mut self, summary: &DocumentSummary, path: Option<PathBuf>) {
        let Some(path) = path else {
            return;
        };
        let entry = RecentDocument {
            path: path.clone(),
            title: summary.title.clone(),
            encoding: summary.encoding,
            line_ending: summary.line_ending,
            last_opened_epoch: system_time_to_epoch(SystemTime::now()),
        };
        self.config
            .recent_documents
            .retain(|item| item.path != path);
        self.config.recent_documents.insert(0, entry);
        if self.config.recent_documents.len() > self.mru_limit {
            self.config.recent_documents.truncate(self.mru_limit);
        }
    }

    fn persist_config(&self) -> DocumentResult<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(&self.config)
            .map_err(|err| DocumentError::InvalidData(err.to_string()))?;
        fs::write(&self.config_path, data)?;
        Ok(())
    }

    /// Persist the current set of open documents to the autosave snapshot.
    pub fn autosave(&self) -> DocumentResult<()> {
        if let Some(parent) = self.autosave_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if self.documents.is_empty() {
            fs::remove_file(&self.autosave_path).ok();
            return Ok(());
        }

        let snapshot = AutosaveSnapshot {
            documents: self
                .documents
                .iter()
                .map(|doc| AutosaveDocument {
                    id: doc.id(),
                    title: doc.title().to_string(),
                    text: doc.text().to_string(),
                    path: doc.path().map(|p| p.to_path_buf()),
                    encoding: doc.encoding(),
                    line_ending: doc.line_ending(),
                    dirty: doc.is_dirty(),
                    read_only: doc.read_only(),
                    edit_override: doc.edit_override(),
                })
                .collect(),
            active_id: self.active_id,
            next_id: self.next_id,
            timestamp_epoch: system_time_to_epoch(SystemTime::now()),
        };

        let data = serde_json::to_vec_pretty(&snapshot)
            .map_err(|err| DocumentError::InvalidData(err.to_string()))?;
        fs::write(&self.autosave_path, data)?;
        Ok(())
    }

    /// Returns the epoch timestamp recorded in the most recent autosave file, if present.
    pub fn autosave_epoch(&self) -> Option<u64> {
        fs::read(&self.autosave_path)
            .ok()
            .and_then(|data| serde_json::from_slice::<AutosaveSnapshot>(&data).ok())
            .map(|snapshot| snapshot.timestamp_epoch)
    }

    // =========================================================================
    // Settings access and persistence
    // =========================================================================

    /// Returns the current UI settings.
    pub fn ui_settings(&self) -> &UiSettings {
        &self.config.ui
    }

    /// Returns the current editor settings.
    pub fn editor_settings(&self) -> &EditorSettings {
        &self.config.editor
    }

    /// Returns the current find defaults.
    pub fn find_defaults(&self) -> &FindDefaults {
        &self.config.find_defaults
    }

    /// Returns the current window state.
    pub fn window_state(&self) -> &WindowState {
        &self.config.window
    }

    /// Updates UI settings and persists them.
    pub fn update_ui_settings(&mut self, settings: UiSettings) -> DocumentResult<()> {
        self.config.ui = settings;
        self.persist_config()
    }

    /// Updates editor settings and persists them.
    pub fn update_editor_settings(&mut self, settings: EditorSettings) -> DocumentResult<()> {
        self.config.editor = settings;
        self.persist_config()
    }

    /// Updates find defaults and persists them.
    pub fn update_find_defaults(&mut self, defaults: FindDefaults) -> DocumentResult<()> {
        self.config.find_defaults = defaults;
        self.persist_config()
    }

    /// Updates window state and persists it.
    pub fn update_window_state(&mut self, state: WindowState) -> DocumentResult<()> {
        self.config.window = state;
        self.persist_config()
    }

    // =========================================================================
    // File watching
    // =========================================================================

    /// Poll for file system events and update affected documents.
    /// Returns a list of document IDs that were affected.
    pub fn poll_file_events(&mut self) -> Vec<u64> {
        let events = match &self.file_watcher {
            Some(watcher) => watcher.poll_events(),
            None => return Vec::new(),
        };

        let mut affected_ids = Vec::new();
        for (doc_id, event) in events {
            if let Some(doc) = self.documents.iter_mut().find(|d| d.id() == doc_id) {
                match event {
                    FileEvent::Modified(_) => {
                        doc.set_externally_modified(true);
                        affected_ids.push(doc_id);
                    }
                    FileEvent::Deleted(_) => {
                        doc.set_externally_deleted(true);
                        affected_ids.push(doc_id);
                    }
                }
            }
        }

        affected_ids
    }

    /// Reload a document from disk, clearing external modification flags.
    pub fn reload_document(&mut self, id: u64) -> DocumentResult<DocumentSummary> {
        let encoding = {
            let document = self.get(id)?;
            document.encoding()
        };
        {
            let document = self.get_mut(id)?;
            document.reload_with_encoding(encoding)?;
            document.clear_external_flags();
        }
        self.refresh_find_session(id)?;
        self.autosave()?;
        self.document_summary(id)
    }

    /// Dismiss external modification notification without reloading.
    pub fn acknowledge_external_change(&mut self, id: u64) -> DocumentResult<DocumentSummary> {
        {
            let document = self.get_mut(id)?;
            document.clear_external_flags();
        }
        self.autosave()?;
        self.document_summary(id)
    }

    /// Attempts to restore a previously saved autosave snapshot.
    pub fn restore_autosave(&mut self) -> DocumentResult<bool> {
        let data = match fs::read(&self.autosave_path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(DocumentError::Io(err)),
        };

        let snapshot: AutosaveSnapshot = serde_json::from_slice(&data)
            .map_err(|err| DocumentError::InvalidData(err.to_string()))?;

        self.documents.clear();
        self.active_id = None;
        self.find_sessions.clear();

        let mut max_id = 0;
        for doc in snapshot.documents {
            let AutosaveDocument {
                id,
                title,
                text,
                path,
                encoding,
                line_ending,
                dirty,
                read_only: _,
                edit_override,
            } = doc;

            max_id = max_id.max(id);
            let mut document = Document::new_empty(id, title);
            if let Some(path) = path {
                document.set_path(&path);
                document.refresh_read_only();
            }
            document.set_encoding(encoding);
            document.set_text(text);
            document.set_line_ending(line_ending);
            if !dirty {
                document.mark_clean();
            }
            document.set_edit_override(edit_override);
            self.documents.push(document);
        }

        self.active_id = snapshot.active_id.and_then(|id| {
            if self.documents.iter().any(|doc| doc.id() == id) {
                Some(id)
            } else {
                None
            }
        });

        self.next_id = snapshot.next_id.max(max_id.saturating_add(1));

        if self.documents.is_empty() {
            self.active_id = None;
        } else if self.active_id.is_none() {
            self.active_id = self.documents.last().map(Document::id);
        }

        Ok(true)
    }
}

fn advance_session(
    session: &mut FindSession,
    direction: SearchDirection,
    wrap: bool,
) -> Option<String> {
    if session.matches.is_empty() {
        session.current = None;
        return Some("No matches found".to_string());
    }

    let len = session.matches.len();
    let mut message = None;
    let next = match (session.current, direction) {
        (Some(idx), SearchDirection::Forward) => {
            if idx + 1 < len {
                idx + 1
            } else if wrap {
                message = Some("Wrapped to top".to_string());
                0
            } else {
                message = Some("Reached end of document".to_string());
                idx
            }
        }
        (Some(idx), SearchDirection::Backward) => {
            if idx > 0 {
                idx - 1
            } else if wrap {
                message = Some("Wrapped to bottom".to_string());
                len - 1
            } else {
                message = Some("Reached start of document".to_string());
                idx
            }
        }
        (None, SearchDirection::Forward) => 0,
        (None, SearchDirection::Backward) => len - 1,
    };

    session.current = Some(next);
    session.ensure_current_within_bounds();
    message
}

fn combine_messages(primary: Option<String>, secondary: Option<String>) -> Option<String> {
    match (primary, secondary) {
        (None, None) => None,
        (Some(p), None) => Some(p),
        (None, Some(s)) => Some(s),
        (Some(p), Some(s)) => Some(format!("{p} — {s}")),
    }
}

fn replace_target(
    regex: &Regex,
    text: &str,
    replacement: &str,
    use_regex: bool,
    target_index: usize,
) -> (String, bool) {
    let counter = Cell::new(0usize);
    let replaced = Cell::new(false);
    let replacement_owned = replacement.to_string();
    let result = regex.replace_all(text, |caps: &Captures| {
        let idx = counter.get();
        counter.set(idx + 1);
        if idx == target_index {
            replaced.set(true);
            if use_regex {
                let mut buf = String::new();
                caps.expand(&replacement_owned, &mut buf);
                buf
            } else {
                replacement_owned.clone()
            }
        } else {
            caps.get(0).unwrap().as_str().to_string()
        }
    });
    (result.into_owned(), replaced.get())
}

fn replace_all_occurrences(
    regex: &Regex,
    text: &str,
    replacement: &str,
    use_regex: bool,
) -> (String, usize) {
    let count = Cell::new(0usize);
    let replacement_owned = replacement.to_string();
    let result = regex.replace_all(text, |caps: &Captures| {
        count.set(count.get() + 1);
        if use_regex {
            let mut buf = String::new();
            caps.expand(&replacement_owned, &mut buf);
            buf
        } else {
            replacement_owned.clone()
        }
    });
    (result.into_owned(), count.get())
}

fn load_config(path: &Path) -> DocumentResult<ManagerConfig> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|err| DocumentError::InvalidData(err.to_string())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ManagerConfig::default()),
        Err(err) => Err(DocumentError::Io(err)),
    }
}

fn default_config_dir() -> PathBuf {
    if let Some(project_dirs) = ProjectDirs::from("dev", "GhostPad", "GhostPad") {
        project_dirs.config_dir().to_path_buf()
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/ghostpad")
    } else {
        PathBuf::from(".ghostpad")
    }
}

fn autosave_path_for(config_path: &Path) -> PathBuf {
    let file_stem = config_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| format!("{stem}.autosave.json"))
        .unwrap_or_else(|| "autosave.json".to_string());

    if let Some(parent) = config_path.parent() {
        parent.join(file_stem)
    } else {
        config_path.with_file_name(file_stem)
    }
}

fn system_time_to_epoch(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Theme mode selection for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Follow the desktop's light color scheme.
    Light,
    /// Follow the desktop's dark color scheme.
    Dark,
    /// Inherit the desktop's active color scheme.
    #[default]
    System,
    /// Tokyo Night (the classic dark "night" variant).
    TokyoNight,
    /// Tokyo Night Storm (lighter blue-grey background).
    TokyoNightStorm,
    /// Tokyo Night Moon (cooler moon background).
    TokyoNightMoon,
}

impl ThemeMode {
    /// Returns the canonical snake_case identifier used in persisted settings
    /// and across the QML bridge.
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::System => "system",
            ThemeMode::TokyoNight => "tokyo_night",
            ThemeMode::TokyoNightStorm => "tokyo_night_storm",
            ThemeMode::TokyoNightMoon => "tokyo_night_moon",
        }
    }

    /// Parses a theme identifier, falling back to [`ThemeMode::System`] for any
    /// unknown value. Accepts the canonical snake_case identifiers.
    pub fn from_str_lenient(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            "tokyo_night" => ThemeMode::TokyoNight,
            "tokyo_night_storm" => ThemeMode::TokyoNightStorm,
            "tokyo_night_moon" => ThemeMode::TokyoNightMoon,
            _ => ThemeMode::System,
        }
    }
}

/// Indentation style preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IndentType {
    #[default]
    Spaces,
    Tabs,
}

/// UI-related settings persisted across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    #[serde(default)]
    pub word_wrap_enabled: bool,
    #[serde(default)]
    pub translucency_enabled: bool,
    #[serde(default = "default_shadow_enabled")]
    pub shadow_enabled: bool,
    #[serde(default)]
    pub theme: ThemeMode,
}

fn default_shadow_enabled() -> bool {
    true
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            word_wrap_enabled: false,
            translucency_enabled: false,
            shadow_enabled: true,
            theme: ThemeMode::System,
        }
    }
}

/// Editor behavior settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_tab_stop")]
    pub tab_stop_distance: u32,
    #[serde(default)]
    pub indent_type: IndentType,
    #[serde(default = "default_indent_size")]
    pub indent_size: u32,
}

fn default_font_family() -> String {
    "monospace".to_string()
}

fn default_font_size() -> u32 {
    11
}

fn default_tab_stop() -> u32 {
    4
}

fn default_indent_size() -> u32 {
    4
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            font_size: default_font_size(),
            tab_stop_distance: default_tab_stop(),
            indent_type: IndentType::Spaces,
            indent_size: default_indent_size(),
        }
    }
}

/// Find/search default options.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FindDefaults {
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub use_regex: bool,
    #[serde(default = "default_wrap_around")]
    pub wrap_around: bool,
}

fn default_wrap_around() -> bool {
    true
}

impl Default for FindDefaults {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            wrap_around: true,
        }
    }
}

/// Window geometry state for restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    #[serde(default = "default_window_width")]
    pub width: u32,
    #[serde(default = "default_window_height")]
    pub height: u32,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
}

fn default_window_width() -> u32 {
    960
}

fn default_window_height() -> u32 {
    620
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: default_window_width(),
            height: default_window_height(),
            x: None,
            y: None,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ManagerConfig {
    #[serde(default)]
    recent_documents: Vec<RecentDocument>,
    #[serde(default)]
    ui: UiSettings,
    #[serde(default)]
    editor: EditorSettings,
    #[serde(default)]
    find_defaults: FindDefaults,
    #[serde(default)]
    window: WindowState,
}

#[derive(Debug, Serialize, Deserialize)]
struct AutosaveSnapshot {
    documents: Vec<AutosaveDocument>,
    active_id: Option<u64>,
    next_id: u64,
    timestamp_epoch: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AutosaveDocument {
    id: u64,
    title: String,
    text: String,
    path: Option<PathBuf>,
    encoding: TextEncoding,
    line_ending: LineEnding,
    dirty: bool,
    #[serde(default)]
    read_only: bool,
    #[serde(default)]
    edit_override: bool,
}
/// Serialized representation of a recently accessed document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentDocument {
    pub path: PathBuf,
    pub title: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub last_opened_epoch: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ghostpad_manager_{label}_{unique}"))
    }

    fn config_path(label: &str) -> PathBuf {
        temp_path(label).with_file_name(format!("config_{label}.json"))
    }

    fn cleanup_autosave(config: &Path) {
        let autosave = autosave_path_for(config);
        fs::remove_file(autosave).ok();
    }

    #[test]
    fn create_and_open_document() {
        let cfg = config_path("open");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();

        let doc_path = temp_path("file.txt");
        fs::write(&doc_path, "hello world").unwrap();

        let summary = manager.open_document(&doc_path).unwrap();
        assert_eq!(
            summary.title,
            doc_path.file_name().unwrap().to_str().unwrap()
        );
        assert!(summary.path.is_some());

        let recents = manager.recent_documents();
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].path, doc_path);

        fs::remove_file(doc_path).ok();
        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn closing_dirty_document_requires_force() {
        let cfg = config_path("dirty");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let summary = manager.new_document("Scratch");

        manager.update_text(summary.id, "data").unwrap();
        let result = manager.close_document(summary.id, false);
        assert!(matches!(result, Err(DocumentError::DocumentDirty(_))));

        manager.close_document(summary.id, true).unwrap();
        assert!(manager.documents().is_empty());

        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn save_document_updates_recent_list() {
        let cfg = config_path("save");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let mut file = File::create(cfg.clone().with_file_name("content.txt")).unwrap();
        writeln!(file, "hello").unwrap();
        let path = cfg.with_file_name("content.txt");

        let summary = manager.open_document(&path).unwrap();
        manager.update_text(summary.id, "goodbye").unwrap();
        manager.save_document(summary.id).unwrap();

        let recents = manager.recent_documents();
        assert_eq!(recents[0].title, "content.txt");

        fs::remove_file(path).ok();
        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn read_only_documents_require_override_for_edits() {
        let cfg = config_path("read_only");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let path = cfg.with_file_name("locked.txt");
        fs::write(&path, "locked").unwrap();

        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms.clone()).unwrap();

        let summary = manager.open_document(&path).unwrap();
        assert!(summary.read_only);
        assert!(summary.editing_locked);

        let err = manager.update_text(summary.id, "changes");
        assert!(matches!(err, Err(DocumentError::ReadOnlyDocument(_))));

        manager.set_read_only_override(summary.id, true).unwrap();
        let updated = manager.update_text(summary.id, "changes").unwrap();
        assert!(!updated.editing_locked);

        // Restore write permission so the temp file can be deleted; the broad
        // permission change is harmless on a throwaway test fixture.
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&path, perms).ok();
        fs::remove_file(path).ok();
        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn autosave_snapshot_records_document_state() {
        let cfg = config_path("autosave_snapshot");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();

        let summary = manager.new_document("Scratch");
        manager.update_text(summary.id, "hello world").unwrap();

        let autosave_path = autosave_path_for(&cfg);
        let data = fs::read(&autosave_path).expect("autosave snapshot missing");
        let snapshot: AutosaveSnapshot = serde_json::from_slice(&data).unwrap();
        assert_eq!(snapshot.documents.len(), 1);
        assert_eq!(snapshot.documents[0].text, "hello world");
        assert!(manager.autosave_epoch().is_some());

        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn restore_autosave_rehydrates_documents() {
        let cfg = config_path("autosave_restore");
        {
            let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
            let summary = manager.new_document("Draft");
            manager.update_text(summary.id, "draft text").unwrap();
        }

        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let restored = manager.restore_autosave().unwrap();
        assert!(restored);
        let documents = manager.documents();
        assert_eq!(documents.len(), 1);
        let doc = &documents[0];
        assert_eq!(doc.title, "Draft");
        let text = manager.document_text(doc.id).unwrap();
        assert_eq!(text, "draft text");
        assert!(manager.autosave_epoch().is_some());

        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn convert_line_endings_updates_buffer() {
        let cfg = config_path("line_endings");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let summary = manager.new_document("LineTest");
        manager.update_text(summary.id, "alpha\nbeta").unwrap();

        let updated = manager
            .set_document_line_ending(summary.id, LineEnding::Crlf)
            .unwrap();
        assert_eq!(updated.line_ending, LineEnding::Crlf);
        let text = manager.document_text(summary.id).unwrap();
        assert_eq!(text, "alpha\r\nbeta");

        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn reload_document_with_forced_encoding() {
        let cfg = config_path("encoding_reload");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let path = cfg.with_file_name("latin1.txt");
        let bytes = [0x63_u8, 0x61, 0x66, 0xE9]; // café in ISO-8859-1
        fs::write(&path, bytes).unwrap();

        let summary = manager.open_document(&path).unwrap();
        assert_eq!(summary.encoding, TextEncoding::Iso8859_1);

        manager
            .set_document_encoding(summary.id, TextEncoding::Utf8)
            .unwrap();
        let reloaded = manager
            .reload_document_with_encoding(summary.id, TextEncoding::Iso8859_1)
            .unwrap();
        assert_eq!(reloaded.encoding, TextEncoding::Iso8859_1);
        assert!(!reloaded.dirty);
        let text = manager.document_text(summary.id).unwrap();
        assert_eq!(text, "café");

        fs::remove_file(&path).ok();
        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn large_files_are_rejected() {
        let cfg = config_path("large_guard");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let big_path = cfg.with_file_name("big.txt");
        let large_buffer = vec![b'a'; (MAX_FILE_SIZE_BYTES + 1) as usize];
        fs::write(&big_path, &large_buffer).unwrap();

        let result = manager.open_document(&big_path);
        assert!(matches!(
            result,
            Err(DocumentError::FileTooLarge { path, size, limit })
            if path == big_path && size == (MAX_FILE_SIZE_BYTES + 1) && limit == MAX_FILE_SIZE_BYTES
        ));

        fs::remove_file(&big_path).ok();
        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn find_update_and_navigation_behave_as_expected() {
        let cfg = config_path("find_nav");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let summary = manager.new_document("FindNav");
        manager.update_text(summary.id, "one two one two").unwrap();

        let snapshot = manager
            .find_update(summary.id, "one", FindOptions::default())
            .unwrap();
        assert_eq!(snapshot.matches.len(), 2);
        assert_eq!(snapshot.current_index, Some(0));

        let next = manager.find_step(summary.id, false, false).unwrap();
        assert_eq!(next.current_index, Some(1));
        assert!(next.message.is_none());

        let end = manager.find_step(summary.id, false, false).unwrap();
        assert_eq!(end.current_index, Some(1));
        assert_eq!(end.message.as_deref(), Some("Reached end of document"));

        manager.clear_find_state(summary.id);

        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn replace_current_and_all_update_document_text() {
        let cfg = config_path("replace_ops");
        let mut manager = DocumentManager::with_config_path(cfg.clone()).unwrap();
        let summary = manager.new_document("ReplaceOps");
        manager.update_text(summary.id, "red red blue").unwrap();

        manager
            .find_update(summary.id, "red", FindOptions::default())
            .unwrap();

        let replace_once = manager
            .replace_current(summary.id, "green", false, false)
            .unwrap();
        assert_eq!(manager.document_text(summary.id).unwrap(), "green red blue");
        assert_eq!(replace_once.current_index, Some(0));
        assert!(
            replace_once
                .message
                .as_deref()
                .unwrap()
                .contains("Replaced 1 occurrence")
        );

        let replace_all_snapshot = manager.replace_all(summary.id, "yellow").unwrap();
        assert_eq!(
            manager.document_text(summary.id).unwrap(),
            "green yellow blue"
        );
        assert!(
            replace_all_snapshot
                .message
                .as_deref()
                .unwrap()
                .contains("Replaced 1 occurrence")
        );
        assert!(replace_all_snapshot.matches.is_empty());

        fs::remove_file(&cfg).ok();
        cleanup_autosave(&cfg);
    }

    #[test]
    fn theme_mode_string_round_trip_matches_serde() {
        let all = [
            ThemeMode::Light,
            ThemeMode::Dark,
            ThemeMode::System,
            ThemeMode::TokyoNight,
            ThemeMode::TokyoNightStorm,
            ThemeMode::TokyoNightMoon,
        ];
        for mode in all {
            // as_str must agree with the serde snake_case representation.
            let serde_repr = serde_json::to_string(&mode).unwrap();
            assert_eq!(serde_repr, format!("\"{}\"", mode.as_str()));
            // and parsing the identifier back must yield the original variant.
            assert_eq!(ThemeMode::from_str_lenient(mode.as_str()), mode);
        }
    }

    #[test]
    fn theme_mode_from_str_lenient_defaults_to_system() {
        assert_eq!(ThemeMode::from_str_lenient("bogus"), ThemeMode::System);
        assert_eq!(ThemeMode::from_str_lenient("  Dark "), ThemeMode::Dark);
    }
}
