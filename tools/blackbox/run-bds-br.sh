#!/usr/bin/env bash
# Официальный BDS (UDP 19150) + RakNet-прослойка 19160 (10 → 11). PID-ы в tmp/run/*.pid.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$HERE/../.."; mkdir -p "$ROOT/tmp/run"
( cd /home/lenox/.cache/mcsheriffanya/bds && LD_LIBRARY_PATH=. exec ./bedrock_server >"$ROOT/tmp/run/bds.out" 2>&1 </dev/null ) &
echo $! > "$ROOT/tmp/run/bds.pid"
python3 -I "$ROOT/tmp/raknet-shim.py" >"$ROOT/tmp/run/shim.out" 2>&1 &
echo $! > "$ROOT/tmp/run/shim.pid"
sleep 8; ss -lunp | grep -e 19150 -e 19160 || true
