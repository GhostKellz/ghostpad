# Theming

GhostPad ships six themes, selectable from **Settings → Appearance → Theme**. The
choice is persisted in `config.json` under `ui.theme` and reapplied on startup.

| Theme | `ui.theme` value | Description |
|-------|------------------|-------------|
| System | `system` | Inherit the active KDE color scheme (default) |
| Light | `light` | Explicit light palette |
| Dark | `dark` | Explicit dark palette |
| Tokyo Night | `tokyo_night` | The classic dark "night" variant |
| Tokyo Night Storm | `tokyo_night_storm` | Lighter blue-grey background |
| Tokyo Night Moon | `tokyo_night_moon` | Cooler "moon" background |

## How theming works

For the **System** theme, GhostPad sets `Kirigami.Theme.inherit = true` and lets
Plasma drive every color. For the explicit themes, `applyTheme()` in
`app/qml/Main.qml` sets `Kirigami.Theme.inherit = false` and assigns the palette's
colors to the writable `Kirigami.Theme` color roles, which then cascade to child
components:

- `backgroundColor`, `textColor`, `disabledTextColor`
- `alternateBackgroundColor`, `highlightColor`
- `negativeTextColor`, `neutralTextColor`, `positiveTextColor`

The palettes themselves live in `app/qml/Theme.qml` as a small lookup table. The
Tokyo Night hex values come from the upstream
[tokyonight.nvim](https://github.com/folke/tokyonight.nvim) palette.

## Adding a new theme

1. **Add the palette** to the `palettes` map in `app/qml/Theme.qml`, supplying all
   eight color roles, plus an entry in the `options` list (with a translatable
   label).
2. **Add the enum variant** to `ThemeMode` in `core/src/document_manager.rs`,
   using `#[serde(rename_all = "snake_case")]` naming, and extend `as_str()` and
   `from_str_lenient()` to round-trip it.
3. Rebuild. The new theme appears in the Appearance combo box automatically; no
   changes to `Main.qml` are needed because it reads the options from
   `Theme.qml`.

Keep the QML `options` value and the Rust `as_str()` string identical — that
string is the value stored in `config.json`.
