#!/usr/bin/env bash
# Регрессия Java: свой официальный 26.1.2 (порт 25963, tmp/v-vanilla) и наша сборка без Bedrock/плагинов
# (порт 25962, tmp/v-reg). bash tools/blackbox/v-javareg.sh start | run <сценарий>… | stop
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
V="$ROOT/tmp/v-vanilla"
R="$ROOT/tmp/v-reg"
OPS='[{"uuid":"26d479c9-4844-3b0a-ba59-8cc656622a78","name":"blackbox","level":4,"bypassesPlayerLimit":true},{"uuid":"8db7aee5-ef46-3281-b3c4-f00d7c34c93c","name":"blackbox2","level":4,"bypassesPlayerLimit":true},{"uuid":"ea6bdeb9-3459-3edf-98d1-a4a1901bc58d","name":"blackbox3","level":4,"bypassesPlayerLimit":true},{"uuid":"3e6020d9-2933-38a5-962a-14434df3f116","name":"blackbox4","level":4,"bypassesPlayerLimit":true},{"uuid":"edb5454a-e6e4-35a6-8995-cf68095c583c","name":"blackbox5","level":4,"bypassesPlayerLimit":true}]'
case "${1:-}" in
start)
  rm -rf "$V" "$R"
  mkdir -p "$V" "$R/config"
  cp "$ROOT/tools/blackbox/vanilla-jar/server.jar" "$V/server.jar"
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
motd=v-vanilla
P
  echo "$OPS" > "$V/ops.json"
  (cd "$V" && nohup /usr/lib/jvm/java-25-openjdk/bin/java -Xmx1G -jar server.jar nogui > out.log 2>&1 < /dev/null & echo $! > "$ROOT/tmp/v-vanilla.pid")
  sed -e 's/^server-port=.*/server-port=25962/' -e 's/^level-type=.*/level-type=minecraft\\:flat/' -e 's/^online-mode=.*/online-mode=false/' -e 's/^view-distance=.*/view-distance=4/' -e 's/^simulation-distance=.*/simulation-distance=4/' "$ROOT/config/server.properties" > "$R/config/server.properties"
  cp "$ROOT"/config/*.toml "$R/config/" 2>/dev/null || true
  echo "$OPS" > "$R/ops.json"
  printf "bedrock-port=0\n" >> "$R/config/mcsa.properties"
  (cd "$R" && nohup "$ROOT/target/release/mcsheriffanya" > out.log 2>&1 < /dev/null & echo $! > "$ROOT/tmp/v-reg.pid")
  ;;
run)
  shift
  cd "$ROOT/tools/blackbox"
  for s in "$@"; do
    echo "=== $s"
    node compare.js "$s" --record --ours-port 25962 --vanilla-port 25963 2>&1 | grep -E "^Совпало"
  done
  ;;
stop)
  for f in v-vanilla v-reg; do
    pid="$(cat "$ROOT/tmp/$f.pid" 2>/dev/null)"
    if [ -n "$pid" ]; then
      # в файле — PID подоболочки, сам сервер — следующий; убиваем только процессы с cwd внутри tmp/v-*
      for p in "$pid" "$((pid + 1))"; do
        case "$(readlink "/proc/$p/cwd")" in "$ROOT/tmp/v-vanilla"|"$ROOT/tmp/v-reg") kill "$p" ;; esac
      done
    fi
  done
  ;;
esac
