---
name: studio-release
description: Build and publish rsm: the macOS app bundle, the web (WebAssembly) build, and the GitHub Pages deployment at rsm.studio.creative.ps, with an Arabic and English screenshot check before publishing. Use whenever the user asks to update the web version, deploy, publish, release, give them a link to try, build the Mac app, or "ارفع النسخة" / "حدّث الويب" / "بدي لينك", and after any change to the interface that the user should see.
---

# Release rsm

The web build is what the user actually tries, so a broken or un-branded deploy costs trust.
Every release is therefore gated on tests and on looking at the result in both languages.

Repo: `~/Documents/CreativeStudio/rsm`. Scripts are in this skill's `scripts/` folder.

## Steps

1. Gates first:
   - `cargo test -q -p vectorcraft-ui-egui`. The only known failure is upstream's
     `save_a_copy_and_template_suggest_their_names`. Report any other failure and stop.
   - `cargo xtask assets` and `cargo xtask brands`.
2. Build: `bash <skill>/scripts/release.sh --web` (add `--mac` when the user wants the app).
3. Look before publishing. Serve `dist/web` locally, then take screenshots with
   `scripts/screenshot.mjs`, using `ar` and then `en`:
   ```sh
   cd ~/Documents/CreativeStudio/rsm/dist/web && python3 -m http.server 8799 &
   node <skill>/scripts/screenshot.mjs http://localhost:8799/ ar /tmp/rsm-ar.png
   node <skill>/scripts/screenshot.mjs http://localhost:8799/ en /tmp/rsm-en.png
   pkill -f "http.server 8799"
   ```
   `playwright-core` must be installed in the folder you run node from, or in a scratch folder.
   Read both images. Check that:
   - the start screen draws;
   - Arabic letters join and words read right to left;
   - numbers and sizes are not reversed;
   - the name is "rsm";
   - no ArtCraft or Discord branding is visible.
4. Publish: `bash <skill>/scripts/release.sh --deploy`. Then confirm the live page answers 200 and
   its `<title>` says rsm (GitHub Pages can take a minute).
5. Make sure `main` is pushed, so the release matches a commit on GitHub.

## Report to the user (Arabic, short)

The live link, what changed since the last release, anything visibly wrong in the screenshots,
and the path to `rsm.app` if it was built. The Mac build isn't signed: the user opens
it the first time with right-click › Open.
