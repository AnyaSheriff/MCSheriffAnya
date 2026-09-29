#!/usr/bin/env bash
# Готовит официальный сервер 26.1.2 для сверки. Jar только запускается — не распаковывается и не изучается.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
V="$HERE/vanilla"
mkdir -p "$V"
if [ ! -f "$V/server.jar" ]; then
  URL=$(curl -s https://piston-meta.mojang.com/mc/game/version_manifest_v2.json \
    | python3 -c "import json,sys,urllib.request; m=json.load(sys.stdin); v=[x for x in m['versions'] if x['id']=='26.1.2'][0]; print(json.load(urllib.request.urlopen(v['url']))['downloads']['server']['url'])")
  echo "Скачиваю официальный сервер 26.1.2…"
  curl -L --progress-bar -o "$V/server.jar" "$URL"
fi
echo "eula=true" > "$V/eula.txt"
cat > "$V/server.properties" <<'P'
online-mode=false
server-port=25566
level-type=minecraft\:flat
generator-settings={"layers"\:[{"block"\:"minecraft\:bedrock","height"\:1},{"block"\:"minecraft\:stone","height"\:3}],"biome"\:"minecraft\:plains"}
level-name=world
spawn-protection=0
max-tick-time=-1
gamemode=creative
difficulty=peaceful
spawn-monsters=false
generate-structures=false
view-distance=4
simulation-distance=4
motd=blackbox vanilla
P
# Права роботам всех полос (blackbox, blackbox2…5 — см. bot.js): офлайн-UUID из ника.
cat > "$V/ops.json" <<'P'
[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4,"bypassesPlayerLimit":true},
 {"uuid":"8db7aee5-ef46-3281-b3c4-f00d7c34c93c","name":"blackbox2","level":4,"bypassesPlayerLimit":true},
 {"uuid":"ea6bdeb9-3459-3edf-98d1-a4a1901bc58d","name":"blackbox3","level":4,"bypassesPlayerLimit":true},
 {"uuid":"3e6020d9-2933-38a5-962a-14434df3f116","name":"blackbox4","level":4,"bypassesPlayerLimit":true},
 {"uuid":"edb5454a-e6e4-35a6-8995-cf68095c583c","name":"blackbox5","level":4,"bypassesPlayerLimit":true}]
P
echo "Готово: $V"
