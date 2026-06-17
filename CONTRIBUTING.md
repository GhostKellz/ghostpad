# Contributing to GhostPad

Thanks for your interest in contributing to GhostPad! This document covers how
to get set up and the standards we hold contributions to.

## Prerequisites

- **Rust 1.96+** (edition 2024) via `rustup`.
- A C++ toolchain with **clang** (cxx-qt compiles generated C++).
- **Qt 6**, **Kirigami**, and **KDE Frameworks 6** development packages.

On Arch Linux:

```bash
sudo pacman -S --needed \
    rust cargo clang pkgconf \
    qt6-base qt6-declarative \
    kirigami kwindowsystem extra-cmake-modules
```

See [docs/getting-started/build.md](docs/getting-started/build.md) for other
distributions.

## Development Setup

```bash
# Clone
git clone git@github.com:ghostkellz/ghostpad.git
cd ghostpad

# Build everything
cargo build --workspace

# Run the app
cargo run -p ghostpad-app

# Run tests
cargo test --workspace
```

In a development checkout the app loads QML from `app/qml/`; an installed build
loads from `/usr/share/ghostpad/qml/`.

## Code Style

- Run `cargo fmt --all` before committing (config in `rustfmt.toml`, edition 2024).
- Ensure `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passes with **zero** warnings. Avoid `#[allow(...)]` band-aids; when one is
  unavoidable, scope it tightly and add a comment explaining why.
- Keep `unsafe` confined to the cxx-qt FFI boundary.
- Comments should explain *why*, not *what*. Don't scatter version numbers
  through code or docs — versioning lives in [`CHANGELOG.md`](CHANGELOG.md).

## Commit Messages

Use conventional, descriptive commit messages:

```
feat: add Tokyo Night Moon theme variant
fix: correct theme persistence in saveUiSettings
docs: document the settings schema
refactor: simplify find-session handling
test: add ThemeMode round-trip coverage
```

## Pull Request Checklist

1. Create a feature branch: `git checkout -b feat/my-feature`.
2. `cargo fmt --all --check` is clean.
3. `cargo clippy --workspace --all-targets --all-features -- -D warnings` is clean.
4. `cargo test --workspace` passes.
5. `cargo audit` and `cargo deny check` pass (add any new advisory to
   `deny.toml` **and** `docs/advisories/accepted.md` if it truly must be
   accepted).
6. Update `CHANGELOG.md` under `[Unreleased]`.
7. Push and open a PR against `main`.

## Project Structure

```
ghostpad/
├── core/          # ghostpad-core: pure-Rust logic (no Qt)
├── ui_bridge/     # ghostpad-ui-bridge: cxx-qt Backend + KWin blur
├── app/           # ghostpad-app: binary + QML UI (Main.qml, Theme.qml)
├── docs/          # documentation
├── packaging/     # PKGBUILD, .desktop, AppStream metadata
├── deny.toml      # cargo-deny config
└── CHANGELOG.md
```

See [docs/internals/architecture.md](docs/internals/architecture.md) for a
deeper tour.

## Adding a Theme

Theme contributions are welcome. Add the palette to `app/qml/Theme.qml` and the
matching `ThemeMode` variant in `core/src/document_manager.rs`; the full recipe
is in [docs/guides/theming.md](docs/guides/theming.md).

## Release Process

1. Move `[Unreleased]` notes in `CHANGELOG.md` under a new version heading.
2. Bump `version` in `[workspace.package]` in the root `Cargo.toml`.
3. Verify locally (fmt, clippy, build, test, audit, deny), then tag the release.
