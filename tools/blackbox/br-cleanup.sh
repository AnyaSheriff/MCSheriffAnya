#!/usr/bin/env bash
# Уборка после проверки: отчёты pvp, временные папки и записи; проверка, что серверы остановлены.
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# отчёты текущих прогонов compare.js (сегодняшняя дата в имени); прежние отчёты не трогаем
rm -f "$HERE"/reports/*-2026-10-11-*.md
rm -rf "$HERE/vanilla-br" "$HERE/ours-pvp" "$HERE/ours-br"
rm -rf "$ROOT/tmp/cap" "$ROOT/tmp/labels.js" "$ROOT/tmp/labels.py" "$ROOT/tmp/prof1.json" "$ROOT/tmp/rep-list.txt" "$ROOT/tmp/ours-br-server.log"
rm -f "$ROOT"/tmp/pvp-*.out "$ROOT/tmp/pvp-summary.txt" "$ROOT"/tmp/bds.* "$ROOT"/tmp/cargo-*.log
echo "порты (должно быть пусто):"
ss -lntu | grep -E ':(25960|25961|25962|19150|19160|19161) ' || echo "  пусто"
