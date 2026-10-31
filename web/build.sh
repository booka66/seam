#!/bin/sh
# web/build.sh: the browser's seam in web/dist, a folder any static host can serve:
# the page, seam.wasm and the little that fetches a change for them. Wants zig for
# the grammars' C, and the wasm32-unknown-unknown target.
set -eu
cd "$(dirname "$0")/.."
CC_wasm32_unknown_unknown=$PWD/web/cc AR_wasm32_unknown_unknown=$PWD/web/ar CARGO_PROFILE_RELEASE_OPT_LEVEL=s \
  cargo build --release --lib --target wasm32-unknown-unknown
mkdir -p web/dist
cp target/wasm32-unknown-unknown/release/seam.wasm web/seam-web.js web/dist/
# The page as seam ships it, with nothing in it to draw and the fetcher after it.
sed 's|</body>|<script src="seam-web.js"></script></body>|' share/view.html | grep -v '<!--SEAM DATA-->' > web/dist/index.html
