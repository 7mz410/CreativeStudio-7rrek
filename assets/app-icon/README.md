# rsm app icon

**Artwork:** the rsm star (`docs/studio/rsm.svg`) on a navy (#0d2d3f) tile. It
replaces upstream VectorCraft's dragon icon.

**Palette:** ink `#ffffff` on field `#000000`.

**Tile:** `viewBox="0 0 512 512"`, a rounded square with `rx=112`, clipped, with the mark inset 72
units on each side. The macOS files add Apple's transparent margin (tile = 824/1024 of the canvas);
Windows and Linux use the tile edge to edge. File names keep upstream's `vectorcraft` names so the
build and packaging scripts work unchanged. Regenerate everything with `packaging/icons.sh`.

## Files

| File | What | Used by |
|---|---|---|
| `vectorcraft.svg` | master vector (traced at 2048 px, about 700 KB) | source for everything below |
| `vectorcraft-small.svg` | lighter vector (traced at 1024 px, about 370 KB) | `hicolor/scalable` |
| `vectorcraft-1024.png` | full tile, 1024 px | docs, store listings |
| `vectorcraft-macos-512.png` | tile with Apple margin, 512 px | runtime Dock icon (`apps/vectorcraft/src/main.rs`) |
| `vectorcraft.icns` | macOS icon set, 16 to 1024 px | `cargo xtask bundle` (`CFBundleIconFile`) |
| `vectorcraft.ico` | Windows icon, 16 to 256 px | `apps/vectorcraft/build.rs` (embedded in the `.exe`) |
| `hicolor/<size>/apps/ai.storyteller.vectorcraft.png` | Linux theme icons, 16 to 512 px; the 256 one is also the runtime icon on Windows and Linux, the 128 one the brand mark in the app bar and About box (`crates/ui-egui/src/brand.rs`) | `packaging/linux/ai.storyteller.vectorcraft.desktop` |
| `hicolor/scalable/apps/ai.storyteller.vectorcraft.svg` | Linux scalable icon | as above |

## How it reaches each OS

- **macOS:** the app sets the Dock / app-switcher icon at runtime (`ViewportBuilder::with_icon`); the
  `.app` from `cargo xtask bundle` carries `vectorcraft.icns`.
- **Windows:** `build.rs` embeds `vectorcraft.ico` and VERSIONINFO with `winresource` (taskbar, Start
  menu, Explorer, Alt-Tab); the runtime icon covers the title bar.
- **Linux:** install `packaging/linux/ai.storyteller.vectorcraft.desktop` to `share/applications/` and
  `hicolor/` to `share/icons/`. The app sets its Wayland app ID / X11 class to
  `ai.storyteller.vectorcraft`, so the dock matches the window to the launcher.

## Regenerate

Edit or replace `vectorcraft.svg` (and `vectorcraft-small.svg`), then run `packaging/icons.sh`
(needs `resvg`; `iconutil` on macOS for the `.icns`). It writes every PNG, the `.ico` (via
`cargo xtask ico`) and the `.icns`.
