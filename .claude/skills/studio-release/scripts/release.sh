#!/usr/bin/env bash
# Build rsm for macOS and the web, and publish the web build to GitHub Pages.
#
#   release.sh [--mac] [--web] [--deploy]     (no flags: all three)
#
# Run from anywhere; the repo is ~/Documents/CreativeStudio/rsm unless STUDIO_ROOT is set.
set -euo pipefail
ROOT="${STUDIO_ROOT:-$HOME/Documents/CreativeStudio/rsm}"
REPO_URL="https://github.com/7mz410/CreativeStudio-7rrek.git"
DOMAIN="7rrek.studio.creative.ps"
# rustup comes from Homebrew and is keg-only; it provides the wasm32 target trunk needs.
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

mac=0 web=0 deploy=0
[ $# -eq 0 ] && mac=1 web=1 deploy=1
for a in "$@"; do
  case "$a" in
    --mac) mac=1 ;;
    --web) web=1 ;;
    --deploy) deploy=1 ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done

cd "$ROOT"

if [ $mac = 1 ]; then
  cargo xtask bundle
  rm -rf "dist/rsm.app"
  mv dist/VectorCraft.app "dist/rsm.app"
  echo "mac: $ROOT/dist/rsm.app"
fi

if [ $web = 1 ]; then
  (cd apps/vectorcraft-web && trunk build --release)
  grep -q "<title>rsm" dist/web/index.html || { echo "web: title is not branded; run studio/rebrand.py" >&2; exit 1; }
  echo "web: $ROOT/dist/web"
fi

if [ $deploy = 1 ]; then
  [ -f dist/web/index.html ] || { echo "deploy: build the web first (--web)" >&2; exit 1; }
  site="$(mktemp -d)"
  trap 'rm -rf "$site"' EXIT
  cp -R dist/web/. "$site/"
  touch "$site/.nojekyll"
  # GitHub Pages reads the custom domain from this file on every deploy.
  echo "$DOMAIN" > "$site/CNAME"
  (
    cd "$site"
    git init -q -b gh-pages
    git add -A
    git -c user.name="Hamza Abu Ayyash" -c user.email=hamza.abu3ayash@gmail.com commit -q -m "Deploy rsm web ($(git -C "$ROOT" rev-parse --short HEAD))"
    git push -q -f "$REPO_URL" gh-pages
  )
  echo "deployed: https://$DOMAIN/"
fi
