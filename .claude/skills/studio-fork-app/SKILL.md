---
name: studio-fork-app
description: Start a new Creative Studio suite app as a branded fork of an open-source app (ArtCraft's PhotoCraft, LightCraft, FilmCraft, EffectCraft, DesignCraft, or another project such as LMMS, Audacity or Penpot), following the same recipe used for rsm. Use whenever the user wants to begin tlween, ttshat, tsweer, effectat, trteeb, mzika, aswat, sharek or any new Creative Studio app, asks "يلا نبلّش بـ ...", or wants to fork, rebrand or evaluate a base project for the suite.
---

# Fork a new Creative Studio app

rsm proved the recipe: fork honestly, rebrand only what users see, keep every licence
and credit, add Arabic, and stay mergeable with upstream. Repeating it the same way for each app
keeps the suite consistent and keeps maintenance manageable.

Read `~/Documents/CreativeStudio/rsm/studio/PROJECT.md` first. Use rsm's files as the working
example: `studio/rebrand.py`, `studio/CHANGES.md`, `README.md`, `NOTICE`, `ASSETS.md`, and
`crates/ui-egui/src/i18n/{bidi.rs,ar.tsv}`.

## 1. Evaluate before forking (answer only, no changes)

Report on these, then wait for the user's go-ahead:
- **Activity:** last push, release cadence, contributors.
- **Licence:** MIT/Apache is easy. GPL means the fork stays GPL, with source published for every
  build. Check the trademark or brand rules too: forks must usually drop the upstream name and logo.
- **Stack and UI toolkit:** egui needs our `bidi.rs` approach. Qt and GTK have native
  right-to-left support, which is easier.
- **Existing translations,** and whether Arabic is present.
- **How it compares with the Adobe app** it replaces, and the Arabic-specific added value we can
  bring. Examples: maqam quarter tones and oriental rhythms for mzika, Arabic speech-to-text for
  aswat, the calligraphy panel for rsm.

## 2. Fork

- `gh repo fork <owner>/<repo> --fork-name CreativeStudio-<app> --clone=false`, then clone to
  `~/Documents/CreativeStudio/<app>`.
- Keep `upstream` as a remote.

## 3. Rebrand (user-visible only)

- Write `studio/rebrand.py` for this app. It must be idempotent and must touch strings, window
  titles, bundle names and IDs (`ps.creative.studio.<app>`), and Help links only. Internals keep their
  names until the user decides on a full hard fork.
- **App icon:** the user's logo from `~/Documents/CreativeStudio/logos/<app>.svg`. Rebuild every icon size
  with the project's own icon script if it has one.
- **Remove upstream brand assets** as their brand terms require. Keep licences, `NOTICE` and
  copyright lines exactly.
- **Credits:**
  - README: "Built on <Upstream>" with links and thanks, plus the suite table.
  - `NOTICE`: the fork paragraph.
  - `studio/CHANGES.md`: what we changed.
  - About box: a credit line.
  - The user plans a separate `CREDITS.md` later, for the independent-fork phase.
- **Commits:** as Hamza Abu Ayyash only, with no co-author lines.

## 4. Arabic

- Add an `ar` language with Arabic plural rules and an OFL Arabic UI font (IBM Plex Sans Arabic or
  Noto Sans Arabic, from Google Fonts), listed wherever the project records assets.
- Translate with the batch workflow from `studio-arabic`.

## 5. Ship

- Build, check screenshots in `ar` and `en`, and publish a link if the app has a web build.
- Add the app to the suite table in rsm's README and to `PROJECT.md`.
- Create `studio/PROJECT.md` inside the new repo, or a section in rsm's.

## Report (Arabic, short)

What was forked, the licence obligations, what was rebranded, the test results, the link, and
the next steps.
