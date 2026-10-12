"""The app's Arabic name in its Arabic interface: "حول رسم", not "حول rsm".

Copied into each app's studio/ folder and called at the end of studio/rebrand.py:

    import arabic_names
    changed += arabic_names.apply(ROOT, 'rsm', 'رسم')

Only the translation side of Arabic catalogs changes (TSV third column, JSON values, the second
string of `ar.rs` rows). A Latin name glued to a dot or a word (`.rsm`, `rsm-cli`) is left alone.
"""
import json
import re
from pathlib import Path


def _swap(text: str, latin: str, arabic: str) -> str:
    return re.sub(r'(?<![\w./-])' + re.escape(latin) + r'(?![\w/-]|\.\w)', arabic, text)


def apply(root: Path, latin: str, arabic: str) -> list:
    changed = []
    for p in list(root.glob('crates/ui-egui/src/i18n/ar.tsv')) + list(root.glob('crates/ui-egui/src/i18n/ar/*.tsv')):
        s = p.read_text(encoding='utf-8')
        rows = []
        for line in s.split('\n'):
            cells = line.split('\t')
            if len(cells) >= 3 and not line.startswith('#'):
                line = '\t'.join(cells[:2] + [_swap(c, latin, arabic) for c in cells[2:]])
            rows.append(line)
        o = '\n'.join(rows)
        if o != s:
            p.write_text(o, encoding='utf-8')
            changed.append(p)
    for p in root.glob('crates/ui-egui/locales/ar*.json'):
        s = p.read_text(encoding='utf-8')
        data = json.loads(s)
        new = {k: _swap(v, latin, arabic) if isinstance(v, str) else v for k, v in data.items()}
        if new != data:
            p.write_text(json.dumps(new, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
            changed.append(p)
    return changed
