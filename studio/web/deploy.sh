#!/bin/bash
# Build each app's web version and deploy it to its gh-pages branch (normal push).
# Usage: rsm/studio/web/deploy.sh rsm ttshat ...
SP=$(mktemp -d)
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
CS=$HOME/Documents/CreativeStudio
for a in "$@"; do
  echo "===== $a"
  cd $CS/$a || continue
  case $a in
    rsm) (cd apps/vectorcraft-web && trunk build --release) >/dev/null 2>$SP/$a.err; out=dist/web ;;
    tlween) (cd apps/photocraft-web && trunk build --release) >/dev/null 2>$SP/$a.err; out=dist/web ;;
    klmat) (cd apps/wordcraft-web && trunk build --release) >/dev/null 2>$SP/$a.err; out=dist/web ;;
    trteeb) (cd apps/designcraft-web && trunk build --release) >/dev/null 2>$SP/$a.err; out=dist/web ;;
    sf7aat) (cd apps/pdfcraft-web && trunk build --release) >/dev/null 2>$SP/$a.err; out=dist/web ;;
    ttshat) cargo xtask web >/dev/null 2>$SP/$a.err; out=target/web ;;
    *) cargo xtask web >/dev/null 2>$SP/$a.err; out=target/web/dist ;;
  esac
  [ -f $out/index.html ] || { echo "$a: build failed"; tail -5 $SP/$a.err; continue; }
  grep -o "<title>[^<]*" $out/index.html | head -1
  d=$SP/pages-$a; rm -rf $d
  git clone -q --depth 1 -b gh-pages https://github.com/7mz410/CreativeStudio-$a.git $d 2>/dev/null \
    || (git init -q -b gh-pages $d && git -C $d remote add origin https://github.com/7mz410/CreativeStudio-$a.git)  # first deploy
  (cd $d && { git rm -rq . 2>/dev/null; true; } && cp -R $CS/$a/$out/. . && touch .nojekyll && echo $a.studio.creative.ps > CNAME && python3 "$CS/rsm/studio/web/fixpages.py" . $a && git add -A \
    && git -c user.name="Hamza Abu Ayyash" -c user.email=hamza.abu3ayash@gmail.com commit -qm "Deploy $a web ($(git -C $CS/$a rev-parse --short HEAD))" \
    && git push -q origin gh-pages && echo "$a deployed")
  rm -rf $d target
done
