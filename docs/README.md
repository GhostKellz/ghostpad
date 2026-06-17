# GhostPad Documentation

GhostPad is a lightweight, Windows 11 Notepad-style text editor for KDE Plasma,
built with Rust, Qt 6, and Kirigami.

## Getting started

- [Build from source](getting-started/build.md) — Arch dependencies and `cargo run`.
- [Installation](getting-started/installation.md) — PKGBUILD / AUR packaging.
- [Configuration](getting-started/configuration.md) — config file location and format.

## Guides

- [Theming](guides/theming.md) — System / Light / Dark and the Tokyo Night variants.
- [Keyboard shortcuts](guides/keyboard-shortcuts.md) — full shortcut reference.
- [KDE integration](guides/kde-integration.md) — KWin blur, Wayland, desktop entry.

## Reference

- [Settings schema](reference/settings.md) — every persisted field and its default.

## Internals

- [Architecture](internals/architecture.md) — the three-crate workspace and cxx-qt bridge.

## Security

- [Accepted advisories](advisories/accepted.md)
- [Resolved advisories](advisories/resolved.md)

See [`SECURITY.md`](../SECURITY.md) for the reporting policy and
[`CONTRIBUTING.md`](../CONTRIBUTING.md) to contribute. Version history lives in
[`CHANGELOG.md`](../CHANGELOG.md).
