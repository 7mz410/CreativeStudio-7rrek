---
name: studio-sync
description: Bring upstream VectorCraft (storytold/vectorcraft) changes into the rsm fork without losing the Creative Studio branding or the Arabic interface. Use whenever the user asks to update from upstream, pull ArtCraft's latest, sync the fork, merge upstream, check what's new upstream, or before starting a big feature on rsm, even if they just say "حدّث من الأصل" or "شو الجديد عندهم".
---

# Sync rsm with upstream

rsm is a soft fork: upstream moves fast, and our changes are kept small so merges stay
easy. The branding is applied by a script after each merge rather than by hand-edited conflicts,
because re-running the script is reliable and conflicts in renamed strings are noisy.

Repo: `~/Documents/CreativeStudio/rsm` (`origin` = 7mz410/CreativeStudio-7rrek, `upstream` = storytold/vectorcraft).
Read `studio/PROJECT.md` first if this session hasn't.

## Steps

1. Start clean: `git status` must show no uncommitted work. If there is some, ask before stashing.
2. See what is coming: `git fetch upstream && git log --oneline main..upstream/main | wc -l`, and skim
   `git log --oneline main..upstream/main | head -40` so the report can name the notable changes.
3. Merge: `git merge upstream/main --no-edit`.
   - Conflicts in strings or tests that only differ by branding: take upstream's side
     (`git checkout --theirs <file>`), because `rebrand.py` re-applies ours.
   - Conflicts in our own additions (`i18n/bidi.rs`, `ar.tsv`, `plural_arabic`, font lines in
     `theme.rs`, `ASSETS.md` rows, `README.md`, `NOTICE`): keep both sides; ours must survive.
   - Anything else ambiguous: stop and show the user the conflict.
4. Re-apply branding: `python3 studio/rebrand.py` (idempotent; a second run prints nothing).
5. Arabic: new upstream UI strings show in English until translated. Run the `studio-arabic`
   skill's "find new strings" step and mention the count in the report; translate if the user
   wants it now.
6. Gates, all must pass before committing:
   - `cargo test -q -p vectorcraft-ui-egui` (known upstream failure:
     `save_a_copy_and_template_suggest_their_names`; anything else is ours to fix)
   - `cargo test -q -p vectorcraft-engine`
   - `cargo xtask assets` and `cargo xtask brands`
7. Commit as the user only (no co-author lines):
   `git -c user.name="Hamza Abu Ayyash" -c user.email=hamza.abu3ayash@gmail.com commit -m "Merge upstream VectorCraft (<N> commits)"`,
   then `git push origin main`.
8. Update `studio/PROJECT.md` §4 with the date and what came in.

## Report to the user (Arabic, short)

- How many upstream commits came in, and the 3–5 most notable features or fixes.
- Conflicts and how they were resolved.
- Test results, naming any failure plainly.
- How many new strings need Arabic translation.
- Whether a release (`studio-release`) is worth doing now.
