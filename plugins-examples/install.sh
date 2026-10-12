#!/usr/bin/env bash
# Кладёт собранные примеры в папки plugins/ и modules/ сервера.
#   ./install.sh [папка-сервера] [--with-misbehave]    (по умолчанию — текущая)
# misbehave — испытательный плагин (виснет, паникует), по умолчанию не ставится.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SERVER="."
EXTRA=0
for arg in "$@"; do
  case "$arg" in
    --with-misbehave) EXTRA=1 ;;
    *) SERVER="$arg" ;;
  esac
done
OUT="$HERE/target/wasm32-wasip2/release"

put() { # папка(plugins|modules) имя описание
  local file="$OUT/${2//-/_}.wasm" dir="$SERVER/$1/$2"
  mkdir -p "$dir"
  cp "$file" "$dir/$2.wasm"
  cat > "$dir/plugin.toml" <<TOML
name = "$2"
version = "0.1.0"
api = "0.1"
author = "MCSheriffAnya"
description = "$3"
TOML
  echo "установлен: $dir"
}

put plugins custom-tablist "Настраиваемый TAB, имена, HUD и чат"
put plugins hello "Приветствие при входе и команда /hello"
put plugins chat-filter "Фильтр чата: запрещённые слова и капс"
put plugins protect-spawn "Защита точки появления от ломания и стройки"
put modules modinfo "Пример модуля: загруженные плагины и команды от консоли"
if [ "$EXTRA" = 1 ]; then
  put plugins misbehave "Испытательный: виснет, паникует, ест память по команде из чата"
fi
