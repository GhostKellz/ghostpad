//! Document model and helpers for GhostPad.

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Snapshot of file metadata used to detect external modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskFingerprint {
    /// Last modification time of the file.
    pub modified: SystemTime,
    /// Size of the file in bytes.
    pub size: u64,
    /// Whether the file is read-only.
    pub read_only: bool,
}

impl DiskFingerprint {
    /// Create a fingerprint from filesystem metadata, if available.
    pub fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        let modified = metadata.modified().ok()?;
        let size = metadata.len();
        let read_only = metadata.permissions().readonly();
        Some(Self {
            modified,
            size,
            read_only,
        })
    }
}

/// Supported text encodings for documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum TextEncoding {
    /// UTF-8 without BOM
    #[default]
    Utf8,
    /// UTF-16 Little Endian with BOM
    Utf16Le,
    /// UTF-16 Big Endian with BOM
    Utf16Be,
    /// ISO-8859-1 (Latin-1)
    Iso8859_1,
}

/// Line ending style stored in a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum LineEnding {
    /// Unix line endings (`\n`).
    #[default]
    Lf,
    /// Windows line endings (`\r\n`).
    Crlf,
}

/// Options controlling how text search behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FindOptions {
    /// Treat the query as case-sensitive when true.
    pub case_sensitive: bool,
    /// Restrict matches to whole-word boundaries.
    pub whole_word: bool,
    /// Interpret the query as a regular expression when true.
    pub use_regex: bool,
}

/// Errors that can occur when working with a document.
#[derive(Debug)]
pub enum DocumentError {
    /// Underlying IO error.
    Io(io::Error),
    /// The document encoding is not supported yet.
    UnsupportedEncoding(&'static str),
    /// The document data could not be decoded.
    InvalidData(String),
    /// Saving requires a file path.
    MissingPath,
    /// Requested document could not be located.
    DocumentNotFound(u64),
    /// Attempted to close a document that still has unsaved changes.
    DocumentDirty(DocumentSummary),
    /// Refused to open a document that exceeds the configured size guard.
    FileTooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
    /// The supplied search pattern could not be parsed.
    InvalidSearchPattern(String),
    /// Attempted to modify a document that is locked for editing.
    ReadOnlyDocument(String),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Io(err) => write!(f, "io error: {err}"),
            DocumentError::UnsupportedEncoding(enc) => {
                write!(f, "unsupported encoding: {enc}")
            }
            DocumentError::InvalidData(reason) => write!(f, "invalid data: {reason}"),
            DocumentError::MissingPath => write!(f, "no file path associated with the document"),
            DocumentError::DocumentNotFound(id) => write!(f, "no document with id {id}"),
            DocumentError::DocumentDirty(summary) => {
                write!(f, "document '{}' has unsaved changes", summary.title)
            }
            DocumentError::FileTooLarge { path, size, limit } => {
                let size_mb = (*size as f64) / (1024.0 * 1024.0);
                let limit_mb = (*limit as f64) / (1024.0 * 1024.0);
                write!(
                    f,
                    "{} is {:.1} MiB which exceeds the {:.1} MiB safety limit",
                    path.display(),
                    size_mb,
                    limit_mb
                )
            }
            DocumentError::InvalidSearchPattern(reason) => {
                write!(f, "invalid search pattern: {reason}")
            }
            DocumentError::ReadOnlyDocument(title) => {
                write!(
                    f,
                    "document '{title}' is read-only; enable editing to proceed"
                )
            }
        }
    }
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DocumentError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for DocumentError {
    fn from(err: io::Error) -> Self {
        DocumentError::Io(err)
    }
}

/// Represents a text buffer and metadata tracked by the application.
#[derive(Debug, Clone)]
pub struct Document {
    id: u64,
    title: String,
    text: String,
    path: Option<PathBuf>,
    encoding: TextEncoding,
    line_ending: LineEnding,
    dirty: bool,
    last_modified: Option<SystemTime>,
    last_size: Option<u64>,
    read_only: bool,
    edit_override: bool,
    externally_modified: bool,
    externally_deleted: bool,
}

impl Document {
    /// Creates a new unsaved document with default metadata.
    pub fn new_empty(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            text: String::new(),
            path: None,
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            dirty: false,
            last_modified: None,
            last_size: None,
            read_only: false,
            edit_override: false,
            externally_modified: false,
            externally_deleted: false,
        }
    }

    /// Opens a document from the given path.
    pub fn open(id: u64, path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path_ref = path.as_ref();
        let mut file = fs::File::open(path_ref)?;
        let metadata = file.metadata().ok();
        let fingerprint = metadata.as_ref().and_then(DiskFingerprint::from_metadata);

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let (encoding, text) = decode_bytes(&bytes)?;
        let line_ending = detect_line_ending(&text);

        let mut document = Self {
            id,
            title: document_title_from_path(path_ref),
            text,
            path: Some(path_ref.to_path_buf()),
            encoding,
            line_ending,
            dirty: false,
            last_modified: None,
            last_size: None,
            read_only: false,
            edit_override: false,
            externally_modified: false,
            externally_deleted: false,
        };

        if let Some(fingerprint) = fingerprint {
            document.apply_fingerprint(&fingerprint);
        }

        Ok(document)
    }

    /// Saves the document to its current path.
    pub fn save(&mut self) -> Result<(), DocumentError> {
        let Some(path) = self.path.clone() else {
            return Err(DocumentError::MissingPath);
        };
        self.save_as(path)
    }

    /// Saves the document to the given path, updating tracked metadata.
    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), DocumentError> {
        let path_ref = path.as_ref();
        let mut file = fs::File::create(path_ref)?;
        let encoded = encode_text(&self.text, self.encoding)?;
        file.write_all(&encoded)?;
        file.sync_all().ok();

        self.path = Some(path_ref.to_path_buf());
        self.title = document_title_from_path(path_ref);
        self.dirty = false;
        match file.metadata() {
            Ok(meta) => {
                if let Some(fingerprint) = DiskFingerprint::from_metadata(&meta) {
                    self.apply_fingerprint(&fingerprint);
                } else {
                    self.last_modified = None;
                    self.last_size = None;
                    self.update_read_only_flag(meta.permissions().readonly());
                }
            }
            Err(_) => {
                self.last_modified = None;
                self.last_size = None;
                self.update_read_only_flag(false);
            }
        }

        Ok(())
    }

    /// Attaches the document to a new path without writing to disk.
    pub fn set_path(&mut self, path: impl AsRef<Path>) {
        let path_ref = path.as_ref();
        self.path = Some(path_ref.to_path_buf());
        self.title = document_title_from_path(path_ref);
        self.last_modified = None;
        self.last_size = None;
        self.read_only = false;
        self.edit_override = false;
    }

    /// Updates the text buffer and marks the document dirty.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.line_ending = detect_line_ending(&self.text);
        self.dirty = true;
    }

    /// Returns true if the file backing this document is read-only.
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns true if a user override has been enabled for editing.
    pub fn edit_override(&self) -> bool {
        self.edit_override
    }

    /// Returns true when edits should be blocked because the file is read-only.
    pub fn editing_locked(&self) -> bool {
        self.read_only && !self.edit_override
    }

    /// Sets whether the user has opted into overriding the read-only lock.
    pub fn set_edit_override(&mut self, enabled: bool) {
        if self.read_only {
            self.edit_override = enabled;
        } else {
            self.edit_override = false;
        }
    }

    /// Returns the cached disk fingerprint for this document if available.
    pub fn disk_fingerprint(&self) -> Option<DiskFingerprint> {
        Some(DiskFingerprint {
            modified: self.last_modified?,
            size: self.last_size?,
            read_only: self.read_only,
        })
    }

    /// Applies a new disk fingerprint and reconciles read-only overrides.
    pub fn apply_fingerprint(&mut self, fingerprint: &DiskFingerprint) {
        self.last_modified = Some(fingerprint.modified);
        self.last_size = Some(fingerprint.size);
        self.update_read_only_flag(fingerprint.read_only);
    }

    /// Updates the in-memory read-only flag without touching cached metadata.
    pub fn update_read_only_flag(&mut self, read_only: bool) {
        self.read_only = read_only;
        if !self.read_only {
            self.edit_override = false;
        }
    }

    /// Refreshes the tracked read-only flag from filesystem metadata.
    pub fn refresh_read_only(&mut self) {
        if let Some(path) = &self.path
            && let Ok(metadata) = fs::metadata(path)
        {
            if let Some(fingerprint) = DiskFingerprint::from_metadata(&metadata) {
                self.apply_fingerprint(&fingerprint);
            } else {
                self.update_read_only_flag(metadata.permissions().readonly());
            }
        }
    }

    /// Marks the document as clean without touching the file system.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Change the tracked encoding.
    pub fn set_encoding(&mut self, encoding: TextEncoding) {
        if self.encoding != encoding {
            self.encoding = encoding;
            self.dirty = true;
        }
    }

    /// Change the tracked line ending preference.
    pub fn set_line_ending(&mut self, line_ending: LineEnding) {
        if self.line_ending != line_ending {
            self.text = convert_line_endings(&self.text, line_ending);
            self.line_ending = line_ending;
            self.dirty = true;
        }
    }

    /// Reload the document contents from disk using the specified encoding.
    pub fn reload_with_encoding(&mut self, encoding: TextEncoding) -> DocumentResult<()> {
        let path = self.path().ok_or(DocumentError::MissingPath)?.to_path_buf();
        let mut file = fs::File::open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let text = decode_bytes_with_encoding(&bytes, encoding)?;

        self.text = text;
        self.encoding = encoding;
        self.line_ending = detect_line_ending(&self.text);
        self.dirty = false;
        if let Ok(metadata) = file.metadata() {
            if let Some(fingerprint) = DiskFingerprint::from_metadata(&metadata) {
                self.apply_fingerprint(&fingerprint);
            } else {
                self.update_read_only_flag(metadata.permissions().readonly());
            }
        }
        Ok(())
    }

    /// Returns the identifier for this document.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns a best-effort display title for UI usage.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the current text buffer.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the optional path backing the document.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the tracked encoding.
    pub fn encoding(&self) -> TextEncoding {
        self.encoding
    }

    /// Returns the tracked line ending style.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Indicates whether the document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the last modification timestamp observed on disk.
    pub fn last_modified(&self) -> Option<SystemTime> {
        self.last_modified
    }

    /// Produce a lightweight summary suitable for UI listings.
    pub fn summary(&self) -> DocumentSummary {
        DocumentSummary {
            id: self.id,
            title: self.title.clone(),
            path: self.path.clone(),
            encoding: self.encoding,
            line_ending: self.line_ending,
            dirty: self.dirty,
            read_only: self.read_only,
            editing_locked: self.editing_locked(),
            externally_modified: self.externally_modified,
            externally_deleted: self.externally_deleted,
        }
    }

    /// Mark the document as externally modified.
    pub fn set_externally_modified(&mut self, modified: bool) {
        self.externally_modified = modified;
    }

    /// Mark the document as externally deleted.
    pub fn set_externally_deleted(&mut self, deleted: bool) {
        self.externally_deleted = deleted;
    }

    /// Check if externally modified.
    pub fn is_externally_modified(&self) -> bool {
        self.externally_modified
    }

    /// Check if externally deleted.
    pub fn is_externally_deleted(&self) -> bool {
        self.externally_deleted
    }

    /// Clear external modification flags (after reload or dismiss).
    pub fn clear_external_flags(&mut self) {
        self.externally_modified = false;
        self.externally_deleted = false;
    }
}

/// Convenience type alias representing the result for document operations.
pub type DocumentResult<T> = Result<T, DocumentError>;

/// High level data exposed to UI listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: u64,
    pub title: String,
    pub path: Option<PathBuf>,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub dirty: bool,
    pub read_only: bool,
    pub editing_locked: bool,
    pub externally_modified: bool,
    pub externally_deleted: bool,
}

/// Represents a single match within a document in both byte and character space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub char_start: usize,
    pub char_end: usize,
}

/// Compiled search data consisting of the regex and all matches for the current text.
#[derive(Debug)]
pub struct CompiledFind {
    pub regex: Regex,
    pub matches: Vec<MatchSpan>,
}

/// Detect the line ending style of a text buffer.
pub(crate) fn detect_line_ending(text: &str) -> LineEnding {
    if text.contains("\r\n") {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

/// Decode raw bytes into a string and inferred encoding.
pub(crate) fn decode_bytes(bytes: &[u8]) -> Result<(TextEncoding, String), DocumentError> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true).map(|text| (TextEncoding::Utf16Le, text));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false).map(|text| (TextEncoding::Utf16Be, text));
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let text = String::from_utf8(bytes[3..].to_vec())
            .map_err(|err| DocumentError::InvalidData(err.to_string()))?;
        return Ok((TextEncoding::Utf8, text));
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => Ok((TextEncoding::Utf8, text)),
        Err(_) => {
            let text = bytes.iter().map(|&b| b as char).collect::<String>();
            Ok((TextEncoding::Iso8859_1, text))
        }
    }
}

/// Encode a string into the requested encoding.
pub(crate) fn encode_text(text: &str, encoding: TextEncoding) -> Result<Vec<u8>, DocumentError> {
    match encoding {
        TextEncoding::Utf8 => Ok(text.as_bytes().to_vec()),
        TextEncoding::Utf16Le => {
            let mut out = Vec::with_capacity(2 + text.len() * 2);
            out.extend_from_slice(&[0xFF, 0xFE]);
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            Ok(out)
        }
        TextEncoding::Utf16Be => {
            let mut out = Vec::with_capacity(2 + text.len() * 2);
            out.extend_from_slice(&[0xFE, 0xFF]);
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            Ok(out)
        }
        TextEncoding::Iso8859_1 => {
            let mut out = Vec::with_capacity(text.len());
            for ch in text.chars() {
                if ch as u32 > 0xFF {
                    return Err(DocumentError::UnsupportedEncoding(
                        "character not representable in ISO-8859-1",
                    ));
                }
                out.push(ch as u8);
            }
            Ok(out)
        }
    }
}

/// Decode raw bytes into a string using a forced encoding.
pub(crate) fn decode_bytes_with_encoding(
    bytes: &[u8],
    encoding: TextEncoding,
) -> Result<String, DocumentError> {
    match encoding {
        TextEncoding::Utf8 => String::from_utf8(bytes.to_vec())
            .map_err(|err| DocumentError::InvalidData(err.to_string())),
        TextEncoding::Utf16Le => decode_utf16(bytes, true),
        TextEncoding::Utf16Be => decode_utf16(bytes, false),
        TextEncoding::Iso8859_1 => Ok(bytes.iter().map(|&b| b as char).collect()),
    }
}

/// Convert the line endings in a text buffer to the requested style.
pub(crate) fn convert_line_endings(text: &str, line_ending: LineEnding) -> String {
    let normalized = text.replace("\r\n", "\n");
    match line_ending {
        LineEnding::Lf => normalized,
        LineEnding::Crlf => normalized.replace('\n', "\r\n"),
    }
}

impl Document {
    /// Compile a search pattern for the document and return all current matches.
    pub(crate) fn compile_find(
        &self,
        query: &str,
        options: &FindOptions,
    ) -> DocumentResult<CompiledFind> {
        let regex = build_find_regex(query, options)?;
        let text = self.text();
        let mut matches = Vec::new();
        for mat in regex.find_iter(text) {
            let byte_start = mat.start();
            let byte_end = mat.end();
            let char_start = count_chars(&text[..byte_start]);
            let char_len = count_chars(&text[byte_start..byte_end]);
            matches.push(MatchSpan {
                byte_start,
                byte_end,
                char_start,
                char_end: char_start + char_len,
            });
        }

        Ok(CompiledFind { regex, matches })
    }
}

fn build_find_regex(query: &str, options: &FindOptions) -> DocumentResult<Regex> {
    if query.is_empty() {
        return Err(DocumentError::InvalidSearchPattern(
            "query cannot be empty".to_string(),
        ));
    }
    let mut pattern = if options.use_regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    if options.whole_word && !pattern.is_empty() {
        pattern = format!(r"\b{pattern}\b");
    }

    let mut builder = RegexBuilder::new(&pattern);
    builder.case_insensitive(!options.case_sensitive);
    builder.multi_line(true);
    builder.dot_matches_new_line(true);
    builder
        .build()
        .map_err(|err| DocumentError::InvalidSearchPattern(err.to_string()))
}

fn count_chars(segment: &str) -> usize {
    segment.chars().count()
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, DocumentError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(DocumentError::InvalidData(
            "utf-16 payload had an odd number of bytes".to_string(),
        ));
    }

    let units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            let array = [chunk[0], chunk[1]];
            if little_endian {
                u16::from_le_bytes(array)
            } else {
                u16::from_be_bytes(array)
            }
        })
        .collect::<Vec<u16>>();

    String::from_utf16(&units).map_err(|err| DocumentError::InvalidData(err.to_string()))
}

fn document_title_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fresh_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ghostpad_{label}_{nanos}"))
    }

    #[test]
    fn new_document_defaults() {
        let doc = Document::new_empty(42, "Untitled");
        assert_eq!(doc.id(), 42);
        assert_eq!(doc.title(), "Untitled");
        assert_eq!(doc.text(), "");
        assert!(doc.path().is_none());
        assert_eq!(doc.encoding(), TextEncoding::Utf8);
        assert_eq!(doc.line_ending(), LineEnding::Lf);
        assert!(!doc.is_dirty());
        assert!(doc.last_modified().is_none());
    }

    #[test]
    fn set_text_marks_dirty_and_updates_line_ending() {
        let mut doc = Document::new_empty(1, "Doc");
        doc.set_text("a\r\nb");
        assert!(doc.is_dirty());
        assert_eq!(doc.line_ending(), LineEnding::Crlf);
    }

    #[test]
    fn mark_clean_clears_dirty_flag() {
        let mut doc = Document::new_empty(1, "Doc");
        doc.set_text("hello");
        assert!(doc.is_dirty());
        doc.mark_clean();
        assert!(!doc.is_dirty());
    }

    #[test]
    fn save_requires_path() {
        let mut doc = Document::new_empty(1, "Doc");
        let err = doc.save().unwrap_err();
        assert!(matches!(err, DocumentError::MissingPath));
    }

    #[test]
    fn save_as_writes_file_and_updates_state() {
        let mut doc = Document::new_empty(1, "Doc");
        doc.set_text("hello world");
        let path = fresh_path("save");

        doc.save_as(&path).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "hello world");
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(doc.title(), path.file_name().unwrap().to_str().unwrap());
        assert!(!doc.is_dirty());
        assert!(doc.last_modified().is_some());

        fs::remove_file(path).ok();
    }

    #[test]
    fn open_reads_utf8_file() {
        let path = fresh_path("open_utf8");
        fs::write(&path, "hello").unwrap();

        let doc = Document::open(7, &path).unwrap();
        assert_eq!(doc.text(), "hello");
        assert_eq!(doc.encoding(), TextEncoding::Utf8);
        assert!(!doc.is_dirty());

        fs::remove_file(path).ok();
    }

    #[test]
    fn open_reads_utf16le_file() {
        let path = fresh_path("open_utf16le");
        let bytes = encode_text("hello", TextEncoding::Utf16Le).unwrap();
        fs::write(&path, &bytes).unwrap();

        let doc = Document::open(7, &path).unwrap();
        assert_eq!(doc.text(), "hello");
        assert_eq!(doc.encoding(), TextEncoding::Utf16Le);

        fs::remove_file(path).ok();
    }

    #[test]
    fn detect_line_ending_variants() {
        assert_eq!(detect_line_ending("foo\nbar"), LineEnding::Lf);
        assert_eq!(detect_line_ending("foo\r\nbar"), LineEnding::Crlf);
    }

    #[test]
    fn roundtrip_utf16be_encoding() {
        let text = "Hello 💖";
        let encoded = encode_text(text, TextEncoding::Utf16Be).unwrap();
        let (encoding, decoded) = decode_bytes(&encoded).unwrap();
        assert_eq!(encoding, TextEncoding::Utf16Be);
        assert_eq!(decoded, text);
    }

    #[test]
    fn decode_iso8859_after_utf8_failure() {
        let bytes = vec![0xC4, 0xE4, 0xF6];
        let (encoding, decoded) = decode_bytes(&bytes).unwrap();
        assert_eq!(encoding, TextEncoding::Iso8859_1);
        assert_eq!(decoded, "Ääö");
    }

    #[test]
    fn iso8859_rejects_extended_chars() {
        let text = "€";
        let result = encode_text(text, TextEncoding::Iso8859_1);
        assert!(matches!(result, Err(DocumentError::UnsupportedEncoding(_))));
    }

    #[test]
    fn summary_reflects_document_state() {
        let mut doc = Document::new_empty(9, "Doc");
        doc.set_text("body");
        doc.set_path("/tmp/demo.txt");
        let summary = doc.summary();
        assert_eq!(summary.id, 9);
        assert_eq!(summary.title, "demo.txt");
        assert_eq!(summary.path.unwrap(), PathBuf::from("/tmp/demo.txt"));
        assert!(summary.dirty);
        assert!(!summary.read_only);
        assert!(!summary.editing_locked);
    }

    #[test]
    fn open_respects_filesystem_read_only() {
        let path = fresh_path("readonly");
        fs::write(&path, "locked").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms.clone()).unwrap();

        let doc = Document::open(42, &path).unwrap();
        assert!(doc.read_only());
        assert!(doc.editing_locked());

        // Restore write permission so the temp file can be deleted; the broad
        // permission change is harmless on a throwaway test fixture.
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&path, perms).ok();
        fs::remove_file(path).ok();
    }

    #[test]
    fn compile_find_literal_case_insensitive() {
        let mut doc = Document::new_empty(1, "Doc");
        doc.set_text("Foo\nfoo\nFOO");
        let options = FindOptions {
            case_sensitive: false,
            ..Default::default()
        };
        let compiled = doc.compile_find("foo", &options).unwrap();
        assert_eq!(compiled.matches.len(), 3);
        assert_eq!(compiled.matches[0].char_start, 0);
        assert_eq!(compiled.matches[2].char_start, 8);
    }

    #[test]
    fn compile_find_respects_whole_word_and_regex() {
        let mut doc = Document::new_empty(1, "Doc");
        doc.set_text("cat scatter catalog\ncat");

        let options = FindOptions {
            case_sensitive: true,
            whole_word: true,
            ..Default::default()
        };
        let compiled = doc.compile_find("cat", &options).unwrap();
        assert_eq!(compiled.matches.len(), 2);
        assert_eq!(compiled.matches[0].char_start, 0);
        assert_eq!(compiled.matches[1].char_start, 20);

        let regex_options = FindOptions {
            use_regex: true,
            ..Default::default()
        };
        let compiled_regex = doc.compile_find("c.t", &regex_options).unwrap();
        assert!(compiled_regex.matches.len() >= 3);
    }
}
