#!/usr/bin/env python3
"""Apply the rsm branding on top of upstream VectorCraft.

Only user-visible text is rebranded; crate names, file formats and other internals keep their
upstream names so `git merge upstream/main` stays easy. The script is idempotent: after merging
upstream, resolve any conflicts in favour of upstream and run it again.

    python3 studio/rebrand.py
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NAME = 'rsm'
REPO = 'https://github.com/7mz410/CreativeStudio-7rrek'
SITE = 'https://studio.creative.ps'
# Persisted names that saved preferences and files refer to; renaming them would break those.
KEEP = ('VectorCraft Default',)

# Exact replacements, applied before the general rename.
SPECIFIC = {
    'crates/engine/src/cmd/help.rs': [
        ('pub const WEBSITE_URL: &str = "https://getartcraft.com";', f'pub const WEBSITE_URL: &str = "{SITE}";'),
        ('format!("{WEBSITE_URL}/apps/{APP_ID}")', 'WEBSITE_URL.to_string()'),
        ('format!("https://github.com/storytold/{APP_ID}")', f'"{REPO}".to_string()'),
        ('"Join Our Discord"', '"Upstream Community on Discord"'),
        ('the ArtCraft community Discord', 'the upstream ArtCraft community Discord'),
        ('"ArtCraft Website"', '"Creative Studio Project"'),
        ('["url"], "https://getartcraft.com/apps/vectorcraft");', f'["url"], "{SITE}");'),
        ('["url"], "https://github.com/storytold/vectorcraft");', f'["url"], "{REPO}");'),
        ('["website"], "https://getartcraft.com");', f'["website"], "{SITE}");'),
    ],
    'crates/engine/src/cmd/fileio/mod.rs': [
        ('label: "VectorCraft",', f'label: "{NAME}",'),
        ('label: "VectorCraft Template",', f'label: "{NAME} Template",'),
        ('extensions: &[vectorcraft_format::EXTENSION, vectorcraft_format::LEGACY_EXTENSION],',
         'extensions: &[vectorcraft_format::STUDIO_EXTENSION, vectorcraft_format::EXTENSION, vectorcraft_format::LEGACY_EXTENSION],'),
        ('pub const OPEN_EXTS: &[&str] = &[\n    "vectorcraft",', 'pub const OPEN_EXTS: &[&str] = &[\n    "rsm",\n    "vectorcraft",'),
        ('pub const PLACE_EXTS: &[&str] = &[\n    "vectorcraft",', 'pub const PLACE_EXTS: &[&str] = &[\n    "rsm",\n    "vectorcraft",'),
    ],
    'crates/ui-egui/src/place.rs': [
        ('let native = ["vectorcraft", vectorcraft_format::LEGACY_EXTENSION, "vctemplate"];',
         'let native = ["rsm", "vectorcraft", vectorcraft_format::LEGACY_EXTENSION, "vctemplate"];'),
    ],
    'crates/ui-egui/src/io.rs': [
        ('const TEMPLATE_EXTS: &[&str] = &["vctemplate", "ait", "vectorcraft", "drawcraft"];',
         'const TEMPLATE_EXTS: &[&str] = &["vctemplate", "ait", "rsm", "vectorcraft", "drawcraft"];'),
    ],
    # No Creative Studio Discord yet: drop the upstream community button and the ArtCraft website link.
    'crates/ui-egui/src/chrome.rs': [
        ('let discord = room >= with_search + 10.0 + gap + crate::community::discord_width(ui, false);',
         'let discord = false; // Creative Studio: no community Discord button yet'),
    ],
    'crates/ui-egui/src/titlebar.rs': [
        ('assert_eq!((has("Discord"), has("Search commands and tools")), (width > 1000.0, width > 1000.0), "{width}: {texts:?}");',
         'assert_eq!((has("Discord"), has("Search commands and tools")), (false, width > 1000.0), "{width}: {texts:?}");'),
    ],
    'crates/ui-egui/src/community.rs': [
        ('''    discord_button(app, ui, true);
    ui.add_space(8.0);
''', ''),
        ('''    link(app, ui, "globe", tl!("ArtCraft website"), "help.website", &s("website"));
''', ''),
        ('''                "https://getartcraft.com",
                "https://getartcraft.com/apps/vectorcraft",
                "https://github.com/storytold/vectorcraft"''',
         f'''                "{SITE}",
                "{SITE}",
                "{REPO}"'''),
    ],
    # Right to left (studio/RTL.md): the Arabic interface flips through egui's switch, and the
    # tool bar and the dock change sides.
    'crates/ui-egui/src/lib.rs': [
        ('pub mod background;\n',
         'pub mod agents;\npub mod background;\n'),
        ('    /// The macOS menu bar, when the desktop app installed one: the in-window menus are hidden then.\n    pub native_menu: Option<native_menu::NativeMenu>,\n',
         '    /// The macOS menu bar, when the desktop app installed one: the in-window menus are hidden then.\n    pub native_menu: Option<native_menu::NativeMenu>,\n    /// Creative Studio: Help › AI Agents (desktop): the control channel for agents and their settings.\n    pub agents: Option<Box<dyn agents::AgentsService>>,\n'),
        ('    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {\n        self.control_rx = Some(rx);\n        self\n    }\n',
         '    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {\n        self.control_rx = Some(rx);\n        self\n    }\n\n    /// Creative Studio: take requests from a control channel started while the app runs (Help › AI Agents).\n    pub fn set_control(&mut self, rx: Receiver<ControlRequest>) {\n        self.control_rx = Some(rx);\n    }\n'),
        ('        dialogs::show(self, &ctx);\n',
         '        dialogs::show(self, &ctx);\n        agents::show(self, &ctx);\n'),
        ('        i18n::set_current(lang);\n',
         '        i18n::set_current(lang);\n'
         '        // Creative Studio: Arabic lays the interface out right to left (vendor/egui). The switch is\n'
         '        // process-wide, so unit tests, which run in parallel, leave it off (tests/rtl.rs covers it).\n'
         '        #[cfg(not(test))]\n'
         '        egui::set_rtl(crate::i18n::bidi::RTL_CODES.contains(&lang.code()));\n'),
    ],
    'crates/ui-egui/src/widgets.rs': [
        ('    let bx = Rect::from_min_size(pos2(rect.left(), rect.center().y - 6.5), Vec2::splat(13.0));\n    ui.painter().galley(pos2(bx.right() + 5.0, rect.center().y - galley.size().y / 2.0), galley, t.text);',
         '    // Creative Studio: right to left, the box sits on the right and its label to its left.\n    let (bx, text_x) = if egui::is_rtl() {\n        let bx = Rect::from_min_size(pos2(rect.right() - 13.0, rect.center().y - 6.5), Vec2::splat(13.0));\n        (bx, bx.left() - 5.0 - galley.size().x)\n    } else {\n        let bx = Rect::from_min_size(pos2(rect.left(), rect.center().y - 6.5), Vec2::splat(13.0));\n        (bx, bx.right() + 5.0)\n    };\n    ui.painter().galley(pos2(text_x, rect.center().y - galley.size().y / 2.0), galley, t.text);'),
    ],
    # Help › AI Agents (crates/ui-egui/src/agents.rs).
    'crates/ui-egui/src/menus.rs': [
        ('    ("help.about", "About VectorCraft", "", "{}"),\n',
         '    ("help.about", "About VectorCraft", "", "{}"),\n    ("help.agents", "AI Agents…", "", "{} opens the AI Agents window: allow agents to control the app, connect agent apps"),\n'),
        ('        "help.about" => {\n            app.ui.about = true;\n            Ok(Value::Null)\n        }\n',
         '        "help.about" => {\n            app.ui.about = true;\n            Ok(Value::Null)\n        }\n        "help.agents" => {\n            crate::agents::open();\n            Ok(Value::Null)\n        }\n'),
        ('                Sep,\n                c("Search Commands…", "help.commandPalette"),',
         '                Sep,\n                c("AI Agents…", "help.agents"),\n                c("Search Commands…", "help.commandPalette"),'),
    ],
    'crates/ui-egui/src/toolbar.rs': [
        ('egui::Panel::left("toolbar")', '(if egui::is_rtl() { egui::Panel::right("toolbar") } else { egui::Panel::left("toolbar") })'),
    ],
    'crates/ui-egui/src/dock.rs': [
        ('egui::Panel::right("dock")', '(if egui::is_rtl() { egui::Panel::left("dock") } else { egui::Panel::right("dock") })'),
        ('egui::Panel::right("icon_column")', '(if egui::is_rtl() { egui::Panel::left("icon_column") } else { egui::Panel::right("icon_column") })'),
    ],
    'crates/ui-egui/src/dialogs/about.rs': [
        ('"Part of ArtCraft. MIT OR Apache-2.0.',
         '"Based on VectorCraft by the ArtCraft team. MIT OR Apache-2.0.'),
    ],
}
# Creative Studio saves documents as `.rsm` (the same contents as `.vectorcraft`, which still opens), with
# its own document icon in Finder. Applied as is, without the general rename.
EXT_FILES = {
    'crates/format/src/lib.rs': [
        ('pub const LEGACY_EXTENSION: &str = "drawcraft";\n',
         'pub const LEGACY_EXTENSION: &str = "drawcraft";\n'
         '/// rsm saves documents as `.rsm` (the same contents as `.vectorcraft`).\n'
         'pub const STUDIO_EXTENSION: &str = "rsm";\n'),
        ('    ext.eq_ignore_ascii_case(EXTENSION) || ext.eq_ignore_ascii_case(LEGACY_EXTENSION)',
         '    ext.eq_ignore_ascii_case(STUDIO_EXTENSION) || ext.eq_ignore_ascii_case(EXTENSION) || ext.eq_ignore_ascii_case(LEGACY_EXTENSION)'),
    ],
    'crates/engine/src/cmd/package.rs': [
        ('format!("{stem}.{}", vectorcraft_format::EXTENSION)', 'format!("{stem}.{}", vectorcraft_format::STUDIO_EXTENSION)'),
    ],
    'packaging/macos/Info.plist.in': [
        ('<string>VectorCraft Document</string>', '<string>rsm Document</string>'),
        # Our own document type: a type shared with other VectorCraft builds lets Finder show their icon.
        ('ai.storyteller.vectorcraft.document', 'ps.creative.studio.7rrek.document'),
        ('<key>UTTypeIconFile</key>\n      <string>VectorCraft</string>', '<key>UTTypeIconFile</key>\n      <string>rsm-document</string>'),
        ('<array><string>vectorcraft</string><string>drawcraft</string></array>',
         '<array><string>rsm</string><string>vectorcraft</string><string>drawcraft</string></array>'),
    ],
    'xtask/src/bundle.rs': [
        ('.map_err(|e| format!("copy icon: {e}"))?;\n',
         '.map_err(|e| format!("copy icon: {e}"))?;\n'
         '    std::fs::copy(root.join("assets/app-icon/rsm-document.icns"), app.join("Resources/rsm-document.icns"))\n'
         '        .map_err(|e| format!("copy document icon: {e}"))?;\n'),
        ('"<array><string>vectorcraft</string><string>drawcraft</string></array>"',
         '"<array><string>rsm</string><string>vectorcraft</string><string>drawcraft</string></array>"'),
    ],
    'Cargo.toml': [
        ('[workspace.dependencies]\n',
         '[patch.crates-io]\n# Creative Studio: egui with a right-to-left switch for the Arabic interface (studio/RTL.md).\negui = { path = "vendor/egui" }\n\n[workspace.dependencies]\n'),
    ],
    'apps/vectorcraft/src/main.rs': [
        ('mod printing;\n',
         'mod printing;\nmod studio_agents;\n'),
        ('                if let Some(port) = control_port {\n                    let rx = control_server::start(port, cc.egui_ctx.clone());\n                    app = app.with_control(rx);\n                }\n',
         '                if let Some(port) = control_port {\n                    let rx = control_server::start(port, cc.egui_ctx.clone());\n                    app = app.with_control(rx);\n                }\n                // Creative Studio: Help › AI Agents; allowed agents connect from the start.\n                let mut agents = studio_agents::Agents::new(prefs_path().and_then(|p| Some(p.parent()?.to_path_buf())), cc.egui_ctx.clone());\n                if control_port.is_some() {\n                    // `--control` already serves agents on its own port.\n                    agents.mark_started();\n                } else if let Some(rx) = agents.start_if_enabled() {\n                    app = app.with_control(rx);\n                }\n                app.services.agents = Some(Box::new(agents));\n'),
    ],
    'crates/engine/src/cmd/fileio/tests.rs': [
        ('        exts,\n        [\n            "vectorcraft",', '        exts,\n        [\n            "rsm",'),
    ],
}
# Tests that expect the names native saves suggest: their `.vectorcraft` file names become `.rsm`.
EXT_TESTS = {
    'crates/engine/src/cmd/fileio/tests_affinity.rs',
    'crates/engine/src/tests_package.rs',
    'crates/engine/src/tests_save.rs',
    'crates/engine/src/tests_saveoptions.rs',
    'crates/ui-egui/src/dialogs/save_options.rs',
    'crates/ui-egui/src/dialogs/tests_package.rs',
    'crates/ui-egui/src/tests_nativeoptions.rs',
    'crates/ui-egui/src/tests_saveext.rs',
}
for _t in EXT_TESTS:
    EXT_FILES.setdefault(_t, [])
# Exact replacements inside string literals and catalogs, before the general rename.
LITERALS = [
    ('VectorCraft on getartcraft.com', 'Creative Studio Website'),
    (f'{NAME} Website', 'Creative Studio Website'),
    ('ArtCraft Website', 'Creative Studio Project'),
    ('Join Our Discord', 'Upstream Community on Discord'),
]
# Catalog rows for strings the rebranded UI no longer shows.
DROPPED_ROWS = {'ArtCraft website'}
# The About credit line must keep naming VectorCraft.
CREDIT = 'Based on VectorCraft by the ArtCraft team'

STRING = re.compile(r'"(?:[^"\\\n]|\\.)*"')


def rename(text: str) -> str:
    for a, b in LITERALS:
        text = text.replace(a, b)
    if CREDIT in text or any(k in text for k in KEEP):
        return text
    return text.replace('VectorCraft', NAME)


def rebrand_rust(path: Path) -> bool:
    src = path.read_text(encoding='utf-8')
    out = src
    for a, b in SPECIFIC.get(path.relative_to(ROOT).as_posix(), []):
        if not b or b not in out:  # some replacements contain what they replace
            out = out.replace(a, b)
    out = STRING.sub(lambda m: rename(m.group(0)), out)
    if out != src:
        path.write_text(out, encoding='utf-8')
    return out != src


def source_strings() -> set:
    """Every string literal in the rebranded sources, without quotes."""
    found = set()
    for p in RUST_FILES():
        found.update(m.group(0)[1:-1] for m in STRING.finditer(p.read_text(encoding='utf-8')))
    return found


def rebrand_catalog(path: Path, sources: set) -> bool:
    # Rows are `context<TAB>English<TAB>translation`. A row is renamed only when its renamed English
    # text is a string the rebranded sources use; rows for other crates' messages keep matching them.
    src = path.read_text(encoding='utf-8')
    rows = []
    for line in src.split('\n'):
        cells = line.split('\t')
        if len(cells) >= 3 and not line.startswith('#') and cells[1] in DROPPED_ROWS:
            continue
        if len(cells) >= 3 and not line.startswith('#'):
            renamed = rename(cells[1])
            if renamed != cells[1] and renamed in sources:
                line = '\t'.join([cells[0], renamed, *(rename(c) for c in cells[2:])])
        rows.append(line)
    out = '\n'.join(rows)
    if out != src:
        path.write_text(out, encoding='utf-8')
    return out != src


def rebrand_plist(path: Path) -> bool:
    src = path.read_text(encoding='utf-8')
    out = re.sub(r'(<key>CFBundle(?:Display)?Name</key>\s*<string>)VectorCraft(</string>)', rf'\g<1>{NAME}\2', src)
    out = out.replace('<string>ai.storyteller.vectorcraft</string>', '<string>ps.creative.studio.7rrek</string>')
    if out != src:
        path.write_text(out, encoding='utf-8')
    return out != src


def RUST_FILES():
    return sorted((ROOT / 'crates/ui-egui/src').rglob('*.rs')) + [ROOT / p for p in SPECIFIC if p.startswith('crates/engine')]


def main():
    changed = []
    for p in RUST_FILES():
        if rebrand_rust(p):
            changed.append(p)
    sources = source_strings()
    # Rows for Creative Studio's own UI strings (studio/i18n/<lang>.tsv; Arabic comes from ar-work).
    for extra in sorted((ROOT / 'studio/i18n').glob('*.tsv')):
        cat = ROOT / 'crates/ui-egui/src/i18n' / extra.name
        src = cat.read_text(encoding='utf-8')
        keys = {tuple(l.split('\t')[:2]) for l in src.split('\n') if not l.startswith('#')}
        rows = [l for l in extra.read_text(encoding='utf-8').split('\n') if l and tuple(l.split('\t')[:2]) not in keys]
        if rows:
            cat.write_text(src.rstrip('\n') + '\n' + '\n'.join(rows) + '\n', encoding='utf-8')
            changed.append(cat)
    for p in sorted((ROOT / 'crates/ui-egui/src/i18n').glob('*.tsv')):
        if rebrand_catalog(p, sources):
            changed.append(p)
    web = ROOT / 'apps/vectorcraft-web/index.html'
    html = web.read_text(encoding='utf-8')
    branded = html.replace('<title>VectorCraft</title>', f'<title>{NAME}</title>').replace('Loading VectorCraft&hellip;', f'Loading {NAME}&hellip;')
    if branded != html:
        web.write_text(branded, encoding='utf-8')
        changed.append(web)
    if rebrand_plist(ROOT / 'packaging/macos/Info.plist.in'):
        changed.append(ROOT / 'packaging/macos/Info.plist.in')
    for rel, pairs in EXT_FILES.items():
        path = ROOT / rel
        src = path.read_text(encoding='utf-8')
        out = src
        for a, b in pairs:
            # 7rrek renames the constant on top of this; don't add a second one.
            if a.startswith('pub const LEGACY_EXTENSION') and 'pub const STUDIO_EXTENSION' in out:
                continue
            # Cargo.toml: one [patch.crates-io] only, even when the merge kept our other pins in it.
            if b.startswith('[patch.crates-io]') and '[patch.crates-io]' in out:
                continue
            if not b or b not in out:
                out = out.replace(a, b)
        if rel in EXT_TESTS:
            out = re.sub(r'\.vectorcraft\b(?!_)', '.rsm', out)
        if out != src:
            path.write_text(out, encoding='utf-8')
            changed.append(path)
    for p in changed:
        print('rebranded', p.relative_to(ROOT))


# Creative Studio: the macOS app folder in the .dmg and the volume name carry our name
# (packaging/macos/package.sh). Bundle and binary names inside the package stay as upstream has them.
PACKAGING_NAMES = {
    'packaging/macos/package.sh': [
        # The document icon the Info.plist names, and the CLI (MCP) inside the app, under our name.
        ('cp "$ROOT/assets/app-icon/vectorcraft.icns" "$APP/Contents/Resources/VectorCraft.icns"\n',
         'cp "$ROOT/assets/app-icon/vectorcraft.icns" "$APP/Contents/Resources/VectorCraft.icns"\n'
         'cp "$ROOT/assets/app-icon/rsm-document.icns" "$APP/Contents/Resources/rsm-document.icns"\n'
         'cp "$WORK/bin/vectorcraft-cli" "$APP/Contents/MacOS/7rrek-cli"\n'),
        ('sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/VectorCraft"\n',
         'sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/7rrek-cli"\n'
         'sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/VectorCraft"\n'),
        ('VectorCraft.app', '7rrek.app'),
        ('-volname "VectorCraft"', '-volname "7rrek"'),
        ('-hfs-volume-name "VectorCraft"', '-hfs-volume-name "7rrek"'),
    ],
}


def apply_packaging_names() -> list:
    changed = []
    for rel, pairs in PACKAGING_NAMES.items():
        path = ROOT / rel
        src = path.read_text(encoding='utf-8')
        out = src
        for a, b in pairs:
            if b not in out:
                out = out.replace(a, b)
        if out != src:
            path.write_text(out, encoding='utf-8')
            changed.append(path)
    return changed


if __name__ == '__main__':
    main()
    for _p in apply_packaging_names():
        print('rebranded', _p.relative_to(ROOT))


# Creative Studio: the app's Arabic name in its Arabic interface (studio/arabic_names.py).
import sys  # noqa: E402
sys.path.insert(0, str(Path(__file__).resolve().parent))
import arabic_names  # noqa: E402
for _p in arabic_names.apply(ROOT, NAME, "رسم"):
    print("rebranded", _p.relative_to(ROOT))
