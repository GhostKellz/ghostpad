use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use ghostpad_core::{
    APP_ID, APP_NAME, APP_VERSION, DocumentError, DocumentManager, DocumentSummary, EditorSettings,
    FindDefaults, FindOptions, FindSnapshot, IndentType, LineEnding, RecentDocument, TextEncoding,
    ThemeMode, UiSettings, WindowState, default_welcome_context,
};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Serialize)]
struct BackendState {
    active_id: Option<u64>,
    documents: Vec<DocumentEntry>,
    recent: Vec<RecentEntry>,
    active_text: Option<String>,
    error: Option<String>,
    pending_close: Option<PendingClose>,
    autosave_epoch: Option<u64>,
    find: Option<FindSnapshot>,
    settings: Option<SettingsSnapshot>,
}

#[derive(Serialize)]
struct DocumentEntry {
    id: u64,
    title: String,
    dirty: bool,
    path: Option<String>,
    encoding: String,
    line_ending: String,
    read_only: bool,
    editing_locked: bool,
    externally_modified: bool,
    externally_deleted: bool,
}

#[derive(Serialize)]
struct RecentEntry {
    path: String,
    title: String,
    last_opened_epoch: u64,
    encoding: String,
    line_ending: String,
}

#[derive(Serialize)]
struct PendingClose {
    id: u64,
    title: String,
    path: Option<String>,
}

#[derive(Serialize)]
struct SettingsSnapshot {
    ui: UiSettingsEntry,
    editor: EditorSettingsEntry,
    find_defaults: FindDefaultsEntry,
    window: WindowStateEntry,
}

#[derive(Serialize)]
struct UiSettingsEntry {
    word_wrap_enabled: bool,
    translucency_enabled: bool,
    shadow_enabled: bool,
    theme: String,
}

#[derive(Serialize)]
struct EditorSettingsEntry {
    font_family: String,
    font_size: u32,
    tab_stop_distance: u32,
    indent_type: String,
    indent_size: u32,
}

#[derive(Serialize)]
struct FindDefaultsEntry {
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
    wrap_around: bool,
}

#[derive(Serialize)]
struct WindowStateEntry {
    width: u32,
    height: u32,
    x: Option<i32>,
    y: Option<i32>,
}

impl From<&UiSettings> for UiSettingsEntry {
    fn from(s: &UiSettings) -> Self {
        Self {
            word_wrap_enabled: s.word_wrap_enabled,
            translucency_enabled: s.translucency_enabled,
            shadow_enabled: s.shadow_enabled,
            theme: s.theme.as_str().to_string(),
        }
    }
}

impl From<&EditorSettings> for EditorSettingsEntry {
    fn from(s: &EditorSettings) -> Self {
        Self {
            font_family: s.font_family.clone(),
            font_size: s.font_size,
            tab_stop_distance: s.tab_stop_distance,
            indent_type: format!("{:?}", s.indent_type).to_lowercase(),
            indent_size: s.indent_size,
        }
    }
}

impl From<&FindDefaults> for FindDefaultsEntry {
    fn from(s: &FindDefaults) -> Self {
        Self {
            case_sensitive: s.case_sensitive,
            whole_word: s.whole_word,
            use_regex: s.use_regex,
            wrap_around: s.wrap_around,
        }
    }
}

impl From<&WindowState> for WindowStateEntry {
    fn from(s: &WindowState) -> Self {
        Self {
            width: s.width,
            height: s.height,
            x: s.x,
            y: s.y,
        }
    }
}

pub struct BackendRust {
    manager: RefCell<DocumentManager>,
}

impl Default for BackendRust {
    fn default() -> Self {
        let mut manager = DocumentManager::new().unwrap_or_else(|_| {
            DocumentManager::with_config_path(fallback_config_path())
                .expect("failed to construct document manager")
        });
        if let Err(err) = manager.restore_autosave() {
            eprintln!("failed to restore autosave snapshot: {err}");
        }
        if manager.documents().is_empty() {
            manager.new_document("Untitled");
        }
        manager.autosave().ok();
        Self {
            manager: RefCell::new(manager),
        }
    }
}

impl BackendRust {
    fn state_response(
        &self,
        include_text: bool,
        error: Option<String>,
        pending_close: Option<PendingClose>,
    ) -> QString {
        let payload = self.build_state(include_text, error, pending_close);
        let json = serde_json::to_string(&payload)
            .unwrap_or_else(|_| "{\"error\":\"state serialization failed\"}".to_string());
        QString::from(json)
    }

    fn build_state(
        &self,
        include_text: bool,
        error: Option<String>,
        pending_close: Option<PendingClose>,
    ) -> BackendState {
        self.build_state_with_settings(include_text, false, error, pending_close)
    }

    fn build_state_with_settings(
        &self,
        include_text: bool,
        include_settings: bool,
        error: Option<String>,
        pending_close: Option<PendingClose>,
    ) -> BackendState {
        let (active_id, active_text, autosave_epoch, find_state, settings) = {
            let manager = self.manager.borrow();
            let active_id = manager.active_document_id();
            let active_text = if include_text {
                active_id.and_then(|id| manager.document_text(id).ok())
            } else {
                None
            };
            let autosave_epoch = manager.autosave_epoch();
            let find_state = manager.active_find_snapshot();
            let settings = if include_settings {
                Some(SettingsSnapshot {
                    ui: manager.ui_settings().into(),
                    editor: manager.editor_settings().into(),
                    find_defaults: manager.find_defaults().into(),
                    window: manager.window_state().into(),
                })
            } else {
                None
            };
            (active_id, active_text, autosave_epoch, find_state, settings)
        };

        BackendState {
            active_id,
            documents: self.document_entries(),
            recent: self.recent_entries(),
            active_text,
            error,
            pending_close,
            autosave_epoch,
            find: find_state,
            settings,
        }
    }

    fn document_entries(&self) -> Vec<DocumentEntry> {
        self.manager
            .borrow()
            .documents()
            .into_iter()
            .map(DocumentEntry::from)
            .collect()
    }

    fn recent_entries(&self) -> Vec<RecentEntry> {
        self.manager
            .borrow()
            .recent_documents()
            .into_iter()
            .map(RecentEntry::from)
            .collect()
    }

    fn next_untitled_title(&self) -> String {
        let titles: HashSet<String> = self
            .manager
            .borrow()
            .documents()
            .into_iter()
            .map(|summary| summary.title)
            .collect();

        for index in 1.. {
            let candidate = if index == 1 {
                "Untitled".to_string()
            } else {
                format!("Untitled {index}")
            };
            if !titles.contains(&candidate) {
                return candidate;
            }
        }
        unreachable!("untitled title generation loop should never exhaust");
    }

    fn parse_encoding(value: &str) -> Option<TextEncoding> {
        match value {
            "Utf8" | "UTF-8" => Some(TextEncoding::Utf8),
            "Utf16Le" | "UTF-16LE" | "UTF-16 LE" => Some(TextEncoding::Utf16Le),
            "Utf16Be" | "UTF-16BE" | "UTF-16 BE" => Some(TextEncoding::Utf16Be),
            "Iso8859_1" | "ISO-8859-1" | "Latin1" => Some(TextEncoding::Iso8859_1),
            _ => None,
        }
    }

    fn parse_line_ending(value: &str) -> Option<LineEnding> {
        match value {
            "Lf" | "LF" => Some(LineEnding::Lf),
            "Crlf" | "CRLF" => Some(LineEnding::Crlf),
            _ => None,
        }
    }

    fn build_find_options(case_sensitive: bool, whole_word: bool, use_regex: bool) -> FindOptions {
        FindOptions {
            case_sensitive,
            whole_word,
            use_regex,
        }
    }
}

fn fallback_config_path() -> PathBuf {
    std::env::temp_dir().join("ghostpad_config.json")
}

fn to_local_path(input: &str) -> String {
    if let Some(stripped) = input.strip_prefix("file://") {
        if cfg!(windows) {
            stripped.trim_start_matches('/').to_string()
        } else {
            stripped.to_string()
        }
    } else {
        input.to_string()
    }
}

impl From<DocumentSummary> for DocumentEntry {
    fn from(summary: DocumentSummary) -> Self {
        let path = summary.path.map(|p| p.to_string_lossy().to_string());
        Self {
            id: summary.id,
            title: summary.title,
            dirty: summary.dirty,
            path,
            encoding: format!("{:?}", summary.encoding),
            line_ending: format!("{:?}", summary.line_ending),
            read_only: summary.read_only,
            editing_locked: summary.editing_locked,
            externally_modified: summary.externally_modified,
            externally_deleted: summary.externally_deleted,
        }
    }
}

impl From<RecentDocument> for RecentEntry {
    fn from(recent: RecentDocument) -> Self {
        Self {
            path: recent.path.to_string_lossy().to_string(),
            title: recent.title,
            last_opened_epoch: recent.last_opened_epoch,
            encoding: format!("{:?}", recent.encoding),
            line_ending: format!("{:?}", recent.line_ending),
        }
    }
}

impl From<DocumentSummary> for PendingClose {
    fn from(summary: DocumentSummary) -> Self {
        Self {
            id: summary.id,
            title: summary.title,
            path: summary.path.map(|p| p.to_string_lossy().to_string()),
        }
    }
}

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn bootstrap(self: &Backend) -> QString;
        #[qinvokable]
        fn new_document(self: &Backend) -> QString;
        #[qinvokable]
        fn open_document(self: &Backend, path: QString) -> QString;
        #[qinvokable]
        fn save_active(self: &Backend) -> QString;
        #[qinvokable]
        fn save_active_as(self: &Backend, path: QString) -> QString;
        #[qinvokable]
        fn set_active_document(self: &Backend, id: u64) -> QString;
        #[qinvokable]
        fn update_active_text(self: &Backend, text: QString) -> QString;
        #[qinvokable]
        fn close_document(self: &Backend, id: u64) -> QString;
        #[qinvokable]
        fn force_close_document(self: &Backend, id: u64) -> QString;
        #[qinvokable]
        fn state(self: &Backend) -> QString;
        #[qinvokable]
        fn autosave(self: &Backend) -> QString;
        #[qinvokable]
        fn set_active_encoding(self: &Backend, encoding: QString) -> QString;
        #[qinvokable]
        fn reload_active_with_encoding(self: &Backend, encoding: QString) -> QString;
        #[qinvokable]
        fn set_active_line_ending(self: &Backend, line_ending: QString) -> QString;
        #[qinvokable]
        fn set_active_edit_override(self: &Backend, allow_edit: bool) -> QString;
        #[qinvokable]
        fn begin_find(
            self: &Backend,
            query: QString,
            case_sensitive: bool,
            whole_word: bool,
            use_regex: bool,
        ) -> QString;
        #[qinvokable]
        fn find_next(self: &Backend, wrap: bool) -> QString;
        #[qinvokable]
        fn find_previous(self: &Backend, wrap: bool) -> QString;
        #[qinvokable]
        fn replace_current(
            self: &Backend,
            replacement: QString,
            wrap: bool,
            backwards: bool,
        ) -> QString;
        #[qinvokable]
        fn replace_all(self: &Backend, replacement: QString) -> QString;
        #[qinvokable]
        fn clear_find(self: &Backend) -> QString;

        #[qinvokable]
        fn welcome_headline(self: &Backend) -> String;
        #[qinvokable]
        fn welcome_tagline(self: &Backend) -> String;
        #[qinvokable]
        fn app_name(self: &Backend) -> String;
        #[qinvokable]
        fn app_version(self: &Backend) -> String;
        #[qinvokable]
        fn app_id(self: &Backend) -> String;

        // Settings
        #[qinvokable]
        fn get_settings(self: &Backend) -> QString;
        #[qinvokable]
        fn update_ui_settings(
            self: &Backend,
            word_wrap: bool,
            translucency: bool,
            shadow: bool,
            theme: QString,
        ) -> QString;
        #[qinvokable]
        fn update_editor_settings(
            self: &Backend,
            font_family: QString,
            font_size: u32,
            tab_stop: u32,
            indent_type: QString,
            indent_size: u32,
        ) -> QString;
        #[qinvokable]
        fn update_find_defaults(
            self: &Backend,
            case_sensitive: bool,
            whole_word: bool,
            use_regex: bool,
            wrap_around: bool,
        ) -> QString;
        #[qinvokable]
        fn update_window_state(self: &Backend, width: u32, height: u32, x: i32, y: i32) -> QString;

        // File watching
        #[qinvokable]
        fn poll_file_events(self: &Backend) -> QString;
        #[qinvokable]
        fn reload_document(self: &Backend, id: u64) -> QString;
        #[qinvokable]
        fn dismiss_external_change(self: &Backend, id: u64) -> QString;
    }
}

impl qobject::Backend {
    fn perform_find_step(&self, wrap: bool, backwards: bool) -> QString {
        let backend = self.rust();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.find_step(id, wrap, backwards)
                };
                match result {
                    Ok(_) => backend.state_response(false, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn bootstrap(&self) -> QString {
        let backend = self.rust();
        // Include settings in bootstrap response
        let payload = backend.build_state_with_settings(true, true, None, None);
        let json = serde_json::to_string(&payload)
            .unwrap_or_else(|_| "{\"error\":\"state serialization failed\"}".to_string());
        QString::from(json)
    }

    fn new_document(&self) -> QString {
        let backend = self.rust();
        let title = backend.next_untitled_title();
        {
            let mut manager = backend.manager.borrow_mut();
            manager.new_document(title);
        }
        backend.state_response(true, None, None)
    }

    fn open_document(&self, path: QString) -> QString {
        let backend = self.rust();
        let path_string = crate::bridge::to_local_path(&path.to_string());
        let result = {
            let mut manager = backend.manager.borrow_mut();
            manager.open_document(&path_string)
        };
        match result {
            Ok(_) => backend.state_response(true, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn save_active(&self) -> QString {
        let backend = self.rust();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.save_document(id)
                };
                match result {
                    Ok(_) => backend.state_response(false, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn save_active_as(&self, path: QString) -> QString {
        let backend = self.rust();
        let path_string = crate::bridge::to_local_path(&path.to_string());
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.save_document_as(id, &path_string)
                };
                match result {
                    Ok(_) => backend.state_response(false, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn set_active_document(&self, id: u64) -> QString {
        let backend = self.rust();
        let result = {
            let mut manager = backend.manager.borrow_mut();
            manager.set_active_document(id)
        };
        match result {
            Ok(_) => backend.state_response(true, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn update_active_text(&self, text: QString) -> QString {
        let backend = self.rust();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.update_text(id, text.to_string())
                };
                match result {
                    Ok(_) => backend.state_response(false, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn close_document(&self, id: u64) -> QString {
        let backend = self.rust();
        let result = {
            let mut manager = backend.manager.borrow_mut();
            manager.close_document(id, false)
        };
        match result {
            Ok(_) => backend.state_response(true, None, None),
            Err(DocumentError::DocumentDirty(summary)) => {
                backend.state_response(false, None, Some(summary.into()))
            }
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn force_close_document(&self, id: u64) -> QString {
        let backend = self.rust();
        let result = {
            let mut manager = backend.manager.borrow_mut();
            manager.close_document(id, true)
        };
        match result {
            Ok(_) => backend.state_response(true, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn state(&self) -> QString {
        self.rust().state_response(true, None, None)
    }

    fn autosave(&self) -> QString {
        let backend = self.rust();
        let result = {
            let manager = backend.manager.borrow();
            manager.autosave()
        };
        match result {
            Ok(_) => backend.state_response(false, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn set_active_encoding(&self, encoding: QString) -> QString {
        let backend = self.rust();
        let encoding_string = encoding.to_string();
        let Some(value) = BackendRust::parse_encoding(&encoding_string) else {
            return backend.state_response(
                false,
                Some(format!("unsupported encoding '{encoding_string}'")),
                None,
            );
        };
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.set_document_encoding(id, value)
                };
                match result {
                    Ok(_) => backend.state_response(true, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn reload_active_with_encoding(&self, encoding: QString) -> QString {
        let backend = self.rust();
        let encoding_string = encoding.to_string();
        let Some(value) = BackendRust::parse_encoding(&encoding_string) else {
            return backend.state_response(
                false,
                Some(format!("unsupported encoding '{encoding_string}'")),
                None,
            );
        };
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.reload_document_with_encoding(id, value)
                };
                match result {
                    Ok(_) => backend.state_response(true, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn set_active_line_ending(&self, line_ending: QString) -> QString {
        let backend = self.rust();
        let line_ending_string = line_ending.to_string();
        let Some(value) = BackendRust::parse_line_ending(&line_ending_string) else {
            return backend.state_response(
                false,
                Some(format!("unsupported line ending '{line_ending_string}'")),
                None,
            );
        };
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.set_document_line_ending(id, value)
                };
                match result {
                    Ok(_) => backend.state_response(true, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn set_active_edit_override(&self, allow_edit: bool) -> QString {
        let backend = self.rust();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.set_read_only_override(id, allow_edit)
                };
                match result {
                    Ok(_) => backend.state_response(false, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn begin_find(
        &self,
        query: QString,
        case_sensitive: bool,
        whole_word: bool,
        use_regex: bool,
    ) -> QString {
        let backend = self.rust();
        let options = BackendRust::build_find_options(case_sensitive, whole_word, use_regex);
        let query_string = query.to_string();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.find_update(id, query_string, options)
                };
                match result {
                    Ok(_) => backend.state_response(false, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn find_next(&self, wrap: bool) -> QString {
        self.perform_find_step(wrap, false)
    }

    fn find_previous(&self, wrap: bool) -> QString {
        self.perform_find_step(wrap, true)
    }

    fn replace_current(&self, replacement: QString, wrap: bool, backwards: bool) -> QString {
        let backend = self.rust();
        let replacement_string = replacement.to_string();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.replace_current(id, replacement_string, wrap, backwards)
                };
                match result {
                    Ok(_) => backend.state_response(true, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn replace_all(&self, replacement: QString) -> QString {
        let backend = self.rust();
        let replacement_string = replacement.to_string();
        let active_id = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        };
        match active_id {
            Some(id) => {
                let result = {
                    let mut manager = backend.manager.borrow_mut();
                    manager.replace_all(id, replacement_string)
                };
                match result {
                    Ok(_) => backend.state_response(true, None, None),
                    Err(err) => backend.state_response(false, Some(err.to_string()), None),
                }
            }
            None => backend.state_response(false, Some("no active document".to_string()), None),
        }
    }

    fn clear_find(&self) -> QString {
        let backend = self.rust();
        if let Some(active_id) = {
            let manager = backend.manager.borrow();
            manager.active_document_id()
        } {
            let mut manager = backend.manager.borrow_mut();
            manager.clear_find_state(active_id);
        }
        backend.state_response(false, None, None)
    }

    fn welcome_headline(&self) -> String {
        default_welcome_context().headline.to_string()
    }

    fn welcome_tagline(&self) -> String {
        default_welcome_context().tagline.to_string()
    }

    fn app_name(&self) -> String {
        APP_NAME.to_string()
    }

    fn app_version(&self) -> String {
        APP_VERSION.to_string()
    }

    fn app_id(&self) -> String {
        APP_ID.to_string()
    }

    // =========================================================================
    // Settings bridge functions
    // =========================================================================

    fn get_settings(&self) -> QString {
        let backend = self.rust();
        let payload = backend.build_state_with_settings(true, true, None, None);
        let json = serde_json::to_string(&payload)
            .unwrap_or_else(|_| "{\"error\":\"settings serialization failed\"}".to_string());
        QString::from(json)
    }

    fn update_ui_settings(
        &self,
        word_wrap: bool,
        translucency: bool,
        shadow: bool,
        theme: QString,
    ) -> QString {
        let backend = self.rust();
        let theme_mode = ThemeMode::from_str_lenient(&theme.to_string());
        let settings = UiSettings {
            word_wrap_enabled: word_wrap,
            translucency_enabled: translucency,
            shadow_enabled: shadow,
            theme: theme_mode,
        };
        match backend.manager.borrow_mut().update_ui_settings(settings) {
            Ok(()) => backend.state_response(false, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn update_editor_settings(
        &self,
        font_family: QString,
        font_size: u32,
        tab_stop: u32,
        indent_type: QString,
        indent_size: u32,
    ) -> QString {
        let backend = self.rust();
        let indent = match indent_type.to_string().to_lowercase().as_str() {
            "tabs" => IndentType::Tabs,
            _ => IndentType::Spaces,
        };
        let settings = EditorSettings {
            font_family: font_family.to_string(),
            font_size,
            tab_stop_distance: tab_stop,
            indent_type: indent,
            indent_size,
        };
        match backend
            .manager
            .borrow_mut()
            .update_editor_settings(settings)
        {
            Ok(()) => backend.state_response(false, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn update_find_defaults(
        &self,
        case_sensitive: bool,
        whole_word: bool,
        use_regex: bool,
        wrap_around: bool,
    ) -> QString {
        let backend = self.rust();
        let defaults = FindDefaults {
            case_sensitive,
            whole_word,
            use_regex,
            wrap_around,
        };
        match backend.manager.borrow_mut().update_find_defaults(defaults) {
            Ok(()) => backend.state_response(false, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn update_window_state(&self, width: u32, height: u32, x: i32, y: i32) -> QString {
        let backend = self.rust();
        let state = WindowState {
            width,
            height,
            x: Some(x),
            y: Some(y),
        };
        match backend.manager.borrow_mut().update_window_state(state) {
            Ok(()) => backend.state_response(false, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    // =========================================================================
    // File watching bridge functions
    // =========================================================================

    fn poll_file_events(&self) -> QString {
        let backend = self.rust();
        let _affected_ids = {
            let mut manager = backend.manager.borrow_mut();
            manager.poll_file_events()
        };
        // Return updated state which will include the modified flags
        backend.state_response(false, None, None)
    }

    fn reload_document(&self, id: u64) -> QString {
        let backend = self.rust();
        let result = {
            let mut manager = backend.manager.borrow_mut();
            manager.reload_document(id)
        };
        match result {
            Ok(_) => backend.state_response(true, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }

    fn dismiss_external_change(&self, id: u64) -> QString {
        let backend = self.rust();
        let result = {
            let mut manager = backend.manager.borrow_mut();
            manager.acknowledge_external_change(id)
        };
        match result {
            Ok(_) => backend.state_response(false, None, None),
            Err(err) => backend.state_response(false, Some(err.to_string()), None),
        }
    }
}

pub use qobject::Backend;
