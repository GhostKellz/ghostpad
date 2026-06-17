//! Core logic for GhostPad.

pub mod app_info;
pub mod document;
pub mod document_manager;
pub mod file_watcher;
pub mod welcome;

pub use app_info::{APP_ID, APP_NAME, APP_VERSION};
pub use document::{
    Document, DocumentError, DocumentSummary, FindOptions, LineEnding, MatchSpan, TextEncoding,
};
pub use document_manager::{
    DocumentManager, EditorSettings, FindDefaults, FindMatch, FindSnapshot, IndentType,
    RecentDocument, ThemeMode, UiSettings, WindowState,
};
pub use welcome::{WelcomeContext, default_welcome_context};
