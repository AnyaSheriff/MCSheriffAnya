#!/usr/bin/env bash
# Поднимает нашу сборку в отдельной папке на порту 25567 (живой сервер на 25565 не трогаем).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$HERE/../.."
O="$HERE/ours"
mkdir -p "$O/config"
sed -e 's/^server-port=.*/server-port=25567/' -e 's/^level-type=.*/level-type=minecraft\\:flat/' -e 's/^online-mode=.*/online-mode=false/' -e 's/^view-distance=.*/view-distance=4/' -e 's/^simulation-distance=.*/simulation-distance=4/' "$ROOT/config/server.properties" > "$O/config/server.properties"
cp "$ROOT"/config/*.toml "$O/config/" 2>/dev/null || true
# Права роботам всех полос (blackbox, blackbox2…5 — см. bot.js): офлайн-UUID из ника.
echo '[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4},{"uuid":"8db7aee5-ef46-3281-b3c4-f00d7c34c93c","name":"blackbox2","level":4},{"uuid":"ea6bdeb9-3459-3edf-98d1-a4a1901bc58d","name":"blackbox3","level":4},{"uuid":"3e6020d9-2933-38a5-962a-14434df3f116","name":"blackbox4","level":4},{"uuid":"edb5454a-e6e4-35a6-8995-cf68095c583c","name":"blackbox5","level":4}]' > "$O/ops.json"
cd "$O" && exec "$ROOT/target/release/mcsheriffanya"
