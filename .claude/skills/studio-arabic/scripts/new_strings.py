#!/usr/bin/env python3
"""List interface strings that the Arabic catalog doesn't translate yet, as a new batch.

    python3 new_strings.py [--write]

The reference is the Spanish catalog, which upstream keeps complete. With --write, the missing
rows go to studio/ar-work/part_NN (the next free number), ready to translate into tr_NN.
"""
import sys
from pathlib import Path

ROOT = Path.home() / 'Documents/CreativeStudio/rsm'
I18N = ROOT / 'crates/ui-egui/src/i18n'
WORK = ROOT / 'studio/ar-work'


def keys(path: Path) -> list:
    out = []
    for line in path.read_text(encoding='utf-8').split('\n'):
        if line.startswith('#') or '\t' not in line:
            continue
        cells = line.split('\t')
        if len(cells) >= 3:
            out.append(f'{cells[0]}\t{cells[1]}')
    return out


done = set(keys(I18N / 'ar.tsv'))
for part in WORK.glob('part_*'):
    done.update(l for l in part.read_text(encoding='utf-8').split('\n') if l)
missing = [k for k in dict.fromkeys(keys(I18N / 'es.tsv')) if k not in done]
print(f'{len(missing)} strings without Arabic')
for k in missing[:20]:
    print('  ', k.split('\t', 1)[1])
if '--write' in sys.argv and missing:
    n = max((int(p.name[5:]) for p in WORK.glob('part_*')), default=-1) + 1
    out = WORK / f'part_{n:02d}'
    out.write_text('\n'.join(missing) + '\n', encoding='utf-8')
    print(f'wrote {out.relative_to(ROOT)}; translate it into tr_{n:02d}')
