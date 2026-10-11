#!/usr/bin/env python3
"""Tidy a gh-pages checkout: app favicon, no build junk, no upstream names. Usage: fixpages.py <dir> <app>"""
import re, shutil, subprocess, sys
from pathlib import Path

site, app = Path(sys.argv[1]), sys.argv[2]
CS = Path.home() / 'Documents/CreativeStudio'
star = CS / 'logos/Creative_Studio' / ('saf7at.svg' if app == 'sf7aat' else f'{app}.svg')
icon = next(p for p in (CS / app / 'assets/app-icon').glob('*.svg') if 'small' not in p.name and 'document' not in p.name)

# Cargo build leftovers that slipped into ttshat's site.
for junk in ['.cargo-artifact-lock', '.cargo-build-lock', '.cargo-lock', '.fingerprint', 'build', 'deps']:
    p = site / junk
    if p.is_dir():
        shutil.rmtree(p)
    elif p.exists():
        p.unlink()

(site / 'favicon.svg').write_text(re.sub(r'<title>.*?</title>', '', star.read_text(encoding='utf-8'), flags=re.S), encoding='utf-8')
# Upstream raster icons (effectat's PWA icons, tsweer's favicon.png) become the app icon.
for name, px in [('icon-256.png', 256), ('icon-512.png', 512), ('favicon.png', 64)]:
    if (site / name).exists():
        subprocess.run(['resvg', '-w', str(px), '-h', str(px), str(icon), str(site / name)], check=True)

idx = site / 'index.html'
s = idx.read_text(encoding='utf-8')
s = re.sub(r'\s*<link rel="icon"[^>]*>', '', s)
s = s.replace('</head>', f'  <link rel="icon" type="image/svg+xml" href="favicon.svg">\n</head>', 1)
s = re.sub(r'<title>Creative nu\.ai</title>', '', s)  # Illustrator export title inside the splash star
idx.write_text(s, encoding='utf-8')
for gz in ('index.html.br', 'index.html.gz'):  # precompressed copies would serve the old page
    (site / gz).unlink(missing_ok=True)

m = site / 'manifest.webmanifest'
if m.exists():
    t = re.sub(r'"(short_)?name": "[^"]*"', lambda x: f'"{x.group(1) or ""}name": "{app}"', m.read_text(encoding='utf-8'))
    m.write_text(t, encoding='utf-8')
print(app, 'ok')
