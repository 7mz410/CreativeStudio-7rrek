# Right-to-left interface (Creative Studio)

When the interface language is Arabic, rsm lays its interface out right to left: the tool
bar sits on the right, the dock on the left, and rows, grids, columns, menus and dialogs run from
the right. The canvas, rulers and artwork are not mirrored.

## How it works

egui lays everything out left to right and has no RTL mode, so we carry a small patch to egui
0.36.2 in `vendor/egui` (used through `[patch.crates-io]` in `Cargo.toml`). Every change is marked
`Creative Studio`:

- `layout.rs`: a process-wide switch, `egui::set_rtl(bool)` / `egui::is_rtl()`, and
  `egui::start_align()` (left, or right when RTL). `Layout::default()` starts from it.
- `containers/panel.rs`: panels start from `Layout::default()`.
- `containers/menu.rs`, `containers/popup.rs`: menu bars run right to left, menus and popups align
  right.
- `ui.rs`: `ui.vertical` aligns to the start edge, and `ui.columns` puts the first column on the
  right. `ui.horizontal` already followed its parent's direction.
- `grid.rs`: grid columns run from the right.

In our code (applied by `studio/rebrand.py`, so upstream merges keep it):
- `lib.rs` turns the switch on each frame when the language is in `i18n::bidi::RTL_CODES`.
  Unit tests leave it off, because they run in parallel and the switch is process-wide;
  `crates/ui-egui/tests/rtl.rs` tests it in its own process.
- `toolbar.rs`, `dock.rs`: the tool bar and the dock change sides.
- `i18n/bidi.rs` keeps `{placeholders}` whole when it puts Arabic text in visual order.

## Upgrading egui

When upstream moves to a newer egui, the patch stops applying (Cargo warns that the patch is
unused, and `egui::set_rtl` no longer exists, so the build fails). Copy the new egui into
`vendor/egui` and re-apply the `Creative Studio` changes listed above.

## Still left to right

Widgets drawn at fixed coordinates: the Layers panel rows, the document tabs, the panel footer
buttons, some dialogs (Save for Web previews). They need mirroring one by one.
