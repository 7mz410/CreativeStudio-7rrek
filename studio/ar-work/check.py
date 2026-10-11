"""Check one translated batch against its keys: line count and placeholders."""
import re, sys
S = sys.argv[1]; n = sys.argv[2]
keys = open(f'{S}/part_{n}', encoding='utf-8').read().split('\n')
if keys[-1] == '': keys.pop()
tr = open(f'{S}/tr_{n}', encoding='utf-8').read().split('\n')
if tr[-1] == '': tr.pop()
assert len(keys) == len(tr), (len(keys), len(tr))
ph = lambda s: sorted(re.findall(r'\{[^}]*\}', s))
bad = 0
for k, t in zip(keys, tr):
    ctx, src = k.split('\t', 1)
    forms = t.split('|') if ctx == '@plural' else [t]
    for f in forms:
        if ph(src.split('|')[-1]) != ph(f) and ph(src.split('|')[0]) != ph(f):
            print('PLACEHOLDER', repr(src), '->', repr(f)); bad += 1
print(n, 'ok' if not bad else f'{bad} bad', len(keys))
