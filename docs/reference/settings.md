# Settings reference

All settings live in `~/.config/ghostpad/config.json` (see
[configuration](../getting-started/configuration.md)). The schema is defined by
`ManagerConfig` in `core/src/document_manager.rs`. Every field is optional in the
file; missing fields fall back to the defaults below.

## Top level

| Key | Type | Description |
|-----|------|-------------|
| `recent_documents` | array | Most-recently-used document list (max 10) |
| `ui` | object | [UI settings](#ui) |
| `editor` | object | [Editor settings](#editor) |
| `find_defaults` | object | [Find defaults](#find-defaults) |
| `window` | object | [Window state](#window-state) |

## `ui`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `word_wrap_enabled` | bool | `false` | Wrap long lines in the editor |
| `translucency_enabled` | bool | `false` | Translucent chrome + KWin blur |
| `shadow_enabled` | bool | `true` | Draw a window shadow |
| `theme` | string | `"system"` | One of `system`, `light`, `dark`, `tokyo_night`, `tokyo_night_storm`, `tokyo_night_moon` |

## `editor`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `font_family` | string | `"monospace"` | Editor font family |
| `font_size` | number | `11` | Font point size |
| `tab_stop_distance` | number | `4` | Tab width in columns |
| `indent_type` | string | `"spaces"` | `spaces` or `tabs` |
| `indent_size` | number | `4` | Indent width |

## `find_defaults`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `case_sensitive` | bool | `false` | Case-sensitive search |
| `whole_word` | bool | `false` | Match whole words only |
| `use_regex` | bool | `false` | Treat the query as a regular expression |
| `wrap_around` | bool | `true` | Wrap past the end/start of the document |

## `window`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | number | `960` | Window width in pixels |
| `height` | number | `620` | Window height in pixels |
| `x` | number \| null | `null` | Window X position (null = let the WM decide) |
| `y` | number \| null | `null` | Window Y position |

## Document encodings & line endings

These are per-document and chosen from the toolbar rather than stored in
`config.json`:

- **Encodings:** UTF-8, UTF-16LE, UTF-16BE, ISO-8859-1.
- **Line endings:** LF (Unix) or CRLF (Windows).
