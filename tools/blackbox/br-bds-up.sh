#!/usr/bin/env bash
# Запускает официальный BDS пользователя (udp 19150) и пишет PID в tmp/bds.pid.
# Его двоичный файл не читается и не копируется — только сеть. Остановка: br-bds-down.sh.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
B=/home/lenox/.cache/mcsheriffanya/bds
mkdir -p "$ROOT/tmp"
if ss -lun | grep -q ':19150 '; then echo "BDS уже слушает 19150"; exit 0; fi
FIFO="$ROOT/tmp/bds.stdin"
rm -f "$FIFO"
mkfifo "$FIFO"
cd "$B" || exit 1
# stdin держим открытым, иначе BDS завершится по концу ввода
exec 3<>"$FIFO"
LD_LIBRARY_PATH=. nohup ./bedrock_server <&3 > "$ROOT/tmp/bds.out" 2>&1 &
echo $! > "$ROOT/tmp/bds.pid"
for _ in $(seq 1 60); do
  ss -lun | grep -q ':19150 ' && { echo "BDS поднят, PID $(cat "$ROOT/tmp/bds.pid")"; exit 0; }
  sleep 1
done
echo "BDS не поднялся"; tail -20 "$ROOT/tmp/bds.out"; exit 1
