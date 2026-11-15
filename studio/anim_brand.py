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
    'crates/ui-egui/src/i18n/tests.rs': [('.find(|m| m.title == "rsm")', '.find(|m| m.title == "7rrek")'),
                                         # The product name kept in English, and the format's file-filter label.
                                         ('const MENU_KEEP_AS_IS: &[&str] = &[\n    "rsm",', 'const MENU_KEEP_AS_IS: &[&str] = &[\n    "7rrek",'),
                                         ('for source in ["SVG", "PNG", "rsm", "unregistered filter"]', 'for source in ["SVG", "PNG", "7rrek", "unregistered filter"]')],
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
    # Windows VERSIONINFO: the name Task Manager, Open With and the file's properties show.
    'apps/vectorcraft/build.rs': [
        ('.set("ProductName", "VectorCraft")', '.set("ProductName", "7rrek")'),
        ('.set("FileDescription", "VectorCraft vector illustration editor")', '.set("FileDescription", "7rrek animation")'),
        ('.set("OriginalFilename", "vectorcraft.exe")', '.set("OriginalFilename", "7rrek.exe")'),
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

# Upstream split some files (sync 2026-11-14): rules move with the text they match, and rules for
# text upstream rewrote are added in its new form.
MOVED = [
    ('crates/ui-egui/src/menus.rs', '.filter(', 'crates/ui-egui/src/menus/bar.rs'),
    ('crates/ui-egui/src/menus.rs', '            "rsm",', 'crates/ui-egui/src/menus/tree.rs'),
    ('crates/ui-egui/src/tests_nativemenu.rs', '', 'crates/ui-egui/src/tests/nativemenu.rs'),
    ('crates/engine/src/cmd/fileio/mod.rs', '', 'crates/engine/src/cmd/fileio/formats.rs'),
    ('crates/ui-egui/src/io.rs', '', 'crates/ui-egui/src/io/open.rs'),
    ('crates/ui-egui/src/chrome.rs', 'window_title(', 'crates/ui-egui/src/chrome/tests.rs'),
    ('crates/ui-egui/src/chrome.rs', 'assert_eq!(app.last', 'crates/ui-egui/src/chrome/tests.rs'),
    ('crates/ui-egui/src/chrome.rs', 'ViewportCommand', 'crates/ui-egui/src/chrome/tests.rs'),
    ('crates/ui-egui/src/dialogs/save_options.rs', '', 'crates/ui-egui/src/dialogs/save_options/tests.rs'),
]
for old, start, new in MOVED:
    move = [p for p in TARGETED.get(old, []) if p[0].startswith(start)]
    TARGETED[old] = [p for p in TARGETED.get(old, []) if p not in move]
    TARGETED.setdefault(new, []).extend(move)
for rel, pairs in {
    'apps/vectorcraft/src/titlebar.rs': [('.with_title("rsm")', '.with_title("7rrek")')],
    'apps/vectorcraft/build.rs': [('.set("ProductName", "rsm")', '.set("ProductName", "7rrek")'),
                                  ('.set("FileDescription", "7rrek vector illustration editor")', '.set("FileDescription", "7rrek animation")')],
    'apps/vectorcraft/src/mac_save_format.rs': [('Some(&["rsm".to_owned()][..])', 'Some(&["7rrek".to_owned()][..])')],
    'crates/engine/src/cmd/findfiles/walk.rs': [('"rsm doesn\'t search other apps', '"7rrek doesn\'t search other apps')],
    'apps/vectorcraft-cli/tests/it/fileio.rs': [('Readable formats: .rsm"', 'Readable formats: .7rrek"')],
    'crates/engine/src/cmd/fileio/tests.rs': [('        [\n            "rsm",\n            "svg",', '        [\n            "7rrek",\n            "svg",')],
    # Linux: .7rrek is a VectorCraft document too, listed before .rsm (which still opens).
    'packaging/linux/ai.storyteller.vectorcraft.mime.xml': [
        ('    <glob pattern="*.rsm"/>\n', '    <glob pattern="*.7rrek"/>\n    <glob pattern="*.rsm"/>\n'),
    ],
    'apps/vectorcraft/src/tests.rs': [
        ('    ("rsm", "application/x-vectorcraft"),\n', '    ("7rrek", "application/x-vectorcraft"),\n    ("rsm", "application/x-vectorcraft"),\n'),
    ],
    # Engine and format messages the user reads, outside the files code() renames.
    'crates/engine/src/cmd/fileio/formats.rs': [('        label: "rsm Template",', '        label: "7rrek Template",')],
    'crates/format/src/lib.rs': [('rsm writes versions {MIN_VERSION} to {VERSION}', '7rrek writes versions {MIN_VERSION} to {VERSION}')],
    'crates/engine/src/cmd/prefscmds.rs': [('a newer version of rsm (version {v})', 'a newer version of 7rrek (version {v})')],
    'crates/engine/src/tests/prefs_file.rs': [('"newer version of rsm (version 2)"', '"newer version of 7rrek (version 2)"')],
    # The window title is the app's name; 7rrek's menu bar has an Animation menu too.
    'crates/ui-egui/tests/safe_mode.rs': [
        ('assert_eq!(chrome::window_title(&app), "rsm");', 'assert_eq!(chrome::window_title(&app), "7rrek");'),
        ('let menu_names = ["File", "Edit", "Object", "Type", "Select", "Effect", "View", "Window", "Help"];',
         'let menu_names = ["File", "Edit", "Object", "Type", "Select", "Effect", "View", "Animation", "Window", "Help"];'),
    ],
    # A bare name typed in the save sheet gets the default format's extension, `.7rrek`.
    'crates/ui-egui/src/tests/picks.rs': [
        ('|p: &str| p == "/art/card.rsm"));', '|p: &str| p == "/art/card.7rrek"));'),
        ('assert_eq!(*written.borrow(), ["/art/card.rsm"]);', 'assert_eq!(*written.borrow(), ["/art/card.7rrek"]);'),
    ],
    # The suggested name's extension in another case.
    'crates/ui-egui/src/tests/saveext.rs': [('&["/art/Untitled-1.RSM", "ai"]', '&["/art/Untitled-1.7RREK", "ai"]')],
    # Unused-option notes name the format by its label, which is the app's name for its own files.
    'crates/engine/src/cmd/fileio/tests_exportas.rs': [
        ("`marks` was not used: rsm files don't read it\", \"`svg` was not used: rsm files don't read it",
         "`marks` was not used: 7rrek files don't read it\", \"`svg` was not used: 7rrek files don't read it"),
        ("`artboard` was not used: rsm files don't read it", "`artboard` was not used: 7rrek files don't read it"),
        ('[("vectorcraft", "rsm"), ("template", "rsm Template")]', '[("vectorcraft", "7rrek"), ("template", "7rrek Template")]'),
    ],
    'crates/engine/src/cmd/fileio/tests_screens.rs': [
        ("`formats[0].separateArtboards` was not used: rsm files don't read it", "`formats[0].separateArtboards` was not used: 7rrek files don't read it"),
    ],
    'crates/engine/src/cmd/findfiles/tests.rs': [
        ('("refused", "rsm doesn\'t search other apps', '("refused", "7rrek doesn\'t search other apps'),
    ],
}.items():
    TARGETED.setdefault(rel, []).extend(pairs)


def apply_targeted(rel: str, text: str) -> str:
    for a, b in TARGETED.get(rel, []):
        if b not in text:
            if a not in text:
                print(f'MISSING in {rel}: {a[:70]!r}')
            text = text.replace(a, b)
    return text


# Tests whose expectations name a saved document `.rsm`: they now expect `.7rrek`. Opening `.rsm` is
# still covered by crates/format/tests/studio_extension.rs. Idempotent: no `.rsm` left after one run.
EXT_TEST_FILES = {
    'crates/engine/src/tests/package.rs',
    'crates/engine/src/tests/save.rs',
    'crates/engine/src/tests/saveoptions.rs',
    'crates/engine/src/cmd/fileio/tests.rs',
    'crates/engine/src/cmd/fileio/tests_affinity.rs',
    'crates/engine/src/cmd/fileio/tests_batch.rs',
    'crates/ui-egui/src/dialogs/tests_package.rs',
    'crates/ui-egui/src/dialogs/save_options.rs',
    'crates/ui-egui/src/dialogs/save_options/tests.rs',
    'crates/engine/src/cmd/addons/document/tests.rs',
    'crates/ui-egui/src/addons/document/tests.rs',
    'crates/ui-egui/src/tests/nativeoptions.rs',
    'crates/ui-egui/src/tests/saveext.rs',
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
            # rebrand.py adds studio/i18n rows whose rsm key isn't in the catalog yet; renamed, a key
            # can repeat one renamed before, so keep each (kind, key) row once.
            # The one kept is the first that names the app as 7rrek (not upstream's or rsm's name).
            old_names = ('VectorCraft', 'rsm', 'وكتور') + (('رسم',) if rel.endswith('/ar.tsv') else ())
            best = {}
            for n, line in enumerate(rows):
                cells = line.split('\t')
                if len(cells) >= 3 and not line.startswith('#'):
                    stale = any(w in c for c in cells[2:] for w in old_names)
                    key = tuple(cells[:2])
                    if key not in best or (best[key][1] and not stale):
                        best[key] = (n, stale)
            keep = {n for n, _ in best.values()}
            o = '\n'.join(line for n, line in enumerate(rows)
                           if n in keep or len(line.split('\t')) < 3 or line.startswith('#'))
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
