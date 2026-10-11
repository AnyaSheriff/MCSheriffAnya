#!/usr/bin/env bash
# Ставит примеры плагинов (protect-spawn, chat-filter, hello) в наш тестовый сервер tmp/v-ours.
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
bash "$ROOT/plugins-examples/install.sh" "$ROOT/tmp/v-ours"
