#!/usr/bin/env bash
# Запускает integration test настоящего подписанного Guard без /proc.
# Требует Linux с разрешёнными unprivileged user/mount namespaces.
# Аргумент: путь к тестовому executable из cargo test --release --no-run.
set -euo pipefail
: "${MCSA_GUARD_TEST_BUNDLE:?Укажите каталог публичных подписанных файлов Guard}"
[[ $# == 1 && -x "$1" ]] || { echo 'Нужен один исполняемый файл тестов ядра' >&2; exit 2; }
runner=$(realpath "$1")
export MCSA_GUARD_TEST_BUNDLE
# Все mount-действия только в новом частном namespace; хост не меняется.
exec unshare --user --map-root-user --mount --propagation private bash -c '
    set -euo pipefail
    mount -t tmpfs -o size=4k,mode=0555 none /proc
    [[ ! -e /proc/self/fd ]]
    "$1" --ignored --exact auth::tests::guard_signed_module_migration_registration_failure_and_secret_loss --nocapture
' bash "$runner"
