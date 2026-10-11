---
name: studio-arabic
description: Translate and maintain the Arabic interface of rsm (catalog ar.tsv, right-to-left ordering in i18n/bidi.rs). Use whenever the user mentions Arabic UI text, a wrong or English label in the Arabic interface, reversed words or numbers, new strings after an upstream sync, terminology choices, or asks to translate, review or fix "العربي" / "الترجمة" in the app, even when they only send a screenshot of a wrong label.
---

# Arabic interface for rsm

The Arabic interface is Creative Studio's main added value, so wording should read like a designer's
tool, not a machine translation, and nothing may break the layout tests.

## How it works

- Strings in code stay English and act as keys. `crates/ui-egui/src/i18n/ar.tsv` maps them to
  Arabic, with rows of the form `context<TAB>English<TAB>Arabic`.
- **Never edit `ar.tsv` by hand.** It is generated: batches live in `studio/ar-work/`
  (`part_NN` holds the keys, `tr_NN` holds one translation per line in the same order), and
  `assemble.py` builds the catalog. `check.py` verifies a batch's line count and `{placeholders}`.
- Translations are written in logical order. `i18n/bidi.rs` puts them in visual order when the
  catalog loads, because egui shapes words but lays them out left to right. Sizes like 1920×1080
  and the spaces around joined fragments are kept intact there.
- Plurals have six forms, `zero|one|two|few|many|other`, and every form keeps `{n}`.

## Adding or fixing translations

1. Find missing strings: `python3 <skill>/scripts/new_strings.py --write` creates the next `part_NN`.
2. Write `tr_NN` with the Write tool, one line per key and in the same order. The Write tool drops
   trailing spaces, but `assemble.py` restores the source's leading and trailing spaces, so you
   don't need to fight it.
3. `python3 studio/ar-work/check.py studio/ar-work NN`, then spot-check alignment:
   `paste -d'|' <(sed -n '50p;150p' part_NN | cut -f2) <(sed -n '50p;150p' tr_NN)`.
4. To fix an existing label, edit its line in the matching `tr_NN`. Find it with
   `grep -n` on the English text in `part_*`.
5. `python3 studio/ar-work/assemble.py`, then `python3 studio/rebrand.py`. Rebrand drops rows
   for strings the rebranded UI no longer shows, so it always runs after assemble.
6. `cargo test -q -p vectorcraft-ui-egui i18n`. All i18n tests must pass.
7. For visible changes, check a screenshot with the `studio-release` steps (locale `ar`).

## Terminology

Use these terms consistently. They are what Arabic designers know.

| English | Arabic |
|---|---|
| Artboard | لوحة الرسم |
| Layer | طبقة |
| Fill / Stroke | التعبئة / الحدود |
| Anchor Point | نقطة ربط |
| Path | مسار |
| Pen / Pencil | القلم / قلم الرصاص |
| Swatch | عينة |
| Gradient | تدرّج |
| Pattern | نقش |
| Brush | فرشاة |
| Symbol | رمز |
| Clipping Mask | قناع القص |
| Opacity Mask | قناع الشفافية |
| Blend | مزج |
| Expand | توسيع |
| Outline (text/stroke) | خط خارجي |
| Selection / Direct Selection | التحديد / التحديد المباشر |
| Transform | تحويل |
| Preset | إعداد |
| Workspace | مساحة العمل |
| Kerning / Tracking / Leading | تقنين المسافة / التتبّع / تباعد الأسطر |

Keep these as they are, as the other catalogs do:
- the product name and the workspace names (Essentials…);
- the perspective preset names;
- "OpenType";
- each language's own name in the language menu ("English" stays "English").

In `@msg` rows, keep everything inside backticks, identifiers such as `colorModel rgb`, and file
extensions as they are.

## Known limits

The Arabic text is right-to-left, but the layout itself (panel sides, menu order, the hint bar
built from fragments, multi-line wrapping) is still left to right until the RTL layout work is
done. Don't try to fix those through translations.
