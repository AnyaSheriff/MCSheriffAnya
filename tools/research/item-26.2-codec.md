# Ограниченный codec обычных Slot26.2

Восемь собственных записей official776: stone3, custom_name, damage7,
attack_damage modifier, map_color, swing_animation, wolf/collar, empty.
Источник и SHA — `tools/protocol-bridge/testdata/item-26.2-observations.json`.
Это codec предмета, а не включённый inventory flow, новый primary или admission.

## Архитектура

`items::Stack` сохраняет canonical775 item/component IDs. Для776 используются
прямые таблицы775↔776: map_color45 и swing_animation40 нельзя прогонять через777,
где этих компонентов нет. Wolf collar:775component84→776component85, DyeColor
сохраняется. Attributes используют существующий перевод известных Attribute
IDs; attack_damage775=2→776=3. Новый sulfur item1052/component78 не имеет
native775 цели и возвращает MissingTarget, без пустого слота/замены.

Открыты ordinary Slot776 и только payload canonical3,6,16,40,45,84 плюс
четыре нижеуказанных nested components. Остальные added components776 получают
UnsupportedComponent; неизвестный registry ID — UnknownId; известный без
native объекта — MissingTarget. Removed component имеет только ID, переводится
точным соответствием и не требует разбора отсутствующего payload.

Верхние `decode_template/encode_template` и Creative `untrusted=true` для776
закрыты ДО чтения даже пустого слота. Независимый `supported_form` отделяет
эти случаи от ordinary Slot. Сам Slot codec не включает EntityTracker или
Negotiated admission776. В общем batch отдельно добавлены собственные
Configuration/registry и ограниченные Play payload helpers; публичный
вход776 и основной игровой профиль26.2 остаются закрыты.

## Nested: схема подтверждена, live776 ещё нет

Повторно используется `nested_items.rs` без второго parser. Для следующих
канонических компонентов pinned775/776 формы полностью совпали:

| Компонент | Вложенная форма |
| --- | --- |
|25 use_remainder|один ItemStackTemplate|
|49 charged_projectiles|VarInt count + ItemStackTemplate[count]|
|50 bundle_contents|VarInt count + ItemStackTemplate[count]|
|75 container|VarInt count + boolean presence + optional ItemStackTemplate|

ItemStackTemplate = itemID перед count, added/removed counts и компоненты.
Каждый вложенный item/component/attribute ID переводится отдельно; внутри
действует тот же776 allowlist. Сохраняются32уровня,256entries/components,
1 MiB вход/выход и count1..127. Дубли добавленных/удалённых компонентов,
unknown IDs, invalid option bool и все усечённые префиксы не пропускаются.

Fixture содержит exact selected JSON shapes и SHA источников. Это проверка
схемы плюс ранее наблюдённые775/777 nested формы, НЕ live776 nested result.
Верхний particle template не открывается по этому аргументу.

## Scope аудита

В items/nested_items/item_components нет ветки «from==777, иначе775» для
формы776. Все компоненты проходят explicit supported version + allowlist
до reuse complex/nested parser. Старый общий Entity metadata такой binary
выбор имеет, но он остаётся за существующими wire/Tracker guards, не вызван
новым standalone Slot codec. Attribute776 alias на777 IDspace ограничен
реестром Attribute и подтверждён отдельно40официальными наблюдениями.

Прежние supported775↔777 bytes/tests сохранены. Дополнительно native775
теперь умеет принять и записать canonical swing40 и collar84 для пути776;
их775↔777 payload scope не открывается. Map_color775→777 по-прежнему отсутствует.

## Проверки

Шесть исходных групп Rust tests: exact8observations/native identity/all prefixes; direct
40/45/84 mapping включая removal; MissingTarget/UnknownId/forms; malformed
values/counts/attribute fields; четыре nested формы с семью inner cases и
префиксами; nested depth/count/duplicate/missing guards. Старые два guard
assertion обновлены только для теперь открытого ordinary776 Slot. Остальные
wire/admission негативные проверки сохранены.

Дополнительно проверен общий предел1MiB после removed components: слот с
added payload ровно на пределе отклоняется, если removal увеличивает общий
размер; вариант на байт короче принимается. Проверка охватывает775/776/777.

Текущий общий release-прогон:2752PASS,83ignored,0FAIL; Clippy и release build
PASS, три экспорта собственных Configuration/Play fixtures PASS. Полный
прогон предшествовал механическому исправлению test-only nested if в let-chain;
после него повторены Clippy/build/экспорты, условия проверки не менялись.
Offline verifier ранее дал8PASS и13selected schema equalities PASS.

Настоящий клиент776 принял Configuration с обычным и пустым Known Packs,
затем отдельный21-кадровый Play fixture: Teleport ACK, Player Loaded, Pong
и4 grounded samples за2.5968с. На вручную проверенном PNG видны stone3,
paper и diamond sword. Это собственный fixture-сервер, не полный runtime
ядра; tooltip/custom name/damage, nested776 и inventory gameplay не доказаны.
Подробные артефакты и сохранённые исходные FAIL — в
[наблюдениях26.2](../protocol-bridge/observations-26.2.md).
