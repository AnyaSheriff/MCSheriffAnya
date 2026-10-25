# Перепись генерации 26.3

Измерения завершены на ветке `wip/genaudit-26.3`. Исходники сервера не исправляются.
Основное семя владельца — `1792932773345722119`, сохранённая точка появления —
`(-528, 96)`. Второе семя — `100554032945340`.

Большие миры и полные отчёты лежат вне репозитория:
`~/.cache/mcsheriffanya/gen-audit-263/`. Папка `worlds/` содержит отдельные
`owner-vanilla`, `owner-ours`, `second-vanilla`, `second-ours`.

Основная выборка — одинаковые чанки на обеих сторонах. Квадрат включает все
столбцы в пределах 1536 блоков от выбранной точки появления по каждой оси,
включая оба края. Для семени владельца это 37 249 чанков, от `(-129, -90)`
до `(63, 102)`. Координаты чанков и настройки сохранены в `*-plan.json`.
Квадрат не означает круг: углы тоже входят в подсчёт. Поля вокруг него,
которые оригинал создал сам, в сравнение не попадают.

Незер: 1089 чанков, от `(-16, -16)` до `(16, 16)`. Энд: 625 чанков вокруг
главного острова и два участка по 1089 чанков на внешних островах,
с центрами около `(2048, 2048)` и `(-2048, 2048)` в блоках.
Статистика этих трёх участков Энда также сохраняется раздельно.

Дополнительные участки находятся командами оригинала `locate biome` и
`locate structure`. Они хранятся отдельно, в `~/.cache/mcsheriffanya/gen-audit-263/targets/`.
Обычный участок биома — квадрат 9 × 9 чанков вокруг найденного места.
Для океанов берётся 33 × 33 чанка, для эродированных бедлендов и ледяных
пиков — 17 × 17: малое окно часто не вмещает берег или характерный рельеф. Для построек
берётся 17 × 17 чанков, для крепости Незера — 25 × 25. Эти выбранные
специально участки нельзя смешивать с основным квадратом для оценки долей
биомов. Их сравнивают по одному, параметром `--target-name`.

Оригинал запускается как чёрный ящик: Java 25, версия 26.3, не более
`-Xmx3G`, порт 26180. Наш release-сервер использует TCP 26280 и UDP 26281.
Миры создаются командами самого сервера. Нет игроков, сложность мирная,
случайные изменения блоков и появление мобов отключены. Наши дополнительные
осенние леса и розовая трава отключены. Семя подтверждается командой `seed`,
ответ записывается в `observed-seed.json`.

В нашей консоли нет общего `execute in … run forceload`. Поэтому Незер
и Энд создаются тем же release-сервером с соответствующим типом основного
мира; после остановки сохранённые регионы переносятся в папку измерения
сводного мира. Это тот же генератор блоков; поведение переходов между
измерениями и тики порталов этим способом не проверяются.

Инструмент читает только сохранённые `.mca` и `.mcs`, проверяет координаты,
полноту чанков и контрольные суммы нашего формата. Он не читает архивы
игры, её код или наборы данных генерации. У оригинала 26.3 поддержаны
новые имена полей палитры. Временные миры не входят в репозиторий.

Допуск — правило сортировки задач, а не доказательство равенства. Для долей
биомов он равен 1 процентному пункту; для высот — 4–5 блокам; для доли
перепадов от 16 блоков — 0,25 процентного пункта; для состава поверхности —
5 процентным пунктам; для подземного воздуха и жидкостей — 2 процентным
пунктам. Для руд и растительности используется 20% от числа оригинала
с небольшим нижним пределом. Точные пределы записаны у каждой строки JSON.

Проверяются все блоки, а также биомы в объёме и у поверхности, гистограммы
высот и уклонов, перепады у границ чанков и внутри них, верхний природный
блок и пять слоёв под ним, воздух и жидкости по слоям высотой 16 блоков,
руды, растительность по биомам и содержание 26.3. Расстояния до суши и
размеры пятен считаются по сетке биомов с шагом 4 блока; значения,
обрезанные краем выборки, отмечены отдельно.

Число оснований стволов и их вертикальный размах — приближение к числу
и высоте деревьев. Горизонтальные брёвна включают и ветви, и упавшие деревья.
Связные объёмы воздуха описывают форму пещеры внутри чанка; они не доказывают,
какой именно приём генерации её создал. Аналогично связные объёмы аметиста,
кости, льда и руды описывают блоки, а не точное число именованных объектов.
Наличие травы под растением само по себе не означает ошибку распада травы.

Наши регионы не хранят начала построек. Дополнительный Rust-инструмент
перечисляет подходящие начала из нашей release-библиотеки с профилем 26.3.
Это проверка выбора мест, а не доказательство фактической постановки всех
блоков. Оригинальные начала читаются из регионов. Для доступных рамок частей также подсчитываются блоки внутри их объединения:
перекрытия не считаются дважды, обрезанные выборкой экземпляры исключаются.
Этот состав включает воздух и природный рельеф, поэтому не равен списку
блоков шаблона. Для остальных построек доступны только блоки стартового чанка. Для лагерей адаптер компилирует неизменённую чистую часть нашего
`abandoned_camp.rs` во временном модуле и открывает рамки его частей;
рабочие исходники при этом не меняются. Эти ограничения не считаются успешным совпадением.

Для воспроизведения из корня репозитория:

```sh
AUDIT_DIR="$HOME/.cache/mcsheriffanya/gen-audit-263"
python3 -m venv "$AUDIT_DIR/venv"
"$AUDIT_DIR/venv/bin/pip" install -r tools/measure/genaudit_263_requirements.txt
while pgrep -x cargo; do sleep 15; done
CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR="$AUDIT_DIR/target" cargo build --release
"$AUDIT_DIR/venv/bin/python" tools/measure/genaudit_263_generate.py \
  --output "$AUDIT_DIR/worlds" --owner-seed 1792932773345722119 \
  --jar "$HOME/.local/share/mcsa-java-26.3-probe/server.jar" \
  --ours "$AUDIT_DIR/target/release/mcsheriffanya"
"$AUDIT_DIR/venv/bin/python" tools/measure/genaudit_263.py \
  "$AUDIT_DIR/worlds/owner-ours/world" \
  "$AUDIT_DIR/worlds/owner-vanilla/world" \
  --plan "$AUDIT_DIR/worlds/owner-plan.json" \
  --release-deps "$AUDIT_DIR/target/release/deps" \
  --cache "$AUDIT_DIR/analysis-cache" --output "$AUDIT_DIR/reports/owner"
```

Для второго семени в последней команде заменить `owner` на `second`.
Для точного повторения в новом каталоге предварительно скопировать туда
сохранённые `owner-plan.json` и `second-plan.json`: они фиксируют также
центры и окна. Сами семена не фиксируют выбранную область. При отсутствии
планов сборщик выбирает центр заново из текущего `level.dat` владельца либо
из точки появления созданного мира; после внешнего сброса это другой центр.
Исходные окна и хеши планов продублированы в малом `provenance.json`.

Адаптер для повторного использования в дополнительных окнах:

```sh
"$AUDIT_DIR/venv/bin/python" tools/measure/genaudit_263_build_starts.py \
  --release-deps "$AUDIT_DIR/target/release/deps" \
  --output "$AUDIT_DIR/genaudit-starts"
```

После создания основных миров запустить сборщик с отдельным выходным
каталогом `--output "$AUDIT_DIR/targets"`, `--owner-seed 1792932773345722119` и `--targets --only vanilla
--label owner`, затем с тем же каталогом и `--targets --only ours --label owner`. Для сравнения
одного найденного биома передать анализатору `--plan …/targets/owner-targets-plan.json
--target-name eroded_badlands` и отдельное имя отчёта. Для построек используется, например,
`--target-name structure:stronghold`. Все такие окна можно обработать
`genaudit_263_targets.py` с двумя папками мира, параметрами `--plan`, `--output`
и `--starts-executable "$AUDIT_DIR/genaudit-starts"`. Отчёт с неполными
чанками по умолчанию отклоняется. `--allow-incomplete` разрешает только
явно предварительное сравнение; разные наборы чанков не сравниваются.

Отдельная выборка лагерей повторяется с `--output "$AUDIT_DIR/camps"`,
`--owner-seed 1792939653127071019 --camps-only --label owner`, сначала
`--only vanilla`, затем `--only ours`. Она использует третье семя намеренно.
Обработать её тем же `genaudit_263_targets.py` с планом из `camps/` и
выходом `reports/camps/`. Для первоначального семени нужен новый отдельный
каталог и `--owner-seed 1792932773345722119`.

Проверки инструмента:

```sh
"$AUDIT_DIR/venv/bin/python" -m unittest discover \
  -s tools/measure -p test_genaudit_263.py -v
```

Пакеты исправлений должны делиться по файлам. Биомы, высоты и поверхность
объединены в A (`terrain.rs`); руды и подземная генерация — в U
(`underground.rs`, `underground_carvers.rs`, `cave_biomes.rs`, `sulfur_caves.rs`).
Деревья — `F-tree` (`trees.rs`), наземная растительность — F (`vegetation.rs`),
морские растения — `F-ocean` (`ocean.rs`), Незер — N (`nether.rs`),
Энд — E (`the_end.rs`). Постройки делятся по своим модулям, например
`village.rs`, `trial_chambers.rs`, `end_city.rs`. Это предположения о месте
причины, а не результаты исправления. Если задача требует общего
`mod.rs` или `terrain.rs`, её нужно согласовать с пакетом A.



В процессе переписи живой исходный мир был извне сменён на семя
`1792939653127071019`. Основные два мира и главная дополнительная выборка
сохраняют первоначальное семя и план. Отдельные лагеря в папке `camps/`
собраны на новом семени с обеих сторон: это третья дополнительная выборка,
её координаты не относятся к первоначальному миру. Для повторения новых
выборок с прежним семенем нужно передавать `--owner-seed 1792932773345722119`.
Уже записанный план всегда сохраняет своё семя.


## Главные задачи по заметности

Во всех числах ниже сначала наш сервер, затем оригинал. Если не сказано иначе,
это первоначальное семя владельца. Координаты — блоки X/Z; они указывают пример
или область проверки, а не обязательно точку максимума. Для общей статистики
местом является весь основной квадрат X от −2064 до 1023, Z от −1440 до 1647.
Все 20 строк требуют разбора; они не означают 20 независимых причин.
Порядок здесь учитывает заметность и широту эффекта; машинный порядок ниже
оценивает величину отклонения от допуска. Имена файлов — предположения.

| № | Что видно / единица | У нас | Оригинал | Где проверять X/Z | Пакет / возможный файл |
|---:|---|---:|---:|---|---|
| 1 | Наличие нужного поверхностного биома в его найденном окне, из 52 | 3 | 52 | Все 52 окна `targets`; пример леса 2576/−1936 | A / biome_distribution.rs |
| 2 | Тёплый океан: медиана расстояния до суши, блоки | 395,35 | 107,63 | Основной квадрат; пример биома −1024/1264 у оригинала | A / terrain.rs |
| 3 | Высота поверхности: p95 / максимум Y | 115 / 169 | 85 / 210 | Основной квадрат; пример горного места −1860/−1261 | A / terrain.rs |
| 4 | Точка появления X/Z; расстояние между ними 562,74 блока | −528/96 | 16/−48 | Эти две точки | A / spawn_search.rs |
| 5 | Доля пустыни у поверхности, % | 14,7737 | 1,0607 | У нас −512/92; оригинал −1012/1464 | A / biome_distribution.rs |
| 6 | Медиана площади пятна пустыни без обрезанных пятен, блоки² | 16 | 5024 | Основной квадрат; сетка 4 блока, 62 / 5 пятен | A / biome_distribution.rs |
| 7 | Новый dappled_forest: доля окна, %; брёвна poplar на чанк | 0; 0 | 36,8682; 13,7531 | Окно около 2576/−1936 | A / biome_distribution.rs |
| 8 | Совпадение чанков подходящих начал построек с сохранёнными оригинальными, 15 семейств | 0% у каждого | Цель 100% | Основной квадрат; пример шахты −1992/120 | R / mineshaft_start.rs |
| 9 | Шахты: подходящие начала / сохранённые начала | 107 | 161 | Основной квадрат; кандидат −1992/120 | R / mineshaft.rs |
| 10 | Древние города: подходящие начала / сохранённые начала | 7 | 0 | Кандидат −1912/−1320 | R / ancient_city.rs |
| 11 | Воздух ниже поверхности, Y 0…15, % | 12,4402 | 7,3150 | Основной квадрат; пример −512/−512, Y=0 | U / underground_carvers.rs |
| 12 | p95 связного объёма пещеры внутри чанка, блоки | 3824,8 | 1755 | Основной квадрат; все подземные слои | U / underground_carvers.rs |
| 13 | Короткая трава в forest на миллион блоков этого биома | 30,8715 | 62,2790 | Лес у нас −164/664; оригинал −512/−512 | F / vegetation.rs |
| 14 | Листовой опад, блоков на чанк | 7,6616 | 18,7969 | Основной квадрат; пример опада −162/666 | F / vegetation.rs |
| 15 | Корни мангра в mangrove_swamp на миллион блоков биома | 560,914 | 1218,899 | Мангры у нас −916/−20; оригинал 280/−512 | T / trees.rs |
| 16 | Стебли ламинарии, блоков на чанк | 31,6243 | 11,6490 | Основной квадрат; пример −147/−507 | O / ocean.rs |
| 17 | Синие коралловые блоки, блоков на чанк | 2,6938 | 0,3627 | Основной квадрат; пример −503/−512 | O / ocean.rs |
| 18 | Незер: доли soul_sand_valley / basalt_deltas, % | 40,1343 / 0 | 13,8659 / 19,9610 | Долина −228/−232; базальт −144/−256 | N / nether.rs |
| 19 | Незер: ископаемые, подходящие начала / сохранённые начала | 54 | 20 | Кандидат −216/−216 | N / nether_fossil.rs |
| 20 | Энд: доля соседних столбцов с перепадом ≥16 блоков, % | 1,4262 | 1,1046 | Три окна Энда; пример −16/−125 | E / the_end.rs |

В строках 8–10 и 19 сравнивается модель подходящих мест с сохранёнными
началами оригинала. Для строки 8 значение 100% — проверяемая цель, а не
дополнительное измерение оригинала. Если в оригинальной выборке нет экземпляров
(древний город), это особенно требует проверки условий допуска места, а не
вывода о семи фактически построенных городах. В строке 7 отсутствие деревьев
нельзя отделить от отсутствия нужного биома. Строки 13 и 15 условны по биому,
но измеряются в разных его пятнах: это плотности, не сравнение одинаковых деревьев.
Глобальные числа растительности также зависят от долей биомов.

## Эродированные бедленды, берега и уже занятые задачи

В найденном оригиналом окне около `−5456/−816` доля eroded_badlands равна
0% у нас против 39,5559% у оригинала. Для сравнения формы именно этого биома
взяты наши 42 384 столбца основного квадрата и 29 265 столбцов оригинального
дополнительного окна: p50 Y = 95 / 120, p95 = 128 / 157, максимум = 137 / 163.
Это **разные участки одного семени**, а не попарная проверка шпилей.
Разность максимума и медианы 42 / 43 блока сама по себе не подтверждает
исчезновение шпилей. Нужна отдельная проверка их формы после выравнивания биомов.
Расположение наших примеров: `−1396/−384`, `−1536/216`.
Терракотовые полосы по Y, поверхность и пять нижних слоёв сохранены в JSON.

Для warm_ocean у второго семени расстояние до суши p50 также больше:
382,27 / 82,46 блока. Пакет берега и пакет шпилей должны входить в A либо
идти последовательно с ним: они затрагивают тот же `terrain.rs`.

В основном квадрате владельца доля перепадов ≥16 блоков между чанками
0,1417% / 0,1798%, внутри чанков 0,1258% / 0,1682% (точные числа — JSON).
По принятому допуску 0,25 процентного пункта это не новый провал.
Проверка не подтверждает общего избытка пограничных обрывов на этом семени;
это не отменяет уже найденных локальных случаев в другой задаче.
Трава с любым не-воздухом сверху: 8,3218 / 18,6063 на 100 столбцов.
Здесь учитываются растения и опад, поэтому это не отдельный диагноз распада
травы или травы под твёрдым блоком. Существующие работы по траве и границам
нужно учитывать перед выдачей пакетов A и F.

## Покрытие и пределы вывода

По двум основным семенам проверены 82 282 пары чанков, то есть 164 564
сохранённых чанка двух серверов. В каждом мире полностью измерены 37 249
чанков обычного мира, 1089 Незера и 2803 Энда. Неполных сравнений нет.
Для обычного мира читается Y −64…319, для Незера и Энда Y 0…255.
Ненулевых блоков вне этих диапазонов не найдено. Это около 15,67 млрд
проверенных ячеек блоков на обеих сторонах вместе.

Основные отчёты содержат 1054 и 1180 строк вне допуска. Главная дополнительная
выборка содержит 66 найденных биомов и 28 найденных целей построек, без ошибок
анализа. Из них 52 поверхностных биома, 4 подземных, 5 биомов Незера и 5 Энда.
Искомый поверхностный биом присутствует у нас только в трёх окнах:
`ocean`, `lukewarm_ocean`, `desert`; у оригинала во всех 52.
Число считается по сохранённым биомам во всём объёме окна, поэтому присутствие
биома не означает совпадения его поверхности. Отдельная дополнительная выборка
закрывает все 18 вариантов заброшенных лагерей и шахту в бедлендах: 19/19,
без ошибок анализа. В главном плане первоначально не нашлись две цели:
`mineshaft_mesa` и несуществующий общий идентификатор `abandoned_camp`.
Вторая выборка исправляет именно выбор целей, а не код генератора.

Всего 115 сравнений и 24 165 строк вне допуска. Все они, с числами и допуском,
сохранены в `testdata/world-profile/genaudit-26.3/discrepancies.json`.
Это компактная таблица: описание столбцов находится в начале файла,
названия показателей вынесены в общий справочник `metrics`, каждая строка
ссылается на его индекс. Случаи сравнения не смешиваются, семя записано
у каждого. Порядок строк внутри случая соответствует полному ранжированному
отчёту. Полные JSON и Markdown в `reports/` содержат также строки внутри допуска,
точные гистограммы, знаменатели, примеры координат, состав рамок построек,
хеши регионов и ограничения. Малые `owner.json` и `second.json` сохраняют
исходные сводные числа для будущих проверок, `provenance.json` — происхождение.

Пересборка компактного списка из сохранённых полных отчётов:

```sh
python3 tools/measure/genaudit_263_catalog.py "$AUDIT_DIR/reports" \
  --output testdata/world-profile/genaudit-26.3/discrepancies.json
```

Метрики охватывают каждый сохранённый вид блока, доступные биомы, верх и пять
нижних слоёв, номинальные группы климата, площади пятен, берега, речную ширину
по биомной сетке, высоты и уклоны, пещеры, жидкости и их границы, руды по Y,
сырые рудные блоки, деревья и растения, поверхности и геометрию ледяных,
костяных, аметистовых и рудных скоплений. Содержимое всех найденных типов
построек сравнивается доступными рамками частей либо составом стартовых чанков.
Лагеря, poplar, leaf_litter, sulfur_caves и другие сохранённые блоки/биомы 26.3
не исключены из переписи.

Это не доказательство проверки абсолютно каждого приёма генерации. Скрытые
климатические шумы из регионов недоступны; используются номинальные группы
биомов. Ширина реки — приближение по её биому. Связные части пещер обрезаны
чанками и не дают точных названий cheese/spaghetti/noodle. Истинное число
деревьев, упавших деревьев, шпилей, валунов, айсбергов, озёр, жил, геод,
данжей и ископаемых нельзя полностью восстановить из одних блоков: для них
записаны счётчики материала и доступные геометрические приближения.
Не все наши постройки имеют доступные рамки; число подходящих начал не равно
числу фактически поставленных объектов. Тиковые изменения и переходы между
измерениями здесь не измерялись. Отсутствие объекта в конечной выборке
не считается доказательством отсутствия реализации.

Большинство общих количеств руд в основном мире владельца в допуске 20%:
железная руда 42,0323 / 48,3459 блока на чанк, глубинная железная
30,0226 / 28,6992, глубинная алмазная 22,4106 / 23,5759.
Границы лавы p50 Y = −55 / −55. На втором семени есть отдельные отклонения
по рудным слоям; полный список не скрывается за общим совпадением.
В Незере второго семени золотая руда 46,2994 / 30,8476 блока на чанк,
древние обломки 1,6529 / 1,3737; долина душ 0% / 64,9392%.
В западном внешнем окне Энда второго семени эндстоун
3602,1919 / 4576,7585 блока на чанк. Это дополнительные задачи N и E.

## Пакеты без одновременной правки одних файлов

Время — грубая оценка часов работы Codex на исправление и повторное измерение,
не обещание срока. Пакеты можно запускать параллельно после согласования
общих интерфейсов; изменения общих файлов проводить отдельно при интеграции.
Сначала A: он может устранить часть вторичных отличий F, T, O и R.

| Пакет | Содержимое и собственные файлы (предположения) | Часы |
|---|---|---:|
| A | Биомы, климат, рельеф, берег, поверхность, шпили, спавн: terrain.rs, biome_distribution.rs, noise.rs, spawn_search.rs | 12–24 |
| U | Пещеры, аквиферы, руды: underground.rs, underground_carvers.rs, cave_biomes.rs, cave_choice.rs, sulfur_caves.rs, underground_dungeons.rs | 8–16 |
| T | Деревья, мангры: trees.rs, tree_shapes_26_3.rs | 4–8 |
| F | Наземная растительность и опад: vegetation.rs | 6–12 |
| O | Морские растения и кораллы: ocean.rs | 3–6 |
| R | Постройки обычного мира: mineshaft.rs, mineshaft_start.rs, ancient_city.rs, village.rs, temples.rs, stronghold.rs, trial_chambers.rs, shipwreck.rs, ocean_ruin.rs, abandoned_camp.rs, template.rs и остальные модули этого измерения | 12–24; отдельный модуль 3–8 |
| N | Незер целиком: nether.rs, nether_fortress.rs, nether_bastion.rs, nether_fossil.rs | 6–12 |
| E | Энд: the_end.rs, end_city.rs | 3–6 |

R не должен править модули Незера/Энда; их автоматические подпакеты в сырых
отчётах следует переназначить N/E. Храмы и их варианты остаются в одном R,
деревни со вспомогательными файлами — также. Общие `mod.rs`, `structure_terrain.rs`
и таблицы блоков зарезервировать для последовательной интеграции.
Одинаковое имя `vegetation.rs` существует в нескольких местах: F владеет
обоими, если причина требует этого; T не должен править эти файлы параллельно.
Пакеты A/F пересекаются с уже работающими задачами владельца и требуют
последовательного объединения. Исходники в этой переписи не менялись.

## Проверки выполнения

`cargo build --release` завершился успешно, 6 минут 7 секунд, с
`CARGO_BUILD_JOBS=4`, после ожидания отсутствия других cargo. Повторных сборок
или тестов Rust не было. Семь Python-тестов чтения регионов, битовых палитр,
координат, семян, контрольных сумм, рамок и размещения прошли. Полные сравнения
и повторный запуск дополнительных отчётов завершились без ошибок.
Исходные серверы остановлены штатно; работающих Java/наших серверов при
возобновлении не найдено. Коммитов и записей в журнал передачи нет.

## Дополнительные окна: сводка

Каждое окно независимо. В таблице указан первый показатель из машинного
рейтинга, а не диагноз причины. Все остальные строки этого окна находятся
в компактной таблице и полном отчёте по указанному имени.

| Отчёт в reports/ | Семя | Строк вне допуска | Первый показатель | У нас / оригинал |
|---|---:|---:|---|---|
| targets/biome-ocean | 1792932773345722119 | 290 | overworld: patches/ocean/p50 (blocks²) | 9808 / 16 |
| targets/biome-deep_ocean | 1792932773345722119 | 368 | overworld: blocks/sculk (per chunk) | 41.9614 / 0 |
| targets/biome-cold_ocean | 1792932773345722119 | 340 | overworld: blocks/terracotta (per chunk) | 1084.12 / 0 |
| targets/biome-deep_cold_ocean | 1792932773345722119 | 262 | overworld: blocks/cherry_leaves (per chunk) | 82.7337 / 0 |
| targets/biome-frozen_ocean | 1792932773345722119 | 281 | overworld: blocks/terracotta (per chunk) | 69.4031 / 0 |
| targets/biome-deep_frozen_ocean | 1792932773345722119 | 182 | overworld: slope/p100 (blocks per block) | 14 / 93 |
| targets/biome-lukewarm_ocean | 1792932773345722119 | 261 | overworld: blocks/kelp_plant (per chunk) | 133.529 / 0.669421 |
| targets/biome-deep_lukewarm_ocean | 1792932773345722119 | 279 | overworld: blocks/sulfur (per chunk) | 63.3756 / 0 |
| targets/biome-warm_ocean | 1792932773345722119 | 292 | overworld: blocks/terracotta (per chunk) | 181.57 / 0 |
| targets/biome-mushroom_fields | 1792932773345722119 | 183 | overworld: blocks/sandstone (per chunk) | 712.691 / 1.87654 |
| targets/biome-beach | 1792932773345722119 | 142 | overworld: blocks/prismarine (per chunk) | 33.9753 / 0 |
| targets/biome-snowy_beach | 1792932773345722119 | 141 | overworld: blocks/sulfur (per chunk) | 119.407 / 0 |
| targets/biome-stony_shore | 1792932773345722119 | 121 | overworld: blocks/sulfur (per chunk) | 442.506 / 0 |
| targets/biome-river | 1792932773345722119 | 144 | overworld: blocks/kelp_plant (per chunk) | 226.864 / 0 |
| targets/biome-frozen_river | 1792932773345722119 | 135 | overworld: blocks/sulfur (per chunk) | 195.802 / 0 |
| targets/biome-swamp | 1792932773345722119 | 189 | overworld: blocks/moss_block (per chunk) | 270.926 / 0 |
| targets/biome-mangrove_swamp | 1792932773345722119 | 173 | overworld: blocks/moss_block (per chunk) | 311.802 / 0 |
| targets/biome-plains | 1792932773345722119 | 185 | overworld: blocks/moss_block (per chunk) | 313.753 / 0 |
| targets/biome-sunflower_plains | 1792932773345722119 | 202 | overworld: blocks/sulfur (per chunk) | 183.358 / 0 |
| targets/biome-snowy_plains | 1792932773345722119 | 142 | overworld: blocks/sulfur (per chunk) | 36.321 / 0 |
| targets/biome-ice_spikes | 1792932773345722119 | 196 | overworld: blocks/oak_leaves (per chunk) | 73.8927 / 0.557093 |
| targets/biome-forest | 1792932773345722119 | 182 | overworld: blocks/kelp_plant (per chunk) | 220.568 / 0 |
| targets/biome-flower_forest | 1792932773345722119 | 178 | overworld: blocks/sandstone (per chunk) | 174.778 / 0.901235 |
| targets/biome-birch_forest | 1792932773345722119 | 145 | overworld: biomes_3d/warm_ocean (percentage points) | 100 / 0 |
| targets/biome-old_growth_birch_forest | 1792932773345722119 | 180 | overworld: blocks/sand (per chunk) | 229.593 / 1.71605 |
| targets/biome-dark_forest | 1792932773345722119 | 198 | overworld: blocks/sulfur (per chunk) | 390.654 / 0 |
| targets/biome-taiga | 1792932773345722119 | 184 | overworld: blocks/tuff_bricks (per chunk) | 64.4198 / 0 |
| targets/biome-snowy_taiga | 1792932773345722119 | 177 | overworld: blocks/sulfur (per chunk) | 56.9506 / 0 |
| targets/biome-old_growth_pine_taiga | 1792932773345722119 | 164 | overworld: blocks/sulfur (per chunk) | 326.815 / 0 |
| targets/biome-old_growth_spruce_taiga | 1792932773345722119 | 177 | overworld: blocks/sulfur (per chunk) | 171.667 / 0 |
| targets/biome-jungle | 1792932773345722119 | 196 | overworld: blocks/tube_coral_block (per chunk) | 16.7778 / 0 |
| targets/biome-sparse_jungle | 1792932773345722119 | 162 | overworld: blocks/sand (per chunk) | 230.938 / 0.222222 |
| targets/biome-bamboo_jungle | 1792932773345722119 | 188 | overworld: blocks/dirt_path (per chunk) | 20.3086 / 0 |
| targets/biome-savanna | 1792932773345722119 | 166 | overworld: blocks/kelp_plant (per chunk) | 249.704 / 0 |
| targets/biome-desert | 1792932773345722119 | 206 | overworld: blocks/sulfur (per chunk) | 196.309 / 0 |
| targets/biome-meadow | 1792932773345722119 | 169 | overworld: blocks/vine (per chunk) | 19.9753 / 0.061728 |
| targets/biome-cherry_grove | 1792932773345722119 | 174 | overworld: blocks/kelp_plant (per chunk) | 86.2099 / 0 |
| targets/biome-savanna_plateau | 1792932773345722119 | 145 | overworld: blocks/sulfur (per chunk) | 216.543 / 0 |
| targets/biome-grove | 1792932773345722119 | 209 | overworld: nominal_climate_columns/[0.8, 0.4] (percentage points) | 88.6574 / 0 |
| targets/biome-snowy_slopes | 1792932773345722119 | 171 | overworld: blocks/sand (per chunk) | 42.7407 / 0.308642 |
| targets/biome-jagged_peaks | 1792932773345722119 | 211 | overworld: blocks/oak_leaves (per chunk) | 255.457 / 0 |
| targets/biome-frozen_peaks | 1792932773345722119 | 197 | overworld: blocks/sand (per chunk) | 879.444 / 0.246914 |
| targets/biome-stony_peaks | 1792932773345722119 | 183 | overworld: blocks/sulfur (per chunk) | 830.951 / 0 |
| targets/biome-windswept_hills | 1792932773345722119 | 185 | overworld: blocks/moss_block (per chunk) | 228.506 / 0 |
| targets/biome-windswept_gravelly_hills | 1792932773345722119 | 224 | overworld: blocks/tuff_bricks (per chunk) | 133.321 / 0 |
| targets/biome-windswept_forest | 1792932773345722119 | 181 | overworld: blocks/sand (per chunk) | 231.593 / 0.222222 |
| targets/biome-windswept_savanna | 1792932773345722119 | 207 | overworld: nominal_climate_columns/[0.5, 0.5] (percentage points) | 100 / 0.154321 |
| targets/biome-badlands | 1792932773345722119 | 222 | overworld: blocks/sand (per chunk) | 912.272 / 0 |
| targets/biome-eroded_badlands | 1792932773345722119 | 296 | overworld: blocks/sandstone (per chunk) | 419.561 / 0 |
| targets/biome-wooded_badlands | 1792932773345722119 | 246 | overworld: blocks/brain_coral_block (per chunk) | 23.7778 / 0 |
| targets/biome-pale_garden | 1792932773345722119 | 190 | overworld: blocks/sulfur (per chunk) | 320.198 / 0 |
| targets/biome-lush_caves | 1792932773345722119 | 178 | overworld: blocks/sulfur (per chunk) | 89.679 / 0 |
| targets/biome-dripstone_caves | 1792932773345722119 | 182 | overworld: blocks/kelp_plant (per chunk) | 220.568 / 0 |
| targets/biome-deep_dark | 1792932773345722119 | 136 | overworld: blocks/sulfur (per chunk) | 325.198 / 0 |
| targets/biome-dappled_forest | 1792932773345722119 | 193 | overworld: blocks/birch_leaves (per chunk) | 89.6667 / 1.37037 |
| targets/biome-sulfur_caves | 1792932773345722119 | 231 | overworld: blocks/moss_block (per chunk) | 294.716 / 0 |
| targets/biome-nether_wastes | 1792932773345722119 | 60 | the_nether: blocks/soul_soil (per chunk) | 889.457 / 0 |
| targets/biome-crimson_forest | 1792932773345722119 | 55 | the_nether: blocks/soul_soil (per chunk) | 364.605 / 0 |
| targets/biome-warped_forest | 1792932773345722119 | 91 | the_nether: blocks/nether_wart_block (per chunk) | 270.593 / 0 |
| targets/biome-soul_sand_valley | 1792932773345722119 | 68 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/biome-basalt_deltas | 1792932773345722119 | 64 | the_nether: blocks/soul_soil (per chunk) | 1383.06 / 0 |
| targets/biome-the_end | 1792932773345722119 | 29 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/biome-small_end_islands | 1792932773345722119 | 77 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/biome-end_midlands | 1792932773345722119 | 69 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/biome-end_highlands | 1792932773345722119 | 61 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/biome-end_barrens | 1792932773345722119 | 66 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/structure-village_plains | 1792932773345722119 | 175 | overworld: blocks/sandstone (per chunk) | 91.2561 / 0.00346 |
| targets/structure-village_desert | 1792932773345722119 | 234 | overworld: blocks/short_grass (per chunk) | 28.6851 / 0.044983 |
| targets/structure-village_savanna | 1792932773345722119 | 155 | overworld: blocks/kelp_plant (per chunk) | 130.862 / 0 |
| targets/structure-village_snowy | 1792932773345722119 | 171 | overworld: blocks/oak_leaves (per chunk) | 82.7751 / 0 |
| targets/structure-village_taiga | 1792932773345722119 | 310 | overworld: blocks/sculk (per chunk) | 57.3979 / 0 |
| targets/structure-pillager_outpost | 1792932773345722119 | 180 | overworld: blocks/moss_block (per chunk) | 79.6159 / 0 |
| targets/structure-desert_pyramid | 1792932773345722119 | 211 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/structure-jungle_pyramid | 1792932773345722119 | 209 | overworld: blocks/sulfur (per chunk) | 209.187 / 0 |
| targets/structure-swamp_hut | 1792932773345722119 | 214 | overworld: blocks/sandstone (per chunk) | 29.9031 / 0 |
| targets/structure-igloo | 1792932773345722119 | 246 | overworld: blocks/sandstone (per chunk) | 292.872 / 0 |
| targets/structure-shipwreck | 1792932773345722119 | 233 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/structure-shipwreck_beached | 1792932773345722119 | 167 | overworld: blocks/tuff_bricks (per chunk) | 140.588 / 0 |
| targets/structure-ocean_ruin_cold | 1792932773345722119 | 227 | overworld: surface_biomes/lukewarm_ocean (percentage points) | 85.4022 / 0 |
| targets/structure-ocean_ruin_warm | 1792932773345722119 | 243 | overworld: patches/lukewarm_ocean/p50 (blocks²) | 16 / 25264 |
| targets/structure-monument | 1792932773345722119 | 150 | overworld: blocks/sandstone (per chunk) | 410.197 / 0.705882 |
| targets/structure-mansion | 1792932773345722119 | 280 | overworld: blocks/sulfur (per chunk) | 476.917 / 0 |
| targets/structure-ancient_city | 1792932773345722119 | 252 | overworld: blocks/oak_leaves (per chunk) | 194.799 / 1.03114 |
| targets/structure-trail_ruins | 1792932773345722119 | 181 | overworld: blocks/tube_coral_block (per chunk) | 25.4775 / 0 |
| targets/structure-trial_chambers | 1792932773345722119 | 208 | overworld: blocks/moss_block (per chunk) | 197.121 / 0 |
| targets/structure-mineshaft | 1792932773345722119 | 203 | overworld: blocks/moss_block (per chunk) | 230.623 / 0 |
| targets/structure-stronghold | 1792932773345722119 | 294 | overworld: blocks/terracotta (per chunk) | 267.048 / 0 |
| targets/structure-buried_treasure | 1792932773345722119 | 276 | overworld: blocks/sculk (per chunk) | 25.91 / 0 |
| targets/structure-ruined_portal | 1792932773345722119 | 159 | overworld: surface_biomes/warm_ocean (percentage points) | 99.6756 / 0 |
| targets/structure-fortress | 1792932773345722119 | 102 | the_nether: blocks/soul_soil (per chunk) | 732.608 / 0 |
| targets/structure-bastion_remnant | 1792932773345722119 | 102 | the_nether: blocks/nether_bricks (per chunk) | 85.609 / 0.017301 |
| targets/structure-nether_fossil | 1792932773345722119 | 80 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| targets/structure-ruined_portal_nether | 1792932773345722119 | 99 | the_nether: blocks/nether_bricks (per chunk) | 130.01 / 0 |
| targets/structure-end_city | 1792932773345722119 | 104 | overworld: spawn_offset (blocks from official spawn) | 562.736 / 0 |
| camps/structure-abandoned_camp_bamboo_jungle | 1792939653127071019 | 273 | overworld: blocks/sandstone (per chunk) | 180.298 / 0 |
| camps/structure-abandoned_camp_sparse_jungle | 1792939653127071019 | 183 | overworld: blocks/packed_ice (per chunk) | 1120.24 / 0 |
| camps/structure-abandoned_camp_forest | 1792939653127071019 | 204 | overworld: blocks/moss_block (per chunk) | 351.478 / 0 |
| camps/structure-abandoned_camp_flower_forest | 1792939653127071019 | 198 | overworld: slope/p100 (blocks per block) | 20 / 77 |
| camps/structure-abandoned_camp_birch_forest | 1792939653127071019 | 217 | overworld: blocks/sandstone (per chunk) | 214.713 / 0.013841 |
| camps/structure-abandoned_camp_old_growth_birch_forest | 1792939653127071019 | 254 | overworld: blocks/jungle_leaves (per chunk) | 31.8927 / 0 |
| camps/structure-abandoned_camp_pale_garden | 1792939653127071019 | 267 | overworld: blocks/sandstone (per chunk) | 89.391 / 0 |
| camps/structure-abandoned_camp_taiga | 1792939653127071019 | 184 | overworld: blocks/oak_leaves (per chunk) | 268.443 / 2.83391 |
| camps/structure-abandoned_camp_snowy_taiga | 1792939653127071019 | 167 | overworld: blocks/oak_leaves (per chunk) | 122.26 / 0 |
| camps/structure-abandoned_camp_old_growth_pine_taiga | 1792939653127071019 | 218 | overworld: blocks/tall_seagrass (per chunk) | 40.0623 / 0.858131 |
| camps/structure-abandoned_camp_old_growth_spruce_taiga | 1792939653127071019 | 246 | overworld: blocks/terracotta (per chunk) | 509.803 / 0 |
| camps/structure-abandoned_camp_meadow | 1792939653127071019 | 281 | overworld: blocks/sandstone (per chunk) | 850.336 / 0 |
| camps/structure-abandoned_camp_cherry_grove | 1792939653127071019 | 304 | overworld: blocks/sandstone (per chunk) | 539.221 / 0 |
| camps/structure-abandoned_camp_savanna | 1792939653127071019 | 283 | overworld: blocks/terracotta (per chunk) | 462.927 / 0 |
| camps/structure-abandoned_camp_windswept_forest | 1792939653127071019 | 279 | overworld: blocks/tuff_bricks (per chunk) | 150.246 / 0 |
| camps/structure-abandoned_camp_swamp | 1792939653127071019 | 274 | overworld: blocks/sandstone (per chunk) | 129.931 / 0.00346 |
| camps/structure-abandoned_camp_wooded_badlands | 1792939653127071019 | 277 | overworld: blocks/sand (per chunk) | 184.111 / 0 |
| camps/structure-abandoned_camp_dappled_forest | 1792939653127071019 | 215 | overworld: blocks/sandstone (per chunk) | 791.851 / 0.044983 |
| camps/structure-mineshaft_mesa | 1792939653127071019 | 361 | overworld: blocks/sulfur (per chunk) | 190.266 / 0 |

## Основные квадраты: полный список отклонений

Все строки ниже выходят за допуск. Это полный машинный список по двум
основным семенам, без дополнительного отбора по величине. У каждого семени
свой порядок. Единицы, знаменатели и координаты — в `discrepancies.json`
и соответствующем полном `reports/owner.json` или `reports/second.json`.
Процентные допуски указаны в процентных пунктах. Строки с малыми
знаменателями и строки, обусловленные отсутствующим биомом, требуют
осторожного разбора; одна строка не равна отдельному дефекту.

| Семя / измерение | Показатель | У нас | Оригинал | Допуск | Пакет / файл (догадка) |
|---|---|---:|---:|---:|---|
| owner / overworld | blocks/terracotta | 40.2169 | 0 | 0.1 | A / terrain.rs |
| owner / overworld | patches/mangrove_swamp/p50 | 16608 | 32 | 64 | A / terrain.rs |
| owner / overworld | slope/p100 | 50 | 215 | 1 | A / terrain.rs |
| owner / overworld | patches/deep_ocean/p50 | 16 | 12448 | 64 | A / terrain.rs |
| owner / overworld | patches/windswept_hills/p50 | 520 | 10312 | 64 | A / terrain.rs |
| owner / overworld | spawn_offset | 562.736 | 0 | 16 | A / spawn_search.rs |
| owner / overworld | patches/desert/p50 | 16 | 5024 | 64 | A / terrain.rs |
| owner / overworld | object_geometry_proxy/ice_body/volume_p95 | 924.85 | 1 | 4 | F / vegetation.rs |
| owner / overworld | blocks/sculk | 10.794 | 0.442347 | 0.1 | U / underground_carvers.rs |
| owner / the_nether | blocks/polished_blackstone_bricks | 7.88246 | 0 | 0.1 | N / nether.rs |
| owner / overworld | patches/sparse_jungle/p50 | 24 | 3296 | 64 | A / terrain.rs |
| owner / overworld | blocks/orange_terracotta | 6.67841 | 0.005933 | 0.1 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/monument | 2123.48 | 0 | 32 | R-monument / monument.rs |
| owner / overworld | blocks/brown_terracotta | 6.45225 | 0 | 0.1 | A / terrain.rs |
| owner / overworld | blocks/packed_ice | 6.11581 | 0.00451 | 0.1 | A / terrain.rs |
| owner / overworld | blocks/red_terracotta | 5.615 | 0.009289 | 0.1 | A / terrain.rs |
| owner / the_nether | biomes_3d/soul_sand_valley | 40.1343 | 13.8659 | 1 | N / nether.rs |
| owner / the_nether | surface_biomes/soul_sand_valley | 40.1343 | 13.8659 | 1 | N / nether.rs |
| owner / overworld | feature_biome/stony_shore/snow | 517.102 | 0 | 5 | A / terrain.rs |
| owner / overworld | patches/stony_shore/p50 | 2136 | 16 | 64 | A / terrain.rs |
| owner / overworld | structure_candidate_spacing_p50/buried_treasure | 1476.95 | 92.3456 | 32 | R-shipwreck / shipwreck.rs |
| owner / overworld | blocks/sculk_vein | 4.28497 | 0.145668 | 0.1 | U / underground_carvers.rs |
| owner / overworld | patches/windswept_savanna/p50 | 1408 | 3168 | 64 | A / terrain.rs |
| owner / overworld | depth/128/underground_air | 11.332 | 65.3478 | 2 | U / underground_carvers.rs |
| owner / overworld | structure_candidate_offset_p95/abandoned_camp | 1293.57 | 0 | 32 | R-abandoned_camp / abandoned_camp.rs |
| owner / overworld | structure_candidate_offset_p95/stronghold | 1292.44 | 0 | 32 | R-stronghold / stronghold.rs |
| owner / the_nether | biomes_3d/basalt_deltas | 0 | 19.961 | 1 | N / nether.rs |
| owner / the_nether | nominal_climate_columns/[2.0, 0.0] | 0 | 19.961 | 1 | N / nether.rs |
| owner / the_nether | nominal_climate_columns/unknown | 100 | 80.039 | 1 | N / nether.rs |
| owner / the_nether | surface_biomes/basalt_deltas | 0 | 19.961 | 1 | N / nether.rs |
| owner / overworld | structure_candidate_offset_p95/buried_treasure | 1212.37 | 0 | 32 | R-shipwreck / shipwreck.rs |
| owner / overworld | height_biome/windswept_savanna/p100 | 86 | 210 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/trail_ruins | 1173.58 | 0 | 32 | R-trail_ruins / trail_ruins.rs |
| owner / the_end | slope/p100 | 118 | 100 | 1 | E / the_end.rs |
| owner / overworld | feature_biome/bamboo_jungle/azalea_leaves | 343.517 | 2.79655 | 5 | F-tree / trees.rs |
| owner / overworld | patches/bamboo_jungle/p50 | 1424 | 32 | 64 | A / terrain.rs |
| owner / the_nether | slope/p100 | 73 | 89 | 1 | N / nether.rs |
| owner / overworld | structure_candidate_offset_p95/village | 1004.18 | 0 | 32 | R-village / village.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/azalea_leaves | 308.719 | 0.5234 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/white_terracotta | 3.04518 | 0.0149 | 0.1 | A / terrain.rs |
| owner / overworld | structure_candidate_coincident/abandoned_camp | 0 | 100 | 5 | R-abandoned_camp / abandoned_camp.rs |
| owner / overworld | structure_candidate_coincident/ancient_city | 0 | 100 | 5 | R-ancient_city / ancient_city.rs |
| owner / the_nether | structure_candidate_coincident/bastion_remnant | 0 | 100 | 5 | N / nether_bastion.rs |
| owner / overworld | structure_candidate_coincident/buried_treasure | 0 | 100 | 5 | R-shipwreck / shipwreck.rs |
| owner / overworld | structure_candidate_coincident/desert_pyramid | 0 | 100 | 5 | R-temples / temples.rs |
| owner / the_end | structure_candidate_coincident/end_city | 0 | 100 | 5 | E / end_city.rs |
| owner / overworld | structure_candidate_coincident/igloo | 0 | 100 | 5 | R-temples / temples.rs |
| owner / overworld | structure_candidate_coincident/mineshaft | 0 | 100 | 5 | R-mineshaft / mineshaft.rs |
| owner / overworld | structure_candidate_coincident/monument | 0 | 100 | 5 | R-monument / monument.rs |
| owner / overworld | structure_candidate_coincident/ocean_ruin | 0 | 100 | 5 | R-ocean_ruin / ocean_ruin.rs |
| owner / overworld | structure_candidate_coincident/pillager_outpost | 0 | 100 | 5 | R-pillager_outpost / pillager_outpost.rs |
| owner / overworld | structure_candidate_coincident/ruined_portal | 0 | 100 | 5 | R-ruined_portal / ruined_portal.rs |
| owner / overworld | structure_candidate_coincident/shipwreck | 0 | 100 | 5 | R-shipwreck / shipwreck.rs |
| owner / overworld | structure_candidate_coincident/stronghold | 0 | 100 | 5 | R-stronghold / stronghold.rs |
| owner / overworld | structure_candidate_coincident/trail_ruins | 0 | 100 | 5 | R-trail_ruins / trail_ruins.rs |
| owner / overworld | structure_candidate_coincident/trial_chambers | 0 | 100 | 5 | R-trial_chambers / trial_chambers.rs |
| owner / overworld | structure_candidate_coincident/village | 0 | 100 | 5 | R-village / village.rs |
| owner / overworld | nominal_climate_columns/[0.5, 0.5] | 49.7204 | 34.7675 | 1 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/shipwreck | 938.913 | 0 | 32 | R-shipwreck / shipwreck.rs |
| owner / the_nether | blocks/cracked_polished_blackstone_bricks | 2.87328 | 0 | 0.1 | N / nether.rs |
| owner / overworld | blocks/yellow_terracotta | 2.84926 | 0.013772 | 0.1 | A / terrain.rs |
| owner / the_nether | structure_candidate_coincident/nether_fossil | 7.40741 | 100 | 5 | N / nether_fossil.rs |
| owner / overworld | surface_biomes/desert | 14.7737 | 1.06073 | 1 | A / terrain.rs |
| owner / overworld | biomes_3d/desert | 14.5204 | 1.02648 | 1 | A / terrain.rs |
| owner / overworld | ocean_coast_distance/warm_ocean/p50 | 395.353 | 107.629 | 16 | A / terrain.rs |
| owner / overworld | patches/birch_forest/p50 | 1168 | 32 | 64 | A / terrain.rs |
| owner / overworld | depth/160/underground_air | 42.6078 | 7.62306 | 2 | U / underground_carvers.rs |
| owner / overworld | structure_candidate_offset_p95/ocean_ruin | 832.612 | 0 | 32 | R-ocean_ruin / ocean_ruin.rs |
| owner / overworld | height/p100 | 169 | 210 | 4 | A / terrain.rs |
| owner / the_end | feature_biome/end_midlands/chorus_plant | 267.111 | 15.801 | 5 | E / the_end.rs |
| owner / overworld | ocean_coast_distance/deep_ocean/p50 | 374.977 | 108.995 | 16 | A / terrain.rs |
| owner / overworld | patches/jungle/p50 | 1120 | 64 | 64 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/pillager_outpost | 776.124 | 0 | 32 | R-pillager_outpost / pillager_outpost.rs |
| owner / overworld | blocks/brain_coral_block | 2.73452 | 0.374158 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | layer/desert/-4/sand | 88.3832 | 10.3273 | 5 | A / terrain.rs |
| owner / overworld | blocks/tube_coral_block | 2.69382 | 0.362667 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | depth/144/underground_air | 13.9704 | 44.9468 | 2 | U / underground_carvers.rs |
| owner / overworld | layer/desert/-4/sandstone | 0.468347 | 77.6844 | 5 | A / terrain.rs |
| owner / overworld | layer/windswept_hills/-3/dirt | 96.7005 | 20.0918 | 5 | A / terrain.rs |
| owner / overworld | blocks/fire_coral_block | 2.65433 | 0.372225 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | height_biome/stony_shore/p100 | 155 | 79 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_hills/spruce_leaves | 284.211 | 28.0888 | 5.61776 | F-tree / trees.rs |
| owner / overworld | blocks/horn_coral_block | 2.61803 | 0.359473 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/bubble_coral_block | 2.61913 | 0.370292 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | ocean_coast_distance/deep_lukewarm_ocean/p50 | 458.345 | 221.269 | 16 | A / terrain.rs |
| owner / overworld | height_biome/windswept_savanna/p95 | 68 | 141 | 5 | A / terrain.rs |
| owner / overworld | layer/windswept_savanna/-3/dirt | 90.4376 | 19.5138 | 5 | A / terrain.rs |
| owner / overworld | blocks/spruce_leaves | 2.16054 | 0.060888 | 0.1 | F-tree / trees.rs |
| owner / the_end | blocks/purpur_block | 2.08919 | 0 | 0.1 | E / the_end.rs |
| owner / overworld | layer/windswept_hills/-3/stone | 2.60152 | 71.7166 | 5 | A / terrain.rs |
| owner / overworld | nominal_climate_columns/[2.0, 0.0] | 24.5753 | 14.5845 | 1 | A / terrain.rs |
| owner / overworld | height_biome/desert/p100 | 166 | 101 | 5 | A / terrain.rs |
| owner / the_end | patches/end_highlands/p50 | 192 | 1024 | 64 | E / the_end.rs |
| owner / overworld | layer/beach/-4/sand | 74.8686 | 10.4455 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/warm_ocean | 13.093 | 3.61761 | 1 | A / terrain.rs |
| owner / overworld | height/p95 | 115 | 85 | 4 | A / terrain.rs |
| owner / overworld | height/p99 | 140 | 110 | 4 | A / terrain.rs |
| owner / overworld | height_biome/sparse_jungle/p95 | 156 | 94 | 5 | A / terrain.rs |
| owner / overworld | height_biome/stony_shore/p95 | 134 | 73 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/warm_ocean | 12.7327 | 3.59066 | 1 | A / terrain.rs |
| owner / overworld | feature_biome/swamp/spruce_leaves | 180.845 | 4.34502 | 5 | F-tree / trees.rs |
| owner / overworld | layer/beach/-4/sandstone | 0.402318 | 58.8159 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/plains/oak_leaves | 436.123 | 54.6422 | 10.9284 | F-tree / trees.rs |
| owner / overworld | surface_band/badlands/terracotta/y112 | 8.56514 | 0 | 0.25 | A / terrain.rs |
| owner / the_end | height_biome/small_end_islands/p95 | -1 | 56 | 5 | E / the_end.rs |
| owner / overworld | layer/windswept_savanna/-3/stone | 6.7216 | 63.4786 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/forest | 7.66262 | 16.0255 | 1 | A / terrain.rs |
| owner / overworld | feature_biome/bamboo_jungle/flowering_azalea_leaves | 165.542 | 0.843078 | 5 | F-tree / trees.rs |
| owner / overworld | biomes_3d/forest | 7.38127 | 15.4362 | 1 | A / terrain.rs |
| owner / overworld | height_biome/plains/p95 | 131 | 78 | 5 | A / terrain.rs |
| owner / overworld | height_biome/windswept_hills/p100 | 85 | 137 | 5 | A / terrain.rs |
| owner / overworld | ore/coal_ore/y96 | 4.8146 | 0.920884 | 0.25 | U / underground.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/flowering_azalea_leaves | 151.274 | 0.157737 | 5 | F-tree / trees.rs |
| owner / overworld | top/meadow/grass_block | 48.1657 | 98.457 | 5 | A / terrain.rs |
| owner / overworld | patches/deep_lukewarm_ocean/p50 | 672 | 32 | 64 | A / terrain.rs |
| owner / overworld | patches/savanna_plateau/p50 | 336 | 976 | 64 | A / terrain.rs |
| owner / overworld | height_mean/stony_shore | 95.1421 | 57.9175 | 5 | A / terrain.rs |
| owner / overworld | height_biome/swamp/p100 | 66 | 115 | 5 | A / terrain.rs |
| owner / overworld | blocks/gray_wool | 1.44463 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | layer/windswept_hills/-2/dirt | 96.4784 | 49.2472 | 5 | A / terrain.rs |
| owner / overworld | blocks/red_sand | 1.41134 | 0 | 0.1 | A / terrain.rs |
| owner / overworld | blocks/deepslate_bricks | 1.40941 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | top/meadow/stone | 48.4517 | 1.49626 | 5 | A / terrain.rs |
| owner / overworld | layer/desert/-3/sand | 94.839 | 48.0726 | 5 | A / terrain.rs |
| owner / overworld | layer/beach/-3/sand | 88.8577 | 42.7652 | 5 | A / terrain.rs |
| owner / overworld | height_biome/mangrove_swamp/p100 | 67 | 113 | 5 | A / terrain.rs |
| owner / overworld | layer/meadow/-1/dirt | 49.0994 | 94.7163 | 5 | A / terrain.rs |
| owner / overworld | top/savanna_plateau/grass_block | 53.5986 | 98.6453 | 5 | A / terrain.rs |
| owner / overworld | nominal_climate_columns/[0.7, 0.8] | 11.3735 | 18.1298 | 1 | A / terrain.rs |
| owner / overworld | height_biome/windswept_hills/p95 | 71 | 116 | 5 | A / terrain.rs |
| owner / overworld | blocks/light_gray_terracotta | 1.33969 | 0.002201 | 0.1 | A / terrain.rs |
| owner / overworld | layer/meadow/-1/stone | 48.1302 | 3.86534 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/savanna | 6.31779 | 12.9537 | 1 | A / terrain.rs |
| owner / overworld | layer/old_growth_birch_forest/-3/dirt | 96.9332 | 52.772 | 5 | A / terrain.rs |
| owner / overworld | nominal_climate_columns/[0.95, 0.9] | 0.404531 | 6.92649 | 1 | A / terrain.rs |
| owner / overworld | layer/bamboo_jungle/-3/dirt | 97.907 | 55.0097 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/savanna | 6.24917 | 12.6519 | 1 | A / terrain.rs |
| owner / the_end | patches/small_end_islands/p50 | 96 | 640 | 64 | E / the_end.rs |
| owner / overworld | top/bamboo_jungle/grass_block | 95.1832 | 52.7788 | 5 | A / terrain.rs |
| owner / overworld | layer/windswept_hills/-2/stone | 2.66498 | 45.0117 | 5 | A / terrain.rs |
| owner / overworld | layer/savanna_plateau/-1/dirt | 55.0608 | 97.3106 | 5 | A / terrain.rs |
| owner / overworld | layer/jungle/-3/dirt | 96.2434 | 54.0677 | 5 | A / terrain.rs |
| owner / overworld | top/river/grass_block | 49.4963 | 7.94462 | 5 | A / terrain.rs |
| owner / overworld | patches/lukewarm_ocean/p50 | 16 | 544 | 64 | A / terrain.rs |
| owner / overworld | top/savanna_plateau/stone | 41.5825 | 1.03501 | 5 | A / terrain.rs |
| owner / overworld | layer/windswept_savanna/-2/dirt | 87.6446 | 47.3029 | 5 | A / terrain.rs |
| owner / overworld | layer/desert/-3/sandstone | 0.166952 | 40.2673 | 5 | A / terrain.rs |
| owner / overworld | ore/coal_ore/y80 | 7.595 | 2.25469 | 0.450938 | U / underground.rs |
| owner / overworld | layer/old_growth_birch_forest/-3/stone | 1.18937 | 40.4183 | 5 | A / terrain.rs |
| owner / overworld | layer/savanna_plateau/-1/stone | 41.4408 | 2.24584 | 5 | A / terrain.rs |
| owner / overworld | blocks/fire_coral_wall_fan | 1.39888 | 0.224543 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | top/windswept_savanna/grass_block | 68.3731 | 29.2758 | 5 | A / terrain.rs |
| owner / overworld | blocks/bubble_coral_wall_fan | 1.39902 | 0.227093 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | layer/plains/-3/dirt | 89.3365 | 50.4559 | 5 | A / terrain.rs |
| owner / overworld | blocks/tube_coral_wall_fan | 1.39665 | 0.23061 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/horn_coral_wall_fan | 1.39107 | 0.226717 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/brain_coral_wall_fan | 1.39279 | 0.231442 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | layer/birch_forest/-3/dirt | 89.6806 | 51.1851 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/terracotta/y64 | 5.7263 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | height_biome/desert/p95 | 121 | 83 | 5 | A / terrain.rs |
| owner / overworld | height_biome/forest/p95 | 137 | 99 | 5 | A / terrain.rs |
| owner / overworld | height_biome/old_growth_birch_forest/p100 | 123 | 161 | 5 | A / terrain.rs |
| owner / overworld | nominal_climate_columns/[0.6, 0.6] | 0.891886 | 6.5495 | 1 | A / terrain.rs |
| owner / overworld | feature_biome/birch_forest/oak_leaves | 136.413 | 23.2714 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/ice | 1.12465 | 0 | 0.1 | A / terrain.rs |
| owner / overworld | layer/swamp/-3/dirt | 95.3704 | 58.018 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/river/short_grass | 126.282 | 14.245 | 5 | F / vegetation.rs |
| owner / overworld | layer/windswept_hills/-1/dirt | 95.6218 | 58.325 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/trial_chambers | 356.435 | 0 | 32 | R-trial_chambers / trial_chambers.rs |
| owner / overworld | layer/jungle/-3/stone | 1.01722 | 38.0886 | 5 | A / terrain.rs |
| owner / overworld | layer/river/-3/dirt | 77.7523 | 40.6919 | 5 | A / terrain.rs |
| owner / overworld | layer/bamboo_jungle/-3/stone | 0.097483 | 37.1221 | 5 | A / terrain.rs |
| owner / overworld | height_biome/meadow/p100 | 161 | 124 | 5 | A / terrain.rs |
| owner / overworld | layer/meadow/-2/dirt | 48.9812 | 85.7388 | 5 | A / terrain.rs |
| owner / overworld | layer/meadow/-2/stone | 48.0711 | 11.3778 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/deep_cold_ocean | 5.48721 | 0 | 1 | A / terrain.rs |
| owner / overworld | top/windswept_hills/stone | 2.09391 | 38.251 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/savanna_plateau/amethyst_block | 310.048 | 58.2029 | 11.6406 | F / vegetation.rs |
| owner / overworld | patches/plains/p50 | 16 | 472 | 64 | A / terrain.rs |
| owner / overworld | structure_candidate_count/ancient_city | 7 | 0 | 1 | R-ancient_city / ancient_city.rs |
| owner / overworld | layer/windswept_hills/-1/stone | 2.76015 | 37.7207 | 5 | A / terrain.rs |
| owner / overworld | top/windswept_hills/grass_block | 92.9886 | 58.3998 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/jungle | 0.22165 | 5.37335 | 1 | A / terrain.rs |
| owner / overworld | covered_grass | 8.3218 | 18.6063 | 1 | A / terrain.rs |
| owner / overworld | top/bamboo_jungle/podzol | 0 | 34.2532 | 5 | A / terrain.rs |
| owner / overworld | layer/savanna/-3/dirt | 86.7376 | 52.5638 | 5 | A / terrain.rs |
| owner / the_nether | structure_candidate_count/nether_fossil | 54 | 20 | 5 | N / nether_fossil.rs |
| owner / overworld | layer/savanna_plateau/-2/dirt | 55.0436 | 88.8147 | 5 | A / terrain.rs |
| owner / overworld | top/stony_shore/stone | 84.9764 | 51.2427 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/deep_cold_ocean | 5.05917 | 0 | 1 | A / terrain.rs |
| owner / overworld | surface_band/badlands/terracotta/y80 | 5.05388 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | layer/beach/-3/sandstone | 0.080346 | 33.2818 | 5 | A / terrain.rs |
| owner / overworld | blocks/snow | 1.04266 | 0.050847 | 0.1 | A / terrain.rs |
| owner / overworld | blocks/deepslate_tiles | 0.991704 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | height_biome/birch_forest/p95 | 115 | 82 | 5 | A / terrain.rs |
| owner / overworld | tree_span/forest/jungle_log/p50 | 20.5 | 1 | 2 | F-tree / trees.rs |
| owner / overworld | top/stony_shore/gravel | 7.20805 | 39.613 | 5 | A / terrain.rs |
| owner / the_end | biomes_3d/end_midlands | 16.2571 | 21.0846 | 1 | E / the_end.rs |
| owner / the_end | surface_biomes/end_midlands | 16.2571 | 21.0846 | 1 | E / the_end.rs |
| owner / overworld | biomes_3d/jungle | 0.219458 | 5.04649 | 1 | A / terrain.rs |
| owner / the_nether | biomes_3d/crimson_forest | 14.8129 | 19.5965 | 1 | N / nether.rs |
| owner / the_nether | surface_biomes/crimson_forest | 14.8129 | 19.5926 | 1 | N / nether.rs |
| owner / overworld | layer/savanna_plateau/-2/stone | 41.5411 | 9.87052 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/ruined_portal | 303.213 | 0 | 32 | R-ruined_portal / ruined_portal.rs |
| owner / overworld | layer/plains/-3/stone | 8.91172 | 40.2372 | 5 | A / terrain.rs |
| owner / overworld | layer/birch_forest/-3/stone | 9.22047 | 40.4887 | 5 | A / terrain.rs |
| owner / overworld | layer/swamp/-3/stone | 3.47222 | 34.6809 | 5 | A / terrain.rs |
| owner / overworld | height_biome/beach/p100 | 112 | 143 | 5 | A / terrain.rs |
| owner / the_nether | structure_candidate_offset_p95/nether_fossil | 288.62 | 0 | 32 | N / nether_fossil.rs |
| owner / the_end | top/end_barrens/air | 94.4877 | 64.4235 | 5 | E / the_end.rs |
| owner / the_end | top/end_barrens/end_stone | 5.51227 | 35.5765 | 5 | E / the_end.rs |
| owner / overworld | height_biome/savanna_plateau/p100 | 152 | 122 | 5 | A / terrain.rs |
| owner / overworld | height_mean/sparse_jungle | 91.1337 | 68.6514 | 5 | A / terrain.rs |
| owner / overworld | ore/coal_ore/y112 | 2.3246 | 0.077667 | 0.25 | U / underground.rs |
| owner / overworld | feature_biome/sulfur_caves/potent_sulfur | 137.459 | 30.1221 | 6.02442 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_hills/amethyst_block | 237.118 | 52.6171 | 10.5234 | F / vegetation.rs |
| owner / overworld | layer/savanna/-3/stone | 10.7742 | 39.8026 | 5 | A / terrain.rs |
| owner / the_end | layer/end_barrens/-1/air | 95.2599 | 66.2826 | 5 | E / the_end.rs |
| owner / the_end | layer/end_barrens/-1/end_stone | 4.74007 | 33.7174 | 5 | E / the_end.rs |
| owner / overworld | layer/windswept_savanna/-2/stone | 6.75623 | 35.7167 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/terracotta/y96 | 4.29781 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | blocks/kelp_plant | 31.6243 | 11.649 | 2.3298 | F-ocean / ocean.rs |
| owner / overworld | height_mean/windswept_hills | 66.0546 | 87.355 | 5 | A / terrain.rs |
| owner / overworld | top/sparse_jungle/stone | 29.4811 | 1.46314 | 5 | A / terrain.rs |
| owner / overworld | layer/windswept_savanna/-1/dirt | 81.8184 | 53.8359 | 5 | A / terrain.rs |
| owner / the_end | layer/end_barrens/-2/air | 95.5116 | 67.799 | 5 | E / the_end.rs |
| owner / the_end | layer/end_barrens/-2/end_stone | 4.48842 | 32.201 | 5 | E / the_end.rs |
| owner / overworld | biomes_3d/deep_lukewarm_ocean | 2.9231 | 7.07626 | 1 | A / terrain.rs |
| owner / overworld | height_biome/savanna/p95 | 127 | 100 | 5 | A / terrain.rs |
| owner / the_end | window2/blocks/purpur_block | 5.37741 | 0 | 1 | E / the_end.rs |
| owner / overworld | surface_biomes/deep_lukewarm_ocean | 3.14723 | 7.17318 | 1 | A / terrain.rs |
| owner / the_end | layer/end_barrens/-3/air | 95.7874 | 68.9676 | 5 | E / the_end.rs |
| owner / the_end | layer/end_barrens/-3/end_stone | 4.21263 | 31.0324 | 5 | E / the_end.rs |
| owner / overworld | surface_biomes/cold_ocean | 4.0194 | 0 | 1 | A / terrain.rs |
| owner / overworld | biomes_3d/cold_ocean | 4.01754 | 0 | 1 | A / terrain.rs |
| owner / overworld | surface_biomes/lukewarm_ocean | 13.3779 | 9.37297 | 1 | A / terrain.rs |
| owner / the_nether | slope/p95 | 5 | 9 | 1 | N / nether.rs |
| owner / overworld | layer/sparse_jungle/-1/stone | 29.5857 | 2.98889 | 5 | A / terrain.rs |
| owner / overworld | layer/forest/-3/dirt | 79.1917 | 52.7907 | 5 | A / terrain.rs |
| owner / overworld | nominal_climate_columns/[0.8, 0.9] | 0.794988 | 4.74801 | 1 | A / terrain.rs |
| owner / overworld | surface_band/badlands/terracotta/y48 | 3.90687 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | height_biome/savanna/p100 | 155 | 129 | 5 | A / terrain.rs |
| owner / the_end | blocks/purpur_stairs | 0.779165 | 0 | 0.1 | E / the_end.rs |
| owner / the_end | layer/end_barrens/-4/air | 96.0471 | 70.1839 | 5 | E / the_end.rs |
| owner / the_end | layer/end_barrens/-4/end_stone | 3.95293 | 29.8161 | 5 | E / the_end.rs |
| owner / overworld | biomes_3d/lukewarm_ocean | 13.065 | 9.22599 | 1 | A / terrain.rs |
| owner / overworld | surface_biomes/river | 5.65015 | 9.43889 | 1 | A / terrain.rs |
| owner / the_end | layer/end_barrens/-5/air | 96.3114 | 71.158 | 5 | E / the_end.rs |
| owner / the_end | layer/end_barrens/-5/end_stone | 3.68864 | 28.842 | 5 | E / the_end.rs |
| owner / overworld | tree_span/plains/jungle_log/p50 | 16 | 1 | 2 | F-tree / trees.rs |
| owner / overworld | surface_biomes/flower_forest | 3.71084 | 0 | 1 | A / terrain.rs |
| owner / overworld | blocks/sandstone | 149.485 | 60.2217 | 12.0443 | A / terrain.rs |
| owner / overworld | biomes_3d/flower_forest | 3.69305 | 0 | 1 | A / terrain.rs |
| owner / overworld | layer/river/-3/stone | 14.9301 | 39.4839 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/mangrove_swamp | 0.790457 | 4.46864 | 1 | A / terrain.rs |
| owner / the_end | biomes_3d/end_barrens | 12.1276 | 8.45523 | 1 | E / the_end.rs |
| owner / the_end | nominal_climate_columns/[0.5, 0.5] | 12.1276 | 8.45523 | 1 | E / the_end.rs |
| owner / the_end | nominal_climate_columns/unknown | 87.8724 | 91.5448 | 1 | E / the_end.rs |
| owner / the_end | surface_biomes/end_barrens | 12.1276 | 8.45523 | 1 | E / the_end.rs |
| owner / overworld | biomes_3d/mangrove_swamp | 0.77978 | 4.43914 | 1 | A / terrain.rs |
| owner / overworld | layer/mangrove_swamp/-3/mud | 63.694 | 39.3698 | 5 | A / terrain.rs |
| owner / the_nether | blocks/soul_soil | 519.396 | 211.309 | 42.2617 | N / nether.rs |
| owner / overworld | height_mean/forest | 87.8734 | 69.8237 | 5 | A / terrain.rs |
| owner / overworld | height_biome/meadow/p95 | 146 | 122 | 5 | A / terrain.rs |
| owner / overworld | height_biome/savanna_plateau/p95 | 141 | 117 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/swamp/brown_mushroom | 78.3661 | 7.67957 | 5 | F / vegetation.rs |
| owner / the_nether | blocks/soul_sand | 539.519 | 224.656 | 44.9311 | N / nether.rs |
| owner / overworld | layer/river/-1/dirt | 65.7969 | 42.5275 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/beach | 1.78814 | 5.2755 | 1 | A / terrain.rs |
| owner / overworld | layer/beach/-5/sandstone | 74.9595 | 51.7815 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/river | 5.63308 | 9.09676 | 1 | A / terrain.rs |
| owner / the_end | blocks/purpur_pillar | 0.686051 | 0 | 0.1 | E / the_end.rs |
| owner / overworld | ocean_coast_distance/lukewarm_ocean/p50 | 109.836 | 36.8782 | 16 | A / terrain.rs |
| owner / overworld | feature_biome/lush_caves/spore_blossom | 87.2411 | 19.0665 | 5 | F / vegetation.rs |
| owner / overworld | biomes_3d/beach | 1.77671 | 5.1603 | 1 | A / terrain.rs |
| owner / overworld | surface_biomes/old_growth_birch_forest | 0.769735 | 4.14978 | 1 | A / terrain.rs |
| owner / the_end | height_mean/end_barrens | 1.74528 | 18.4036 | 5 | E / the_end.rs |
| owner / overworld | feature_biome/stony_shore/sculk | 66.0939 | 0 | 5 | U / underground_carvers.rs |
| owner / overworld | ocean_coast_distance/ocean/p50 | 109.179 | 39.3954 | 16 | A / terrain.rs |
| owner / overworld | feature_biome/stony_shore/packed_ice | 64.9965 | 0.546374 | 5 | A / terrain.rs |
| owner / overworld | layer/mangrove_swamp/-2/mud | 82.2132 | 60.7672 | 5 | A / terrain.rs |
| owner / overworld | blocks/cracked_deepslate_bricks | 0.63846 | 0 | 0.1 | F / vegetation.rs |
| owner / the_end | layer/end_midlands/-5/air | 5.51022 | 26.7462 | 5 | E / the_end.rs |
| owner / the_end | layer/end_midlands/-5/end_stone | 94.4898 | 73.2538 | 5 | E / the_end.rs |
| owner / overworld | height_biome/swamp/p95 | 66 | 87 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/red_terracotta/y64 | 3.14825 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | layer/forest/-3/stone | 18.5884 | 39.5439 | 5 | A / terrain.rs |
| owner / overworld | depth/48/water | 38.1276 | 29.7525 | 2 | U / underground_carvers.rs |
| owner / the_end | layer/end_midlands/-4/air | 5.01903 | 25.9154 | 5 | E / the_end.rs |
| owner / the_end | layer/end_midlands/-4/end_stone | 94.981 | 74.0846 | 5 | E / the_end.rs |
| owner / overworld | layer/sparse_jungle/-1/dirt | 67.3808 | 88.2509 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/jungle/leaf_litter | 66.2024 | 3.76106 | 5 | F / vegetation.rs |
| owner / overworld | biomes_3d/old_growth_birch_forest | 0.690348 | 3.80893 | 1 | A / terrain.rs |
| owner / overworld | layer/sparse_jungle/-2/stone | 29.3074 | 8.88431 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_offset_p95/mineshaft | 195.305 | 0 | 32 | R-mineshaft / mineshaft.rs |
| owner / the_end | layer/end_midlands/-3/air | 4.67014 | 24.8592 | 5 | E / the_end.rs |
| owner / the_end | layer/end_midlands/-3/end_stone | 95.3299 | 75.1408 | 5 | E / the_end.rs |
| owner / overworld | tree_span/ocean/dark_oak_log/p50 | 4 | 16 | 2 | F-tree / trees.rs |
| owner / overworld | layer/river/-2/dirt | 73.7804 | 53.8597 | 5 | A / terrain.rs |
| owner / overworld | cave_component_volume/p95 | 3824.8 | 1755 | 351 | U / underground_carvers.rs |
| owner / the_nether | inside_jump_ge16_share | 1.44169 | 1.93239 | 0.25 | N / nether.rs |
| owner / overworld | feature_biome/deep_ocean/kelp_plant | 2007.46 | 598.95 | 119.79 | F-ocean / ocean.rs |
| owner / overworld | surface_band/badlands/terracotta/y128 | 2.91696 | 0 | 0.25 | A / terrain.rs |
| owner / the_end | layer/end_midlands/-2/air | 4.37526 | 23.8037 | 5 | E / the_end.rs |
| owner / the_end | layer/end_midlands/-2/end_stone | 95.6247 | 76.1963 | 5 | E / the_end.rs |
| owner / overworld | structure_candidate_count/ocean_ruin | 43 | 22 | 5.5 | R-ocean_ruin / ocean_ruin.rs |
| owner / the_end | layer/end_midlands/-1/air | 4.16352 | 22.8307 | 5 | E / the_end.rs |
| owner / the_end | layer/end_midlands/-1/end_stone | 95.8365 | 77.1693 | 5 | E / the_end.rs |
| owner / the_nether | jump_ge16_share | 1.44256 | 1.90831 | 0.25 | N / nether.rs |
| owner / overworld | tree_span/savanna/dark_oak_log/p50 | 14 | 3 | 2 | F-tree / trees.rs |
| owner / overworld | blocks/sand | 285.393 | 136.127 | 27.2255 | A / terrain.rs |
| owner / the_end | blocks/end_stone_bricks | 0.547628 | 0 | 0.1 | E / the_end.rs |
| owner / overworld | feature_biome/sulfur_caves/cave_vines_plant | 55.8372 | 1.19024 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/mangrove_swamp/-3/stone | 25.8982 | 44.0568 | 5 | A / terrain.rs |
| owner / overworld | layer/windswept_savanna/-1/stone | 6.91615 | 24.9761 | 5 | A / terrain.rs |
| owner / overworld | height_mean/plains | 78.8393 | 65.308 | 5 | A / terrain.rs |
| owner / overworld | layer/beach/-2/sand | 89.094 | 71.0666 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/bamboo_jungle/birch_leaves | 53.9885 | 0 | 5 | F-tree / trees.rs |
| owner / overworld | layer/sparse_jungle/-2/dirt | 67.2407 | 85.1058 | 5 | A / terrain.rs |
| owner / the_end | top/end_midlands/air | 3.71948 | 21.454 | 5 | E / the_end.rs |
| owner / the_end | top/end_midlands/end_stone | 96.2805 | 78.546 | 5 | E / the_end.rs |
| owner / overworld | feature_biome/deep_dark/sculk_sensor | 119.273 | 38.2456 | 7.64912 | U / underground_carvers.rs |
| owner / overworld | ore/copper_ore/y64 | 4.93686 | 2.40788 | 0.481575 | U / underground.rs |
| owner / the_end | depth/48/underground_air | 0.034252 | 7.03267 | 2 | E / the_end.rs |
| owner / overworld | layer/mangrove_swamp/-1/mud | 86.0022 | 68.5146 | 5 | A / terrain.rs |
| owner / overworld | layer/sparse_jungle/-3/dirt | 67.1455 | 49.7191 | 5 | A / terrain.rs |
| owner / overworld | layer/swamp/-4/stone | 90.7407 | 73.4685 | 5 | A / terrain.rs |
| owner / overworld | layer/meadow/-3/dirt | 48.8252 | 31.6241 | 5 | A / terrain.rs |
| owner / overworld | top/forest/stone | 18.4811 | 1.4172 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/brown_terracotta/y48 | 2.55623 | 0 | 0.25 | A / terrain.rs |
| owner / the_end | depth/32/underground_air | 6.67068 | 13.4711 | 2 | E / the_end.rs |
| owner / overworld | structure_candidate_spacing_p95/buried_treasure | 1635.11 | 720.178 | 180.044 | R-shipwreck / shipwreck.rs |
| owner / overworld | nominal_climate_columns/[0.8, 0.4] | 8.7197 | 11.2497 | 1 | A / terrain.rs |
| owner / overworld | top/sparse_jungle/grass_block | 65.9425 | 82.6956 | 5 | A / terrain.rs |
| owner / overworld | blocks/bamboo | 0 | 7.64552 | 1.5291 | F / vegetation.rs |
| owner / overworld | blocks/dark_oak_leaves | 0 | 7.12776 | 1.42555 | F-tree / trees.rs |
| owner / overworld | blocks/glow_lichen | 0 | 2.96735 | 0.593471 | F / vegetation.rs |
| owner / overworld | tree_span/windswept_savanna/jungle_log/p50 | 22 | 12 | 2 | F-tree / trees.rs |
| owner / overworld | layer/plains/-4/stone | 87.8164 | 71.154 | 5 | A / terrain.rs |
| owner / overworld | blocks/podzol | 0.002792 | 1.39279 | 0.278558 | A / terrain.rs |
| owner / overworld | feature_biome/meadow/birch_leaves | 72.1612 | 22.347 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/dripstone_block | 0.121802 | 27.9425 | 5.5885 | U / underground_carvers.rs |
| owner / overworld | blocks/dark_oak_log | 0.013262 | 2.68141 | 0.536283 | F-tree / trees.rs |
| owner / overworld | blocks/pointed_dripstone | 0.04376 | 6.26927 | 1.25385 | U / underground_carvers.rs |
| owner / overworld | ore/coal_ore/y64 | 11.8608 | 5.99718 | 1.19944 | U / underground.rs |
| owner / overworld | layer/beach/-5/stone | 17.8357 | 34.0571 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/orange_terracotta/y112 | 2.42839 | 0 | 0.25 | A / terrain.rs |
| owner / the_nether | blocks/basalt | 96.5721 | 2882.58 | 576.515 | N / nether.rs |
| owner / overworld | layer/forest/-1/stone | 18.663 | 2.65294 | 5 | A / terrain.rs |
| owner / overworld | height_biome/forest/p100 | 169 | 153 | 5 | A / terrain.rs |
| owner / overworld | blocks/cobbled_deepslate | 0.502564 | 0.024725 | 0.1 | F / vegetation.rs |
| owner / overworld | tree_base_estimates/jungle/jungle_log | 0.01345 | 0.490751 | 0.1 | F-tree / trees.rs |
| owner / overworld | layer/river/-4/stone | 77.2948 | 61.572 | 5 | A / terrain.rs |
| owner / overworld | blocks/muddy_mangrove_roots | 0.14642 | 2.57043 | 0.514086 | F / vegetation.rs |
| owner / overworld | surface_band/eroded_badlands/terracotta/y96 | 2.3563 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | layer/bamboo_jungle/-4/stone | 89.598 | 74.0073 | 5 | A / terrain.rs |
| owner / overworld | blocks/sulfur_spike | 0.121882 | 1.62522 | 0.325045 | U / underground_carvers.rs |
| owner / overworld | blocks/mangrove_roots | 0.439448 | 5.43499 | 1.087 | F / vegetation.rs |
| owner / overworld | top/mangrove_swamp/mud | 86.2144 | 70.9153 | 5 | A / terrain.rs |
| owner / overworld | blocks/jungle_leaves | 1.58853 | 18.6429 | 3.72859 | F-tree / trees.rs |
| owner / overworld | ore/copper_ore/y80 | 1.55465 | 0.413971 | 0.25 | U / underground.rs |
| owner / overworld | layer/forest/-4/stone | 88.9068 | 73.7204 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/birch_forest | 0.122151 | 2.39973 | 1 | A / terrain.rs |
| owner / overworld | blocks/jungle_log | 0.422052 | 4.70144 | 0.940288 | F-tree / trees.rs |
| owner / overworld | layer/desert/-2/sand | 95.1648 | 80.0265 | 5 | A / terrain.rs |
| owner / overworld | blocks/dead_bush | 0.483583 | 0.029934 | 0.1 | F / vegetation.rs |
| owner / the_nether | depth/48/air | 54.4369 | 48.4037 | 2 | N / nether.rs |
| owner / overworld | layer/ocean/-3/dirt | 23.7298 | 8.67072 | 5 | A / terrain.rs |
| owner / overworld | blocks/sea_pickle | 0.550673 | 0.099627 | 0.1 | F-ocean / ocean.rs |
| owner / the_end | patches/end_barrens/p50 | 64 | 256 | 64 | E / the_end.rs |
| owner / the_end | patches/end_midlands/p50 | 64 | 256 | 64 | E / the_end.rs |
| owner / overworld | structure_candidate_count/desert_pyramid | 3 | 0 | 1 | R-temples / temples.rs |
| owner / overworld | structure_candidate_count/trail_ruins | 1 | 4 | 1 | R-trail_ruins / trail_ruins.rs |
| owner / overworld | blocks/cracked_deepslate_tiles | 0.448952 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | blocks/clay | 8.16731 | 79.0625 | 15.8125 | A / terrain.rs |
| owner / overworld | blocks/vine | 3.54477 | 33.5228 | 6.70456 | F / vegetation.rs |
| owner / overworld | layer/stony_shore/-2/stone | 88.2797 | 73.5764 | 5 | A / terrain.rs |
| owner / overworld | layer/meadow/-3/stone | 48.0404 | 62.7338 | 5 | A / terrain.rs |
| owner / overworld | blocks/mangrove_leaves | 6.27504 | 52.3623 | 10.4725 | F-tree / trees.rs |
| owner / overworld | blocks/mangrove_log | 0.387527 | 3.21797 | 0.643593 | F-tree / trees.rs |
| owner / overworld | depth/16/underground_air | 13.4916 | 7.6282 | 2 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lush_caves/small_dripleaf | 193.66 | 70.2655 | 14.0531 | F / vegetation.rs |
| owner / overworld | feature_biome/birch_forest/leaf_litter | 61.8193 | 17.9574 | 5 | F / vegetation.rs |
| owner / overworld | depth/16/air | 13.4156 | 7.60561 | 2 | U / underground_carvers.rs |
| owner / overworld | layer/sparse_jungle/-4/stone | 88.4919 | 73.9681 | 5 | A / terrain.rs |
| owner / overworld | top/windswept_savanna/gravel | 5.35467 | 19.8377 | 5 | A / terrain.rs |
| owner / overworld | layer/jungle/-4/dirt | 1.57078 | 16.0388 | 5 | A / terrain.rs |
| owner / overworld | blocks/mangrove_propagule | 0.126446 | 0.926334 | 0.185267 | F / vegetation.rs |
| owner / overworld | top/river/dirt | 15.0953 | 29.2675 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/birch_forest | 0.119718 | 2.23556 | 1 | A / terrain.rs |
| owner / overworld | surface_biomes/dark_forest | 0 | 2.10425 | 1 | A / terrain.rs |
| owner / overworld | height_biome/river/p100 | 91 | 77 | 5 | A / terrain.rs |
| owner / overworld | blocks/wildflowers | 0.190851 | 1.1894 | 0.23788 | F / vegetation.rs |
| owner / overworld | blocks/cave_vines_plant | 3.63438 | 1.97858 | 0.395715 | F / vegetation.rs |
| owner / overworld | structure_candidate_count/buried_treasure | 5 | 16 | 4 | R-shipwreck / shipwreck.rs |
| owner / overworld | blocks/fern | 0.236892 | 1.353 | 0.270601 | F / vegetation.rs |
| owner / overworld | blocks/sugar_cane | 0.454724 | 0.043947 | 0.1 | F / vegetation.rs |
| owner / overworld | top/windswept_savanna/coarse_dirt | 0 | 13.4718 | 5 | A / terrain.rs |
| owner / overworld | layer/stony_shore/-3/stone | 87.4788 | 74.0379 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/deep_dark/amethyst_block | 534.486 | 204.967 | 40.9933 | F / vegetation.rs |
| owner / overworld | top/river/sand | 14.5711 | 27.9591 | 5 | A / terrain.rs |
| owner / the_nether | structure_box_composition/nether_fossil/air | 64.7761 | 84.8291 | 5 | N / nether_fossil.rs |
| owner / overworld | slope/p99 | 5 | 7 | 1 | A / terrain.rs |
| owner / the_end | slope/p99 | 51 | 53 | 1 | E / the_end.rs |
| owner / the_end | inside_jump_ge16_share | 1.43396 | 1.1009 | 0.25 | E / the_end.rs |
| owner / overworld | depth/80/air | 89.0986 | 94.3963 | 2 | U / underground_carvers.rs |
| owner / the_end | biomes_3d/end_highlands | 18.7054 | 16.7321 | 1 | E / the_end.rs |
| owner / the_end | surface_biomes/end_highlands | 18.7054 | 16.7321 | 1 | E / the_end.rs |
| owner / overworld | layer/old_growth_birch_forest/-4/stone | 87.4292 | 74.4102 | 5 | A / terrain.rs |
| owner / overworld | height_biome/bamboo_jungle/p100 | 97 | 84 | 5 | A / terrain.rs |
| owner / the_end | height_biome/end_barrens/p95 | 45 | 58 | 5 | E / the_end.rs |
| owner / overworld | height_biome/jungle/p100 | 138 | 125 | 5 | A / terrain.rs |
| owner / the_end | height_biome/small_end_islands/p100 | 83 | 70 | 5 | E / the_end.rs |
| owner / overworld | top/swamp/dirt | 0.694444 | 13.6899 | 5 | A / terrain.rs |
| owner / the_end | feature_biome/end_midlands/chorus_flower | 41.2203 | 2.24622 | 5 | E / the_end.rs |
| owner / overworld | layer/sparse_jungle/-3/stone | 29.1187 | 42.0937 | 5 | A / terrain.rs |
| owner / overworld | surface_band/eroded_badlands/terracotta/y112 | 1.94389 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | depth/96/underground_air | 9.83953 | 4.66092 | 2 | U / underground_carvers.rs |
| owner / overworld | layer/mangrove_swamp/-2/dirt | 0.802643 | 13.7277 | 5 | A / terrain.rs |
| owner / overworld | blocks/mud | 6.59677 | 28.9714 | 5.79427 | A / terrain.rs |
| owner / the_end | jump_ge16_share | 1.42617 | 1.10465 | 0.25 | E / the_end.rs |
| owner / overworld | depth/0/underground_air | 12.4402 | 7.31501 | 2 | U / underground_carvers.rs |
| owner / overworld | depth/0/air | 12.4402 | 7.31521 | 2 | U / underground_carvers.rs |
| owner / overworld | blocks/deepslate_tile_stairs | 0.383339 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | layer/old_growth_birch_forest/-4/dirt | 1.86512 | 14.6182 | 5 | A / terrain.rs |
| owner / overworld | ore/coal_ore/y128 | 0.96059 | 0.006282 | 0.25 | U / underground.rs |
| owner / overworld | layer/bamboo_jungle/-4/dirt | 1.47371 | 14.076 | 5 | A / terrain.rs |
| owner / the_end | height_mean/end_midlands | 52.4715 | 43.0413 | 5 | E / the_end.rs |
| owner / overworld | layer/savanna/-4/stone | 86.326 | 73.8348 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/oak_log | 39.5986 | 2.20832 | 5 | F-tree / trees.rs |
| owner / overworld | depth/32/underground_air | 9.58329 | 4.59944 | 2 | U / underground_carvers.rs |
| owner / overworld | height_mean/windswept_savanna | 63.058 | 72.3919 | 5 | A / terrain.rs |
| owner / overworld | layer/stony_shore/-1/stone | 86.2307 | 73.8465 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/dark_forest | 0 | 1.85477 | 1 | A / terrain.rs |
| owner / overworld | layer/plains/-4/dirt | 2.41149 | 14.7441 | 5 | A / terrain.rs |
| owner / overworld | layer/bamboo_jungle/-2/dirt | 98.0446 | 85.7552 | 5 | A / terrain.rs |
| owner / overworld | layer/jungle/-2/dirt | 96.7449 | 84.4971 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_count/shipwreck | 29 | 18 | 4.5 | R-shipwreck / shipwreck.rs |
| owner / overworld | layer/savanna/-4/dirt | 2.30758 | 14.4725 | 5 | A / terrain.rs |
| owner / overworld | blocks/cinnabar | 47.9222 | 175.68 | 35.1361 | U / underground_carvers.rs |
| owner / overworld | layer/birch_forest/-4/stone | 86.6844 | 74.594 | 5 | A / terrain.rs |
| owner / overworld | height_biome/bamboo_jungle/p95 | 84 | 72 | 5 | A / terrain.rs |
| owner / overworld | height_biome/deep_lukewarm_ocean/p100 | 53 | 65 | 5 | A / terrain.rs |
| owner / overworld | layer/warm_ocean/-4/stone | 73.4063 | 61.4861 | 5 | A / terrain.rs |
| owner / overworld | top/jungle/grass_block | 93.9109 | 82.0031 | 5 | A / terrain.rs |
| owner / overworld | layer/forest/-4/dirt | 1.93366 | 13.8351 | 5 | A / terrain.rs |
| owner / the_nether | blocks/blackstone | 183.067 | 632.666 | 126.533 | N / nether.rs |
| owner / overworld | blocks/birch_log | 0.98212 | 3.3752 | 0.675041 | F-tree / trees.rs |
| owner / the_nether | tree_base_estimates/crimson_forest/crimson_stem | 0.484848 | 0.130395 | 0.1 | N / nether.rs |
| owner / overworld | layer/sparse_jungle/-4/dirt | 1.98745 | 13.7295 | 5 | A / terrain.rs |
| owner / overworld | surface_band/eroded_badlands/terracotta/y80 | 1.75819 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | blocks/sulfur | 63.8774 | 212.986 | 42.5973 | U / underground_carvers.rs |
| owner / overworld | feature_biome/deep_dark/sculk | 11869.7 | 4945.94 | 989.187 | U / underground_carvers.rs |
| owner / overworld | blocks/birch_leaves | 8.00199 | 26.6226 | 5.32452 | F-tree / trees.rs |
| owner / overworld | top/swamp/grass_block | 94.6759 | 83.0556 | 5 | A / terrain.rs |
| owner / overworld | depth/96/air | 93.2814 | 97.9177 | 2 | U / underground_carvers.rs |
| owner / overworld | height_mean/desert | 74.4226 | 65.7905 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/badlands | 1.70642 | 0 | 1 | A / terrain.rs |
| owner / overworld | layer/beach/-1/sand | 89.3726 | 78.0355 | 5 | A / terrain.rs |
| owner / overworld | layer/old_growth_birch_forest/-2/dirt | 97.1117 | 85.7935 | 5 | A / terrain.rs |
| owner / overworld | layer/river/-4/dirt | 2.22984 | 13.5259 | 5 | A / terrain.rs |
| owner / overworld | layer/lukewarm_ocean/-1/stone | 66.5235 | 55.2738 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_hills/spruce_log | 36.3526 | 2.62096 | 5 | F-tree / trees.rs |
| owner / overworld | top/mangrove_swamp/grass_block | 0.286563 | 11.383 | 5 | A / terrain.rs |
| owner / the_nether | depth/96/air | 36.8026 | 41.2282 | 2 | N / nether.rs |
| owner / the_end | height_biome/end_barrens/p100 | 59 | 70 | 5 | E / the_end.rs |
| owner / overworld | height_biome/lukewarm_ocean/p100 | 79 | 68 | 5 | A / terrain.rs |
| owner / overworld | height_mean/savanna | 78.3354 | 70.0902 | 5 | A / terrain.rs |
| owner / the_nether | depth/16/underground_lava | 25.0608 | 20.6803 | 2 | N / nether.rs |
| owner / overworld | biomes_3d/badlands | 1.6418 | 0 | 1 | A / terrain.rs |
| owner / overworld | layer/warm_ocean/-5/stone | 73.3221 | 62.387 | 5 | A / terrain.rs |
| owner / overworld | depth/112/underground_air | 11.6788 | 16.0343 | 2 | U / underground_carvers.rs |
| owner / overworld | layer/warm_ocean/-1/sand | 0.095073 | 10.9619 | 5 | A / terrain.rs |
| owner / overworld | layer/jungle/-4/stone | 83.0526 | 72.1975 | 5 | A / terrain.rs |
| owner / overworld | top/plains/grass_block | 87.9682 | 77.1184 | 5 | A / terrain.rs |
| owner / overworld | layer/forest/-1/dirt | 79.0964 | 89.9074 | 5 | A / terrain.rs |
| owner / overworld | layer/lukewarm_ocean/-2/stone | 66.5295 | 55.7228 | 5 | A / terrain.rs |
| owner / overworld | top/river/gravel | 19.3493 | 30.1211 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/deep_dark/budding_amethyst | 48.7065 | 16.7092 | 5 | F / vegetation.rs |
| owner / overworld | layer/swamp/-4/dirt | 3.24074 | 13.9039 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/dripstone_caves | 0.013526 | 1.59933 | 1 | A / terrain.rs |
| owner / overworld | surface_band/badlands/white_terracotta/y48 | 1.58439 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | depth/-48/underground_air | 12.7065 | 8.51146 | 2 | U / underground_carvers.rs |
| owner / overworld | depth/-48/air | 12.7065 | 8.51177 | 2 | U / underground_carvers.rs |
| owner / overworld | surface_biomes/sunflower_plains | 1.57135 | 0 | 1 | A / terrain.rs |
| owner / overworld | biomes_3d/sunflower_plains | 1.56739 | 0 | 1 | A / terrain.rs |
| owner / overworld | top/windswept_savanna/stone | 2.5057 | 12.9399 | 5 | A / terrain.rs |
| owner / overworld | top/beach/sand | 89.8594 | 79.4395 | 5 | A / terrain.rs |
| owner / the_nether | height/p50 | 59 | 64 | 4 | N / nether.rs |
| owner / overworld | feature_biome/windswept_savanna/acacia_leaves | 404.31 | 180.576 | 36.1151 | F-tree / trees.rs |
| owner / overworld | feature_biome/deep_lukewarm_ocean/kelp_plant | 1135.43 | 508.052 | 101.61 | F-ocean / ocean.rs |
| owner / overworld | horizontal_log_blocks/oak_log | 0.292625 | 0.763027 | 0.152605 | F-tree / trees.rs |
| owner / overworld | layer/stony_shore/-4/stone | 87.4577 | 77.2467 | 5 | A / terrain.rs |
| owner / the_nether | surface_biomes/nether_wastes | 45.0528 | 46.5805 | 1 | N / nether.rs |
| owner / the_nether | biomes_3d/nether_wastes | 45.0528 | 46.5766 | 1 | N / nether.rs |
| owner / overworld | depth/32/water | 13.2279 | 9.17887 | 2 | U / underground_carvers.rs |
| owner / overworld | structure_candidate_count/pillager_outpost | 1 | 3 | 1 | R-pillager_outpost / pillager_outpost.rs |
| owner / overworld | feature_biome/plains/oak_log | 37.3497 | 7.40588 | 5 | F-tree / trees.rs |
| owner / overworld | layer/warm_ocean/-2/sand | 0.055106 | 9.94678 | 5 | A / terrain.rs |
| owner / overworld | blocks/leaf_litter | 7.6616 | 18.7969 | 3.75938 | F / vegetation.rs |
| owner / overworld | layer/stony_shore/-5/stone | 86.8774 | 77.0396 | 5 | A / terrain.rs |
| owner / overworld | height_mean/birch_forest | 74.5567 | 67.3102 | 5 | A / terrain.rs |
| owner / overworld | layer/ocean/-2/dirt | 20.2206 | 10.5598 | 5 | A / terrain.rs |
| owner / overworld | layer/beach/-3/stone | 5.18849 | 14.839 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/oak_leaves | 44.0688 | 15.1428 | 5 | F-tree / trees.rs |
| owner / overworld | layer/plains/-2/dirt | 89.5963 | 79.9577 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/deep_dark/sculk_shrieker | 31.4569 | 2.59922 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/river/-1/sand | 14.3558 | 23.975 | 5 | A / terrain.rs |
| owner / overworld | blocks/kelp | 4.30838 | 2.7364 | 0.547279 | F-ocean / ocean.rs |
| owner / overworld | layer/mangrove_swamp/-1/dirt | 8.74814 | 18.3024 | 5 | A / terrain.rs |
| owner / overworld | blocks/red_mushroom_block | 0 | 0.285404 | 0.1 | F / vegetation.rs |
| owner / overworld | layer/birch_forest/-4/dirt | 2.47253 | 11.9323 | 5 | A / terrain.rs |
| owner / overworld | nominal_climate_columns/[0.0, 0.5] | 1.41195 | 0 | 1 | A / terrain.rs |
| owner / overworld | layer/mangrove_swamp/-4/mud | 0 | 9.31878 | 5 | A / terrain.rs |
| owner / the_nether | depth/16/lava | 30.8242 | 27.1035 | 2 | N / nether.rs |
| owner / the_nether | blocks/nether_bricks | 27.9936 | 62.8825 | 12.5765 | N / nether.rs |
| owner / overworld | layer/lukewarm_ocean/-1/sand | 0.283065 | 9.51832 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/tube_coral_block | 214.468 | 101.962 | 20.3924 | F-ocean / ocean.rs |
| owner / overworld | surface_biomes/bamboo_jungle | 0.18288 | 1.55313 | 1 | A / terrain.rs |
| owner / overworld | blocks/infested_deepslate | 0 | 0.272813 | 0.1 | F / vegetation.rs |
| owner / overworld | top/beach/gravel | 6.44236 | 15.5141 | 5 | A / terrain.rs |
| owner / overworld | tree_base_estimates/forest/oak_log | 0.289753 | 0.632822 | 0.126564 | F-tree / trees.rs |
| owner / overworld | height_biome/old_growth_birch_forest/p95 | 90 | 81 | 5 | A / terrain.rs |
| owner / overworld | depth/112/air | 96.0065 | 99.6064 | 2 | U / underground_carvers.rs |
| owner / overworld | top/ocean/gravel | 76.1446 | 85.1177 | 5 | A / terrain.rs |
| owner / overworld | blocks/oak_leaves | 29.9067 | 64.7828 | 12.9566 | F-tree / trees.rs |
| owner / overworld | feature_biome/warm_ocean/brain_coral_block | 217.683 | 104.845 | 20.969 | F-ocean / ocean.rs |
| owner / the_nether | blocks/bone_block | 0.557392 | 0.289256 | 0.1 | N / nether.rs |
| owner / overworld | layer/forest/-2/stone | 18.5851 | 9.65963 | 5 | A / terrain.rs |
| owner / overworld | layer/warm_ocean/-1/dirt | 9.97852 | 1.08561 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/orange_terracotta/y128 | 1.32457 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/horn_coral_block | 208.415 | 101.194 | 20.2388 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/forest/sculk | 27.5897 | 1.15705 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/jungle/-2/stone | 0.889478 | 9.61945 | 5 | A / terrain.rs |
| owner / overworld | surface_biomes/ocean | 3.72527 | 5.03327 | 1 | A / terrain.rs |
| owner / overworld | layer/desert/-2/sandstone | 0.05558 | 8.77023 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_savanna/acacia_log | 45.6197 | 19.6043 | 5 | F-tree / trees.rs |
| owner / overworld | nominal_climate_columns/[0.95, 0.8] | 0.561424 | 1.85922 | 1 | A / terrain.rs |
| owner / overworld | surface_biomes/sparse_jungle | 0.561424 | 1.85922 | 1 | A / terrain.rs |
| owner / overworld | surface_biomes/frozen_ocean | 1.29769 | 0 | 1 | A / terrain.rs |
| owner / overworld | biomes_3d/frozen_ocean | 1.29521 | 0 | 1 | A / terrain.rs |
| owner / overworld | feature_biome/bamboo_jungle/tall_grass | 35.2543 | 9.4178 | 5 | F / vegetation.rs |
| owner / overworld | height_mean/river | 60.9462 | 54.5001 | 5 | A / terrain.rs |
| owner / overworld | layer/bamboo_jungle/-2/stone | 0 | 8.59402 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/sparse_jungle | 0.534438 | 1.81802 | 1 | A / terrain.rs |
| owner / overworld | surface_band/badlands/yellow_terracotta/y128 | 1.28259 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | top/savanna/stone | 9.82193 | 1.28405 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/fire_coral_block | 211.358 | 104.647 | 20.9294 | F-ocean / ocean.rs |
| owner / overworld | height_mean/bamboo_jungle | 72.029 | 65.6598 | 5 | A / terrain.rs |
| owner / overworld | blocks/spruce_log | 0.259953 | 0.006228 | 0.1 | F-tree / trees.rs |
| owner / overworld | blocks/polished_deepslate | 0.252624 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | depth/32/air | 7.44235 | 4.08424 | 2 | U / underground_carvers.rs |
| owner / overworld | blocks/azalea_leaves | 0.275739 | 0.024027 | 0.1 | F-tree / trees.rs |
| owner / the_nether | ore/nether_quartz_ore/y112 | 12.6804 | 8.44077 | 1.68815 | N / nether.rs |
| owner / the_nether | blocks/soul_fire | 0.382002 | 0.131313 | 0.1 | N / nether.rs |
| owner / overworld | feature_biome/bamboo_jungle/bamboo | 0 | 5741.36 | 1148.27 | F / vegetation.rs |
| owner / overworld | feature_biome/beach/mangrove_leaves | 0 | 79.8492 | 15.9698 | F-tree / trees.rs |
| owner / overworld | feature_biome/beach/sulfur | 0 | 35.3151 | 7.06301 | U / underground_carvers.rs |
| owner / overworld | feature_biome/birch_forest/cinnabar | 0 | 94.1361 | 18.8272 | U / underground_carvers.rs |
| owner / overworld | feature_biome/birch_forest/dripstone_block | 0 | 107.427 | 21.4854 | U / underground_carvers.rs |
| owner / overworld | feature_biome/birch_forest/moss_block | 0 | 29.3427 | 5.86854 | A / terrain.rs |
| owner / overworld | feature_biome/birch_forest/sulfur | 0 | 117.896 | 23.5792 | U / underground_carvers.rs |
| owner / overworld | feature_biome/deep_lukewarm_ocean/sulfur | 0 | 26.8531 | 5.37062 | U / underground_carvers.rs |
| owner / overworld | feature_biome/deep_ocean/powder_snow | 0 | 65.0623 | 13.0125 | A / terrain.rs |
| owner / overworld | feature_biome/dripstone_caves/cinnabar | 0 | 57.0839 | 11.4168 | U / underground_carvers.rs |
| owner / overworld | feature_biome/dripstone_caves/mossy_cobblestone | 0 | 39.4618 | 7.89236 | U / underground_carvers.rs |
| owner / overworld | feature_biome/dripstone_caves/mossy_stone_bricks | 0 | 25.8867 | 5.17734 | U / underground_carvers.rs |
| owner / overworld | feature_biome/dripstone_caves/sulfur | 0 | 76.3794 | 15.2759 | U / underground_carvers.rs |
| owner / overworld | feature_biome/forest/cinnabar | 0 | 79.6949 | 15.939 | U / underground_carvers.rs |
| owner / overworld | feature_biome/forest/sulfur | 0 | 92.6932 | 18.5386 | U / underground_carvers.rs |
| owner / overworld | feature_biome/jungle/cinnabar | 0 | 201.241 | 40.2482 | U / underground_carvers.rs |
| owner / overworld | feature_biome/jungle/moss_block | 0 | 46.7994 | 9.35989 | A / terrain.rs |
| owner / overworld | feature_biome/jungle/sulfur | 0 | 252.099 | 50.4198 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lukewarm_ocean/cinnabar | 0 | 72.4387 | 14.4877 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lukewarm_ocean/sulfur | 0 | 90.3797 | 18.0759 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lush_caves/cinnabar | 0 | 90.3537 | 18.0707 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lush_caves/sulfur | 0 | 116.095 | 23.2189 | U / underground_carvers.rs |
| owner / overworld | feature_biome/mangrove_swamp/cinnabar | 0 | 35.3555 | 7.07109 | U / underground_carvers.rs |
| owner / overworld | feature_biome/mangrove_swamp/dripstone_block | 0 | 26.3674 | 5.27348 | U / underground_carvers.rs |
| owner / overworld | feature_biome/mangrove_swamp/sulfur | 0 | 32.2118 | 6.44236 | U / underground_carvers.rs |
| owner / overworld | feature_biome/ocean/cinnabar | 0 | 54.9805 | 10.9961 | U / underground_carvers.rs |
| owner / overworld | feature_biome/ocean/sulfur | 0 | 58.9153 | 11.7831 | U / underground_carvers.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/dripstone_block | 0 | 281.209 | 56.2419 | U / underground_carvers.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/pointed_dripstone | 0 | 29.21 | 5.84201 | U / underground_carvers.rs |
| owner / overworld | feature_biome/plains/sulfur | 0 | 40.1215 | 8.02431 | U / underground_carvers.rs |
| owner / overworld | feature_biome/savanna/cinnabar | 0 | 155.188 | 31.0376 | U / underground_carvers.rs |
| owner / overworld | feature_biome/savanna/mangrove_leaves | 0 | 32.1687 | 6.43374 | F-tree / trees.rs |
| owner / overworld | feature_biome/savanna/sulfur | 0 | 187.922 | 37.5845 | U / underground_carvers.rs |
| owner / overworld | feature_biome/savanna_plateau/cinnabar | 0 | 56.7672 | 11.3534 | U / underground_carvers.rs |
| owner / overworld | feature_biome/savanna_plateau/dripstone_block | 0 | 60.8535 | 12.1707 | U / underground_carvers.rs |
| owner / overworld | feature_biome/savanna_plateau/sulfur | 0 | 43.5141 | 8.70283 | U / underground_carvers.rs |
| owner / overworld | feature_biome/sparse_jungle/mangrove_leaves | 0 | 52.6957 | 10.5391 | F-tree / trees.rs |
| owner / overworld | feature_biome/sparse_jungle/moss_block | 0 | 78.0971 | 15.6194 | A / terrain.rs |
| owner / overworld | feature_biome/swamp/amethyst_block | 0 | 146.417 | 29.2834 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/birch_leaves | 0 | 31.7288 | 6.34575 | F-tree / trees.rs |
| owner / overworld | feature_biome/swamp/cinnabar | 0 | 175.216 | 35.0431 | U / underground_carvers.rs |
| owner / overworld | feature_biome/swamp/dripstone_block | 0 | 54.8685 | 10.9737 | U / underground_carvers.rs |
| owner / overworld | feature_biome/swamp/leaf_litter | 0 | 40.823 | 8.1646 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/mangrove_leaves | 0 | 342.954 | 68.5907 | F-tree / trees.rs |
| owner / overworld | feature_biome/swamp/mangrove_roots | 0 | 33.1434 | 6.62868 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/mossy_cobblestone | 0 | 26.8785 | 5.3757 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/sulfur | 0 | 82.7575 | 16.5515 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_hills/cinnabar | 0 | 117.004 | 23.4008 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_hills/dripstone_block | 0 | 284.152 | 56.8304 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_hills/leaf_litter | 0 | 49.5511 | 9.91021 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_hills/pointed_dripstone | 0 | 30.4131 | 6.08261 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_hills/snow | 0 | 92.6733 | 18.5347 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_hills/sulfur | 0 | 77.3432 | 15.4686 | U / underground_carvers.rs |
| owner / overworld | feature_biome/river/dripstone_block | 0.009696 | 65.8034 | 13.1607 | U / underground_carvers.rs |
| owner / overworld | feature_biome/forest/dripstone_block | 0.044398 | 119.601 | 23.9202 | U / underground_carvers.rs |
| owner / overworld | layer/warm_ocean/-2/dirt | 9.96274 | 1.64567 | 5 | A / terrain.rs |
| owner / overworld | layer/old_growth_birch_forest/-2/stone | 1.14714 | 9.46269 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/bubble_coral_block | 208.359 | 104.328 | 20.8655 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/desert/moss_block | 0.199361 | 41.025 | 8.205 | A / terrain.rs |
| owner / overworld | feature_biome/plains/dripstone_block | 0.562041 | 104.694 | 20.9388 | U / underground_carvers.rs |
| owner / overworld | surface_band/eroded_badlands/terracotta/y64 | 1.24156 | 0 | 0.25 | A / terrain.rs |
| owner / the_nether | blocks/nether_brick_fence | 0.669421 | 1.32874 | 0.265748 | N / nether.rs |
| owner / overworld | feature_biome/sulfur_caves/cave_vines | 25.6069 | 0.824009 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/warm_ocean/-3/dirt | 9.93871 | 1.74365 | 5 | A / terrain.rs |
| owner / overworld | covered_grass_by/leaf_litter | 10.6116 | 15.5098 | 2 | A / terrain.rs |
| owner / overworld | layer/warm_ocean/-5/sandstone | 0.01666 | 8.15037 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/beach/moss_block | 0.845396 | 35.0663 | 7.01327 | A / terrain.rs |
| owner / overworld | layer/river/-2/stone | 14.818 | 22.9144 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/beach/cinnabar | 0 | 24.0267 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/desert/bone_block | 0.067708 | 24.0776 | 5 | F / vegetation.rs |
| owner / overworld | height_biome/birch_forest/p100 | 128 | 120 | 5 | A / terrain.rs |
| owner / the_end | height_biome/end_midlands/p100 | 62 | 70 | 5 | E / the_end.rs |
| owner / overworld | height_biome/ocean/p100 | 76 | 68 | 5 | A / terrain.rs |
| owner / overworld | height_biome/river/p95 | 71 | 63 | 5 | A / terrain.rs |
| owner / overworld | layer/deep_lukewarm_ocean/-1/dirt | 9.44481 | 1.44777 | 5 | A / terrain.rs |
| owner / overworld | biomes_3d/ocean | 3.67135 | 4.86531 | 1 | A / terrain.rs |
| owner / overworld | feature_biome/ocean/moss_block | 1.48027 | 31.237 | 6.2474 | A / terrain.rs |
| owner / overworld | layer/warm_ocean/-4/sandstone | 0 | 7.9318 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/bamboo_jungle/jungle_leaves | 3664.41 | 1879.35 | 375.869 | F-tree / trees.rs |
| owner / the_nether | depth/0/underground_lava | 8.07903 | 4.95548 | 2 | N / nether.rs |
| owner / overworld | biomes_3d/bamboo_jungle | 0.160351 | 1.3281 | 1 | A / terrain.rs |
| owner / overworld | surface_band/badlands/brown_terracotta/y64 | 1.16602 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | structure_box_composition/ruined_portal/water | 28.7719 | 17.1135 | 5 | R-ruined_portal / ruined_portal.rs |
| owner / overworld | blocks/short_grass | 11.4146 | 21.3887 | 4.27774 | F / vegetation.rs |
| owner / overworld | layer/savanna_plateau/-4/dirt | 3.05932 | 10.8256 | 5 | A / terrain.rs |
| owner / overworld | layer/lukewarm_ocean/-2/dirt | 10.6868 | 18.408 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/beach/jungle_leaves | 2.29026 | 31.0072 | 6.20144 | F-tree / trees.rs |
| owner / overworld | feature_biome/sparse_jungle/sculk | 23.1481 | 0 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/beach/-2/stone | 5.24127 | 12.9562 | 5 | A / terrain.rs |
| owner / overworld | layer/jungle/-1/dirt | 97.0051 | 89.323 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/moss_block | 2.10407 | 26.5516 | 5.31031 | A / terrain.rs |
| owner / the_end | window1/blocks/chorus_plant | 8.34343 | 12.0376 | 2.40753 | E / the_end.rs |
| owner / overworld | feature_biome/desert/sugar_cane | 28.1494 | 5.13478 | 5 | F / vegetation.rs |
| owner / overworld | layer/swamp/-5/stone | 89.8148 | 82.1434 | 5 | A / terrain.rs |
| owner / the_end | top/the_end/end_stone | 20.1756 | 27.7919 | 5 | E / the_end.rs |
| owner / the_end | top/the_end/air | 79.53 | 71.9294 | 5 | E / the_end.rs |
| owner / overworld | feature_biome/deep_lukewarm_ocean/cinnabar | 0 | 22.6851 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/beach/-1/stone | 5.30637 | 12.8671 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/meadow/azure_bluet | 10.7474 | 109.947 | 21.9894 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/tall_grass | 67.9557 | 35.7551 | 7.15103 | F / vegetation.rs |
| owner / overworld | layer/savanna/-1/stone | 10.5458 | 3.04282 | 5 | A / terrain.rs |
| owner / the_nether | tree_span/nether_wastes/crimson_stem/p50 | 5.5 | 10 | 2 | N / nether.rs |
| owner / overworld | feature_biome/plains/cinnabar | 0 | 22.4515 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/savanna/mossy_stone_bricks | 22.2438 | 0.006476 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_hills/oak_leaves | 304.866 | 161.362 | 32.2725 | F-tree / trees.rs |
| owner / overworld | top/forest/grass_block | 76.086 | 83.4798 | 5 | A / terrain.rs |
| owner / overworld | surface_band/badlands/brown_terracotta/y128 | 1.108 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | blocks/chiseled_deepslate | 0.22116 | 0 | 0.1 | F / vegetation.rs |
| owner / the_nether | ore/nether_gold_ore/y112 | 4.25069 | 2.95225 | 0.59045 | N / nether.rs |
| owner / overworld | blocks/oak_log | 2.84987 | 5.07866 | 1.01573 | F-tree / trees.rs |
| owner / overworld | height_mean/beach | 64.8646 | 59.3889 | 5 | A / terrain.rs |
| owner / overworld | layer/ocean/-3/stone | 57.0568 | 64.3349 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/savanna_plateau/budding_amethyst | 27.8747 | 6.07431 | 5 | F / vegetation.rs |
| owner / overworld | layer/bamboo_jungle/-1/dirt | 98.2625 | 91.0366 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/bamboo_jungle/moss_block | 17.7123 | 131.171 | 26.2341 | A / terrain.rs |
| owner / overworld | height_mean/old_growth_birch_forest | 73.0799 | 67.6807 | 5 | A / terrain.rs |
| owner / overworld | layer/deep_lukewarm_ocean/-2/dirt | 9.40749 | 2.22524 | 5 | A / terrain.rs |
| owner / overworld | layer/plains/-5/stone | 86.8927 | 79.7151 | 5 | A / terrain.rs |
| owner / the_end | layer/the_end/-1/end_stone | 20.0806 | 27.255 | 5 | E / the_end.rs |
| owner / the_end | layer/the_end/-1/air | 79.6475 | 72.4737 | 5 | E / the_end.rs |
| owner / overworld | top/birch_forest/stone | 8.2761 | 1.1113 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/meadow/cornflower | 11.949 | 84.0246 | 16.8049 | F / vegetation.rs |
| owner / overworld | blocks/cave_air | 31.3167 | 54.7408 | 10.9482 | A / terrain.rs |
| owner / overworld | layer/warm_ocean/-3/sand | 0.042451 | 7.1552 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_spacing_p95/abandoned_camp | 1181.92 | 772.959 | 193.24 | R-abandoned_camp / abandoned_camp.rs |
| owner / the_end | window2/blocks/end_stone_bricks | 1.40955 | 0 | 1 | E / the_end.rs |
| owner / overworld | layer/deep_lukewarm_ocean/-3/dirt | 9.33685 | 2.30214 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_spacing_p95/ocean_ruin | 328.38 | 692.418 | 173.105 | R-ocean_ruin / ocean_ruin.rs |
| owner / overworld | blocks/seagrass | 13.6808 | 9.63076 | 1.92615 | F-ocean / ocean.rs |
| owner / overworld | height_biome/plains/p100 | 160 | 153 | 5 | A / terrain.rs |
| owner / overworld | layer/ocean/-1/dirt | 13.9754 | 7.01621 | 5 | A / terrain.rs |
| owner / overworld | layer/beach/-2/sandstone | 0.026391 | 6.88648 | 5 | A / terrain.rs |
| owner / the_nether | blocks/red_mushroom | 0.121212 | 0.326905 | 0.1 | N / nether.rs |
| owner / overworld | feature_biome/meadow/sculk | 28.771 | 162.239 | 32.4478 | U / underground_carvers.rs |
| owner / overworld | surface_band/eroded_badlands/terracotta/y48 | 1.02188 | 0 | 0.25 | A / terrain.rs |
| owner / the_end | layer/the_end/-2/end_stone | 20.0431 | 26.8438 | 5 | E / the_end.rs |
| owner / the_end | layer/the_end/-2/air | 79.685 | 72.885 | 5 | E / the_end.rs |
| owner / overworld | top/plains/sand | 0.943976 | 7.74324 | 5 | A / terrain.rs |
| owner / overworld | height_mean/deep_ocean | 42.5561 | 37.4568 | 5 | A / terrain.rs |
| owner / overworld | blocks/infested_stone | 0 | 0.203603 | 0.1 | F / vegetation.rs |
| owner / the_nether | depth/64/air | 51.427 | 48.7165 | 2 | N / nether.rs |
| owner / overworld | feature_biome/mangrove_swamp/bone_block | 0 | 20.3016 | 5 | F / vegetation.rs |
| owner / overworld | blocks/rooted_dirt | 0.047813 | 0.249993 | 0.1 | A / terrain.rs |
| owner / overworld | blocks/short_dry_grass | 0.217858 | 0.016081 | 0.1 | F / vegetation.rs |
| owner / overworld | structure_candidate_count/mineshaft | 107 | 161 | 40.25 | R-mineshaft / mineshaft.rs |
| owner / overworld | blocks/grass_block | 71.7784 | 119.741 | 23.9483 | A / terrain.rs |
| owner / overworld | tree_span/birch_forest/oak_log/p50 | 1 | 5 | 2 | F-tree / trees.rs |
| owner / overworld | tree_span/desert/jungle_log/p50 | 15 | 11 | 2 | F-tree / trees.rs |
| owner / overworld | tree_span/lukewarm_ocean/spruce_log/p50 | 5 | 1 | 2 | F-tree / trees.rs |
| owner / overworld | blocks/tall_dry_grass | 0.215549 | 0.015813 | 0.1 | F / vegetation.rs |
| owner / overworld | top/plains/stone | 8.34533 | 1.68882 | 5 | A / terrain.rs |
| owner / overworld | blocks/cherry_leaves | 0 | 0.199656 | 0.1 | F-tree / trees.rs |
| owner / overworld | depth/-16/air | 8.36583 | 11.0253 | 2 | U / underground_carvers.rs |
| owner / overworld | layer/river/-5/stone | 76.3378 | 69.6898 | 5 | A / terrain.rs |
| owner / the_end | cave_shape_proxy/thin_like | 0.044238 | 0.243667 | 0.1 | E / the_end.rs |
| owner / overworld | depth/-16/underground_air | 8.36583 | 11.0212 | 2 | U / underground_carvers.rs |
| owner / overworld | layer/desert/-1/sand | 95.4512 | 88.8165 | 5 | A / terrain.rs |
| owner / overworld | layer/bamboo_jungle/-5/stone | 88.5773 | 81.9457 | 5 | A / terrain.rs |
| owner / overworld | structure_candidate_spacing_p50/village | 624.212 | 417.229 | 104.307 | R-village / village.rs |
| owner / overworld | feature_biome/jungle/bamboo | 0 | 19.8335 | 5 | F / vegetation.rs |
| owner / overworld | layer/birch_forest/-2/dirt | 89.4488 | 82.8394 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/mangrove_swamp/short_grass | 276.184 | 154.064 | 30.8128 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/poppy | 19.7592 | 0 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/river/moss_block | 0.02424 | 19.6789 | 5 | A / terrain.rs |
| owner / overworld | depth/64/air | 73.9968 | 76.5967 | 2 | U / underground_carvers.rs |
| owner / overworld | feature_biome/sulfur_caves/sulfur_spike | 614.743 | 2731.18 | 546.236 | U / underground_carvers.rs |
| owner / the_end | layer/the_end/-3/end_stone | 20.0069 | 26.4625 | 5 | E / the_end.rs |
| owner / the_end | layer/the_end/-3/air | 79.7212 | 73.2662 | 5 | E / the_end.rs |
| owner / overworld | blocks/coal_ore | 108.613 | 78.3216 | 15.6643 | U / underground.rs |
| owner / overworld | layer/lukewarm_ocean/-3/stone | 66.5091 | 60.0801 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_hills/birch_leaves | 0 | 19.2863 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/small_dripleaf | 0.285484 | 0.094096 | 0.1 | F / vegetation.rs |
| owner / overworld | top/mangrove_swamp/stone | 0.033167 | 6.40034 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/fire_coral_wall_fan | 111.134 | 63.0742 | 12.6148 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/bamboo_jungle/oak_log | 124.327 | 70.613 | 14.1226 | F-tree / trees.rs |
| owner / the_nether | structure_box_composition/nether_fossil/netherrack | 9.49254 | 0 | 5 | N / nether_fossil.rs |
| owner / the_nether | cave_component_volume/p50 | 37 | 67 | 16 | N / nether.rs |
| owner / overworld | cave_shape_proxy/other | 1.62461 | 2.59403 | 0.518806 | U / underground_carvers.rs |
| owner / overworld | surface_band/badlands/white_terracotta/y128 | 0.932884 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | layer/swamp/-2/dirt | 95.3704 | 89.1592 | 5 | A / terrain.rs |
| owner / overworld | layer/lukewarm_ocean/-4/stone | 73.1681 | 66.9648 | 5 | A / terrain.rs |
| owner / overworld | top/deep_lukewarm_ocean/sand | 89.0921 | 95.2855 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/savanna_plateau/sculk | 9.53851 | 36.9981 | 7.39961 | U / underground_carvers.rs |
| owner / overworld | feature_biome/warm_ocean/bubble_coral_wall_fan | 111.108 | 63.7815 | 12.7563 | F-ocean / ocean.rs |
| owner / overworld | top/plains/dirt | 1.81733 | 7.99996 | 5 | A / terrain.rs |
| owner / overworld | layer/mangrove_swamp/-2/stone | 12.2572 | 18.3862 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/horn_coral_wall_fan | 110.574 | 63.7815 | 12.7563 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/deep_ocean/kelp | 229.474 | 132.426 | 26.4852 | F-ocean / ocean.rs |
| owner / overworld | layer/plains/-1/sand | 0.942019 | 7.03921 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/lukewarm_ocean/moss_block | 2.51669 | 20.7945 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/plains/sculk | 18.2611 | 0 | 5 | U / underground_carvers.rs |
| owner / the_end | layer/the_end/-4/end_stone | 19.9725 | 26.0581 | 5 | E / the_end.rs |
| owner / the_end | layer/the_end/-4/air | 79.7556 | 73.6706 | 5 | E / the_end.rs |
| owner / overworld | feature_biome/sparse_jungle/leaf_litter | 19.2645 | 1.03649 | 5 | F / vegetation.rs |
| owner / the_nether | depth/0/lava | 8.13854 | 5.71047 | 2 | N / nether.rs |
| owner / overworld | feature_biome/windswept_savanna/mangrove_leaves | 25.6073 | 93.6151 | 18.723 | F-tree / trees.rs |
| owner / overworld | surface_band/badlands/yellow_terracotta/y96 | 0.907649 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | blocks/oak_planks | 3.23917 | 5.08038 | 1.01608 | F / vegetation.rs |
| owner / overworld | top/old_growth_birch_forest/grass_block | 94.1975 | 88.1639 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/birch_forest/dark_oak_leaves | 0 | 18.0796 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/stony_shore/snow_block | 18.064 | 0 | 5 | A / terrain.rs |
| owner / overworld | structure_box_composition/abandoned_camp/stone | 10.0559 | 1.05205 | 5 | R-abandoned_camp / abandoned_camp.rs |
| owner / overworld | surface_band/badlands/red_terracotta/y96 | 0.897313 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | structure_box_composition/ruined_portal/air | 36.3237 | 45.2634 | 5 | R-ruined_portal / ruined_portal.rs |
| owner / overworld | layer/mangrove_swamp/-1/stone | 3.03678 | 8.99469 | 5 | A / terrain.rs |
| owner / the_nether | depth/112/air | 2.34415 | 4.72705 | 2 | N / nether.rs |
| owner / the_end | depth/48/air | 74.4299 | 72.0516 | 2 | E / the_end.rs |
| owner / overworld | feature_biome/dripstone_caves/budding_amethyst | 109.026 | 63.6409 | 12.7282 | U / underground_carvers.rs |
| owner / overworld | layer/forest/-5/stone | 88.0855 | 82.153 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/warm_ocean/tube_coral_wall_fan | 110.984 | 64.8844 | 12.9769 | F-ocean / ocean.rs |
| owner / overworld | top/lukewarm_ocean/sand | 88.9514 | 83.0342 | 5 | A / terrain.rs |
| owner / overworld | blocks/brain_coral_fan | 0.209563 | 0.03235 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/dripstone_caves/moss_block | 0 | 17.6904 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/warm_ocean/-3/stone | 67.3991 | 61.5119 | 5 | A / terrain.rs |
| owner / overworld | blocks/tube_coral | 0.20763 | 0.031357 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/prismarine | 0.36205 | 0.185884 | 0.1 | F / vegetation.rs |
| owner / overworld | top/desert/sand | 95.8541 | 89.989 | 5 | A / terrain.rs |
| owner / overworld | blocks/horn_coral | 0.20669 | 0.030981 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/brain_coral | 0.206556 | 0.030954 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/warm_ocean/brain_coral_wall_fan | 110.652 | 65.0137 | 13.0027 | F-ocean / ocean.rs |
| owner / overworld | tree_span/river/mangrove_log/p50 | 1.5 | 5 | 2 | F-tree / trees.rs |
| owner / overworld | blocks/fire_coral | 0.206314 | 0.03133 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | tree_base_estimates/old_growth_birch_forest/birch_log | 0.04937 | 0.223657 | 0.1 | F-tree / trees.rs |
| owner / overworld | ore/deepslate_lapis_ore/y0 | 1.64466 | 1.20951 | 0.25 | U / underground.rs |
| owner / overworld | blocks/bubble_coral_fan | 0.20618 | 0.032135 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/bubble_coral | 0.205697 | 0.031679 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/fire_coral_fan | 0.204193 | 0.030524 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | blocks/tube_coral_fan | 0.205965 | 0.032377 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | ore/iron_ore/y112 | 0.457865 | 0.024457 | 0.25 | U / underground.rs |
| owner / overworld | layer/savanna_plateau/-4/stone | 89.5292 | 83.7796 | 5 | A / terrain.rs |
| owner / the_end | layer/the_end/-5/air | 79.81 | 74.0919 | 5 | E / the_end.rs |
| owner / the_end | layer/the_end/-5/end_stone | 19.9187 | 25.6369 | 5 | E / the_end.rs |
| owner / overworld | layer/birch_forest/-1/stone | 9.04018 | 3.32471 | 5 | A / terrain.rs |
| owner / overworld | blocks/horn_coral_fan | 0.203173 | 0.032135 | 0.1 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/dripstone_caves/amethyst_block | 1160.92 | 689.839 | 137.968 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lush_caves/cave_vines_plant | 2450.45 | 1456.14 | 291.228 | F / vegetation.rs |
| owner / overworld | top/warm_ocean/sand | 89.8959 | 95.5662 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/mangrove_swamp/muddy_mangrove_roots | 188.839 | 586.457 | 117.291 | F / vegetation.rs |
| owner / the_nether | blocks/fire | 0.918274 | 1.38567 | 0.277135 | N / nether.rs |
| owner / the_end | feature_biome/end_barrens/chorus_plant | 16.8326 | 0 | 5 | E / the_end.rs |
| owner / overworld | mountain_local_peaks_ge128 | 0.012794 | 0.001573 | 0.01 | A / terrain.rs |
| owner / overworld | top/desert/gravel | 2.80078 | 8.38169 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/beach/oak_leaves | 12.5426 | 37.9506 | 7.59012 | F-tree / trees.rs |
| owner / overworld | layer/ocean/-4/stone | 73.3833 | 67.8188 | 5 | A / terrain.rs |
| owner / overworld | blocks/acacia_log | 0.405407 | 0.608473 | 0.121695 | F-tree / trees.rs |
| owner / overworld | ore/deepslate_iron_ore/y-48 | 5.60807 | 4.20481 | 0.840962 | U / underground.rs |
| owner / overworld | surface_band/badlands/orange_terracotta/y80 | 0.834143 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_hills/mossy_cobblestone | 0 | 16.6654 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/mangrove_swamp/tall_seagrass | 23.3247 | 69.5482 | 13.9096 | F-ocean / ocean.rs |
| owner / the_nether | top/soul_sand_valley/soul_soil | 26.8179 | 32.3132 | 5 | N / nether.rs |
| owner / overworld | feature_biome/sparse_jungle/jungle_log | 89.3219 | 54.0927 | 10.8185 | F-tree / trees.rs |
| owner / overworld | layer/forest/-2/dirt | 79.2687 | 84.6786 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/windswept_hills/oak_log | 25.612 | 9.39591 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/bamboo_jungle/jungle_log | 1020.84 | 619.992 | 123.998 | F-tree / trees.rs |
| owner / overworld | layer/ocean/-1/stone | 56.976 | 62.3629 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/jungle/birch_leaves | 18.915 | 2.77615 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/cave_vines | 1.48302 | 1.12245 | 0.224489 | F / vegetation.rs |
| owner / overworld | cave_component_volume/p99 | 7157.64 | 5418 | 1083.6 | U / underground_carvers.rs |
| owner / the_nether | layer/soul_sand_valley/-1/soul_soil | 25.1055 | 30.4041 | 5 | N / nether.rs |
| owner / overworld | top/sparse_jungle/sand | 1.16557 | 6.45778 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/birch_forest/pointed_dripstone | 0 | 15.8319 | 5 | U / underground_carvers.rs |
| owner / the_nether | blocks/polished_blackstone_brick_stairs | 0.157943 | 0 | 0.1 | N / nether.rs |
| owner / overworld | blocks/smooth_sandstone | 0.157776 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | blocks/acacia_leaves | 3.48106 | 5.07224 | 1.01445 | F-tree / trees.rs |
| owner / overworld | layer/warm_ocean/-2/stone | 67.3317 | 62.1206 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/savanna_plateau/mossy_cobblestone | 10.372 | 27.5001 | 5.50001 | F / vegetation.rs |
| owner / the_nether | layer/soul_sand_valley/-5/netherrack | 77.7653 | 72.6149 | 5 | N / nether.rs |
| owner / overworld | cave_shape_proxy/thin_like | 1.78364 | 2.57687 | 0.515375 | U / underground_carvers.rs |
| owner / overworld | top/ocean/sand | 11.0201 | 5.89112 | 5 | A / terrain.rs |
| owner / overworld | blocks/prismarine_bricks | 0.33021 | 0.176703 | 0.1 | F / vegetation.rs |
| owner / overworld | layer/ocean/-2/stone | 57.0179 | 62.1283 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/lush_caves/dripstone_block | 0 | 15.2401 | 5 | U / underground_carvers.rs |
| owner / overworld | layer/plains/-1/dirt | 89.4103 | 84.3714 | 5 | A / terrain.rs |
| owner / the_nether | cave_shape_proxy/other | 0.306703 | 0.4573 | 0.1 | N / nether.rs |
| owner / overworld | tree_span/lukewarm_ocean/oak_log/p50 | 1 | 4 | 2 | F-tree / trees.rs |
| owner / overworld | tree_span/windswept_savanna/mangrove_log/p50 | 1 | 4 | 2 | F-tree / trees.rs |
| owner / overworld | ore/deepslate_copper_ore/y0 | 3.47932 | 2.67744 | 0.535488 | U / underground.rs |
| owner / overworld | feature_biome/forest/pointed_dripstone | 0.022199 | 14.9833 | 5 | U / underground_carvers.rs |
| owner / overworld | surface_band/badlands/light_gray_terracotta/y96 | 0.747242 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/swamp/short_grass | 331.549 | 207.955 | 41.5909 | F / vegetation.rs |
| owner / overworld | feature_biome/deep_lukewarm_ocean/moss_block | 8.69802 | 23.4453 | 5 | A / terrain.rs |
| owner / the_end | blocks/purpur_slab | 0.146272 | 0 | 0.1 | E / the_end.rs |
| owner / overworld | feature_biome/swamp/oak_leaves | 837.915 | 1991.84 | 398.368 | F-tree / trees.rs |
| owner / the_end | feature_biome/end_highlands/chorus_plant | 290.006 | 688.533 | 137.707 | E / the_end.rs |
| owner / overworld | blocks/oak_fence | 2.18664 | 3.06207 | 0.612414 | F / vegetation.rs |
| owner / overworld | ore/gold_ore/y32 | 0.357406 | 0.000698 | 0.25 | U / underground.rs |
| owner / overworld | feature_biome/jungle/amethyst_block | 161.275 | 102.696 | 20.5392 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/wildflowers | 65.9531 | 153.3 | 30.66 | F / vegetation.rs |
| owner / overworld | feature_biome/river/tall_seagrass | 119.136 | 276.462 | 55.2924 | F-ocean / ocean.rs |
| owner / the_nether | feature_biome/crimson_forest/crimson_stem | 945.158 | 602.397 | 120.479 | N / nether.rs |
| owner / overworld | blocks/blue_ice | 0.142098 | 0 | 0.1 | A / terrain.rs |
| owner / overworld | blocks/tall_grass | 1.30922 | 1.82714 | 0.365427 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/sea_pickle | 43.7866 | 27.9966 | 5.59933 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/plains/pointed_dripstone | 0.51 | 14.597 | 5 | U / underground_carvers.rs |
| owner / overworld | ore/gold_ore/y48 | 0.351607 | 0 | 0.25 | U / underground.rs |
| owner / overworld | blocks/coarse_dirt | 0.002013 | 0.142635 | 0.1 | A / terrain.rs |
| owner / overworld | feature_biome/sparse_jungle/jungle_leaves | 496.636 | 318.112 | 63.6224 | F-tree / trees.rs |
| owner / overworld | feature_biome/river/seagrass | 97.5819 | 221.538 | 44.3076 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/sparse_jungle/dripstone_block | 0 | 13.8198 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/beach/leaf_litter | 0.968363 | 14.7071 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/savanna_plateau/oak_log | 5.74162 | 19.4378 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/rail | 0.833365 | 1.14564 | 0.229128 | F / vegetation.rs |
| owner / overworld | feature_biome/lukewarm_ocean/kelp_plant | 713.736 | 462.842 | 92.5683 | F-ocean / ocean.rs |
| owner / overworld | blocks/sunflower | 0.135413 | 0 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/budding_amethyst | 0 | 13.5403 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/dandelion | 17.8901 | 38.8837 | 7.77674 | F / vegetation.rs |
| owner / overworld | feature_biome/mangrove_swamp/mangrove_roots | 560.914 | 1218.9 | 243.78 | F / vegetation.rs |
| owner / overworld | feature_biome/savanna_plateau/oak_leaves | 53.8046 | 115.743 | 23.1486 | F-tree / trees.rs |
| owner / the_nether | blocks/glowstone | 2.41414 | 3.29477 | 0.658953 | N / nether.rs |
| owner / overworld | feature_biome/plains/mangrove_leaves | 0 | 13.28 | 5 | F-tree / trees.rs |
| owner / overworld | ore/iron_ore/y-16 | 0 | 0.33166 | 0.25 | U / underground.rs |
| owner / overworld | blocks/water | 2474.65 | 1958.02 | 391.604 | A / terrain.rs |
| owner / overworld | covered_grass_by/oak_leaves | 1.24918 | 3.88151 | 2 | A / terrain.rs |
| owner / overworld | surface_band/eroded_badlands/red_terracotta/y64 | 0.65685 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | ore/iron_ore/y96 | 0.456308 | 0.127923 | 0.25 | U / underground.rs |
| owner / overworld | feature_biome/river/mangrove_leaves | 4.18388 | 17.2682 | 5 | F-tree / trees.rs |
| owner / the_end | cave_component_volume/p99 | 7743.05 | 10481 | 2096.19 | E / the_end.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/dark_oak_leaves | 0 | 13.042 | 5 | F-tree / trees.rs |
| owner / the_nether | blocks/brown_mushroom | 0.184573 | 0.314968 | 0.1 | N / nether.rs |
| owner / overworld | ore/copper_ore/y48 | 19.1741 | 15.2504 | 3.05007 | U / underground.rs |
| owner / overworld | blocks/lily_pad | 0.030605 | 0.157078 | 0.1 | F / vegetation.rs |
| owner / overworld | blocks/bone_block | 0.007624 | 0.133883 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/forest/short_grass | 30.8715 | 62.279 | 12.4558 | F / vegetation.rs |
| owner / overworld | blocks/flowering_azalea_leaves | 0.134661 | 0.008618 | 0.1 | F-tree / trees.rs |
| owner / overworld | feature_biome/jungle/dripstone_block | 0 | 12.4683 | 5 | U / underground_carvers.rs |
| owner / overworld | structure_box_composition/abandoned_camp/grass_block | 3.20298 | 9.3577 | 5 | R-abandoned_camp / abandoned_camp.rs |
| owner / the_end | feature_biome/end_highlands/chorus_flower | 44.3813 | 87.1931 | 17.4386 | E / the_end.rs |
| owner / overworld | blocks/orange_tulip | 0.122768 | 8.1e-05 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/mangrove_swamp/amethyst_block | 173.009 | 116.082 | 23.2164 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/mangrove_log | 0 | 12.2267 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/sculk_sensor | 0.124701 | 0.003034 | 0.1 | U / underground_carvers.rs |
| owner / overworld | feature_biome/swamp/tall_seagrass | 0 | 12.1256 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/beach/dark_oak_leaves | 0 | 12.1139 | 5 | F-tree / trees.rs |
| owner / overworld | blocks/cobweb | 0.745282 | 0.983382 | 0.196676 | F / vegetation.rs |
| owner / overworld | ore/copper_ore/y96 | 0.367795 | 0.066472 | 0.25 | U / underground.rs |
| owner / the_nether | feature_biome/crimson_forest/weeping_vines | 162.13 | 109.468 | 21.8936 | N / nether.rs |
| owner / overworld | structure_box_composition/shipwreck/water | 58.8377 | 52.8636 | 5 | R-shipwreck / shipwreck.rs |
| owner / overworld | feature_biome/savanna_plateau/pointed_dripstone | 0 | 11.9277 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/lush_caves/short_grass | 788.241 | 1501.45 | 300.289 | F / vegetation.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/leaf_litter | 23.6563 | 11.8446 | 5 | F / vegetation.rs |
| owner / overworld | tree_base_estimates/birch_forest/birch_log | 0.006014 | 0.123037 | 0.1 | F-tree / trees.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/jungle_leaves | 12.4215 | 0.731327 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/sulfur_caves/dripstone_block | 0 | 11.5819 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/sparse_jungle/amethyst_block | 232.707 | 159.093 | 31.8187 | F / vegetation.rs |
| owner / overworld | object_geometry_proxy/bone_body/volume_p95 | 1 | 10.2 | 4 | F / vegetation.rs |
| owner / overworld | structure_candidate_spacing_p95/mineshaft | 321.696 | 249.928 | 62.482 | R-mineshaft / mineshaft.rs |
| owner / overworld | feature_biome/meadow/leaf_litter | 24.4988 | 45.1409 | 9.02818 | F / vegetation.rs |
| owner / overworld | feature_biome/jungle/oak_log | 88.6016 | 162.272 | 32.4544 | F-tree / trees.rs |
| owner / overworld | feature_biome/bamboo_jungle/oak_leaves | 1858.26 | 1282.57 | 256.514 | F-tree / trees.rs |
| owner / overworld | blocks/dirt_path | 0.140541 | 0.252222 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/short_grass | 139.583 | 251.627 | 50.3254 | F / vegetation.rs |
| owner / the_nether | blocks/gilded_blackstone | 0.111111 | 0 | 0.1 | N / nether.rs |
| owner / the_end | blocks/magenta_stained_glass | 0.110239 | 0 | 0.1 | E / the_end.rs |
| owner / the_nether | feature_biome/crimson_forest/crimson_roots | 434.553 | 301.735 | 60.347 | N / nether.rs |
| owner / overworld | feature_biome/bamboo_jungle/moss_carpet | 0 | 11.0011 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/beach/ice | 10.9748 | 0 | 5 | A / terrain.rs |
| owner / overworld | tree_base_estimates/flower_forest/oak_log | 0.109453 | 0 | 0.1 | F-tree / trees.rs |
| owner / overworld | feature_biome/jungle/oak_leaves | 1988.68 | 3529.48 | 705.897 | F-tree / trees.rs |
| owner / overworld | feature_biome/lush_caves/big_dripleaf | 80.4768 | 142.531 | 28.5062 | F / vegetation.rs |
| owner / overworld | ore/iron_ore/y0 | 12.4966 | 15.9682 | 3.19365 | U / underground.rs |
| owner / overworld | feature_biome/sparse_jungle/bamboo | 0 | 10.8456 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/deep_dark/mossy_cobblestone | 47.3893 | 83.1749 | 16.635 | F / vegetation.rs |
| owner / overworld | feature_biome/bamboo_jungle/short_grass | 727.398 | 509.466 | 101.893 | F / vegetation.rs |
| owner / overworld | blocks/small_amethyst_bud | 0.16285 | 0.055921 | 0.1 | F / vegetation.rs |
| owner / overworld | ore/gold_ore/y64 | 0.26696 | 0 | 0.25 | U / underground.rs |
| owner / overworld | feature_biome/birch_forest/amethyst_block | 57.4851 | 100.183 | 20.0366 | F / vegetation.rs |
| owner / overworld | blocks/medium_amethyst_bud | 0.160622 | 0.05525 | 0.1 | F / vegetation.rs |
| owner / overworld | blocks/large_amethyst_bud | 0.161615 | 0.056592 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_hills/powder_snow | 0 | 10.4839 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/bamboo_jungle/dark_oak_leaves | 0 | 10.4459 | 5 | F-tree / trees.rs |
| owner / overworld | surface_band/eroded_badlands/brown_terracotta/y48 | 0.521303 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | blocks/spore_blossom | 0.128111 | 0.024135 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_hills/budding_amethyst | 14.8715 | 4.50015 | 5 | F / vegetation.rs |
| owner / overworld | ore/coal_ore/y16 | 19.4583 | 16.1568 | 3.23136 | U / underground.rs |
| owner / overworld | feature_biome/deep_ocean/mossy_stone_bricks | 10.1882 | 0 | 5 | F / vegetation.rs |
| owner / overworld | blocks/gold_ore | 4.59336 | 3.81809 | 0.763618 | U / underground.rs |
| owner / overworld | surface_band/eroded_badlands/yellow_terracotta/y96 | 0.50745 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/jungle/mangrove_leaves | 0 | 10.1197 | 5 | F-tree / trees.rs |
| owner / overworld | tree_base_estimates/dark_forest/dark_oak_log | 0 | 0.101157 | 0.1 | F-tree / trees.rs |
| owner / overworld | feature_biome/savanna_plateau/short_grass | 533.416 | 895.132 | 179.026 | F / vegetation.rs |
| owner / overworld | structure_box_composition/abandoned_camp/air | 55.0279 | 60.0775 | 5 | R-abandoned_camp / abandoned_camp.rs |
| owner / overworld | blocks/amethyst_cluster | 0.158045 | 0.057075 | 0.1 | F / vegetation.rs |
| owner / overworld | feature_biome/jungle/cocoa | 1.61773 | 11.6241 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/dripstone_caves/dripstone_block | 8825.03 | 14683.2 | 2936.64 | U / underground_carvers.rs |
| owner / overworld | feature_biome/dripstone_caves/powder_snow | 0 | 9.97218 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/river/pointed_dripstone | 0.135746 | 10.0181 | 5 | U / underground_carvers.rs |
| owner / the_nether | feature_biome/crimson_forest/crimson_fungus | 51.2686 | 37.1091 | 7.42182 | N / nether.rs |
| owner / overworld | feature_biome/lush_caves/tall_grass | 353.407 | 570.863 | 114.173 | F / vegetation.rs |
| owner / overworld | feature_biome/river/oak_leaves | 36.8938 | 26.7548 | 5.35097 | F-tree / trees.rs |
| owner / overworld | feature_biome/meadow/spruce_leaves | 9.27882 | 0 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/savanna/sculk | 11.7599 | 2.58809 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/meadow/oak_leaves | 37.3155 | 58.5491 | 11.7098 | F-tree / trees.rs |
| owner / overworld | surface_band/badlands/terracotta/y144 | 0.446213 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/desert/sculk | 8.8979 | 0 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_savanna/amethyst_block | 87.339 | 135.252 | 27.0504 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_hills/short_grass | 23.1335 | 35.655 | 7.131 | F / vegetation.rs |
| owner / the_nether | feature_biome/crimson_forest/weeping_vines_plant | 359.258 | 265.913 | 53.1825 | N / nether.rs |
| owner / overworld | feature_biome/bamboo_jungle/dripstone_block | 0 | 8.71866 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/warm_ocean/oak_log | 1.33408 | 10.0395 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/mossy_cobblestone | 1.62192 | 10.3103 | 5 | F / vegetation.rs |
| owner / overworld | surface_band/eroded_badlands/white_terracotta/y48 | 0.432253 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/swamp/oak_log | 66.3098 | 101.249 | 20.2498 | F-tree / trees.rs |
| owner / overworld | feature_biome/windswept_savanna/mangrove_roots | 1.29112 | 9.89209 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/desert/mangrove_leaves | 8.43899 | 0 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/river/amethyst_block | 168.208 | 125.939 | 25.1877 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_hills/acacia_leaves | 8.26195 | 0 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/windswept_hills/tall_grass | 8.26195 | 0 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/stony_shore/moss_block | 0 | 8.19561 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/sparse_jungle/oak_log | 23.4547 | 34.8801 | 6.97602 | F-tree / trees.rs |
| owner / overworld | surface_band/badlands/orange_terracotta/y96 | 0.406669 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/swamp/pointed_dripstone | 0 | 8.08376 | 5 | U / underground_carvers.rs |
| owner / overworld | surface_band/badlands/orange_terracotta/y64 | 0.399662 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/mangrove_swamp/mangrove_leaves | 7965.32 | 11690.4 | 2338.07 | F-tree / trees.rs |
| owner / overworld | surface_band/eroded_badlands/red_terracotta/y96 | 0.396977 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/desert/oak_log | 1.44067 | 9.31178 | 5 | F-tree / trees.rs |
| owner / the_nether | feature_biome/soul_sand_valley/bone_block | 20.9822 | 30.6186 | 6.12373 | N / nether.rs |
| owner / overworld | feature_biome/river/jungle_leaves | 2.72461 | 10.5644 | 5 | F-tree / trees.rs |
| owner / overworld | surface_band/badlands/brown_terracotta/y80 | 0.390937 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/deep_dark/packed_ice | 7.77799 | 0 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/lush_caves/mossy_cobblestone | 30.677 | 44.5247 | 8.90493 | F / vegetation.rs |
| owner / overworld | feature_biome/mangrove_swamp/mangrove_log | 500.886 | 726.113 | 145.223 | F-tree / trees.rs |
| owner / overworld | feature_biome/beach/mangrove_roots | 0 | 7.75312 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/brain_coral | 16.4465 | 8.69333 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/warm_ocean/horn_coral | 16.4465 | 8.72376 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/meadow/oak_log | 0.333771 | 8.04491 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/warm_ocean/tube_coral | 16.513 | 8.80742 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/lush_caves/moss_carpet | 983.656 | 752.223 | 150.445 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/fire_coral_fan | 16.2471 | 8.57925 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/warm_ocean/fire_coral | 16.4229 | 8.82263 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/warm_ocean/brain_coral_fan | 16.6674 | 9.11165 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/warm_ocean/bubble_coral | 16.3886 | 8.86826 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/forest/amethyst_block | 152.733 | 117.533 | 23.5065 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/birch_log | 9.67934 | 2.2347 | 5 | F-tree / trees.rs |
| owner / overworld | object_geometry_proxy/amethyst_body/volume_p95 | 323 | 459 | 91.8 | U / underground.rs |
| owner / overworld | feature_biome/deep_ocean/amethyst_block | 86.5397 | 122.803 | 24.5605 | F / vegetation.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/amethyst_block | 127.341 | 98.3778 | 19.6756 | F / vegetation.rs |
| owner / overworld | feature_biome/beach/sugar_cane | 9.09954 | 1.74114 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/bubble_coral_fan | 16.4015 | 9.0508 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/stony_shore/mossy_cobblestone | 10.889 | 3.55143 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/tube_coral_fan | 16.3693 | 9.03559 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/deep_ocean/seagrass | 107.545 | 83.263 | 16.6526 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/swamp/moss_carpet | 0 | 7.27538 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/seagrass | 18.0845 | 25.4638 | 5.09277 | F-ocean / ocean.rs |
| owner / the_end | feature_biome/small_end_islands/chorus_plant | 7.18427 | 0 | 5 | E / the_end.rs |
| owner / overworld | feature_biome/savanna_plateau/tall_grass | 31.4863 | 44.1768 | 8.83536 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/horn_coral_fan | 16.172 | 9.01277 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/mangrove_swamp/oak_leaves | 0.140088 | 7.27165 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/beach/birch_leaves | 2.88972 | 9.98644 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/jungle/mossy_stone_bricks | 0 | 6.94848 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/bamboo_jungle/fern | 201.307 | 158.17 | 31.634 | F / vegetation.rs |
| owner / overworld | feature_biome/birch_forest/spruce_leaves | 7.75593 | 1.05057 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/stony_shore/kelp_plant | 0 | 6.69308 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/cave_vines_plant | 9.49418 | 2.81059 | 5 | F / vegetation.rs |
| owner / overworld | surface_band/eroded_badlands/light_gray_terracotta/y96 | 0.334103 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/savanna/dripstone_block | 0 | 6.66989 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/sparse_jungle/budding_amethyst | 20.3376 | 13.8349 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/river/leaf_litter | 13.977 | 7.48427 | 5 | F / vegetation.rs |
| owner / overworld | surface_band/badlands/red_terracotta/y48 | 0.3217 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | surface_band/eroded_badlands/brown_terracotta/y64 | 0.318801 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/desert/jungle_leaves | 1.05135 | 7.3696 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/bamboo_jungle/amethyst_block | 54.4995 | 72.916 | 14.5832 | F / vegetation.rs |
| owner / overworld | feature_biome/deep_dark/oak_log | 7.37027 | 1.11395 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/mangrove_swamp/budding_amethyst | 16.8806 | 10.6614 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/warm_ocean/amethyst_block | 153.057 | 122.597 | 24.5193 | F / vegetation.rs |
| owner / overworld | feature_biome/sparse_jungle/cave_vines_plant | 0 | 6.20391 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/jungle/jungle_log | 935.046 | 749.246 | 149.849 | F-tree / trees.rs |
| owner / overworld | feature_biome/sparse_jungle/moss_carpet | 0 | 6.14382 | 5 | F / vegetation.rs |
| owner / overworld | surface_band/eroded_badlands/orange_terracotta/y80 | 0.303042 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/lush_caves/budding_amethyst | 59.2515 | 47.8944 | 9.57889 | F / vegetation.rs |
| owner / overworld | feature_biome/stony_shore/ice | 5.90878 | 0 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/lush_caves/azalea | 160.789 | 210.47 | 42.0941 | F / vegetation.rs |
| owner / overworld | feature_biome/beach/short_grass | 3.65826 | 9.51014 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/meadow/snow | 5.80761 | 0 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/river/dark_oak_leaves | 0 | 5.76106 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/mangrove_swamp/seagrass | 28.3679 | 36.8258 | 7.36516 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/deep_ocean/budding_amethyst | 8.24045 | 13.8074 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/windswept_savanna/sugar_cane | 5.75625 | 0.269784 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/dead_bush | 0 | 5.45654 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/plains/amethyst_block | 173.665 | 142.659 | 28.5317 | F / vegetation.rs |
| owner / overworld | feature_biome/swamp/firefly_bush | 0 | 5.35549 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/birch_forest/dark_oak_log | 0 | 5.35058 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/sulfur_caves/mossy_cobblestone | 33.7869 | 42.9858 | 8.59716 | U / underground_carvers.rs |
| owner / overworld | feature_biome/old_growth_birch_forest/moss_block | 18.7906 | 24.0908 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/lush_caves/amethyst_block | 654.491 | 540.079 | 108.016 | F / vegetation.rs |
| owner / overworld | feature_biome/lush_caves/cave_vines | 996.051 | 822.532 | 164.506 | F / vegetation.rs |
| owner / overworld | feature_biome/savanna/acacia_leaves | 474.008 | 391.505 | 78.301 | F-tree / trees.rs |
| owner / overworld | feature_biome/swamp/blue_orchid | 0 | 5.25444 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/dripstone_caves/oak_log | 0 | 5.24222 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/beach/jungle_log | 0.030742 | 5.26577 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/meadow/oxeye_daisy | 5.20682 | 0 | 5 | F / vegetation.rs |
| owner / overworld | feature_biome/sparse_jungle/powder_snow | 0 | 5.19746 | 5 | A / terrain.rs |
| owner / overworld | feature_biome/sulfur_caves/sulfur | 423113 | 350522 | 70104.4 | U / underground_carvers.rs |
| owner / overworld | feature_biome/windswept_savanna/seagrass | 10.9207 | 5.7554 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/beach/kelp_plant | 0 | 5.1123 | 5 | F-ocean / ocean.rs |
| owner / overworld | feature_biome/sulfur_caves/moss_block | 16.0043 | 10.8952 | 5 | U / underground_carvers.rs |
| owner / overworld | feature_biome/river/sculk | 5.09532 | 0 | 5 | U / underground_carvers.rs |
| owner / overworld | surface_band/eroded_badlands/orange_terracotta/y112 | 0.252946 | 0 | 0.25 | A / terrain.rs |
| owner / overworld | feature_biome/deep_ocean/mangrove_leaves | 0 | 5.02089 | 5 | F-tree / trees.rs |
| owner / overworld | feature_biome/windswept_savanna/mossy_cobblestone | 8.3923 | 13.3993 | 5 | F / vegetation.rs |
| second / overworld | blocks/packed_ice | 83.6309 | 0.014336 | 0.1 | A / terrain.rs |
| second / overworld | object_geometry_proxy/ice_body/volume_p95 | 4926.65 | 1 | 4 | F / vegetation.rs |
| second / the_nether | blocks/warped_wart_block | 29.5822 | 0 | 0.1 | N / nether.rs |
| second / overworld | patches/mangrove_swamp/p50 | 72 | 12000 | 64 | A / terrain.rs |
| second / the_nether | blocks/warped_nylium | 23.2479 | 0 | 0.1 | N / nether.rs |
| second / overworld | slope/p100 | 55 | 171 | 1 | A / terrain.rs |
| second / overworld | spawn_offset | 705.634 | 0 | 16 | A / spawn_search.rs |
| second / the_nether | biomes_3d/soul_sand_valley | 0 | 64.9392 | 1 | N / nether.rs |
| second / the_nether | surface_biomes/soul_sand_valley | 0 | 64.9392 | 1 | N / nether.rs |
| second / overworld | patches/eroded_badlands/p50 | 24 | 5456 | 64 | A / terrain.rs |
| second / the_nether | blocks/polished_blackstone_bricks | 11.6152 | 0.009183 | 0.1 | N / nether.rs |
| second / the_nether | biomes_3d/crimson_forest | 72.5321 | 14.9793 | 1 | N / nether.rs |
| second / the_nether | surface_biomes/crimson_forest | 72.5321 | 14.9793 | 1 | N / nether.rs |
| second / the_end | patches/end_highlands/p50 | 256 | 5120 | 64 | E / the_end.rs |
| second / overworld | structure_candidate_offset_p95/stronghold | 3476.86 | 0 | 32 | R-stronghold / stronghold.rs |
| second / overworld | structure_candidate_offset_p95/pillager_outpost | 3268.27 | 0 | 32 | R-pillager_outpost / pillager_outpost.rs |
| second / the_end | slope/p100 | 83 | 131 | 1 | E / the_end.rs |
| second / overworld | patches/savanna_plateau/p50 | 16 | 3664 | 64 | A / terrain.rs |
| second / overworld | structure_candidate_offset_p95/buried_treasure | 2067.4 | 0 | 32 | R-shipwreck / shipwreck.rs |
| second / the_end | depth/96/underground_air | 0 | 82.2134 | 2 | E / the_end.rs |
| second / the_nether | blocks/warped_stem | 5.8393 | 0 | 0.1 | N / nether.rs |
| second / overworld | nominal_climate_columns/[0.5, 0.5] | 64.1901 | 39.6326 | 1 | A / terrain.rs |
| second / overworld | structure_candidate_offset_p95/desert_pyramid | 1556.6 | 0 | 32 | R-temples / temples.rs |
| second / the_nether | blocks/cracked_polished_blackstone_bricks | 4.2764 | 0.00551 | 0.1 | N / nether.rs |
| second / overworld | structure_candidate_offset_p95/monument | 1298.17 | 0 | 32 | R-monument / monument.rs |
| second / overworld | patches/meadow/p50 | 16 | 1632 | 64 | A / terrain.rs |
| second / overworld | patches/jungle/p50 | 48 | 1488 | 64 | A / terrain.rs |
| second / the_nether | blocks/crimson_nylium | 179.294 | 23.5308 | 4.70615 | N / nether.rs |
| second / overworld | patches/deep_ocean/p50 | 16 | 1392 | 64 | A / terrain.rs |
| second / overworld | feature_biome/sunflower_plains/oak_leaves | 343.233 | 20.8586 | 5 | F-tree / trees.rs |
| second / the_nether | blocks/shroomlight | 8.59229 | 1.15427 | 0.230854 | N / nether.rs |
| second / the_nether | blocks/crimson_stem | 49.82 | 6.73737 | 1.34747 | N / nether.rs |
| second / overworld | structure_candidate_coincident/abandoned_camp | 0 | 100 | 5 | R-abandoned_camp / abandoned_camp.rs |
| second / overworld | structure_candidate_coincident/ancient_city | 0 | 100 | 5 | R-ancient_city / ancient_city.rs |
| second / the_nether | structure_candidate_coincident/bastion_remnant | 0 | 100 | 5 | N / nether_bastion.rs |
| second / overworld | structure_candidate_coincident/buried_treasure | 0 | 100 | 5 | R-shipwreck / shipwreck.rs |
| second / overworld | structure_candidate_coincident/desert_pyramid | 0 | 100 | 5 | R-temples / temples.rs |
| second / overworld | structure_candidate_coincident/monument | 0 | 100 | 5 | R-monument / monument.rs |
| second / overworld | structure_candidate_coincident/ocean_ruin | 0 | 100 | 5 | R-ocean_ruin / ocean_ruin.rs |
| second / overworld | structure_candidate_coincident/pillager_outpost | 0 | 100 | 5 | R-pillager_outpost / pillager_outpost.rs |
| second / overworld | structure_candidate_coincident/ruined_portal | 0 | 100 | 5 | R-ruined_portal / ruined_portal.rs |
| second / the_nether | structure_candidate_coincident/ruined_portal | 0 | 100 | 5 | N / ruined_portal.rs |
| second / overworld | structure_candidate_coincident/shipwreck | 0 | 100 | 5 | R-shipwreck / shipwreck.rs |
| second / overworld | structure_candidate_coincident/stronghold | 0 | 100 | 5 | R-stronghold / stronghold.rs |
| second / overworld | structure_candidate_coincident/swamp_hut | 0 | 100 | 5 | R-temples / temples.rs |
| second / overworld | structure_candidate_coincident/trial_chambers | 0 | 100 | 5 | R-trial_chambers / trial_chambers.rs |
| second / overworld | structure_candidate_coincident/village | 0 | 100 | 5 | R-village / village.rs |
| second / the_end | feature_biome/end_midlands/chorus_plant | 313.784 | 14.0587 | 5 | E / the_end.rs |
| second / the_nether | blocks/nether_wart_block | 251.49 | 36.0046 | 7.20092 | N / nether.rs |
| second / overworld | structure_candidate_coincident/mineshaft | 0.757576 | 100 | 5 | R-mineshaft / mineshaft.rs |
| second / the_nether | blocks/weeping_vines | 8.94674 | 1.32782 | 0.265565 | N / nether.rs |
| second / overworld | nominal_climate_columns/[2.0, 0.0] | 10.6739 | 24.9178 | 1 | A / terrain.rs |
| second / the_nether | blocks/crimson_roots | 24.2029 | 3.62626 | 0.725253 | N / nether.rs |
| second / overworld | top/savanna_plateau/grass_block | 5.21834 | 99.6725 | 5 | A / terrain.rs |
| second / overworld | ocean_coast_distance/warm_ocean/p50 | 382.267 | 82.4621 | 16 | A / terrain.rs |
| second / overworld | layer/savanna_plateau/-1/dirt | 6.72343 | 99.9454 | 5 | A / terrain.rs |
| second / overworld | height_biome/windswept_savanna/p100 | 73 | 165 | 5 | A / terrain.rs |
| second / overworld | layer/savanna_plateau/-2/dirt | 6.78349 | 98.0622 | 5 | A / terrain.rs |
| second / overworld | layer/snowy_slopes/-2/snow_block | 0 | 89.8274 | 5 | A / terrain.rs |
| second / overworld | layer/snowy_slopes/-2/stone | 92.5014 | 3.40777 | 5 | A / terrain.rs |
| second / overworld | top/savanna_plateau/stone | 88.1324 | 0 | 5 | A / terrain.rs |
| second / overworld | layer/savanna_plateau/-1/stone | 87.5318 | 0 | 5 | A / terrain.rs |
| second / overworld | layer/savanna_plateau/-2/stone | 87.9098 | 1.80131 | 5 | A / terrain.rs |
| second / the_nether | blocks/crimson_fungus | 2.93297 | 0.381084 | 0.1 | N / nether.rs |
| second / overworld | ocean_coast_distance/deep_ocean/p50 | 424.415 | 153.675 | 16 | A / terrain.rs |
| second / the_nether | blocks/weeping_vines_plant | 19.8935 | 3.35078 | 0.670156 | N / nether.rs |
| second / the_nether | blocks/warped_roots | 2.40771 | 0 | 0.1 | N / nether.rs |
| second / overworld | structure_candidate_offset_p95/shipwreck | 769.052 | 0 | 32 | R-shipwreck / shipwreck.rs |
| second / overworld | layer/desert/-4/sand | 90.6253 | 13.0547 | 5 | A / terrain.rs |
| second / overworld | structure_candidate_offset_p95/ocean_ruin | 728.659 | 0 | 32 | R-ocean_ruin / ocean_ruin.rs |
| second / overworld | patches/stony_shore/p50 | 1512 | 544 | 64 | A / terrain.rs |
| second / overworld | layer/desert/-4/sandstone | 1.63474 | 76.5148 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/desert | 3.74507 | 14.9178 | 1 | A / terrain.rs |
| second / overworld | layer/windswept_savanna/-3/dirt | 87.7419 | 13.8533 | 5 | A / terrain.rs |
| second / overworld | layer/snowy_slopes/-3/snow_block | 0 | 73.2961 | 5 | A / terrain.rs |
| second / overworld | layer/snowy_slopes/-3/stone | 92.3035 | 19.0908 | 5 | A / terrain.rs |
| second / overworld | layer/savanna_plateau/-3/dirt | 6.83649 | 79.5852 | 5 | A / terrain.rs |
| second / overworld | biomes_3d/desert | 3.66752 | 14.4011 | 1 | A / terrain.rs |
| second / the_nether | blocks/nether_sprouts | 2.11754 | 0 | 0.1 | N / nether.rs |
| second / overworld | structure_candidate_offset_p95/village | 672.911 | 0 | 32 | R-village / village.rs |
| second / the_nether | biomes_3d/warped_forest | 10.3994 | 0 | 1 | N / nether.rs |
| second / the_nether | surface_biomes/warped_forest | 10.3994 | 0 | 1 | N / nether.rs |
| second / overworld | layer/savanna_plateau/-3/stone | 87.9452 | 18.75 | 5 | A / terrain.rs |
| second / overworld | ocean_coast_distance/deep_lukewarm_ocean/p50 | 384 | 164.195 | 16 | A / terrain.rs |
| second / the_nether | tree_base_estimates/crimson_forest/crimson_stem | 2.22314 | 0.173554 | 0.1 | N / nether.rs |
| second / overworld | height_biome/savanna_plateau/p100 | 177 | 109 | 5 | A / terrain.rs |
| second / the_nether | slope/p100 | 79 | 89 | 1 | N / nether.rs |
| second / overworld | height_mean/savanna_plateau | 148.165 | 99.6965 | 5 | A / terrain.rs |
| second / overworld | height_biome/cherry_grove/p100 | 168 | 104 | 5 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/leaf_litter | 200.466 | 11.1159 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/oak_leaves | 208.997 | 21.3744 | 5 | F-tree / trees.rs |
| second / overworld | structure_candidate_offset_p95/abandoned_camp | 588.1 | 0 | 32 | R-abandoned_camp / abandoned_camp.rs |
| second / overworld | layer/beach/-4/sand | 71.5955 | 10.5069 | 5 | A / terrain.rs |
| second / the_nether | depth/64/air | 41.0125 | 65.0353 | 2 | N / nether.rs |
| second / overworld | height_biome/savanna_plateau/p95 | 167 | 107 | 5 | A / terrain.rs |
| second / overworld | top/cherry_grove/grass_block | 23.335 | 82.573 | 5 | A / terrain.rs |
| second / the_end | height_biome/small_end_islands/p100 | 60 | 119 | 5 | E / the_end.rs |
| second / overworld | blocks/spruce_leaves | 1.81865 | 0.051867 | 0.1 | F-tree / trees.rs |
| second / overworld | feature_biome/plains/oak_leaves | 450.849 | 57.0422 | 11.4084 | F-tree / trees.rs |
| second / overworld | layer/cherry_grove/-1/dirt | 25.1094 | 82.4818 | 5 | A / terrain.rs |
| second / overworld | patches/sunflower_plains/p50 | 24 | 752 | 64 | A / terrain.rs |
| second / overworld | layer/beach/-4/sandstone | 0.080105 | 56.9369 | 5 | A / terrain.rs |
| second / overworld | layer/windswept_savanna/-3/stone | 8.81716 | 65.6496 | 5 | A / terrain.rs |
| second / overworld | top/cherry_grove/stone | 70.7869 | 13.9599 | 5 | A / terrain.rs |
| second / the_nether | depth/48/air | 42.2153 | 64.8053 | 2 | N / nether.rs |
| second / overworld | layer/cherry_grove/-1/stone | 70.3959 | 14.1423 | 5 | A / terrain.rs |
| second / the_end | height_biome/small_end_islands/p95 | -1 | 55 | 5 | E / the_end.rs |
| second / overworld | blocks/kelp_plant | 35.0919 | 8.08191 | 1.61638 | F-ocean / ocean.rs |
| second / overworld | layer/cherry_grove/-2/dirt | 25.0984 | 80.5657 | 5 | A / terrain.rs |
| second / overworld | layer/cherry_grove/-2/stone | 70.6584 | 15.2372 | 5 | A / terrain.rs |
| second / overworld | patches/sparse_jungle/p50 | 16 | 712 | 64 | A / terrain.rs |
| second / overworld | feature_biome/eroded_badlands/amethyst_block | 162.787 | 0 | 5 | F / vegetation.rs |
| second / overworld | height_biome/eroded_badlands/p100 | 158 | 106 | 5 | A / terrain.rs |
| second / overworld | top/windswept_savanna/grass_block | 58.7594 | 7.92323 | 5 | A / terrain.rs |
| second / the_nether | structure_box_composition/ruined_portal/air | 84.4251 | 8.53909 | 5 | N / ruined_portal.rs |
| second / overworld | height_biome/cherry_grove/p95 | 151 | 102 | 5 | A / terrain.rs |
| second / overworld | blocks/sculk_vein | 2.52098 | 0.640098 | 0.12802 | U / underground_carvers.rs |
| second / overworld | layer/windswept_savanna/-2/dirt | 84.3696 | 35.999 | 5 | A / terrain.rs |
| second / overworld | depth/48/water | 53.4014 | 34.2137 | 2 | U / underground_carvers.rs |
| second / overworld | layer/desert/-3/sand | 95.5743 | 47.8603 | 5 | A / terrain.rs |
| second / overworld | layer/flower_forest/-3/dirt | 97.1943 | 49.5667 | 5 | A / terrain.rs |
| second / the_nether | depth/80/air | 47.6285 | 66.5011 | 2 | N / nether.rs |
| second / overworld | height_mean/cherry_grove | 123.068 | 87.9964 | 5 | A / terrain.rs |
| second / overworld | layer/old_growth_birch_forest/-3/dirt | 94.035 | 47.3913 | 5 | A / terrain.rs |
| second / overworld | structure_candidate_offset_p95/ruined_portal | 436.053 | 0 | 32 | R-ruined_portal / ruined_portal.rs |
| second / the_end | biomes_3d/end_midlands | 15.4143 | 22.2262 | 1 | E / the_end.rs |
| second / the_end | surface_biomes/end_midlands | 15.4143 | 22.2262 | 1 | E / the_end.rs |
| second / overworld | height_biome/sunflower_plains/p100 | 163 | 118 | 5 | A / terrain.rs |
| second / overworld | feature_biome/windswept_savanna/acacia_leaves | 352.016 | 55.1981 | 11.0396 | F-tree / trees.rs |
| second / overworld | layer/windswept_savanna/-1/dirt | 77.0761 | 32.4311 | 5 | A / terrain.rs |
| second / overworld | layer/beach/-3/sand | 87.3531 | 43.189 | 5 | A / terrain.rs |
| second / overworld | height_biome/sparse_jungle/p100 | 92 | 136 | 5 | A / terrain.rs |
| second / overworld | blocks/sculk | 7.45048 | 2.05487 | 0.410975 | U / underground_carvers.rs |
| second / overworld | height/p99 | 139 | 118 | 4 | A / terrain.rs |
| second / overworld | layer/river/-3/dirt | 77.9864 | 34.6134 | 5 | A / terrain.rs |
| second / overworld | blocks/snow | 1.69164 | 0.39491 | 0.1 | A / terrain.rs |
| second / overworld | layer/forest/-3/dirt | 93.2269 | 50.015 | 5 | A / terrain.rs |
| second / overworld | depth/144/underground_air | 21.7187 | 4.45863 | 2 | U / underground_carvers.rs |
| second / overworld | layer/birch_forest/-3/dirt | 100 | 56.8611 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/deep_cold_ocean | 6.44229 | 0 | 1 | A / terrain.rs |
| second / overworld | biomes_3d/deep_cold_ocean | 6.44129 | 0 | 1 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/jungle_leaves | 142.175 | 13.3514 | 5 | F-tree / trees.rs |
| second / overworld | layer/flower_forest/-3/stone | 1.82161 | 44.4992 | 5 | A / terrain.rs |
| second / overworld | top/river/grass_block | 47.3477 | 5.15576 | 5 | A / terrain.rs |
| second / overworld | layer/desert/-3/sandstone | 0.367664 | 42.3735 | 5 | A / terrain.rs |
| second / overworld | height_biome/savanna/p100 | 159 | 117 | 5 | A / terrain.rs |
| second / overworld | layer/jungle/-3/dirt | 94.3227 | 52.7888 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/lukewarm_ocean | 13.4517 | 7.23493 | 1 | A / terrain.rs |
| second / the_nether | structure_box_composition/ruined_portal/netherrack | 8.05481 | 70.1646 | 5 | N / ruined_portal.rs |
| second / overworld | biomes_3d/lukewarm_ocean | 13.4036 | 7.23353 | 1 | A / terrain.rs |
| second / overworld | height_mean/snowy_slopes | 146.997 | 116.152 | 5 | A / terrain.rs |
| second / overworld | height_biome/snowy_slopes/p100 | 168 | 127 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/deep_frozen_ocean | 6.07047 | 0 | 1 | A / terrain.rs |
| second / overworld | layer/savanna_plateau/-4/dirt | 1.74534 | 41.8941 | 5 | A / terrain.rs |
| second / overworld | height_biome/snowy_slopes/p95 | 165 | 125 | 5 | A / terrain.rs |
| second / the_nether | slope/p95 | 3 | 9 | 1 | N / nether.rs |
| second / overworld | layer/savanna_plateau/-4/stone | 92.8985 | 53.6026 | 5 | A / terrain.rs |
| second / overworld | patches/old_growth_birch_forest/p50 | 1920 | 1424 | 64 | A / terrain.rs |
| second / overworld | feature_biome/river/short_grass | 129.185 | 13.4623 | 5 | F / vegetation.rs |
| second / overworld | depth/128/underground_air | 18.6968 | 3.34297 | 2 | U / underground_carvers.rs |
| second / overworld | layer/savanna/-3/dirt | 91.0321 | 52.6987 | 5 | A / terrain.rs |
| second / overworld | blocks/pointed_dripstone | 10.4295 | 3.16288 | 0.632575 | U / underground_carvers.rs |
| second / overworld | layer/old_growth_birch_forest/-3/stone | 3.99655 | 42.2281 | 5 | A / terrain.rs |
| second / overworld | height_biome/plains/p100 | 164 | 126 | 5 | A / terrain.rs |
| second / overworld | layer/forest/-3/stone | 4.84256 | 42.7456 | 5 | A / terrain.rs |
| second / overworld | layer/jungle/-3/stone | 3.02748 | 39.6755 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sulfur_caves/potent_sulfur | 162.889 | 30.2134 | 6.04268 | U / underground_carvers.rs |
| second / overworld | biomes_3d/deep_frozen_ocean | 5.47589 | 0 | 1 | A / terrain.rs |
| second / overworld | top/meadow/grass_block | 61.9544 | 98.3625 | 5 | A / terrain.rs |
| second / overworld | height_biome/eroded_badlands/p95 | 138 | 102 | 5 | A / terrain.rs |
| second / overworld | nominal_climate_columns/[0.7, 0.8] | 6.79314 | 12.1698 | 1 | A / terrain.rs |
| second / overworld | layer/mangrove_swamp/-3/mud | 63.4524 | 27.6667 | 5 | A / terrain.rs |
| second / overworld | layer/plains/-3/dirt | 86.9512 | 51.6391 | 5 | A / terrain.rs |
| second / overworld | biomes_3d/warm_ocean | 23.803 | 18.5695 | 1 | A / terrain.rs |
| second / overworld | layer/birch_forest/-3/stone | 0 | 34.8579 | 5 | A / terrain.rs |
| second / overworld | top/windswept_savanna/gravel | 7.17432 | 41.9783 | 5 | A / terrain.rs |
| second / overworld | layer/sparse_jungle/-3/dirt | 90.2017 | 55.616 | 5 | A / terrain.rs |
| second / overworld | layer/mangrove_swamp/-3/stone | 24.6131 | 58.9417 | 5 | A / terrain.rs |
| second / overworld | top/meadow/stone | 35.7134 | 1.44399 | 5 | A / terrain.rs |
| second / overworld | layer/meadow/-1/dirt | 62.8777 | 97.0899 | 5 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/pointed_dripstone | 102.366 | 0 | 5 | U / underground_carvers.rs |
| second / overworld | height_biome/flower_forest/p100 | 98 | 132 | 5 | A / terrain.rs |
| second / overworld | height_biome/plains/p95 | 127 | 93 | 5 | A / terrain.rs |
| second / the_end | top/end_barrens/air | 94.1394 | 60.1504 | 5 | E / the_end.rs |
| second / the_end | top/end_barrens/end_stone | 5.86061 | 39.8496 | 5 | E / the_end.rs |
| second / the_nether | depth/32/air | 48.3191 | 61.9109 | 2 | N / nether.rs |
| second / overworld | patches/snowy_slopes/p50 | 7328 | 6896 | 64 | A / terrain.rs |
| second / overworld | blocks/orange_poplar_leaves | 1.00993 | 0 | 0.1 | F-tree / trees.rs |
| second / the_end | height/p100 | 103 | 119 | 4 | E / the_end.rs |
| second / the_end | slope/p99 | 51 | 46 | 1 | E / the_end.rs |
| second / overworld | layer/meadow/-1/stone | 35.3417 | 2.08498 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/warm_ocean | 23.9115 | 18.9395 | 1 | A / terrain.rs |
| second / overworld | blocks/cherry_leaves | 1.00403 | 0.01439 | 0.1 | F-tree / trees.rs |
| second / overworld | feature_biome/meadow/birch_leaves | 109.702 | 10.9657 | 5 | F-tree / trees.rs |
| second / overworld | depth/32/water | 21.4118 | 8.29493 | 2 | U / underground_carvers.rs |
| second / overworld | feature_biome/grove/amethyst_block | 98.345 | 0 | 5 | F / vegetation.rs |
| second / overworld | layer/sparse_jungle/-3/stone | 6.42291 | 39.0273 | 5 | A / terrain.rs |
| second / the_end | layer/end_barrens/-1/air | 95.0542 | 62.5796 | 5 | E / the_end.rs |
| second / the_end | layer/end_barrens/-1/end_stone | 4.94581 | 37.4204 | 5 | E / the_end.rs |
| second / overworld | structure_candidate_offset_p95/trial_chambers | 311.52 | 0 | 32 | R-trial_chambers / trial_chambers.rs |
| second / overworld | layer/savanna/-3/stone | 6.89405 | 39.1526 | 5 | A / terrain.rs |
| second / overworld | layer/windswept_savanna/-2/stone | 9.2035 | 40.7972 | 5 | A / terrain.rs |
| second / the_end | layer/end_barrens/-2/air | 95.2966 | 64.2866 | 5 | E / the_end.rs |
| second / the_end | layer/end_barrens/-2/end_stone | 4.70341 | 35.7134 | 5 | E / the_end.rs |
| second / overworld | layer/plains/-3/stone | 11.0853 | 41.8278 | 5 | A / terrain.rs |
| second / overworld | blocks/prismarine_bricks | 1.10586 | 0.1872 | 0.1 | F / vegetation.rs |
| second / the_nether | blocks/twisting_vines_plant | 0.915519 | 0 | 0.1 | N / nether.rs |
| second / overworld | blocks/blue_ice | 0.910145 | 0 | 0.1 | A / terrain.rs |
| second / overworld | blocks/prismarine | 1.0843 | 0.178099 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/stony_shore/snow | 90.5908 | 0.260268 | 5 | A / terrain.rs |
| second / overworld | height_biome/badlands/p100 | 152 | 122 | 5 | A / terrain.rs |
| second / overworld | height_biome/flower_forest/p95 | 83 | 113 | 5 | A / terrain.rs |
| second / the_end | patches/end_barrens/p50 | 128 | 512 | 64 | E / the_end.rs |
| second / overworld | tree_span/forest/jungle_log/p50 | 19 | 1 | 2 | F-tree / trees.rs |
| second / the_end | layer/end_barrens/-3/air | 95.539 | 65.6648 | 5 | E / the_end.rs |
| second / the_end | layer/end_barrens/-3/end_stone | 4.46099 | 34.3352 | 5 | E / the_end.rs |
| second / overworld | nominal_climate_columns/[0.95, 0.9] | 0.84415 | 5.27723 | 1 | A / terrain.rs |
| second / overworld | layer/beach/-3/sandstone | 0.061414 | 29.5365 | 5 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-5/terracotta | 54.7525 | 25.7515 | 5 | A / terrain.rs |
| second / overworld | height_biome/stony_shore/p95 | 116 | 87 | 5 | A / terrain.rs |
| second / the_end | layer/end_barrens/-4/air | 95.8198 | 66.8426 | 5 | E / the_end.rs |
| second / the_end | layer/end_barrens/-4/end_stone | 4.1802 | 33.1574 | 5 | E / the_end.rs |
| second / overworld | top/badlands/terracotta | 36.5499 | 64.9611 | 5 | A / terrain.rs |
| second / the_end | layer/end_barrens/-5/air | 96.0984 | 67.6975 | 5 | E / the_end.rs |
| second / the_end | layer/end_barrens/-5/end_stone | 3.90159 | 32.3025 | 5 | E / the_end.rs |
| second / overworld | blocks/tube_coral_block | 4.92131 | 1.82859 | 0.365717 | F-ocean / ocean.rs |
| second / overworld | height_biome/desert/p95 | 108 | 80 | 5 | A / terrain.rs |
| second / overworld | height_biome/lukewarm_ocean/p100 | 94 | 66 | 5 | A / terrain.rs |
| second / overworld | layer/river/-3/stone | 15.5687 | 43.5338 | 5 | A / terrain.rs |
| second / the_nether | inside_jump_ge16_share | 1.37282 | 2.06879 | 0.25 | N / nether.rs |
| second / the_nether | jump_ge16_share | 1.37499 | 2.06878 | 0.25 | N / nether.rs |
| second / the_end | depth/64/underground_air | 0 | 11.0653 | 2 | E / the_end.rs |
| second / overworld | nominal_climate_columns/[0.95, 0.8] | 0.291115 | 4.43416 | 1 | A / terrain.rs |
| second / overworld | surface_biomes/sparse_jungle | 0.291115 | 4.43416 | 1 | A / terrain.rs |
| second / overworld | blocks/ice | 0.828425 | 0 | 0.1 | A / terrain.rs |
| second / overworld | blocks/horn_coral_block | 4.88102 | 1.83911 | 0.367822 | F-ocean / ocean.rs |
| second / overworld | structure_candidate_offset_p95/mineshaft | 264.315 | 0 | 32 | R-mineshaft / mineshaft.rs |
| second / overworld | blocks/brain_coral_block | 4.85804 | 1.83202 | 0.366404 | F-ocean / ocean.rs |
| second / overworld | blocks/bubble_coral_block | 4.89597 | 1.85363 | 0.370727 | F-ocean / ocean.rs |
| second / overworld | blocks/fire_coral_block | 4.83916 | 1.83659 | 0.367317 | F-ocean / ocean.rs |
| second / overworld | layer/windswept_savanna/-1/stone | 9.58983 | 36.7618 | 5 | A / terrain.rs |
| second / the_nether | height/p50 | 64 | 51 | 4 | N / nether.rs |
| second / overworld | layer/eroded_badlands/-1/orange_terracotta | 6.61386 | 33.651 | 5 | A / terrain.rs |
| second / overworld | height_biome/mangrove_swamp/p100 | 71 | 98 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/jungle | 0.84415 | 4.89047 | 1 | A / terrain.rs |
| second / overworld | height_mean/grove | 84.8551 | 105.025 | 5 | A / terrain.rs |
| second / overworld | biomes_3d/sparse_jungle | 0.272013 | 4.27824 | 1 | A / terrain.rs |
| second / the_nether | slope/p99 | 37 | 41 | 1 | N / nether.rs |
| second / overworld | surface_biomes/sunflower_plains | 4.9305 | 0.940315 | 1 | A / terrain.rs |
| second / the_nether | border_jump_ge16_share | 1.40862 | 2.06854 | 0.25 | N / nether.rs |
| second / overworld | biomes_3d/jungle | 0.817982 | 4.76063 | 1 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-2/orange_terracotta | 6.94059 | 33.1928 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/wooded_badlands | 0 | 3.93264 | 1 | A / terrain.rs |
| second / the_end | depth/80/underground_air | 0 | 10.4391 | 2 | E / the_end.rs |
| second / overworld | layer/river/-1/dirt | 64.7365 | 38.7876 | 5 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-4/terracotta | 56.4257 | 30.6268 | 5 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-3/orange_terracotta | 7.60149 | 33.2845 | 5 | A / terrain.rs |
| second / overworld | layer/river/-2/dirt | 73.4117 | 47.7325 | 5 | A / terrain.rs |
| second / overworld | biomes_3d/wooded_badlands | 0 | 3.83975 | 1 | A / terrain.rs |
| second / overworld | surface_band/eroded_badlands/terracotta/y64 | 4.03705 | 0.222583 | 0.25 | A / terrain.rs |
| second / overworld | biomes_3d/sunflower_plains | 4.69827 | 0.898158 | 1 | A / terrain.rs |
| second / overworld | blocks/red_poplar_leaves | 0.757604 | 0 | 0.1 | F-tree / trees.rs |
| second / overworld | height/p50 | 50 | 62 | 4 | A / terrain.rs |
| second / overworld | height_biome/beach/p100 | 99 | 74 | 5 | A / terrain.rs |
| second / overworld | height_biome/sunflower_plains/p95 | 132 | 107 | 5 | A / terrain.rs |
| second / overworld | patches/flower_forest/p50 | 48 | 368 | 64 | A / terrain.rs |
| second / overworld | structure_candidate_count/monument | 6 | 1 | 1 | R-monument / monument.rs |
| second / the_end | height_mean/end_barrens | 1.92801 | 20.5625 | 5 | E / the_end.rs |
| second / overworld | depth/112/underground_air | 12.4849 | 2.56581 | 2 | U / underground_carvers.rs |
| second / the_end | border_jump_ge16_share | 1.49567 | 0.88726 | 0.25 | E / the_end.rs |
| second / overworld | structure_candidate_count/shipwreck | 42 | 19 | 4.75 | R-shipwreck / shipwreck.rs |
| second / overworld | height_biome/birch_forest/p100 | 99 | 123 | 5 | A / terrain.rs |
| second / overworld | height_biome/stony_shore/p100 | 136 | 112 | 5 | A / terrain.rs |
| second / overworld | top/sparse_jungle/grass_block | 67.2226 | 91.1307 | 5 | A / terrain.rs |
| second / overworld | layer/mangrove_swamp/-2/mud | 82.753 | 58.9583 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/flower_forest | 0.701193 | 4.23073 | 1 | A / terrain.rs |
| second / overworld | layer/birch_forest/-4/stone | 90.6022 | 67.2092 | 5 | A / terrain.rs |
| second / the_nether | blocks/warped_fungus | 0.73921 | 0.038567 | 0.1 | N / nether.rs |
| second / overworld | layer/eroded_badlands/-3/terracotta | 55.5 | 32.2581 | 5 | A / terrain.rs |
| second / overworld | feature_biome/snowy_slopes/cherry_leaves | 69.4485 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/lush_caves/spore_blossom | 91.5502 | 22.1203 | 5 | F / vegetation.rs |
| second / overworld | layer/eroded_badlands/-1/terracotta | 51.5124 | 28.4641 | 5 | A / terrain.rs |
| second / overworld | ore/coal_ore/y96 | 3.29708 | 1.38613 | 0.277226 | U / underground.rs |
| second / overworld | layer/eroded_badlands/-4/orange_terracotta | 8.28465 | 31.2317 | 5 | A / terrain.rs |
| second / the_nether | height_mean/crimson_forest | 55.4898 | 38.3991 | 5 | N / nether.rs |
| second / overworld | feature_biome/grove/dripstone_block | 68.3039 | 0 | 5 | U / underground_carvers.rs |
| second / overworld | biomes_3d/flower_forest | 0.679416 | 4.08274 | 1 | A / terrain.rs |
| second / overworld | top/stony_shore/gravel | 11.0926 | 33.7166 | 5 | A / terrain.rs |
| second / overworld | layer/snowy_slopes/-4/stone | 92.8152 | 70.5626 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/river | 3.55614 | 6.87657 | 1 | A / terrain.rs |
| second / overworld | feature_biome/old_growth_birch_forest/oak_leaves | 81.448 | 15.3013 | 5 | F-tree / trees.rs |
| second / overworld | biomes_3d/river | 3.33206 | 6.62334 | 1 | A / terrain.rs |
| second / overworld | blocks/kelp | 4.30258 | 1.85906 | 0.371811 | F-ocean / ocean.rs |
| second / the_end | biomes_3d/end_barrens | 12.7051 | 9.45416 | 1 | E / the_end.rs |
| second / the_end | nominal_climate_columns/[0.5, 0.5] | 12.7051 | 9.45416 | 1 | E / the_end.rs |
| second / the_end | nominal_climate_columns/unknown | 87.2949 | 90.5458 | 1 | E / the_end.rs |
| second / the_end | surface_biomes/end_barrens | 12.7051 | 9.45416 | 1 | E / the_end.rs |
| second / the_nether | depth/48/underground_air | 26.2897 | 34.9272 | 2 | N / nether.rs |
| second / overworld | layer/snowy_slopes/-4/snow_block | 0 | 21.5487 | 5 | A / terrain.rs |
| second / overworld | layer/grove/-4/dirt | 2.29232 | 23.7599 | 5 | A / terrain.rs |
| second / the_end | jump_ge16_share | 1.47728 | 0.944824 | 0.25 | E / the_end.rs |
| second / overworld | layer/eroded_badlands/-2/terracotta | 53.953 | 32.8262 | 5 | A / terrain.rs |
| second / the_end | inside_jump_ge16_share | 1.4761 | 0.948537 | 0.25 | E / the_end.rs |
| second / overworld | top/eroded_badlands/terracotta | 34.6856 | 55.7735 | 5 | A / terrain.rs |
| second / overworld | height_biome/badlands/p95 | 137 | 116 | 5 | A / terrain.rs |
| second / overworld | feature_biome/savanna_plateau/amethyst_block | 62.7821 | 0 | 5 | F / vegetation.rs |
| second / overworld | blocks/tube_coral_wall_fan | 2.56866 | 1.13906 | 0.227813 | F-ocean / ocean.rs |
| second / overworld | cave_component_volume/p95 | 3973 | 1764 | 352.8 | U / underground_carvers.rs |
| second / overworld | layer/beach/-5/sandstone | 71.5786 | 50.8019 | 5 | A / terrain.rs |
| second / the_nether | depth/96/air | 46.5777 | 54.8453 | 2 | N / nether.rs |
| second / overworld | blocks/bubble_coral_wall_fan | 2.56853 | 1.14728 | 0.229456 | F-ocean / ocean.rs |
| second / overworld | top/stony_shore/stone | 77.9272 | 57.3087 | 5 | A / terrain.rs |
| second / overworld | blocks/horn_coral_wall_fan | 2.57725 | 1.15351 | 0.230701 | F-ocean / ocean.rs |
| second / overworld | blocks/brain_coral_wall_fan | 2.5605 | 1.1516 | 0.23032 | F-ocean / ocean.rs |
| second / overworld | blocks/fire_coral_wall_fan | 2.5631 | 1.15592 | 0.231185 | F-ocean / ocean.rs |
| second / overworld | layer/meadow/-2/dirt | 62.6978 | 82.9122 | 5 | A / terrain.rs |
| second / overworld | structure_box_composition/ruined_portal/water | 57.747 | 27.6701 | 5 | R-ruined_portal / ruined_portal.rs |
| second / overworld | layer/meadow/-2/stone | 35.0809 | 15.0611 | 5 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-4/stone | 89.5333 | 69.5213 | 5 | A / terrain.rs |
| second / the_nether | structure_candidate_count/nether_fossil | 0 | 99 | 24.75 | N / nether_fossil.rs |
| second / overworld | structure_candidate_count/ocean_ruin | 52 | 26 | 6.5 | R-ocean_ruin / ocean_ruin.rs |
| second / overworld | layer/beach/-2/sand | 87.5908 | 67.5984 | 5 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-5/orange_terracotta | 10.0099 | 29.8021 | 5 | A / terrain.rs |
| second / the_nether | biomes_3d/basalt_deltas | 0 | 2.96717 | 1 | N / nether.rs |
| second / the_nether | nominal_climate_columns/[2.0, 0.0] | 0 | 2.96717 | 1 | N / nether.rs |
| second / the_nether | nominal_climate_columns/unknown | 100 | 97.0328 | 1 | N / nether.rs |
| second / the_nether | surface_biomes/basalt_deltas | 0 | 2.96717 | 1 | N / nether.rs |
| second / overworld | feature_biome/windswept_savanna/oak_leaves | 58.7646 | 0 | 5 | F-tree / trees.rs |
| second / the_nether | blocks/glowstone | 3.16253 | 1.45454 | 0.290909 | N / nether.rs |
| second / overworld | layer/birch_forest/-4/dirt | 2.28102 | 21.6474 | 5 | A / terrain.rs |
| second / overworld | layer/birch_forest/-2/dirt | 100 | 80.788 | 5 | A / terrain.rs |
| second / the_end | layer/end_midlands/-5/end_stone | 94.0149 | 74.852 | 5 | E / the_end.rs |
| second / the_end | layer/end_midlands/-5/air | 5.9851 | 25.1473 | 5 | E / the_end.rs |
| second / overworld | height_mean/birch_forest | 82.8253 | 68.5123 | 5 | A / terrain.rs |
| second / overworld | height_biome/grove/p95 | 110 | 129 | 5 | A / terrain.rs |
| second / the_nether | ore/nether_quartz_ore/y64 | 19.9458 | 9.34527 | 1.86905 | N / nether.rs |
| second / overworld | height/p100 | 177 | 168 | 4 | A / terrain.rs |
| second / the_end | depth/48/underground_air | 0.001574 | 7.50113 | 2 | E / the_end.rs |
| second / overworld | feature_biome/deep_ocean/kelp_plant | 2199.98 | 679.5 | 135.9 | F-ocean / ocean.rs |
| second / the_end | layer/end_midlands/-4/end_stone | 94.5176 | 75.9599 | 5 | E / the_end.rs |
| second / the_end | layer/end_midlands/-4/air | 5.48242 | 24.0394 | 5 | E / the_end.rs |
| second / overworld | layer/badlands/-5/terracotta | 54.3315 | 36.078 | 5 | A / terrain.rs |
| second / overworld | ore/coal_ore/y112 | 1.65153 | 0.286988 | 0.25 | U / underground.rs |
| second / overworld | patches/deep_lukewarm_ocean/p50 | 16 | 248 | 64 | A / terrain.rs |
| second / overworld | patches/forest/p50 | 48 | 280 | 64 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-4/dirt | 1.95699 | 20.0622 | 5 | A / terrain.rs |
| second / overworld | nominal_climate_columns/[0.8, 0.4] | 12.3119 | 9.59771 | 1 | A / terrain.rs |
| second / overworld | blocks/acacia_leaves | 2.68818 | 1.29034 | 0.258069 | F-tree / trees.rs |
| second / overworld | height_biome/savanna/p95 | 115 | 97 | 5 | A / terrain.rs |
| second / the_nether | ore/nether_quartz_ore/y48 | 19.6299 | 9.5225 | 1.9045 | N / nether.rs |
| second / overworld | layer/meadow/-3/dirt | 62.512 | 44.8336 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-4/terracotta | 54.7805 | 37.1962 | 5 | A / terrain.rs |
| second / the_end | layer/end_midlands/-3/air | 5.09366 | 22.6111 | 5 | E / the_end.rs |
| second / the_end | layer/end_midlands/-3/end_stone | 94.9063 | 77.3889 | 5 | E / the_end.rs |
| second / overworld | top/badlands/red_sand | 28.1061 | 10.663 | 5 | A / terrain.rs |
| second / overworld | top/sunflower_plains/stone | 18.8444 | 1.41302 | 5 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-3/dirt | 78.9631 | 61.6622 | 5 | A / terrain.rs |
| second / overworld | layer/ocean/-3/dirt | 24.1831 | 6.98691 | 5 | A / terrain.rs |
| second / overworld | layer/savanna/-4/stone | 87.0815 | 69.8915 | 5 | A / terrain.rs |
| second / overworld | layer/old_growth_birch_forest/-2/dirt | 92.7293 | 75.596 | 5 | A / terrain.rs |
| second / overworld | feature_biome/lush_caves/small_dripleaf | 217.661 | 71.2419 | 14.2484 | F / vegetation.rs |
| second / overworld | height_biome/sparse_jungle/p95 | 76 | 93 | 5 | A / terrain.rs |
| second / the_nether | ore/nether_gold_ore/y64 | 6.40129 | 3.19743 | 0.639486 | N / nether.rs |
| second / overworld | blocks/bamboo | 0 | 1.6986 | 0.339719 | F / vegetation.rs |
| second / the_nether | blocks/bone_block | 0 | 1.07163 | 0.214325 | N / nether.rs |
| second / overworld | blocks/dark_oak_leaves | 0 | 7.39776 | 1.47955 | F-tree / trees.rs |
| second / overworld | blocks/glow_lichen | 0 | 2.87989 | 0.575978 | F / vegetation.rs |
| second / overworld | blocks/pale_moss_block | 0 | 0.635024 | 0.127005 | A / terrain.rs |
| second / overworld | blocks/pale_oak_leaves | 0 | 1.63803 | 0.327606 | F-tree / trees.rs |
| second / overworld | blocks/pale_oak_log | 0 | 0.612768 | 0.122554 | F-tree / trees.rs |
| second / the_end | blocks/purpur_block | 0 | 0.585801 | 0.11716 | E / the_end.rs |
| second / the_nether | blocks/soul_fire | 0 | 0.578512 | 0.115702 | N / nether.rs |
| second / the_nether | blocks/soul_soil | 0 | 914.78 | 182.956 | N / nether.rs |
| second / overworld | blocks/dark_oak_log | 0.010497 | 2.75865 | 0.55173 | F-tree / trees.rs |
| second / the_nether | blocks/soul_sand | 5.46189 | 989.725 | 197.945 | N / nether.rs |
| second / overworld | layer/mangrove_swamp/-1/mud | 86.2202 | 69.8083 | 5 | A / terrain.rs |
| second / the_end | layer/end_midlands/-2/air | 4.7926 | 21.1496 | 5 | E / the_end.rs |
| second / the_end | layer/end_midlands/-2/end_stone | 95.2074 | 78.8504 | 5 | E / the_end.rs |
| second / overworld | surface_biomes/cold_ocean | 2.45225 | 0 | 1 | A / terrain.rs |
| second / overworld | height_mean/flower_forest | 69.1365 | 81.393 | 5 | A / terrain.rs |
| second / overworld | blocks/yellow_terracotta | 0.571881 | 28.0547 | 5.61095 | A / terrain.rs |
| second / overworld | layer/river/-4/stone | 78.2763 | 61.9947 | 5 | A / terrain.rs |
| second / overworld | biomes_3d/cold_ocean | 2.43669 | 0 | 1 | A / terrain.rs |
| second / overworld | layer/savanna/-4/dirt | 2.39801 | 18.6188 | 5 | A / terrain.rs |
| second / the_nether | ore/nether_gold_ore/y48 | 6.46648 | 3.28742 | 0.657484 | N / nether.rs |
| second / the_nether | blocks/nether_bricks | 6.82186 | 197.302 | 39.4604 | N / nether.rs |
| second / overworld | layer/meadow/-3/stone | 34.6403 | 50.6445 | 5 | A / terrain.rs |
| second / overworld | height_biome/desert/p100 | 152 | 168 | 5 | A / terrain.rs |
| second / overworld | blocks/rooted_dirt | 0.005638 | 0.482751 | 0.1 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-1/stone | 18.8925 | 3.07251 | 5 | A / terrain.rs |
| second / overworld | layer/desert/-2/sand | 96.0811 | 80.2711 | 5 | A / terrain.rs |
| second / overworld | height_mean/sunflower_plains | 88.9735 | 77.1418 | 5 | A / terrain.rs |
| second / overworld | blocks/calcite | 37.7925 | 19.5086 | 3.90173 | A / terrain.rs |
| second / overworld | blocks/brown_terracotta | 2.35282 | 37.186 | 7.4372 | A / terrain.rs |
| second / overworld | blocks/orange_terracotta | 4.03329 | 61.4367 | 12.2873 | A / terrain.rs |
| second / the_nether | blocks/basalt | 34.786 | 518.731 | 103.746 | N / nether.rs |
| second / overworld | surface_biomes/dark_forest | 0 | 2.32798 | 1 | A / terrain.rs |
| second / the_end | feature_biome/end_midlands/chorus_flower | 48.1359 | 2.17983 | 5 | E / the_end.rs |
| second / overworld | blocks/clay | 6.6045 | 81.3321 | 16.2664 | A / terrain.rs |
| second / overworld | height_mean/meadow | 108.012 | 119.428 | 5 | A / terrain.rs |
| second / overworld | blocks/sea_pickle | 0.985557 | 0.515262 | 0.103052 | F-ocean / ocean.rs |
| second / overworld | feature_biome/windswept_savanna/short_grass | 61.9076 | 16.428 | 5 | F / vegetation.rs |
| second / overworld | blocks/sulfur_spike | 0.080083 | 0.83868 | 0.167736 | U / underground_carvers.rs |
| second / the_end | layer/end_midlands/-1/air | 4.5711 | 19.6215 | 5 | E / the_end.rs |
| second / the_end | layer/end_midlands/-1/end_stone | 95.4289 | 80.3785 | 5 | E / the_end.rs |
| second / overworld | layer/grove/-4/stone | 88.1563 | 73.1151 | 5 | A / terrain.rs |
| second / overworld | height_biome/forest/p100 | 147 | 132 | 5 | A / terrain.rs |
| second / overworld | patches/badlands/p50 | 16 | 208 | 64 | A / terrain.rs |
| second / the_end | patches/end_midlands/p50 | 64 | 256 | 64 | E / the_end.rs |
| second / the_nether | structure_candidate_count/fortress | 0 | 3 | 1 | N / nether_fortress.rs |
| second / overworld | structure_candidate_count/trail_ruins | 0 | 3 | 1 | R-trail_ruins / trail_ruins.rs |
| second / overworld | surface_band/eroded_badlands/terracotta/y48 | 2.47207 | 0.229724 | 0.25 | A / terrain.rs |
| second / overworld | blocks/sandstone | 44.9941 | 429.818 | 85.9635 | A / terrain.rs |
| second / overworld | top/river/gravel | 20.7113 | 35.6162 | 5 | A / terrain.rs |
| second / overworld | depth/0/air | 13.5085 | 7.56538 | 2 | U / underground_carvers.rs |
| second / overworld | depth/0/underground_air | 13.5084 | 7.56622 | 2 | U / underground_carvers.rs |
| second / overworld | blocks/fern | 0.214153 | 1.85831 | 0.371661 | F / vegetation.rs |
| second / the_nether | ore/nether_quartz_ore/y32 | 16.9568 | 9.0202 | 1.80404 | N / nether.rs |
| second / the_nether | tree_base_estimates/warped_forest/warped_stem | 0.438017 | 0 | 0.1 | N / nether.rs |
| second / overworld | blocks/gray_wool | 0.433864 | 0 | 0.1 | F / vegetation.rs |
| second / overworld | height_mean/stony_shore | 73.973 | 63.1537 | 5 | A / terrain.rs |
| second / overworld | blocks/light_gray_terracotta | 2.45813 | 17.8665 | 3.57329 | A / terrain.rs |
| second / overworld | layer/plains/-4/stone | 88.2489 | 73.9021 | 5 | A / terrain.rs |
| second / overworld | feature_biome/savanna_plateau/snow_block | 42.8698 | 0 | 5 | A / terrain.rs |
| second / overworld | blocks/dripstone_block | 28.7737 | 15.4945 | 3.0989 | U / underground_carvers.rs |
| second / overworld | layer/mangrove_swamp/-2/stone | 9.73214 | 23.775 | 5 | A / terrain.rs |
| second / the_nether | ore/nether_gold_ore/y80 | 5.7438 | 3.11754 | 0.623508 | N / nether.rs |
| second / overworld | blocks/jungle_leaves | 2.70402 | 17.0254 | 3.40508 | F-tree / trees.rs |
| second / the_nether | blocks/nether_brick_fence | 0.460055 | 2.88154 | 0.576309 | N / nether.rs |
| second / overworld | height_biome/grove/p100 | 121 | 135 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-3/brown_terracotta | 5.68099 | 19.6552 | 5 | A / terrain.rs |
| second / overworld | layer/beach/-5/stone | 21.0962 | 35.0529 | 5 | A / terrain.rs |
| second / overworld | covered_grass | 7.91411 | 12.0911 | 1 | A / terrain.rs |
| second / overworld | top/mangrove_swamp/mud | 86.4732 | 72.6167 | 5 | A / terrain.rs |
| second / overworld | layer/plains/-4/dirt | 2.25081 | 16.0988 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-3/terracotta | 52.8712 | 39.0369 | 5 | A / terrain.rs |
| second / overworld | blocks/red_sand | 1.0873 | 6.34887 | 1.26977 | A / terrain.rs |
| second / overworld | blocks/vine | 2.79573 | 16.0607 | 3.21215 | F / vegetation.rs |
| second / overworld | tree_base_estimates/jungle/jungle_log | 0.049639 | 0.462214 | 0.1 | F-tree / trees.rs |
| second / the_end | top/end_midlands/air | 4.1308 | 17.8252 | 5 | E / the_end.rs |
| second / the_end | top/end_midlands/end_stone | 95.8692 | 82.1748 | 5 | E / the_end.rs |
| second / the_end | depth/32/underground_air | 6.70137 | 12.1657 | 2 | E / the_end.rs |
| second / overworld | top/ocean/gravel | 75.6207 | 89.2634 | 5 | A / terrain.rs |
| second / overworld | blocks/jungle_log | 0.731536 | 4.02089 | 0.804177 | F-tree / trees.rs |
| second / overworld | blocks/white_terracotta | 3.3297 | 18.2309 | 3.64618 | A / terrain.rs |
| second / overworld | blocks/dead_bush | 0.18226 | 0.99047 | 0.198094 | F / vegetation.rs |
| second / overworld | structure_box_composition/ruined_portal/air | 13.1423 | 33.4856 | 5 | R-ruined_portal / ruined_portal.rs |
| second / overworld | layer/sparse_jungle/-4/dirt | 1.58501 | 15.1441 | 5 | A / terrain.rs |
| second / overworld | blocks/deepslate_bricks | 0.404548 | 0 | 0.1 | F / vegetation.rs |
| second / overworld | layer/cherry_grove/-3/stone | 70.5627 | 57.1168 | 5 | A / terrain.rs |
| second / overworld | ocean_coast_distance/lukewarm_ocean/p50 | 99.0353 | 56.1427 | 16 | A / terrain.rs |
| second / overworld | biomes_3d/dark_forest | 0 | 2.00738 | 1 | A / terrain.rs |
| second / overworld | structure_candidate_count/abandoned_camp | 4 | 12 | 3 | R-abandoned_camp / abandoned_camp.rs |
| second / overworld | depth/-48/air | 13.1801 | 7.87126 | 2 | U / underground_carvers.rs |
| second / overworld | depth/-48/underground_air | 13.1801 | 7.87124 | 2 | U / underground_carvers.rs |
| second / overworld | feature_biome/birch_forest/dripstone_block | 39.8089 | 0 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/savanna_plateau/sculk | 41.9328 | 2.23854 | 5 | U / underground_carvers.rs |
| second / overworld | blocks/cinnabar | 27.0504 | 129.875 | 25.9749 | U / underground_carvers.rs |
| second / overworld | layer/sunflower_plains/-3/stone | 18.8987 | 32.069 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-4/brown_terracotta | 4.96972 | 18.0577 | 5 | A / terrain.rs |
| second / overworld | top/birch_forest/grass_block | 97.6734 | 84.6263 | 5 | A / terrain.rs |
| second / overworld | layer/beach/-1/sand | 87.9691 | 75.0243 | 5 | A / terrain.rs |
| second / overworld | blocks/terracotta | 24.0844 | 107.817 | 21.5635 | A / terrain.rs |
| second / overworld | ore/gold_ore/y32 | 0.209053 | 1.17778 | 0.25 | U / underground.rs |
| second / overworld | feature_biome/cherry_grove/orange_poplar_leaves | 38.6161 | 0 | 5 | F-tree / trees.rs |
| second / overworld | layer/old_growth_birch_forest/-4/dirt | 1.69671 | 14.5591 | 5 | A / terrain.rs |
| second / overworld | layer/birch_forest/-2/stone | 0 | 12.8615 | 5 | A / terrain.rs |
| second / the_end | biomes_3d/small_end_islands | 32.1776 | 30.2533 | 1 | E / the_end.rs |
| second / the_end | surface_biomes/small_end_islands | 32.1776 | 30.2533 | 1 | E / the_end.rs |
| second / overworld | blocks/yellow_poplar_leaves | 0.384762 | 0 | 0.1 | F-tree / trees.rs |
| second / overworld | height_mean/desert | 75.0284 | 65.4173 | 5 | A / terrain.rs |
| second / overworld | feature_biome/jungle/leaf_litter | 41.466 | 3.0461 | 5 | F / vegetation.rs |
| second / overworld | layer/badlands/-2/brown_terracotta | 6.5593 | 19.3046 | 5 | A / terrain.rs |
| second / overworld | layer/flower_forest/-4/dirt | 1.7244 | 14.4592 | 5 | A / terrain.rs |
| second / overworld | feature_biome/deep_dark/amethyst_block | 690.304 | 273.219 | 54.6438 | F / vegetation.rs |
| second / overworld | layer/ocean/-2/dirt | 20.7243 | 8.03398 | 5 | A / terrain.rs |
| second / the_nether | ore/nether_gold_ore/y32 | 5.764 | 3.27824 | 0.655647 | N / nether.rs |
| second / overworld | top/river/dirt | 15.924 | 28.5232 | 5 | A / terrain.rs |
| second / overworld | layer/sparse_jungle/-1/dirt | 81.5166 | 94.095 | 5 | A / terrain.rs |
| second / overworld | blocks/sulfur | 36.6294 | 149.053 | 29.8105 | U / underground_carvers.rs |
| second / the_nether | ore/nether_quartz_ore/y80 | 17.0376 | 9.74931 | 1.94986 | N / nether.rs |
| second / overworld | feature_biome/old_growth_birch_forest/spruce_leaves | 37.3376 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/old_growth_birch_forest/azalea_leaves | 37.1639 | 0 | 5 | F-tree / trees.rs |
| second / overworld | layer/old_growth_birch_forest/-4/stone | 83.6559 | 71.2828 | 5 | A / terrain.rs |
| second / overworld | layer/river/-2/stone | 15.6209 | 27.8893 | 5 | A / terrain.rs |
| second / overworld | layer/forest/-4/dirt | 2.00306 | 14.2578 | 5 | A / terrain.rs |
| second / overworld | height_mean/plains | 80.0932 | 71.0419 | 5 | A / terrain.rs |
| second / overworld | top/beach/sand | 88.7692 | 76.7063 | 5 | A / terrain.rs |
| second / overworld | height_biome/forest/p95 | 90 | 102 | 5 | A / terrain.rs |
| second / overworld | height_biome/ocean/p100 | 75 | 63 | 5 | A / terrain.rs |
| second / overworld | height_biome/old_growth_birch_forest/p95 | 82 | 94 | 5 | A / terrain.rs |
| second / overworld | layer/flower_forest/-2/dirt | 96.6888 | 84.7598 | 5 | A / terrain.rs |
| second / overworld | blocks/infested_deepslate | 0 | 0.357325 | 0.1 | F / vegetation.rs |
| second / overworld | layer/jungle/-4/dirt | 2.26968 | 14.0504 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-2/terracotta | 51.9406 | 40.2649 | 5 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-1/dirt | 79.1048 | 90.7758 | 5 | A / terrain.rs |
| second / overworld | feature_biome/deep_dark/sculk_sensor | 112.628 | 46.998 | 9.39959 | U / underground_carvers.rs |
| second / overworld | biomes_3d/plains | 5.54284 | 7.28481 | 1 | A / terrain.rs |
| second / overworld | structure_candidate_spacing_p50/buried_treasure | 90.5097 | 698.891 | 174.723 | R-shipwreck / shipwreck.rs |
| second / overworld | feature_biome/deep_dark/budding_amethyst | 62.4113 | 26.2085 | 5.2417 | F / vegetation.rs |
| second / overworld | layer/badlands/-1/terracotta | 52.4431 | 40.9444 | 5 | A / terrain.rs |
| second / the_nether | layer/nether_wastes/-5/netherrack | 85.3228 | 73.8326 | 5 | N / nether.rs |
| second / overworld | depth/16/underground_air | 11.618 | 7.04459 | 2 | U / underground_carvers.rs |
| second / overworld | depth/64/air | 79.7043 | 75.1555 | 2 | U / underground_carvers.rs |
| second / overworld | top/river/sand | 14.4973 | 25.8505 | 5 | A / terrain.rs |
| second / overworld | layer/old_growth_birch_forest/-2/stone | 4.14899 | 15.4601 | 5 | A / terrain.rs |
| second / overworld | feature_biome/windswept_savanna/acacia_log | 40.478 | 6.5712 | 5 | F-tree / trees.rs |
| second / overworld | patches/cherry_grove/p50 | 848 | 704 | 64 | A / terrain.rs |
| second / overworld | biomes_3d/deep_ocean | 2.07002 | 0.390492 | 1 | A / terrain.rs |
| second / overworld | surface_biomes/deep_ocean | 2.07002 | 0.390615 | 1 | A / terrain.rs |
| second / overworld | layer/birch_forest/-1/dirt | 100 | 88.8114 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-5/brown_terracotta | 4.8601 | 16.0376 | 5 | A / terrain.rs |
| second / overworld | top/mangrove_swamp/grass_block | 1.81548 | 12.9417 | 5 | A / terrain.rs |
| second / overworld | ore/coal_ore/y128 | 0.873312 | 0.047518 | 0.25 | U / underground.rs |
| second / the_end | height_biome/end_barrens/p100 | 58 | 69 | 5 | E / the_end.rs |
| second / the_end | height_biome/end_barrens/p95 | 46 | 57 | 5 | E / the_end.rs |
| second / overworld | height_biome/jungle/p100 | 126 | 115 | 5 | A / terrain.rs |
| second / overworld | depth/16/air | 11.4249 | 7.03211 | 2 | U / underground_carvers.rs |
| second / overworld | blocks/mangrove_leaves | 0.538967 | 1.56587 | 0.313174 | F-tree / trees.rs |
| second / overworld | layer/beach/-3/stone | 6.47429 | 17.4006 | 5 | A / terrain.rs |
| second / the_end | biomes_3d/end_highlands | 17.4055 | 15.7688 | 1 | E / the_end.rs |
| second / the_end | surface_biomes/end_highlands | 17.4055 | 15.7688 | 1 | E / the_end.rs |
| second / the_nether | layer/nether_wastes/-4/netherrack | 87.7017 | 76.8046 | 5 | N / nether.rs |
| second / overworld | layer/ocean/-3/stone | 56.1917 | 67.0606 | 5 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-1/yellow_terracotta | 1.50495 | 12.3534 | 5 | A / terrain.rs |
| second / overworld | blocks/wildflowers | 0.057773 | 0.382051 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/grove/pointed_dripstone | 32.4127 | 0 | 5 | U / underground_carvers.rs |
| second / the_nether | layer/nether_wastes/-5/air | 11.5795 | 22.3466 | 5 | N / nether.rs |
| second / overworld | structure_candidate_spacing_p95/buried_treasure | 135.764 | 698.891 | 174.723 | R-shipwreck / shipwreck.rs |
| second / overworld | layer/desert/-2/sandstone | 0.082885 | 10.6662 | 5 | A / terrain.rs |
| second / overworld | layer/savanna/-2/dirt | 90.9532 | 80.3794 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-1/brown_terracotta | 7.23011 | 17.7596 | 5 | A / terrain.rs |
| second / overworld | feature_biome/river/oak_leaves | 51.4708 | 19.9482 | 5 | F-tree / trees.rs |
| second / overworld | layer/eroded_badlands/-5/brown_terracotta | 6.17822 | 16.6606 | 5 | A / terrain.rs |
| second / overworld | blocks/podzol | 0.004349 | 0.318452 | 0.1 | A / terrain.rs |
| second / overworld | layer/jungle/-4/stone | 84.6241 | 74.1697 | 5 | A / terrain.rs |
| second / overworld | layer/forest/-4/stone | 85.3119 | 74.9674 | 5 | A / terrain.rs |
| second / overworld | depth/32/underground_air | 8.54922 | 4.41634 | 2 | U / underground_carvers.rs |
| second / overworld | top/sparse_jungle/dirt | 13.9481 | 3.69487 | 5 | A / terrain.rs |
| second / overworld | layer/grove/-3/dirt | 92.6381 | 82.4405 | 5 | A / terrain.rs |
| second / overworld | layer/grove/-3/stone | 5.28507 | 15.4762 | 5 | A / terrain.rs |
| second / overworld | blocks/water | 3444.95 | 2143.55 | 428.709 | A / terrain.rs |
| second / overworld | layer/cherry_grove/-4/stone | 92.1014 | 82.0255 | 5 | A / terrain.rs |
| second / the_end | depth/16/air | 75.9891 | 71.9609 | 2 | E / the_end.rs |
| second / overworld | feature_biome/mangrove_swamp/lily_pad | 43.5931 | 13.4935 | 5 | F / vegetation.rs |
| second / overworld | height_biome/river/p100 | 91 | 81 | 5 | A / terrain.rs |
| second / overworld | height_biome/river/p95 | 72 | 62 | 5 | A / terrain.rs |
| second / overworld | structure_candidate_count/ancient_city | 2 | 0 | 1 | R-ancient_city / ancient_city.rs |
| second / overworld | structure_candidate_count/buried_treasure | 4 | 2 | 1 | R-shipwreck / shipwreck.rs |
| second / overworld | structure_candidate_count/jungle_pyramid | 0 | 2 | 1 | R-temples / temples.rs |
| second / overworld | structure_candidate_count/village | 6 | 12 | 3 | R-village / village.rs |
| second / overworld | tree_span/lukewarm_ocean/jungle_log/p50 | 9 | 3 | 2 | F-tree / trees.rs |
| second / overworld | layer/badlands/-5/yellow_terracotta | 1.76838 | 11.7263 | 5 | A / terrain.rs |
| second / overworld | structure_candidate_spacing_p95/abandoned_camp | 1489.48 | 852.967 | 213.242 | R-abandoned_camp / abandoned_camp.rs |
| second / overworld | blocks/moss_block | 7.28468 | 18.0556 | 3.61112 | A / terrain.rs |
| second / overworld | layer/river/-4/dirt | 2.1545 | 12.0952 | 5 | A / terrain.rs |
| second / overworld | layer/cherry_grove/-3/dirt | 25.1258 | 35.0365 | 5 | A / terrain.rs |
| second / overworld | height_mean/river | 60.7908 | 53.3689 | 5 | A / terrain.rs |
| second / the_nether | layer/nether_wastes/-4/air | 9.26782 | 19.1566 | 5 | N / nether.rs |
| second / the_nether | ore/nether_quartz_ore/y112 | 12.1093 | 7.61065 | 1.52213 | N / nether.rs |
| second / overworld | feature_biome/cherry_grove/sculk | 29.5007 | 0 | 5 | U / underground_carvers.rs |
| second / the_end | depth/48/air | 76.1732 | 72.2478 | 2 | E / the_end.rs |
| second / the_nether | blocks/twisting_vines | 0.292929 | 0 | 0.1 | N / nether.rs |
| second / overworld | layer/warm_ocean/-1/sand | 0.071311 | 9.83006 | 5 | A / terrain.rs |
| second / overworld | blocks/sunflower | 0.360493 | 0.068243 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/cherry_grove/snow_block | 29.1692 | 0 | 5 | A / terrain.rs |
| second / overworld | layer/flower_forest/-4/stone | 86.0403 | 76.3271 | 5 | A / terrain.rs |
| second / overworld | layer/stony_shore/-2/stone | 82.6124 | 72.9025 | 5 | A / terrain.rs |
| second / overworld | feature_biome/plains/oak_log | 36.844 | 7.8688 | 5 | F-tree / trees.rs |
| second / overworld | layer/flower_forest/-2/stone | 1.76777 | 11.4101 | 5 | A / terrain.rs |
| second / overworld | layer/ocean/-2/stone | 56.0242 | 65.6308 | 5 | A / terrain.rs |
| second / overworld | layer/forest/-2/dirt | 92.3574 | 82.7549 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/deep_lukewarm_ocean | 3.37543 | 1.94384 | 1 | A / terrain.rs |
| second / overworld | biomes_3d/deep_lukewarm_ocean | 3.37542 | 1.94396 | 1 | A / terrain.rs |
| second / overworld | blocks/coarse_dirt | 0.106875 | 0.392896 | 0.1 | A / terrain.rs |
| second / overworld | layer/ocean/-1/dirt | 14.5931 | 5.11283 | 5 | A / terrain.rs |
| second / the_nether | layer/nether_wastes/-3/netherrack | 89.8663 | 80.4242 | 5 | N / nether.rs |
| second / overworld | blocks/birch_log | 0.471261 | 1.08677 | 0.217353 | F-tree / trees.rs |
| second / overworld | layer/ocean/-1/stone | 55.8647 | 65.286 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/badlands | 0.803545 | 2.21617 | 1 | A / terrain.rs |
| second / overworld | blocks/deepslate_tiles | 0.281887 | 0 | 0.1 | F / vegetation.rs |
| second / overworld | layer/badlands/-1/orange_terracotta | 8.18543 | 17.5812 | 5 | A / terrain.rs |
| second / overworld | layer/eroded_badlands/-4/brown_terracotta | 5.67327 | 15.066 | 5 | A / terrain.rs |
| second / the_nether | blocks/nether_quartz_ore | 140.779 | 90.1038 | 18.0208 | N / nether.rs |
| second / overworld | top/beach/gravel | 7.77289 | 17.1425 | 5 | A / terrain.rs |
| second / overworld | blocks/birch_leaves | 4.0105 | 9.13485 | 1.82697 | F-tree / trees.rs |
| second / overworld | surface_biomes/savanna | 5.1144 | 3.71293 | 1 | A / terrain.rs |
| second / overworld | layer/warm_ocean/-1/dirt | 10.2672 | 0.946279 | 5 | A / terrain.rs |
| second / overworld | horizontal_log_blocks/oak_log | 0.247255 | 0.553733 | 0.110747 | F-tree / trees.rs |
| second / overworld | feature_biome/warm_ocean/tube_coral_block | 209.564 | 99.5082 | 19.9016 | F-ocean / ocean.rs |
| second / overworld | surface_biomes/ocean | 2.86031 | 4.23652 | 1 | A / terrain.rs |
| second / overworld | feature_biome/sunflower_plains/oak_log | 29.2262 | 1.70274 | 5 | F-tree / trees.rs |
| second / the_nether | ore/nether_gold_ore/y112 | 3.92654 | 2.5326 | 0.50652 | N / nether.rs |
| second / overworld | layer/warm_ocean/-4/stone | 72.7153 | 63.5525 | 5 | A / terrain.rs |
| second / overworld | biomes_3d/ocean | 2.85443 | 4.22332 | 1 | A / terrain.rs |
| second / overworld | top/plains/stone | 10.3753 | 1.25098 | 5 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-2/stone | 18.9021 | 9.80974 | 5 | A / terrain.rs |
| second / the_nether | cave_component_volume/p50 | 48 | 105.5 | 21.1 | N / nether.rs |
| second / overworld | biomes_3d/badlands | 0.772583 | 2.13441 | 1 | A / terrain.rs |
| second / overworld | blocks/red_mushroom_block | 0 | 0.27147 | 0.1 | F / vegetation.rs |
| second / overworld | layer/meadow/-4/dirt | 2.18225 | 11.2302 | 5 | A / terrain.rs |
| second / overworld | layer/jungle/-2/dirt | 94.0121 | 84.9654 | 5 | A / terrain.rs |
| second / overworld | blocks/bone_block | 0.008645 | 0.280034 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/warm_ocean/brain_coral_block | 206.934 | 99.4405 | 19.8881 | F-ocean / ocean.rs |
| second / overworld | feature_biome/warm_ocean/horn_coral_block | 207.803 | 99.9053 | 19.9811 | F-ocean / ocean.rs |
| second / overworld | height_biome/old_growth_birch_forest/p100 | 104 | 113 | 5 | A / terrain.rs |
| second / overworld | layer/beach/-2/stone | 6.57309 | 15.5666 | 5 | A / terrain.rs |
| second / overworld | depth/96/underground_air | 8.91845 | 5.34267 | 2 | U / underground_carvers.rs |
| second / overworld | feature_biome/warm_ocean/bubble_coral_block | 208.603 | 100.773 | 20.1546 | F-ocean / ocean.rs |
| second / overworld | top/windswept_savanna/sand | 11.8898 | 20.7923 | 5 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/fire_coral_block | 206.193 | 99.7758 | 19.9552 | F-ocean / ocean.rs |
| second / overworld | layer/cherry_grove/-5/stone | 91.8909 | 83.0292 | 5 | A / terrain.rs |
| second / overworld | layer/warm_ocean/-2/sand | 0.026007 | 8.87636 | 5 | A / terrain.rs |
| second / overworld | layer/deep_lukewarm_ocean/-1/dirt | 9.94588 | 1.11351 | 5 | A / terrain.rs |
| second / overworld | layer/beach/-1/stone | 6.7155 | 15.5198 | 5 | A / terrain.rs |
| second / overworld | layer/river/-1/stone | 15.8249 | 24.6256 | 5 | A / terrain.rs |
| second / overworld | top/eroded_badlands/brown_terracotta | 10.1114 | 1.35631 | 5 | A / terrain.rs |
| second / overworld | layer/warm_ocean/-2/dirt | 10.2536 | 1.50441 | 5 | A / terrain.rs |
| second / the_nether | depth/16/lava | 32.3459 | 35.8394 | 2 | N / nether.rs |
| second / overworld | feature_biome/cherry_grove/cherry_leaves | 2643.63 | 1296.96 | 259.391 | F-tree / trees.rs |
| second / overworld | layer/warm_ocean/-3/dirt | 10.2343 | 1.63946 | 5 | A / terrain.rs |
| second / overworld | height_mean/windswept_savanna | 61.8397 | 55.3954 | 5 | A / terrain.rs |
| second / overworld | top/sunflower_plains/grass_block | 78.1721 | 86.7274 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sulfur_caves/cave_vines_plant | 29.3979 | 3.81781 | 5 | U / underground_carvers.rs |
| second / overworld | nominal_climate_columns/[0.0, 0.5] | 1.27419 | 0 | 1 | A / terrain.rs |
| second / overworld | layer/birch_forest/-5/stone | 91.1496 | 82.7094 | 5 | A / terrain.rs |
| second / overworld | feature_biome/deep_lukewarm_ocean/kelp_plant | 1082.47 | 538.643 | 107.729 | F-ocean / ocean.rs |
| second / overworld | surface_biomes/plains | 6.20319 | 7.46416 | 1 | A / terrain.rs |
| second / overworld | layer/river/-5/stone | 77.2692 | 68.8685 | 5 | A / terrain.rs |
| second / overworld | structure_box_composition/abandoned_camp/air | 47.6479 | 60.2489 | 5 | R-abandoned_camp / abandoned_camp.rs |
| second / the_nether | blocks/nether_gold_ore | 46.2994 | 30.8476 | 6.16951 | N / nether.rs |
| second / overworld | feature_biome/deep_dark/sculk_shrieker | 30.0582 | 5.02494 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/beach/dark_oak_leaves | 0 | 26.4567 | 5.29134 | F-tree / trees.rs |
| second / overworld | feature_biome/beach/moss_block | 0 | 32.3202 | 6.46405 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/amethyst_block | 0 | 64.8888 | 12.9778 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/cinnabar | 0 | 76.2497 | 15.2499 | U / underground_carvers.rs |
| second / overworld | feature_biome/birch_forest/dark_oak_leaves | 0 | 27.162 | 5.43241 | F-tree / trees.rs |
| second / overworld | feature_biome/birch_forest/moss_block | 0 | 81.8842 | 16.3768 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/sulfur | 0 | 138.75 | 27.75 | U / underground_carvers.rs |
| second / overworld | feature_biome/desert/cinnabar | 0 | 132.069 | 26.4138 | U / underground_carvers.rs |
| second / overworld | feature_biome/desert/moss_block | 0 | 31.3124 | 6.26249 | A / terrain.rs |
| second / overworld | feature_biome/desert/sulfur | 0 | 154.857 | 30.9715 | U / underground_carvers.rs |
| second / overworld | feature_biome/dripstone_caves/cinnabar | 0 | 176.192 | 35.2383 | U / underground_carvers.rs |
| second / overworld | feature_biome/dripstone_caves/moss_block | 0 | 63.2222 | 12.6444 | U / underground_carvers.rs |
| second / overworld | feature_biome/dripstone_caves/sulfur | 0 | 170.965 | 34.1931 | U / underground_carvers.rs |
| second / overworld | feature_biome/flower_forest/cinnabar | 0 | 105.145 | 21.0289 | U / underground_carvers.rs |
| second / overworld | feature_biome/flower_forest/sulfur | 0 | 133.426 | 26.6852 | U / underground_carvers.rs |
| second / overworld | feature_biome/forest/cinnabar | 0 | 142.377 | 28.4754 | U / underground_carvers.rs |
| second / overworld | feature_biome/forest/sulfur | 0 | 152.878 | 30.5756 | U / underground_carvers.rs |
| second / overworld | feature_biome/grove/birch_leaves | 0 | 185.381 | 37.0763 | F-tree / trees.rs |
| second / overworld | feature_biome/grove/moss_block | 0 | 241.29 | 48.258 | A / terrain.rs |
| second / overworld | feature_biome/jungle/moss_block | 0 | 53.6424 | 10.7285 | A / terrain.rs |
| second / overworld | feature_biome/lush_caves/cinnabar | 0 | 62.6014 | 12.5203 | U / underground_carvers.rs |
| second / overworld | feature_biome/lush_caves/dripstone_block | 0 | 25.1527 | 5.03055 | U / underground_carvers.rs |
| second / overworld | feature_biome/lush_caves/sulfur | 0 | 91.2436 | 18.2487 | U / underground_carvers.rs |
| second / overworld | feature_biome/mangrove_swamp/cinnabar | 0 | 204.143 | 40.8286 | U / underground_carvers.rs |
| second / overworld | feature_biome/mangrove_swamp/oak_log | 0 | 25.4635 | 5.0927 | F-tree / trees.rs |
| second / overworld | feature_biome/mangrove_swamp/sulfur | 0 | 191.52 | 38.304 | U / underground_carvers.rs |
| second / overworld | feature_biome/old_growth_birch_forest/dark_oak_leaves | 0 | 54.098 | 10.8196 | F-tree / trees.rs |
| second / overworld | feature_biome/old_growth_birch_forest/dripstone_block | 0 | 34.9505 | 6.9901 | U / underground_carvers.rs |
| second / overworld | feature_biome/old_growth_birch_forest/moss_block | 0 | 117.226 | 23.4453 | A / terrain.rs |
| second / overworld | feature_biome/river/moss_block | 0 | 36.07 | 7.214 | A / terrain.rs |
| second / overworld | feature_biome/savanna/cinnabar | 0 | 40.5558 | 8.11117 | U / underground_carvers.rs |
| second / overworld | feature_biome/savanna/sulfur | 0 | 57.6075 | 11.5215 | U / underground_carvers.rs |
| second / overworld | feature_biome/savanna_plateau/oak_leaves | 0 | 170.875 | 34.175 | F-tree / trees.rs |
| second / overworld | feature_biome/sparse_jungle/moss_block | 0 | 33.3339 | 6.66678 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/cinnabar | 0 | 38.1402 | 7.62805 | U / underground_carvers.rs |
| second / overworld | feature_biome/warm_ocean/sulfur | 0 | 35.6298 | 7.12596 | U / underground_carvers.rs |
| second / overworld | feature_biome/windswept_savanna/brain_coral_block | 0 | 25.6277 | 5.12554 | F-ocean / ocean.rs |
| second / overworld | feature_biome/windswept_savanna/mossy_stone_bricks | 0 | 285.847 | 57.1694 | F / vegetation.rs |
| second / overworld | surface_band/badlands/light_gray_terracotta/y80 | 0 | 2.12199 | 0.424398 | A / terrain.rs |
| second / overworld | surface_band/badlands/light_gray_terracotta/y96 | 0 | 2.91262 | 0.582523 | A / terrain.rs |
| second / overworld | surface_band/badlands/orange_terracotta/y80 | 0 | 2.79221 | 0.558442 | A / terrain.rs |
| second / overworld | surface_band/badlands/white_terracotta/y80 | 0 | 2.11971 | 0.423942 | A / terrain.rs |
| second / overworld | surface_band/badlands/yellow_terracotta/y64 | 0 | 2.56608 | 0.513216 | A / terrain.rs |
| second / overworld | surface_band/badlands/yellow_terracotta/y80 | 0 | 8.0912 | 1.61824 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/brown_terracotta/y32 | 0 | 9.93347 | 1.98669 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/brown_terracotta/y64 | 0 | 1.55312 | 0.310623 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/brown_terracotta/y80 | 0 | 5.82727 | 1.16545 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/brown_terracotta/y96 | 0 | 1.50767 | 0.301533 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/light_gray_terracotta/y48 | 0 | 5.66662 | 1.13332 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/light_gray_terracotta/y64 | 0 | 1.62356 | 0.324712 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/light_gray_terracotta/y80 | 0 | 1.27083 | 0.254165 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/orange_terracotta/y48 | 0 | 25.873 | 5.17461 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/orange_terracotta/y64 | 0 | 18.1956 | 3.63912 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/orange_terracotta/y80 | 0 | 1.58807 | 0.317614 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/terracotta/y32 | 0 | 1.70356 | 0.340713 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/terracotta/y48 | 0 | 39.3877 | 7.87755 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/terracotta/y64 | 0 | 11.5657 | 2.31314 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/terracotta/y80 | 0 | 5.14961 | 1.02992 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/terracotta/y96 | 0 | 4.21907 | 0.843813 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/white_terracotta/y48 | 0 | 6.45786 | 1.29157 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/white_terracotta/y64 | 0 | 1.61747 | 0.323493 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/yellow_terracotta/y48 | 0 | 6.18599 | 1.2372 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/yellow_terracotta/y64 | 0 | 3.86537 | 0.773073 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/yellow_terracotta/y80 | 0 | 5.91202 | 1.1824 | A / terrain.rs |
| second / overworld | tree_span/warm_ocean/jungle_log/p50 | 9 | 4 | 2 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/brown_terracotta/y96 | 0.004967 | 4.77597 | 0.955193 | A / terrain.rs |
| second / the_nether | layer/nether_wastes/-3/air | 6.92249 | 15.231 | 5 | N / nether.rs |
| second / overworld | blocks/pale_hanging_moss | 0 | 0.249215 | 0.1 | F / vegetation.rs |
| second / overworld | layer/warm_ocean/-5/stone | 72.7307 | 64.4259 | 5 | A / terrain.rs |
| second / the_end | height_mean/end_midlands | 52.3031 | 46.0919 | 5 | E / the_end.rs |
| second / overworld | feature_biome/beach/amethyst_block | 194.613 | 97.8828 | 19.5766 | F / vegetation.rs |
| second / overworld | layer/savanna_plateau/-5/stone | 92.7183 | 84.4978 | 5 | A / terrain.rs |
| second / overworld | feature_biome/savanna_plateau/tall_grass | 4.45097 | 186.545 | 37.309 | F / vegetation.rs |
| second / the_end | patches/small_end_islands/p50 | 152 | 256 | 64 | E / the_end.rs |
| second / the_nether | ore/nether_quartz_ore/y96 | 18.877 | 12.6915 | 2.53829 | N / nether.rs |
| second / the_nether | blocks/magma_block | 187.079 | 125.836 | 25.1671 | N / nether.rs |
| second / overworld | feature_biome/windswept_savanna/horn_coral_block | 0 | 24.3134 | 5 | F-ocean / ocean.rs |
| second / overworld | layer/deep_lukewarm_ocean/-2/dirt | 9.92382 | 1.8224 | 5 | A / terrain.rs |
| second / overworld | layer/meadow/-4/stone | 90.5725 | 82.4915 | 5 | A / terrain.rs |
| second / overworld | feature_biome/grove/sculk | 6.64066 | 216.278 | 43.2556 | U / underground_carvers.rs |
| second / overworld | height_mean/beach | 64.6105 | 58.5743 | 5 | A / terrain.rs |
| second / overworld | feature_biome/lush_caves/mossy_stone_bricks | 0 | 24.0935 | 5 | F / vegetation.rs |
| second / overworld | layer/stony_shore/-1/stone | 81.1601 | 73.1363 | 5 | A / terrain.rs |
| second / overworld | layer/stony_shore/-2/gravel | 2.05036 | 10.0641 | 5 | A / terrain.rs |
| second / overworld | height_biome/beach/p95 | 75 | 67 | 5 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/moss_block | 1.11634 | 26.7161 | 5.34322 | A / terrain.rs |
| second / overworld | layer/mangrove_swamp/-1/stone | 1.80059 | 9.76667 | 5 | A / terrain.rs |
| second / overworld | feature_biome/snowy_slopes/powder_snow | 32.6816 | 734.748 | 146.95 | A / terrain.rs |
| second / overworld | biomes_3d/savanna | 4.88863 | 3.69484 | 1 | A / terrain.rs |
| second / overworld | blocks/tall_seagrass | 20.3667 | 13.7874 | 2.75749 | F-ocean / ocean.rs |
| second / the_nether | depth/32/underground_air | 39.8805 | 43.0486 | 2 | N / nether.rs |
| second / overworld | surface_band/wooded_badlands/orange_terracotta/y32 | 0 | 1.18774 | 0.25 | A / terrain.rs |
| second / overworld | layer/deep_lukewarm_ocean/-3/dirt | 9.81819 | 1.94702 | 5 | A / terrain.rs |
| second / overworld | feature_biome/meadow/dripstone_block | 25.7385 | 456.619 | 91.3238 | U / underground_carvers.rs |
| second / overworld | surface_band/wooded_badlands/white_terracotta/y80 | 0 | 1.17928 | 0.25 | A / terrain.rs |
| second / overworld | top/eroded_badlands/red_terracotta | 7.81683 | 0 | 5 | A / terrain.rs |
| second / overworld | layer/plains/-1/stone | 10.9712 | 3.17395 | 5 | A / terrain.rs |
| second / overworld | layer/river/-1/sand | 14.2458 | 22.0221 | 5 | A / terrain.rs |
| second / overworld | depth/160/underground_air | 15.0863 | 11.9816 | 2 | U / underground_carvers.rs |
| second / overworld | layer/eroded_badlands/-5/yellow_terracotta | 1.01238 | 8.72434 | 5 | A / terrain.rs |
| second / the_nether | blocks/polished_blackstone_brick_stairs | 0.235996 | 0.00551 | 0.1 | N / nether.rs |
| second / overworld | feature_biome/savanna_plateau/short_grass | 74.7294 | 916.308 | 183.262 | F / vegetation.rs |
| second / overworld | ore/deepslate_copper_ore/y0 | 3.38221 | 2.31802 | 0.463604 | U / underground.rs |
| second / overworld | surface_band/badlands/terracotta/y96 | 1.30465 | 15.5186 | 3.10372 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/brown_terracotta/y48 | 0 | 1.1436 | 0.25 | A / terrain.rs |
| second / overworld | blocks/horn_coral_fan | 0.382024 | 0.154474 | 0.1 | F-ocean / ocean.rs |
| second / the_end | window1/blocks/purpur_block | 0 | 1.50781 | 1 | E / the_end.rs |
| second / overworld | feature_biome/deep_dark/sculk | 12081.2 | 6348.86 | 1269.77 | U / underground_carvers.rs |
| second / overworld | blocks/horn_coral | 0.381943 | 0.15638 | 0.1 | F-ocean / ocean.rs |
| second / overworld | blocks/fire_coral_fan | 0.379795 | 0.154313 | 0.1 | F-ocean / ocean.rs |
| second / overworld | feature_biome/jungle/dripstone_block | 9.01435 | 90.7349 | 18.147 | U / underground_carvers.rs |
| second / the_nether | tree_span/nether_wastes/crimson_stem/p50 | 7 | 2.5 | 2 | N / nether.rs |
| second / overworld | blocks/tube_coral | 0.376977 | 0.153346 | 0.1 | F-ocean / ocean.rs |
| second / overworld | top/snowy_slopes/powder_snow | 0.982533 | 8.43243 | 5 | A / terrain.rs |
| second / overworld | surface_band/badlands/brown_terracotta/y80 | 0.861902 | 7.9688 | 1.59376 | A / terrain.rs |
| second / overworld | feature_biome/meadow/sculk | 4.33795 | 39.9969 | 7.99938 | U / underground_carvers.rs |
| second / overworld | blocks/bubble_coral_fan | 0.379876 | 0.157078 | 0.1 | F-ocean / ocean.rs |
| second / overworld | feature_biome/jungle/bamboo | 0 | 22.2348 | 5 | F / vegetation.rs |
| second / overworld | blocks/bubble_coral | 0.380413 | 0.158179 | 0.1 | F-ocean / ocean.rs |
| second / the_nether | layer/nether_wastes/-2/netherrack | 91.7956 | 84.396 | 5 | N / nether.rs |
| second / overworld | feature_biome/savanna_plateau/acacia_leaves | 45.5638 | 405.922 | 81.1843 | F-tree / trees.rs |
| second / overworld | blocks/brain_coral | 0.379983 | 0.158098 | 0.1 | F-ocean / ocean.rs |
| second / overworld | blocks/stone_bricks | 0.104003 | 0.325781 | 0.1 | F / vegetation.rs |
| second / overworld | height_mean/sparse_jungle | 65.0221 | 70.5586 | 5 | A / terrain.rs |
| second / overworld | feature_biome/savanna_plateau/snow | 22.1377 | 0 | 5 | A / terrain.rs |
| second / overworld | feature_biome/grove/cave_vines_plant | 0 | 22.0692 | 5 | F / vegetation.rs |
| second / overworld | height_mean/savanna | 75.9073 | 70.3948 | 5 | A / terrain.rs |
| second / overworld | blocks/fire_coral | 0.37891 | 0.159038 | 0.1 | F-ocean / ocean.rs |
| second / overworld | feature_biome/cherry_grove/budding_amethyst | 10.4413 | 83.6746 | 16.7349 | F / vegetation.rs |
| second / overworld | blocks/tube_coral_fan | 0.376117 | 0.157534 | 0.1 | F-ocean / ocean.rs |
| second / overworld | surface_band/badlands/terracotta/y80 | 1.61859 | 12.8444 | 2.56888 | A / terrain.rs |
| second / overworld | surface_band/badlands/orange_terracotta/y96 | 0.168756 | 1.33808 | 0.267615 | A / terrain.rs |
| second / overworld | feature_biome/sunflower_plains/dripstone_block | 6.89965 | 54.6399 | 10.928 | U / underground_carvers.rs |
| second / overworld | blocks/short_dry_grass | 0.057934 | 0.275819 | 0.1 | F / vegetation.rs |
| second / the_nether | blocks/blackstone_slab | 0.217631 | 0 | 0.1 | N / nether.rs |
| second / overworld | feature_biome/snowy_slopes/sculk | 43.3486 | 21.6039 | 5 | U / underground_carvers.rs |
| second / overworld | blocks/brain_coral_fan | 0.375393 | 0.158098 | 0.1 | F-ocean / ocean.rs |
| second / overworld | feature_biome/sulfur_caves/moss_block | 6.29956 | 47.8543 | 9.57087 | U / underground_carvers.rs |
| second / overworld | layer/beach/-2/sandstone | 0.033822 | 7.26769 | 5 | A / terrain.rs |
| second / overworld | blocks/dirt_path | 0.078203 | 0.295015 | 0.1 | F / vegetation.rs |
| second / overworld | blocks/tall_dry_grass | 0.059867 | 0.276598 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/savanna_plateau/oak_log | 0 | 21.6392 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/savanna_plateau/acacia_log | 6.44219 | 46.2631 | 9.25263 | F-tree / trees.rs |
| second / overworld | layer/forest/-2/stone | 4.84204 | 11.9684 | 5 | A / terrain.rs |
| second / overworld | feature_biome/forest/dripstone_block | 26.2274 | 177.321 | 35.4641 | U / underground_carvers.rs |
| second / overworld | surface_band/wooded_badlands/brown_terracotta/y112 | 0 | 1.06003 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/mangrove_swamp/dripstone_block | 21.1738 | 0 | 5 | U / underground_carvers.rs |
| second / overworld | blocks/oak_leaves | 25.5223 | 44.2486 | 8.84972 | F-tree / trees.rs |
| second / overworld | blocks/infested_stone | 0 | 0.211361 | 0.1 | F / vegetation.rs |
| second / overworld | structure_candidate_spacing_p50/abandoned_camp | 1010.29 | 661.035 | 165.259 | R-abandoned_camp / abandoned_camp.rs |
| second / the_nether | blocks/netherrack | 18781.4 | 13208.1 | 2641.62 | N / nether.rs |
| second / overworld | feature_biome/meadow/pointed_dripstone | 9.35069 | 58.6943 | 11.7389 | U / underground_carvers.rs |
| second / overworld | layer/mangrove_swamp/-2/dirt | 4.24107 | 11.2417 | 5 | A / terrain.rs |
| second / the_end | height_biome/end_midlands/p100 | 62 | 69 | 5 | E / the_end.rs |
| second / overworld | surface_band/badlands/brown_terracotta/y64 | 0.001638 | 1.04875 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/cherry_grove/amethyst_block | 125.875 | 765.376 | 153.075 | F / vegetation.rs |
| second / overworld | ore/gold_ore/y48 | 0.193535 | 0.715724 | 0.25 | U / underground.rs |
| second / overworld | feature_biome/dripstone_caves/pointed_dripstone | 5503.09 | 2999.75 | 599.95 | U / underground_carvers.rs |
| second / overworld | top/deep_lukewarm_ocean/sand | 88.5013 | 95.451 | 5 | A / terrain.rs |
| second / overworld | surface_band/badlands/white_terracotta/y96 | 0.541518 | 3.26025 | 0.65205 | A / terrain.rs |
| second / overworld | surface_band/wooded_badlands/terracotta/y112 | 0 | 1.04183 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/cherry_grove/cherry_log | 153.387 | 83.6746 | 16.7349 | F-tree / trees.rs |
| second / overworld | layer/warm_ocean/-5/sandstone | 0.005044 | 6.89233 | 5 | A / terrain.rs |
| second / overworld | patches/beach/p50 | 128 | 40 | 64 | A / terrain.rs |
| second / overworld | feature_biome/badlands/sculk | 8.625 | 48.339 | 9.66781 | U / underground_carvers.rs |
| second / overworld | covered_grass_by/oak_leaves | 1.18248 | 5.28663 | 2 | A / terrain.rs |
| second / overworld | blocks/azalea | 0.124164 | 0.328975 | 0.1 | F / vegetation.rs |
| second / overworld | surface_band/wooded_badlands/white_terracotta/y96 | 0 | 1.01742 | 0.25 | A / terrain.rs |
| second / overworld | height_mean/badlands | 89.1957 | 94.2699 | 5 | A / terrain.rs |
| second / overworld | surface_band/badlands/orange_terracotta/y48 | 0.578432 | 3.06148 | 0.612296 | A / terrain.rs |
| second / overworld | blocks/gold_ore | 3.84708 | 6.45655 | 1.29131 | U / underground.rs |
| second / overworld | layer/jungle/-2/stone | 2.92188 | 9.64934 | 5 | A / terrain.rs |
| second / overworld | layer/badlands/-2/orange_terracotta | 9.06113 | 15.7769 | 5 | A / terrain.rs |
| second / overworld | top/warm_ocean/sand | 89.5547 | 96.2608 | 5 | A / terrain.rs |
| second / overworld | surface_biomes/frozen_ocean | 1.00187 | 0 | 1 | A / terrain.rs |
| second / overworld | tree_span/windswept_savanna/acacia_log/p50 | 4 | 8 | 2 | F-tree / trees.rs |
| second / the_nether | ore/nether_quartz_ore/y16 | 24.0413 | 17.1745 | 3.43489 | N / nether.rs |
| second / the_nether | layer/nether_wastes/-2/air | 4.56246 | 11.2005 | 5 | N / nether.rs |
| second / overworld | feature_biome/sulfur_caves/mossy_cobblestone | 47.3967 | 26.3956 | 5.27912 | U / underground_carvers.rs |
| second / overworld | layer/desert/-1/sand | 96.3715 | 89.7575 | 5 | A / terrain.rs |
| second / overworld | layer/sparse_jungle/-1/sand | 9.3804 | 2.7867 | 5 | A / terrain.rs |
| second / overworld | top/badlands/red_terracotta | 7.31494 | 0.747653 | 5 | A / terrain.rs |
| second / overworld | feature_biome/cherry_grove/tall_grass | 12.5958 | 59.0644 | 11.8129 | F / vegetation.rs |
| second / overworld | blocks/spruce_log | 0.203549 | 0.0069 | 0.1 | F-tree / trees.rs |
| second / overworld | cave_shape_proxy/other | 1.53609 | 2.52742 | 0.505485 | U / underground_carvers.rs |
| second / overworld | feature_biome/mangrove_swamp/bone_block | 0 | 19.5873 | 5 | F / vegetation.rs |
| second / the_end | blocks/purpur_stairs | 0 | 0.195862 | 0.1 | E / the_end.rs |
| second / overworld | top/mangrove_swamp/stone | 0.044643 | 6.55833 | 5 | A / terrain.rs |
| second / overworld | blocks/cave_vines | 0.804344 | 1.31993 | 0.263986 | F / vegetation.rs |
| second / overworld | feature_biome/sulfur_caves/oak_log | 5.6996 | 25.869 | 5.1738 | U / underground_carvers.rs |
| second / overworld | layer/badlands/-4/yellow_terracotta | 2.17295 | 8.64438 | 5 | A / terrain.rs |
| second / overworld | top/mangrove_swamp/dirt | 9.61309 | 3.15833 | 5 | A / terrain.rs |
| second / overworld | top/old_growth_birch_forest/gravel | 3.08192 | 9.53679 | 5 | A / terrain.rs |
| second / overworld | ore/coal_ore/y80 | 4.99154 | 3.59873 | 0.719745 | U / underground.rs |
| second / overworld | feature_biome/mangrove_swamp/tall_seagrass | 41.5172 | 22.1989 | 5 | F-ocean / ocean.rs |
| second / overworld | depth/32/underground_water | 5.95034 | 3.38692 | 2 | U / underground_carvers.rs |
| second / overworld | layer/warm_ocean/-3/sand | 0.015438 | 6.41233 | 5 | A / terrain.rs |
| second / overworld | depth/48/underground_water | 4.80526 | 2.24839 | 2 | U / underground_carvers.rs |
| second / overworld | feature_biome/warm_ocean/tube_coral_wall_fan | 109.261 | 61.9268 | 12.3854 | F-ocean / ocean.rs |
| second / overworld | top/sparse_jungle/sand | 9.3804 | 3.01232 | 5 | A / terrain.rs |
| second / the_nether | blocks/fire | 0.372819 | 0.603306 | 0.120661 | N / nether.rs |
| second / overworld | layer/windswept_savanna/-2/sand | 4.10168 | 10.4085 | 5 | A / terrain.rs |
| second / overworld | blocks/cracked_deepslate_bricks | 0.188972 | 0 | 0.1 | F / vegetation.rs |
| second / overworld | surface_band/badlands/orange_terracotta/y64 | 1.34071 | 5.43231 | 1.08646 | A / terrain.rs |
| second / overworld | layer/warm_ocean/-4/sandstone | 0 | 6.27651 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sulfur_caves/dripstone_block | 0 | 18.8258 | 5 | U / underground_carvers.rs |
| second / overworld | top/plains/grass_block | 83.9448 | 90.2165 | 5 | A / terrain.rs |
| second / overworld | top/ocean/sand | 10.6027 | 4.33681 | 5 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/bubble_coral_wall_fan | 109.26 | 62.3753 | 12.4751 | F-ocean / ocean.rs |
| second / overworld | patches/windswept_savanna/p50 | 528 | 608 | 64 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/horn_coral_wall_fan | 109.661 | 62.6974 | 12.5395 | F-ocean / ocean.rs |
| second / overworld | layer/old_growth_birch_forest/-1/dirt | 89.1702 | 82.9344 | 5 | A / terrain.rs |
| second / overworld | layer/savanna/-2/stone | 6.71915 | 12.9352 | 5 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/brain_coral_wall_fan | 108.906 | 62.5709 | 12.5142 | F-ocean / ocean.rs |
| second / overworld | layer/plains/-1/dirt | 86.3704 | 92.5342 | 5 | A / terrain.rs |
| second / overworld | layer/windswept_savanna/-1/sand | 11.3663 | 17.5197 | 5 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/fire_coral_wall_fan | 109.046 | 62.8136 | 12.5627 | F-ocean / ocean.rs |
| second / overworld | layer/eroded_badlands/-1/red_terracotta | 6.13119 | 0 | 5 | A / terrain.rs |
| second / overworld | top/ocean/dirt | 9.70185 | 3.57316 | 5 | A / terrain.rs |
| second / overworld | feature_biome/old_growth_birch_forest/dark_oak_log | 0 | 18.2278 | 5 | F-tree / trees.rs |
| second / the_nether | ore/nether_gold_ore/y96 | 6.3067 | 4.62259 | 0.924518 | N / nether.rs |
| second / overworld | top/savanna/grass_block | 87.4672 | 81.4044 | 5 | A / terrain.rs |
| second / overworld | surface_band/badlands/yellow_terracotta/y48 | 0 | 0.904266 | 0.25 | A / terrain.rs |
| second / overworld | height_biome/deep_lukewarm_ocean/p100 | 53 | 59 | 5 | A / terrain.rs |
| second / overworld | height_biome/mangrove_swamp/p95 | 69 | 75 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sulfur_caves/budding_amethyst | 99.593 | 57.9913 | 11.5983 | U / underground_carvers.rs |
| second / overworld | ore/deepslate_iron_ore/y-48 | 5.66963 | 4.17775 | 0.83555 | U / underground.rs |
| second / the_nether | layer/crimson_forest/-4/netherrack | 87.4866 | 81.5517 | 5 | N / nether.rs |
| second / overworld | layer/sparse_jungle/-4/stone | 81.0843 | 75.1579 | 5 | A / terrain.rs |
| second / overworld | feature_biome/mangrove_swamp/muddy_mangrove_roots | 192.225 | 656.174 | 131.235 | F / vegetation.rs |
| second / overworld | feature_biome/grove/oak_log | 0 | 17.6554 | 5 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/brown_terracotta/y112 | 0.237053 | 1.11501 | 0.25 | A / terrain.rs |
| second / overworld | layer/stony_shore/-3/stone | 81.3085 | 75.4567 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sparse_jungle/oak_log | 39.6571 | 22.1566 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/meadow/cornflower | 15.6166 | 51.2971 | 10.2594 | F / vegetation.rs |
| second / overworld | layer/birch_forest/-1/stone | 0 | 5.79139 | 5 | A / terrain.rs |
| second / the_end | depth/32/air | 64.1293 | 61.815 | 2 | E / the_end.rs |
| second / overworld | feature_biome/birch_forest/powder_snow | 0 | 17.2404 | 5 | A / terrain.rs |
| second / overworld | feature_biome/cherry_grove/mossy_cobblestone | 17.2364 | 0 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/desert/bone_block | 0.268068 | 17.471 | 5 | F / vegetation.rs |
| second / the_nether | ore/nether_gold_ore/y16 | 7.74564 | 5.77227 | 1.15445 | N / nether.rs |
| second / overworld | layer/sunflower_plains/-5/stone | 88.8846 | 83.2367 | 5 | A / terrain.rs |
| second / overworld | layer/sunflower_plains/-2/dirt | 79.0882 | 84.7345 | 5 | A / terrain.rs |
| second / the_nether | structure_box_composition/ruined_portal/lava | 0.802139 | 9.25926 | 5 | N / ruined_portal.rs |
| second / overworld | surface_band/badlands/red_terracotta/y80 | 0.84115 | 0 | 0.25 | A / terrain.rs |
| second / overworld | layer/lukewarm_ocean/-1/sand | 0.222262 | 5.82458 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sulfur_caves/sulfur_spike | 670.753 | 2016.99 | 403.398 | U / underground_carvers.rs |
| second / overworld | blocks/emerald_ore | 0.198717 | 0.032779 | 0.1 | U / underground.rs |
| second / overworld | blocks/poplar_log | 0.165615 | 0 | 0.1 | F-tree / trees.rs |
| second / the_nether | layer/crimson_forest/-3/netherrack | 89.8006 | 84.3008 | 5 | N / nether.rs |
| second / overworld | depth/64/underground_air | 5.05267 | 7.24753 | 2 | U / underground_carvers.rs |
| second / overworld | layer/stony_shore/-1/gravel | 5.4946 | 10.9567 | 5 | A / terrain.rs |
| second / overworld | feature_biome/old_growth_birch_forest/flowering_azalea_leaves | 16.3243 | 0 | 5 | F-tree / trees.rs |
| second / overworld | layer/savanna_plateau/-5/dirt | 1.7736 | 7.20524 | 5 | A / terrain.rs |
| second / overworld | blocks/powder_snow | 0.003973 | 0.166823 | 0.1 | A / terrain.rs |
| second / overworld | cave_component_volume/p99 | 7327.97 | 5529 | 1105.8 | U / underground_carvers.rs |
| second / the_end | cave_shape_proxy/thin_like | 0.047449 | 0.210132 | 0.1 | E / the_end.rs |
| second / overworld | blocks/moss_carpet | 0.797229 | 1.17971 | 0.235942 | F / vegetation.rs |
| second / overworld | feature_biome/mangrove_swamp/oak_leaves | 16.1917 | 0 | 5 | F-tree / trees.rs |
| second / overworld | layer/ocean/-1/sand | 9.77078 | 4.40389 | 5 | A / terrain.rs |
| second / overworld | ore/coal_ore/y144 | 0.405004 | 0.003329 | 0.25 | U / underground.rs |
| second / the_end | window2/blocks/end_stone | 3602.19 | 4576.76 | 915.352 | E / the_end.rs |
| second / overworld | layer/eroded_badlands/-2/yellow_terracotta | 1.33168 | 6.65323 | 5 | A / terrain.rs |
| second / overworld | layer/sparse_jungle/-1/stone | 6.72911 | 1.41097 | 5 | A / terrain.rs |
| second / overworld | blocks/tall_grass | 1.17582 | 1.72644 | 0.345287 | F / vegetation.rs |
| second / overworld | top/snowy_slopes/stone | 5.84061 | 0.551044 | 5 | A / terrain.rs |
| second / overworld | feature_biome/old_growth_birch_forest/cave_vines_plant | 0 | 15.803 | 5 | F / vegetation.rs |
| second / the_nether | cave_shape_proxy/thin_like | 0.822773 | 0.626263 | 0.125253 | N / nether.rs |
| second / overworld | top/desert/sand | 96.9663 | 91.7389 | 5 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/sculk | 0 | 15.6786 | 5 | U / underground_carvers.rs |
| second / overworld | layer/eroded_badlands/-4/yellow_terracotta | 1.14356 | 6.35997 | 5 | A / terrain.rs |
| second / overworld | feature_biome/eroded_badlands/sculk | 15.6474 | 0 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/lush_caves/cave_vines_plant | 2736.14 | 1689.84 | 337.969 | F / vegetation.rs |
| second / overworld | top/birch_forest/gravel | 0 | 5.15656 | 5 | A / terrain.rs |
| second / overworld | blocks/short_grass | 10.8416 | 15.6915 | 3.13829 | F / vegetation.rs |
| second / overworld | ore/deepslate_lapis_ore/y0 | 1.59583 | 1.21031 | 0.25 | U / underground.rs |
| second / overworld | feature_biome/sulfur_caves/amethyst_block | 1053.53 | 651.662 | 130.332 | U / underground_carvers.rs |
| second / overworld | cave_shape_proxy/thin_like | 1.70227 | 2.46034 | 0.492067 | U / underground_carvers.rs |
| second / overworld | blocks/sand | 164.664 | 237.954 | 47.5908 | A / terrain.rs |
| second / overworld | top/sparse_jungle/gravel | 5.96542 | 0.860866 | 5 | A / terrain.rs |
| second / overworld | surface_band/eroded_badlands/white_terracotta/y64 | 0.794169 | 0.029612 | 0.25 | A / terrain.rs |
| second / the_nether | feature_biome/crimson_forest/crimson_fungus | 61.5857 | 38.2581 | 7.65161 | N / nether.rs |
| second / overworld | feature_biome/meadow/amethyst_block | 67.19 | 171.399 | 34.2799 | F / vegetation.rs |
| second / overworld | structure_candidate_spacing_p95/shipwreck | 412.912 | 665.877 | 166.469 | R-shipwreck / shipwreck.rs |
| second / overworld | blocks/iron_chain | 0.072002 | 0.223523 | 0.1 | F / vegetation.rs |
| second / overworld | top/savanna/gravel | 1.01477 | 6.06458 | 5 | A / terrain.rs |
| second / overworld | feature_biome/sparse_jungle/oak_leaves | 803.483 | 500.36 | 100.072 | F-tree / trees.rs |
| second / overworld | blocks/big_dripleaf | 0.062337 | 0.213724 | 0.1 | F / vegetation.rs |
| second / overworld | surface_band/eroded_badlands/light_gray_terracotta/y64 | 0.791082 | 0.034363 | 0.25 | A / terrain.rs |
| second / the_nether | blocks/gilded_blackstone | 0.150597 | 0 | 0.1 | N / nether.rs |
| second / overworld | top/warm_ocean/dirt | 5.05516 | 0.042414 | 5 | A / terrain.rs |
| second / overworld | blocks/oak_log | 2.30307 | 3.2933 | 0.658659 | F-tree / trees.rs |
| second / overworld | layer/river/-2/sand | 5.40542 | 10.4058 | 5 | A / terrain.rs |
| second / the_nether | structure_candidate_offset_p95/ruined_portal | 48 | 0 | 32 | N / ruined_portal.rs |
| second / overworld | tree_span/beach/jungle_log/p50 | 4 | 1 | 2 | F-tree / trees.rs |
| second / overworld | tree_span/desert/oak_log/p50 | 1 | 4 | 2 | F-tree / trees.rs |
| second / overworld | tree_span/meadow/oak_log/p50 | 1 | 4 | 2 | F-tree / trees.rs |
| second / overworld | tree_span/river/oak_log/p50 | 1 | 4 | 2 | F-tree / trees.rs |
| second / overworld | feature_biome/cherry_grove/snow | 14.8332 | 0 | 5 | A / terrain.rs |
| second / overworld | feature_biome/grove/cave_vines | 0 | 14.7128 | 5 | F / vegetation.rs |
| second / overworld | blocks/seagrass | 17.1642 | 13.2646 | 2.65292 | F-ocean / ocean.rs |
| second / overworld | feature_biome/desert/mossy_stone_bricks | 14.6395 | 0.003793 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/meadow/azure_bluet | 15.4238 | 36.9116 | 7.38233 | F / vegetation.rs |
| second / overworld | feature_biome/mangrove_swamp/mangrove_roots | 567.125 | 1354.35 | 270.87 | F / vegetation.rs |
| second / overworld | surface_band/wooded_badlands/light_gray_terracotta/y32 | 0 | 0.725791 | 0.25 | A / terrain.rs |
| second / overworld | surface_band/badlands/white_terracotta/y48 | 0 | 0.72504 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/jungle/sulfur | 0 | 14.4274 | 5 | U / underground_carvers.rs |
| second / overworld | structure_box_composition/shipwreck/air | 7.64158 | 0.434664 | 5 | R-shipwreck / shipwreck.rs |
| second / overworld | feature_biome/river/leaf_litter | 17.982 | 3.60782 | 5 | F / vegetation.rs |
| second / the_end | blocks/purpur_pillar | 0 | 0.143418 | 0.1 | E / the_end.rs |
| second / overworld | feature_biome/lukewarm_ocean/kelp_plant | 748.217 | 476.698 | 95.3397 | F-ocean / ocean.rs |
| second / overworld | feature_biome/sparse_jungle/dripstone_block | 40.6611 | 94.4673 | 18.8935 | U / underground_carvers.rs |
| second / overworld | ore/gold_ore/y64 | 0.085237 | 0.440817 | 0.25 | U / underground.rs |
| second / overworld | feature_biome/meadow/leaf_litter | 14.1706 | 0 | 5 | F / vegetation.rs |
| second / overworld | blocks/orange_tulip | 0.025316 | 0.166501 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/eroded_badlands/budding_amethyst | 14.0357 | 0 | 5 | F / vegetation.rs |
| second / overworld | surface_band/wooded_badlands/light_gray_terracotta/y96 | 0 | 0.698569 | 0.25 | A / terrain.rs |
| second / overworld | blocks/mangrove_roots | 0.039061 | 0.178314 | 0.1 | F / vegetation.rs |
| second / overworld | object_geometry_proxy/bone_body/volume_p95 | 1 | 12 | 4 | F / vegetation.rs |
| second / the_nether | blocks/nether_brick_stairs | 0.097337 | 0.233242 | 0.1 | N / nether.rs |
| second / overworld | feature_biome/plains/leaf_litter | 16.3281 | 2.77788 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/flower_forest/dripstone_block | 8.56167 | 21.8664 | 5 | U / underground_carvers.rs |
| second / the_nether | feature_biome/crimson_forest/crimson_stem | 1044.81 | 682.004 | 136.401 | N / nether.rs |
| second / overworld | ore/copper_ore/y16 | 27.3333 | 21.5927 | 4.31854 | U / underground.rs |
| second / the_end | blocks/end_stone_bricks | 0 | 0.132358 | 0.1 | E / the_end.rs |
| second / overworld | structure_candidate_spacing_p95/village | 1192.98 | 896.971 | 224.243 | R-village / village.rs |
| second / overworld | feature_biome/beach/oak_leaves | 14.1206 | 29.885 | 5.977 | F-tree / trees.rs |
| second / the_end | feature_biome/small_end_islands/chorus_plant | 13.179 | 0 | 5 | E / the_end.rs |
| second / overworld | feature_biome/mangrove_swamp/seagrass | 25.3255 | 12.1876 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/stony_shore/oak_leaves | 3.45675 | 16.5625 | 5 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/brown_terracotta/y32 | 0 | 0.652742 | 0.25 | A / terrain.rs |
| second / overworld | ore/iron_ore/y-16 | 0 | 0.325834 | 0.25 | U / underground.rs |
| second / overworld | surface_band/eroded_badlands/terracotta/y80 | 0.956455 | 0.306183 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/lush_caves/mossy_cobblestone | 52.8485 | 34.7693 | 6.95387 | F / vegetation.rs |
| second / overworld | feature_biome/river/tall_seagrass | 140.528 | 292.696 | 58.5391 | F-ocean / ocean.rs |
| second / the_nether | feature_biome/crimson_forest/crimson_roots | 506.44 | 333.659 | 66.7318 | N / nether.rs |
| second / the_end | feature_biome/end_highlands/chorus_plant | 326.428 | 676.496 | 135.299 | E / the_end.rs |
| second / overworld | feature_biome/deep_dark/mossy_cobblestone | 39.4176 | 26.0115 | 5.20229 | F / vegetation.rs |
| second / overworld | feature_biome/river/seagrass | 108.916 | 224.456 | 44.8912 | F-ocean / ocean.rs |
| second / overworld | feature_biome/sparse_jungle/leaf_litter | 14.5577 | 1.69797 | 5 | F / vegetation.rs |
| second / overworld | blocks/acacia_log | 0.318827 | 0.190421 | 0.1 | F-tree / trees.rs |
| second / overworld | feature_biome/savanna/acacia_leaves | 512.966 | 340.53 | 68.106 | F-tree / trees.rs |
| second / the_end | cave_component_volume/p99 | 7813.36 | 10458.6 | 2091.72 | E / the_end.rs |
| second / overworld | feature_biome/meadow/birch_log | 13.785 | 1.15233 | 5 | F-tree / trees.rs |
| second / overworld | structure_box_composition/abandoned_camp/dirt | 14.2552 | 7.94438 | 5 | R-abandoned_camp / abandoned_camp.rs |
| second / overworld | blocks/cracked_deepslate_tiles | 0.125829 | 0 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/meadow/oak_leaves | 16.9662 | 34.1238 | 6.82475 | F-tree / trees.rs |
| second / overworld | feature_biome/jungle/cinnabar | 0 | 12.5057 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/jungle/birch_leaves | 14.2226 | 1.73244 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/stony_shore/birch_leaves | 0 | 12.4219 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/dripstone_caves/amethyst_block | 662.514 | 442.612 | 88.5223 | U / underground_carvers.rs |
| second / overworld | feature_biome/warm_ocean/sea_pickle | 41.9504 | 28.0338 | 5.60677 | F-ocean / ocean.rs |
| second / overworld | feature_biome/cherry_grove/short_grass | 138.885 | 275.634 | 55.1268 | F / vegetation.rs |
| second / overworld | blocks/white_tulip | 0.022282 | 0.145292 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/sulfur_caves/cave_vines | 14.699 | 2.4355 | 5 | U / underground_carvers.rs |
| second / overworld | blocks/grass_block | 64.0624 | 84.6941 | 16.9388 | A / terrain.rs |
| second / overworld | blocks/deepslate_tile_stairs | 0.121319 | 0 | 0.1 | F / vegetation.rs |
| second / overworld | blocks/red_terracotta | 1.90695 | 1.53601 | 0.307203 | A / terrain.rs |
| second / overworld | feature_biome/forest/short_grass | 35.4974 | 68.5141 | 13.7028 | F / vegetation.rs |
| second / overworld | feature_biome/dripstone_caves/budding_amethyst | 59.5131 | 40.2272 | 8.04543 | U / underground_carvers.rs |
| second / overworld | feature_biome/jungle/oak_log | 83.0321 | 159.424 | 31.8849 | F-tree / trees.rs |
| second / overworld | feature_biome/old_growth_birch_forest/sculk | 0 | 11.9149 | 5 | U / underground_carvers.rs |
| second / overworld | blocks/big_dripleaf_stem | 0.139279 | 0.258128 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/mangrove_swamp/short_grass | 318.852 | 216.766 | 43.3532 | F / vegetation.rs |
| second / overworld | feature_biome/cherry_grove/yellow_poplar_leaves | 11.6014 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/lush_caves/short_grass | 845.942 | 1547.36 | 309.472 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/mossy_cobblestone | 0 | 11.3303 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/jungle/oak_leaves | 1874.15 | 3420.49 | 684.099 | F-tree / trees.rs |
| second / overworld | ore/iron_ore/y0 | 12.4676 | 16.0976 | 3.21951 | U / underground.rs |
| second / overworld | feature_biome/beach/leaf_litter | 1.49128 | 12.6964 | 5 | F / vegetation.rs |
| second / overworld | tree_base_estimates/flower_forest/oak_log | 0.022041 | 0.133963 | 0.1 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/white_terracotta/y112 | 0.559317 | 0.000107 | 0.25 | A / terrain.rs |
| second / the_end | blocks/chorus_flower | 1.10631 | 0.905458 | 0.181092 | E / the_end.rs |
| second / overworld | blocks/deepslate_copper_ore | 8.70163 | 7.12432 | 1.42487 | U / underground.rs |
| second / overworld | surface_band/wooded_badlands/orange_terracotta/y96 | 0 | 0.550485 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/lush_caves/big_dripleaf | 83.3774 | 148.299 | 29.6599 | F / vegetation.rs |
| second / overworld | surface_band/eroded_badlands/orange_terracotta/y64 | 0.887782 | 0.340761 | 0.25 | A / terrain.rs |
| second / overworld | blocks/mud | 0.602137 | 0.769309 | 0.153862 | A / terrain.rs |
| second / the_nether | ore/nether_gold_ore/y0 | 3.9449 | 5.03949 | 1.0079 | N / nether.rs |
| second / overworld | surface_band/badlands/light_gray_terracotta/y48 | 0 | 0.542619 | 0.25 | A / terrain.rs |
| second / overworld | blocks/snow_block | 1.75932 | 1.44602 | 0.289205 | A / terrain.rs |
| second / overworld | blocks/cobbled_deepslate | 0.13697 | 0.028672 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/old_growth_birch_forest/mossy_cobblestone | 0 | 10.828 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/sparse_jungle/seagrass | 11.5457 | 0.74047 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/meadow/powder_snow | 0 | 10.7427 | 5 | A / terrain.rs |
| second / overworld | feature_biome/snowy_slopes/snow_block | 6279.19 | 10985.1 | 2197.03 | A / terrain.rs |
| second / overworld | ore/coal_ore/y16 | 20.179 | 16.6356 | 3.32711 | U / underground.rs |
| second / overworld | blocks/flowering_azalea | 0.081506 | 0.187898 | 0.1 | F / vegetation.rs |
| second / overworld | blocks/large_amethyst_bud | 0.162582 | 0.056565 | 0.1 | F / vegetation.rs |
| second / overworld | ore/coal_ore/y64 | 9.08108 | 7.49811 | 1.49962 | U / underground.rs |
| second / overworld | tree_base_estimates/dark_forest/dark_oak_log | 0 | 0.105345 | 0.1 | F-tree / trees.rs |
| second / overworld | feature_biome/deep_ocean/kelp | 211.587 | 149.104 | 29.8208 | F-ocean / ocean.rs |
| second / overworld | blocks/pale_moss_carpet | 0 | 0.104674 | 0.1 | F / vegetation.rs |
| second / overworld | blocks/pink_petals | 0.114151 | 0.009772 | 0.1 | F / vegetation.rs |
| second / overworld | blocks/medium_amethyst_bud | 0.163897 | 0.059787 | 0.1 | F / vegetation.rs |
| second / overworld | ore/deepslate_iron_ore/y-32 | 6.75159 | 5.58928 | 1.11786 | U / underground.rs |
| second / overworld | blocks/amethyst_cluster | 0.163494 | 0.059921 | 0.1 | F / vegetation.rs |
| second / overworld | ore/lapis_ore/y32 | 1.54909 | 1.9531 | 0.39062 | U / underground.rs |
| second / overworld | feature_biome/grove/birch_log | 0 | 10.299 | 5 | F-tree / trees.rs |
| second / the_nether | blocks/brown_mushroom | 0.082645 | 0.185491 | 0.1 | N / nether.rs |
| second / the_end | feature_biome/end_highlands/chorus_flower | 49.7914 | 84.5447 | 16.9089 | E / the_end.rs |
| second / overworld | ore/iron_ore/y32 | 8.27061 | 10.4023 | 2.08046 | U / underground.rs |
| second / overworld | blocks/small_amethyst_bud | 0.161964 | 0.059814 | 0.1 | F / vegetation.rs |
| second / overworld | feature_biome/desert/sugar_cane | 15.5256 | 5.35717 | 5 | F / vegetation.rs |
| second / the_nether | blocks/ancient_debris | 1.65289 | 1.37374 | 0.274747 | N / nether.rs |
| second / the_nether | feature_biome/crimson_forest/weeping_vines | 187.539 | 133.389 | 26.6777 | N / nether.rs |
| second / overworld | feature_biome/river/acacia_leaves | 10.3843 | 0.239147 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/jungle/cocoa | 1.30207 | 11.4329 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/flower_forest/sculk | 2.17056 | 12.2944 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/meadow/budding_amethyst | 5.01274 | 15.1289 | 5 | F / vegetation.rs |
| second / overworld | structure_box_composition/ocean_ruin/water | 72.2303 | 67.1737 | 5 | R-ocean_ruin / ocean_ruin.rs |
| second / overworld | feature_biome/plains/dripstone_block | 26.7831 | 44.9297 | 8.98595 | U / underground_carvers.rs |
| second / overworld | ore/iron_ore/y112 | 0.334157 | 0.081774 | 0.25 | U / underground.rs |
| second / overworld | cave_shape_proxy/large_chamber_like | 0.976617 | 0.813606 | 0.162721 | U / underground_carvers.rs |
| second / overworld | surface_band/wooded_badlands/red_terracotta/y96 | 0 | 0.500711 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/grove/mossy_cobblestone | 9.80288 | 0 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/mangrove_swamp/mossy_cobblestone | 0 | 9.79365 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/beach/kelp_plant | 0 | 9.67006 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/stony_shore/leaf_litter | 0 | 9.6299 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/lush_caves/moss_carpet | 1067.71 | 771.925 | 154.385 | F / vegetation.rs |
| second / overworld | feature_biome/river/dark_oak_leaves | 0 | 9.54527 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/snowy_slopes/acacia_leaves | 9.53214 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/birch_forest/birch_leaves | 3861.46 | 2799.07 | 559.814 | F-tree / trees.rs |
| second / overworld | feature_biome/eroded_badlands/dead_bush | 73.6034 | 118.57 | 23.7141 | F / vegetation.rs |
| second / the_nether | feature_biome/nether_wastes/crimson_stem | 10.8361 | 1.39181 | 5 | N / nether.rs |
| second / overworld | feature_biome/meadow/mossy_cobblestone | 20.533 | 11.1144 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/ocean/sulfur | 0 | 9.33095 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/cherry_grove/pink_petals | 352.351 | 558.651 | 111.73 | F / vegetation.rs |
| second / overworld | feature_biome/sparse_jungle/jungle_log | 89.7556 | 65.8124 | 13.1625 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/terracotta/y128 | 0.45075 | 0 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/birch_forest/cave_vines_plant | 0 | 8.88049 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/jungle/mossy_cobblestone | 9.01435 | 17.8865 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/beach/budding_amethyst | 17.7789 | 9.0317 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/plains/cherry_leaves | 8.71585 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/old_growth_birch_forest/cave_vines | 0 | 8.65401 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/savanna/dripstone_block | 8.63089 | 0 | 5 | U / underground_carvers.rs |
| second / overworld | object_geometry_proxy/amethyst_body/volume_p95 | 321 | 490 | 98 | U / underground.rs |
| second / overworld | feature_biome/dripstone_caves/short_grass | 0 | 8.55957 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/lush_caves/oak_log | 4.21468 | 12.7737 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/old_growth_birch_forest/leaf_litter | 28.8281 | 20.2763 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/lush_caves/tall_grass | 386.615 | 587.174 | 117.435 | F / vegetation.rs |
| second / overworld | feature_biome/grove/spruce_leaves | 3649.52 | 2721.87 | 544.374 | F-tree / trees.rs |
| second / overworld | feature_biome/mangrove_swamp/mangrove_leaves | 7787.8 | 11726 | 2345.21 | F-tree / trees.rs |
| second / overworld | feature_biome/forest/amethyst_block | 88.7754 | 133.643 | 26.7285 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/dark_oak_log | 0 | 8.39053 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/meadow/poppy | 30.2692 | 45.2753 | 9.05506 | F / vegetation.rs |
| second / overworld | feature_biome/grove/budding_amethyst | 8.22177 | 0 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/meadow/cherry_leaves | 8.1939 | 0 | 5 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/brown_terracotta/y128 | 0.408172 | 0 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/deep_ocean/mossy_cobblestone | 7.81018 | 15.8755 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/beach/sugar_cane | 9.34383 | 1.3713 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/savanna/acacia_log | 62.4832 | 47.378 | 9.47559 | F-tree / trees.rs |
| second / overworld | feature_biome/grove/spruce_log | 403.183 | 306.026 | 61.2053 | F-tree / trees.rs |
| second / overworld | surface_band/badlands/orange_terracotta/y112 | 0.461704 | 0.06674 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/warm_ocean/horn_coral_fan | 16.2804 | 8.39162 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/windswept_savanna/amethyst_block | 190.866 | 145.224 | 29.0447 | F / vegetation.rs |
| second / overworld | feature_biome/sulfur_caves/short_grass | 0 | 7.8331 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/birch_forest/birch_log | 427.946 | 326.006 | 65.2011 | F-tree / trees.rs |
| second / overworld | feature_biome/warm_ocean/mossy_stone_bricks | 0.014915 | 7.8107 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/warm_ocean/fire_coral_fan | 16.1714 | 8.39309 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/warm_ocean/horn_coral | 16.2632 | 8.50486 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/eroded_badlands/mossy_cobblestone | 7.72298 | 0 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/warm_ocean/tube_coral | 16.0532 | 8.35191 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/sunflower_plains/sculk | 20.914 | 30.1629 | 6.03258 | U / underground_carvers.rs |
| second / overworld | feature_biome/warm_ocean/bubble_coral_fan | 16.184 | 8.55192 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/warm_ocean/bubble_coral | 16.2035 | 8.59457 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/warm_ocean/brain_coral | 16.176 | 8.59751 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/badlands/amethyst_block | 123.613 | 177.209 | 35.4418 | F / vegetation.rs |
| second / overworld | feature_biome/mangrove_swamp/mangrove_log | 518.55 | 743.229 | 148.646 | F-tree / trees.rs |
| second / overworld | feature_biome/meadow/oxeye_daisy | 8.0011 | 0.446062 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/river/jungle_leaves | 3.13087 | 10.6338 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/warm_ocean/fire_coral | 16.137 | 8.6431 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/warm_ocean/tube_coral_fan | 16.0165 | 8.55339 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/meadow/tall_grass | 95.4349 | 73.5259 | 14.7052 | F / vegetation.rs |
| second / overworld | feature_biome/lukewarm_ocean/amethyst_block | 181.466 | 139.826 | 27.9652 | F / vegetation.rs |
| second / overworld | feature_biome/deep_lukewarm_ocean/amethyst_block | 200.107 | 154.195 | 30.839 | F / vegetation.rs |
| second / overworld | feature_biome/grove/oak_leaves | 7.43121 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/warm_ocean/brain_coral_fan | 15.9798 | 8.59457 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/ocean/amethyst_block | 133.772 | 189.096 | 37.8191 | F / vegetation.rs |
| second / overworld | feature_biome/sparse_jungle/amethyst_block | 95.8798 | 134.919 | 26.9837 | F / vegetation.rs |
| second / overworld | feature_biome/stony_shore/kelp_plant | 0 | 7.16919 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/snowy_slopes/amethyst_block | 0 | 7.12928 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/meadow/wildflowers | 108.545 | 151.364 | 30.2728 | F / vegetation.rs |
| second / overworld | feature_biome/cherry_grove/red_poplar_leaves | 7.0437 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/old_growth_birch_forest/amethyst_block | 81.6217 | 113.171 | 22.6342 | F / vegetation.rs |
| second / overworld | feature_biome/forest/jungle_leaves | 20.2107 | 28.0138 | 5.60276 | F-tree / trees.rs |
| second / overworld | feature_biome/sunflower_plains/pointed_dripstone | 2.38901 | 9.21307 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/meadow/dandelion | 25.6421 | 35.2389 | 7.04778 | F / vegetation.rs |
| second / overworld | surface_band/badlands/red_terracotta/y96 | 0.564337 | 0.903809 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/sparse_jungle/jungle_leaves | 500.985 | 395.353 | 79.0707 | F-tree / trees.rs |
| second / overworld | surface_band/wooded_badlands/light_gray_terracotta/y112 | 0 | 0.331875 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/old_growth_birch_forest/seagrass | 8.16217 | 1.54685 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/old_growth_birch_forest/short_grass | 73.2859 | 99.6256 | 19.9251 | F / vegetation.rs |
| second / overworld | feature_biome/beach/birch_leaves | 0.768943 | 7.37667 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/desert/mangrove_leaves | 0 | 6.56135 | 5 | F-tree / trees.rs |
| second / the_nether | feature_biome/nether_wastes/brown_mushroom | 7.22407 | 13.7544 | 5 | N / nether.rs |
| second / overworld | surface_band/badlands/terracotta/y64 | 5.94628 | 8.03093 | 1.60619 | A / terrain.rs |
| second / overworld | surface_band/eroded_badlands/brown_terracotta/y80 | 0.561035 | 0.238476 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/desert/amethyst_block | 98.8797 | 132.763 | 26.5526 | F / vegetation.rs |
| second / overworld | surface_band/eroded_badlands/red_terracotta/y80 | 0.317082 | 0 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/deep_ocean/seagrass | 100.754 | 80.4965 | 16.0993 | F-ocean / ocean.rs |
| second / overworld | feature_biome/beach/dark_oak_log | 0 | 6.26544 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/birch_forest/rose_bush | 8.53048 | 2.26606 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/beach/jungle_leaves | 0.792245 | 7.04567 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/plains/cinnabar | 0 | 6.18182 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/deep_ocean/amethyst_block | 176.494 | 234.426 | 46.8852 | F / vegetation.rs |
| second / the_nether | feature_biome/crimson_forest/weeping_vines_plant | 417.268 | 335.062 | 67.0124 | N / nether.rs |
| second / overworld | feature_biome/meadow/short_grass | 206.39 | 272.99 | 54.598 | F / vegetation.rs |
| second / overworld | feature_biome/windswept_savanna/leaf_litter | 6.09552 | 0 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/lush_caves/azalea | 169.504 | 222.262 | 44.4524 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/wildflowers | 227.48 | 184.01 | 36.802 | F / vegetation.rs |
| second / overworld | surface_band/badlands/light_gray_terracotta/y112 | 0 | 0.294478 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/grove/tall_grass | 0 | 5.88512 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/deep_ocean/budding_amethyst | 15.6599 | 21.5403 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/budding_amethyst | 0 | 5.78763 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/lush_caves/powder_snow | 0 | 5.77412 | 5 | A / terrain.rs |
| second / overworld | feature_biome/plains/sulfur | 0 | 5.7582 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/plains/pointed_dripstone | 11.9972 | 6.35052 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/old_growth_birch_forest/pointed_dripstone | 0 | 5.64392 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/lush_caves/pointed_dripstone | 0 | 5.62873 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/sparse_jungle/tall_seagrass | 6.42546 | 0.81707 | 5 | F-ocean / ocean.rs |
| second / the_end | feature_biome/end_barrens/chorus_plant | 5.52723 | 0 | 5 | E / the_end.rs |
| second / overworld | surface_band/wooded_badlands/white_terracotta/y32 | 0 | 0.27496 | 0.25 | A / terrain.rs |
| second / overworld | surface_band/desert/terracotta/y48 | 0.032404 | 0.303713 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/cherry_grove/dripstone_block | 7.87237 | 2.46102 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/jungle/amethyst_block | 120.625 | 153.906 | 30.7811 | F / vegetation.rs |
| second / overworld | feature_biome/meadow/packed_ice | 5.39834 | 0 | 5 | A / terrain.rs |
| second / overworld | feature_biome/savanna_plateau/budding_amethyst | 5.38801 | 0 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/birch_forest/cave_vines | 0 | 5.35891 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/windswept_savanna/brain_coral_wall_fan | 0 | 5.25696 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/windswept_savanna/fire_coral_wall_fan | 0 | 5.25696 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/windswept_savanna/moss_block | 0 | 5.25696 | 5 | A / terrain.rs |
| second / overworld | feature_biome/windswept_savanna/oak_log | 5.23834 | 0 | 5 | F-tree / trees.rs |
| second / overworld | feature_biome/savanna_plateau/poppy | 0 | 5.22326 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/deep_lukewarm_ocean/tall_seagrass | 754.87 | 953.492 | 190.698 | F-ocean / ocean.rs |
| second / overworld | feature_biome/birch_forest/lilac | 5.68699 | 0.489958 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/badlands/dead_bush | 111.489 | 140.744 | 28.1487 | F / vegetation.rs |
| second / overworld | feature_biome/dripstone_caves/mossy_cobblestone | 33.0275 | 27.3454 | 5.46908 | U / underground_carvers.rs |
| second / overworld | feature_biome/windswept_savanna/seagrass | 15.0483 | 9.8568 | 5 | F-ocean / ocean.rs |
| second / overworld | feature_biome/savanna/amethyst_block | 150.803 | 190.214 | 38.0428 | F / vegetation.rs |
| second / overworld | feature_biome/ocean/budding_amethyst | 12.1698 | 17.3234 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/lush_caves/bone_block | 0.732988 | 5.87797 | 5 | F / vegetation.rs |
| second / overworld | feature_biome/warm_ocean/amethyst_block | 164.103 | 136.444 | 27.2888 | F / vegetation.rs |
| second / overworld | surface_band/eroded_badlands/terracotta/y128 | 0.251094 | 0 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/old_growth_birch_forest/moss_carpet | 0 | 5.01682 | 5 | F / vegetation.rs |
| second / overworld | surface_band/eroded_badlands/terracotta/y112 | 0.250664 | 0 | 0.25 | A / terrain.rs |
| second / overworld | feature_biome/sulfur_caves/pointed_dripstone | 0 | 5.00265 | 5 | U / underground_carvers.rs |
| second / overworld | feature_biome/flower_forest/white_tulip | 28.4987 | 35.6257 | 7.12514 | F / vegetation.rs |
