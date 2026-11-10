# Собственный BlockPlace baseline, отдельная партия

Base: reviewed combined candidate 815e26af, не main. Production SDK/C/WIT и весь
Host exact 3c80f6c6. Новые два fixture повторяют только контролируемую логику
priority/cancel/uncancel/ignoreCancelled; старые fixtures и их ожидания не меняются.

12 Host-входов и 10 действий через настоящий place_block исполняются в 22 отдельных
процессах. Каждый процесс запускает production worker с пустой папкой, загружает
компоненты в заданном порядке, публикует PlayerState и Java UI view, проверяет
команды режима через настоящие NBT ответы. Host-входы проверяют старый block_place
без изменения его реализации. Native-входы отдельно сохраняют настоящий hand из
пакета; Java Event не получает выдуманные поля.

Шесть public trace mismatches и два wrong Deny — прогноз из source. Verifier
отдельно вычисляет наблюдённые public и legacy результаты; прогноз не требуется
для валидности записи и не превращает дефект в conformance PASS. Rust nonzero,
неполные записи и ошибки контекста не скрываются этим разделением.

Native Deny доказывает предотвращение commit предмета/мира. Сериализованные
apply_reaction ACK/коррекции сохраняются как raw bytes, но клиентский визуальный
rollback и синхронизация предмета не проверяются. canBuild=true не permission.
against/replaced/item/hand getters UNSUPPORTED. Side effects плагина не откатываются.
Все остаточные reentrant/equal-priority ограничения предыдущего прототипа остаются.

Первый push и CI требуют root review frozen Java/spec/criteria SHA и source scope.
Локальных Cargo/TeaVM/серверов нет. Production hooks/getters/ABI не изменяются.
