# Аудит питания редстоуна: наша реализация против вики

Дата: 2026-09-20. Версия игры, к которой всё сверялось: Java Edition 26.1.2 (протокол 775).
Источник — только minecraft.wiki (сохранённые страницы `Conductivity`, `Redstone_Dust`,
`Redstone_components`, `Redstone_mechanics`, `Redstone_Comparator`, `Redstone_Torch`).
Кодом Minecraft и серверных ядер не пользовался.

Отдельно отмечено, где вики описывает Bedrock, а не Java: такие места помечены `{{only|bedrock}}`
и к 26.1.2 Java не относятся.

---

## 1. Таблица проводимости построена не по тому признаку

**Как в игре**

> «Conductivity is a block property that determines whether redstone signals can be conducted
> through them (i.e. powered). […] Opaque full blocks (like stone) tend to be conductive while
> partial blocks (like slabs) or transparent blocks (like glass) tend to be non-conductive.
> **However, there are numerous exceptions (see below).**»
> — https://minecraft.wiki/w/Conductivity

> «However, whether or not you can see through the block does not determine whether a block can
> be powered.»
> — https://minecraft.wiki/w/Conductivity

**Как у нас**

`tools/make_block_table.py:479-486` строит `CONDUCTIVE` как «полный куб И не `transparent`
(по данным minecraft-data)»:

```
for low, high in ranges(
    [block for block in blocks if is_full_cube(block) and not block.get("transparent")]
):
```

`is_full_cube` (`tools/make_block_table.py:184-191`) — это `boundingBox == "block"` минус список
частичных имён/суффиксов. Признак `transparent` в minecraft-data — про свет, а не про
проводимость. `src/blocks.rs:292-300` (`conductive`) просто ищет состояние в этой таблице.

То есть у нас нет списка исключений вики вообще — отсюда все расхождения раздела 2.

**Важность**: высокая — это корень почти всех остальных пунктов про проводимость.

---

## 2. Конкретные блоки с неверной проводимостью

**Как в игре** (списки со страницы https://minecraft.wiki/w/Conductivity)

Непроводящие:
> «Block of redstone, Composter, Copper bulb, Copper grate, Decorated pot, Dirt path, Enchanting
> table, Farmland, Glass, Glowstone, Honey block{{only|Java}}, Hopper, Ice, Leaves, Observer,
> Piston, Slab, Stairs, Tinted glass, TNT»

Проводящие (выдержки):
> «Barrier […] Big dripleaf{{only|Bedrock}} […] Double slab […] Mangrove roots […] Slime block
> […] Soul sand, Soul soil […] Target […]»

И отдельно:
> «Blocks of redstone, observers, and pistons are full solid blocks made of materials with the
> solid-blocking property, but are non-conductive.»

**Как у нас** (проверено по `src/blocks_table.rs:14752` через состояния по умолчанию)

| блок | по вики (Java) | у нас `conductive()` |
|---|---|---|
| `redstone_block` | нет | **да** |
| `observer` | нет | **да** |
| `piston` / `sticky_piston` | нет | **да** |
| `glowstone` | нет | **да** |
| `tnt` | нет | **да** |
| `copper_bulb` (все окисления и вощёные) | нет | **да** |
| `dirt_path` | нет | **да** |
| `farmland` | нет | **да** |
| `enchanting_table` | нет | **да** |
| `slime_block` | да | **нет** |
| `mangrove_roots` | да | **нет** |
| `barrier` | да | **нет** |
| двойная плита (`*_slab` с `type=double`) | да | **нет** |
| `big_dripleaf` | нет (в списке проводящих стоит `{{only|Bedrock}}`) | **да** |
| `honey_block` | нет (Java) | нет — совпадает |
| `hopper`, `composter`, `copper_grate`, `decorated_pot`, `ice`, стекло, листва, плиты, ступени | нет | нет — совпадает |
| `soul_sand`, `mud`, `target`, `packed_ice`, `blue_ice`, `redstone_lamp` | да | да — совпадает |

Двойная плита — отдельный случай: `src/blocks.rs:307-317` (`full_cube`) двойную плиту учитывает
особо, а `conductive` (`src/blocks.rs:292`) — нет, потому что таблица собрана по имени с
суффиксом `_slab`.

Вики про сундуки, печи, воронки-как-полные-блоки прямо не пишет; у нас `chest`, `trapped_chest`,
`furnace`, `dispenser`, `dropper`, `barrel`, `jukebox`, `crafter` помечены проводящими, потому что
в minecraft-data у них `boundingBox == "block"`. Для `chest`/`trapped_chest` это почти наверняка
неверно (сундук не полный куб), но списками вики это не подтверждается — проверять в игре.

**Чем это заметно в игре**

- `glowstone`: пыль на светокамне. По вики пыль на него ставится («Redstone dust can be placed on
  conductive blocks as well as glowstone, upside-down slabs, glass, upside-down stairs, and
  hoppers» — https://minecraft.wiki/w/Redstone_Dust), но светокамень нельзя запитать, он не режет
  пыль и сигнал вниз с него не идёт. У нас он ведёт себя как камень.
- `redstone_block`: блок редстоуна над пылью у нас режет вертикальную связь; в игре — нет.
  И пыль у нас «залезает» на блок редстоуна как на камень.
- `piston`: пыль, лежащая на поршне сверху, у нас соединяется вниз-вбок и питает поршень как
  проводящий блок; в игре поршень непроводящий.
- `slime_block`: в игре по слизи сигнал идёт вверх/вниз как по камню, у нас — нет.
- двойная плита: в игре ведёт себя как полный блок, у нас сигнал через неё не идёт.

**Как проверить**

1. Светокамень: `[рычаг] [камень] [светокамень]`, сверху на камне и на светокамне по пыли,
   рядом со светокамнем внизу — лампа. Ожидание: лампа не горит (светокамень не питается, сигнал
   вниз не проходит). У нас загорится.
2. Блок редстоуна: пыль на земле, над ней на высоте +1 блок редстоуна, сбоку на +1 ещё пыль,
   идущая вниз. Ожидание: связь вниз сохраняется (блок редстоуна не режет). У нас порвётся.
3. Двойная плита: линия пыли, поднимающаяся на двойную плиту. Ожидание: сигнал идёт, как по камню.

**Важность**: высокая (`redstone_block`, `piston`, `observer`, `glowstone`, двойная плита),
средняя (остальные).

---

## 3. Взвешенные нажимные плиты всегда дают 15

**Как в игре**

> «wood and stone pressure plates emit redstone signals of strength 15
> light and heavy weighted pressure plates emit a redstone signal whose strength depends on the
> number of entities on the pressure plate»
> — https://minecraft.wiki/w/Redstone_components#Pressure_plate

**Как у нас**

`src/redstone.rs:235-252` (`kind_of`) относит к `Kind::Plate` всё, что кончается на
`_pressure_plate`, включая `light_weighted_pressure_plate` и `heavy_weighted_pressure_plate`.
`src/redstone.rs:416` в `source_output` выдаёт `MAX_POWER` для всех плит,
`src/redstone.rs:474` в `strong_power` — тоже `MAX_POWER`.

**Чем это заметно в игре**: сравнитель, читающий золотую плиту с одним предметом, должен дать 1,
у нас даст 15. Любая счётная схема на взвешенных плитах ломается.

**Как проверить**: положить золотую плиту, бросить на неё один предмет, вывести на линию пыли
длиной 15. Ожидание: хвост погаснет через 1 блок.

**Важность**: средняя (эти плиты редко ставят, но разница грубая).

---

## 4. Соседняя пыль питает нас, даже если она в нашу сторону не смотрит

**Как в игре**

> «A redstone dot does not power anything adjacent to it, but powers the block under it.»
> — https://minecraft.wiki/w/Redstone_Dust

> «While redstone wire always provides power to the directions it points into, it can still point
> into directions in which it cannot give power.»
> — https://minecraft.wiki/w/Redstone_Dust

**Как у нас**

`src/redstone.rs:577-599` в `wire_power` берёт силу соседней пыли без проверки формы:

```
if let Some(wire) = wire_at(world, beside) {
    power = power.max(wire_power_of(&wire).saturating_sub(1));
}
```

То же в ветках «этажом выше» (`src/redstone.rs:587-592`) и «этажом ниже`
(`src/redstone.rs:595-599`). Проверка `wire_points` тут не вызывается вовсе — она есть только в
`weak_power` (`src/redstone.rs:502`), `power_from` (`src/redstone.rs:528`) и `comparator_side`
(`src/redstone.rs:748`).

Замечание: в обычной игре связь пыли с пылью взаимна, поэтому точка рядом с пылью сама
перестраивается в линию. Разница видна только там, где обновление подавлено (update suppression)
или где состояние пыли выставлено командой.

**Чем это заметно в игре**: пыль-точка (`east=north=west=south=none`, `power=15`), рядом с которой
командой поставлена пыль с `power=0`. В игре вторая останется нулевой, у нас станет 14.

**Как проверить**: `/setblock` двумя пылями подряд — одна точка с `power=15`, вторая с `power=0` и
формой, не смотрящей на первую. Ожидание: вторая остаётся 0 до следующего обновления.

**Важность**: низкая.

---

## 5. Пыль не тянется к блокам, которые в игре считаются источниками

**Как в игре**

> «Redstone wire configures itself to point toward adjacent redstone power components and
> transmission component connection points.»
> — https://minecraft.wiki/w/Redstone_Dust

> «Target blocks, pistons{{only|bedrock}}, bells{{only|bedrock}} and jukeboxes{{only|bedrock}} are
> unique in that they connect to adjacent redstone dust.»
> — https://minecraft.wiki/w/Conductivity

Список источников со страницы https://minecraft.wiki/w/Redstone_components (раздел
«Power components») — и то, что каждый из них питает:

| источник | что питает (по вики) |
|---|---|
| Block of redstone | «It does not power any adjacent conductive block» |
| Button | «It powers the conductive block it is attached to» |
| Calibrated sculk sensor | «It powers the conductive block beneath it» |
| Daylight detector | «It does not power any adjacent conductive block» |
| Detector rail | «It powers the conductive block it is attached to» |
| Jukebox | «It does not power any adjacent conductive block» |
| Lectern | «It powers the conductive block beneath it» |
| Lever | «It powers the conductive block it is attached to» |
| Lightning rod | «It powers the conductive block it is attached to» |
| Observer | «It does not activate adjacent components / It powers the conductive block behind it» |
| Pressure plate | «It powers the conductive block that supports it» |
| Redstone torch | «It powers the conductive block that is above it / It does not power/activate the block/component it is attached to» |
| Sculk sensor | «It powers the conductive block beneath it» |
| Target | «It does not power any adjacent conductive block» |
| Trapped chest | «It powers the conductive block beneath it» |
| Tripwire hook | «It powers the conductive block it is attached to» |
| Redstone dust | «It weakly powers the conductive blocks beneath it and it points to» |
| Repeater | «It powers the conductive block it points to» |
| Comparator | «It powers the conductive block it points to» |

**Как у нас**

`src/redstone.rs:175-190` (`Kind`) знает только `Wire, Piston, PistonHead, Torch, Lever, Button,
Plate, RedstoneBlock, Lamp, Repeater, Comparator, Door, Trapdoor, Other`. Соответственно
`wire_connects_to` (`src/redstone.rs:606-624`) тянет пыль только к проводу, факелу, рычагу,
кнопке, плите, блоку редстоуна, повторителю (по оси) и сравнителю. `is_on`
(`src/redstone.rs:397-406`) и `source_output` (`src/redstone.rs:410-441`) тоже знают только их.

Нет как источников: наблюдатель, тарджет, ловушка-сундук, датчик дневного света, крюк
растяжки, громоотвод, кафедра, сенсор скалка, детекторная рельса.

**Чем это заметно в игре**: тарджет — пыль к нему должна тянуться и получать силу от попадания;
у нас пыль его не видит вовсе. Датчик дневного света у нас не даёт сигнала вообще.

**Как проверить**: положить тарджет, рядом пыль. Ожидание: пыль повёрнута к тарджету.

**Важность**: средняя (это известный пробел — деталей просто нет; важно, что когда их будут
добавлять, правило «кого именно они питают» уже выписано выше).

---

## 6. Механизмов, слушающих сигнал, почти нет

**Как в игре**

> «Redstone mechanism components are activated when they receive a redstone signal. This signal can
> be supplied by an adjacent power component generating a signal, a powered block, powered redstone
> dust being configured to point into the mechanism or placed on top of the mechanism, or a powered
> redstone repeater or a redstone comparator pointing into the mechanism.»
> — https://minecraft.wiki/w/Redstone_mechanics

**Как у нас**

`src/redstone.rs:904-1046` (`neighbour_changed`) реагирует только на `Wire, Torch, Repeater,
Comparator, Lamp, Door, Trapdoor, Lever, Button, Plate, Piston, PistonHead`. Всё остальное —
`Kind::Other` (`src/redstone.rs:1045`) и на сигнал не откликается: раздатчик, выбрасыватель,
воронка, крафтер, нотный блок, ТНТ, рельсы, медная лампа, колокол, калитка, люк-калитка,
звонкий блок и т. п.

**Важность**: средняя. Это пробел, а не ошибка формул питания.

---

## 7. Квазисвязность у нас только у поршня

**Как в игре**

> «Quasi-connectivity is a property of dispensers, droppers, and pistons that allows them to be
> activated by anything that would activate the space above them, no matter what is actually in
> that space. […] Despite a functional similarity to dispensers and droppers, crafters are not
> affected by quasi-connectivity.»
> — https://minecraft.wiki/w/Tutorials/Quasi-connectivity

**Как у нас**

`src/redstone.rs:1054-1067` (`piston_powered`) реализует квазисвязность только для поршня.
Раздатчика и выбрасывателя как механизмов нет вовсе (см. пункт 6).

**Важность**: низкая, пока нет раздатчика и выбрасывателя; станет средней, когда они появятся.

---

## Что совпадает

Проверено и расхождений не найдено:

- **Сила 0..15 и затухание на 1 за блок.** `wire_power` (`src/redstone.rs:581`) берёт
  `wire_power_of(...) - 1`, верхняя граница `MAX_POWER = 15` (`src/redstone.rs:32`, ограничение на
  `src/redstone.rs:602`). Вики: «Power level drops by 1 for every block of redstone wire it
  crosses» (Redstone_Dust).
- **Пыль слушает только источники и сильно запитанные блоки.** `power_from` с
  `receiver_is_wire = true` берёт `strong_power` (`src/redstone.rs:539`). Вики: «Redstone dust can
  be powered by an adjacent power component, another transmission component, or a strongly powered
  block» (Redstone_components).
- **Слабо запитанный блок пыль не питает, но питает механизмы, повторители и сравнители.**
  `power_from` с `receiver_is_wire = false` берёт `weak_power` (`src/redstone.rs:541`). Вики:
  «A weakly-powered block cannot power other adjacent redstone wire, but can still power redstone
  repeaters and comparators, and activate adjacent mechanism components» (Redstone_Dust).
- **Пыль слабо питает блок под собой и блоки, в которые смотрит; блок над собой — никогда.**
  `wire_points` (`src/redstone.rs:486-492`): `Down => true`, `Up => false`, по горизонтали — по
  форме. Вики: «It weakly powers the conductive blocks beneath it and it points to»
  (Redstone_components).
- **Точка не питает ничего вокруг, но питает блок под собой.** Форма точки — все четыре стороны
  `none`, поэтому `wire_points` по горизонтали даёт false, а вниз — true. Вики: «A redstone dot
  does not power anything adjacent to it, but powers the block under it» (Redstone_Dust).
- **Непроводящий блок запитать нельзя.** `power_from` (`src/redstone.rs:535-537`) на
  непроводящем соседе возвращает 0. Вики: «Non-conductive blocks cannot be powered» (Redstone_Dust).
- **Вертикальные правила пыли.** `wire_power` (`src/redstone.rs:587-599`): вверх сигнал идёт
  всегда (лишь бы над нижней пылью не было проводящего блока), вниз — только если верхняя пыль
  лежит на проводящем блоке. Вики: «Redstone dust can be powered by redstone dust that is one level
  lower, or on a conductive block one level higher. A non-conductive block cannot{{only|java}} pass
  power downward. The block "between" the two dust blocks must be air or non-conductive. A
  conductive block there "cuts" the connection» (Redstone_Dust). Схема «the signal can never go
  down from slabs» тоже выполняется. Оговорка: связь рисуется (`wire_shape`,
  `src/redstone.rs:627-682`) и без проводящей опоры, а сила вниз не идёт — это и есть поведение
  Java.
- **Сильно питают: повторитель и сравнитель в блок, куда смотрят; факел — блок над собой;
  рычаг и кнопка — блок, к которому прикреплены; плита — блок под собой.** `strong_power`
  (`src/redstone.rs:445-482`). Совпадает с таблицей вики из пункта 5.
- **Блок редстоуна не питает соседние блоки, но питает пыль, механизмы и диоды на 15.**
  `source_output` (`src/redstone.rs:416`) даёт 15, а в `strong_power`/`weak_power` его нет вовсе.
  Вики: «A block of redstone does not power any adjacent blocks» (Redstone_components).
- **Факел не питает и не включает блок, к которому прикреплён.** `source_output`
  (`src/redstone.rs:419-425`). Вики: «A redstone torch does not power or activate the block it is
  attached to» (Redstone_components).
- **Факел гаснет, когда запитан блок, к которому он прикреплён (в том числе слабо).**
  `torch_should_be_lit` (`src/redstone.rs:1293-1307`) смотрит `weak_power`. Вики: «A redstone torch
  is active by default, but is deactivated while the block it is attached to is powered»
  (Redstone_components).
- **Рычаг, кнопка, плита, факел, блок редстоуна дают 15 пыли и диодам, смотрящим от них.**
  `source_output` (`src/redstone.rs:410-441`); для диода вход берётся только со стороны `facing`
  (`diode_input`, `src/redstone.rs:702-707`). Вики: «powers adjacent redstone dust, and redstone
  repeaters and redstone comparators facing away from the lever with a signal of strength 15».
- **Задний вход повторителя и сравнителя принимает и слабо, и сильно запитанный блок.**
  `diode_input` → `power_from(..., false)`. Вики: «A redstone repeater can be powered by a power
  component, transmission component, or a strongly or weakly powered block providing a redstone
  signal to the back input».
- **Боковой вход сравнителя принимает только пыль, повторитель, сравнитель, блок редстоуна и
  сильно запитанный блок.** `comparator_side` (`src/redstone.rs:742-760`). Вики: «Side inputs are
  accepted only if the signal received is strongly powering either side of the redstone comparator.
  {{IN|je}}, blocks of redstone are also accepted if placed directly next to the comparator»
  (Redstone_Comparator).
- **Пыль тянется к повторителю только вдоль его оси, а к сравнителю — со всех сторон.**
  `wire_connects_to` (`src/redstone.rs:615-621`).
- **Пыль сама не поворачивается к механизмам.** `wire_connects_to` возвращает false для
  `Kind::Other`, `Kind::Lamp`, `Kind::Door`, `Kind::Trapdoor` (`src/redstone.rs:622`). Вики:
  «Redstone dust does not automatically configure itself to point toward adjacent mechanism
  components» (Redstone_Dust).
- **Одинокая пыль — крестик, правым щелчком переключается в точку.** `wire_shape`
  (`src/redstone.rs:653-664`) и `use_block` (`src/redstone.rs:1505-1522`). Вики: «When there are no
  adjacent components, a single redstone wire configures itself into a plus sign […] By
  right-clicking, it can be changed into a dot» (Redstone_Dust).
- **С одним соседом пыль вытягивается в линию «к нему и от него».** `wire_shape`
  (`src/redstone.rs:666-679`). Вики: «redstone wire configures itself into a line pointing both at
  the neighbor and away from it».
- **Механизм включается от пыли, которая в него смотрит или лежит на нём.** `any_power`
  (`src/redstone.rs:548-552`) через `power_from` с `wire_points`. Вики: «Powered redstone dust
  activates mechanisms components the dust points to, or is placed on top of».
- **Сильно запитанный блок питает и пыль на себе, и пыль под собой.** `wire_power`
  (`src/redstone.rs:570-572`) перебирает все шесть сторон, включая верх и низ. Вики: «A block is
  strongly powered when it can power adjacent redstone dust (including redstone dust on and beneath
  the block)» (Redstone_mechanics).
