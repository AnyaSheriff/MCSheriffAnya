#!/bin/bash
set -euo pipefail

cd /home/container

# Формат STARTUP из документации панелей: {{VAR}} превращается в ${VAR}.
# https://docs.pterodactyl.io/v1/guides/egg-creation/creating-custom-image
# "Convert all of the ... parts of the command". Разбираем один раз,
# чтобы содержимое переменных не исполнялось повторно как команды оболочки.
startup=$(printf '%s' "${STARTUP:-/usr/local/bin/mcsheriffanya}" | sed -e 's/{{/${/g' -e 's/}}/}/g')
eval "set -- $startup"
if [ "$#" -eq 0 ]; then
    printf '%s\n' 'STARTUP must contain an executable.' >&2
    exit 1
fi

printf 'container@mcsa~ '
printf '%q ' "$@"
printf '\n'

# Без фонового процесса и конвейера: stdin и сигналы достаются ядру.
exec "$@"
