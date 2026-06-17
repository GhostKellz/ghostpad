# Installation

## Arch Linux (PKGBUILD)

A `PKGBUILD` is provided under `packaging/arch/`. From a source tarball or a
checkout arranged as `ghostpad-<version>/`:

```bash
cd packaging/arch
makepkg -si
```

This builds with `cargo build --release` and installs:

| Path | Contents |
|------|----------|
| `/usr/bin/ghostpad` | The application binary |
| `/usr/share/ghostpad/qml/` | QML UI files (`Main.qml`, `Theme.qml`, …) |
| `/usr/share/applications/dev.ghostpad.GhostPad.desktop` | Desktop entry |
| `/usr/share/metainfo/dev.ghostpad.GhostPad.metainfo.xml` | AppStream metadata |
| `/usr/share/icons/hicolor/256x256/apps/dev.ghostpad.GhostPad.png` | App icon |
| `/usr/share/licenses/ghostpad/LICENSE` | License |

### Runtime dependencies

`qt6-base`, `qt6-declarative`, `kirigami`, `kwindowsystem`, `hicolor-icon-theme`.

### Build dependencies

`rust`, `cargo`, `cmake`, `extra-cmake-modules`, `clang`, `qt6-tools`,
`corrosion`.

## AUR

When published, install with your preferred AUR helper:

```bash
paru -S ghostpad   # or: yay -S ghostpad
```

## Verifying the install

```bash
ghostpad --version   # reports the APP_VERSION baked in at build time
```

The application ID is `dev.ghostpad.GhostPad`, so it appears in the KDE
application launcher as **GhostPad**.
