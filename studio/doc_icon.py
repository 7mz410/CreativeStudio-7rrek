#!/usr/bin/env python3
"""Draw the Finder document icon: a white page with a folded corner, the app's star from
assets/app-icon/vectorcraft.svg, and the file extension under it.

Writes assets/app-icon/<name>-document.svg and .icns (name defaults to the extension; macOS only:
needs resvg and iconutil).
The outputs are committed, so building never needs these tools.

    python3 studio/doc_icon.py rsm
    python3 studio/doc_icon.py 7rrek rsm   # 7rrek's art under rsm's file name
"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIR = ROOT / 'assets/app-icon'
ACCENT = '#eb1b3c'


def glyph():
    # The app icon is a white tile holding the star as a nested <svg>; reuse that element as is.
    src = (DIR / 'vectorcraft.svg').read_text()
    m = re.search(r'<svg x="[^"]*" y="[^"]*" width="[^"]*" height="[^"]*" (viewBox=.*</svg>)\s*</g>\s*</svg>\s*$', src, re.S)
    if not m:
        sys.exit('error: vectorcraft.svg has no nested star <svg>')
    return m.group(1)


def page(ext):
    # Apple's document grid: the page is 3/4 as wide as the canvas, with a soft shadow.
    size = 34 if len(ext) <= 3 else 30 if len(ext) <= 5 else 24
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <defs><filter id="s" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="6" stdDeviation="8" flood-opacity=".22"/></filter></defs>
  <path d="M104 24h214l90 90v354a20 20 0 0 1-20 20H104a20 20 0 0 1-20-20V44a20 20 0 0 1 20-20z" fill="#ffffff" stroke="#e4e0d8" stroke-width="6" filter="url(#s)"/>
  <path d="M318 24v70a20 20 0 0 0 20 20h70" fill="#f3f0ea" stroke="#e4e0d8" stroke-width="6" stroke-linejoin="round"/>
  <svg x="146" y="138" width="220" height="220" {glyph()}
  <text x="256" y="432" text-anchor="middle" font-family="Helvetica Neue, Helvetica, Arial, sans-serif" font-weight="700" font-size="{size * 2}" fill="{ACCENT}">{ext.upper()}</text>
</svg>
'''


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    ext = sys.argv[1]
    name = sys.argv[2] if len(sys.argv) == 3 else ext
    svg = DIR / f'{name}-document.svg'
    svg.write_text(page(ext))
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / 'doc.iconset'
        iconset.mkdir()
        for s in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                png = f'icon_{s}x{s}{"@2x" if scale == 2 else ""}.png'
                px = s * scale
                subprocess.run(['resvg', '-w', str(px), '-h', str(px), str(svg), str(iconset / png)], check=True)
        subprocess.run(['iconutil', '-c', 'icns', str(iconset), '-o', str(DIR / f'{name}-document.icns')], check=True)
    print(f'wrote {svg.name} and {name}-document.icns')


if __name__ == '__main__':
    main()
