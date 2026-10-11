#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
export CARGO_TARGET_DIR="$ROOT/target" CARGO_NET_OFFLINE=false TMPDIR="$ROOT/tmp"
mkdir -p "$TMPDIR" tools/seedmap/dist
VERSION=0.2.104
BINDGEN="$ROOT/target/seedmap-tools/bin/wasm-bindgen"
if [[ ! -x "$BINDGEN" ]] || [[ "$("$BINDGEN" --version)" != "wasm-bindgen $VERSION" ]]; then
    nice -n 19 cargo install wasm-bindgen-cli --version "$VERSION" --locked --root "$ROOT/target/seedmap-tools" -j 2
fi
nice -n 19 cargo build --release -j 2 -p seedmap-core --target wasm32-unknown-unknown
"$BINDGEN" --target no-modules --no-typescript --out-dir tools/seedmap/dist --out-name seedmap target/wasm32-unknown-unknown/release/seedmap_core.wasm
python3 tools/seedmap/package.py
printf '%s\n' 'Ready: tools/seedmap/dist/index.html (also works directly via file://)'
