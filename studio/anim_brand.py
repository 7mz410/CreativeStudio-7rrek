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
    ('CreativeStudio-rsm', 'CreativeStudio-7rrek'),
    ('rsm.studio.creative.ps', '7rrek.studio.creative.ps'),
    ('ps.creative.studio.rsm', 'ps.creative.studio.7rrek'),
    ('"studio-rsm"', '"studio-7rrek"'),
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
        if not p.is_file() or rel.startswith('studio/ar-work') or p.suffix in ('.png', '.icns', '.ico', '.ttf', '.otf'):
            continue
        try:
            s = p.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            continue
        o = s
        for a, b in EXACT:
            o = o.replace(a, b)
        if rel.endswith('.rs') and (rel.startswith('crates/ui-egui/') or rel.startswith('apps/') or rel in (
                'crates/engine/src/cmd/help.rs',)):
            o = code(o)
        elif rel.startswith('crates/ui-egui/src/i18n/') and rel.endswith('.tsv'):
            rows = []
            for line in o.split('\n'):
                cells = line.split('\t')
                if len(cells) >= 3 and not line.startswith('#'):
                    named = APP.search(cells[1]) is not None
                    cells = [cells[0]] + [rename_app(c) for c in cells[1:]]
                    if rel.endswith('/ar.tsv') and named:
                        # rsm's Arabic name in rows about the app (not "رسم" the word for drawing).
                        cells[2:] = [c.replace('رسم', NAME_AR) for c in cells[2:]]
                    line = '\t'.join(cells)
                rows.append(line)
            o = '\n'.join(rows)
        elif rel == 'apps/vectorcraft-web/index.html':
            o = o.replace('<title>rsm', f'<title>{NAME}').replace('Loading rsm&hellip;', f'Loading {NAME}&hellip;')
        elif rel == 'packaging/macos/Info.plist.in':
            o = re.sub(r'(<key>CFBundle(?:Display)?Name</key>\s*<string>)rsm(</string>)', rf'\g<1>{NAME}\2', o)
        if o != s:
            p.write_text(o, encoding='utf-8')
            changed.append(rel)
    for c in changed:
        print('7rrek', c)


if __name__ == '__main__':
    main()
