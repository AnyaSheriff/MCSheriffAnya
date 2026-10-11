#!/usr/bin/env bash
# Последняя запись нашего сервера, итоговое сравнение с BDS (tmp/cap/bds-all.jsonl) и уборка временного.
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
bash "$HERE/br-ours.sh" "$HERE/br-cases.js" "$ROOT/tmp/cap/ours-7.jsonl"
mkdir -p "$HERE/reports-br"
node "$HERE/br-diff.js" "$ROOT/tmp/cap/bds-all.jsonl" "$ROOT/tmp/cap/ours-7.jsonl" - "$ROOT/tmp/cap/bds-d.jsonl" > "$ROOT/tmp/cap/diff-7.txt"
{
  echo "# Итог сравнения с BDS (br-diff.js, затем br-triage.js отсекает случайное у BDS)"
  node "$HERE/br-triage.js" "$ROOT/tmp/cap/diff-7.txt"
  echo
  echo "# Полный вывод br-diff.js"
  cat "$ROOT/tmp/cap/diff-7.txt"
} > "$HERE/reports-br/diff-final.txt"
tail -3 "$HERE/reports-br/diff-final.txt" | head -1
grep -c "совпало" "$ROOT/tmp/cap/diff-7.txt"
