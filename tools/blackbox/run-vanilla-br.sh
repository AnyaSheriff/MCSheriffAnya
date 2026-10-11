#!/usr/bin/env bash
# Свой официальный 26.1.2 (Java, порт 25963, плоский мир по setup.sh) для прогона pvp-*.json. Jar только запускается.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$HERE/../.."
V="$ROOT/tmp/vanilla-br"; mkdir -p "$V" "$ROOT/tmp/run"
[ -f "$V/server.jar" ] || cp "$HERE/vanilla-jar/server.jar" "$V/server.jar"
echo "eula=true" > "$V/eula.txt"
cat > "$V/server.properties" <<'P'
online-mode=false
server-port=25963
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
motd=blackbox vanilla br
P
cp "$HERE/van-passives/../../../tools/blackbox/van-passives/ops.json" "$V/ops.json" 2>/dev/null || cat > "$V/ops.json" <<'P'
[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4,"bypassesPlayerLimit":true},
 {"uuid":"8db7aee5-ef46-3281-b3c4-f00d7c34c93c","name":"blackbox2","level":4,"bypassesPlayerLimit":true}]
P
cd "$V"
/usr/lib/jvm/java-25-openjdk/bin/java -Xmx1G -jar server.jar nogui >"$ROOT/tmp/run/vanilla.out" 2>&1 </dev/null &
echo $! > "$ROOT/tmp/run/vanilla.pid"
for i in $(seq 1 60); do ss -ltn | grep -q ':25963 ' && break; sleep 2; done
ss -ltnp | grep 25963
