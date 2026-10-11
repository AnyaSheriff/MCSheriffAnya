#!/usr/bin/env bash
# Перезапуск нашего сервера (только своего процесса — по PID из tmp/v-ours.pid, после проверки cwd).
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PID="$(cat "$ROOT/tmp/v-ours.pid" 2>/dev/null)"
if [ -n "$PID" ] && [ "$(readlink "/proc/$PID/cwd")" = "$ROOT/tmp/v-ours" ]; then
  kill "$PID"
  for _ in 1 2 3 4 5 6 7 8 9 10; do kill -0 "$PID" 2>/dev/null || break; sleep 1; done
fi
bash "$ROOT/tools/blackbox/v-ours.sh"
for _ in 1 2 3 4 5 6 7 8 9 10; do ss -lunp | grep -q ":19165 " && break; sleep 1; done
ss -lunp | grep ":19165 "
