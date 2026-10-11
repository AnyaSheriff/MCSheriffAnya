#!/usr/bin/env bash
# Запись BDS: bash tools/blackbox/br-bds.sh <cases.js> <out.jsonl>
# BDS (udp 19150) уже запущен пользователем; наш только клиент через прослойку на 19160.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$HERE/../../tmp"
node "$HERE/br-shim.js" 19160 19150 &
SHIM=$!
trap 'kill $SHIM 2>/dev/null' EXIT
sleep 1
[ -n "${3:-}" ] && export ONLY="$3"
mkdir -p "$(dirname "$(realpath -m "$2")")"
node "$HERE/br-capture.js" bds 19160 "$(realpath -m "$2")" "$(realpath "$1")"
