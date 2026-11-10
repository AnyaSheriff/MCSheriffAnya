# Закрытый native reference768

Это подготовка собственного harness, не выполненная запись и не адаптер.
Public768/767 остаются закрыты. Exact официальные1.21.3 metadata/client/server
и разрешённый protocolJSON закреплены до записи в capture-768-pins.json и runner.
Критерии заранее в capture-768-criteria.json. player_loaded в768 отсутствует.

Запуск намеренно отказывается ДО server/client: BLOCKED_NATIVE_RUNTIME.
Штатный live_client направляет LWJGL/JNA automatic extraction в private каталог;
это не подходит текущему запрету JAR unpack. Проверены known own client1.21.4
cache и via42684071176 artifacts: готовых modern native.so нет. Все8Linux LWJGL
native pins3.3.3 совпадают с1.21.4; совпадение opaque JAR SHA не создаёт готовый.so.
JNA5.14.0 отличается. Старый legacy LWJGL2 не подходит и не использован.

Для продолжения нужен approved готовый extraction-free native runtime с явными
provenance/SHA, launcher без nativeJAR вclasspath и без extractionfallback,
immutable доступность runtime вGitHub. Затем отдельно проверить source/metadata/
SHA/negativeguards, выполнить native reference вGitHub/Xvfb и независимо разобрать
actualraw/lifecycle. До этого нельзя снимать gate или считать synthetic guards
доказательством записи. No local server/GUI/Cargo.

Relay не вставляет пакеты; lock охватывает наблюдение И forwarding ACK.
Registry/chunk содержимое не выгружается: только длины/SHA. Выбранные Login/
ConfigurationACK/Play/inventory/teleport raw ограничены budget. Полные registryNBT,
непустые46slots и дальнейший gameplay остаются отдельными открытыми задачами.
