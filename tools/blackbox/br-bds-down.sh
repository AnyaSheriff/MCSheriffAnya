#!/usr/bin/env bash
# Останавливает BDS, запущенный br-bds-up.sh, по PID (после проверки каталога процесса).
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PID="$(cat "$ROOT/tmp/bds.pid" 2>/dev/null)"
[ -n "$PID" ] || { echo "нет tmp/bds.pid"; exit 0; }
if [ "$(readlink /proc/$PID/cwd)" = "/home/lenox/.cache/mcsheriffanya/bds" ]; then
  kill "$PID"
  sleep 3
  echo "BDS $PID остановлен"
else
  echo "PID $PID не наш BDS — не трогаю"
fi
rm -f "$ROOT/tmp/bds.pid" "$ROOT/tmp/bds.stdin"
