#!/usr/bin/env python3
"""7rrek (حرّك) is built on rsm: give it its own name on top of rsm's branding.

Run after studio/rebrand.py (which brands upstream VectorCraft as the app named in its NAME). The
document format and the `.rsm` extension stay shared with rsm, so a 7rrek file opens in rsm.
Idempotent.

    python3 studio/anim_brand.py
"""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NAME, NAME_AR = '7rrek', 'حرّك'

EXACT = [
    ('CreativeStudio-' + 'rsm', 'CreativeStudio-7rrek'),
    ('rsm' + '.studio.creative.ps', '7rrek.studio.creative.ps'),
    ('ps.creative.studio.' + 'rsm', 'ps.creative.studio.7rrek'),
    ('"studio-' + 'rsm"', '"studio-7rrek"'),
]
# The app's name, not the `.rsm` extension: `"rsm"` alone (an extension list entry), `.rsm`,
# `rsm-document` and `rsm/` paths are left as they are.
APP = re.compile(r'(?<![\w./-])rsm(?![\w/-]|\.\w)')


def rename_app(text: str) -> str:
    return APP.sub(NAME, text)


def code(text: str) -> str:
    """App names inside string literals, except a bare "rsm" (an extension)."""
    def lit(m):
        s = m.group(0)
        return s if s == '"rsm"' else rename_app(s)
    return re.sub(r'"(?:[^"\\\n]|\\.)*"', lit, text)


# The extension and the title literals that a bare "rsm" in code() leaves alone. Saves use `.7rrek`;
# `.rsm` (rsm's extension) still opens, so it is listed after 7rrek in every extension list.
# A rule runs only while its new text is missing, so a second run changes nothing.
TARGETED = {
    # The app's name where a bare "rsm" literal is the name, not the extension.
    'crates/ui-egui/src/native_menu.rs': [('pub const APP_NAME: &str = "rsm";', 'pub const APP_NAME: &str = "7rrek";')],
    'crates/ui-egui/src/menus.rs': [('.filter(|(title, _)| *title != "rsm")', '.filter(|(title, _)| *title != "7rrek")'),
        ('            "rsm",\n            vec![c("About 7rrek"', '            "7rrek",\n            vec![c("About 7rrek"'),
    ],
    'crates/engine/src/cmd/findfiles/walk.rs': [
        ('Err(format!("VectorCraft doesn\'t search other apps', 'Err(format!("7rrek doesn\'t search other apps'),
    ],
    'apps/vectorcraft-cli/tests/fileio.rs': [
        ('Readable formats: .rsm, .vectorcraft', 'Readable formats: .7rrek, .rsm, .vectorcraft'),
    ],
    'crates/ui-egui/src/tests_nativemenu.rs': [('assert_eq!(titles, ["rsm", "File"', 'assert_eq!(titles, ["7rrek", "File"')],
    'crates/ui-egui/src/i18n/tests.rs': [('.find(|m| m.title == "rsm")', '.find(|m| m.title == "7rrek")')],
    'crates/ui-egui/src/dialogs/about.rs': [('egui::RichText::new("rsm").font(theme::semibold(22.0))', 'egui::RichText::new("7rrek").font(theme::semibold(22.0))')],
    'crates/ui-egui/src/dialogs/export_for_screens.rs': [
        ('    ("rsm", "vectorcraft", None, false),', '    ("7rrek", "vectorcraft", None, false),'),
        ('"EPS", "AI", "rsm"];', '"EPS", "AI", "7rrek"];'),
    ],
    'crates/format/src/lib.rs': [
        ('pub const STUDIO_EXTENSION: &str = "rsm";', 'pub const STUDIO_EXTENSION: &str = "7rrek";'),
        ('/// rsm saves documents as `.rsm` (the same contents as `.vectorcraft`).',
         '/// 7rrek saves documents as `.7rrek` (the same contents as `.vectorcraft`); `.rsm` still opens.'),
        ('ext.eq_ignore_ascii_case(LEGACY_EXTENSION)\n}',
         'ext.eq_ignore_ascii_case(LEGACY_EXTENSION) || ext.eq_ignore_ascii_case("rsm")\n}'),
    ],
    'crates/engine/src/cmd/fileio/mod.rs': [
        ('        label: "rsm",', '        label: "7rrek",'),
        ('extensions: &[vectorcraft_format::STUDIO_EXTENSION, vectorcraft_format::EXTENSION, vectorcraft_format::LEGACY_EXTENSION],',
         'extensions: &[vectorcraft_format::STUDIO_EXTENSION, vectorcraft_format::EXTENSION, vectorcraft_format::LEGACY_EXTENSION, "rsm"],'),
        ('    "rsm",\n    "vectorcraft",', '    "7rrek",\n    "rsm",\n    "vectorcraft",'),
    ],
    'crates/engine/src/cmd/fileio/tests.rs': [
        ('        ["rsm", "svg", "svgz",', '        ["7rrek", "svg", "svgz",'),
    ],
    'crates/ui-egui/src/place.rs': [
        ('let native = ["rsm", "vectorcraft"', 'let native = ["7rrek", "rsm", "vectorcraft"'),
    ],
    'crates/ui-egui/src/io.rs': [
        ('"ait", "rsm", "vectorcraft"', '"ait", "7rrek", "rsm", "vectorcraft"'),
    ],
    'crates/ui-egui/src/chrome.rs': [
        ('CENTER_CENTER, "rsm", egui::FontId', 'CENTER_CENTER, "7rrek", egui::FontId'),
        ('layout_no_wrap("rsm".into()', 'layout_no_wrap("7rrek".into()'),
        ('.unwrap_or_else(|| "rsm".into())', '.unwrap_or_else(|| "7rrek".into())'),
        ('window_title(&title_test_app(true)), "rsm");', 'window_title(&title_test_app(true)), "7rrek");'),
        ('assert_eq!(app.last_window_title, "rsm");', 'assert_eq!(app.last_window_title, "7rrek");'),
        ('ViewportCommand::Title(t) if t == "rsm"', 'ViewportCommand::Title(t) if t == "7rrek"'),
        ('text.galley.job.text == "rsm")', 'text.galley.job.text == "7rrek")'),
        ('text.galley.job.text != "rsm")', 'text.galley.job.text != "7rrek")'),
    ],
    # The process name macOS shows in bold in the menu bar, and the first window title.
    'apps/vectorcraft/src/main.rs': [
        ('.with_title("VectorCraft")', '.with_title("7rrek")'),
        ('linux_stylus::run_native("VectorCraft", options, app_creator)', 'linux_stylus::run_native("7rrek", options, app_creator)'),
        ('eframe::run_native("VectorCraft", options, app_creator)', 'eframe::run_native("7rrek", options, app_creator)'),
    ],
    'apps/vectorcraft/src/mac_save_format.rs': [
        ('Some(&["rsm"][..])', 'Some(&["7rrek"][..])'),
    ],
    'packaging/macos/Info.plist.in': [
        ('<string>rsm Document</string>', '<string>7rrek Document</string>'),
        ('<array><string>rsm</string><string>vectorcraft</string><string>drawcraft</string></array>',
         '<array><string>7rrek</string><string>rsm</string><string>vectorcraft</string><string>drawcraft</string></array>'),
    ],
    'xtask/src/bundle.rs': [
        ('"<array><string>rsm</string><string>vectorcraft</string><string>drawcraft</string></array>"',
         '"<array><string>7rrek</string><string>rsm</string><string>vectorcraft</string><string>drawcraft</string></array>"'),
    ],
    'crates/ui-egui/src/tests_picks.rs': [
        ('/art/card.vectorcraft', '/art/card.7rrek'),
    ],
    'crates/ui-egui/src/dialogs/save_options.rs': [
        ('["rsm", "7rrek Template"', '["7rrek", "7rrek Template"'),
        ('assert_eq!(pick.name, "Untitled-1.rsm");', 'assert_eq!(pick.name, "Untitled-1.7rrek");'),
    ],
}


def apply_targeted(rel: str, text: str) -> str:
    for a, b in TARGETED.get(rel, []):
        if b not in text:
            text = text.replace(a, b)
    return text


# Tests whose expectations name a saved document `.rsm`: they now expect `.7rrek`. Opening `.rsm` is
# still covered by crates/format/tests/studio_extension.rs. Idempotent: no `.rsm` left after one run.
EXT_TEST_FILES = {
    'crates/engine/src/tests_package.rs',
    'crates/engine/src/tests_save.rs',
    'crates/engine/src/tests_saveoptions.rs',
    'crates/engine/src/cmd/fileio/tests.rs',
    'crates/engine/src/cmd/fileio/tests_affinity.rs',
    'crates/engine/src/cmd/fileio/tests_batch.rs',
    'crates/ui-egui/src/dialogs/tests_package.rs',
    'crates/ui-egui/src/dialogs/save_options.rs',
    'crates/ui-egui/src/tests_nativeoptions.rs',
    'crates/ui-egui/src/tests_saveext.rs',
}


def files(*globs):
    out = []
    for g in globs:
        out += sorted(ROOT.glob(g))
    return out


def main():
    changed = []
    tracked = subprocess.run(['git', '-C', str(ROOT), 'ls-files'], capture_output=True, text=True).stdout.split()
    for rel in tracked:
        p = ROOT / rel
        if not p.is_file() or rel.startswith('studio/ar-work') or rel == 'studio/anim_brand.py' or rel == '.github/workflows/studio-release.yml' or p.suffix in ('.png', '.icns', '.ico', '.ttf', '.otf'):
            continue
        try:
            s = p.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            continue
        o = s
        for a, b in EXACT:
            o = o.replace(a, b)
        if rel.endswith('.rs') and (rel.startswith('crates/ui-egui/') or rel.startswith('apps/') or rel in (
                'crates/engine/src/cmd/help.rs', 'crates/engine/src/cmd/fileio/mod.rs')):
            o = code(o)
        elif rel.startswith('crates/ui-egui/src/i18n/') and rel.endswith('.tsv'):
            rows = []
            for line in o.split('\n'):
                cells = line.split('\t')
                if len(cells) >= 3 and not line.startswith('#') and cells[1] != 'rsm':
                    named = APP.search(cells[1]) is not None
                    cells = [cells[0]] + [rename_app(c) for c in cells[1:]]
                    if rel.endswith('/ar.tsv') and named:
                        # rsm's Arabic name in rows about the app (not "رسم" the word for drawing).
                        cells[2:] = [c.replace('رسم', NAME_AR) for c in cells[2:]]
                    line = '\t'.join(cells)
                rows.append(line)
            o = '\n'.join(rows)
            # Catalog keys that name the app by upstream's name where the source says 7rrek.
            o = o.replace('\tVectorCraft doesn\'t search other apps', '\t7rrek doesn\'t search other apps')
            o = o.replace('\tnot a VectorCraft shortcut set', '\tnot a 7rrek shortcut set')
        elif rel == 'apps/vectorcraft-web/index.html':
            o = o.replace('<title>rsm', f'<title>{NAME}').replace('Loading rsm&hellip;', f'Loading {NAME}&hellip;')
        elif rel == 'packaging/macos/Info.plist.in':
            o = re.sub(r'(<key>CFBundle(?:Display)?Name</key>\s*<string>)rsm(</string>)', rf'\g<1>{NAME}\2', o)
        o = apply_targeted(rel, o)
        if rel in EXT_TEST_FILES:
            o = re.sub(r'\.rsm\b', '.7rrek', o)
        if o != s:
            p.write_text(o, encoding='utf-8')
            changed.append(rel)
    for c in changed:
        print('7rrek', c)


if __name__ == '__main__':
    main()
