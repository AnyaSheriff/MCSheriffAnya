# MCSheriffAnya на Minecraft-хостинге

MCSheriffAnya — самостоятельное серверное ядро на Rust с открытым исходным
кодом и лицензией [MIT](../LICENSE). Java Runtime для запуска не нужен.
Проект: https://github.com/AnyaSheriff/MCSheriffAnya.
Это **альфа**: совместимость с игровыми механиками неполная. Поддерживается
только Minecraft Java **26.1.2, протокол 775**; поддержка Bedrock
**26.10–26.13, протокол 944** частичная. Подробности — в [README](../README_ru.md).

## Образы и данные

| Назначение | Образ | Каталог данных |
| --- | --- | --- |
| Обычный Docker | `sheriffanya/mcsa:<version>`, `sheriffanya/mcsa:latest` | `/data`, UID/GID 10001 |
| Pterodactyl / Pelican | `sheriffanya/mcsa:<version>-panel`, `sheriffanya/mcsa:panel` | `/home/container`, пользователь `container`, UID/GID 999 |

`<version>` — тег релиза без начальной `v`, например `0.2.3-alpha`.
Workflow `.github/workflows/docker.yml` публикует оба варианта для
`linux/amd64` и `linux/arm64` при отправке тега `v*`. Владелец репозитория
должен настроить `DOCKERHUB_USERNAME` и `DOCKERHUB_TOKEN`. Перед установкой
хостер проверяет наличие нужного тега в Docker Hub: изменение файлов в git
само по себе образ не публикует. Для воспроизводимого развёртывания выберите
версионный тег вместо обновляемого `:panel`.

В панели используйте вариант **`-panel` / `:panel`**. Бинарник уже находится
в `/usr/local/bin/mcsheriffanya`, вне тома данных. Установка egg ничего не
скачивает из GitHub Releases и не собирает Rust: отдельный контейнер
`debian:bookworm-slim` создаёт каталоги и два начальных файла конфигурации.
При переустановке существующие файлы сохраняются.

Домашний и рабочий каталог `/home/container`, имя `container`, переменные
`USER`/`HOME` и обработка `STARTUP` следуют
[требованиям образов Pterodactyl](https://docs.pterodactyl.io/v1/guides/egg-creation/creating-custom-image).
UID 999 выбран для этого образа; документация не требует именно это число.
Права на подключённый том настраивает Wings, учитывая свою конфигурацию UID.
Entrypoint переводит `{{VAR}}` в переменные оболочки, разбирает одну команду
с аргументами и запускает её через `exec`. Штатный STARTUP:
`/usr/local/bin/mcsheriffanya`. Составные shell-сценарии помещайте в отдельный
скрипт с завершающим `exec`, а не в STARTUP. `tini` передаёт сигналы процессу.

## Импорт и первый запуск

1. Скачайте JSON [egg-mcsheriffanya.json](../docker/pterodactyl/egg-mcsheriffanya.json)
   через Raw в GitHub. Это формат экспорта **PTDL_v2**, а не ответ API со
   списком eggs. Формат указан и в
   [документации экспорта Pterodactyl](https://docs.pterodactyl.io/v2/api/admin-api/adminExportEgg).
2. В Pterodactyl 1.x откройте **Admin → Nests → Import Egg**, выберите свой
   nest и загрузите JSON. Такой порядок описан в
   [инструкции Pterodactyl](https://docs.pterodactyl.io/v1/guides/egg-creation/creating-custom-egg).
   В Pelican используйте импорт в **Admin → Eggs**. [проверить] Импорт PTDL_v2
   и преобразование настроек на версии Pelican конкретного хостера здесь
   не запускались. В Pelican нет nests; настройки egg описаны в
   [его документации](https://pelican.dev/docs/eggs/creating-a-custom-egg/).
3. Создайте сервер с этим egg и образом `sheriffanya/mcsa:panel` (или добавьте
   в список образов версионный `-panel`). Основную allocation назначьте Java.
   Для Bedrock назначьте дополнительную allocation и разрешите UDP в сети.
4. В Startup задайте параметры из таблицы ниже. `BEDROCK_PORT` меняет
   администратор: он должен совпадать с выделенным UDP-портом. Для сервера
   только Java установите `0`.
5. Выполните установку, затем запустите сервер. В сборке с первоначальным
   выбором ядро остановится до настройки: откройте `config/mcsa.properties`,
   замените `java-version=choose` предложенной в консоли версией, а
   `send-error-reports=choose` — на `true` или `false`. Запустите снова.
   [Подробнее о выборе](java-multiversion.md#выбор-основной-версии-и-первый-запуск).
   Панель ждёт подстроку
   `Готово (` из строки `Готово (<время> с)! Чтобы узнать команды, набери help`.
   Проверьте доступность обоих протоколов отдельно: эта строка не проверяет
   проброс UDP и не заменяет подключение клиента.

Парсер конфигурации egg использует `server.build.default.port` для Java
(синтаксис [Pterodactyl](https://docs.pterodactyl.io/v1/guides/egg-creation/egg-config-parser)).
Если установленный Pelican не преобразовал это поле при импорте, в
**Configuration Files** замените только `{{server.build.default.port}}` на
`{{server.allocations.default.port}}`, как в его документации. Перед первым
входом проверьте фактический порт в `config/server.properties` и консоли.

## Порты и настройки

| Назначение | По умолчанию | Как задаётся |
| --- | --- | --- |
| Java | TCP 25565 | Основная allocation → `config/server.properties`: `server-port` |
| Bedrock | UDP 19132 | `BEDROCK_PORT` → `config/mcsa.properties`: `bedrock-port` |

Панель может выделить другие номера. Одного `EXPOSE` в Dockerfile или
изменения переменной недостаточно: allocation, проброс и межсетевой экран
должны разрешать нужный порт и протокол. `server-ip` записывается как
`0.0.0.0`; сейчас ядро не читает этот ключ и само слушает `0.0.0.0`.

Пути ниже относительны `/home/container` (корню файлового менеджера панели).
Перед каждым запуском панель применяет значения egg к конфигам:

| Переменная | Значение по умолчанию | Файл и ключ |
| --- | --- | --- |
| `BEDROCK_PORT` | `19132` (`0` отключает Bedrock) | `config/mcsa.properties`: `bedrock-port` |
| `ONLINE_MODE` | `true` | `config/server.properties`: `online-mode` |
| `AUTH_ENABLED` | `true` | `config/mcsa.properties`: `auth-enabled` |
| `MAX_PLAYERS` | `20` | `config/server.properties`: `max-players` |
| `MOTD` | `MCSheriffAnya Server` | `config/server.properties`: `motd` |

Эти параметры меняйте в Startup: ручные правки тех же ключей панель
перезапишет. Остальные настройки редактируйте в файлах при остановленном
сервере. `ONLINE_MODE=true` проверяет учётные записи Java. Встроенная
регистрация/пароль с `AUTH_ENABLED=true` действует для Java при
`online-mode=false`; проверенные premium-входы могут её пропускать
(`auth-premium=true` по умолчанию). Bedrock управляется отдельно:
`bedrock-online-mode` в `config/mcsa.properties`, по умолчанию `false`.

**Режим игры:** переменная `GAMEMODE` намеренно отсутствует. В текущем
`src/config/server_properties.rs` ключ `gamemode` только записан в шаблон,
но не читается. В консоли доступны `gamemode creative <игрок>` и
`defaultgamemode survival` (также `creative`, `adventure`, `spectator`).
`defaultgamemode` меняет режим для новых игроков в текущем процессе;
его сохранение после перезапуска не реализовано. Начальный режим в коде —
creative, даже если шаблон свойств содержит `gamemode=survival`.

Настройки скинов находятся в `config/skins.toml`, мир — в `world/`, игроки —
в `playerdata/`, журналы — в `logs/`, база паролей — `auth.db`, операторы —
`ops.json`. Плагины ядра используют `plugins/` и `modules/`; совместимость
с Java-плагинами Bukkit/Spigot не предполагается. Резервируйте весь каталог
сервера. Обновление бинарника выполняется выбором нового образа и
пересозданием контейнера средствами панели; данные остаются в томе.

## Ресурсы и проверка хостером

README не устанавливает минимальные RAM/CPU и не даёт подтверждённых
замеров на игрока. Лимиты подбираются по тестовой нагрузке, генерации мира
и числу игроков. Это нативный процесс: JVM и параметры `-Xmx` не нужны;
память и CPU ограничиваются панелью. При подборе ресурсов учитывайте
`view-distance`, `simulation-distance` и `prefetch-chunks`.

Кнопка Stop должна отправлять `stop` в stdin; ядро также обрабатывает
SIGTERM. Дайте ему закончить сохранение мира до принудительной остановки.

[проверить] Docker-сборка, импорт egg, права тома, консоль `help`/`stop`,
SIGTERM с сохранением, повторный запуск, смена выделенного порта и вход
Java/Bedrock требуют проверки на тестовом узле хостера. Здесь серверы и
Docker не запускались. Для владельца подготовлен
[текст обращения в поддержку](hosting-request.md).
