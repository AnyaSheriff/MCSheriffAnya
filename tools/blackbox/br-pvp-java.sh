#!/usr/bin/env bash
# Прогон всех pvp-*.json (оригинал на 25963, наш на 25962): итоговая строка каждого сценария → tmp/run/pvp-java.txt.
# Уже записанные в файле сценарии пропускаются (можно запускать повторно после обрыва).
cd "$(dirname "$0")"
out=../../tmp/run/pvp-java.txt; touch "$out"
for f in scenarios/pvp-*.json scenarios/itemaudit2-pvp-*.json; do
  n=$(basename "$f" .json)
  grep -q "^$n: " "$out" && continue
  r=$(node compare.js "$n" --record --ours-port 25962 --vanilla-port 25963 2>&1 | grep "^Совпало" | tail -1)
  echo "$n: $r" >> "$out"
done
