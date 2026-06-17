# KDE integration

GhostPad is built specifically for KDE Plasma on Wayland and uses several KDE
Frameworks 6 facilities directly.

## KWin blur

When the chrome is translucent (**Settings → Appearance → Translucent chrome**),
GhostPad asks KWin to blur the content behind the window. This is implemented in
`ui_bridge/src/kwin_blur.cpp`, a small `KWinBlurHelper` that wraps
`KWindowEffects::enableBlurBehind()`.

The helper is only compiled with blur support when KF6WindowSystem is found at
build time: `ui_bridge/build.rs` probes for it via `pkg-config` and defines
`HAS_KWINDOWEFFECTS`. If the library is absent the build still succeeds, but blur
becomes a no-op (`isAvailable()` returns false).

## Wayland

At startup `app/src/main.rs` configures a few environment variables for a clean
Plasma/Wayland experience:

- `QT_QUICK_CONTROLS_STYLE=org.kde.desktop` — use the native KDE control style.
- `QT_WAYLAND_DISABLE_WINDOWDECORATION=1` — let the app draw its own chrome
  rather than relying on server-side decorations.

## Desktop entry & AppStream

Packaging installs:

- `dev.ghostpad.GhostPad.desktop` — the application launcher entry. The same ID
  is set on the running application (`QGuiApplication::set_desktop_file_name`) so
  the window associates with the launcher icon and task manager entry.
- `dev.ghostpad.GhostPad.metainfo.xml` — AppStream metadata for software
  centers.

The icon is installed into the hicolor theme at
`256x256/apps/dev.ghostpad.GhostPad.png`.
