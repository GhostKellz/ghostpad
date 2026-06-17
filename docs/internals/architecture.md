# Architecture

GhostPad is a Cargo workspace of three crates. Logic lives in pure Rust; the UI
is QML; the two meet through a single cxx-qt bridge object.

```
┌─────────────────────────────────────────────┐
│ app  (ghostpad-app)                           │
│   main.rs → QGuiApplication + QQmlEngine       │
│   qml/Main.qml, qml/Theme.qml  (Kirigami UI)   │
└───────────────────────┬───────────────────────┘
                        │ GhostPad.Backend (QObject)
┌───────────────────────┴───────────────────────┐
│ ui_bridge  (ghostpad-ui-bridge)                │
│   bridge.rs   cxx-qt Backend, JSON state        │
│   kwin_blur.cpp   KWindowEffects blur helper    │
└───────────────────────┬───────────────────────┘
                        │ calls
┌───────────────────────┴───────────────────────┐
│ core  (ghostpad-core)                          │
│   document.rs, document_manager.rs,             │
│   file_watcher.rs, welcome.rs, app_info.rs      │
└─────────────────────────────────────────────┘
```

## Crates

### `core` — application logic

Plain Rust with no Qt dependency, so it is unit-testable in isolation.

- `app_info.rs` — `APP_ID`, `APP_NAME`, `APP_VERSION` constants.
- `document.rs` — the document model: text buffer, `TextEncoding`, `LineEnding`,
  `FindOptions`, and error types.
- `document_manager.rs` — the central `DocumentManager` service: open documents,
  active document, find/replace sessions, recent documents, autosave, and the
  persisted settings structs (`UiSettings`, `EditorSettings`, `FindDefaults`,
  `WindowState`, `ThemeMode`).
- `file_watcher.rs` — watches open files for external modification/deletion via
  the `notify` crate.
- `welcome.rs` — welcome-screen headline/tagline content.

### `ui_bridge` — Rust ↔ Qt bridge

- `bridge.rs` — the cxx-qt `Backend` QObject, registered into QML as
  `GhostPad.Backend` via `#[qml_element]`. It owns a `DocumentManager` and
  exposes invokable methods (new/open/save, find/replace, settings updates, file
  event polling, …). Application state is passed to QML as serialized JSON, so
  the QML side reads a single state snapshot rather than many fine-grained
  properties.
- `kwin_blur.cpp` — `KWinBlurHelper` wrapping `KWindowEffects::enableBlurBehind()`
  (see [KDE integration](../guides/kde-integration.md)).

`build.rs` drives the cxx-qt 0.8 build: it declares the `GhostPad` QML module,
compiles `bridge.rs`, links the `Gui` Qt module, and compiles `kwin_blur.cpp`
with KF6WindowSystem include paths when available.

### `app` — the binary

- `main.rs` — creates `QGuiApplication` and `QQmlApplicationEngine`, sets
  application metadata, configures KDE/Wayland environment, and loads `Main.qml`.
  The QML path resolves to `/usr/share/ghostpad/qml/Main.qml` when installed, or
  `app/qml/Main.qml` in a development checkout.
- `qml/Main.qml` — the entire Kirigami UI: tabbed documents, editor, find/replace
  panel, settings sheet, status bar.
- `qml/Theme.qml` — palette tables for the explicit themes, auto-resolved as a
  sibling component of `Main.qml`.

## State flow

1. QML calls a `Backend` invokable (e.g. `open_document(path)`).
2. `bridge.rs` delegates to `DocumentManager` in `core`.
3. The backend returns the new application state as a JSON string.
4. QML parses the JSON and updates its view.

## Background activity

- **Autosave** snapshots all open documents to `config.autosave.json` on a
  one-minute timer.
- **File watching** polls for external changes (default 2 s); QML calls
  `poll_file_events()` and surfaces a reload/dismiss banner when a file changes
  on disk.
