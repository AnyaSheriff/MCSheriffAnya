#!/usr/bin/env bash
# Регрессия Java: все pvp-*.json против своего официального сервера (java 25961) и нашей сборки (java 25962).
#   bash tools/blackbox/br-pvp.sh [сценарий ...]   (без аргументов — все pvp-*)
# Итоги — в tmp/pvp-summary.txt; серверы останавливаются по PID, папки удаляются.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
V="$HERE/vanilla-br"
O="$HERE/ours-pvp"
JAR=/home/lenox/Загрузки/minecraft_rust_server/tools/blackbox/vanilla/server.jar
JAVA=/usr/lib/jvm/java-25-openjdk/bin/java
mkdir -p "$ROOT/tmp"
rm -rf "$V" "$O"
mkdir -p "$V" "$O/config"
cp "$JAR" "$V/server.jar"
echo "eula=true" > "$V/eula.txt"
cat > "$V/server.properties" <<'P'
online-mode=false
server-port=25961
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
OPS='[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4,"bypassesPlayerLimit":true},{"uuid":"8db7aee5-ef46-3281-b3c4-f00d7c34c93c","name":"blackbox2","level":4,"bypassesPlayerLimit":true},{"uuid":"ea6bdeb9-3459-3edf-98d1-a4a1901bc58d","name":"blackbox3","level":4,"bypassesPlayerLimit":true},{"uuid":"3e6020d9-2933-38a5-962a-14434df3f116","name":"blackbox4","level":4,"bypassesPlayerLimit":true},{"uuid":"edb5454a-e6e4-35a6-8995-cf68095c583c","name":"blackbox5","level":4,"bypassesPlayerLimit":true}]'
echo "$OPS" > "$V/ops.json"
sed -e 's/^server-port=.*/server-port=25962/' -e 's/^level-type=.*/level-type=minecraft\\:flat/' -e 's/^online-mode=.*/online-mode=false/' -e 's/^view-distance=.*/view-distance=4/' -e 's/^simulation-distance=.*/simulation-distance=4/' "$ROOT/config/server.properties" > "$O/config/server.properties"
cp "$ROOT"/config/*.toml "$O/config/" 2>/dev/null || true
echo "$OPS" > "$O/ops.json"
printf "bedrock-port=0\n" > "$O/config/mcsa.properties"

cd "$V" || exit 1
"$JAVA" -Xmx1G -jar server.jar nogui > "$V/out.log" 2>&1 &
VPID=$!
cd "$O" || exit 1
"$ROOT/target/release/mcsheriffanya" > "$O/server.log" 2>&1 &
OPID=$!
cleanup() {
  for p in "$VPID" "$OPID"; do
    kill "$p" 2>/dev/null
  done
  sleep 3
  rm -rf "$V" "$O"
}
trap cleanup EXIT

for _ in $(seq 1 120); do
  ss -ltn | grep -q ':25961 ' && ss -ltn | grep -q ':25962 ' && break
  sleep 1
done
ss -ltn | grep -q ':25961 ' || { echo "официальный сервер не поднялся"; tail -5 "$V/out.log"; exit 1; }
ss -ltn | grep -q ':25962 ' || { echo "наш сервер не поднялся"; tail -5 "$O/server.log"; exit 1; }

cd "$HERE" || exit 1
if [ "$#" -gt 0 ]; then LIST="$*"; else LIST="$(cd scenarios && ls pvp-*.json | sed 's/\.json$//')"; fi
: > "$ROOT/tmp/pvp-summary.txt"
for s in $LIST; do
  echo "=== $s" >> "$ROOT/tmp/pvp-summary.txt"
  node compare.js "$s" --record --ours-port 25962 --vanilla-port 25961 > "$ROOT/tmp/pvp-$s.out" 2>&1
  tail -12 "$ROOT/tmp/pvp-$s.out" >> "$ROOT/tmp/pvp-summary.txt"
done
echo "готово" >> "$ROOT/tmp/pvp-summary.txt"
