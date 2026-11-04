/// Сколько блоков вниз дают единицу глубины (вики: 1/128 на блок).
const DEPTH_BLOCKS: f64 = 128.0;

/// Промежуток глубины пышных и капельниковых пещер.
const CAVE_DEPTH: (f64, f64) = (0.2, 0.9);

/// Влажность пышных пещер (вики: 0,7…1,0).
const LUSH_HUMIDITY: f64 = 0.7;

/// Континентальность и глубина капельниковых пещер — с поправкой на наш
/// климат (см. начало файла).
const DRIPSTONE_CONTINENT: f64 = 1.0;
const DRIPSTONE_DEPTH: f64 = 0.5;

/// Эрозия глубокой тьмы (вики: −1,0…−0,375).
const DEEP_DARK_EROSION: f64 = -0.375;

/// Во сколько раз промах по климату весит больше промаха по глубине.
const CLIMATE_WEIGHT: f64 = 3.0;

/// Глубина глубокой тьмы — одна точка.
const DEEP_DARK_DEPTH: f64 = 1.1;

/// Выше этой эрозии глубокой тьмы не бывает ни на какой глубине. Промах
/// по эрозии весит втрое, а выигрыш по глубине у наземного биома даже у дна
/// мира (глубина около 3) — не больше 0,39 в квадрате расстояния: промах
/// должен быть меньше √0,39 / 3 ≈ 0,21. С запасом на то, что столбец каймы
/// стоит в блоке от середины соседской клетки, — 0,26.
const DEEP_DARK_NEAR: f64 = DEEP_DARK_EROSION + 0.26;

/// Выше этой глубины пещерный биом не может оказаться ближе наземного:
/// до промежутка пещерных глубин ещё дальше, чем до поверхности.
const NO_CAVES_ABOVE: f64 = CAVE_DEPTH.0 / 2.0;

/// Пещерный биом клетки, середина которой на высоте `y`, под столбцом
/// `column`. None — клетка остаётся наземного биома.
pub fn biome_at(column: &Column, y: i32) -> Option<Biome> {
    let depth = (column.height - y) as f64 / DEPTH_BLOCKS;

    if depth <= NO_CAVES_ABOVE {
        return None;
    }

    if column.climate_263 { cave_biome_263(&column.climate, depth) }
    else { cave_biome(&column.climate, depth) }
}

/// Именованные октавы 26.3 уже имеют масштаб таблицы биомов. Поправки
/// старого растянутого климата (вес ×3, точка капельника) здесь неверны.
/// Вики: «closest biome interval in the 6D space».
/// https://minecraft.wiki/w/World_generation#Overworld
pub fn cave_biome_263(climate: &Climate, depth: f64) -> Option<Biome> {
    if depth <= NO_CAVES_ABOVE { return None; }
    let surface = if depth <= 1.0 { depth.min(1.0 - depth) } else { depth - 1.0 };
    let caves = gap(depth, CAVE_DEPTH.0, CAVE_DEPTH.1).powi(2);
    let candidates = [
        (gap(climate.humidity, LUSH_HUMIDITY, 1.0).powi(2) + caves, Biome::LushCaves),
        (gap(climate.continent, 0.8, 1.0).powi(2) + caves, Biome::DripstoneCaves),
        (gap(climate.erosion, -1.0, DEEP_DARK_EROSION).powi(2) + (depth - DEEP_DARK_DEPTH).powi(2), Biome::DeepDark),
        // Сера участвует в том же выборе, а не перекрывает биомы отдельным
        // пятном шума. Вики: «Weirdness=-1.1~-0.85»; ничьи [проверить].
        (gap(climate.weirdness, -1.1, -0.85).powi(2)
            + gap(climate.continent, -0.19, 0.55).powi(2)
            + gap(climate.erosion, 0.45, 1.0).powi(2) + caves, Biome::SulfurCaves),
    ];
    let mut best = surface * surface;
    let mut chosen = None;
    for (distance, biome) in candidates {
        if distance < best { best = distance; chosen = Some(biome); }
    }
    chosen
}

/// Какой биом ближе всего по климату и глубине: пещерный или наземный
/// (None).
pub fn cave_biome(climate: &Climate, depth: f64) -> Option<Biome> {
    // Наземные биомы стоят на глубине 0 и 1, по остальным числам наш выбор
    // наземного биома точен — расстояние до него только по глубине.
    let surface = if depth <= 1.0 {
        depth.min(1.0 - depth)
    } else {
        depth - 1.0
    };
    let candidates = [
        (
            gap(climate.humidity, LUSH_HUMIDITY, 1.0),
            gap(depth, CAVE_DEPTH.0, CAVE_DEPTH.1),
            Biome::LushCaves,
        ),
        (
            gap(climate.continent, DRIPSTONE_CONTINENT, 1.0),
            depth - DRIPSTONE_DEPTH,
            Biome::DripstoneCaves,
        ),
        (
            gap(climate.erosion, -1.0, DEEP_DARK_EROSION),
            depth - DEEP_DARK_DEPTH,
            Biome::DeepDark,
        ),
    ];

    let mut best = surface * surface;
    let mut chosen = None;

    for (along, deep, biome) in candidates {
        let along = along * CLIMATE_WEIGHT;
        let distance = along * along + deep * deep;

        if distance < best {
            best = distance;
            chosen = Some(biome);
        }
    }

    chosen
}

/// Насколько число лежит за промежутком (0 — внутри).
fn gap(value: f64, low: f64, high: f64) -> f64 {
    if value < low {
        low - value
    } else if value > high {
        value - high
    } else {
        0.0
    }
}

/// Есть ли под этим местом пышные пещеры — хоть на какой-то глубине.
/// Середина их промежутка — самое выгодное для них место.
fn lush_below(column: &Column) -> bool {
    let depth = (CAVE_DEPTH.0 + CAVE_DEPTH.1) / 2.0;
    let biome = if column.climate_263 { cave_biome_263(&column.climate, depth) }
        else { cave_biome(&column.climate, depth) };
    biome == Some(Biome::LushCaves)
}

