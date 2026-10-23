// Заброшенные лагеря 26.3 (abandoned camp): где стоят и чем сложены. Только профиль Preview26_3.
//
// Где — вики (Abandoned Camp, «Generation»): области 37×37 чанка, одна попытка в каждой; лагерь
// встаёт, если биом в точке попытки — один из 18 (бамбуковые и редкие джунгли, лес, цветочный лес,
// берёзовый и старовозрастный берёзовый леса, бледный сад, тайга, снежная, обе старовозрастные
// тайги, луг, вишнёвая роща, саванна, ветреный лес, болото, лесистые бедленды, пятнистый лес).
// Сдвиг попытки 0…29 чанков по каждой оси подтверждён семью лагерями официального 26.3
// (область 37, а не 34: у трёх лагерей сдвиг больше 26). Соль и поток случаев — свои [проверить],
// поэтому положение лагерей не совпадает с оригиналом в том же мире.
//
// Чем — замер официального 26.3 (tools/measure/camp_templates_263.py): `/place template` каждого
// из 297 кусков (180 шатров: 18 биомов × 10; 45 общих лагерей: особый, сундук, бочка по 15; 72
// лагеря по биомам: 4 на биом) в залитую барьером ячейку; воздух образца затирает барьер, поэтому
// рамка точная. Раскладка — как у оригинала по стыкам jigsaw: шатёр (старт) с блоком, смотрящим
// на запад, и лагерь с блоком, смотрящим на восток, прикладываются стык к стыку, дерево растёт
// над блоком-стыком с пулом `abandoned_camp/trees/*`. Весь лагерь поворачивается как целое
// вокруг угла чанка старта (замер: PosX/PosZ шатра = угол чанка). Выбор шатра — один из десяти
// биома, лагеря — один из 49 (45 общих и 4 биомных), равновероятно (вики).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::template::{self, Cell, Orient, Template};
use crate::blocks;

pub const SPACING: i32 = 37;
pub const SEPARATION: i32 = 8;
const SALT: u64 = 0xA8A4_D0ED;

/// Дальше этого числа чанков от старта кусков нет (шатёр и лагерь по 10 блоков, деревья до 8).
const REACH_CHUNKS: i32 = 3;

/// Стык jigsaw куска: клетка, куда смотрит, пул, имя и цель.
#[derive(Clone, Debug)]
struct Joint {
    cell: (i32, i32, i32),
    front: (i32, i32, i32),
    pool: String,
    name: String,
}

struct Library {
    templates: Vec<Template>,
    joints: Vec<Vec<Joint>>,
}

fn front_of(orientation: &str) -> (i32, i32, i32) {
    match orientation.split('_').next().unwrap_or("") {
        "west" => (-1, 0, 0),
        "east" => (1, 0, 0),
        "north" => (0, 0, -1),
        "south" => (0, 0, 1),
        "up" => (0, 1, 0),
        _ => (0, -1, 0),
    }
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();

    LIBRARY.get_or_init(|| {
        let text = include_str!("abandoned_camp.txt");
        let templates = template::parse(text);
        let mut joints: Vec<Vec<Joint>> = vec![Vec::new(); templates.len()];

        for line in text.lines() {
            if line.starts_with("template") {
                break;
            }

            // jigsaw <образец> a y b <orientation> <final_state> <pool> <name> <target>
            let words: Vec<&str> = line.split_whitespace().collect();

            if let ["jigsaw", name, a, y, b, orientation, _, pool, joint_name, _] = words.as_slice()
                && let Some(at) = templates.iter().position(|known| known.name == *name)
            {
                let number = |text: &str| text.parse::<i32>().unwrap_or(0);

                joints[at].push(Joint {
                    cell: (number(a), number(y), number(b)),
                    front: front_of(orientation),
                    pool: pool.to_string(),
                    name: joint_name.to_string(),
                });
            }
        }

        Library { templates, joints }
    })
}

fn find(name: &str) -> Option<usize> {
    library().templates.iter().position(|known| known.name == name)
}

/// Название биома в путях образцов, если в нём бывают лагеря (вики: 18 биомов).
pub fn folder(biome: super::terrain::Biome) -> Option<&'static str> {
    use super::terrain::Biome;

    Some(match biome {
        Biome::BambooJungle | Biome::SparseJungle | Biome::Forest | Biome::FlowerForest | Biome::BirchForest
        | Biome::OldGrowthBirchForest | Biome::PaleGarden | Biome::Taiga | Biome::SnowyTaiga | Biome::OldGrowthPineTaiga
        | Biome::OldGrowthSpruceTaiga | Biome::Meadow | Biome::CherryGrove | Biome::Savanna | Biome::WindsweptForest
        | Biome::Swamp | Biome::WoodedBadlands | Biome::DappledForest => biome.name(),
        _ => return None,
    })
}

pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Поворот точки (x, z) на `turns` четвертей по часовой.
fn spin(x: i32, z: i32, turns: u8) -> (i32, i32) {
    match turns % 4 {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

/// Кусок лагеря: образец и смещение его угла от угла шатра до поворота.
#[derive(Clone, Copy, Debug)]
struct Piece {
    template: usize,
    offset: (i32, i32, i32),
}

/// Лагерь области.
#[derive(Clone, Debug)]
pub struct Site {
    pub chunk: (i32, i32),
    pivot: (i32, i32),
    turns: u8,
    base_y: i32,
    pieces: Vec<Piece>,
    /// Деревья: клетка земли (до поворота, от угла шатра) и пул.
    trees: Vec<TreeSpot>,
}

impl Site {
    /// Мировые x, z клетки (a, b) куска; y кусок не меняет.
    fn world(&self, a: i32, b: i32) -> (i32, i32) {
        let (dx, dz) = spin(a, b, self.turns);

        (self.pivot.0 + dx, self.pivot.1 + dz)
    }

    /// Название шатра и лагеря — для проверок и `locate`.
    #[cfg(all(test, not(seedmap_core)))]
    #[allow(dead_code)]
    fn names(&self) -> Vec<&str> {
        self.pieces.iter().map(|piece| library().templates[piece.template].name.as_str()).collect()
    }
}

/// Складывает лагерь из шатра `tent` и лагеря `camp` по стыку; None — стыки не сошлись.
/// Деревья лагеря: клетка земли (до поворота, от угла шатра) и пул.
type TreeSpot = ((i32, i32, i32), String);

fn assemble(tent: usize, camp: usize) -> Option<(Vec<Piece>, Vec<TreeSpot>)> {
    let library = library();
    let from = library.joints[tent].iter().find(|joint| joint.pool.starts_with("abandoned_camp/camp/"))?;
    let to = library.joints[camp].iter().find(|joint| joint.name == "end_connector")?;

    if from.front.0 + to.front.0 != 0 || from.front.2 + to.front.2 != 0 {
        return None;
    }

    // Стык лагеря встаёт в клетку перед стыком шатра.
    let offset = (from.cell.0 + from.front.0 - to.cell.0, from.cell.1 + from.front.1 - to.cell.1, from.cell.2 + from.front.2 - to.cell.2);
    let mut trees = Vec::new();

    for joint in &library.joints[tent] {
        if joint.pool.starts_with("abandoned_camp/trees/") {
            trees.push((joint.cell, joint.pool.clone()));
        }
    }

    for joint in &library.joints[camp] {
        if joint.pool.starts_with("abandoned_camp/trees/") {
            trees.push(((joint.cell.0 + offset.0, joint.cell.1 + offset.1, joint.cell.2 + offset.2), joint.pool.clone()));
        }
    }

    Some((vec![Piece { template: tent, offset: (0, 0, 0) }, Piece { template: camp, offset }], trees))
}

/// Какие образцы выбраны для биома по случаю `dice`: шатёр из десяти, лагерь из сорока девяти.
/// `retry` сдвигает выбор лагеря: если выбранный не стыкуется (у `campsite_pale_garden_2` нет блока
/// `end_connector`), игра берёт другой элемент пула, и мы тоже.
fn choose(folder: &str, dice: u64, retry: u64) -> Option<(usize, usize)> {
    let tent = find(&format!("abandoned_camp/tent/{folder}/tent_{folder}_{}", 1 + dice % 10))?;
    let pick = ((dice >> 8) + retry) % 49;
    let camp = if pick < 45 {
        let kind = ["special", "chest", "barrel"][(pick / 15) as usize];

        find(&format!("abandoned_camp/camp/default/campsite_default_{kind}_{}", 1 + pick % 15))?
    } else {
        find(&format!("abandoned_camp/camp/{folder}/campsite_{folder}_{}", pick - 44))?
    };

    Some((tent, camp))
}

pub fn site_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Arc<Site>> {
    type Sites = Mutex<HashMap<(i64, i32, i32), Option<Arc<Site>>>>;
    static CACHE: OnceLock<Sites> = OnceLock::new();

    if !terrain.preview() {
        return None;
    }

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (terrain.seed(), region_x, region_z);

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let seed = terrain.seed();
    let (chunk_x, chunk_z) = attempt(seed, region_x, region_z);
    let pivot = (chunk_x * 16, chunk_z * 16);
    let found = (|| {
        let folder = folder(terrain.column_in(pivot.0 + 8, pivot.1 + 8, None).biome)?;
        let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x51DE);
        let ((tent, _), (pieces, trees)) = (0..49).find_map(|retry| {
            let pair = choose(folder, dice >> 8, retry)?;

            assemble(pair.0, pair.1).map(|joined| (pair, joined))
        })?;
        let turns = (dice % 4) as u8;
        let library = library();
        let size = (library.templates[tent].width, library.templates[tent].length);
        let mut site = Site { chunk: (chunk_x, chunk_z), pivot, turns, base_y: 0, pieces, trees };
        let centre = site.world(size.0 / 2, size.1 / 2);

        // Низ шатра — верхний блок земли в середине шатра (замер: PosY шатра = высота земли).
        site.base_y = terrain.column_at(centre.0, centre.1).height;
        Some(Arc::new(site))
    })();

    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found.clone());
    found
}

/// Шерстяные ступени каталога Preview26_3 по записи `white_wool_stairs[facing=…,half=…,shape=…,waterlogged=…]`.
fn wool_stairs(text: &str) -> Option<i32> {
    use crate::gameplay_catalog::{Catalog, Profile, StairState, STAIR_FACINGS, STAIR_SHAPES, WOOL_STAIRS};

    let (name, rest) = text.split_once('[').unwrap_or((text, ""));
    let row = WOOL_STAIRS.iter().find(|row| row.name == name)?;
    let mut state = StairState { facing: 0, top: false, shape: 0, waterlogged: false };

    for pair in rest.trim_end_matches(']').split(',') {
        match pair.split_once('=') {
            Some(("facing", value)) => state.facing = STAIR_FACINGS.iter().position(|known| *known == value)? as u8,
            Some(("half", value)) => state.top = value == "top",
            Some(("shape", value)) => state.shape = STAIR_SHAPES.iter().position(|known| *known == value)? as u8,
            Some(("waterlogged", value)) => state.waterlogged = value == "true",
            _ => {}
        }
    }

    Catalog::new(Profile::Preview26_3).stair_state_for(row, state).ok()
}

/// Запись без свойств в образце — состояние по умолчанию; у блоков с направлением оно «на север»,
/// поэтому перед поворотом лагеря свойства дописываются (иначе сундук не повернулся бы).
fn explicit(text: &str) -> &str {
    match text {
        "barrel" => "barrel[facing=north,open=false]",
        "chest" => "chest[facing=north,type=single,waterlogged=false]",
        "oxidized_copper_chest" => "oxidized_copper_chest[facing=north,type=single,waterlogged=false]",
        "straw_bed" => "straw_bed[facing=north,occupied=false,part=foot]",
        "white_wool_stairs" => "white_wool_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
        "wildflowers" => "wildflowers[facing=north,flower_amount=1]",
        other => other,
    }
}

fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text)
        .or_else(|| wool_stairs(text))
        .or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text)))
        .unwrap_or(super::AIR)
}

fn table(name: &str) -> &'static [crate::loot::Pool] {
    match name {
        "chests/abandoned_camp_secret_chest" => &crate::loot::ABANDONED_CAMP_SECRET,
        "barrels/abandoned_camp_barrel" => &crate::loot::ABANDONED_CAMP_BARREL,
        _ => &crate::loot::ABANDONED_CAMP_COMMON,
    }
}

pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    if !terrain.preview() {
        return;
    }

    let seed = terrain.seed();
    let library = library();
    let (cx0, cz0) = (chunk_x * 16, chunk_z * 16);

    for region_x in (chunk_x - REACH_CHUNKS).div_euclid(SPACING)..=(chunk_x + REACH_CHUNKS).div_euclid(SPACING) {
        for region_z in (chunk_z - REACH_CHUNKS).div_euclid(SPACING)..=(chunk_z + REACH_CHUNKS).div_euclid(SPACING) {
            let (sx, sz) = attempt(seed, region_x, region_z);

            if (sx - chunk_x).abs() > REACH_CHUNKS || (sz - chunk_z).abs() > REACH_CHUNKS {
                continue;
            }

            let Some(site) = site_in(terrain, region_x, region_z) else { continue };
            let orient = Orient { turns: site.turns, mirror: false };

            for piece in &site.pieces {
                let template = &library.templates[piece.template];

                for (a, y, b, cell) in template.cells() {
                    let (x, z) = site.world(a + piece.offset.0, b + piece.offset.2);
                    let wy = site.base_y + piece.offset.1 + y;

                    if x < cx0 || x > cx0 + 15 || z < cz0 || z > cz0 + 15 || !(super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT).contains(&wy) {
                        continue;
                    }

                    let (lx, lz) = (x - cx0, z - cz0);

                    let Cell::Block(index) = cell else {
                        // Воздух образца затирает всё, что стояло (замер на барьере).
                        if cell == Cell::Air {
                            chunk.put_generated(lx, wy, lz, super::AIR, true);
                        }

                        continue;
                    };
                    let text = template::orient_text(explicit(&template.texts[usize::from(index)]), orient);

                    chunk.put_generated(lx, wy, lz, state_of(&text), true);

                    if text.starts_with("chest") || text.starts_with("barrel") {
                        let kind = if text.starts_with("chest") { crate::containers::Kind::Chest } else { crate::containers::Kind::Barrel };
                        let mut dice = crate::loot::Dice::new(template::roll(seed, x, z, 0xCA3F ^ (wy as u64)));
                        let mut container = crate::containers::Container::new(kind);

                        container.slots = crate::loot::fill(table(template.loot_at(a, y, b).unwrap_or("")), container.slots.len(), &mut dice);
                        chunk.put_generated_container(lx, wy, lz, container);
                    }
                }
            }

            // Деревья над блоками-стыками: земля под ними уже уложена куском.
            let ground = super::TreeGround { seed, ..super::TreeGround::new() };

            for ((a, y, b), pool) in &site.trees {
                let (x, z) = site.world(*a, *b);
                let wy = site.base_y + y;

                if (x - cx0 + super::terrain::TREE_REACH).abs() > 15 + 2 * super::terrain::TREE_REACH {
                    continue;
                }

                if let Some(tree) = super::trees::camp_tree(seed, x, z, pool) {
                    super::draw_tree(chunk, &tree, x - cx0, z - cz0, wy, x, z, &ground);
                } else if pool.ends_with("/bamboo") {
                    bamboo(chunk, seed, (x - cx0, wy, z - cz0), (x, z));
                }
            }
        }
    }
}

/// Бамбук над блоком-стыком: стебель 5–12 блоков и листва на трёх верхних [проверить: высота].
fn bamboo(chunk: &mut super::Chunk, seed: i64, at: (i32, i32, i32), world: (i32, i32)) {
    let height = 5 + (template::roll(seed, world.0, world.1, 0xBA3B) % 8) as i32;

    for step in 1..=height {
        let leaves = match height - step {
            0 => "large",
            1 => "large",
            2 => "small",
            _ => "none",
        };
        let state = state_of(&format!("bamboo[age=1,leaves={leaves},stage=0]"));

        chunk.put_generated(at.0, at.1 + step, at.2, state, false);
    }
}

/// Ближайший лагерь (`locate structure`): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| site_in(terrain, region_x, region_z).map(|site| site.chunk))
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    /// 297 образцов: 180 шатров, 45 общих лагерей и 72 лагеря по биомам.
    #[test]
    fn library_has_all_pieces() {
        let templates = &library().templates;

        assert_eq!(templates.len(), 297);
        assert_eq!(templates.iter().filter(|t| t.name.starts_with("abandoned_camp/tent/")).count(), 180);
        assert_eq!(templates.iter().filter(|t| t.name.contains("/camp/default/")).count(), 45);
    }

    /// У каждого шатра есть стык с пулом лагеря, у каждого лагеря — встречный; все сходятся.
    #[test]
    fn every_pair_joins() {
        let templates = &library().templates;

        for (tent, template) in templates.iter().enumerate().filter(|(_, t)| t.name.contains("/tent/")) {
            let folder = template.name.split('/').nth(2).unwrap();

            let mut joined = 0;

            for pick in 0..49u64 {
                let (_, camp) = choose(folder, (tent as u64 % 10) + (pick << 8), 0).expect("пара");

                joined += i32::from(assemble(tent, camp).is_some());
            }

            // Не стыкуется только `campsite_pale_garden_2` (в бледном саду).
            assert_eq!(joined, if folder == "pale_garden" { 48 } else { 49 }, "{}", template.name);
        }
    }

    /// Замер двух лагерей официального 26.3 (PosX, PosY, PosZ кусков в записи о строении):
    /// угол образца в мире после поворота всего лагеря должен совпасть с записью оригинала.
    #[test]
    fn matches_the_recorded_camps() {
        // (шатёр, лагерь, поворот по часовой, угол старта, PosX/Y/Z шатра, PosX/Y/Z лагеря).
        let samples = [
            ("abandoned_camp/tent/forest/tent_forest_6", "abandoned_camp/camp/default/campsite_default_chest_14", 3u8, (80, 80), (80, 62, 80), (79, 62, 90)),
            ("abandoned_camp/tent/forest/tent_forest_3", "abandoned_camp/camp/default/campsite_default_special_5", 3, (656, -560), (656, 71, -560), (655, 70, -550)),
            ("abandoned_camp/tent/flower_forest/tent_flower_forest_7", "abandoned_camp/camp/default/campsite_default_special_8", 1, (-320, -768), (-320, 62, -768), (-319, 62, -778)),
            ("abandoned_camp/tent/meadow/tent_meadow_7", "abandoned_camp/camp/default/campsite_default_barrel_8", 3, (688, 336), (688, 100, 336), (687, 100, 346)),
        ];

        for (tent_name, camp_name, turns, pivot, tent_pos, camp_pos) in samples {
            let (tent, camp) = (find(tent_name).unwrap(), find(camp_name).unwrap());
            let (pieces, _) = assemble(tent, camp).unwrap();
            let site = Site { chunk: (0, 0), pivot, turns, base_y: tent_pos.1, pieces, trees: Vec::new() };

            for (piece, expected) in site.pieces.iter().zip([tent_pos, camp_pos]) {
                let (x, z) = site.world(piece.offset.0, piece.offset.2);

                assert_eq!((x, site.base_y + piece.offset.1, z), expected, "{}", library().templates[piece.template].name);
            }
        }
    }

    /// Все состояния образцов известны таблице блоков (иначе кусок рассыпался бы воздухом).
    #[test]
    fn every_block_state_resolves() {
        let mut missing = std::collections::BTreeSet::new();

        for template in &library().templates {
            for text in &template.texts {
                if blocks::state_from_text(text).or_else(|| wool_stairs(text)).or_else(|| blocks::state_by_name(text.split('[').next().unwrap())).is_none() {
                    missing.insert(text.clone());
                }
            }
        }

        assert!(missing.is_empty(), "не найдены: {missing:?}");
    }

    /// Попытки лежат в своей области, сдвиг 0…29.
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -5..5 {
            for region_z in -5..5 {
                let (x, z) = attempt(17, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }
    }
}
