# Building from source

GhostPad targets Arch Linux with KDE Plasma on Wayland. It is a Rust workspace
that links against Qt 6, Kirigami, and KDE Frameworks 6 through
[cxx-qt](https://github.com/KDAB/cxx-qt).

## Prerequisites

- Rust 1.96 or newer (edition 2024) via `rustup`.
- A C++ toolchain with `clang` (cxx-qt compiles generated C++).

### Arch Linux

```bash
sudo pacman -S --needed \
    rust cargo clang pkgconf \
    qt6-base qt6-declarative \
    kirigami kwindowsystem extra-cmake-modules
```

### Debian / Ubuntu (24.04+)

```bash
sudo apt-get install -y \
    qt6-base-dev qt6-declarative-dev \
    libkf6kirigami-dev libkf6windowsystem-dev \
    extra-cmake-modules clang pkg-config
```

## Build and run

```bash
# Build the whole workspace
cargo build --workspace

# Run the application
cargo run -p ghostpad-app
```

In a development checkout the binary loads QML from `app/qml/`. When installed,
it loads from `/usr/share/ghostpad/qml/` instead (resolved at startup in
`app/src/main.rs`).

## Tests, formatting, and lints

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Dependency auditing

```bash
cargo audit
cargo deny check
```

See [docs/advisories](../advisories/) for how advisories are tracked.
