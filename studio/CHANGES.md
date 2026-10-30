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
- Animation engine (`crates/engine/src/cmd/anim.rs`): timeline rows are the top-level layers,
  keyframes are sublayers that hold until the row's next keyframe. Commands `anim.info`,
  `anim.goto` / `next` / `prev` (view state), `anim.keyframe`, `anim.blankKeyframe`,
  `anim.removeKeyframe`, `anim.setFps`, `anim.setFrames`, `anim.exportGif`. The timing is kept in
  the document's preserved extra data (`"animation"`), so the file format is unchanged.
- Timeline panel (`crates/ui-egui/src/timeline.rs`) under the canvas: play/pause at the frame
  rate, first/previous/next/last frame, frame rate and length, Insert Keyframe / Blank Keyframe /
  Clear Keyframe, a row per top-level layer with keyframe dots, hold bands and the playhead;
  clicking a frame goes there and to that row's layer. Labels of the new commands in every
  complete catalog (`studio/i18n/*.tsv`).
- Video export (`crates/video`, `anim.exportVideo`): WebM (VP9, Opus sound) and MP4 (H.264, AAC
  sound, through filmcraft's encoder and muxer), and PNG sequences with alpha (`anim.exportFrames`).
  Exports take a step of 1–4, so an animation can run on twos.
- Tweens: motion tweens that move, turn and scale about the centre and mix fill, stroke and
  opacity; shape tweens that morph unlike shapes; easing per tween (linear, ease in/out, CSS
  cubic-bezier).
- Camera keys (centre, zoom, rotation, eased) applied to exports; the camera turns as in Harmony.
- Sound (`crates/audio`): WAV, MP3 and Ogg clips embedded in the document, mixed into exports and
  played with the timeline on desktop, with waveforms on the timeline. Lip sync guesses a mouth
  shape (Preston Blair A–H and X) per frame from a sound.
- Rigging: bones with linear blend skinning and IK (two-bone and FABRIK), pegs with keyed
  transforms that nest, mesh deformers (ARAP) for images, and curve and envelope deformers, each
  with its tool and canvas overlay (Bone, Mesh Deform, Curve Deform).
- Drawing substitution and an Xsheet panel; palettes per scene and per row; cutters and mattes;
  multiplane depth with a Top View and Camera Preview; row effects (blur, glow, drop shadow, colour
  ops, chroma key, matte-driven effects, motion blur); a Node View for the composite with Display
  and Write nodes, groups and cache lock.
- Drawing aids: pen pressure as line width, Colour Art fill and gap-closing paint, textured lines,
  palette tints and protected colours, and a light table.
- Characters: templates (`.7rrekch`, Save/Load Character…), a character contract
  (`character.check`) and auto-rig from an rsm file (`character.fromRsm`), views (front, 3/4, side)
  the character turns between, action templates by contract part (`action.apply`, a walk cycle and
  an idle) with squash and stretch, motion paths and timing charts.
- Scenes: `scene.setup` (stage size, frame rate, length, safe areas) and `scene.build`, which turns
  a scene spec into a shot.
- The timeline mirrors right to left in Arabic, and the new commands, panels and rows have Arabic
  labels.
