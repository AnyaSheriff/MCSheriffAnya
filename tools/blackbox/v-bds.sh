#!/usr/bin/env bash
# Официальный BDS (UDP 19150) + RakNet-прослойка 19160 (клиент RakNet 10 -> 11).
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd /home/lenox/.cache/mcsheriffanya/bds
LD_LIBRARY_PATH=. nohup ./bedrock_server > "$ROOT/tmp/v-bds.log" 2>&1 < /dev/null &
echo $! > "$ROOT/tmp/v-bds.pid"
nohup python3 "$ROOT/tools/blackbox/v-shim.py" > "$ROOT/tmp/v-shim.log" 2>&1 &
echo $! > "$ROOT/tmp/v-shim.pid"
