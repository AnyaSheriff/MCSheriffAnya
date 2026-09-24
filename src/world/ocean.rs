// Подводное и морское: коралловые рифы и морские огурцы тёплого океана,
// айсберги и лёд пятнами замёрзших океанов, магма на дне подводных пещер
// и пузырьковые колонны над ней.
//
// Проход идёт по готовому чанку после растительности. Всё решается числом
// от координат и семени, поэтому соседние чанки сходятся без сговора. Что
// заходит к соседу — рифы и конусные айсберги, — ищется в полосе вокруг
// чанка, и каждый чанк ставит свою часть, как деревья и валуны.
//
// Что и где — по вики: «Coral Reef» (и китайская страница «珊瑚礁» —
// подробнее о формах), «Iceberg (feature)», «Terrain features» (большие
// айсберги), «Blue Ice (feature)», «Underwater Magma», «Bubble Column»,
// «Sea Pickle», «Biome» (пятна тепла у замёрзших океанов). Формы — свои,
// по описаниям; густота подобрана по переписи сохранённого мира оригинала:
// tools/measure/ocean_census.py против
// `cargo test --release ocean_census -- --ignored --nocapture`.

use std::sync::OnceLock;

use super::noise::Noise;
use super::terrain::{Biome, Column, Terrain, SEA};
use super::{Chunk, Generator, AIR, CHUNK_SIZE, MIN_Y, WORLD_HEIGHT};

/// Сторона квадрата столбцов, который чанк считает вместе с каймой.
const SIDE: i32 = CHUNK_SIZE + 2;

/// Ставит на чанк всё морское.
///
/// `columns` — столбцы чанка с каймой в блок, как их считает
/// `Chunk::generated`: по X, внутри — по Z.
pub(super) fn decorate(generator: &Generator, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, columns: &[Column]) {
    let Generator::Normal(terrain) = generator else {
        return;
    };

    let area = Area { terrain, columns, chunk_x, chunk_z, key: key_of(terrain) };
    let blocks = palette();

    // Порядок — как шаги оригинала: айсберги, голубой лёд, магма, кораллы
    // и огурцы. Лёд на воде оригинал ставит последним, но у нас он уже
    // лежит от рельефа: его пятна правятся первыми, а айсберги ставятся
    // поверх.
    let cold = area.cold();

    if cold {
        thaw(&area, chunk, blocks);
        large_icebergs(&area, chunk, blocks);
        cone_icebergs(&area, chunk, blocks);
        blue_ice(&area, chunk, blocks);
    }

    underwater_magma(&area, chunk, blocks);

    if area.warm() {
        reefs(&area, chunk, blocks);
        sea_pickles(&area, chunk, blocks);
    }
}

/// Семя мира, перемешанное для моря — своё, не как у растительности.
fn key_of(terrain: &Terrain) -> u64 {
    (terrain.seed() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x0CEA_2C0B
}

/// Чанк и его окрестность: откуда брать столбцы.
struct Area<'a> {
    terrain: &'a Terrain,
    columns: &'a [Column],
    chunk_x: i32,
    chunk_z: i32,
    /// Семя мира, перемешанное для моря.
    key: u64,
}

impl Area<'_> {
    /// Место в чанке по мировым координатам.
    fn local(&self, x: i32, z: i32) -> (i32, i32) {
        (x - self.chunk_x * CHUNK_SIZE, z - self.chunk_z * CHUNK_SIZE)
    }

    /// Столбец чанка или его каймы по местным координатам.
    fn near(&self, local_x: i32, local_z: i32) -> &Column {
        &self.columns[((local_x + 1) * SIDE + local_z + 1) as usize]
    }

    /// Столбец в любом месте: из каймы, если он в ней, иначе считаем заново —
    /// ответ тот же, только дороже.
    fn column(&self, x: i32, z: i32) -> Column {
        let (local_x, local_z) = self.local(x, z);

        if (-1..=CHUNK_SIZE).contains(&local_x) && (-1..=CHUNK_SIZE).contains(&local_z) {
            *self.near(local_x, local_z)
        } else {
            self.terrain.column_at(x, z)
        }
    }

    /// Есть ли в чанке с каймой тёплый океан. Биом один на клетку 4×4, а
    /// кайма задевает клетки соседей: риф, начатый в клетке, которой кайма
    /// не видит, лежит от чанка дальше, чем дотягиваются его блоки
    /// (`REEF_REACH`) и украшения на них.
    fn warm(&self) -> bool {
        self.columns.iter().any(|column| column.biome == Biome::WarmOcean)
    }

    /// Холодно ли возле чанка — не ближе ли замёрзший океан, чем
    /// дотягивается конусный айсберг. Температура меняется медленно, и
    /// запаса в 0,1 от границы самого холодного уровня хватает с лихвой.
    fn cold(&self) -> bool {
        self.columns.iter().any(|column| column.climate.temperature < FROZEN_BELOW + 0.1)
    }
}

/// Граница самого холодного уровня температуры: ниже неё море замёрзшее.
const FROZEN_BELOW: f64 = -0.45;

/// Замёрзший ли это океан.
fn frozen_sea(biome: Biome) -> bool {
    matches!(biome, Biome::FrozenOcean | Biome::DeepFrozenOcean)
}

// ---------------------------------------------------------------------------
// Случайность от места — как у растительности
// ---------------------------------------------------------------------------

/// Число от места и соли: одно и то же при каждом обращении.
fn hash(key: u64, x: i32, z: i32, salt: u64) -> u64 {
    let mut value = ((x as u32 as u64) << 32 | z as u32 as u64) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ key;

    // Перемешивание как у splitmix64: соседние места дают несхожие числа.
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// Доля от 0 до 1 по месту.
fn unit(key: u64, x: i32, z: i32, salt: u64) -> f64 {
    (hash(key, x, z, salt) >> 11) as f64 / (1u64 << 53) as f64
}

/// Выпало ли событие с такой вероятностью в этом месте.
fn roll(key: u64, x: i32, z: i32, salt: u64, part: f64) -> bool {
    part > 0.0 && unit(key, x, z, salt) < part
}

/// Кости для одной штуки: вереница чисел от её места.
struct Dice(u64);

impl Dice {
    fn new(key: u64, x: i32, z: i32, salt: u64) -> Dice {
        Dice(hash(key, x, z, salt))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);

        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// Число от 0 до `below - 1`.
    fn below(&mut self, below: i32) -> i32 {
        (self.next() % below.max(1) as u64) as i32
    }

    /// Число от `from` до `to` включительно.
    fn between(&mut self, from: i32, to: i32) -> i32 {
        from + self.below(to - from + 1)
    }

    /// Доля от 0 до 1.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// Соли: у каждой штуки своя, чтобы решения не совпадали.
const SALT_THAW: i64 = 72_001;
const SALT_BERG: i64 = 72_011;
const SALT_BERG_DEPTH: i64 = 72_013;
const SALT_CONE: u64 = 72_021;
const SALT_BLUE_CONE: u64 = 72_023;
const SALT_BLUE: u64 = 72_031;
const SALT_MAGMA: u64 = 72_041;
const SALT_REEF: u64 = 72_051;
const SALT_REEF_ZONE: i64 = 72_053;
const SALT_DECOR: u64 = 72_061;
const SALT_PICKLE: u64 = 72_071;

// ---------------------------------------------------------------------------
// Состояния блоков
// ---------------------------------------------------------------------------

/// Виды кораллов.
const KINDS: [&str; 5] = ["tube", "brain", "bubble", "fire", "horn"];

/// Стороны света в порядке настенных вееров: шаг к соседу и имя стороны.
const SIDES: [(i32, i32, &str); 4] = [(0, -1, "north"), (0, 1, "south"), (-1, 0, "west"), (1, 0, "east")];

/// Состояния всех блоков прохода — считаются один раз на запуск.
struct Palette {
    water: i32,
    ice: i32,
    packed_ice: i32,
    blue_ice: i32,
    snow_block: i32,
    magma: i32,
    bubble_column: i32,

    seagrass: i32,
    tall_seagrass: [i32; 2],
    /// Стебель и верхушки ламинарии: числа от и до.
    kelp: (i32, i32),
    kelp_plant: i32,

    /// Коралловые блоки, кораллы, веера и настенные веера (по `SIDES`) — по
    /// `KINDS`. Всё, у чего есть такое свойство, — в воде.
    coral_blocks: [i32; 5],
    corals: [i32; 5],
    fans: [i32; 5],
    wall_fans: [[i32; 4]; 5],
    /// Морские огурцы в воде: от одного до четырёх.
    pickles: [i32; 4],

    /// Природный камень и грунт: на нём лежит дно подводной пещеры, и его
    /// заменяет магма. Числа от и до.
    rock: Vec<(i32, i32)>,
    /// Дно, на котором лежат морские огурцы.
    sand: i32,
    gravel: i32,
}

impl Palette {
    /// Вода или то, что растёт в воде и что крупное затирает.
    fn watery(&self, block: i32) -> bool {
        block == self.water
            || block == self.seagrass
            || block == self.tall_seagrass[0]
            || block == self.tall_seagrass[1]
            || block == self.kelp_plant
            || (self.kelp.0..=self.kelp.1).contains(&block)
    }

    fn rock(&self, block: i32) -> bool {
        self.rock.iter().any(|(low, high)| (*low..=*high).contains(&block))
    }

    fn coral_block(&self, block: i32) -> bool {
        self.coral_blocks.contains(&block)
    }
}

fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();

    PALETTE.get_or_init(|| {
        let wet = |name: &str| state(&format!("{}[waterlogged=true]", name));

        Palette {
            water: state("water"),
            ice: state("ice"),
            packed_ice: state("packed_ice"),
            blue_ice: state("blue_ice"),
            snow_block: state("snow_block"),
            magma: state("magma_block"),
            bubble_column: state("bubble_column[drag=true]"),

            seagrass: state("seagrass"),
            tall_seagrass: [state("tall_seagrass[half=lower]"), state("tall_seagrass[half=upper]")],
            kelp: crate::blocks::states_of("kelp").expect("ламинарии нет в таблице"),
            kelp_plant: state("kelp_plant"),

            coral_blocks: KINDS.map(|kind| state(&format!("{}_coral_block", kind))),
            corals: KINDS.map(|kind| wet(&format!("{}_coral", kind))),
            fans: KINDS.map(|kind| wet(&format!("{}_coral_fan", kind))),
            wall_fans: KINDS.map(|kind| {
                SIDES.map(|(_, _, side)| state(&format!("{}_coral_wall_fan[facing={},waterlogged=true]", kind, side)))
            }),
            pickles: [1, 2, 3, 4].map(|count| state(&format!("sea_pickle[pickles={},waterlogged=true]", count))),

            rock: [
                "stone", "deepslate", "granite", "diorite", "andesite", "tuff", "dirt", "gravel", "sand", "clay",
                "calcite", "coarse_dirt",
            ]
            .iter()
            .filter_map(|name| crate::blocks::states_of(name))
            .collect(),
            sand: state("sand"),
            gravel: state("gravel"),
        }
    })
}

/// Состояние по записи; блока нет в таблице — это ошибка в коде, а не мира.
fn state(text: &str) -> i32 {
    crate::blocks::state_from_text(text).unwrap_or_else(|| panic!("блока {} нет в таблице", text))
}

// ---------------------------------------------------------------------------
// Постановка в чанк
// ---------------------------------------------------------------------------

/// Что стоит в чанке по местным координатам; за краем чанка — None.
fn block_in(chunk: &Chunk, local_x: i32, y: i32, local_z: i32) -> Option<i32> {
    let inside = (0..CHUNK_SIZE).contains(&local_x)
        && (0..CHUNK_SIZE).contains(&local_z)
        && (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y);

    inside.then(|| chunk.block(local_x, y, local_z))
}

/// Ставит блок вместо воды или водного растения. Высокая морская трава и
/// ламинария не остаются висеть обрубками: у травы уходит и другая половина,
/// у ламинарии — всё, что выше, а стебель под местом становится верхушкой.
fn put_in_water(chunk: &mut Chunk, blocks: &Palette, local_x: i32, y: i32, local_z: i32, state: i32) -> bool {
    let Some(here) = block_in(chunk, local_x, y, local_z) else {
        return false;
    };

    if !blocks.watery(here) {
        return false;
    }

    if here == blocks.tall_seagrass[0] && block_in(chunk, local_x, y + 1, local_z) == Some(blocks.tall_seagrass[1]) {
        chunk.put_generated(local_x, y + 1, local_z, blocks.water, true);
    }

    if here == blocks.tall_seagrass[1] && block_in(chunk, local_x, y - 1, local_z) == Some(blocks.tall_seagrass[0]) {
        chunk.put_generated(local_x, y - 1, local_z, blocks.seagrass, true);
    }

    if here == blocks.kelp_plant || (blocks.kelp.0..=blocks.kelp.1).contains(&here) {
        let mut above = y + 1;

        while let Some(up) = block_in(chunk, local_x, above, local_z)
            && (up == blocks.kelp_plant || (blocks.kelp.0..=blocks.kelp.1).contains(&up))
        {
            chunk.put_generated(local_x, above, local_z, blocks.water, true);
            above += 1;
        }

        if block_in(chunk, local_x, y - 1, local_z) == Some(blocks.kelp_plant) {
            chunk.put_generated(local_x, y - 1, local_z, blocks.kelp.0, true);
        }
    }

    chunk.put_generated(local_x, y, local_z, state, true);
    true
}

// ---------------------------------------------------------------------------
// Лёд пятнами
// ---------------------------------------------------------------------------

/// Поперечник пятен тепла на замёрзшем океане, блоков.
const THAW_SCALE: f64 = 48.0;

/// Выше этого значения шума пятно тёплое и вода не замерзает.
const THAW_ABOVE: f64 = -0.04;

/// Оттаивает воду там, где оригинал её не замораживает. У замёрзшего океана
/// температура по вики не везде одна: пятнами по шуму она как у простого
/// холодного места (0,2), и там вода у уровня моря открыта — лёд лежит
/// кусками. Глубокий замёрзший океан теплее (0,5) и не мёрзнет вовсе:
/// айсберги плавают в открытой воде.
fn thaw(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let mut noise = None;

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let biome = area.near(local_x, local_z).biome;

            if !frozen_sea(biome) || chunk.block(local_x, SEA, local_z) != blocks.ice {
                continue;
            }

            let open = biome == Biome::DeepFrozenOcean || {
                let noise = noise.get_or_insert_with(|| Noise::new(area.terrain.seed() ^ SALT_THAW));
                let (x, z) = (area.chunk_x * CHUNK_SIZE + local_x, area.chunk_z * CHUNK_SIZE + local_z);

                noise.octaves(x as f64 / THAW_SCALE, z as f64 / THAW_SCALE, 3) > THAW_ABOVE
            };

            if open {
                chunk.put_generated(local_x, SEA, local_z, blocks.water, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Большие айсберги
// ---------------------------------------------------------------------------

/// Поперечник полей больших айсбергов, блоков.
const BERG_SCALE: f64 = 55.0;

/// Порог шума, выше которого из воды поднимается айсберг: у глубокого
/// замёрзшего океана айсбергов больше.
const BERG_ABOVE: f64 = 0.15;
const DEEP_BERG_ABOVE: f64 = 0.10;

/// Удалённость от суши, ниже которой океан глубокий.
const DEEP_SEA_BELOW: f64 = -0.455;

/// На сколько температура должна уйти ниже границы замёрзшего океана,
/// чтобы айсберги выросли в полную высоту.
const BERG_FADE: f64 = 0.04;

/// Самая большая высота над водой.
const BERG_HIGHEST: f64 = 44.0;

/// На сколько шум поднимается над порогом, пока айсберг вырастает на
/// полтора десятка блоков.
const BERG_RISE: f64 = 0.15;

/// Большие айсберги — часть рельефа замёрзших океанов: поля плотного льда,
/// что пологими горбами поднимаются над водой на несколько блоков, изредка
/// — на три-четыре десятка, и уходят под воду на несколько блоков. Высота
/// над водой — от одного шума, глубина — от другого: у оригинала она с
/// высотой почти не связана.
fn large_icebergs(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let mut noises = None;

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let column = area.near(local_x, local_z);
            let biome = column.biome;

            if !frozen_sea(biome) {
                continue;
            }

            // У края холода айсберги сходят на нет, а не обрываются стеной
            // на границе с тёплым океаном.
            let chill = ((FROZEN_BELOW - column.climate.temperature) / BERG_FADE).clamp(0.0, 1.0);
            let surface = chunk.block(local_x, SEA, local_z);

            if surface != blocks.water && surface != blocks.ice {
                continue;
            }

            let (height, noise_depth) = noises.get_or_insert_with(|| {
                let seed = area.terrain.seed();

                (Noise::new(seed ^ SALT_BERG), Noise::new(seed ^ SALT_BERG_DEPTH))
            });
            let (x, z) = (
                (area.chunk_x * CHUNK_SIZE + local_x) as f64,
                (area.chunk_z * CHUNK_SIZE + local_z) as f64,
            );
            // Глубокий океан от простого отделяет удалённость от суши; по ней
            // же, плавно, и айсбергов больше, и уходят они глубже — без ступени
            // на границе биомов.
            let shallow = ((column.climate.continent - DEEP_SEA_BELOW + 0.05) / 0.1).clamp(0.0, 1.0);
            let threshold = DEEP_BERG_ABOVE + (BERG_ABOVE - DEEP_BERG_ABOVE) * shallow;
            let value = height.octaves(x / BERG_SCALE, z / BERG_SCALE, 4);

            if value <= threshold || chill <= 0.0 {
                continue;
            }

            // Над водой: пологий подъём от края, к середине поля круче.
            let rise = (value - threshold) / BERG_RISE * chill;
            let above = (14.0 * rise.powf(1.3)).min(BERG_HIGHEST) as i32;
            // Под водой: от нуля до полутора десятков, чаще мелко; над
            // глубоким океаном — глубже.
            let deep = (noise_depth.octaves(x / 23.0, z / 23.0, 2) + 0.55).max(0.0);
            let reach = 26.0 - 12.0 * shallow;
            let below = ((deep * deep * reach) as i32 + above / 12).min(26);

            for y in SEA - below..=SEA + above {
                let Some(here) = block_in(chunk, local_x, y, local_z) else {
                    break;
                };

                // Лёд идёт только сквозь воду, лёд и воздух: дно и берег он не
                // режет.
                if y < SEA && !blocks.watery(here) {
                    continue;
                }

                if here == AIR || here == blocks.ice || blocks.watery(here) {
                    chunk.put_generated(local_x, y, local_z, blocks.packed_ice, true);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Конусные айсберги
// ---------------------------------------------------------------------------

/// Как далеко от своей точки заходит конусный айсберг.
const CONE_REACH: i32 = 11;

/// Доля чанков, где пробуется конусный айсберг: по вики 1/16, у голубого —
/// ещё 1/200 независимо.
const CONE_CHANCE: f64 = 1.0 / 16.0;
const BLUE_CONE_CHANCE: f64 = 1.0 / 200.0;

/// Конусный айсберг: где стоит и какой.
struct Cone {
    x: i32,
    z: i32,
    /// Полуоси у воды: вдоль и поперёк, и поворот.
    long: f64,
    short: f64,
    turn: (f64, f64),
    /// Высота над водой и глубина под ней.
    above: i32,
    below: i32,
    /// Снежная шапка.
    snowy: bool,
    /// Выемка: середина шара и радиус.
    carve: Option<(f64, f64, f64, f64)>,
    state: i32,
}

/// Ставит части конусных айсбергов, что задевают чанк. Точка каждого — в
/// своём чанке; айсберг не шире 11 блоков от точки, поэтому хватает чанков
/// вокруг.
fn cone_icebergs(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    for chunk_dx in -1..=1 {
        for chunk_dz in -1..=1 {
            let (cone_x, cone_z) = (area.chunk_x + chunk_dx, area.chunk_z + chunk_dz);

            for (salt, chance, state) in
                [(SALT_CONE, CONE_CHANCE, blocks.packed_ice), (SALT_BLUE_CONE, BLUE_CONE_CHANCE, blocks.blue_ice)]
            {
                if !roll(area.key, cone_x, cone_z, salt, chance) {
                    continue;
                }

                let cone = cone_at(area, cone_x, cone_z, salt, state);

                if let Some(cone) = cone {
                    put_cone(area, chunk, blocks, &cone);
                }
            }
        }
    }
}

/// Айсберг, что начат в этом чанке, если место — замёрзший океан.
fn cone_at(area: &Area, cone_x: i32, cone_z: i32, salt: u64, state: i32) -> Option<Cone> {
    let mut dice = Dice::new(area.key, cone_x, cone_z, salt + 1);
    let x = cone_x * CHUNK_SIZE + dice.below(CHUNK_SIZE);
    let z = cone_z * CHUNK_SIZE + dice.below(CHUNK_SIZE);
    let column = area.column(x, z);

    if !frozen_sea(column.biome) || column.height >= SEA {
        return None;
    }

    // Размеры — по вики: круглый радиусом до 11 и высотой 3–17 (изредка
    // 10–42), глубиной 3–18; вытянутый (30%) — полуоси 7–11 и 3–5, высота
    // 6–11, глубина 6–18. Снег сверху — у трети, выемка — у 30% круглых и
    // 90% вытянутых.
    let long_shaped = dice.unit() < 0.3;
    let (long, short, above, below) = if long_shaped {
        let long = dice.between(7, 11) as f64;
        let short = dice.between(3, 5) as f64;

        (long, short, dice.between(6, 11), dice.between(6, 18))
    } else {
        let radius = dice.between(2, 11) as f64;
        let above = if dice.unit() < 0.9 { dice.between(3, 17) } else { dice.between(10, 42) };

        (radius, radius, above, dice.between(3, 18))
    };
    let angle = dice.unit() * std::f64::consts::PI;
    let snowy = dice.unit() < 0.3;
    let carved = dice.unit() < if long_shaped { 0.9 } else { 0.3 };
    // Выемка — шар сбоку у воды или снизу.
    let carve = carved.then(|| {
        let side = dice.unit() * std::f64::consts::TAU;
        let reach = long * 0.8;
        let depth = -(dice.below(below.max(1)) as f64);

        (side.cos() * reach, depth, side.sin() * reach, (short * 0.8).max(2.0))
    });

    Some(Cone {
        x,
        z,
        long: long + 0.5,
        short: short + 0.5,
        turn: (angle.cos(), angle.sin()),
        above,
        below,
        snowy,
        carve,
        state,
    })
}

/// Доля поперечника айсберга на высоте `dy` от воды. По вики он как
/// рожок мороженого: под водой — конус остриём вниз, над водой —
/// скруглённая шапка.
fn cone_share(cone: &Cone, dy: i32) -> f64 {
    if dy >= 0 {
        let part = dy as f64 / (cone.above as f64 + 1.0);

        1.0 - part * part
    } else {
        1.0 - -dy as f64 / (cone.below as f64 + 1.0)
    }
}

fn put_cone(area: &Area, chunk: &mut Chunk, blocks: &Palette, cone: &Cone) {
    let (local_x, local_z) = area.local(cone.x, cone.z);

    if local_x + CONE_REACH < 0
        || local_x - CONE_REACH >= CHUNK_SIZE
        || local_z + CONE_REACH < 0
        || local_z - CONE_REACH >= CHUNK_SIZE
    {
        return;
    }

    let inside = |dx: i32, dy: i32, dz: i32| {
        let share = cone_share(cone, dy);

        if share <= 0.0 {
            return false;
        }

        let (along, across) = (
            dx as f64 * cone.turn.0 + dz as f64 * cone.turn.1,
            -(dx as f64) * cone.turn.1 + dz as f64 * cone.turn.0,
        );
        let (a, b) = (cone.long * share, cone.short * share);

        if (along / a).powi(2) + (across / b).powi(2) > 1.0 {
            return false;
        }

        match cone.carve {
            Some((cx, cy, cz, radius)) => {
                (dx as f64 - cx).powi(2) + (dy as f64 - cy).powi(2) + (dz as f64 - cz).powi(2) > radius * radius
            }
            None => true,
        }
    };

    for dx in -CONE_REACH..=CONE_REACH {
        for dz in -CONE_REACH..=CONE_REACH {
            let (bx, bz) = (local_x + dx, local_z + dz);

            if !(0..CHUNK_SIZE).contains(&bx) || !(0..CHUNK_SIZE).contains(&bz) {
                continue;
            }

            for dy in -cone.below..=cone.above {
                if !inside(dx, dy, dz) {
                    continue;
                }

                let y = SEA + dy;
                let Some(here) = block_in(chunk, bx, y, bz) else {
                    continue;
                };

                // Как у оригинала: айсберг занимает только воздух, воду, лёд
                // и снег.
                if here != AIR && here != blocks.ice && here != blocks.snow_block && !blocks.watery(here) {
                    continue;
                }

                // Шапка — снег в блок толщиной по всей надводной части.
                let shell = dy > 0
                    && cone.snowy
                    && (!inside(dx, dy + 1, dz) || SIDES.iter().any(|(sx, sz, _)| !inside(dx + sx, dy, dz + sz)));
                let state = if shell { blocks.snow_block } else { cone.state };

                chunk.put_generated(bx, y, bz, state, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Голубой лёд
// ---------------------------------------------------------------------------

/// Сколько раз на чанк пробуется голубой лёд.
const BLUE_TRIES: i32 = 7;

/// Голубой лёд — комья у низа айсбергов, ниже уровня воды (вики, «Blue Ice
/// (feature)»): ком ложится под плотный лёд и частью заменяет его.
fn blue_ice(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let mut dice = Dice::new(area.key, area.chunk_x, area.chunk_z, SALT_BLUE);

    for _ in 0..BLUE_TRIES {
        // Ком целиком в чанке: он мал, а под водой шва не видно.
        let (local_x, local_z) = (dice.between(2, CHUNK_SIZE - 3), dice.between(2, CHUNK_SIZE - 3));
        let size = dice.between(2, 7);
        let mut seed = dice.next();

        if !frozen_sea(area.near(local_x, local_z).biome) || chunk.block(local_x, SEA, local_z) != blocks.packed_ice {
            continue;
        }

        // Низ айсберга в этом столбце.
        let mut bottom = SEA;

        while block_in(chunk, local_x, bottom - 1, local_z) == Some(blocks.packed_ice) {
            bottom -= 1;
        }

        // Ком — несколько шагов от низа: вниз в воду и вбок по льду.
        let (mut x, mut y, mut z) = (local_x, bottom, local_z);

        for _ in 0..size {
            if let Some(here) = block_in(chunk, x, y, z)
                && y <= SEA
                && (here == blocks.packed_ice || blocks.watery(here))
            {
                chunk.put_generated(x, y, z, blocks.blue_ice, true);
            }

            seed = seed.rotate_left(5) ^ 0x9E37_79B9;

            match seed % 6 {
                0 => y -= 1,
                1 => y += 1,
                2 => x = (x - 1).max(0),
                3 => x = (x + 1).min(CHUNK_SIZE - 1),
                4 => z = (z - 1).max(0),
                _ => z = (z + 1).min(CHUNK_SIZE - 1),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Магма и пузырьковые колонны
// ---------------------------------------------------------------------------

/// Сколько раз на чанк пробуется подводная магма.
const MAGMA_TRIES: i32 = 6;

/// Как далеко вниз ищется дно от пробной точки.
const MAGMA_SEARCH: i32 = 5;

/// Подводная магма: ком до 3×3×3 на дне воды в пещерах (вики, «Underwater
/// Magma»: дно ищется на пять блоков вниз, магма ложится в блоке от него с
/// вероятностью 1/2). Пробные точки — не выше чем в двух блоках под
/// поверхностью земли или дна моря: у оригинала на самом дне океана магмы
/// почти нет, она в затопленных пещерах. Над каждой магмой, что смотрит в
/// воду, до верха воды встаёт пузырьковая колонна, тянущая вниз.
fn underwater_magma(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let mut dice = Dice::new(area.key, area.chunk_x, area.chunk_z, SALT_MAGMA);

    for _ in 0..MAGMA_TRIES {
        // Ком целиком в чанке.
        let (local_x, local_z) = (dice.between(1, CHUNK_SIZE - 2), dice.between(1, CHUNK_SIZE - 2));
        let top = area.near(local_x, local_z).height - 2;
        let lowest = MIN_Y + 6;
        let start = lowest + dice.below(top - lowest + 1);
        let mut seed = dice.next();

        if top <= lowest {
            continue;
        }

        let Some(floor) = (0..=MAGMA_SEARCH).map(|down| start - down).find(|&y| {
            block_in(chunk, local_x, y, local_z) == Some(blocks.water)
                && block_in(chunk, local_x, y - 1, local_z).is_some_and(|below| blocks.rock(below))
        }) else {
            continue;
        };
        let floor = floor - 1;

        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    seed = seed.rotate_left(7) ^ 0xA076_1D64_78BD_642F;

                    let (x, y, z) = (local_x + dx, floor + dy, local_z + dz);

                    if seed >> 63 == 0 || !block_in(chunk, x, y, z).is_some_and(|here| blocks.rock(here)) {
                        continue;
                    }

                    chunk.put_generated(x, y, z, blocks.magma, true);
                    bubble_column(chunk, blocks, x, y + 1, z);
                }
            }
        }
    }
}

/// Пузырьковая колонна от этого места вверх, пока идёт вода.
fn bubble_column(chunk: &mut Chunk, blocks: &Palette, local_x: i32, from: i32, local_z: i32) {
    let mut y = from;

    while block_in(chunk, local_x, y, local_z) == Some(blocks.water) {
        chunk.put_generated(local_x, y, local_z, blocks.bubble_column, true);
        y += 1;
    }
}

// ---------------------------------------------------------------------------
// Коралловые рифы
// ---------------------------------------------------------------------------

/// Как далеко от своей точки заходит риф.
const REEF_REACH: i32 = 3;

/// Самая большая густота рифов — на столбец дна.
const REEF_DENSEST: f64 = 0.092;

/// Поперечник полей гуще и реже рифов, блоков.
const REEF_SCALE: f64 = 90.0;

/// Форма рифа.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Reef {
    Tree,
    Claw,
    Mushroom,
}

/// Густота рифов в этом месте: по полям шума — где гуще, где пусто.
fn reef_density(noise: &Noise, x: i32, z: i32) -> f64 {
    let value = noise.octaves(x as f64 / REEF_SCALE, z as f64 / REEF_SCALE, 2);

    (REEF_DENSEST * (value + 0.2) / 0.6).clamp(0.0, REEF_DENSEST)
}

/// Коралловые рифы тёплого океана. У оригинала их три формы — дерево,
/// клешня и гриб; каждый риф из коралловых блоков одного вида, а на блоках
/// растут кораллы, веера и морские огурцы любых видов (китайская вики,
/// «珊瑚礁»): сбоку у блока в 20% настенный веер, сверху в 25% коралл или
/// веер, иначе изредка морской огурец (`TOP_PICKLE`). Рифы не выходят из
/// воды.
///
/// Два прохода: сначала блоки всех рифов, что задевают чанк, потом
/// украшения — их решает число от места блока, поэтому соседние чанки
/// украшают общий риф одинаково.
fn reefs(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let noise = Noise::new(area.terrain.seed() ^ SALT_REEF_ZONE);
    let reach = REEF_REACH + 1;
    let mut placed: Vec<(i32, i32, i32)> = Vec::new();

    for local_x in -reach..CHUNK_SIZE + reach {
        for local_z in -reach..CHUNK_SIZE + reach {
            let (x, z) = (area.chunk_x * CHUNK_SIZE + local_x, area.chunk_z * CHUNK_SIZE + local_z);

            // Сперва дешёвое «а не здесь ли», и только потом столбец.
            if !roll(area.key, x, z, SALT_REEF, REEF_DENSEST)
                || unit(area.key, x, z, SALT_REEF + 1) * REEF_DENSEST >= reef_density(&noise, x, z)
            {
                continue;
            }

            let column = area.column(x, z);

            if column.biome != Biome::WarmOcean || SEA - column.height < 3 {
                continue;
            }

            let start = placed.len();
            reef_blocks(area.key, x, column.height + 1, z, &mut placed);

            for &(bx, by, bz) in &placed[start..] {
                let (bx, bz) = area.local(bx, bz);
                let state = placed_kind(area.key, x, z, blocks);

                put_in_water(chunk, blocks, bx, by, bz, state);
            }
        }
    }

    for &(x, y, z) in &placed {
        decorate_coral(area, chunk, blocks, x, y, z);
    }
}

/// Коралловый блок рифа с точкой в этом месте: вид один на весь риф.
fn placed_kind(key: u64, x: i32, z: i32, blocks: &Palette) -> i32 {
    blocks.coral_blocks[(hash(key, x, z, SALT_REEF + 2) % KINDS.len() as u64) as usize]
}

/// Все блоки рифа от точки на дне (мировые координаты). Блоки выше воды
/// не попадают в список.
fn reef_blocks(key: u64, x: i32, y: i32, z: i32, out: &mut Vec<(i32, i32, i32)>) {
    let mut dice = Dice::new(key, x, z, SALT_REEF + 3);
    let reef = match dice.below(3) {
        0 => Reef::Tree,
        1 => Reef::Claw,
        _ => Reef::Mushroom,
    };
    let mut put = |dx: i32, dy: i32, dz: i32| {
        let dx = dx.clamp(-REEF_REACH, REEF_REACH);
        let dz = dz.clamp(-REEF_REACH, REEF_REACH);

        // Верх рифа — не выше блока под поверхностью воды.
        if y + dy < SEA {
            out.push((x + dx, y + dy, z + dz));
        }
    };

    // Порядок сторон — перетасованный: ветви идут в разные стороны.
    let mut sides = [0, 1, 2, 3];

    for i in (1..4).rev() {
        sides.swap(i, dice.below(i as i32 + 1) as usize);
    }

    match reef {
        // Дерево: ствол в 1–3 блока, от его верха 2–4 ветви — вверх и
        // в стороны, ступенями.
        Reef::Tree => {
            let trunk = dice.between(1, 3);

            for dy in 0..trunk {
                put(0, dy, 0);
            }

            for &side in &sides[..dice.between(2, 4) as usize] {
                let (sx, sz, _) = SIDES[side];
                let (mut bx, mut by, mut bz) = (sx, trunk, sz);

                put(bx, by, bz);

                for _ in 0..dice.between(1, 4) {
                    by += 1;

                    if dice.below(3) != 0 {
                        bx += sx;
                        bz += sz;
                    }

                    put(bx, by, bz);
                }
            }
        }
        // Клешня: 2–3 ветви в стороны у дна, у конца загибаются вверх.
        Reef::Claw => {
            put(0, 0, 0);

            for &side in &sides[..dice.between(2, 3) as usize] {
                let (sx, sz, _) = SIDES[side];
                let (mut bx, mut by, mut bz) = (0, dice.below(2), 0);

                for _ in 0..dice.between(2, 3) {
                    bx += sx;
                    bz += sz;
                    put(bx, by, bz);
                }

                // Палец: вверх, иногда с шагом вбок.
                let (tx, tz) = (sz, sx);
                let bend = dice.below(3) - 1;

                for step in 0..dice.between(1, 2) {
                    by += 1;

                    if step == 1 {
                        bx += tx * bend;
                        bz += tz * bend;
                    }

                    put(bx, by, bz);
                }
            }
        }
        // Гриб: домик — редкие стены и крыша без углов, внутри вода.
        Reef::Mushroom => {
            let (width, length, height) = (dice.between(3, 4), dice.between(3, 4), dice.between(2, 4));
            let (from_x, from_z) = (-(width / 2), -(length / 2));

            for dx in from_x..from_x + width {
                for dz in from_z..from_z + length {
                    let edge_x = dx == from_x || dx == from_x + width - 1;
                    let edge_z = dz == from_z || dz == from_z + length - 1;

                    for dy in 0..height {
                        let roof = dy == height - 1;

                        // Угол крыши срезан — купол; в стенах проёмы.
                        if roof && edge_x && edge_z {
                            continue;
                        }

                        let chance = if roof { 4 } else { 2 };

                        if (roof || edge_x || edge_z) && dice.below(chance) != 0 {
                            put(dx, dy, dz);
                        }
                    }
                }
            }
        }
    }
}

/// Доля морских огурцов на верху кораллового блока, где не вырос коралл.
/// По китайской вики — 5%; у нас формы рифов плотнее, и верхов меньше,
/// поэтому доля поднята до той, что даёт перепись оригинала (огурцов на
/// коралловых блоках около четырёх на чанк).
const TOP_PICKLE: f64 = 0.07;

/// Украшает коралловый блок рифа (мировые координаты): сверху коралл,
/// веер или морской огурец, сбоку настенные веера. Решает число от места
/// блока. Ставится только в воду — на блок, которого не стало (там земля),
/// украшение садится как на землю.
fn decorate_coral(area: &Area, chunk: &mut Chunk, blocks: &Palette, x: i32, y: i32, z: i32) {
    let (local_x, local_z) = area.local(x, z);

    // Задевает ли блок с соседями чанк.
    if !(-1..=CHUNK_SIZE).contains(&local_x) || !(-1..=CHUNK_SIZE).contains(&local_z) {
        return;
    }

    let mut dice = Dice::new(area.key, x, z, SALT_DECOR ^ (y as u64 & 0x1FF) << 20);
    let top = dice.unit();
    let kind = dice.below(KINDS.len() as i32) as usize;
    let fan = dice.below(2) == 0;
    let pickles = dice.below(4) as usize;

    if top < 0.25 {
        let state = if fan { blocks.fans[kind] } else { blocks.corals[kind] };

        put_decoration(chunk, blocks, local_x, y + 1, local_z, state);
    } else if top < 0.25 + 0.75 * TOP_PICKLE {
        put_decoration(chunk, blocks, local_x, y + 1, local_z, blocks.pickles[pickles]);
    }

    for (side, (dx, dz, _)) in SIDES.iter().enumerate() {
        let chance = dice.unit();
        let kind = dice.below(KINDS.len() as i32) as usize;

        if chance < 0.2 {
            put_decoration(chunk, blocks, local_x + dx, y, local_z + dz, blocks.wall_fans[kind][side]);
        }
    }
}

/// Украшение — только под водой, в воду или вместо морской травы: у
/// оригинала трава растёт после кораллов и места у них не отнимает.
fn put_decoration(chunk: &mut Chunk, blocks: &Palette, local_x: i32, y: i32, local_z: i32, state: i32) {
    if y < SEA {
        put_in_water(chunk, blocks, local_x, y, local_z, state);
    }
}

// ---------------------------------------------------------------------------
// Морские огурцы
// ---------------------------------------------------------------------------

/// Доля чанков тёплого океана, где лежит кучка морских огурцов (китайская
/// вики, «海泡菜»: 1/16).
const PICKLE_CHANCE: f64 = 1.0 / 16.0;

/// Сколько мест пробуется в кучке.
const PICKLE_TRIES: i32 = 20;

/// Кучка морских огурцов на дне тёплого океана: по 1–4 в месте, на песке,
/// гравии и коралловых блоках. Кучка целиком в чанке.
fn sea_pickles(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    if !roll(area.key, area.chunk_x, area.chunk_z, SALT_PICKLE, PICKLE_CHANCE) {
        return;
    }

    let mut dice = Dice::new(area.key, area.chunk_x, area.chunk_z, SALT_PICKLE + 1);
    let (middle_x, middle_z) = (dice.between(4, CHUNK_SIZE - 5), dice.between(4, CHUNK_SIZE - 5));

    if area.near(middle_x, middle_z).biome != Biome::WarmOcean {
        return;
    }

    for _ in 0..PICKLE_TRIES {
        let local_x = middle_x + dice.between(-4, 4);
        let local_z = middle_z + dice.between(-4, 4);
        let count = dice.below(4) as usize;

        // Дно — первый не водный блок под поверхностью.
        let mut y = SEA;

        while block_in(chunk, local_x, y, local_z).is_some_and(|here| blocks.watery(here)) {
            y -= 1;
        }

        let Some(floor) = block_in(chunk, local_x, y, local_z) else {
            continue;
        };

        if floor == blocks.sand || floor == blocks.gravel || blocks.coral_block(floor) {
            put_decoration(chunk, blocks, local_x, y + 1, local_z, blocks.pickles[count]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все записи блоков есть в таблице.
    #[test]
    fn palette_is_complete() {
        let blocks = palette();

        assert_ne!(blocks.coral_blocks[0], blocks.coral_blocks[4]);
        assert_ne!(blocks.wall_fans[2][0], blocks.wall_fans[2][3]);
        assert_ne!(blocks.pickles[0], blocks.pickles[3]);
        assert!(blocks.rock(state("stone")));
        assert!(!blocks.rock(blocks.water));
    }

    /// Настенные веера рифа держатся за блок и на стыке чанков: соседние
    /// чанки ставят части общего рифа одинаково, и веер у края одного чанка
    /// не смотрит в пустую воду соседнего.
    #[test]
    fn reefs_meet_at_chunk_borders() {
        use crate::world::terrain::Style;

        // Тёплый океан у этого семени — найден переписью.
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, Style::Vanilla));
        let (middle_x, middle_z) = (-1936, 256);
        let chunks: Vec<Vec<Chunk>> = (-1..=1)
            .map(|dx| (-1..=1).map(|dz| Chunk::generated(&generator, middle_x + dx, middle_z + dz)).collect())
            .collect();
        let at = |x: i32, y: i32, z: i32| {
            let (chunk_x, chunk_z) = (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));

            chunks[(chunk_x + 1) as usize][(chunk_z + 1) as usize].block(
                x.rem_euclid(CHUNK_SIZE),
                y,
                z.rem_euclid(CHUNK_SIZE),
            )
        };
        let blocks = palette();
        let (mut fans, mut corals) = (0, 0);

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for y in MIN_Y..SEA {
                    let here = at(x, y, z);

                    if blocks.coral_block(here) {
                        corals += 1;
                    }

                    for (side, (dx, dz, _)) in SIDES.iter().enumerate() {
                        if blocks.wall_fans.iter().any(|kind| kind[side] == here) {
                            fans += 1;

                            let support = at(x - dx, y, z - dz);

                            assert!(
                                support != blocks.water && support != AIR,
                                "веер в {:?} ни за что не держится",
                                (x, y, z)
                            );
                        }
                    }
                }
            }
        }

        assert!(corals > 0 && fans > 0, "в тёплом океане нет рифов: {} блоков, {} вееров", corals, fans);
    }

    /// Риф решается местом: одна и та же точка даёт одни и те же блоки, и
    /// риф не выходит из воды и не уходит от точки дальше, чем положено.
    #[test]
    fn reefs_are_repeatable_and_stay_near() {
        for i in 0..400 {
            let (x, z) = (i * 7 - 1400, i * 13 - 2600);
            let (mut one, mut two) = (Vec::new(), Vec::new());

            reef_blocks(99, x, 40, z, &mut one);
            reef_blocks(99, x, 40, z, &mut two);

            assert_eq!(one, two);

            for &(bx, by, bz) in &one {
                assert!((bx - x).abs() <= REEF_REACH && (bz - z).abs() <= REEF_REACH);
                assert!((40..SEA).contains(&by));
            }
        }
    }
}
