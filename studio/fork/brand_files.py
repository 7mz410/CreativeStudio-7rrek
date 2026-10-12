#!/usr/bin/env python3
"""Creative Studio fork, the non-code part: app icon, attribution rows, README, NOTICE, CHANGES.

Run once from a fresh fork, after `docs/studio/<app>.svg` is in place and before the first commit:

    python3 ~/Documents/CreativeStudio/rsm/studio/fork/brand_files.py <repo> <app> <Upstream> <slug> "<what>" "<ما هو>"
    e.g. ... klmat klmat WordCraft wordcraft "word processing" "معالجة النصوص"

It removes upstream's docs/brand/ (ArtCraft trademarks), points the app-icon rows of ASSETS.md or
ATTRIBUTION.md at the new icon, moves upstream's README to docs/UPSTREAM-README.md, and writes our
README, the NOTICE fork paragraph and studio/CHANGES.md. Re-running it is safe.
"""
import re
import subprocess
import sys
from pathlib import Path

repo, app, up, slug, what, what_ar = Path(sys.argv[1]).resolve(), *sys.argv[2:7]
CS = repo.parent
SUITE = [  # app, what it's for, upstream
    ('rsm', 'Vector illustration', 'VectorCraft'), ('ttshat', 'Photo library and raw development', 'LightCraft'),
    ('tlween', 'Image editing', 'PhotoCraft'), ('tsweer', 'Video editing', 'FilmCraft'),
    ('effectat', 'Motion graphics and effects', 'EffectCraft'), ('sf7aat', 'PDF documents', 'PdfCraft'),
    ('trteeb', 'Page layout', 'DesignCraft'), ('klmat', 'Word processing', 'WordCraft'),
    ('arqam', 'Spreadsheets', 'GridCraft'), ('shoof', 'Presentations', 'DeckCraft'),
    ('muhandes', 'Computer-aided design', 'CADCraft'),
]


def git(*a):
    return subprocess.run(['git', '-C', str(repo), *a], capture_output=True, text=True).stdout


# 1. App icon: the app's star on a navy tile, in the frame the icon script expects (rounded 512).
logo = (repo / f'docs/studio/{app}.svg').read_text(encoding='utf-8')
logo = re.sub(r'<\?xml[^>]*\?>\s*', '', logo)
logo = re.sub(r'<title>.*?</title>', '', logo, flags=re.S)
vb = re.search(r'viewBox="([^"]+)"', logo).group(1)
body = re.sub(r'^<svg[^>]*>|</svg>\s*$', '', logo.strip())
icon = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><defs><clipPath id="t"><rect width="512" height="512" rx="112"/></clipPath></defs>'
        f'<g clip-path="url(#t)"><rect width="512" height="512" fill="#0d2d3f"/><svg x="80" y="80" width="352" height="352" viewBox="{vb}">{body}</svg></g></svg>\n')
for svg in (repo / 'assets/app-icon').glob('*.svg'):
    if 'document' not in svg.name:
        svg.write_text(icon, encoding='utf-8')
if (repo / 'packaging/icons.sh').exists():
    subprocess.run(['bash', str(repo / 'packaging/icons.sh')], check=True, capture_output=True)
(repo / 'assets/app-icon/README.md').write_text(
    f'# {app} app icon\n\n**Artwork:** the {app} star (`docs/studio/{app}.svg`, by Hamza Abu Ayyash for Creative Studio) on a navy '
    '(#0d2d3f) tile. Regenerate every size with `packaging/icons.sh`.\n', encoding='utf-8')

# 2. Upstream brand files go (their brand licence requires it of forks).
if (repo / 'docs/brand').exists():
    git('rm', '-rq', 'docs/brand')

# 3. Attribution table.
for name in ('ASSETS.md', 'ATTRIBUTION.md'):
    f = repo / name
    if not f.exists():
        continue
    t = f.read_text(encoding='utf-8')
    t = re.sub(r'^\| `docs/brand/[^\n]*\n', '', t, flags=re.M)
    t = re.sub(r' The one exception is the ArtCraft brand files in `docs/brand/`[^.]*\.[^.]*\.',
               f' This fork ({app}, Creative Studio) carries no ArtCraft marks: upstream\'s `docs/brand/` was removed as its brand licence requires of forks.', t)
    rows = []
    for line in t.split('\n'):
        if line.startswith('| `assets/app-icon/') and 'README' not in line.split('|')[1]:
            cells = line.split('|')
            path = cells[1]
            line = f'|{path}| Hamza Abu Ayyash (Creative Studio) | the {app} star (`docs/studio/{app}.svg`) on a navy tile, rendered by `packaging/icons.sh` | MIT OR Apache-2.0 | App icon (see `assets/app-icon/README.md`) |'
        rows.append(line)
    t = '\n'.join(rows)
    if f'`docs/studio/{app}.svg`' not in t.split('|---')[-1]:
        first = re.search(r'^\| `', t, flags=re.M)
        row = f'| `docs/studio/{app}.svg` | Hamza Abu Ayyash (Creative Studio) | Original artwork | MIT OR Apache-2.0 | {app} logo; replaces the upstream brand |\n'
        t = t[:first.start()] + row + t[first.start():]
    f.write_text(t, encoding='utf-8')

# 4. README (upstream's moves to docs/), NOTICE, CHANGES.
if not (repo / 'docs/UPSTREAM-README.md').exists():
    git('mv', 'README.md', 'docs/UPSTREAM-README.md')
table = '\n'.join(
    f'| **{a}** | {w} | {u} |' if a == app else f'| [{a}](https://github.com/7mz410/CreativeStudio-{a}) | {w} | {u} |'
    for a, w, u in SUITE)
(repo / 'README.md').write_text(f'''<p align="center">
  <img alt="{app}" src="docs/studio/{app}.svg" width="140">
</p>

<h1 align="center">{app}</h1>

<p align="center">
  <b>Free, open-source {what} — part of the Creative Studio creative suite.</b><br>
  <b>{what_ar}، مجاني ومفتوح المصدر — جزء من حزمة Creative Studio.</b>
</p>

<p align="center">
  <a href="https://{app}.studio.creative.ps/">{app}.studio.creative.ps</a> ·
  <a href="https://studio.creative.ps/">studio.creative.ps</a>
</p>

<p align="center">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-555555">
  <img alt="Based on {up}" src="https://img.shields.io/badge/based%20on-{up}-0d2d3f">
</p>

## Built on {up}

{app} is a fork of **[{up}](https://github.com/storytold/{slug})**, the open-source, clean-room
application written in Rust by the **[ArtCraft](https://getartcraft.com/)** team and the {up}
contributors. Almost everything that makes this app work is their work. Thank you. If you like
{app}, please star and support the upstream project as well.

What we add: the Creative Studio identity, the Arabic interface laid out right to left, and Arabic
features to come. Our changes are listed in [`studio/CHANGES.md`](studio/CHANGES.md).
Upstream's own README, with the full feature list and build instructions, is in
[`docs/UPSTREAM-README.md`](docs/UPSTREAM-README.md).

## The Creative Studio suite

| App | What it's for | Based on |
|---|---|---|
{table}

## Building

```sh
cargo run --release -p {slug}
# after merging upstream:
git fetch upstream && git merge upstream/main && python3 studio/rebrand.py
```

## Licence

Code: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`), like upstream. The Creative Studio
name and logos in `docs/studio/` belong to the Creative Studio project.
''', encoding='utf-8')

n = (repo / 'NOTICE').read_text(encoding='utf-8')
para = f'''{app} is a fork of {up} by the ArtCraft Team and the {up} contributors
(https://github.com/storytold/{slug}), made by the Creative Studio project (https://studio.creative.ps).
Our changes are listed in studio/CHANGES.md. The Creative Studio name and logos (docs/studio/) belong
to the Creative Studio project. The ArtCraft name and logos are trademarks of the ArtCraft Team; this fork
does not use them, as their brand terms require (upstream's docs/brand/ was removed).'''
if para not in n:
    n2 = re.sub(r'The ArtCraft name, wordmark and logos in docs/brand/.*?LICENSE-brand\.txt\.', lambda m: para, n, count=1, flags=re.S)
    if n2 == n:  # no brand paragraph upstream: after the licence lines
        parts = n.split('\n\n', 1)
        n2 = parts[0] + '\n\n' + para + ('\n\n' + parts[1] if len(parts) > 1 else '\n')
    (repo / 'NOTICE').write_text(n2, encoding='utf-8')

ch = repo / 'studio/CHANGES.md'
if not ch.exists():
    ch.write_text(f'''# {app}: changes from upstream {up}

Required by the Apache License 2.0 (section 4b): files we changed carry this notice through
this list.

- Branding (user-visible only, applied by `studio/rebrand.py`): the app name {app}, the bundle id
  `ps.creative.studio.{app}`, links to {app}.studio.creative.ps and this repository. Crate names,
  settings folders and the file format keep their upstream names.
- No Discord buttons; community links point to Creative Studio.
- App icon: the {app} star on navy (`assets/app-icon/`).
- Removed upstream's ArtCraft brand assets (`docs/brand/`), as their brand terms require of forks.
- README (upstream's is `docs/UPSTREAM-README.md`) and NOTICE: the fork, its credits and the suite.
''', encoding='utf-8')
print(app, 'brand files ok')
