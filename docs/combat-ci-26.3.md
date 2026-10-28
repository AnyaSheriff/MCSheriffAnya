# Повтор13 прежних боевых случаев26.3

`combat-checks.yml` запускается вручную или только в `wip/validate-combat-*`.
Ветка `wip/prepare-combat-ci-*` сама ничего не запускает. Cargo и npm в этом
workflow отсутствуют: используется готовый runtime успешного migration-checks.

Root передаёт полный build_run_id/head/runtimeSHA256 либо после независимой
проверки записывает их в `tools/blackbox/ci/runtime-pin.json`. Начальный pin
закрепляет успешный run40693373598, код0920966b и отдельно проверенный SHA
бинарника. Частичные параметры, несовпадение заполненного pin, неуспешный run,
чужой repository/workflow, другой receipt или binarySHA останавливают запуск.
Не выбирается latest-run/latest-artifact и не производится скрытая пересборка.

Загружаются только закреплённый непрозрачный официальный JAR26.3 и пять
разрешённых JSONтаблиц из `tools/blackbox/ci/sources-777.json`. JAR только
исполняется, не распаковывается и не запускает reports. Собственный Python
recorder пишет обе стороны; NodeVM replay подменяет foreign protocol modules
заглушками. Чужой код не устанавливается и не изучается.

Список13сценариев закреплён sources-777.json. Это базовые случаи B01–B04,
не вся игровая матрица. Сценарии, decoder и прежние random-допуски не меняются.
`--isolate-vitals` использует уже существующее выравнивание здоровья обеих сторон.
Любой shift/missing/extra/warning остаётся mismatch-open и делает job неуспешным;
неполные записи, decoder или cleanupошибки отдельно дают infrastructure-failed.

Ориентир15–20минут. Общая запись ограничена24минутами; внешний watchdog и
отдельные группы серверов дают ограниченное завершение. В PIDреестре хранится
Linuxstarttime, чтобы поздний cleanup не послал сигнал чужому переиспользованному
PID. Неудачные сценарии не прекращают запись остальных. Оригинал и ядро имеют
свежие отдельные config/world и четыре порта26140…26143.

Artifact загружается через if:always и хранится7дней. Он содержит обе rawзаписи,
проекции/тексты сравнения, серверные логи/config, точные JSONтаблицы,
собственный source snapshot со всеми локальными imports/scenarios, SHAmanifest,
build-selection/build-receipt и ci-receipt. JAR, runtimebinary, миры, symlink
не включаются. Сохранение evidence после ошибки не означает успешного сравнения.

Локальная проверка оркестратора без серверов:
```
PYTHONDONTWRITEBYTECODE=1 python -m unittest discover -s tools/blackbox -p test_combat_ci.py -v
PYTHONDONTWRITEBYTECODE=1 python -m unittest discover -s tools/blackbox -p test_record777.py -v
```
