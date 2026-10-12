# 7rrek: changes from upstream VectorCraft

7rrek (حرّك) starts from rsm, the Creative Studio fork of VectorCraft: everything below came with
it, and `studio/anim_brand.py` gives it its own name over rsm's (the `.rsm` document format is
shared). Animation work is listed in the 7rrek section at the end.

rsm is a fork of [VectorCraft](https://github.com/storytold/vectorcraft) by the ArtCraft
team. As the Apache License 2.0 asks, this file lists what the fork changes. Upstream code keeps
its copyright and licence (MIT OR Apache-2.0); see `NOTICE`.

## Branding

- App name in the interface, window title, macOS menus and About box: "rsm".
- App icon: the rsm mark (`docs/studio/`), regenerated with `packaging/icons.sh`.
- macOS bundle name and identifier (`ps.creative.studio.7rrek`).
- Help menu links point to the rsm repository.
- Upstream's ArtCraft brand files (`docs/brand/`) are removed.
- The upstream community Discord button (title bar, start screen) and the ArtCraft website link are
  removed; the start screen links to this repository.

The rename is applied by `studio/rebrand.py` and touches user-visible text only. Crate names, the
`.vectorcraft` file format and other internals keep their upstream names so upstream updates merge
cleanly.

## Updating from upstream

```sh
git fetch upstream
git merge upstream/main          # resolve conflicts in branded strings in favour of upstream
python3 studio/rebrand.py      # re-apply the branding
```

## Arabic interface

- Arabic (`ar`) added to the interface languages, with a catalog in `crates/ui-egui/src/i18n/ar.tsv`
  (being completed) and Arabic plural rules.
- IBM Plex Sans Arabic (OFL) added to the UI fonts so Arabic draws without system fonts, including
  in the web build.
- `crates/ui-egui/src/i18n/bidi.rs` stores right-to-left translations in visual order, because egui
  shapes words but lays them out left to right. Sizes such as 1920×1080 stay left to right, and
  the leading and trailing spaces of joined fragments stay in place.
- The Help/Community "Website" link reads "Creative Studio Website" and opens https://studio.creative.ps; the
  GitHub link still opens this repository (`rebrand.py`, `SITE`).
- Documents save as `.rsm` (the same contents as `.vectorcraft`; `.vectorcraft` and `.drawcraft`
  still open), and Finder shows them with the rsm document icon
  (`assets/app-icon/rsm-document.icns`). Applied by `rebrand.py` (`EXT_FILES`, `EXT_TESTS`).
- Right-to-left interface for Arabic through a patched egui in `vendor/egui` (`studio/RTL.md`),
  with `crates/ui-egui/tests/rtl.rs`. Arabic `{placeholders}` stay whole in visual order.
- Help › AI Agents (`crates/ui-egui/src/agents.rs`, `apps/vectorcraft/src/studio_agents.rs`):
  allow agents to control the app (the localhost control channel then starts with the app; the
  choice is saved in `studio-agents.json` next to the preferences), and add the app's MCP server
  to Claude Desktop, Claude Code or Gemini CLI. Strings for other languages: `studio/i18n/`.
- Checkboxes and radio buttons put their box on the right in the right-to-left interface.

## 7rrek

- Name 7rrek (حرّك), icon and links (`studio/anim_brand.py`, `docs/studio/7rrek.svg`).
