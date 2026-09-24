# Подводные механики в Minecraft Java 26.1.2

Документация по структуре дна, реках, подводных растениях и структурах, собрана с открытых источников.

---

## 1. Уровень моря и глубины

- **Уровень моря (sea level)**: Y=63 — поверхность воды в стандартных океанах и реках.
- **Дно обычного океана**: около Y=45 (примерно 18 блоков ниже уровня моря).
- **Дно глубокого океана**: около Y=30 (примерно 33 блока ниже уровня моря, в два раза глубже обычного).
- **Выбор биома**: Deep Ocean начинается, когда continentalness значительно ниже, чем для обычного Ocean.

**Источник**: Ocean и Deep Ocean страницы Minecraft Wiki.

---

## 2. Биомы рек

### River (Обычная река)
- **Как выбирается**: генерируется в долинах, где значения PV (Peaks and Valleys) находятся на минимуме (deepest valleys).
- **Характеристики**: ширина и глубина зависят от окружающего рельефа.
- **Ширина**: у океана — широкие, с островами; далее вглубь суши — узкие, часто пересыхают.
- **Границы с биомами**: служат разделением между биомами.
- **Дно**: dirt, clay, sand и gravel с покрытием из seagrass.
- **Не генерируется в**: swamps, mangrove swamps (там вместо них — вода самого биома).
- **Растения**: seagrass на дне, firefly bushes и sugar cane по берегам (sugar cane в воде без условия уникальности).

**Источник**: River страница Minecraft Wiki.

### Frozen River (Замёрзшая река)
- **Температура**: 0.0 (самая холодная).
- **Замерзание**: полная поверхность покрыта льдом (Ice).
- **Дно**: seagrass генерируется, но не может заменить лёд; dirt, clay, sand, gravel как в обычной реке.
- **Растения**: seagrass под водой; firefly bushes и sugar cane (последний вымерзает) по берегам.
- **Впадение**: обычно в frozen ocean, но могут и зацикливаться.
- **Как замерзает**: полностью в холодных температурных зонах.

**Источник**: Frozen River страница Minecraft Wiki.

---

## 3. Дно по биомам — материалы и структуры

### Ocean (Обычный océan)
- **Дно**: в основном один слой **gravel** с пятнами clay, dirt, sand.
- **Строения**: Ocean Ruins, Shipwrecks (NOT Ocean Monuments).
- **Растительность**: seagrass и kelp обильно.
- **Особенность**: kelp "леса" часто касаются или превышают поверхность воды.

**Источник**: Ocean страница Minecraft Wiki.

### Deep Ocean (Глубокий océан)
- **Дно**: **gravel** как в обычном océане, но глубже (Y≈30).
- **Строения**: Ocean Monuments, Ocean Ruins, Shipwrecks.
- **Растительность**: tall seagrass чаще, чем в shallow oceans.
- **Особенность**: на дне сидят стражи (Guardians, Elder Guardians).

**Источник**: Deep Ocean страница Minecraft Wiki.

### Warm Ocean (Тёплый océан)
- **Дно**: один слой **sand** (без dirt, clay, gravel в глубокой части).
- **Корали**: коралловые рифы (coral reefs) с пятью видами (tube, brain, bubble, fire, horn).
- **Морские растения**: **sea pickles** (1–4 штуки на месте, светят underwater: L=6, L=9, L=12, L=15 соответственно); NO kelp.
- **Строения**: Shipwrecks, Ocean Ruins (variant: warm ocean ruin).
- **Рыбы**: tropical fish и pufferfish (NO cod, NO salmon).
- **Отличие**: кораллы требуют, чтобы рядом была вода; умирают за 3–4.95 сек вне воды.

**Источник**: Warm Ocean, Coral, Sea Pickle страницы Minecraft Wiki.

### Cold Ocean, Lukewarm Ocean, Deep Cold Ocean, Deep Lukewarm Ocean, etc.
- **Дно**: грнeral ocean structure (грavель, песок, глина по биомам).
- **Растительность**: seagrass и kelp (в cold/lukewarm, но не в warm/frozen).
- **Особенность**: температурные варианты; структуры зависят от глубины.

**Источник**: страницы отдельных вариантов на Minecraft Wiki.

### Frozen Ocean (Замёрзший océan)
- **Дно**: **stone и варианты** (NO gravel как в других) — сырое каменное дно.
- **Поверхность**: полностью льдом (ice), кроме отдельных участков.
- **Айсберги**: структуры из packed ice, snow blocks и blue ice.
- **Растительность**: seagrass и kelp НЕ генерируются на дне.
- **Рыбы**: cod и salmon (на Bedrock); на поверхности — polar bears и rabbits.
- **Структуры**: Shipwrecks, Cold Ocean Ruins (часто внутри айсбергов).

**Источник**: Frozen Ocean, Iceberg, Blue Ice страницы Minecraft Wiki.

### Deep Frozen Ocean
- **Дно**: stone (как в обычном Frozen Ocean).
- **Айсберги**: есть, большие.
- **Глубина**: в два раза глубже, чем обычный Frozen Ocean.

**Источник**: Frozen Ocean страница.

### Swamp, Mangrove Swamp (с водой)
- **Дно в воде**: patches of **clay** на песочном дне (озёра); в глубокой части — gravel.
- **Растительность**: seagrass обильно; lily pads на поверхности; sand/gravel disks НЕ генерируются.
- **Вода**: teal (mangrove) / серо-зелёная (swamp); толстый underwater fog (видимость ≈30 блоков).
- **Рыбы**: tropical fish в mangrove swamp; drowned underwater.

**Источник**: Swamp, Mangrove Swamp страницы Minecraft Wiki.

### Beach (Пляж)
- **Материал**: sand с sandstone под ним.
- **Дно**: sand продолжается несколько блоков вглубь воды.
- **Структуры**: Buried Treasure, Shipwrecks.

**Источник**: Beach страница Minecraft Wiki.

---

## 4. Подводная растительность

### Seagrass (Водоросли)
- **Биомы**: реки, frozen rivers, non-frozen oceans, swamps, mangrove swamps.
- **Варианты**: single (1 блок) и tall (2 блока).
- **Требование**: вода (source или flowing), требуется доступ к sky.
- **Заготовка**: только ножницами (shears), иначе не дропается.
- **Частота**: обильно на дне подводных биомов; tall seagrass чаще в deep oceans.

**Источник**: Seagrass страница Minecraft Wiki.

### Kelp (Ламинария)
- **Биомы**: ocean (все кроме warm, frozen, deep frozen).
- **Часто**: вместе с seagrass, "леса" часто касаются поверхности.
- **Высота**: 2–26 блоков в зависимости от age (0–24 при посадке; прекращает рост на age=25).
- **Шанс**: примерно 1/18 за чанк (чанки с растительностью).
- **Рост**: медленный без bone meal; 14% шанс за random tick (в среднем ~487.6 сек = ~9752 тика на рост на 1 блок).

**Источник**: Kelp страница Minecraft Wiki.

### Sea Pickle (Морской огурец)
- **Биомы**: только warm ocean на дне рифов (на coral blocks).
- **Группы**: 1–4 штуки на месте (так же как свечи).
- **Свет**: L6 (1), L9 (2), L12 (3), L15 (4) под водой.
- **Частота**: примерно 1/6 шанс за чанк.
- **Размножение**: через bone meal на coral blocks (радиус ≈2 блока).

**Источник**: Sea Pickle страница Minecraft Wiki.

### Sugar Cane (Сахарный тростник)
- **Биомы**: у побережья реки, swamps, mangrove swamps.
- **На воде**: может расти на edge блоков рядом с водой.
- **В холодной воде**: вымерзает (uproots itself в frozen rivers).

**Источник**: Swamp, Frozen River страницы.

### Lily Pad (Кувшинка)
- **Биомы**: swamps, mangrove swamps — рассеяны по поверхности воды.

**Источник**: Swamp, Mangrove Swamp страницы.

---

## 5. Подводные структуры (назначение только, для деталей см. отдельные документы)

- **Ocean Ruins**: генерируются в oceans и deep oceans.
- **Warm Ocean Ruins**: вариант в warm oceans.
- **Cold Ocean Ruins**: вариант в cold oceans и frozen oceans, часто в айсбергах.
- **Shipwrecks**: генерируются во всех océans и на пляжах.
- **Ocean Monuments**: ТОЛЬКО в deep oceans (содержат Guardians, Elder Guardians).
- **Buried Treasure**: генерируется на пляжах.

**Источник**: страницы отдельных структур.

---

## 6. Коралловые рифы в тёплом océане (Coral reef feature)

### Виды кораллей
- **Tube Coral** — трубчатый.
- **Brain Coral** — мозговой.
- **Bubble Coral** — пузырьковый.
- **Fire Coral** — огненный.
- **Horn Coral** — рогатый.

### Формы/Структуры
- **Tree** — древовидная (основная).
- **Mushroom** — грибовидная.
- **Claw** — когтевидная.

### Характеристики
- **Требование**: рядом ДОЛЖНА быть вода (source или waterlogged block), иначе мертвеет за 3–4.95 сек.
- **Мертвые кораллы**: становятся серыми (dead coral variants).
- **Освещение**: НЕ излучают свет (в отличие от sea pickles).
- **Добыча**: Silk Touch, иначе дропа нет.

**Источник**: Coral, Coral Block, Coral Fan страницы Minecraft Wiki.

---

## 7. Магма и пузырьковые колонны (Bubble columns)

### Магма блоки (Magma blocks)
- **Генерация**: в underwater aquifers как кластеры 1–8 блоков (редко); в sulfur springs как 1–3 блока.
- **Эффект**: наносят урон огнём (1 ед. за 0.5 сек с immunity), создают downward bubble columns.
- **Свет**: L=3.

### Пузырьковые колонны (Bubble columns)
- **Downward (whirlpool)**: от магма блоков — тянут вниз (4.9 блок/сек в Java).
- **Upward**: от soul sand — пускают вверх (11 блок/сек в Java).
- **Воздух**: дают air при нахождении в них (как выход из воды).
- **Высота**: от водяного дна до поверхности воды.
- **Создание/разрушение**: 20 тиков после размещения/разрушения источника.

**Источник**: Magma Block, Bubble Column страницы Minecraft Wiki.

---

## 8. Айсберги в Frozen Ocean

### Состав
- **Snow blocks** — основная масса.
- **Packed Ice** — внутри и по краям.
- **Blue Ice** — на дне (bottom).

### Размер и форма
- **Большие** — часто касаются или выступают над поверхностью.
- **Редко**: целый айсберг из Blue Ice.

### Особенности
- **Структуры** (Shipwrecks, Cold Ocean Ruins) часто генерируются внутри айсбергов.

**Источник**: Iceberg, Blue Ice, Frozen Ocean страницы.

---

## 9. Специальные материалы под водой

### Ice, Packed Ice, Blue Ice (Лёд)
- **Ice**: стандартный лёд на поверхности frozen waters.
- **Packed Ice**: в айсбергах, не плавится от света.
- **Blue Ice**: самый скользкий (коэффициент 0.989 vs 0.98 для льда); на дне айсбергов; арки в frozen oceans.

**Источник**: страницы Ice, Packed Ice, Blue Ice.

### Sand (Песок)
- **Где**: на дне warm oceans (один слой), в beach biomes (четыре слоя глубже), в shallow waters других биомов.
- **Поддержка**: в oceans обычно NO sandstone под песком (в теплых океанах); на пляжах — YES sandstone.

**Источник**: Sand страница.

### Gravel (Гравий)
- **Где**: на дне обычных oceans (один слой), в cold/lukewarm oceans, в других грassy biomes в shallow waters как disk.
- **Не генерируется**: в frozen oceans (вместо него — stone).

**Источник**: Gravel страница.

### Clay (Глина)
- **Где**: in shallow waters (disk-like) в rivers, lakes, oceans; в swamps на дне как patches; в lush caves обильно.
- **Требование**: заменяет dirt в shallow water.

**Источник**: Clay страница.

---

## 10. Aquifers и водные озёра под поверхностью

- **Aquifer**: наполняет пещеры водой на разных уровнях (в том числе под поверхностью).
- **Underwater magma**: кластеры магма блоков в aquifer на дне.
- **Можно встретить**: водяные озёра в caves.

**Источник**: Lake, Aquifer страницы; упомянуто в Magma Block.

---

## 11. Растения у реки (Sugar Cane — дополнение)

- **Частота**: более обычна на берегах рек (extended coastlines).
- **Условие**: на блоке рядом с водой (dirt, grass, sand, gravel).
- **В холодных реках**: вымерзает и исчезает.

**Источник**: River, Frozen River, Swamp страницы.

---

## Итоговые численные параметры

| Параметр | Значение | Источник |
|----------|----------|----------|
| Sea level (Y) | 63 | Ocean |
| Shallow ocean floor (Y) | ~45 | Ocean |
| Deep ocean floor (Y) | ~30 | Deep Ocean |
| Kelp min height | 2 blocks (age 24) | Kelp |
| Kelp max height (natural) | 26 blocks (age 0) | Kelp |
| Kelp growth chance | 14% per random tick | Kelp |
| Kelp avg growth time | ~487.6 sec (~9752 ticks) | Kelp |
| Sea Pickle groups | 1–4 per location | Sea Pickle |
| Sea Pickle spawn chance | ~1/6 per chunk | Sea Pickle |
| Seagrass generation | Everywhere underwater (tall more in deep) | Seagrass |
| Ice top surface | Partial in frozen oceans | Frozen Ocean |
| Magma clusters underwater | 1–8 blocks | Magma Block |
| Magma in sulfur springs | 1–3 blocks | Magma Block |
| Bubble column fall speed (Java) | 4.9 blocks/sec (downward) | Bubble Column |
| Bubble column rise speed (Java) | 11 blocks/sec (upward) | Bubble Column |
| Sea Pickle light L=1 | 6 | Sea Pickle |
| Sea Pickle light L=2 | 9 | Sea Pickle |
| Sea Pickle light L=3 | 12 | Sea Pickle |
| Sea Pickle light L=4 | 15 | Sea Pickle |

---

## Замечания

1. **Версия 26.1.2**: все данные актуальны для Java Edition.
2. **Численные параметры**: где указаны шансы, это вероятности на уровне feature generation, не гарантии.
3. **Floored/ceiling значения**: depth и height примерны и зависят от шума генерации (noise router).
4. **Нет инструментов для Bedrock Edition 26.1.2**: если нужны данные для BE, требуется отдельное исследование (может отличаться).

---

Составлено: 2026-09-23  
Источники: minecraft.wiki, Minecraft Java Edition 26.1.2 Wiki
