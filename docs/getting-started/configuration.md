# Configuration

GhostPad persists its settings, recent-document list, and window geometry as a
single JSON file. Most users never need to edit it by hand — the Settings sheet
inside the app writes every value — but the format is documented here and in the
[settings reference](../reference/settings.md).

## File locations

| File | Path | Purpose |
|------|------|---------|
| Config | `~/.config/ghostpad/config.json` | Persisted settings, recents, window state |
| Autosave | `~/.config/ghostpad/config.autosave.json` | Snapshot of open documents |

The directory is resolved with the [`directories`](https://crates.io/crates/directories)
crate (`ProjectDirs::from("dev", "GhostPad", "GhostPad")`). If that lookup fails
it falls back to `$HOME/.config/ghostpad/`, then to `.ghostpad/` in the working
directory.

## Format

`config.json` is plain JSON. Unknown keys are ignored and missing keys fall back
to defaults, so older config files remain compatible across upgrades. Example:

```json
{
  "recent_documents": [],
  "ui": {
    "word_wrap_enabled": false,
    "translucency_enabled": false,
    "shadow_enabled": true,
    "theme": "system"
  },
  "editor": {
    "font_family": "monospace",
    "font_size": 11,
    "tab_stop_distance": 4,
    "indent_type": "spaces",
    "indent_size": 4
  },
  "find_defaults": {
    "case_sensitive": false,
    "whole_word": false,
    "use_regex": false,
    "wrap_around": true
  },
  "window": {
    "width": 960,
    "height": 620,
    "x": null,
    "y": null
  }
}
```

## Autosave

Open documents are snapshotted to `config.autosave.json` on a one-minute timer.
On the next launch GhostPad restores from this snapshot, so unsaved work survives
a crash or forced quit. The snapshot is independent of saving a file to its real
path on disk.

See the [settings reference](../reference/settings.md) for the type and default
of every field.
