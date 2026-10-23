#!/bin/bash
set -eu
export CARGO_BUILD_JOBS=6
export CARGO_TARGET_DIR=/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target
proof=tools/protocol-bridge/testdata/client-chain-770-773/completion771-verification
while pgrep -x cargo >/dev/null; do sleep 30; done
cargo test --release --workspace -- --test-threads=1 > "$proof/rust-final.log" 2>&1
while pgrep -x cargo >/dev/null; do sleep 30; done
cargo clippy --release --workspace --all-targets -- -D warnings > "$proof/clippy-workspace-final.log" 2>&1
while pgrep -x cargo >/dev/null; do sleep 30; done
cargo clippy --release --all-targets -- -D warnings > "$proof/clippy-root-final.log" 2>&1
while pgrep -x cargo >/dev/null; do sleep 30; done
cargo build --release > "$proof/build-final.log" 2>&1
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/protocol-bridge -p 'test_*.py' > "$proof/python-final.log" 2>&1
