#!/usr/bin/env bash
# Поднимает нашу сборку в отдельной папке на порту 25567 (живой сервер на 25565 не трогаем).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$HERE/../.."
O="$HERE/ours"
mkdir -p "$O/config"
sed -e 's/^server-port=.*/server-port=25567/' -e 's/^online-mode=.*/online-mode=false/' -e 's/^view-distance=.*/view-distance=4/' -e 's/^simulation-distance=.*/simulation-distance=4/' "$ROOT/config/server.properties" > "$O/config/server.properties"
cp "$ROOT"/config/*.toml "$O/config/" 2>/dev/null || true
echo '[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4}]' > "$O/ops.json"
cd "$O" && exec "$ROOT/target/debug/rustcraft"
