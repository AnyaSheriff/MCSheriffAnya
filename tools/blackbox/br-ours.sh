#!/usr/bin/env bash
# Запись нашего сервера: bash tools/blackbox/br-ours.sh <cases.js> <out.jsonl>
# Java 25960, Bedrock 19161, плоский мир, офлайн; Java-бот blackbox — оператор.
# Сервер останавливается по своему PID, папка удаляется.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
CASES="$(realpath "$1")"; OUT="$(realpath -m "$2")"
O="$HERE/ours-br"
rm -rf "$O"
mkdir -p "$O/config" "$ROOT/tmp"
sed -e 's/^server-port=.*/server-port=25960/' -e 's/^level-type=.*/level-type=minecraft\\:flat/' -e 's/^online-mode=.*/online-mode=false/' -e 's/^view-distance=.*/view-distance=6/' -e 's/^simulation-distance=.*/simulation-distance=6/' "$ROOT/config/server.properties" > "$O/config/server.properties"
cp "$ROOT"/config/*.toml "$O/config/" 2>/dev/null || true
echo '[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4}]' > "$O/ops.json"
printf "bedrock-port=19161\n" > "$O/config/mcsa.properties"
cd "$O" || exit 1
"$ROOT/target/release/mcsheriffanya" > "$O/server.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null; sleep 1; rm -rf "$O"' EXIT
for _ in $(seq 1 60); do
  ss -ltn | grep -q ':25960 ' && ss -lun | grep -q ':19161 ' && break
  sleep 1
done
ss -ltn | grep -q ':25960 ' || { echo "сервер не поднялся"; tail -20 "$O/server.log"; exit 1; }
[ -n "${3:-}" ] && export ONLY="$3"
mkdir -p "$(dirname "$OUT")"
cd "$HERE" && node br-capture.js ours 19161 "$OUT" "$CASES" 25960
cp "$O/server.log" "$ROOT/tmp/ours-br-server.log" 2>/dev/null
