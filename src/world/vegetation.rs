// Растительность и мелочь на поверхности: трава и папоротники, цветы по
// биомам, кусты, опавшие листья, сухая трава, тростник, кактусы, тыквы,
// кувшинки, морская трава и ламинария, валуны, упавшие брёвна, огромные
// грибы.
//
// Проход идёт по готовому чанку после деревьев. Всё решается числом от
// координат, поэтому соседние чанки сходятся без сговора. Крупное — валуны,
// брёвна, грибы — начинается в своей точке и может зайти к соседу: такие
// штуки ищутся в полосе вокруг чанка, и каждый чанк рисует свою часть, как
// деревья и гнёзда руды.
//
// Мелкие растения ставятся только в воздух, водные — только в воду; трава
// и папоротники ещё и вместо слоя снега — как у оригинала в заснеженной
// тайге.
// Крупные вдобавок затирают короткую траву, папоротник и слой снега: иначе
// в бревне и валуне зияли бы дыры там, где до них выросла трава.
//
// Где что растёт — по вики и tools/research/worldgen-trees.md; густота
// подобрана по замеру сохранённого мира оригинала: сколько чего на чанк
// в каждом биоме (tools/measure/plants_census.py против
// `cargo test --release plants_census -- --ignored --nocapture`).

use std::sync::OnceLock;

use super::terrain::{Biome, Column, Terrain, SEA};
use super::{Chunk, Generator, AIR, CHUNK_SIZE, MIN_Y, WORLD_HEIGHT};

/// Сторона квадрата столбцов, который чанк считает вместе с каймой.
const SIDE: i32 = CHUNK_SIZE + 2;

/// Насколько далеко от своей точки заходят валун и огромный гриб.
const ROUND_REACH: i32 = 3;

/// Насколько далеко от пня лежит конец упавшего бревна: зазор до двух
/// блоков и ствол до одиннадцати.
const FALLEN_REACH: i32 = 13;

/// Растит на чанке всё мелкое и крупное, что не деревья.
///
/// `columns` — столбцы чанка с каймой в блок, как их считает
/// `Chunk::generated`: по X, внутри — по Z.
pub(super) fn decorate(generator: &Generator, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, columns: &[Column]) {
    let Generator::Normal(terrain) = generator else {
        return;
    };

    // Семя мира подмешивается во все решения растительности: иначе трава,
    // цветы и брёвна стояли бы на одних и тех же местах в любом мире.
    let key = key_of(terrain);
    let area = Area { terrain, columns, chunk_x, chunk_z, key };
    let blocks = palette();

    // Крупное — первым: оно заметнее, и мелочь потом обходит его сама.
    round_features(&area, chunk, blocks);
    fallen_trees(&area, chunk, blocks);
    patches(&area, chunk, blocks);
    single_plants(&area, chunk, blocks);
}

/// Семя мира, перемешанное для растительности.
fn key_of(terrain: &Terrain) -> u64 {
    (terrain.seed() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x7E6E_7A7E
}

/// Чанк и его окрестность: откуда брать столбцы.
struct Area<'a> {
    terrain: &'a Terrain,
    columns: &'a [Column],
    chunk_x: i32,
    chunk_z: i32,
    /// Семя мира, перемешанное для растительности.
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

    /// Задевает ли прямоугольник (мировые координаты, включительно) чанк.
    fn touches(&self, (x_from, z_from): (i32, i32), (x_to, z_to): (i32, i32)) -> bool {
        let (x0, z0) = (self.chunk_x * CHUNK_SIZE, self.chunk_z * CHUNK_SIZE);

        x_to >= x0 && x_from < x0 + CHUNK_SIZE && z_to >= z0 && z_from < z0 + CHUNK_SIZE
    }
}

// ---------------------------------------------------------------------------
// Случайность от места
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

    /// Доля от 0 до 1.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Высота 2–4 или 1–3 для тростника и кактуса: по вики нижняя в 11
    /// случаях из 18, средняя — в 5, верхняя — в 2.
    fn stalk(&mut self, lowest: i32) -> i32 {
        match self.below(18) {
            0..=10 => lowest,
            11..=15 => lowest + 1,
            _ => lowest + 2,
        }
    }
}

// Соли: у каждой штуки своя, чтобы решения не совпадали.
const SALT_ROUND: u64 = 71_003;
const SALT_FALLEN: u64 = 71_011;
const SALT_CANE: u64 = 71_021;
const SALT_CACTUS: u64 = 71_027;
const SALT_COVER: u64 = 71_039;
const SALT_MEADOW: u64 = 71_059;
const SALT_FLOWER: u64 = 71_069;
const SALT_LILY_PAD: u64 = 71_077;
const SALT_SEAGRASS: u64 = 71_081;
const SALT_KELP: u64 = 71_087;
const SALT_KELP_ZONE: u64 = 71_089;

// ---------------------------------------------------------------------------
// Состояния блоков
// ---------------------------------------------------------------------------

/// Состояния всех блоков прохода — считаются один раз на запуск: разбирать
/// запись на каждый блок было бы дорого.
struct Palette {
    water: i32,
    ground: Ground,

    short_grass: i32,
    fern: i32,
    snow: i32,
    tall_grass: [i32; 2],
    large_fern: [i32; 2],
    sunflower: [i32; 2],
    tall_flowers: [[i32; 2]; 3],
    lily_of_the_valley: i32,
    /// Простые цветы в порядке `Flower`.
    flowers: [i32; FLOWERS.len()],
    pink_petals: [[i32; 4]; 4],
    wildflowers: [[i32; 4]; 4],
    /// Опавшие листья: по стороне (север, юг, запад, восток) и числу
    /// кучек от одной до четырёх.
    leaf_litter: [[i32; 4]; 4],
    bush: i32,
    firefly_bush: i32,
    short_dry_grass: i32,
    tall_dry_grass: i32,
    /// Листва всех пород — числа от и до: по ней видно крону над местом.
    leaves: Vec<(i32, i32)>,

    sugar_cane: i32,
    cactus: i32,
    cactus_flower: Option<i32>,
    dead_bush: i32,
    pumpkin: i32,
    melon: i32,
    berry_bush: i32,

    lily_pad: i32,
    seagrass: i32,
    tall_seagrass: [i32; 2],
    kelp_plant: i32,
    kelp: [i32; 26],

    mossy_cobblestone: i32,
    brown_mushroom: i32,
    red_mushroom: i32,
    /// Шляпка огромного гриба по маске открытых граней (см. `FACES`):
    /// коричневая и красная.
    caps: [[i32; 64]; 2],
    stem: i32,
    /// Лиана, прицепленная к блоку с севера, юга, запада, востока.
    vines: [i32; 4],
    /// Брёвна упавших деревьев вдоль X, Y, Z — по породе (`Wood`).
    logs: [[i32; 3]; 4],
}

/// Земля, на которой что-то растёт.
struct Ground {
    grass_block: i32,
    dirt: i32,
    podzol: i32,
    coarse_dirt: i32,
    sand: i32,
    red_sand: i32,
    terracotta: i32,
    gravel: i32,
    clay: i32,
    stone: i32,
    mud: i32,
    /// Трава под слоем снега.
    snowy_grass: i32,
}

impl Ground {
    /// Земляная почва: трава, земля, подзол.
    fn soil(&self, block: i32) -> bool {
        block == self.grass_block || block == self.dirt || block == self.podzol || block == self.coarse_dirt
    }

    /// Песок, на котором растёт кактус.
    fn sandy(&self, block: i32) -> bool {
        block == self.sand || block == self.red_sand
    }

    /// Дно, за которое держатся морская трава и ламинария.
    fn seabed(&self, block: i32) -> bool {
        self.sandy(block)
            || block == self.gravel
            || block == self.dirt
            || block == self.clay
            || block == self.stone
            || block == self.mud
            || block == self.grass_block
    }
}

/// Порядок граней в маске шляпки гриба: бит 0 — низ, дальше верх, север,
/// юг, запад, восток.
const FACES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

/// Смещения к соседу по тем же граням.
const FACE_STEPS: [(i32, i32, i32); 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];

fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();

    PALETTE.get_or_init(|| {
        let pair = |name: &str| [state(&format!("{}[half=lower]", name)), state(&format!("{}[half=upper]", name))];
        let flowery = |name: &str, amount_name: &str| {
            let mut states = [[0; 4]; 4];

            for (facing, row) in ["north", "south", "west", "east"].iter().zip(states.iter_mut()) {
                for (amount, place) in row.iter_mut().enumerate() {
                    *place = state(&format!("{}[facing={},{}={}]", name, facing, amount_name, amount + 1));
                }
            }

            states
        };
        let cap = |name: &str| {
            let mut states = [0; 64];

            for (mask, place) in states.iter_mut().enumerate() {
                let faces: Vec<String> = FACES
                    .iter()
                    .enumerate()
                    .map(|(bit, face)| format!("{}={}", face, mask >> bit & 1 == 1))
                    .collect();

                *place = state(&format!("{}[{}]", name, faces.join(",")));
            }

            states
        };
        let logs = |name: &str| ["x", "y", "z"].map(|axis| state(&format!("{}[axis={}]", name, axis)));

        Palette {
            water: state("water"),
            ground: Ground {
                grass_block: state("grass_block"),
                dirt: state("dirt"),
                podzol: state("podzol"),
                coarse_dirt: state("coarse_dirt"),
                sand: state("sand"),
                red_sand: state("red_sand"),
                terracotta: state("terracotta"),
                gravel: state("gravel"),
                clay: state("clay"),
                stone: state("stone"),
                mud: state("mud"),
                snowy_grass: state("grass_block[snowy=true]"),
            },

            short_grass: state("short_grass"),
            fern: state("fern"),
            snow: state("snow"),
            tall_grass: pair("tall_grass"),
            large_fern: pair("large_fern"),
            sunflower: pair("sunflower"),
            tall_flowers: [pair("lilac"), pair("rose_bush"), pair("peony")],
            lily_of_the_valley: state("lily_of_the_valley"),
            flowers: FLOWERS.map(|flower| state(flower.name())),
            pink_petals: flowery("pink_petals", "flower_amount"),
            wildflowers: flowery("wildflowers", "flower_amount"),
            leaf_litter: flowery("leaf_litter", "segment_amount"),
            bush: state("bush"),
            firefly_bush: state("firefly_bush"),
            short_dry_grass: state("short_dry_grass"),
            tall_dry_grass: state("tall_dry_grass"),
            leaves: [
                "oak_leaves",
                "spruce_leaves",
                "birch_leaves",
                "jungle_leaves",
                "acacia_leaves",
                "dark_oak_leaves",
                "mangrove_leaves",
                "cherry_leaves",
                "azalea_leaves",
                "flowering_azalea_leaves",
                "pale_oak_leaves",
            ]
            .iter()
            .filter_map(|name| crate::blocks::states_of(name))
            .collect(),

            sugar_cane: state("sugar_cane"),
            cactus: state("cactus"),
            cactus_flower: crate::blocks::state_by_name("cactus_flower"),
            dead_bush: state("dead_bush"),
            pumpkin: state("pumpkin"),
            melon: state("melon"),
            berry_bush: state("sweet_berry_bush[age=3]"),

            lily_pad: state("lily_pad"),
            seagrass: state("seagrass"),
            tall_seagrass: pair("tall_seagrass"),
            kelp_plant: state("kelp_plant"),
            kelp: std::array::from_fn(|age| state(&format!("kelp[age={}]", age))),

            mossy_cobblestone: state("mossy_cobblestone"),
            brown_mushroom: state("brown_mushroom"),
            red_mushroom: state("red_mushroom"),
            caps: [cap("brown_mushroom_block"), cap("red_mushroom_block")],
            stem: state("mushroom_stem[up=false,down=false]"),
            vines: [
                state("vine[north=true]"),
                state("vine[south=true]"),
                state("vine[west=true]"),
                state("vine[east=true]"),
            ],
            logs: [logs("oak_log"), logs("birch_log"), logs("spruce_log"), logs("jungle_log")],
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

/// Ставит мелкое растение, только если место свободно.
fn put_in_air(chunk: &mut Chunk, local_x: i32, y: i32, local_z: i32, state: i32) -> bool {
    if block_in(chunk, local_x, y, local_z) != Some(AIR) {
        return false;
    }

    chunk.put_generated(local_x, y, local_z, state, true);
    true
}

/// Ставит часть крупной штуки: в воздух или поверх травы и снега.
fn put_over_plants(chunk: &mut Chunk, blocks: &Palette, local_x: i32, y: i32, local_z: i32, state: i32) {
    let Some(here) = block_in(chunk, local_x, y, local_z) else {
        return;
    };

    if here == AIR || here == blocks.short_grass || here == blocks.fern || here == blocks.snow {
        chunk.put_generated(local_x, y, local_z, state, true);
    }
}

/// Ставит двухблочное растение, если свободны оба места.
fn put_pair(chunk: &mut Chunk, local_x: i32, y: i32, local_z: i32, pair: [i32; 2]) -> bool {
    if block_in(chunk, local_x, y + 1, local_z) != Some(AIR) || block_in(chunk, local_x, y, local_z) != Some(AIR) {
        return false;
    }

    chunk.put_generated(local_x, y, local_z, pair[0], true);
    chunk.put_generated(local_x, y + 1, local_z, pair[1], true);
    true
}

/// Суша ли это, на которой что-то может расти.
fn on_land(column: &Column) -> bool {
    column.height >= SEA && !column.biome.is_ocean()
}

// ---------------------------------------------------------------------------
// Валуны и огромные грибы
// ---------------------------------------------------------------------------

/// Что за круглая штука стоит в этом месте.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Round {
    Boulder,
    BrownMushroom,
    RedMushroom,
}

/// Самая большая густота круглых штук: дешёвая проверка перед столбцом.
const ROUND_DENSEST: f64 = 0.006;

/// Какая круглая штука стоит в этой точке, если стоит.
fn round_at(key: u64, column: &Column, x: i32, z: i32) -> Option<Round> {
    if !on_land(column) {
        return None;
    }

    let value = unit(key, x, z, SALT_ROUND);

    // Валуны из мшистого булыжника — в старовозрастной тайге; огромные
    // грибы — на грибных полях помногу, в тёмном лесу изредка и чуть чаще
    // красные (вики, «Huge Mushroom»).
    let (density, red_share, boulder) = match column.biome {
        Biome::OldGrowthPineTaiga | Biome::OldGrowthSpruceTaiga => (0.006, 0.0, true),
        Biome::MushroomFields => (0.006, 0.5, false),
        Biome::DarkForest => (0.002, 0.55, false),
        _ => return None,
    };

    if value >= density {
        return None;
    }

    if boulder {
        return Some(Round::Boulder);
    }

    Some(if unit(key, x, z, SALT_ROUND + 1) < red_share { Round::RedMushroom } else { Round::BrownMushroom })
}

/// Блок круглой штуки: смещение от земли под точкой и состояние.
type Placed = (i32, i32, i32, i32);

/// Все блоки круглой штуки относительно её точки.
fn round_blocks(key: u64, round: Round, blocks: &Palette, x: i32, z: i32) -> Vec<Placed> {
    let mut dice = Dice::new(key, x, z, SALT_ROUND + 2);

    match round {
        Round::Boulder => boulder(&mut dice, blocks),
        Round::BrownMushroom | Round::RedMushroom => huge_mushroom(&mut dice, blocks, round == Round::RedMushroom),
    }
}

/// Валун: три слипшихся шара мшистого булыжника, наполовину в земле.
fn boulder(dice: &mut Dice, blocks: &Palette) -> Vec<Placed> {
    let mut placed = Vec::new();

    for _ in 0..3 {
        let (cx, cy, cz) = (dice.below(3) - 1, dice.below(2), dice.below(3) - 1);
        let radius = 1.0 + dice.unit();
        let reach = (ROUND_REACH - 1).min(radius.ceil() as i32);

        for dx in -reach..=reach {
            for dy in -reach..=reach {
                for dz in -reach..=reach {
                    if ((dx * dx + dy * dy + dz * dz) as f64) > radius * radius {
                        continue;
                    }

                    let at = (cx + dx, cy + dy, cz + dz, blocks.mossy_cobblestone);

                    if !placed.contains(&at) {
                        placed.push(at);
                    }
                }
            }
        }
    }

    placed
}

/// Огромный гриб. Высота по вики — 5–7 блоков, в одном случае из
/// двенадцати вдвое выше без одного. Коричневый — ножка и плоская шляпка
/// 7×7 без углов; красный — ножка и купол из пяти пластин 3×3: одна сверху,
/// четыре по бокам.
fn huge_mushroom(dice: &mut Dice, blocks: &Palette, red: bool) -> Vec<Placed> {
    let mut height = 5 + dice.below(3);

    if dice.below(12) == 0 {
        height = height * 2 - 1;
    }

    // Сначала только где что стоит: грани шляпки зависят от соседей.
    let mut cap: Vec<(i32, i32, i32)> = Vec::new();

    if red {
        for a in -1..=1 {
            for b in -1..=1 {
                cap.push((a, height, b));
            }

            for y in (height - 3)..height {
                cap.extend([(2, y, a), (-2, y, a), (a, y, 2), (a, y, -2)]);
            }
        }
    } else {
        for dx in -3..=3_i32 {
            for dz in -3..=3_i32 {
                if dx.abs() == 3 && dz.abs() == 3 {
                    continue;
                }

                cap.push((dx, height, dz));
            }
        }
    }

    let stem: Vec<(i32, i32, i32)> = (1..height).map(|y| (0, y, 0)).collect();
    let caps = &blocks.caps[usize::from(red)];

    // Грань шляпки открыта, если за ней не другой блок гриба: так снаружи
    // шляпка, а внутри — поры.
    let mut placed: Vec<Placed> = cap
        .iter()
        .map(|&(x, y, z)| {
            let mut mask = 0;

            for (bit, (sx, sy, sz)) in FACE_STEPS.iter().enumerate() {
                let next = (x + sx, y + sy, z + sz);

                if !cap.contains(&next) && !stem.contains(&next) {
                    mask |= 1 << bit;
                }
            }

            (x, y, z, caps[mask])
        })
        .collect();

    placed.extend(stem.iter().map(|&(x, y, z)| (x, y, z, blocks.stem)));
    placed
}

/// Валуны и огромные грибы этого чанка — и зашедшие к нему от соседей.
fn round_features(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let key = area.key;

    for local_x in -ROUND_REACH..(CHUNK_SIZE + ROUND_REACH) {
        for local_z in -ROUND_REACH..(CHUNK_SIZE + ROUND_REACH) {
            let x = area.chunk_x * CHUNK_SIZE + local_x;
            let z = area.chunk_z * CHUNK_SIZE + local_z;

            // Сперва дешёвое «а не здесь ли», и только потом столбец.
            if !roll(key, x, z, SALT_ROUND, ROUND_DENSEST) {
                continue;
            }

            let column = area.column(x, z);

            let Some(round) = round_at(key, &column, x, z) else {
                continue;
            };

            for (dx, dy, dz, state) in round_blocks(key, round, blocks, x, z) {
                put_over_plants(chunk, blocks, local_x + dx, column.height + dy, local_z + dz, state);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Упавшие деревья
// ---------------------------------------------------------------------------

/// Порода упавшего дерева.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wood {
    Oak,
    Birch,
    Spruce,
    Jungle,
}

impl Wood {
    /// Длина лежачего ствола по вики: дуб 4–7, берёза 5–8, ель 6–10,
    /// джунгли 4–11.
    fn lengths(self) -> (i32, i32) {
        match self {
            Wood::Oak => (4, 7),
            Wood::Birch => (5, 8),
            Wood::Spruce => (6, 10),
            Wood::Jungle => (4, 11),
        }
    }
}

/// Самая большая густота упавших деревьев.
const FALLEN_DENSEST: f64 = 0.003;

/// Направления, в которые может лечь ствол.
const DIRECTIONS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

/// Какое дерево упало в этой точке, если упало. По вики они есть везде, где
/// растут стоячие той же породы, кроме лугов, рощ и бамбуковых джунглей;
/// в цветочном лесу — только берёзы.
fn fallen_at(key: u64, column: &Column, x: i32, z: i32) -> Option<Wood> {
    if !on_land(column) {
        return None;
    }

    let (density, wood) = match column.biome {
        Biome::Forest => {
            let birch = unit(key, x, z, SALT_FALLEN + 1) < 0.2;
            (0.0025, if birch { Wood::Birch } else { Wood::Oak })
        }
        Biome::FlowerForest => (0.0015, Wood::Birch),
        Biome::BirchForest | Biome::OldGrowthBirchForest => (0.003, Wood::Birch),
        Biome::Taiga | Biome::OldGrowthPineTaiga | Biome::OldGrowthSpruceTaiga => (0.003, Wood::Spruce),
        Biome::Jungle | Biome::SparseJungle => (0.002, Wood::Jungle),
        Biome::Plains | Biome::SunflowerPlains => (0.0003, Wood::Oak),
        _ => return None,
    };

    (unit(key, x, z, SALT_FALLEN) < density).then_some(wood)
}

/// Куда ляжет ствол — решается раньше породы: по нему видно, может ли
/// дерево дотянуться до чанка, и столбец зря не считается.
fn fallen_direction(key: u64, x: i32, z: i32) -> (i32, i32) {
    DIRECTIONS[(hash(key, x, z, SALT_FALLEN + 2) % 4) as usize]
}

/// Блоки упавшего дерева относительно земли под пнём: пень, зазор в один-
/// два блока и лежачий ствол, если земля под ним ровная; на стволе грибы,
/// на пне лианы. `column` — столбец в любом месте, нужен для проверки земли.
fn fallen_blocks(
    key: u64,
    wood: Wood,
    blocks: &Palette,
    x: i32,
    z: i32,
    height: i32,
    column: &dyn Fn(i32, i32) -> Column,
) -> Vec<Placed> {
    let mut dice = Dice::new(key, x, z, SALT_FALLEN + 3);
    let logs = &blocks.logs[wood as usize];
    let (step_x, step_z) = fallen_direction(key, x, z);
    let mut placed = vec![(0, 1, 0, logs[1])];

    // Лианы на пне — у дуба и джунглей, в трёх случаях из четырёх.
    if matches!(wood, Wood::Oak | Wood::Jungle) && dice.below(4) != 0 {
        for (index, (dx, dz)) in DIRECTIONS.iter().enumerate() {
            // Лиана с северной стороны пня держится за южный от неё блок.
            let hold = [1, 0, 3, 2][index];

            if dice.below(2) == 0 {
                placed.push((*dx, 1, *dz, blocks.vines[hold]));
            }
        }
    }

    let (shortest, longest) = wood.lengths();
    let length = shortest + dice.below(longest - shortest + 1);
    let start = 2 + dice.below(2);

    // Ствол ложится только на ровную землю; иначе от дерева остаётся пень —
    // вики так и пишет: ствол бывает не всегда.
    let flat = (start..start + length).all(|along| {
        let here = column(x + step_x * along, z + step_z * along);

        here.height == height && on_land(&here)
    });

    if !flat {
        return placed;
    }

    let axis = if step_x != 0 { 0 } else { 2 };

    for along in start..start + length {
        placed.push((step_x * along, 1, step_z * along, logs[axis]));
    }

    // Грибы на стволе: красные у ели и берёзы, коричневые у берёзы — один
    // или два.
    let mushrooms = match wood {
        Wood::Spruce => [blocks.red_mushroom, blocks.red_mushroom],
        Wood::Birch => [blocks.red_mushroom, blocks.brown_mushroom],
        Wood::Oak | Wood::Jungle => return placed,
    };

    for _ in 0..1 + dice.below(2) {
        let along = start + dice.below(length);
        let mushroom = mushrooms[dice.below(2) as usize];
        let at = (step_x * along, 2, step_z * along, mushroom);

        if !placed.contains(&at) {
            placed.push(at);
        }
    }

    placed
}

/// Упавшие деревья этого чанка и зашедшие к нему от соседей.
fn fallen_trees(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let key = area.key;

    for local_x in -FALLEN_REACH..(CHUNK_SIZE + FALLEN_REACH) {
        for local_z in -FALLEN_REACH..(CHUNK_SIZE + FALLEN_REACH) {
            let x = area.chunk_x * CHUNK_SIZE + local_x;
            let z = area.chunk_z * CHUNK_SIZE + local_z;

            if !roll(key, x, z, SALT_FALLEN, FALLEN_DENSEST) {
                continue;
            }

            // Дотянется ли дерево до чанка: от пня с лианами до самого
            // дальнего конца ствола.
            let (step_x, step_z) = fallen_direction(key, x, z);
            let far = (x + step_x * FALLEN_REACH, z + step_z * FALLEN_REACH);
            let from = ((x - 1).min(far.0), (z - 1).min(far.1));
            let to = ((x + 1).max(far.0), (z + 1).max(far.1));

            if !area.touches(from, to) {
                continue;
            }

            let column = area.column(x, z);

            let Some(wood) = fallen_at(key, &column, x, z) else {
                continue;
            };

            let lookup = |x: i32, z: i32| area.column(x, z);

            for (dx, dy, dz, state) in fallen_blocks(key, wood, blocks, x, z, column.height, &lookup) {
                put_over_plants(chunk, blocks, local_x + dx, column.height + dy, local_z + dz, state);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Грядки: тыквы, арбузы, ягоды, лепестки, цветы
// ---------------------------------------------------------------------------

/// Что растёт грядкой: несколько штук вокруг одной точки.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Patch {
    Pumpkin,
    Melon,
    Berries,
    Petals,
    Wildflowers,
    TallFlowers,
    Sunflowers,
    LilyOfTheValley,
    Bushes,
}

/// Как часто грядки и как они густы.
struct PatchRule {
    patch: Patch,
    salt: u64,
    /// Доля мест, где начинается грядка.
    centers: f64,
    radius: i32,
    /// Доля мест внутри грядки, где что-то выросло, — до поправки на биом.
    cells: f64,
}

/// Частота середин грядок — на один столбец, и густота внутри. Подобраны
/// так, чтобы на чанк выходило столько же, сколько в сохранённом мире
/// оригинала (tools/measure/plants_census.py): тыквы — в среднем одна на
/// двадцать чанков, арбузы в джунглях — одна-две на десять, кусты — по
/// нескольку штук тесной кучкой.
const PATCHES: [PatchRule; 9] = [
    PatchRule { patch: Patch::Pumpkin, salt: 72_001, centers: 0.000_03, radius: 3, cells: 0.2 },
    PatchRule { patch: Patch::Melon, salt: 72_011, centers: 0.000_65, radius: 3, cells: 0.25 },
    PatchRule { patch: Patch::Berries, salt: 72_019, centers: 0.000_33, radius: 3, cells: 0.3 },
    PatchRule { patch: Patch::Petals, salt: 72_031, centers: 0.02, radius: 3, cells: 0.65 },
    PatchRule { patch: Patch::Wildflowers, salt: 72_043, centers: 0.012, radius: 3, cells: 0.4 },
    PatchRule { patch: Patch::TallFlowers, salt: 72_047, centers: 0.006, radius: 2, cells: 0.35 },
    PatchRule { patch: Patch::Sunflowers, salt: 72_053, centers: 0.005, radius: 2, cells: 0.15 },
    PatchRule { patch: Patch::LilyOfTheValley, salt: 72_059, centers: 0.004, radius: 2, cells: 0.3 },
    PatchRule { patch: Patch::Bushes, salt: 72_061, centers: 0.0013, radius: 1, cells: 0.35 },
];

/// Насколько грядке рады в этом биоме: 0 — не растёт вовсе.
///
/// Биом берётся у самого места, а не у середины грядки: середина может
/// лежать у соседа, и столбец там считать незачем.
fn patch_share(patch: Patch, biome: Biome) -> f64 {
    match patch {
        // Тыквы — редкими грядками почти везде, где есть трава; в лесах,
        // саванне и болоте заметно реже (перепись мира оригинала).
        Patch::Pumpkin => match biome {
            Biome::Plains
            | Biome::SnowyPlains
            | Biome::FlowerForest
            | Biome::Taiga
            | Biome::OldGrowthPineTaiga
            | Biome::OldGrowthSpruceTaiga
            | Biome::SavannaPlateau
            | Biome::SparseJungle
            | Biome::WindsweptHills
            | Biome::WindsweptForest => 1.0,
            Biome::Savanna | Biome::SunflowerPlains => 0.3,
            Biome::BirchForest | Biome::Forest | Biome::OldGrowthBirchForest => 0.15,
            Biome::DarkForest | Biome::Swamp => 0.1,
            _ => 0.0,
        },
        Patch::Melon => match biome {
            Biome::BambooJungle => 0.35,
            Biome::Jungle => 0.23,
            Biome::SparseJungle => 0.1,
            _ => 0.0,
        },
        Patch::Berries => match biome {
            Biome::Taiga => 0.43,
            Biome::WindsweptForest => 0.3,
            Biome::OldGrowthSpruceTaiga => 0.2,
            Biome::OldGrowthPineTaiga => 0.13,
            _ => 0.0,
        },
        // С настройкой `pink-cherry-groves` лепестков в роще чуть больше.
        Patch::Petals => match biome {
            Biome::CherryGrove if super::terrain::PINK_CHERRY_GROVES.load(std::sync::atomic::Ordering::Relaxed) => 1.35,
            Biome::CherryGrove => 1.0,
            _ => 0.0,
        },
        Patch::Wildflowers => match biome {
            Biome::BirchForest => 0.37,
            Biome::OldGrowthBirchForest => 0.36,
            Biome::Meadow => 0.23,
            _ => 0.0,
        },
        Patch::TallFlowers => match biome {
            Biome::FlowerForest => 0.16,
            Biome::BirchForest => 0.057,
            Biome::Forest => 0.04,
            Biome::OldGrowthBirchForest => 0.026,
            _ => 0.0,
        },
        Patch::Sunflowers => if biome == Biome::SunflowerPlains { 1.0 } else { 0.0 },
        Patch::LilyOfTheValley => match biome {
            Biome::FlowerForest => 0.125,
            Biome::OldGrowthBirchForest => 0.025,
            Biome::Forest => 0.02,
            Biome::BirchForest => 0.006,
            _ => 0.0,
        },
        // Кусты — по вики: равнины, леса, берёзовые леса, продуваемые
        // холмы и берега рек.
        Patch::Bushes => match biome {
            Biome::Plains => 1.0,
            Biome::WindsweptForest => 1.45,
            Biome::WindsweptHills => 0.65,
            Biome::BirchForest | Biome::OldGrowthBirchForest => 0.57,
            Biome::Forest => 0.4,
            Biome::WindsweptGravellyHills => 0.25,
            Biome::River | Biome::FrozenRiver => 0.15,
            _ => 0.0,
        },
    }
}

/// Грядки, задевающие чанк: середина может быть и у соседа, но каждый
/// ставит только то, что внутри него.
fn patches(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let key = area.key;

    for rule in &PATCHES {
        let reach = rule.radius;

        for center_x in -reach..(CHUNK_SIZE + reach) {
            for center_z in -reach..(CHUNK_SIZE + reach) {
                let x = area.chunk_x * CHUNK_SIZE + center_x;
                let z = area.chunk_z * CHUNK_SIZE + center_z;

                if !roll(key, x, z, rule.salt, rule.centers) {
                    continue;
                }

                // Высокий цветок один на всю грядку: сирень, розовый куст
                // или пион.
                let kind = (hash(key, x, z, rule.salt + 1) % 3) as usize;
                let cell_salt = hash(key, x, z, rule.salt + 2);

                for dx in -reach..=reach {
                    for dz in -reach..=reach {
                        let (local_x, local_z) = (center_x + dx, center_z + dz);

                        if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
                            continue;
                        }

                        let column = *area.near(local_x, local_z);
                        let share = patch_share(rule.patch, column.biome);

                        if share <= 0.0 || !on_land(&column) || !roll(key, x + dx, z + dz, cell_salt, rule.cells * share) {
                            continue;
                        }

                        grow_in_patch(key, chunk, blocks, rule.patch, kind, (x + dx, z + dz), (local_x, local_z), &column);
                    }
                }
            }
        }
    }
}

/// Одно растение грядки.
#[allow(clippy::too_many_arguments)]
fn grow_in_patch(
    key: u64,
    chunk: &mut Chunk,
    blocks: &Palette,
    patch: Patch,
    kind: usize,
    (x, z): (i32, i32),
    (local_x, local_z): (i32, i32),
    column: &Column,
) {
    let y = column.height + 1;
    let Some(ground) = block_in(chunk, local_x, column.height, local_z) else {
        return;
    };

    if !blocks.ground.soil(ground) {
        return;
    }

    // Лепестки лежат пучками по одному–четыре и смотрят в разные стороны.
    let mut dice = Dice::new(key, x, z, 72_101);

    match patch {
        Patch::Pumpkin | Patch::Melon => {
            if ground == blocks.ground.grass_block {
                let fruit = if patch == Patch::Pumpkin { blocks.pumpkin } else { blocks.melon };
                put_in_air(chunk, local_x, y, local_z, fruit);
            }
        }
        Patch::Berries => {
            put_in_air(chunk, local_x, y, local_z, blocks.berry_bush);
        }
        Patch::Petals | Patch::Wildflowers => {
            let states = if patch == Patch::Petals { &blocks.pink_petals } else { &blocks.wildflowers };
            let state = states[dice.below(4) as usize][dice.below(4) as usize];
            put_in_air(chunk, local_x, y, local_z, state);
        }
        Patch::TallFlowers => {
            put_pair(chunk, local_x, y, local_z, blocks.tall_flowers[kind]);
        }
        Patch::Sunflowers => {
            put_pair(chunk, local_x, y, local_z, blocks.sunflower);
        }
        Patch::LilyOfTheValley => {
            put_in_air(chunk, local_x, y, local_z, blocks.lily_of_the_valley);
        }
        Patch::Bushes => {
            if ground == blocks.ground.grass_block {
                put_in_air(chunk, local_x, y, local_z, blocks.bush);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Растения по одному: трава, тростник, кактус, водоросли
// ---------------------------------------------------------------------------

/// Густота кактусов. В бесплодных землях реже, чем в пустыне (вики);
/// числа — по замеру мира оригинала: кактус там редкость, один на
/// несколько чанков.
fn cactus_density(biome: Biome) -> f64 {
    match biome {
        Biome::Desert => 0.0015,
        Biome::Badlands | Biome::ErodedBadlands | Biome::WoodedBadlands => 0.0012,
        _ => 0.0,
    }
}

/// Стоит ли в этом месте кактус. Смотрится только место и биом: соседу
/// нужно знать это и за краем чанка.
fn cactus_here(key: u64, x: i32, z: i32, biome: Biome) -> bool {
    roll(key, x, z, SALT_CACTUS, cactus_density(biome))
}

/// Доля клеток 8×8, где у воды растёт кучка тростника. Подогнано под
/// перепись мира оригинала (стеблей на чанк по биомам).
fn cane_density(biome: Biome) -> f64 {
    match biome {
        Biome::Desert => 0.6,
        Biome::WoodedBadlands => 0.12,
        Biome::Swamp => 0.03,
        Biome::Beach | Biome::Badlands | Biome::ErodedBadlands => 0.1,
        Biome::MangroveSwamp => 0.03,
        Biome::River => 0.018,
        _ => 0.035,
    }
}

/// Простой цветок в один блок.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Flower {
    Dandelion,
    Poppy,
    BlueOrchid,
    Allium,
    AzureBluet,
    RedTulip,
    OrangeTulip,
    WhiteTulip,
    PinkTulip,
    OxeyeDaisy,
    Cornflower,
}

impl Flower {
    fn name(self) -> &'static str {
        match self {
            Flower::Dandelion => "dandelion",
            Flower::Poppy => "poppy",
            Flower::BlueOrchid => "blue_orchid",
            Flower::Allium => "allium",
            Flower::AzureBluet => "azure_bluet",
            Flower::RedTulip => "red_tulip",
            Flower::OrangeTulip => "orange_tulip",
            Flower::WhiteTulip => "white_tulip",
            Flower::PinkTulip => "pink_tulip",
            Flower::OxeyeDaisy => "oxeye_daisy",
            Flower::Cornflower => "cornflower",
        }
    }
}

/// Все простые цветы — в порядке `Flower`.
const FLOWERS: [Flower; 11] = [
    Flower::Dandelion,
    Flower::Poppy,
    Flower::BlueOrchid,
    Flower::Allium,
    Flower::AzureBluet,
    Flower::RedTulip,
    Flower::OrangeTulip,
    Flower::WhiteTulip,
    Flower::PinkTulip,
    Flower::OxeyeDaisy,
    Flower::Cornflower,
];

/// Какие цветы растут в биоме и в какой доле — по замеру мира оригинала
/// (сколько каждого на чанк, в сотых).
type Bouquet = &'static [(Flower, u32)];

const PLAINS_FLOWERS: Bouquet = &[
    (Flower::Dandelion, 95),
    (Flower::AzureBluet, 13),
    (Flower::Poppy, 12),
    (Flower::OxeyeDaisy, 12),
    (Flower::Cornflower, 12),
    (Flower::WhiteTulip, 2),
    (Flower::RedTulip, 1),
    (Flower::OrangeTulip, 1),
    (Flower::PinkTulip, 1),
];
const MEADOW_FLOWERS: Bouquet = &[
    (Flower::Poppy, 30),
    (Flower::Dandelion, 29),
    (Flower::AzureBluet, 17),
    (Flower::Cornflower, 16),
    (Flower::OxeyeDaisy, 8),
    (Flower::Allium, 7),
];
const FLOWER_FOREST_FLOWERS: Bouquet = &[
    (Flower::OrangeTulip, 30),
    (Flower::RedTulip, 24),
    (Flower::WhiteTulip, 24),
    (Flower::PinkTulip, 16),
    (Flower::AzureBluet, 13),
    (Flower::Allium, 8),
    (Flower::OxeyeDaisy, 7),
    (Flower::Poppy, 3),
    (Flower::Cornflower, 2),
    (Flower::Dandelion, 1),
];
const BIRCH_FLOWERS: Bouquet = &[(Flower::Poppy, 12), (Flower::Allium, 4), (Flower::OxeyeDaisy, 3)];
const SWAMP_FLOWERS: Bouquet = &[
    (Flower::BlueOrchid, 27),
    (Flower::PinkTulip, 13),
    (Flower::WhiteTulip, 7),
    (Flower::AzureBluet, 6),
    (Flower::OrangeTulip, 5),
    (Flower::RedTulip, 2),
];
const HILLS_FLOWERS: Bouquet =
    &[(Flower::Dandelion, 5), (Flower::OrangeTulip, 5), (Flower::WhiteTulip, 3), (Flower::Allium, 2)];
const COMMON_FLOWERS: Bouquet = &[(Flower::Dandelion, 1), (Flower::Poppy, 1)];

/// Что покрывает землю биома: доля столбцов под каждым растением.
///
/// Числа подобраны по замеру мира оригинала — сколько чего на чанк
/// (tools/measure/plants_census.py) — с поправкой на то, что часть столбцов
/// занята деревьями, водой и грядками.
#[derive(Clone, Copy)]
struct Cover {
    grass: f64,
    fern: f64,
    tall_grass: f64,
    large_fern: f64,
    /// Простые цветы — в среднем по биому; растут они полянками (`meadows`).
    flowers: f64,
    bouquet: Bouquet,
    /// Доля клеток 8×8, где цветы есть; 1 — растут всюду.
    meadows: f64,
    /// Опавшие листья под кроной и на открытом месте рядом.
    litter: (f64, f64),
    brown_mushroom: f64,
    /// Кусты со светлячками: у воды, в болоте — где угодно.
    firefly_bush: f64,
    dead_bush: f64,
    /// Сухая трава на песке и терракоте: короткая и высокая.
    dry_grass: f64,
}

const BARE: Cover = Cover {
    grass: 0.0,
    fern: 0.0,
    tall_grass: 0.0,
    large_fern: 0.0,
    flowers: 0.0,
    bouquet: COMMON_FLOWERS,
    meadows: 0.25,
    litter: (0.0, 0.0),
    brown_mushroom: 0.0,
    firefly_bush: 0.0,
    dead_bush: 0.0,
    dry_grass: 0.0,
};

/// Покров земли в биоме.
fn cover(biome: Biome) -> Cover {
    // Светлячки — почти везде, где у воды растёт трава.
    let grassy = Cover { firefly_bush: 0.01, ..BARE };

    match biome {
        Biome::Plains => Cover {
            grass: 0.21,
            fern: 0.0003,
            tall_grass: 0.013,
            flowers: 0.0073,
            bouquet: PLAINS_FLOWERS,
            ..grassy
        },
        Biome::SunflowerPlains => Cover {
            grass: 0.176,
            tall_grass: 0.012,
            flowers: 0.0058,
            bouquet: PLAINS_FLOWERS,
            ..grassy
        },
        Biome::Meadow => Cover {
            grass: 0.11,
            tall_grass: 0.025,
            flowers: 0.054,
            bouquet: MEADOW_FLOWERS,
            meadows: 1.0,
            ..BARE
        },
        Biome::Forest => Cover {
            grass: 0.032,
            fern: 0.0004,
            tall_grass: 0.0004,
            flowers: 0.00015,
            litter: (0.7, 0.26),
            ..grassy
        },
        Biome::DarkForest => Cover {
            grass: 0.032,
            fern: 0.0004,
            tall_grass: 0.001,
            litter: (0.53, 0.22),
            ..grassy
        },
        Biome::WoodedBadlands => Cover {
            grass: 0.004,
            fern: 0.0005,
            litter: (0.11, 0.002),
            dead_bush: 0.066,
            dry_grass: 0.0105,
            ..BARE
        },
        Biome::FlowerForest => Cover {
            grass: 0.019,
            tall_grass: 0.0006,
            flowers: 0.065,
            bouquet: FLOWER_FOREST_FLOWERS,
            meadows: 1.0,
            ..grassy
        },
        Biome::BirchForest => Cover {
            grass: 0.045,
            tall_grass: 0.0003,
            flowers: 0.001,
            bouquet: BIRCH_FLOWERS,
            ..grassy
        },
        Biome::OldGrowthBirchForest => Cover { grass: 0.04, fern: 0.0002, ..grassy },
        Biome::Taiga => Cover {
            grass: 0.0035,
            fern: 0.018,
            tall_grass: 0.0004,
            large_fern: 0.015,
            brown_mushroom: 0.0022,
            ..grassy
        },
        Biome::SnowyTaiga => Cover {
            grass: 0.0047,
            fern: 0.015,
            large_fern: 0.0159,
            brown_mushroom: 0.0014,
            ..grassy
        },
        Biome::OldGrowthPineTaiga => Cover {
            grass: 0.04,
            fern: 0.106,
            tall_grass: 0.002,
            large_fern: 0.013,
            brown_mushroom: 0.0246,
            dead_bush: 0.0023,
            ..grassy
        },
        Biome::OldGrowthSpruceTaiga => Cover {
            grass: 0.037,
            fern: 0.114,
            tall_grass: 0.0025,
            large_fern: 0.0188,
            brown_mushroom: 0.013,
            dead_bush: 0.0022,
            ..grassy
        },
        Biome::Jungle => Cover { grass: 0.254, fern: 0.087, dead_bush: 0.0003, ..grassy },
        Biome::SparseJungle => Cover { grass: 0.44, fern: 0.143, dead_bush: 0.0004, ..grassy },
        Biome::BambooJungle => Cover { grass: 0.32, fern: 0.09, tall_grass: 0.0097, ..grassy },
        Biome::Savanna => Cover { grass: 0.37, tall_grass: 0.012, flowers: 0.0003, ..grassy },
        Biome::SavannaPlateau => Cover { grass: 0.37, tall_grass: 0.011, flowers: 0.0003, ..grassy },
        Biome::WindsweptSavanna => Cover { grass: 0.04, ..grassy },
        Biome::Swamp => Cover {
            grass: 0.13,
            fern: 0.0007,
            flowers: 0.0043,
            bouquet: SWAMP_FLOWERS,
            brown_mushroom: 0.027,
            firefly_bush: 0.002,
            dead_bush: 0.0022,
            ..BARE
        },
        Biome::MangroveSwamp => Cover { grass: 0.25, firefly_bush: 0.01, dead_bush: 0.004, ..BARE },
        Biome::SnowyPlains => Cover { grass: 0.02, ..BARE },
        Biome::Grove => Cover { grass: 0.0024, fern: 0.0005, tall_grass: 0.0005, ..BARE },
        Biome::SnowySlopes => Cover { grass: 0.009, tall_grass: 0.001, ..BARE },
        Biome::JaggedPeaks => Cover { grass: 0.07, tall_grass: 0.012, ..BARE },
        Biome::StonyPeaks => Cover { grass: 0.13, ..BARE },
        Biome::FrozenPeaks => Cover { grass: 0.0035, ..BARE },
        Biome::WindsweptHills => Cover {
            grass: 0.008,
            tall_grass: 0.0004,
            flowers: 0.0008,
            bouquet: HILLS_FLOWERS,
            ..grassy
        },
        Biome::WindsweptForest => Cover {
            grass: 0.03,
            fern: 0.0008,
            tall_grass: 0.0014,
            large_fern: 0.0003,
            ..grassy
        },
        Biome::WindsweptGravellyHills => Cover { grass: 0.007, tall_grass: 0.0012, ..grassy },
        Biome::River => Cover { grass: 0.1, fern: 0.002, tall_grass: 0.002, ..grassy },
        Biome::FrozenRiver => Cover { grass: 0.15, fern: 0.003, large_fern: 0.006, ..BARE },
        Biome::Beach | Biome::StonyShore => Cover { grass: 0.05, ..grassy },
        Biome::CherryGrove => Cover { grass: 0.37, tall_grass: 0.02, ..grassy },
        Biome::Desert => Cover { dead_bush: 0.007, dry_grass: 0.012, ..BARE },
        Biome::Badlands => Cover { dead_bush: 0.066, dry_grass: 0.0082, ..BARE },
        Biome::ErodedBadlands => Cover { dead_bush: 0.04, dry_grass: 0.0069, ..BARE },
        Biome::MushroomFields | Biome::IceSpikes => BARE,
        _ => Cover { grass: 0.05, ..BARE },
    }
}

/// Все растения по одному на столбец.
fn single_plants(area: &Area, chunk: &mut Chunk, blocks: &Palette) {
    let key = area.key;

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let x = area.chunk_x * CHUNK_SIZE + local_x;
            let z = area.chunk_z * CHUNK_SIZE + local_z;
            let column = *area.near(local_x, local_z);

            if column.height < SEA {
                underwater(key, chunk, blocks, &column, x, z, local_x, local_z);
            } else {
                on_ground(area, chunk, blocks, &column, x, z, local_x, local_z);
            }
        }
    }
}

/// Растение на суше, если место над землёй свободно.
#[allow(clippy::too_many_arguments)]
fn on_ground(area: &Area, chunk: &mut Chunk, blocks: &Palette, column: &Column, x: i32, z: i32, local_x: i32, local_z: i32) {
    let key = area.key;

    let height = column.height;
    let y = height + 1;
    let biome = column.biome;

    if biome.is_ocean() {
        return;
    }

    let here = chunk.block(local_x, y, local_z);
    let ground = chunk.block(local_x, height, local_z);

    // Под слоем снега трава всё равно растёт: растение встаёт вместо снега,
    // а земля под ним перестаёт быть заснеженной.
    let snowed = here == blocks.snow && ground == blocks.ground.snowy_grass;

    if here != AIR && !snowed {
        return;
    }

    let soil = blocks.ground.soil(ground);
    let sandy = blocks.ground.sandy(ground);

    if here == AIR {
        // Тростник: на берегу у самой воды, если вода вровень с землёй сбоку.
        if height == SEA && !biome.is_freezing() && (soil || sandy) {
            let shore = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dz)| {
                let next = area.near(local_x + dx, local_z + dz);

                next.height < SEA && !next.biome.is_freezing()
            });

            // Тростник растёт кучками: у оригинала это грядка из нескольких
            // стеблей вокруг одной точки у воды, а не ряд вдоль всего берега.
            // Кучка — клетка 8×8, внутри неё стебель у большей части мест
            // у воды.
            let clump = roll(key, x >> 3, z >> 3, SALT_CANE + 7, cane_density(biome));

            if shore && clump && roll(key, x, z, SALT_CANE, 0.6) {
                let mut dice = Dice::new(key, x, z, SALT_CANE + 1);

                for dy in 0..dice.stalk(2) {
                    if !put_in_air(chunk, local_x, y + dy, local_z, blocks.sugar_cane) {
                        break;
                    }
                }

                return;
            }
        }

        // Кактус: 1–3 блока, в четверти случаев с цветком. Сбоку не должно
        // быть ни земли, ни другого кактуса — иначе он бы сломался.
        if sandy && cactus_here(key, x, z, biome) {
            let clear = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dz)| {
                let next = area.near(local_x + dx, local_z + dz);

                next.height <= height && !cactus_here(key, x + dx, z + dz, next.biome)
            });

            if clear {
                let mut dice = Dice::new(key, x, z, SALT_CACTUS + 1);
                let tall = dice.stalk(1);
                let mut grown = 0;

                while grown < tall && put_in_air(chunk, local_x, y + grown, local_z, blocks.cactus) {
                    grown += 1;
                }

                if let Some(flower) = blocks.cactus_flower.filter(|_| grown == tall && dice.below(4) == 0) {
                    put_in_air(chunk, local_x, y + grown, local_z, flower);
                }

                return;
            }
        }
    }

    let cover = cover(biome);
    let dry = sandy || ground == blocks.ground.terracotta;
    let value = unit(key, x, z, SALT_COVER);

    // Мёртвые кусты и сухая трава — на песке, терракоте, а мёртвые кусты
    // и на земле; в мангровом болоте — на иле.
    let bush_ground = soil || dry || (biome == Biome::MangroveSwamp && ground == blocks.ground.mud);
    let mut edge = cover.dead_bush;

    if value < edge {
        if bush_ground && !snowed {
            put_in_air(chunk, local_x, y, local_z, blocks.dead_bush);
        }

        return;
    }

    edge += cover.dry_grass;

    if value < edge {
        if dry && !snowed {
            // Короткой и высокой у оригинала поровну.
            let tall = unit(key, x, z, SALT_COVER + 1) < 0.5;
            put_in_air(chunk, local_x, y, local_z, if tall { blocks.tall_dry_grass } else { blocks.short_dry_grass });
        }

        return;
    }

    // Дальше — то, что растёт на земле с травой; в мангровом болоте — и на
    // иле.
    let earthy = soil || snowed || (biome == Biome::MangroveSwamp && ground == blocks.ground.mud);
    let grow = |chunk: &mut Chunk, state: i32| {
        if snowed {
            chunk.put_generated(local_x, height, local_z, blocks.ground.grass_block, true);
        }

        chunk.put_generated(local_x, y, local_z, state, true);
    };

    let two_high = |chunk: &mut Chunk, pair: [i32; 2]| {
        if block_in(chunk, local_x, y + 1, local_z) != Some(AIR) {
            return;
        }

        if snowed {
            chunk.put_generated(local_x, height, local_z, blocks.ground.grass_block, true);
        }

        chunk.put_generated(local_x, y, local_z, pair[0], true);
        chunk.put_generated(local_x, y + 1, local_z, pair[1], true);
    };

    edge += cover.tall_grass;

    if value < edge {
        if earthy {
            two_high(chunk, blocks.tall_grass);
        }

        return;
    }

    edge += cover.large_fern;

    if value < edge {
        if earthy {
            two_high(chunk, blocks.large_fern);
        }

        return;
    }

    edge += cover.grass;

    if value < edge {
        if earthy {
            grow(chunk, blocks.short_grass);
        }

        return;
    }

    edge += cover.fern;

    if value < edge {
        if earthy {
            grow(chunk, blocks.fern);
        }

        return;
    }

    // Цветы растут полянками, а не ровной россыпью: у оригинала они
    // сбиваются в пятна, между которыми их нет вовсе.
    if cover.flowers > 0.0 {
        let meadow = cover.meadows >= 1.0 || roll(key, x >> 3, z >> 3, SALT_MEADOW, cover.meadows);

        edge += if meadow { cover.flowers / cover.meadows } else { 0.0 };

        if value < edge {
            if earthy {
                grow(chunk, blocks.flowers[pick_flower(key, x, z, cover.bouquet) as usize]);
            }

            return;
        }
    }

    edge += cover.brown_mushroom;

    if value < edge {
        if earthy {
            grow(chunk, blocks.brown_mushroom);
        }

        return;
    }

    if cover.firefly_bush > 0.0 {
        edge += cover.firefly_bush;

        // У воды: рядом столбец под уровнем моря. В болоте — где угодно.
        if value < edge {
            let wet = biome == Biome::Swamp
                || [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dz)| area.near(local_x + dx, local_z + dz).height < SEA);

            if earthy && wet {
                grow(chunk, blocks.firefly_bush);
            }

            return;
        }
    }

    // Опавшие листья — вокруг деревьев: гуще всего под кроной, реже на
    // открытом месте рядом. Кроны ищем, только если листьям здесь место.
    let (under, open) = cover.litter;

    if under <= 0.0 || value >= edge + under || snowed {
        return;
    }

    let canopy = (y + 1..=y + 20).any(|up| {
        let state = chunk.block(local_x, up, local_z);

        blocks.leaves.iter().any(|(low, high)| (*low..=*high).contains(&state))
    });

    if value >= edge + if canopy { under } else { open } {
        return;
    }

    if earthy || dry || ground == blocks.ground.gravel || ground == blocks.ground.stone {
        // Кучек у оригинала по одной, две и три поровну, четыре — вдвое
        // реже.
        let mut dice = Dice::new(key, x, z, SALT_COVER + 3);
        let amount = [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3][dice.below(17) as usize];

        put_in_air(chunk, local_x, y, local_z, blocks.leaf_litter[dice.below(4) as usize][amount]);
    }
}

/// Какой цветок из букета растёт в этом месте.
fn pick_flower(key: u64, x: i32, z: i32, bouquet: Bouquet) -> Flower {
    let total: u32 = bouquet.iter().map(|(_, weight)| weight).sum();
    let mut left = (hash(key, x, z, SALT_FLOWER) % u64::from(total.max(1))) as u32;

    for (flower, weight) in bouquet {
        if left < *weight {
            return *flower;
        }

        left -= weight;
    }

    bouquet[0].0
}

/// Растение под водой или на ней: кувшинки, ламинария, морская трава.
#[allow(clippy::too_many_arguments)]
fn underwater(key: u64, chunk: &mut Chunk, blocks: &Palette, column: &Column, x: i32, z: i32, local_x: i32, local_z: i32) {
    let biome = column.biome;
    let bottom = column.height + 1;
    let depth = SEA - column.height;

    // Кувшинки — на болотной воде.
    let lily_pads = match biome {
        Biome::Swamp => 0.032,
        Biome::MangroveSwamp => 0.05,
        _ => 0.0,
    };

    if lily_pads > 0.0
        && chunk.block(local_x, SEA, local_z) == blocks.water
        && roll(key, x, z, SALT_LILY_PAD, lily_pads)
    {
        put_in_air(chunk, local_x, SEA + 1, local_z, blocks.lily_pad);
    }

    if chunk.block(local_x, bottom, local_z) != blocks.water || !blocks.ground.seabed(chunk.block(local_x, column.height, local_z)) {
        return;
    }

    let put_water = |chunk: &mut Chunk, y: i32, state: i32| {
        if block_in(chunk, local_x, y, local_z) != Some(blocks.water) {
            return false;
        }

        chunk.put_generated(local_x, y, local_z, state, true);
        true
    };

    // Ламинария — в холодных, обычных и тёплых-умеренных океанах, лесами:
    // в одних клетках 8×8 её много, в других нет вовсе. Густота — по замеру
    // мира оригинала.
    let kelp = match biome {
        Biome::DeepOcean => 0.24,
        Biome::DeepColdOcean => 0.21,
        Biome::ColdOcean => 0.19,
        Biome::Ocean => 0.18,
        Biome::LukewarmOcean => 0.126,
        Biome::DeepLukewarmOcean => 0.12,
        _ => 0.0,
    };

    if depth >= 3 && roll(key, x >> 3, z >> 3, SALT_KELP_ZONE, 0.35) && roll(key, x, z, SALT_KELP, kelp) {
        let mut dice = Dice::new(key, x, z, SALT_KELP + 1);
        let length = 1 + dice.below((depth - 1).min(25));

        for dy in 0..length - 1 {
            if !put_water(chunk, bottom + dy, blocks.kelp_plant) {
                return;
            }
        }

        put_water(chunk, bottom + length - 1, blocks.kelp[dice.below(25) as usize]);
        return;
    }

    // Морская трава — в реках, озёрах, болотах и незамёрзших океанах; в глубоких
    // океанах она большей частью высокая. Густота и доля высокой — по
    // замеру мира оригинала.
    let (density, tall_share) = match biome {
        Biome::DeepOcean => (0.17, 0.75),
        Biome::DeepLukewarmOcean => (0.2, 0.77),
        Biome::DeepColdOcean => (0.15, 0.8),
        Biome::LukewarmOcean => (0.25, 0.3),
        Biome::WarmOcean => (0.23, 0.3),
        Biome::Ocean => (0.166, 0.31),
        Biome::ColdOcean => (0.12, 0.33),
        Biome::River => (0.125, 0.43),
        Biome::Swamp => (0.114, 0.83),
        Biome::MangroveSwamp => (0.04, 0.6),
        // Реки и озёра посреди суши другого биома — изредка; у пляжей и
        // каменистых берегов — пореже, чем в море.
        Biome::Beach | Biome::StonyShore => (0.02, 0.33),
        _ if !biome.is_freezing() && !biome.is_ocean() => (0.017, 0.4),
        _ => return,
    };

    if !roll(key, x, z, SALT_SEAGRASS, density) {
        return;
    }

    let tall = depth >= 2
        && unit(key, x, z, SALT_SEAGRASS + 1) < tall_share
        && block_in(chunk, local_x, bottom + 1, local_z) == Some(blocks.water);

    if tall {
        put_water(chunk, bottom, blocks.tall_seagrass[0]);
        put_water(chunk, bottom + 1, blocks.tall_seagrass[1]);
    } else {
        put_water(chunk, bottom, blocks.seagrass);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::Style;
    use std::collections::HashMap;

    /// Семя мира решает, где растёт растительность.
    #[test]
    fn plants_move_with_the_seed() {
        let spots = |seed: i64| {
            let key = key_of(&Terrain::new(seed, Style::Vanilla));

            (0..4000).filter(|&i| roll(key, i % 64, i / 64, SALT_COVER, 0.1)).collect::<Vec<_>>()
        };

        assert_eq!(spots(1), spots(1));
        assert_ne!(spots(1), spots(2), "растительность стоит одинаково в разных мирах");
    }

    /// Крупные штуки не выходят за полосу, которую чанк просматривает.
    #[test]
    fn features_stay_within_reach() {
        let blocks = palette();
        let key = 0;

        for x in -200..200 {
            let z = x * 7 + 3;

            for round in [Round::Boulder, Round::BrownMushroom, Round::RedMushroom] {
                for (dx, dy, dz, _) in round_blocks(key, round, blocks, x, z) {
                    assert!(dx.abs() <= ROUND_REACH && dz.abs() <= ROUND_REACH, "{:?}: {} {}", round, dx, dz);
                    assert!(dy >= -2, "{:?} ушёл в землю на {}", round, dy);
                }
            }

            let flat = |_: i32, _: i32| Column::flat(70);

            for wood in [Wood::Oak, Wood::Birch, Wood::Spruce, Wood::Jungle] {
                let placed = fallen_blocks(key, wood, blocks, x, z, 70, &flat);
                let (shortest, longest) = wood.lengths();
                let logs = placed.iter().filter(|b| b.1 == 1 && blocks.logs[wood as usize].contains(&b.3)).count() as i32;

                // Пень и ствол положенной длины.
                assert!((shortest + 1..=longest + 1).contains(&logs), "{:?}: брёвен {}", wood, logs);

                for (dx, _, dz, _) in placed {
                    assert!(dx.abs().max(dz.abs()) <= FALLEN_REACH, "{:?} дотянулось до {} {}", wood, dx, dz);
                }
            }
        }
    }

    /// Высота тростника и кактуса — по вики: нижняя 11/18, средняя 5/18,
    /// верхняя 2/18.
    #[test]
    fn stalks_follow_the_wiki() {
        let mut counts = [0; 3];
        let key = 0;

        for x in 0..18_000 {
            counts[(Dice::new(key, x, 5, 1).stalk(2) - 2) as usize] += 1;
        }

        assert!((10_500..11_500).contains(&counts[0]), "{:?}", counts);
        assert!((4_500..5_500).contains(&counts[1]), "{:?}", counts);
        assert!((1_700..2_300).contains(&counts[2]), "{:?}", counts);
    }

    /// Огромный гриб: шляпка снаружи — шляпка, ножка без верха и низа.
    #[test]
    fn mushroom_caps_face_outwards() {
        let blocks = palette();
        let key = 0;

        for red in [false, true] {
            let placed = huge_mushroom(&mut Dice::new(key, 1, 2, 3), blocks, red);
            let top = placed.iter().map(|b| b.1).max().unwrap();

            // Середина верха шляпки: сверху открыта, снизу под ней ножка.
            let middle = placed.iter().find(|b| (b.0, b.1, b.2) == (0, top, 0)).unwrap();
            let mask = blocks.caps[usize::from(red)].iter().position(|s| *s == middle.3).unwrap();

            assert_eq!(mask & 0b11, 0b10, "низ закрыт, верх открыт");
            assert!(placed.iter().any(|b| b.3 == blocks.stem));
        }
    }

    /// Один и тот же чанк складывается одинаково при каждом обращении.
    #[test]
    fn decoration_repeats() {
        for style in [Style::Smooth, Style::Vanilla] {
            let generator = Generator::Normal(Terrain::new(4_242, style));

            for (chunk_x, chunk_z) in [(0, 0), (-3, 7), (11, -5)] {
                let one = Chunk::generated(&generator, chunk_x, chunk_z).to_bytes();
                let two = Chunk::generated(&generator, chunk_x, chunk_z).to_bytes();

                assert!(one == two, "чанк {} {} вышел разным", chunk_x, chunk_z);
            }
        }
    }

    /// Крупное, что лежит через границу чанков, дорисовано с обеих сторон:
    /// ни один его блок не остался воздухом ни в одном из чанков.
    #[test]
    fn features_cross_chunk_borders() {
        let blocks = palette();
        let mut crossing = 0;

        for style in [Style::Smooth, Style::Vanilla] {
            let terrain = Terrain::new(20_260_923, style);
            let generator = Generator::Normal(Terrain::new(20_260_923, style));
            let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
            let lookup = |x: i32, z: i32| terrain.column_at(x, z);
            let key = key_of(&terrain);

            'search: for x in -600..600 {
                for z in -600..600 {
                    let round = roll(key, x, z, SALT_ROUND, ROUND_DENSEST);
                    let fallen = roll(key, x, z, SALT_FALLEN, FALLEN_DENSEST);

                    if !round && !fallen {
                        continue;
                    }

                    let column = terrain.column_at(x, z);
                    let mut placed = Vec::new();

                    if let Some(kind) = round.then(|| round_at(key, &column, x, z)).flatten() {
                        placed = round_blocks(key, kind, blocks, x, z);
                    } else if let Some(wood) = fallen.then(|| fallen_at(key, &column, x, z)).flatten() {
                        placed = fallen_blocks(key, wood, blocks, x, z, column.height, &lookup);
                    }

                    let touched: Vec<(i32, i32)> = placed
                        .iter()
                        .map(|b| ((x + b.0).div_euclid(CHUNK_SIZE), (z + b.2).div_euclid(CHUNK_SIZE)))
                        .collect();

                    if placed.is_empty() || touched.iter().all(|c| *c == touched[0]) {
                        continue;
                    }

                    crossing += 1;

                    for ((dx, dy, dz, state), key) in placed.iter().zip(&touched) {
                        let chunk = chunks.entry(*key).or_insert_with(|| Chunk::generated(&generator, key.0, key.1));
                        let (local_x, local_z) = ((x + dx).rem_euclid(CHUNK_SIZE), (z + dz).rem_euclid(CHUNK_SIZE));
                        let here = chunk.block(local_x, column.height + dy, local_z);

                        // Вытеснить штуку может только то, что стояло раньше
                        // (дерево, земля); воздух на её месте — значит,
                        // соседний чанк свою часть не дорисовал.
                        assert!(
                            here != AIR,
                            "{:?}: блок {:?} у {} {} пропал в чанке {:?}",
                            style,
                            crate::blocks::block_at_state(*state),
                            x + dx,
                            z + dz,
                            key
                        );
                    }

                    if chunks.len() > 40 {
                        break 'search;
                    }
                }
            }
        }

        assert!(crossing > 0, "не нашлось ни одной штуки на границе чанков");
    }

    /// Растительность вообще есть: на полосе мира растёт всё, что обещано.
    #[test]
    fn plants_do_grow() {
        let generator = Generator::Normal(Terrain::new(7, Style::Smooth));
        let mut seen: HashMap<&'static str, usize> = HashMap::new();

        for chunk_x in 0..12 {
            for chunk_z in 0..12 {
                let chunk = Chunk::generated(&generator, chunk_x * 9, chunk_z * 9);

                for section in chunk.sections.iter().flatten() {
                    for state in section.iter() {
                        if let Some(name) = crate::blocks::block_at_state(*state as i32) {
                            *seen.entry(name).or_default() += 1;
                        }
                    }
                }
            }
        }

        for name in ["tall_grass", "seagrass", "short_grass", "leaf_litter", "bush"] {
            assert!(seen.get(name).copied().unwrap_or(0) > 0, "не выросло {}; есть {:?}", name, seen.keys().collect::<Vec<_>>());
        }
    }

    /// Сколько чего выросло — посмотреть глазами:
    /// `cargo test vegetation_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn vegetation_census() {
        let generator = Generator::Normal(Terrain::new(20_260_923, Style::Vanilla));
        let mut seen: HashMap<&'static str, usize> = HashMap::new();

        for chunk_x in 0..20 {
            for chunk_z in 0..20 {
                let chunk = Chunk::generated(&generator, chunk_x * 7 - 70, chunk_z * 7 - 70);

                for section in chunk.sections.iter().flatten() {
                    for state in section.iter() {
                        if let Some(name) = crate::blocks::block_at_state(*state as i32) {
                            *seen.entry(name).or_default() += 1;
                        }
                    }
                }
            }
        }

        let mut rows: Vec<_> = seen.into_iter().collect();
        rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

        for (name, n) in rows {
            println!("{:>9}  {}", n, name);
        }
    }
}
