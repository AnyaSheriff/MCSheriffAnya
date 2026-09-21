# Консоль ванильного сервера Minecraft (Java Edition): что и как печатается

Цель файла — собрать дословные английские строки, которые обычный (ванильный) сервер
пишет в консоль и в `logs/latest.log`, чтобы консоль RustCraft повторяла их состав и
структуру (текст у нас русский, набор строк — как у оригинала).

Обозначения надёжности:

- **[лог]** — строка взята из настоящего файла лога (ссылка/путь указаны).
- **[вики/док]** — формулировка из вики или документации хостинга, дословного лога не нашлось.
- **[?]** — формулировка известна по пересказам, точного лога не нашёл (см. раздел «Чего не нашлось»).

Важная оговорка про источники: часть строк подтверждена на логах **Paper** (ядро,
совместимое с ванилью). Paper печатает собственные строки (помечены как «Paper-only»),
но строки, унаследованные от ванили, в нём идентичны. Где ваниль и Paper расходятся —
отмечено отдельно. Исходный код игры и ядер не использовался: только файлы логов,
вики и форумы.

---

## 1. Формат строки

Общий вид (и в консоли, и в `logs/latest.log` — они идентичны):

```
[HH:MM:SS] [<поток>/<УРОВЕНЬ>]: <сообщение>
```

Пример **[лог]**:

```
[12:34:56] [Server thread/INFO]: Done (3.456s)! For help, type "help"
```
> «[12:34:56] [Поток сервера/ИНФО]: Готово (3.456 с)! Команда справки — "help"»

Детали:

- Дата в строку **не пишется** — только время. Дата есть в имени архивного файла:
  `logs/2026-04-19-1.log.gz`, текущий файл всегда `logs/latest.log`.
- Файл и консоль отличаются только тем, что в консоли (при запуске с GUI/в терминале)
  строки могут подсвечиваться; сам формат тот же.
- Одна строка = одно сообщение; многострочные вещи (stack trace, EULA-предупреждение)
  печатаются несколькими строками, каждая со своим префиксом.

### Потоки, которые реально встречаются

| Поток | Когда встречается | Источник |
|---|---|---|
| `ServerMain` | ранний этап: датафиксер, окружение, загрузка рецептов/достижений, ошибки датапаков | **[лог]** локальные логи, minecraftforum |
| `Server thread` | основной поток: почти всё — старт, вход/выход, чат, команды, тик, остановка | **[лог]** |
| `Worker-Main-<N>` | параллельная генерация чанков — строки `Preparing spawn area: N%` | **[лог]** minecraftforum |
| `User Authenticator #<N>` | проверка аккаунта при входе — строка `UUID of player ...` | **[лог]** локальные логи |
| `main` | у лаунчеров/модлоадеров (Fabric, Forge) вместо `ServerMain` | **[лог]** itzg/docker-minecraft-server#2124 |
| `RCON Listener #<N>` | приём RCON-подключения | **[лог]** itzg/docker-minecraft-server#2495 |
| `RCON Client /<ip> #<N>` | обработка одного RCON-клиента | **[лог]** там же |
| `Netty Epoll IO #<N>` / `Netty Server IO #<N>` | сетевые ошибки/варны | **[лог]** (в наших логах — Paper-имена, см. «Чего не нашлось») |

Уровни: `INFO`, `WARN`, `ERROR`, `FATAL`, `DEBUG`, `TRACE`
(источник: oxygenserv «How to read Minecraft server logs»). В обычной работе ванили видны
только `INFO`, `WARN`, `ERROR`; `DEBUG`/`TRACE` в `latest.log` не пишутся (для них есть
отдельный `debug.log` у ядер, у ванили — только при отдельной настройке log4j).

### 1.21 против 26.x

Отличий формата не нашлось: меняется только строка версии
(`Starting minecraft server version 26.2` вместо `... 1.21.11`) и требования к Java
(26.1+ требует Java 25). Источник: mc-node.net, winternode.com (обзорные статьи).
Дословного лога 26.x найти не удалось — см. «Чего не нашлось».

---

## 2. Запуск сервера

Подтверждённая последовательность ванили (лог 1.19.3, minecraftforum #3172436) **[лог]**:

```
[02:17:42] [ServerMain/INFO]: Building unoptimized datafixer
[02:17:43] [ServerMain/INFO]: Environment: authHost='https://authserver.mojang.com', accountsHost='https://api.mojang.com', sessionHost='https://sessionserver.mojang.com', servicesHost='https://api.minecraftservices.com', name='PROD'
[02:17:44] [ServerMain/INFO]: Loaded 7 recipes
[02:17:44] [ServerMain/INFO]: Loaded 1179 advancements
[02:17:45] [Server thread/INFO]: Starting minecraft server version 1.19.3
[02:17:45] [Server thread/INFO]: Loading properties
[02:17:45] [Server thread/INFO]: Default game type: SURVIVAL
[02:17:45] [Server thread/INFO]: Generating keypair
[02:17:45] [Server thread/INFO]: Starting Minecraft server on localhost:25565
[02:17:45] [Server thread/INFO]: Using default channel type
[02:17:45] [Server thread/INFO]: Preparing level "world"
[02:17:47] [Server thread/INFO]: Preparing start region for dimension minecraft:overworld
[02:17:48] [Server thread/INFO]: Preparing spawn area: 0%
[02:17:48] [Worker-Main-5/INFO]: Preparing spawn area: 8%
[02:17:49] [Worker-Main-2/INFO]: Preparing spawn area: 11%
...
[02:17:59] [Worker-Main-4/INFO]: Preparing spawn area: 91%
[02:18:00] [Server thread/INFO]: Time elapsed: 12380 ms
[02:18:00] [Server thread/INFO]: Done (14.773s)! For help, type "help"
```

Перевод построчно (предложение):

| Оригинал | Русский вариант |
|---|---|
| `Building unoptimized datafixer` | `Сборка неоптимизированного конвертера данных` |
| `Environment: ...` | `Окружение: ...` |
| `Loaded 7 recipes` | `Загружено рецептов: 7` |
| `Loaded 1179 advancements` | `Загружено достижений: 1179` |
| `Starting minecraft server version 1.19.3` | `Запуск сервера Minecraft версии 1.19.3` |
| `Loading properties` | `Чтение настроек` |
| `Default game type: SURVIVAL` | `Режим игры по умолчанию: ВЫЖИВАНИЕ` |
| `Generating keypair` | `Создание пары ключей` |
| `Starting Minecraft server on *:25565` | `Сервер Minecraft запускается на *:25565` |
| `Using epoll channel type` | `Используется тип канала epoll` |
| `Using default channel type` | `Используется тип канала по умолчанию` |
| `Preparing level "world"` | `Подготовка мира «world»` |
| `Preparing start region for dimension minecraft:overworld` | `Подготовка стартовой области измерения minecraft:overworld` |
| `Preparing spawn area: 0%` | `Подготовка области спавна: 0%` |
| `Time elapsed: 12380 ms` | `Затрачено времени: 12380 мс` |
| `Done (14.773s)! For help, type "help"` | `Готово (14.773 с)! Команда справки — "help"` |

Варианты и дополнения:

- `Starting Minecraft server on *:25565` — когда `server-ip` пуст; если задан,
  печатается именно он (`... on localhost:25565`) **[лог]**.
- `Using epoll channel type` (Linux с epoll) / `Using default channel type` —
  оба варианта подтверждены логами **[лог]** (itzg#2124 и minecraftforum).
- Современные версии (1.21.x) печатают `Environment` в стиле record’а **[лог]**
  (локальный лог Paper 1.21.11, строка ванильная):
  ```
  [14:37:58] [ServerMain/INFO]: Environment: Environment[sessionHost=https://sessionserver.mojang.com, servicesHost=https://api.minecraftservices.com, profilesHost=https://api.mojang.com, name=PROD]
  ```
- Датапаки (ваниль, `ServerMain`) **[лог]**:
  ```
  [15:48:30] [ServerMain/INFO]: Found new data pack file/<имя>.zip, loading it automatically
  [15:48:33] [ServerMain/ERROR]: Couldn't load tag minecraft:climbable as it is missing following references: minecraft:chain (from file/ClimbableChain.zip)
  [15:48:33] [ServerMain/ERROR]: Failed to load function <namespace>:<путь>
  [15:48:33] [ServerMain/ERROR]: Registry loading errors:
  ```
  > `Найден новый датапак ..., загружаю автоматически` / `Не удалось загрузить тег ...` /
  > `Не удалось загрузить функцию ...` / `Ошибки загрузки реестров:`

### Предупреждения при старте

EULA (сервер после этого завершает работу) **[лог]** (локальный лог; текст ванильный):

```
[Server thread/INFO]: You need to agree to the EULA in order to run the server. Go to eula.txt for more info.
[Server thread/WARN]: Failed to load eula.txt
```
> `Чтобы запустить сервер, нужно принять лицензионное соглашение. Подробности в eula.txt.`
> `Не удалось прочитать eula.txt`

Offline-mode (4 строки подряд, уровень WARN) **[лог]** (локальный лог Paper 1.21.11,
текст ванильный):

```
[14:38:01] [Server thread/WARN]: **** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!
[14:38:01] [Server thread/WARN]: The server will make no attempt to authenticate usernames. Beware.
[14:38:01] [Server thread/WARN]: While this makes the game possible to play without internet access, it also opens up the ability for hackers to connect with any username they choose.
[14:38:01] [Server thread/WARN]: To change this, set "online-mode" to "true" in the server.properties file.
```
> `**** СЕРВЕР РАБОТАЕТ В НЕБЕЗОПАСНОМ РЕЖИМЕ БЕЗ ПРОВЕРКИ АККАУНТОВ!`
> `Сервер не будет проверять подлинность ников. Будьте осторожны.`
> `Это позволяет играть без интернета, но и даёт любому подключиться под любым ником.`
> `Чтобы изменить это, поставьте "online-mode=true" в файле server.properties.`

Занятый порт **[вики/док]** (Shockbyte/Apex Hosting; дословный лог достать не удалось,
формулировки в статьях совпадают):

```
[Server thread/WARN]: **** FAILED TO BIND TO PORT!
[Server thread/WARN]: The exception was: java.net.BindException: Address already in use
[Server thread/WARN]: Perhaps a server is already running on that port?
```
> `**** НЕ УДАЛОСЬ ЗАНЯТЬ ПОРТ!` / `Ошибка: ...` / `Возможно, на этом порту уже запущен сервер?`

RCON и Query (печатаются сразу после `Done`, если включены в server.properties) **[лог]**
(itzg/docker-minecraft-server#2495):

```
[20:12:52] [Server thread/INFO]: Thread RCON Listener started
[20:12:52] [Server thread/INFO]: RCON running on 0.0.0.0:25575
```
> `Поток RCON Listener запущен` / `RCON слушает 0.0.0.0:25575`

Для Query аналогичные строки — `Starting GS4 status listener`, `Query running on 0.0.0.0:25565` **[?]**
(строки существуют, но подтверждающего лога в открытом доступе найти не удалось).

Про память ваниль сама ничего не пишет: подсказки вида «запускайте с -Xmx…» печатает
скрипт запуска/хостинг, а не сервер. **[?]**

---

## 3. Вход и выход игрока

Полный набор строк одного входа **[лог]** (локальный лог; порядок именно такой:
`UUID` из потока авторизации, затем `joined`/`logged in` — порядок последних двух
в разных версиях может меняться местами, в логах встречаются оба):

```
[14:38:07] [User Authenticator #0/INFO]: UUID of player SheriffAnya is 495b5264-926d-3087-8268-f937d40f5421
[14:38:10] [Server thread/INFO]: SheriffAnya[/127.0.0.1:43020] logged in with entity id 19 at ([world]-496.5, 63.0, -312.5)
[14:38:10] [Server thread/INFO]: SheriffAnya joined the game
```

| Оригинал | Русский вариант |
|---|---|
| `UUID of player <ник> is <uuid>` | `UUID игрока <ник>: <uuid>` |
| `<ник>[/<ip>:<порт>] logged in with entity id <N> at ([<мир>]<x>, <y>, <z>)` | `<ник>[/<ip>:<порт>] вошёл с идентификатором сущности <N> в точке ([<мир>]<x>, <y>, <z>)` |
| `<ник> joined the game` | `<ник> присоединился к игре` |

Замечание по формату координат: в 1.21 печатается `at ([world]-496.5, 63.0, -312.5)`
(имя мира в квадратных скобках) **[лог]**; в старых версиях (1.16 и раньше) встречается
без имени мира: `logged in with entity id 277 at (223.49023041744536, 63.0, 326.45463402555754)` **[лог]**
(mclo.gs/UTBkS26, minecraftforum). Координаты печатаются полной точностью double.

Выход **[лог]**:

```
[00:20:56] [Server thread/INFO]: Mishco lost connection: Disconnected
[Server thread/INFO]: Mishco left the game
```
| `<ник> lost connection: <причина>` | `<ник> потерял соединение: <причина>` |
| `<ник> left the game` | `<ник> вышел из игры` |

Причины в `lost connection:` — дословно **[лог]**:

| Строка | Когда | Русский вариант |
|---|---|---|
| `Disconnected` | игрок вышел сам | `Отключился` |
| `Timed out` | таймаут keep-alive | `Тайм-аут` |
| `Server closed` | сервер останавливается | `Сервер остановлен` |
| `You are banned from this server.` | бан (точка в конце есть) | `Вы забанены на этом сервере.` |
| `Flying is not enabled on this server` | античит полёта | `Полёт на этом сервере запрещён` |
| `Kicked by an operator` | кик оператором | `Кикнут оператором` — **[вики/док]** (Apex Hosting), лога не нашёл |
| `You are not whitelisted on this server!` | вайтлист | `Вас нет в белом списке сервера!` — **[вики/док]**, в старых версиях писалось `white-listed` |
| `Сервер заполнен!` (переводится по локали клиента) | сервер полон | — строка берётся из перевода, у ванили английский вариант `The server is full!` **[?]** |

Форма строки меняется в зависимости от фазы подключения **[лог]**:

- уже в игре: `<ник> lost connection: <причина>`;
- с UUID (после авторизации, до входа в мир): `<ник> (<uuid>) lost connection: Timed out`;
- на фазе логина: `<ник> (/<ip>:<порт>) lost connection: You are banned from this server.`
- в старых версиях на фазе логина вместо ника печатался весь профиль **[лог]** (minecraftforum):
  `com.mojang.authlib.GameProfile@1f917180[id=<null>,name=bonovic,properties={},legacy=false] (/151.51.14.17:59328) lost connection: Disconnected`

Отказ во входе печатается ещё и строкой `Disconnecting` **[лог]**:

```
[Server thread/INFO]: Disconnecting nikitaomg1 (/1.2.3.4:56789): You are banned from this server.
```
> `Отключаю <ник> (/<ip>:<порт>): <причина>`

Прочие предупреждения по игроку **[лог]**:

```
[Server thread/WARN]: <ник> moved wrongly!
[Server thread/WARN]: <ник> moved too quickly! <dx>,<dy>,<dz>
```
> `<ник> двигается неправильно!` / `<ник> двигается слишком быстро! ...`

Строки `<ник> was kicked due to keepalive timeout!` и `<ник> was kicked for floating too long!`
встречаются в наших логах, но это **Paper/Spigot**, у ванили их нет (у ванили вместо
первой — `lost connection: Timed out`) **[лог]** (локальные логи Paper).

Занятый ник (повторный вход тем же аккаунтом) — ваниль выкидывает старую сессию с
причиной `You logged in from another location`. **[?]** Дословного лога не нашёл.

---

## 4. Чат и команды

Чат **[лог]** (с 1.19 непроверенные подписью сообщения помечаются `[Not Secure]`):

```
[Server thread/INFO]: [Not Secure] <nikitaomg2222> привет
[Server thread/INFO]: <nikitaomg2222> привет
```
> `[Не подписано] <ник> сообщение`

- `/say` от консоли печатается как `[Server] <текст>` **[?]**; от игрока — `[<ник>] <текст>` **[?]**.
- `/me` печатается как `* <ник> <текст>` **[?]**. Дословных логов на эти две формы не нашёл.

Обратная связь команд. Ваниль дублирует в консоль результат команды игрока в виде
`[<ник>: <сообщение результата>]` (это работает, пока включено правило `sendCommandFeedback`
/ `logAdminCommands`). Подтверждённые примеры **[лог]** (локальные логи):

```
[Server thread/INFO]: [SheriffAnya: Set own game mode to Creative Mode]
[Server thread/INFO]: [Tama_X2: Set own game mode to Survival Mode]
[Server thread/INFO]: [Tama_X2: Set own game mode to Adventure Mode]
[Server thread/INFO]: [Tama_X2: Set own game mode to Spectator Mode]
[Server thread/INFO]: [SheriffAnya: Set the time to 1000]
[Server thread/INFO]: [SheriffAnya: The difficulty has been set to Peaceful]
[Server thread/INFO]: [Tama_X2: The difficulty has been set to Hard]
[Server thread/INFO]: [Tama_X2: Gamerule locator_bar is now set to: false]
[Server thread/INFO]: [nikitaomg2222: Gave 1 [Enchanted Book] to nikitaomg2222]
[Server thread/INFO]: [nikitaomg2222: Applied effect Health Boost to Bive15335]
[Server thread/INFO]: [SheriffAnya: Applied effect Bad Omen to nikitaomg2222]
[Server thread/INFO]: [SheriffAnya [Эфрит]: Stopping the server]
```

| Оригинал | Русский вариант |
|---|---|
| `Set own game mode to Creative Mode` | `Режим игры изменён на творческий` |
| `Set <ник>'s game mode to Survival Mode` **[?]** | `Режим игры игрока <ник> изменён на выживание` |
| `Set the time to 1000` | `Время установлено на 1000` |
| `The difficulty has been set to Peaceful` | `Сложность изменена на «мирная»` |
| `Gamerule <имя> is now set to: <значение>` | `Правило <имя> теперь: <значение>` |
| `Gave 1 [Enchanted Book] to <ник>` | `Выдано 1 × [Зачарованная книга] игроку <ник>` |
| `Applied effect <эффект> to <ник>` | `Эффект <эффект> наложен на <ник>` |
| `Stopping the server` | `Остановка сервера` |

Команды, выполненные из консоли, печатают свой результат **без обёртки** `[ник: ...]`,
просто текстом **[лог]**:

```
[14:38:13] [Server thread/INFO]: Made SheriffAnya a server operator
[Server thread/INFO]: Nothing changed. The player already is an operator
```
> `<ник> назначен оператором сервера` / `Ничего не изменилось. Игрок уже оператор`

Ошибка команды **[вики/док]** (Minecraft Wiki / форумы; дословного лога с позицией не нашёл):

```
Unknown or incomplete command, see below for error
<команда><--[HERE]
```
> `Неизвестная или неполная команда, ошибка ниже` + строка с указателем `<--[ЗДЕСЬ]`

Важно: строка `<ник> issued server command: /time set 0` — это **Bukkit/Paper**, у ванили
её нет (ваниль печатает только результат в скобках). **[лог]** (локальные логи Paper).

---

## 5. Служебные сообщения

Перегрузка тика **[вики/док]** (SpigotMC, Apex Hosting; формулировка повторяется всюду одинаково):

```
[Server thread/WARN]: Can't keep up! Is the server overloaded? Running 9307ms or 186 ticks behind
```
> `Не успеваем! Сервер перегружен? Отставание 9307 мс (186 тиков)`

Сохранение по `/save-all` **[?]** (формулировки общеизвестны, дословного лога не нашёл):

```
Saving the game (this may take a moment!)
Saved the game
```
> `Сохранение игры (это может занять время!)` / `Игра сохранена`

Автосохранение (каждые 6000 тиков) в ванили в `latest.log` ничего не печатает — в наших
логах строк автосейва нет. **[лог]** (по отсутствию).

Остановка сервера **[лог]** (локальные логи; строки ванильные):

```
[Server thread/INFO]: Stopping the server
[Server thread/INFO]: Stopping server
[Server thread/INFO]: Saving players
[Server thread/INFO]: Saving worlds
[Server thread/INFO]: Saving chunks for level 'ServerLevel[world]'/minecraft:overworld
[Server thread/INFO]: Saving chunks for level 'ServerLevel[world]'/minecraft:the_nether
[Server thread/INFO]: Saving chunks for level 'ServerLevel[world]'/minecraft:the_end
[Server thread/INFO]: ThreadedAnvilChunkStorage (world): All chunks are saved
[Server thread/INFO]: ThreadedAnvilChunkStorage (DIM-1): All chunks are saved
[Server thread/INFO]: ThreadedAnvilChunkStorage (DIM1): All chunks are saved
[Server thread/INFO]: ThreadedAnvilChunkStorage: All dimensions are saved
```

| Оригинал | Русский вариант |
|---|---|
| `Stopping the server` | `Остановка сервера` (ответ на команду `/stop`) |
| `Stopping server` | `Сервер останавливается` |
| `Saving players` | `Сохранение игроков` |
| `Saving worlds` | `Сохранение миров` |
| `Saving chunks for level 'ServerLevel[world]'/minecraft:overworld` | `Сохранение чанков мира 'ServerLevel[world]'/minecraft:overworld` |
| `ThreadedAnvilChunkStorage (world): All chunks are saved` | `Хранилище чанков (world): все чанки сохранены` |
| `ThreadedAnvilChunkStorage: All dimensions are saved` | `Хранилище чанков: все измерения сохранены` |

Разница ваниль/Paper: в ванили все три измерения живут в одной папке мира, поэтому
в `Saving chunks for level` имя уровня одно и то же (`ServerLevel[world]`), а в скобках
у `ThreadedAnvilChunkStorage` — `world`, `DIM-1`, `DIM1`. Paper печатает отдельные миры
(`ServerLevel[world_nether]`) и свои строки `[ChunkHolderManager] Saving all chunkholders for world '...'`
— последних у ванили нет. **[лог]**

Список игроков (`list`) **[вики/док]** (Minecraft Wiki, digminecraft):

```
There are 1 of a max of 20 players online: SheriffAnya
```
> `Сейчас в сети 1 из 20 игроков: SheriffAnya`
(в очень старых версиях формат был `There are 2/20 players online: ...`)

Смерти игроков печатаются в консоль дословно так же, как в чате **[лог]**:

```
[Server thread/INFO]: Bive15335 was slain by Zombie
[Server thread/INFO]: Bive15335 was slain by nikitaomg2222
[Server thread/INFO]: Bive15335 fell from a high place
[Server thread/INFO]: Bive15335 burned to death
[Server thread/INFO]: Nohomejoe4 tried to swim in lava
[Server thread/INFO]: Fortnite_KID08 was shot by Skeleton
```
> `<ник> убит существом <моб>` / `<ник> разбился при падении` / `<ник> сгорел` /
> `<ник> пытался плавать в лаве` / `<ник> застрелен ...`

Достижения — три разные формулировки по типу **[лог]**:

```
[Server thread/INFO]: Bive15335 has made the advancement [Stone Age]
[Server thread/INFO]: Bive15335 has completed the challenge [Cover Me in Debris]
[Server thread/INFO]: Bive15335 has reached the goal [Postmortal]
```
> `<ник> получил достижение [...]` / `<ник> выполнил испытание [...]` / `<ник> достиг цели [...]`

RCON во время работы **[лог]** (itzg#2495):

```
[20:27:35] [RCON Listener #1/INFO]: Thread RCON Client /127.0.0.1 started
[20:27:35] [RCON Client /127.0.0.1 #10/INFO]: Thread RCON Client /127.0.0.1 shutting down
```
> `Поток RCON-клиента /<ip> запущен` / `Поток RCON-клиента /<ip> завершается`

Прочие ошибки, которые реально всплывают **[лог]**:

```
[Server thread/ERROR]: Couldn't save chunk; already in use by another instance of Minecraft?
```
> `Не удалось сохранить чанк: файл занят другой копией Minecraft?`

---

## 6. Чего ваниль НЕ печатает в обычном режиме

Подтверждено отсутствием в просмотренных логах (≈183 000 строк локальных логов + чужие логи):

- отдельные пакеты протокола (ни входящие, ни исходящие) — ни имён, ни размеров;
- движение игроков и сущностей (кроме предупреждений `moved wrongly!` / `moved too quickly!`);
- keep-alive и пинги: ни отправка, ни ответ, ни время отклика;
- статусные пинги из списка серверов (подключение ради иконки/онлайна) — ни строки;
- загрузка/выгрузка чанков в обычной работе (только `Preparing spawn area` на старте
  и `Saving chunks ...` при остановке);
- установка/ломание блоков, инвентарь, редстоун, тики мира;
- обмен пакетами Configuration/реестрами, список датапаков клиента.

Вывод для RustCraft: всё перечисленное держим за флагом отладки (`--debug` / уровень `DEBUG`),
в обычный `latest.log` не пишем.

---

## 7. Где в строках видно IP игрока и адрес сервера

IP игрока у ванили виден только здесь:

1. `<ник>[/<ip>:<порт>] logged in with entity id ...` — единственная строка обычного входа
   с IP. **[лог]**
2. Строки фазы логина/отказа: `Disconnecting <ник> (/<ip>:<порт>): <причина>` и
   `<ник> (/<ip>:<порт>) lost connection: <причина>`. **[лог]**
3. В старом формате — внутри `GameProfile@...[...] (/<ip>:<порт>) lost connection: ...`. **[лог]**
4. RCON: IP клиента входит в имя потока `RCON Client /127.0.0.1 #10`. **[лог]**

В строках `joined the game`, `left the game`, `lost connection: ...` (после входа в мир),
в чате и в командах IP **не печатается**.

Адрес сервера, к которому подключился игрок (hostname из handshake, например
`mc.example.com` против `1.2.3.4`), ваниль **нигде не печатает** — ни одной такой строки
в логах не встретилось, и описания такой строки не нашлось. Сервер печатает только
собственный адрес прослушивания при старте: `Starting Minecraft server on *:25565`.

Раз пользователь хочет видеть адрес подключения, это наше расширение. Предложение —
не ломать ванильный формат, а расширить существующую строку входа, например:

```
Ксения[/192.168.0.9:34648 -> mc.example.com:25565] вошла с идентификатором сущности 19 в точке ([world]12.5, 64.0, -8.5)
```

либо печатать отдельную строку уровня INFO сразу после `UUID of player ...`.

---

## Чего не нашлось

Точных (дословных, из настоящего лога) формулировок не удалось подтвердить для:

1. **Лог версии 26.x целиком.** Ни одного полного лога 26.1/26.2 в открытом доступе
   найти не удалось. Есть только пересказы («строка вида `Starting minecraft server version 26.2`»).
   Отличается ли что-то ещё от 1.21 — неизвестно.
2. **Query-листенер**: `Starting GS4 status listener`, `Query running on 0.0.0.0:25565` —
   строки упоминаются, лога с ними не нашёл.
3. **`/save-all`**: `Saving the game (this may take a moment!)` / `Saved the game` —
   формулировки повторяются во множестве статей, но дословного лога с префиксом не нашёл.
   Также не подтверждены `Turned on world auto-saving` / `Turned off world auto-saving`
   для `/save-on` и `/save-off`.
4. **`/say` и `/me`**: формы `[Server] <текст>`, `[<ник>] <текст>`, `* <ник> <текст>` —
   лога не нашёл.
5. **`/kick`**: печатает ли ваниль в консоль строку вида `Kicked <ник>: <причина>` —
   подтверждения нет. В логах видно только последствие: `lost connection: Kicked by an operator`
   (сама формулировка причины — по статьям хостингов, не по логу).
6. **Сервер полон**: английский оригинал (`The server is full!` или иная формулировка) —
   в наших логах причина уже переведена клиентской локалью.
7. **Занятый ник**: `You logged in from another location` — формулировка не подтверждена логом.
8. **Предупреждение о нехватке памяти** от самого сервера — такой строки не нашлось,
   похоже, её печатают только скрипты запуска и хостинги.
9. **Имена сетевых потоков у ванили**: в наших логах видно `Netty Epoll IO #N` (это Paper).
   Ванильное имя (`Netty Server IO #N` / `Netty Epoll Server IO #N`) по логам не подтверждено.
10. **`Unknown or incomplete command`**: сама первая строка подтверждается статьями,
    но формат второй строки с `<--[HERE]` в логе сервера (а не в чате клиента) не проверен.
11. **Порядок `joined the game` и `logged in with entity id`**: в разных логах встречаются
    оба порядка; от чего зависит — не выяснено.

---

## Источники

Логи (первичные данные):

- Локальные логи серверов на машине пользователя (Paper 1.21.11):
  `/home/lenox/Рабочий стол/server/logs/`, `/home/lenox/Рабочий стол/test/logs/`,
  `/home/lenox/Рабочий стол/sasi/logs/`, `/home/lenox/viruses/serve/logs/` — около 183 000 строк.
- https://mclo.gs/UTBkS26 — «Vanilla Server Log», вход/выход, смерти.
- https://www.minecraftforum.net/forums/support/server-support-and/3172436-minecraft-server-not-starting-spawn-area-not — полный старт ванили 1.19.3.
- https://github.com/itzg/docker-minecraft-server/issues/2124 — старт 1.19.4, `Using epoll channel type`.
- https://github.com/itzg/docker-minecraft-server/discussions/2495 — строки RCON.
- https://www.minecraftforum.net/forums/support/server-support-and/2303639-com-mojang-authlib-gameprofile-1f917180-id-null — старый формат `GameProfile ... lost connection`.

Документация и статьи:

- https://www.oxygenserv.com/en/how-to-read-minecraft-server-logs — разбор формата строки и уровней.
- https://minecraft.wiki/w/Commands/list , https://minecraft.wiki/w/Commands/save — описание команд.
- https://www.spigotmc.org/threads/cant-keep-up-is-the-server-overloaded-running-9307ms-or-186-ticks-behind.492941/ — точная строка «Can't keep up!».
- https://apexminecrafthosting.com/failed-to-bind-to-port-error/ , https://shockbyte.com/billing/knowledgebase/98/Failed-to-Bind-Port-Error.html — блок «FAILED TO BIND TO PORT».
- https://apexminecrafthosting.com/guides/minecraft/server-errors/kicked-by-an-operator/ — «Kicked by an operator».
- https://www.digminecraft.com/game_commands/list_command.php — вывод `/list`.
- https://mc-node.net/blog/en/minecraft-versions-2026-explained/ , https://winternode.com/blog/minecraft/java/26-1-tiny-takeover — версии 26.x и требования к Java.

Ограничение соблюдено: исходный код Minecraft (в том числе декомпилированный,
client.jar/server.jar) и исходный код серверных ядер/библиотек (Paper, Spigot, Fabric,
Forge, Bukkit и любых других) не использовался. Использовались только файлы логов
(текстовый вывод работающих серверов), вики, форумы, баг-трекеры и документация хостингов.
