#!/usr/bin/env bash
# Наш сервер: Java 25964, Bedrock UDP 19165. Папка tmp/v-ours внутри копии.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
O="$ROOT/tmp/v-ours"
mkdir -p "$O/config"
sed -e 's/^server-port=.*/server-port=25964/' -e 's/^level-type=.*/level-type=minecraft\\:flat/' -e 's/^online-mode=.*/online-mode=false/' -e 's/^view-distance=.*/view-distance=4/' -e 's/^simulation-distance=.*/simulation-distance=4/' "$ROOT/config/server.properties" > "$O/config/server.properties"
cp "$ROOT"/config/*.toml "$O/config/" 2>/dev/null || true
echo '[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4},{"uuid":"8db7aee5-ef46-3281-b3c4-f00d7c34c93c","name":"blackbox2","level":4}]' > "$O/ops.json"
sed -i '/^bedrock-port=/d' "$O/config/mcsa.properties" 2>/dev/null || true
echo "bedrock-port=19165" >> "$O/config/mcsa.properties"
cd "$O"
nohup "$ROOT/target/release/mcsheriffanya" > "$O/out.log" 2>&1 &
echo $! > "$ROOT/tmp/v-ours.pid"
