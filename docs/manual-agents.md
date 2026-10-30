# 7rrek for AI Agents

How an AI agent drives 7rrek (حرّك), the Creative Studio animation app, over MCP. The human manual is [manual-en.md](manual-en.md) ([Arabic](manual-ar.md)). Read its section 1 if the animation terms below are new to you.

## Connecting

- **From the app:** Help › AI Agents… lets agents control the running app and adds 7rrek's MCP server to Claude Desktop, Claude Code or Gemini CLI. With a running app, every change shows on screen and `ui_screenshot` works.
- **Claude Desktop extension:** each release ships a `.mcpb` for macOS and Windows.
- **By hand:** the CLI inside the app bundle runs the server on stdio: `<cli> mcp` connects to a running app on `127.0.0.1:7979`, else runs headless. Use `--headless` for no window, and `--automation-read-root DIR` / `--automation-write-root DIR` to confine the files it may read and write. On macOS the CLI is `7rrek.app/Contents/MacOS/vectorcraft-cli` up to v1.2.0 (later releases name it `7rrek-cli`).

The server exposes typed tools for common work (`draw_shape`, `draw_path`, `set_paint`, `doc_inspect`, `render_preview`, `ui_screenshot`, `open_file`, `save_file`, `export`…) and **every** app action as a command through `command_list` / `command_run` / `command_batch`. The commands below are the animation ones. Always read a command's params with `command_list {filter: "<id>"}` before the first call: the params string is the contract.

## The model in five lines

1. **Rows are top-level layers.** Each top-level layer is one row of the timeline.
2. **Keyframes are sublayers** of the row, named `Frame N`. A keyframe holds until the row's next keyframe.
3. **`anim.keyframe` copies the art in effect** at that frame. Draw the art in the row first, then key frame 1, then go to frame N, key again, and change the copy.
4. **Tweens belong to a keyframe** and run toward the row's next keyframe. Objects pair by name, else by order.
5. **Pegs** are keyed transforms that rows hang from, and they nest. Characters are rows plus pegs, one per body part.

Frames are 1-based. Coordinates are document points, y down.

## Recipe: a tween

```text
command_run file.new {}
command_run scene.setup {width: 1920, height: 1080, fps: 24, frames: 24}
command_run layer.new {name: "ball"}                      → {id: ROW}
draw_shape {shape: "ellipse", x: 100, y: 800, width: 120, height: 120, fill: "#e04040", name: "ball"}
command_run anim.keyframe {row: ROW, frame: 1}
command_run anim.keyframe {row: ROW, frame: 24}            → move the copy in frame 24 (object.move / transform)
command_run anim.tween {row: ROW, frame: 1, ease: "easeInOut"}
command_run anim.exportVideo {path: "/abs/out.mp4", format: "mp4"}
```

Along a curve instead of a straight line: draw the path **in another row**, then `motionPath.set {path: PATH_ID, row: ROW, orient?: true}`. Check positions with `anim.goto {frame}` plus `render_preview` or `ui_screenshot`.

## Recipe: a character from a drawing

1. `character.contract {}` returns the part names, views and tree a character file must follow. `character.check {path}` checks an `.rsm` file against it (`missing`, `warnings`).
2. `character.fromRsm {path, name: "ana", view: "side"}` rigs it: one row and one peg per part, named `ana/<part>`, all hanging from the peg `ana`.
3. `action.list {}` lists the action templates (a walk cycle, an idle and others), and `action.apply {character: "ana", action: "walk", at: 1}` applies one from frame `at`. Repeat the call to loop it.
4. Move the whole character with `peg.key` on its root peg (its index comes back from step 2), with `ease` when needed.
5. Turn it with `character.view {character, frame, view: "front" | "3q" | "side"}`.
6. A reusable character template is saved with `character.save` and loaded with `character.load {path}` (`.7rrekchar`).

## Recipe: a whole shot in one call

`scene.build {spec}` builds characters, actions, moves, dialogue (sound and captions) and camera keys from one JSON spec, as a single undo step. Get the schema first with `scene.specSchema {}`. Paths inside the spec are relative to the spec file. Read `skipped` in the reply: it lists what was left out and why (for example a blink on a side view, which has only one eye).

```json
{"width": 1536, "height": 1024, "fps": 24, "frames": 120,
 "characters": [{"name": "ana", "file": "char.rsm", "view": "side", "x": -1000, "y": 0}],
 "actions": [{"character": "ana", "action": "walk", "at": 1, "times": 2}],
 "moves": [{"character": "ana", "from": 1, "to": 48, "dx": 500}],
 "dialogue": [{"character": "ana", "sound": "line.wav", "at": 66, "text": "مرحبا"}],
 "camera": [{"frame": 1, "x": 768, "y": 512, "zoom": 1},
            {"frame": 120, "x": 768, "y": 300, "zoom": 1.8, "ease": "easeInOut"}]}
```

## Other commands

| Need | Command |
|---|---|
| Timeline state (rows, keys, fps, length) | `anim.info` |
| Frame rate, length | `anim.setFps`, `anim.setFrames`, or `scene.setup` for both plus stage size |
| Empty keyframe for frame-by-frame drawing | `anim.blankKeyframe` |
| Remove a keyframe | `anim.removeKeyframe` |
| Onion skin, light table | `anim.onion {before, after}`, `anim.lightTable` |
| Camera move, zoom, turn | `anim.cameraKey {frame, x, y, zoom, rotation, ease}` |
| Sound | `anim.soundImport {path, start}` (WAV, MP3, Ogg) |
| Mouth shapes from a sound | `anim.lipSync {row, sound}` (reply lists `missing` mouth drawings) |
| Pegs | `peg.add`, `peg.key`, `peg.info` |
| Bones and IK | `rig.*` (`rig.info` first) |
| Export | `anim.exportVideo` (mp4/webm, with sound), `anim.exportGif`, `anim.exportFrames` (PNG sequence) |

`step: 2` on any export animates on twos.

## Saving

`document.save {path: "/abs/shot.7rrek"}`. The extension picks the format; `.7rrek` is the native one. `.rsm` (rsm's) and `.vectorcraft` files open too.

## Working well

- **Keep replies small.** `doc_inspect` on a rigged scene and `peg.info` return a lot. Use `doc_inspect {depth: 0}`, `{ids: [...]}` or `{kind: ...}`.
- **Look at the result** after visual changes: `render_preview`, or `ui_screenshot` when the app is running. `anim.goto` first to see a given frame.
- **Batch** related commands with `command_batch` so they form one undo step.
- **One document at a time:** commands act on the active document. A running app may have the person's own documents open with unsaved changes. Start your own with `file.new` and never close or save theirs.
- **UI-only commands** (`view.fitArtboard`, `view.zoomIn`, panels) go through `invoke_menu` and only exist with a running app.
- **Errors name the field** (`actions[2].at`). Fix that field and retry. Don't guess other params.
- **File access** may be confined by `--automation-read-root` / `--automation-write-root`. Use absolute paths inside the allowed folders.
