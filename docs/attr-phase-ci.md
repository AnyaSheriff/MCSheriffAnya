# Проверка чётности атрибутов двумя настоящими роботами

Workflow `attr-phase-checks.yml` запускается для `wip/attr-phase-*` либо вручную.
Он не собирает Rust. Используется только runtime из точного успешного
`migration-checks` run, закреплённого полным commit SHA и SHA256 в
`tools/blackbox/ci/attr-phase/runtime-pin.json`. Ни latest, ни неподтверждённый
артефакт не подходят; проверяются GitHub run и receipt самого артефакта.

`pvp-weakness`, `pvp-resistance`, `pvp-shield-back` выполняются по два раза.
Обе стороны каждого сравнения записываются заново, одновременно, через
одинаковый непрозрачный ViaProxy3.4.14. `via_harness.py` вызывает штатные
`bot.js` и `via_ticks.js`; весь setup, возрождения, экипировка и финальная
проверка `attr_phase` сохраняются. rawrecord777/replay777 не используются.

У каждой записи проверяются обе чётности, их равенство, соответствие
`base_tick` и началу окна. Список команд без ответов сервера, пустая запись,
ненулевой выход робота/сравнения, неполная матрица, предупреждение или
расхождение означают failure. Допуски сценариев не расширяются. Два повтора
подтверждают только этот ограниченный набор, а не полную совместимость боя.

Java25/Node22 устанавливаются GitHub Actions. npm использует отдельный lock
с integrity, без lifecycle scripts; версии mineflayer4.39.0,
minecraft-protocol1.68.0, minecraft-data3.117.0 и vec3 0.1.10 сохранены по
метаданным прежнего рабочего окружения. Исходники зависимостей не читаются.
`MCSA_BB_NODE_MODULES` задаёт каталог модулей, не меняя HOME.
JAR скачиваются только по pinned HTTPS URL с проверкой размера/SHA256
(для vanilla также SHA1). JAR не исследуются и не попадают в артефакт.

Максимум работы harness1800секунд, внешний watchdog1850секунд, шаг34минуты,
job40минут. Каждый сервер и робот имеет собственную группу процессов;
завершение записывается в lifecycle, затем проверяются шесть отдельных
TCP-портов26170/26171/26174/26270/26271/26274. Локальные25565 и VPS не нужны.

Артефакт `attr-phase-777-<run>-<attempt>` содержит обе записи каждой пары,
логи роботов/сравнения/серверов, фазы, точные команды, build selection/receipt,
lifecycle, версии, копии участвующих собственных исходников/сценариев/lock и
контрольные суммы всех файлов. Миры, JAR, runtime и node_modules исключены.
При раннем отказе сохраняются доступные логи и error receipt; прежние PASS
не используются. Сохраняются даже несовпавшие и незавершённые результаты.

Проверки без Minecraft, Cargo и сети:

```sh
PYTHONDONTWRITEBYTECODE=1 python -m unittest discover -s tools/blackbox -p test_attr_phase_ci.py -v
PYTHONDONTWRITEBYTECODE=1 python -m unittest discover -s tools/blackbox -p test_combat_ci.py -v
node --test tools/blackbox/*.test.js
```
