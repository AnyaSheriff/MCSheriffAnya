#!/usr/bin/env bash
# Только прослойка (BDS уже запущен): bash tools/blackbox/v-shim.sh
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
nohup python3 "$ROOT/tools/blackbox/v-shim.py" > "$ROOT/tmp/v-shim.log" 2>&1 &
echo $! > "$ROOT/tmp/v-shim.pid"
