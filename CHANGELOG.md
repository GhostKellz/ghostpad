# Changelog

All notable changes to this project will be documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) starting today.

## [0.1.0] - 2026-06-17
### Added
- Tokyo Night themes (Night, Storm, Moon) alongside System, Light, and Dark, with a theme selector in Settings → Appearance and persistence in `config.json`.
- Documentation tree under `docs/` covering getting started, theming, keyboard shortcuts, KDE integration, the settings schema, and architecture.
- `SECURITY.md` and `CONTRIBUTING.md`.
- Advisory tracking under `docs/advisories/` plus a root `deny.toml` (cargo-deny config).
- Dependabot enabled for the repository (cargo + github-actions, weekly).
- Workspace lint configuration and `rustfmt.toml` (edition 2024); workspace package metadata (MIT license, repository).

### Changed
- Modernized dependencies: `cxx-qt`/`cxx-qt-lib`/`cxx-qt-build` 0.7 → 0.8 (new QML-module registration API), `notify` 6 → 8, `directories` 5 → 6.
- Rewrote `README.md` around accurate feature/theme/tech-stack content and tech badges.

### Removed
- Unused `notify-debouncer-mini` dependency.

### Security
- `cargo audit` and `cargo deny check` pass with an empty advisory ignore list; advisory history documented under `docs/advisories/`.

## [0.0.0] - 2025-11-05
### Added
- Read-only workflow: automatic detection of filesystem permissions, a warning banner with enable-edit toggle, and lock-aware actions that keep accidental writes at bay.
- Find & replace workspace featuring an inline panel with incremental search, highlight-all overlays, regex/case/whole-word toggles, and the expected shortcuts (`Ctrl+F`, `F3`, `Shift+F3`, `Ctrl+H`).
- Word-wrap toggle with persistent monospace font styling plus toolbar and menu wiring to match common shortcuts.
- Status bar enhancements including live line/column readout, encoding and line-ending selectors, and a reload control for reopening files with a different encoding.
- Editing affordances spanning undo/redo, duplicate line, cut/copy/paste, and select-all, all bridged through the Rust backend for consistency.
- Encoding reopen-as workflow covering UTF-8, UTF-16 LE/BE, and ISO-8859-1 alongside manual line-ending conversion between LF and CRLF.

### Changed
- Toolbar, menus, and status bar now surface read-only state with lock icons, path annotations, and disabled editing commands until the override is enabled.
- Document manager now enforces a 25 MiB safety guard when opening files and surfaces a friendly error in the UI.
- Per-document metadata persists encoding and line-ending choices on save, ensuring round-trips match expectations across platforms.
- QML shell synchronizes format selectors and status messaging with backend state to keep the toolbar in lockstep with document changes.

### Fixed
- Switching line-ending styles no longer leaves stale `\r` characters behind; buffers are normalized before conversion.
- Reloading a document after changing encoding resets dirty state and cursor info to avoid spurious save prompts.

## [0.0.0] - 2025-11-03
### Added
- Cross-platform file pickers for Open / Save / Save As workflows wired into the Rust backend.
- Dynamic document tabs with dirty-state indicators, MRU-backed welcome page, and an inline autosave status footer.
- Automatic autosave snapshots with crash/session restore on startup plus pending-close prompts for unsaved work.

### Changed
- Core document manager now persists state to JSON autosave snapshots and exposes autosave metadata through the UI bridge.
- QML shell upgraded to synchronize document text in real time with backend operations.

### Fixed
- Ensured document state is retained when reopening autosave snapshots.
- Prevented duplicate entries in the recent-documents shelf when re-opening an already tracked file.

## [0.0.0] - 2025-10-15
### Added
- Initial document manager capable of opening, saving, and tracking multiple documents with MRU persistence.
- `cxx-qt` bridge exposing document operations to the QML layer.
- Basic Kirigami shell with welcome view, menu bar, and placeholder editor surface.

### Infrastructure
- Rust workspace scaffolding (`core`, `ui_bridge`, `app`) with VS Code-friendly layout.
