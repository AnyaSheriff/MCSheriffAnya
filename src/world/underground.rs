// Подземелье: то, что ложится в чанк после руды.
//
// Порядок прохода:
//   1. переход камня в глубинный сланец — постепенный, в Y 0…8;
//   2. большие рудные жилы — медная и железная;
//   3. жеоды аметиста;
//   4. данжи.
//
// Всё решается числом от координат и от мира, а не от того, в каком порядке
// складывались чанки. Жила — это просто шум: каждый блок знает, жила он или
// нет, и граница чанка ей не видна. Жеода больше чанка, поэтому каждый чанк
// перебирает жеоды своих соседей и рисует из них то, что попало к нему, —
// как деревья в `grow_trees` и гнёзда руды в `ore_blocks_in`. Данж же
// целиком ставится внутри своего чанка: ему надо видеть пещеру, в которую он
// выходит, а соседний чанк чужую пещеру не видит.
//
// Числа — с вики (Ore vein, Amethyst Geode, Monster Room, Deepslate); сама
// форма жил и жеод своя.

use super::noise::{Noise, Random};
use super::terrain::{self, Terrain};
use super::{Chunk, Generator, AIR, CHUNK_SIZE, MIN_Y, WORLD_HEIGHT};

/// Дорабатывает подземелье чанка после руды.
pub(super) fn dig(generator: &Generator, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
    let Generator::Normal(terrain) = generator else {
        return;
    };

    let key = world_key(terrain);
    let palette = Palette::new();

    blend_deepslate(chunk, chunk_x, chunk_z, key, &palette);
    lay_veins(chunk, chunk_x, chunk_z, key, &palette);

    for geode in geodes_near(terrain, key, chunk_x, chunk_z) {
        carve_geode(chunk, chunk_x, chunk_z, &geode, &palette);
    }

    build_dungeons(chunk, chunk_x, chunk_z, key, &palette);
}

/// Число мира, от которого считается всё подземелье: семя, перемешанное
/// со своей солью, чтобы подземелье не повторяло другие решения по семени.
fn world_key(terrain: &Terrain) -> u64 {
    scramble(terrain.seed() as u64 ^ 0x005E_ED0F_D1C0)
}

/// Перемешивание числа: из близких входов — далёкие выходы.
pub(super) fn scramble(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// Своё число для точки объёма и для вопроса `salt`.
pub(super) fn hash(key: u64, x: i32, y: i32, z: i32, salt: u64) -> u64 {
    let mut value = scramble(key ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93));

    value = scramble(value ^ x as u32 as u64);
    value = scramble(value ^ y as u32 as u64);
    scramble(value ^ z as u32 as u64)
}

/// Число от 0 до 1 из перемешанного.
pub(super) fn unit(value: u64) -> f64 {
    (value >> 11) as f64 / (1u64 << 53) as f64
}

/// Число от 0 до 1 из генератора.
fn next_unit(random: &mut Random) -> f64 {
    unit(scramble(random.next()))
}

/// Целое от `low` до `high` включительно.
fn next_in(random: &mut Random, low: i32, high: i32) -> i32 {
    low + (scramble(random.next()) % (high - low + 1) as u64) as i32
}

/// Вопросы, которые задаются числу от точки: у каждого свой.
const SALT_DEEPSLATE: u64 = 1;
const SALT_VEIN_SKIP: u64 = 2;
const SALT_VEIN_ORE: u64 = 3;
const SALT_VEIN_RAW: u64 = 4;
const SALT_GEODE: u64 = 5;
const SALT_BUDDING: u64 = 6;
const SALT_BUD: u64 = 7;
const SALT_BUD_SIZE: u64 = 8;
const SALT_DUNGEON: u64 = 9;
const SALT_MOSS: u64 = 10;

/// Семена шумов жил — от числа мира.
const SALT_NOISE_TOGGLE: u64 = 101;
const SALT_NOISE_RIDGE_A: u64 = 102;
const SALT_NOISE_RIDGE_B: u64 = 103;
const SALT_NOISE_GAP: u64 = 104;
const SALT_NOISE_GEODE: u64 = 105;

/// Состояния блоков, нужные проходу: ищутся по имени один раз на чанк.
struct Palette {
    stone: i32,
    deepslate: i32,
    bedrock: i32,
    water: i32,
    lava: i32,
    /// Руда в камне и та же руда в глубинном сланце.
    deep_ores: Vec<(i32, i32)>,
    granite: i32,
    tuff: i32,
    copper_ore: i32,
    deep_copper_ore: i32,
    raw_copper: i32,
    iron_ore: i32,
    deep_iron_ore: i32,
    raw_iron: i32,
    smooth_basalt: i32,
    calcite: i32,
    amethyst: i32,
    budding_amethyst: i32,
    cobblestone: i32,
    mossy_cobblestone: i32,
    spawner: i32,
}

impl Palette {
    fn new() -> Palette {
        let named = terrain::block_named;
        let deep_ores = ["coal", "iron", "copper", "gold", "redstone", "emerald", "lapis", "diamond"]
            .iter()
            .map(|ore| (named(&format!("{}_ore", ore)), named(&format!("deepslate_{}_ore", ore))))
            .collect();

        Palette {
            stone: named("stone"),
            deepslate: named("deepslate"),
            bedrock: named("bedrock"),
            water: named("water"),
            lava: named("lava"),
            deep_ores,
            granite: named("granite"),
            tuff: named("tuff"),
            copper_ore: named("copper_ore"),
            deep_copper_ore: named("deepslate_copper_ore"),
            raw_copper: named("raw_copper_block"),
            iron_ore: named("iron_ore"),
            deep_iron_ore: named("deepslate_iron_ore"),
            raw_iron: named("raw_iron_block"),
            smooth_basalt: named("smooth_basalt"),
            calcite: named("calcite"),
            amethyst: named("amethyst_block"),
            budding_amethyst: named("budding_amethyst"),
            cobblestone: named("cobblestone"),
            mossy_cobblestone: named("mossy_cobblestone"),
            spawner: named("spawner"),
        }
    }

    /// Жидкость: вода или лава.
    fn liquid(&self, state: i32) -> bool {
        state == self.water || state == self.lava
    }

    /// Твёрдый ли блок для данжа: не воздух и не жидкость.
    fn solid(&self, state: i32) -> bool {
        state != AIR && !self.liquid(state)
    }
}

/// Верх мира — первая высота, где блоков уже нет.
const TOP: i32 = MIN_Y + WORLD_HEIGHT;

// ---------------------------------------------------------------------------
// Камень → глубинный сланец
// ---------------------------------------------------------------------------

/// Выше этой высоты глубинного сланца нет, ниже нуля — только он (вики,
/// «Deepslate»: переход между Y 8 и 0).
const DEEPSLATE_BLEND_TOP: i32 = 8;

/// Размывает границу камня и глубинного сланца: в Y 0…8 камень сменяется
/// сланцем с долей, растущей книзу от нуля до целого. Руда в таком месте
/// становится своей глубинной разновидностью.
fn blend_deepslate(chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, key: u64, palette: &Palette) {
    for y in 0..DEEPSLATE_BLEND_TOP {
        let part = (DEEPSLATE_BLEND_TOP - y) as f64 / DEEPSLATE_BLEND_TOP as f64;

        for local_x in 0..CHUNK_SIZE {
            for local_z in 0..CHUNK_SIZE {
                let here = chunk.block(local_x, y, local_z);

                let deep = if here == palette.stone {
                    palette.deepslate
                } else if let Some((_, deep)) = palette.deep_ores.iter().find(|(ore, _)| *ore == here) {
                    *deep
                } else {
                    continue;
                };

                let (x, z) = (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);

                if unit(hash(key, x, y, z, SALT_DEEPSLATE)) < part {
                    chunk.put_generated(local_x, y, local_z, deep, true);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Большие рудные жилы
// ---------------------------------------------------------------------------

/// Какая жила где лежит (вики, «Ore vein»): медная в Y 0…50, гуще всего
/// в 20…30; железная в Y −60…−8, гуще всего в −40…−28. К краям промежутка
/// жилы сходят на нет.
const COPPER_LOW: i32 = 0;
const COPPER_HIGH: i32 = 50;
const IRON_LOW: i32 = -60;
const IRON_HIGH: i32 = -8;

/// На каком расстоянии от края промежутка жила набирает полную силу: от
/// края до «гуще всего» у обеих жил двадцать блоков.
const VEIN_TAPER: f64 = 20.0;

/// Шаг сетки, в узлах которой считаются шумы жил; между узлами значения
/// тянутся по прямой. Шум жил плавный, и так его в десятки раз дешевле
/// считать. Узлы стоят на кратных четырём координатах мира, поэтому у
/// соседних чанков общая грань сетки и жила на стыке не рвётся.
const VEIN_CELL: i32 = 4;

/// Нижний и верхний узлы сетки по высоте: охватывают обе жилы.
const VEIN_GRID_LOW: i32 = -64;
const VEIN_GRID_LAYERS: usize = 30;

/// Узлов сетки вдоль стороны чанка.
const VEIN_GRID_SIDE: usize = (CHUNK_SIZE / VEIN_CELL) as usize + 1;

/// Масштабы шумов, в блоках на единицу шума.
const TOGGLE_SCALE: f64 = 80.0;
const RIDGE_SCALE: f64 = 24.0;
const GAP_SCALE: f64 = 12.0;

/// Насколько шум-переключатель должен отойти от нуля, чтобы тут вообще
/// была жила: между медью и железом остаётся пустая полоса.
const TOGGLE_MIN: f64 = 0.12;

/// Толщина ленты жилы: оба шума-гребня ближе к нулю, чем это.
const RIDGE_WIDTH: f64 = 0.09;

/// Доли блоков внутри жилы (вики): 70% не трогаются; из остальных
/// 10…30% руда (тем больше, чем сильнее жила), прочее — наполнитель;
/// из руды 2% — блок сырого металла.
const VEIN_UNTOUCHED: f64 = 0.7;
const VEIN_ORE_LEAST: f64 = 0.1;
const VEIN_ORE_MOST: f64 = 0.3;
const VEIN_RAW: f64 = 0.02;

/// Там, где шум «прорех» ниже этого, в жиле лежит только наполнитель.
const GAP_THRESHOLD: f64 = -0.3;

/// Что в этом месте по шумам жил.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Vein {
    /// Медная: гранит, медная руда, иногда блок сырой меди.
    Copper,
    /// Железная: туф, железная руда, иногда блок сырого железа.
    Iron,
}

/// Во что превращается блок жилы.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VeinBlock {
    Filler,
    Ore,
    Raw,
}

/// Значения шумов жил в узлах сетки чанка.
struct VeinField {
    /// По узлу — переключатель, два гребня и прорехи.
    nodes: Vec<[f64; 4]>,
}

impl VeinField {
    /// Считает шумы в узлах сетки над этим чанком.
    fn new(key: u64, chunk_x: i32, chunk_z: i32) -> VeinField {
        let noise = |salt: u64| Noise::new(scramble(key ^ salt) as i64);
        let (toggle, ridge_a, ridge_b, gap) = (
            noise(SALT_NOISE_TOGGLE),
            noise(SALT_NOISE_RIDGE_A),
            noise(SALT_NOISE_RIDGE_B),
            noise(SALT_NOISE_GAP),
        );
        let mut nodes = Vec::with_capacity(VEIN_GRID_SIDE * VEIN_GRID_SIDE * VEIN_GRID_LAYERS);

        for layer in 0..VEIN_GRID_LAYERS {
            let y = (VEIN_GRID_LOW + layer as i32 * VEIN_CELL) as f64;

            for node_z in 0..VEIN_GRID_SIDE {
                let z = (chunk_z * CHUNK_SIZE + node_z as i32 * VEIN_CELL) as f64;

                for node_x in 0..VEIN_GRID_SIDE {
                    let x = (chunk_x * CHUNK_SIZE + node_x as i32 * VEIN_CELL) as f64;

                    nodes.push([
                        toggle.at3(x / TOGGLE_SCALE, y / TOGGLE_SCALE, z / TOGGLE_SCALE),
                        ridge_a.at3(x / RIDGE_SCALE, y / RIDGE_SCALE, z / RIDGE_SCALE),
                        ridge_b.at3(x / RIDGE_SCALE, y / RIDGE_SCALE, z / RIDGE_SCALE),
                        gap.at3(x / GAP_SCALE, y / GAP_SCALE, z / GAP_SCALE),
                    ]);
                }
            }
        }

        VeinField { nodes }
    }

    fn node(&self, node_x: usize, layer: usize, node_z: usize) -> [f64; 4] {
        self.nodes[(layer * VEIN_GRID_SIDE + node_z) * VEIN_GRID_SIDE + node_x]
    }

    /// Шумы в точке чанка: по прямой между узлами сетки.
    fn at(&self, local_x: i32, y: i32, local_z: i32) -> [f64; 4] {
        let from_bottom = y - VEIN_GRID_LOW;
        let (node_x, node_z, layer) = (
            (local_x / VEIN_CELL) as usize,
            (local_z / VEIN_CELL) as usize,
            (from_bottom / VEIN_CELL) as usize,
        );
        let step = VEIN_CELL as f64;
        let (tx, tz, ty) = (
            (local_x % VEIN_CELL) as f64 / step,
            (local_z % VEIN_CELL) as f64 / step,
            (from_bottom % VEIN_CELL) as f64 / step,
        );

        let mut out = [0.0; 4];

        for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
            for (dz, wz) in [(0, 1.0 - tz), (1, tz)] {
                for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
                    let weight = wx * wy * wz;

                    if weight == 0.0 {
                        continue;
                    }

                    let values = self.node(node_x + dx, layer + dy, node_z + dz);

                    for (slot, value) in out.iter_mut().zip(values) {
                        *slot += weight * value;
                    }
                }
            }
        }

        out
    }

    /// Жила в этом месте и её сила от 0 до 1 — или никакой.
    fn vein_at(&self, local_x: i32, y: i32, local_z: i32) -> Option<(Vein, f64)> {
        if !(IRON_LOW..=COPPER_HIGH).contains(&y) {
            return None;
        }

        let [toggle, ridge_a, ridge_b, _] = self.at(local_x, y, local_z);

        let (vein, low, high) = if toggle > 0.0 {
            (Vein::Copper, COPPER_LOW, COPPER_HIGH)
        } else {
            (Vein::Iron, IRON_LOW, IRON_HIGH)
        };

        if !(low..=high).contains(&y) {
            return None;
        }

        // У краёв промежутка жила слабеет: переключателю приходится
        // отходить от нуля дальше, а лента становится тоньше.
        let edge = ((y - low).min(high - y) as f64 / VEIN_TAPER).clamp(0.0, 1.0);
        let strength = (toggle.abs() - TOGGLE_MIN - 0.2 * (1.0 - edge)) / 0.25;

        if strength <= 0.0 {
            return None;
        }

        let width = RIDGE_WIDTH * (0.5 + 0.5 * edge);

        if ridge_a.abs().max(ridge_b.abs()) >= width {
            return None;
        }

        Some((vein, strength.min(1.0)))
    }
}

/// Во что превращается блок жилы в этой точке — или он остаётся как был.
fn vein_block(key: u64, x: i32, y: i32, z: i32, strength: f64, gap: f64) -> Option<VeinBlock> {
    if unit(hash(key, x, y, z, SALT_VEIN_SKIP)) < VEIN_UNTOUCHED {
        return None;
    }

    let ore = VEIN_ORE_LEAST + (VEIN_ORE_MOST - VEIN_ORE_LEAST) * strength;

    if gap > GAP_THRESHOLD && unit(hash(key, x, y, z, SALT_VEIN_ORE)) < ore {
        if unit(hash(key, x, y, z, SALT_VEIN_RAW)) < VEIN_RAW {
            return Some(VeinBlock::Raw);
        }

        return Some(VeinBlock::Ore);
    }

    Some(VeinBlock::Filler)
}

/// Прокладывает жилы по чанку. Жила идёт только по камню и глубинному
/// сланцу: пещеры её перебивают, гнёзда руды остаются.
fn lay_veins(chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, key: u64, palette: &Palette) {
    let field = VeinField::new(key, chunk_x, chunk_z);

    for y in IRON_LOW..=COPPER_HIGH {
        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let here = chunk.block(local_x, y, local_z);

                if here != palette.stone && here != palette.deepslate {
                    continue;
                }

                let Some((vein, strength)) = field.vein_at(local_x, y, local_z) else {
                    continue;
                };

                let (x, z) = (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);
                let gap = field.at(local_x, y, local_z)[3];

                let Some(kind) = vein_block(key, x, y, z, strength, gap) else {
                    continue;
                };

                let deep = here == palette.deepslate;
                let state = match (vein, kind) {
                    (Vein::Copper, VeinBlock::Filler) => palette.granite,
                    (Vein::Copper, VeinBlock::Ore) if deep => palette.deep_copper_ore,
                    (Vein::Copper, VeinBlock::Ore) => palette.copper_ore,
                    (Vein::Copper, VeinBlock::Raw) => palette.raw_copper,
                    (Vein::Iron, VeinBlock::Filler) => palette.tuff,
                    (Vein::Iron, VeinBlock::Ore) if deep => palette.deep_iron_ore,
                    (Vein::Iron, VeinBlock::Ore) => palette.iron_ore,
                    (Vein::Iron, VeinBlock::Raw) => palette.raw_iron,
                };

                chunk.put_generated(local_x, y, local_z, state, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Жеоды аметиста
// ---------------------------------------------------------------------------

/// Жеода на чанк выпадает с шансом 1/24, центр — в Y −58…30 (вики).
const GEODE_CHANCE: f64 = 1.0 / 24.0;
const GEODE_LOW: i32 = -58;
const GEODE_HIGH: i32 = 30;

/// Границы слоёв — по «расстоянию» от центральных точек жеоды: внутри
/// пусто, дальше аметист, кальцит и гладкий базальт. Внутренняя граница —
/// 1.7, как у вики; вики задаёт толщины в своей мере, у нас мера — блоки,
/// и тоньше блока оболочка выходит дырявой, поэтому каждый слой здесь
/// ровно в блок толщиной.
const GEODE_AIR: f64 = 1.7;
const GEODE_AMETHYST: f64 = 2.7;
const GEODE_CALCITE: f64 = 3.7;
const GEODE_BASALT: f64 = 4.7;

/// Центральных точек 3 или 4 (вики); каждая отходит от центра на столько
/// блоков по каждой оси, не дальше.
const GEODE_POINTS_LOW: i32 = 3;
const GEODE_POINTS_HIGH: i32 = 4;
const GEODE_POINT_SPREAD: i32 = 2;

/// Насколько шум сдвигает границы слоёв, в блоках.
const GEODE_WOBBLE: f64 = 0.6;

/// С трещиной — 95% жеод (вики); ширина трещины от оси, в блоках.
const GEODE_CRACK_CHANCE: f64 = 0.95;
const GEODE_CRACK_HALF_WIDTH: f64 = 1.0;

/// 8.3% аметиста — растущий аметист; у такого блока почка сидит на
/// свободной стороне с шансом 0.35 (вики).
const GEODE_BUDDING: f64 = 0.083;
const GEODE_BUD: f64 = 0.35;

/// Дальше этого от центра жеода не заходит ни одним блоком.
const GEODE_REACH: i32 = GEODE_BASALT as i32 + GEODE_POINT_SPREAD + 2;

/// Над жеодой столько блоков толщи до поверхности: иначе она вскрыла бы
/// дно моря или склон.
const GEODE_COVER: i32 = 10;

/// Слой жеоды.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GeodeLayer {
    Hollow,
    Amethyst,
    Budding,
    Calcite,
    Basalt,
}

/// Одна жеода: всё, что нужно, чтобы любой чанк нарисовал свою её часть.
#[derive(Clone, Debug, PartialEq)]
struct Geode {
    center: (i32, i32, i32),
    points: Vec<(f64, f64, f64)>,
    /// Направление трещины по горизонтали, если она есть.
    crack: Option<(f64, f64)>,
    /// Семя, от которого считаются почки и растущий аметист.
    key: u64,
}

/// Жеоды, которые могут дотянуться до этого чанка: свои и соседей.
fn geodes_near(terrain: &Terrain, key: u64, chunk_x: i32, chunk_z: i32) -> Vec<Geode> {
    let mut found = Vec::new();

    for from_x in (chunk_x - 1)..=(chunk_x + 1) {
        for from_z in (chunk_z - 1)..=(chunk_z + 1) {
            let Some(geode) = geode_of(terrain, key, from_x, from_z) else {
                continue;
            };

            let (cx, _, cz) = geode.center;
            let near_x = cx.clamp(chunk_x * CHUNK_SIZE, chunk_x * CHUNK_SIZE + CHUNK_SIZE - 1);
            let near_z = cz.clamp(chunk_z * CHUNK_SIZE, chunk_z * CHUNK_SIZE + CHUNK_SIZE - 1);

            if (cx - near_x).abs() <= GEODE_REACH && (cz - near_z).abs() <= GEODE_REACH {
                found.push(geode);
            }
        }
    }

    found
}

/// Жеода, начатая в этом чанке, — если выпала.
fn geode_of(terrain: &Terrain, key: u64, chunk_x: i32, chunk_z: i32) -> Option<Geode> {
    let seed = hash(key, chunk_x, 0, chunk_z, SALT_GEODE);
    let mut random = Random::new(seed as i64);

    if next_unit(&mut random) >= GEODE_CHANCE {
        return None;
    }

    let x = chunk_x * CHUNK_SIZE + next_in(&mut random, 0, CHUNK_SIZE - 1);
    let z = chunk_z * CHUNK_SIZE + next_in(&mut random, 0, CHUNK_SIZE - 1);
    let y = next_in(&mut random, GEODE_LOW, GEODE_HIGH);

    let count = next_in(&mut random, GEODE_POINTS_LOW, GEODE_POINTS_HIGH);
    let points = (0..count)
        .map(|_| {
            let mut offset = || next_in(&mut random, -GEODE_POINT_SPREAD, GEODE_POINT_SPREAD) as f64;

            (x as f64 + offset(), y as f64 + offset(), z as f64 + offset())
        })
        .collect();

    let crack = (next_unit(&mut random) < GEODE_CRACK_CHANCE).then(|| {
        let angle = next_unit(&mut random) * std::f64::consts::TAU;

        (angle.cos(), angle.sin())
    });

    // Жеода не должна вскрывать поверхность: над ней нужна толща земли.
    // Столбец считается только у выпавших жеод — их одна на два десятка
    // чанков.
    if y + GEODE_COVER > terrain.column_at(x, z).height {
        return None;
    }

    Some(Geode {
        center: (x, y, z),
        points,
        crack,
        key: scramble(seed),
    })
}

impl Geode {
    /// «Расстояние» от точки до жеоды: гармоническое среднее расстояний до
    /// её центральных точек. Около одной точки оно мало, между точками
    /// плавно растёт — получается неровный ком, а не шар.
    fn distance(&self, x: i32, y: i32, z: i32, wobble: &Noise) -> f64 {
        let (fx, fy, fz) = (x as f64, y as f64, z as f64);
        let mut inverse = 0.0;

        for (px, py, pz) in &self.points {
            let d = ((fx - px).powi(2) + (fy - py).powi(2) + (fz - pz).powi(2)).sqrt().max(0.1);

            inverse += 1.0 / d;
        }

        self.points.len() as f64 / inverse + GEODE_WOBBLE * wobble.at3(fx / 3.0, fy / 3.0, fz / 3.0)
    }

    /// Лежит ли точка в трещине: у луча от центра в сторону трещины.
    fn in_crack(&self, x: i32, y: i32, z: i32) -> bool {
        let Some((dir_x, dir_z)) = self.crack else {
            return false;
        };

        let (cx, cy, cz) = self.center;
        let (dx, dy, dz) = ((x - cx) as f64, (y - cy) as f64, (z - cz) as f64);
        let along = dx * dir_x + dz * dir_z;

        if along <= 0.0 {
            return false;
        }

        // Трещина — вертикальная щель: поперёк неё узко, по высоте выше.
        let across = (dx * dir_z - dz * dir_x).abs();

        across <= GEODE_CRACK_HALF_WIDTH && dy.abs() <= GEODE_CRACK_HALF_WIDTH * 2.0
    }

    /// Слой жеоды в точке, или None — точка вне жеоды.
    fn layer(&self, x: i32, y: i32, z: i32, wobble: &Noise) -> Option<GeodeLayer> {
        let d = self.distance(x, y, z, wobble);

        if d >= GEODE_BASALT {
            return None;
        }

        if d < GEODE_AIR || self.in_crack(x, y, z) {
            return Some(GeodeLayer::Hollow);
        }

        Some(if d < GEODE_AMETHYST {
            if unit(hash(self.key, x, y, z, SALT_BUDDING)) < GEODE_BUDDING {
                GeodeLayer::Budding
            } else {
                GeodeLayer::Amethyst
            }
        } else if d < GEODE_CALCITE {
            GeodeLayer::Calcite
        } else {
            GeodeLayer::Basalt
        })
    }

    /// Почка аметиста в пустой точке жеоды: какая и куда смотрит. Сидит она
    /// на соседнем растущем аметисте — первом по порядку сторон, где выпал
    /// шанс.
    fn bud(&self, x: i32, y: i32, z: i32, wobble: &Noise) -> Option<(&'static str, &'static str)> {
        // Сторона, где растущий аметист, и куда смотрит почка — от него.
        const SIDES: [((i32, i32, i32), &str); 6] = [
            ((0, -1, 0), "up"),
            ((0, 1, 0), "down"),
            ((1, 0, 0), "west"),
            ((-1, 0, 0), "east"),
            ((0, 0, 1), "north"),
            ((0, 0, -1), "south"),
        ];
        const SIZES: [&str; 4] = ["small_amethyst_bud", "medium_amethyst_bud", "large_amethyst_bud", "amethyst_cluster"];

        for (index, ((dx, dy, dz), facing)) in SIDES.iter().enumerate() {
            if self.layer(x + dx, y + dy, z + dz, wobble) != Some(GeodeLayer::Budding) {
                continue;
            }

            if unit(hash(self.key, x, y, z, SALT_BUD + 16 * index as u64)) >= GEODE_BUD {
                continue;
            }

            let size = SIZES[(hash(self.key, x, y, z, SALT_BUD_SIZE) % SIZES.len() as u64) as usize];

            return Some((size, facing));
        }

        None
    }
}

/// Рисует часть жеоды, попавшую в чанк. Бедрок, вода и лава не трогаются:
/// жеода не пробивает дно мира и не выпускает жидкость.
fn carve_geode(chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, geode: &Geode, palette: &Palette) {
    let wobble = Noise::new(scramble(geode.key ^ SALT_NOISE_GEODE) as i64);
    let (cx, cy, cz) = geode.center;
    let (base_x, base_z) = (chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE);

    let x_range = (cx - GEODE_REACH).max(base_x)..=(cx + GEODE_REACH).min(base_x + CHUNK_SIZE - 1);
    let z_range = (cz - GEODE_REACH).max(base_z)..=(cz + GEODE_REACH).min(base_z + CHUNK_SIZE - 1);
    let y_range = (cy - GEODE_REACH).max(MIN_Y)..=(cy + GEODE_REACH).min(TOP - 1);

    for x in x_range {
        for z in z_range.clone() {
            for y in y_range.clone() {
                let Some(layer) = geode.layer(x, y, z, &wobble) else {
                    continue;
                };

                let (local_x, local_z) = (x - base_x, z - base_z);
                let here = chunk.block(local_x, y, local_z);

                if here == palette.bedrock || palette.liquid(here) {
                    continue;
                }

                let state = match layer {
                    GeodeLayer::Hollow => match geode.bud(x, y, z, &wobble) {
                        Some((size, facing)) => crate::blocks::state_from_text(&format!("{}[facing={}]", size, facing))
                            .unwrap_or_else(|| terrain::block_named(size)),
                        None => AIR,
                    },
                    GeodeLayer::Amethyst => palette.amethyst,
                    GeodeLayer::Budding => palette.budding_amethyst,
                    GeodeLayer::Calcite => palette.calcite,
                    GeodeLayer::Basalt => palette.smooth_basalt,
                };

                chunk.put_generated(local_x, y, local_z, state, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Данжи
// ---------------------------------------------------------------------------

/// Попыток на чанк (вики): десять в Y 0…320 и четыре в Y −58…−1.
const DUNGEON_TRIES_HIGH: u32 = 10;
const DUNGEON_TRIES_LOW: u32 = 4;
const DUNGEON_LOW: i32 = -58;

/// Сторона комнаты со стенами — 7 или 9, по каждой оси своя; высота со
/// стенами, полом и потолком — 6.
const DUNGEON_SIDES: [i32; 2] = [7, 9];
const DUNGEON_HEIGHT: i32 = 6;

/// Сколько выходов в пещеру у пола допускается (вики: 1–5).
const DUNGEON_OPENINGS: std::ops::RangeInclusive<u32> = 1..=5;

/// Доля мшистого булыжника в полу (вики: 75%).
const DUNGEON_MOSS: f64 = 0.75;

/// Сундуков два, у каждого три попытки встать (вики).
const DUNGEON_CHESTS: u32 = 2;
const DUNGEON_CHEST_TRIES: u32 = 3;

/// Где пробовать поставить данж: угол комнаты внутри чанка и её размеры.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DungeonSpot {
    local_x: i32,
    local_z: i32,
    /// Высота пола.
    y: i32,
    size_x: i32,
    size_z: i32,
}

/// Места всех четырнадцати попыток чанка. Комната целиком внутри своего
/// чанка: проверять, выходит ли она в пещеру, можно только по своим
/// блокам.
fn dungeon_spots(key: u64, chunk_x: i32, chunk_z: i32) -> Vec<(DungeonSpot, u64)> {
    let mut random = Random::new(hash(key, chunk_x, 0, chunk_z, SALT_DUNGEON) as i64);
    let mut spots = Vec::new();

    for attempt in 0..(DUNGEON_TRIES_HIGH + DUNGEON_TRIES_LOW) {
        let size_x = DUNGEON_SIDES[next_in(&mut random, 0, 1) as usize];
        let size_z = DUNGEON_SIDES[next_in(&mut random, 0, 1) as usize];
        let local_x = next_in(&mut random, 0, CHUNK_SIZE - size_x);
        let local_z = next_in(&mut random, 0, CHUNK_SIZE - size_z);
        let y = if attempt < DUNGEON_TRIES_HIGH {
            next_in(&mut random, 0, TOP - DUNGEON_HEIGHT)
        } else {
            next_in(&mut random, DUNGEON_LOW, -1)
        };
        let seed = random.next();

        spots.push((DungeonSpot { local_x, local_z, y, size_x, size_z }, seed));
    }

    spots
}

fn build_dungeons(chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, key: u64, palette: &Palette) {
    for (spot, seed) in dungeon_spots(key, chunk_x, chunk_z) {
        try_dungeon(chunk, spot, seed, palette);
    }
}

/// Сколько выходов в воздух у стен комнаты на уровне над полом — или None,
/// если пол или потолок где-то не сплошные.
fn dungeon_openings(chunk: &Chunk, spot: DungeonSpot, palette: &Palette) -> Option<u32> {
    let DungeonSpot { local_x, local_z, y, size_x, size_z } = spot;
    let ceiling = y + DUNGEON_HEIGHT - 1;
    let mut openings = 0;

    for x in local_x..local_x + size_x {
        for z in local_z..local_z + size_z {
            if !palette.solid(chunk.block(x, y, z)) || !palette.solid(chunk.block(x, ceiling, z)) {
                return None;
            }

            let wall = x == local_x || x == local_x + size_x - 1 || z == local_z || z == local_z + size_z - 1;

            // Выход — два блока воздуха друг над другом прямо над полом.
            if wall && chunk.block(x, y + 1, z) == AIR && chunk.block(x, y + 2, z) == AIR {
                openings += 1;
            }
        }
    }

    Some(openings)
}

/// Ставит данж, если место подходит. Возвращает, поставлен ли он.
fn try_dungeon(chunk: &mut Chunk, spot: DungeonSpot, seed: u64, palette: &Palette) -> bool {
    let DungeonSpot { local_x, local_z, y, size_x, size_z } = spot;

    if y < MIN_Y || y + DUNGEON_HEIGHT > TOP {
        return false;
    }

    match dungeon_openings(chunk, spot, palette) {
        Some(openings) if DUNGEON_OPENINGS.contains(&openings) => {}
        _ => return false,
    }

    let ceiling = y + DUNGEON_HEIGHT - 1;

    // Стены, пол и потолок — из булыжника там, где был твёрдый блок;
    // выходы в пещеру остаются открытыми. Внутри — пусто.
    for x in local_x..local_x + size_x {
        for z in local_z..local_z + size_z {
            let wall = x == local_x || x == local_x + size_x - 1 || z == local_z || z == local_z + size_z - 1;

            for level in y..=ceiling {
                let shell = wall || level == y || level == ceiling;
                let here = chunk.block(x, level, z);

                let state = if !shell {
                    AIR
                } else if !palette.solid(here) {
                    continue;
                } else if level == y && unit(hash(seed, x, level, z, SALT_MOSS)) < DUNGEON_MOSS {
                    palette.mossy_cobblestone
                } else {
                    palette.cobblestone
                };

                chunk.put_generated(x, level, z, state, true);
            }
        }
    }

    // Спавнер — посередине, на полу.
    chunk.put_generated(local_x + size_x / 2, y + 1, local_z + size_z / 2, palette.spawner, true);

    // Сундук встаёт в пустое место, у которого ровно одна твёрдая сторона
    // из четырёх, и смотрит от неё.
    let mut random = Random::new(seed as i64);

    for _ in 0..DUNGEON_CHESTS {
        for _ in 0..DUNGEON_CHEST_TRIES {
            let x = next_in(&mut random, local_x + 1, local_x + size_x - 2);
            let z = next_in(&mut random, local_z + 1, local_z + size_z - 2);

            if chunk.block(x, y + 1, z) != AIR {
                continue;
            }

            let sides = [((1, 0), "west"), ((-1, 0), "east"), ((0, 1), "north"), ((0, -1), "south")];
            let solid: Vec<&str> = sides
                .iter()
                .filter(|((dx, dz), _)| palette.solid(chunk.block(x + dx, y + 1, z + dz)))
                .map(|(_, facing)| *facing)
                .collect();

            if let [facing] = solid[..] {
                let chest = crate::blocks::state_from_text(&format!("chest[facing={}]", facing))
                    .unwrap_or_else(|| terrain::block_named("chest"));

                chunk.put_generated(x, y + 1, z, chest, true);
                break;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: i64 = 100_554_032_945_340;

    fn generator() -> Generator {
        Generator::Normal(Terrain::smooth(SEED))
    }

    fn key() -> u64 {
        match generator() {
            Generator::Normal(terrain) => world_key(&terrain),
            Generator::Flat => unreachable!(),
        }
    }

    fn name(state: i32) -> &'static str {
        crate::blocks::block_at_state(state).unwrap_or("?")
    }

    /// Одно семя — одно подземелье; другое семя — другое.
    #[test]
    fn the_same_seed_gives_the_same_underground() {
        let one = Chunk::generated(&generator(), 3, -2);
        let same = Chunk::generated(&generator(), 3, -2);

        assert_eq!(one.to_bytes(), same.to_bytes());

        let other = Generator::Normal(Terrain::smooth(SEED + 1));

        match (&generator(), &other) {
            (Generator::Normal(a), Generator::Normal(b)) => assert_ne!(world_key(a), world_key(b)),
            _ => unreachable!(),
        }
    }

    /// Жилы лежат только в своих промежутках высот, и обе встречаются.
    #[test]
    fn veins_keep_to_their_heights() {
        let key = key();
        let (mut copper, mut iron) = (0u32, 0u32);

        for chunk_x in -6..6 {
            for chunk_z in -6..6 {
                let field = VeinField::new(key, chunk_x, chunk_z);

                for y in MIN_Y..TOP.min(VEIN_GRID_LOW + (VEIN_GRID_LAYERS as i32 - 1) * VEIN_CELL) {
                    for x in 0..CHUNK_SIZE {
                        for z in 0..CHUNK_SIZE {
                            match field.vein_at(x, y, z) {
                                Some((Vein::Copper, _)) => {
                                    assert!((COPPER_LOW..=COPPER_HIGH).contains(&y), "медь на {}", y);
                                    copper += 1;
                                }
                                Some((Vein::Iron, _)) => {
                                    assert!((IRON_LOW..=IRON_HIGH).contains(&y), "железо на {}", y);
                                    iron += 1;
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
        }

        assert!(copper > 0 && iron > 0, "медь {}, железо {}", copper, iron);
    }

    /// Шумы жил на общей грани двух чанков одни и те же: жила не рвётся.
    #[test]
    fn veins_meet_across_the_border() {
        let key = key();
        let west = VeinField::new(key, 4, 7);
        let east = VeinField::new(key, 5, 7);

        for y in (VEIN_GRID_LOW..VEIN_GRID_LOW + (VEIN_GRID_LAYERS as i32 - 1) * VEIN_CELL).step_by(4) {
            for z in (0..CHUNK_SIZE).step_by(4) {
                let layer = ((y - VEIN_GRID_LOW) / VEIN_CELL) as usize;
                let node_z = (z / VEIN_CELL) as usize;

                assert_eq!(west.node(VEIN_GRID_SIDE - 1, layer, node_z), east.node(0, layer, node_z));
            }
        }
    }

    /// Ищет жеоду, которая заходит в соседний с востока чанк.
    fn geode_across_the_border(terrain: &Terrain, key: u64) -> (Geode, i32, i32) {
        for chunk_x in -40..40 {
            for chunk_z in -40..40 {
                if let Some(geode) = geode_of(terrain, key, chunk_x, chunk_z) {
                    let (x, _, _) = geode.center;

                    if x - (chunk_x * CHUNK_SIZE) >= CHUNK_SIZE - 4 {
                        return (geode, chunk_x, chunk_z);
                    }
                }
            }
        }

        panic!("жеоды у границы не нашлось");
    }

    /// Жеода у границы видна обоим чанкам одинаковой, и каждый рисует свою
    /// половину: на стыке слои сходятся.
    #[test]
    fn a_geode_is_drawn_by_both_chunks() {
        let generator = generator();
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let key = world_key(terrain);
        let (geode, chunk_x, chunk_z) = geode_across_the_border(terrain, key);

        assert!(geodes_near(terrain, key, chunk_x, chunk_z).contains(&geode));
        assert!(geodes_near(terrain, key, chunk_x + 1, chunk_z).contains(&geode));

        let west = Chunk::generated(&generator, chunk_x, chunk_z);
        let east = Chunk::generated(&generator, chunk_x + 1, chunk_z);
        let wobble = Noise::new(scramble(geode.key ^ SALT_NOISE_GEODE) as i64);
        let (_, cy, cz) = geode.center;
        let border = (chunk_x + 1) * CHUNK_SIZE;

        let geode_blocks = ["smooth_basalt", "calcite", "amethyst_block", "budding_amethyst"];
        let mut seen = [0u32; 2];

        for (side, chunk, x) in [(0, &west, border - 1), (1, &east, border)] {
            for y in (cy - GEODE_REACH).max(MIN_Y)..=(cy + GEODE_REACH) {
                for z in (cz - GEODE_REACH)..=(cz + GEODE_REACH) {
                    if z.div_euclid(CHUNK_SIZE) != chunk_z {
                        continue;
                    }

                    let state = chunk.block(x, y, z);

                    // Где по жеоде оболочка, там и стоит оболочка — если
                    // место не бедрок и не жидкость.
                    if geode.layer(x, y, z, &wobble) == Some(GeodeLayer::Calcite)
                        && !matches!(name(state), "bedrock" | "water" | "lava")
                    {
                        assert_eq!(name(state), "calcite", "в {} {} {}", x, y, z);
                    }

                    if geode_blocks.contains(&name(state)) {
                        seen[side] += 1;
                    }
                }
            }
        }

        assert!(seen[0] > 0 && seen[1] > 0, "по сторонам: {:?}", seen);
    }

    /// Жеоды лежат в своих высотах и не выходят за мир.
    #[test]
    fn geodes_stay_in_the_world() {
        let generator = generator();
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let key = world_key(terrain);
        let mut count = 0;

        for chunk_x in -30..30 {
            for chunk_z in -30..30 {
                if let Some(geode) = geode_of(terrain, key, chunk_x, chunk_z) {
                    let (_, y, _) = geode.center;

                    assert!((GEODE_LOW..=GEODE_HIGH).contains(&y));
                    count += 1;
                }
            }
        }

        // 3600 чанков по 1/24 — около 150, но часть отсеяна поверхностью.
        assert!(count > 30, "жеод {}", count);
    }

    /// Переход в глубинный сланец: на нуле камня нет, у восьми — почти
    /// один камень, посередине вперемешку; дно мира — бедрок.
    #[test]
    fn deepslate_comes_gradually() {
        let generator = generator();
        let stone = terrain::block_named("stone");
        let deepslate = terrain::block_named("deepslate");
        let bedrock = terrain::block_named("bedrock");
        let mut at = [(0u32, 0u32); 9];

        for i in 0..4 {
            let chunk = Chunk::generated(&generator, i * 5, -i * 3);

            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    assert_eq!(chunk.block(x, MIN_Y, z), bedrock);

                    for y in 0..=8 {
                        match chunk.block(x, y, z) {
                            s if s == stone => at[y as usize].0 += 1,
                            s if s == deepslate => at[y as usize].1 += 1,
                            _ => {}
                        }
                    }
                }
            }
        }

        assert_eq!(at[0].0, 0, "камень на нуле");
        assert_eq!(at[8].1, 0, "сланец на восьми");

        let (stone_mid, deep_mid) = at[4];
        let part = deep_mid as f64 / (stone_mid + deep_mid) as f64;

        assert!((0.35..0.65).contains(&part), "доля сланца на Y 4: {}", part);
    }

    /// Чанк из сплошного камня с ходом в два блока высотой на уровне `y`.
    fn stone_with_tunnel(y: i32, tunnel: bool) -> Chunk {
        let mut chunk = Chunk::new();
        let stone = terrain::block_named("stone");

        for level in (y - 3)..(y + 10) {
            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    chunk.put_generated(x, level, z, stone, true);
                }
            }
        }

        if tunnel {
            // Ход с запада, вдоль z = 5, до середины чанка.
            for x in 0..8 {
                chunk.put_generated(x, y + 1, 5, AIR, true);
                chunk.put_generated(x, y + 2, 5, AIR, true);
            }
        }

        chunk
    }

    /// Данж встаёт только там, где комната выходит в пещеру: в сплошном
    /// камне его нет, у хода — есть, со спавнером посередине, сундуками
    /// у стен и мшистым полом.
    #[test]
    fn a_dungeon_needs_a_cave() {
        let palette = Palette::new();
        let spot = DungeonSpot { local_x: 3, local_z: 2, y: 20, size_x: 7, size_z: 9 };

        let mut solid = stone_with_tunnel(20, false);
        assert!(!try_dungeon(&mut solid, spot, 7, &palette));
        assert_eq!(solid.block(6, 21, 6), terrain::block_named("stone"));

        let mut open = stone_with_tunnel(20, true);
        assert!(try_dungeon(&mut open, spot, 7, &palette));

        assert_eq!(name(open.block(6, 21, 6)), "spawner");
        assert_eq!(name(open.block(3, 21, 2)), "cobblestone");
        assert_eq!(open.block(3, 21, 5), AIR, "выход в ход остался открытым");
        assert_eq!(open.block(4, 22, 3), AIR, "внутри пусто");

        let mut chests = 0;
        let mut floor = [0u32; 2];

        for x in 3..10 {
            for z in 2..11 {
                if name(open.block(x, 21, z)) == "chest" {
                    chests += 1;
                }

                match name(open.block(x, 20, z)) {
                    "mossy_cobblestone" => floor[0] += 1,
                    "cobblestone" => floor[1] += 1,
                    other => panic!("в полу {}", other),
                }

                assert_eq!(name(open.block(x, 25, z)), "cobblestone", "потолок");
            }
        }

        assert!((1..=2).contains(&chests), "сундуков {}", chests);
        assert!(floor[0] > floor[1], "мха меньше, чем камня: {:?}", floor);
    }

    /// Большая пещера — больше пяти выходов — данжу не подходит.
    #[test]
    fn a_dungeon_is_not_built_into_a_hall() {
        let palette = Palette::new();
        let mut chunk = stone_with_tunnel(20, false);

        for x in 0..CHUNK_SIZE {
            for z in 0..4 {
                chunk.put_generated(x, 21, z, AIR, true);
                chunk.put_generated(x, 22, z, AIR, true);
            }
        }

        let spot = DungeonSpot { local_x: 3, local_z: 2, y: 20, size_x: 7, size_z: 7 };

        assert!(!try_dungeon(&mut chunk, spot, 7, &palette));
    }

    /// Все попытки данжей — внутри своего чанка и внутри мира.
    #[test]
    fn dungeon_spots_stay_inside() {
        let key = key();

        for chunk_x in -20..20 {
            for chunk_z in -20..20 {
                let spots = dungeon_spots(key, chunk_x, chunk_z);

                assert_eq!(spots.len(), 14);

                for (spot, _) in spots {
                    assert!(spot.local_x >= 0 && spot.local_x + spot.size_x <= CHUNK_SIZE);
                    assert!(spot.local_z >= 0 && spot.local_z + spot.size_z <= CHUNK_SIZE);
                    assert!(spot.y >= DUNGEON_LOW && spot.y + DUNGEON_HEIGHT <= TOP);
                    assert!(DUNGEON_SIDES.contains(&spot.size_x) && DUNGEON_SIDES.contains(&spot.size_z));
                }
            }
        }
    }
}

