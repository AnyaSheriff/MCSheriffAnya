// Кораблекрушения (shipwreck) и зарытые сокровища (buried treasure).
//
// Где — по вики (Structure set «shipwrecks»: spacing 24, separation 4):
// мир поделён на области 24×24 чанка, в каждой одна попытка — чанк со
// сдвигом 0…19. Биом середины чанка: океан — кораблекрушение на дне,
// пляж и снежный пляж — выброшенное на берег (shipwreck_beached; вики,
// «Generation»).
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/template_merge.py
// templates shipwreck, 70 кораблей, src/world/shipwreck.txt): 20 образцов,
// у всех экземпляров одного образца блоки одни и те же (случайности нет,
// переписи сошлись с вики до блока по дубовым доскам), разное только
// дерево: основное и второе — одно из восьми сочетаний вики («Primary dark
// oak with secondary jungle and spruce, primary jungle with secondary spruce,
// primary oak with secondary birch and spruce, primary spruce with secondary
// dark oak, jungle, and oak»). В образце основное — дуб, второе — ель.
// Повернут на случайную четверть, без отражения (в записях оригинала поля
// Mirror нет). Сундуки — с таблицами по месту (поле LootTable у экземпляров:
// в носу — припасы, в корме — карта и сокровище).
//
// Высота океанских кораблей 26.3: среднее первой свободной клетки над дном
// по неповёрнутому размеру от TP. Проверено 35 окнами без структур и
// четырьмя дополнительными контролями. Старые профили и береговые корабли
// сохраняют медиану и прежнее заглубление [проверить].
//
// Зарытое сокровище (вики, Buried Treasure; Structure set: spacing 1,
// «probability of 1%», locate_offset 9, 0, 9): в чанке пляжа с долей 1 %
// сундук в клетке 9, 9 чанка, «always facing east», в воде — waterlogged;
// глубина под верхом земли — замер оригинала (см. `TREASURE_DEPTH`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::template::{self, Cell, Orient, Template};
use crate::blocks;

/// Область попыток, зазор и соль (вики, Structure set).
pub const SPACING: i32 = 24;
pub const SEPARATION: i32 = 4;
const SALT: u64 = 165_745_295;

/// Сочетания дерева (основное, второе) — вики, «Structure».
const WOODS: [(&str, &str); 8] = [
    ("dark_oak", "jungle"),
    ("dark_oak", "spruce"),
    ("jungle", "spruce"),
    ("oak", "birch"),
    ("oak", "spruce"),
    ("spruce", "dark_oak"),
    ("spruce", "jungle"),
    ("spruce", "oak"),
];

pub fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| template::parse(include_str!("shipwreck.txt")))
}

/// Порядок выбора снят с полей Template/Rot сохранённых начал 26.3.
/// minecraft.wiki/w/Shipwreck: «upright, keeled sideways, or upside-down».
const OCEAN_POOL: [&str; 20] = [
    "with_mast", "upsidedown_full", "upsidedown_fronthalf", "upsidedown_backhalf",
    "sideways_full", "sideways_fronthalf", "sideways_backhalf", "rightsideup_full",
    "rightsideup_fronthalf", "rightsideup_backhalf", "with_mast_degraded",
    "upsidedown_full_degraded", "upsidedown_fronthalf_degraded", "upsidedown_backhalf_degraded",
    "sideways_full_degraded", "sideways_fronthalf_degraded", "sideways_backhalf_degraded",
    "rightsideup_full_degraded", "rightsideup_fronthalf_degraded", "rightsideup_backhalf_degraded",
];

const BEACHED_POOL: [&str; 11] = [
    "with_mast", "sideways_full", "sideways_fronthalf", "sideways_backhalf", "rightsideup_full",
    "rightsideup_fronthalf", "rightsideup_backhalf", "with_mast_degraded",
    "rightsideup_full_degraded", "rightsideup_fronthalf_degraded", "rightsideup_backhalf_degraded",
];

fn selection(seed: i64, x: i32, z: i32, beached: bool) -> (usize, Orient) {
    let mut random = template::StructureRandom::for_chunk(seed, x, z);
    let orient = Orient { turns: random.below(4) as u8, mirror: false };
    let pool: &[&str] = if beached { &BEACHED_POOL } else { &OCEAN_POOL };
    let name = pool[random.below(pool.len() as i32) as usize];
    let index = library().iter().position(|t| t.name.strip_prefix("shipwreck/") == Some(name)).expect("образец корабля");
    (index, orient)
}

/// Поворот рамки вокруг (4, 15), подтверждён BB/TPX/TPZ всех измеренных
/// кораблей: Orient принимает нижний угол, а сохранённый TP — опорную точку.
fn frame_corner(x: i32, z: i32, t: &Template, turns: u8) -> (i32, i32) {
    let (dx, dz) = match turns % 4 {
        0 => (0, 0),
        1 => (20 - t.length, 11),
        2 => (9 - t.width, 31 - t.length),
        _ => (-11, 20 - t.width),
    };
    (x + dx, z + dz)
}

/// Корабль области: образец, поворот, угол рамки, дерево, на берегу ли.
#[derive(Clone, Copy, Debug)]
pub struct Ship {
    pub template: usize,
    pub orient: Orient,
    pub origin: (i32, i32, i32),
    pub woods: (&'static str, &'static str),
    pub beached: bool,
    pub chunk: (i32, i32),
}

impl Ship {
    pub fn footprint(&self) -> (i32, i32) {
        library()[self.template].footprint(self.orient)
    }
}

pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::grid_attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

fn legacy_attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Мир 26.1.2 сохраняет прежнюю сетку; исправление относится к 26.3.
pub(super) fn attempt_for(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> (i32, i32) {
    if terrain.generation_profile() & 2 != 0 { attempt(terrain.seed(), region_x, region_z) }
    else { legacy_attempt(terrain.seed(), region_x, region_z) }
}


/// Вид по биому: Some(false) — на дне, Some(true) — на берегу.
pub fn kind_of(biome: super::terrain::Biome) -> Option<bool> {
    use super::terrain::Biome;

    match biome {
        biome if biome.is_ocean() => Some(false),
        Biome::Beach | Biome::SnowyBeach => Some(true),
        _ => None,
    }
}

/// Корабль области обычного мира.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Ship> {
    type Starts = Mutex<HashMap<(i64, u8, i32, i32), Option<Ship>>>;
    static CACHE: OnceLock<Starts> = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (terrain.seed(), terrain.generation_profile(), region_x, region_z);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let seed = terrain.seed();
    let (chunk_x, chunk_z) = attempt_for(terrain, region_x, region_z);
    let (x, z) = (chunk_x * 16, chunk_z * 16);
    let found = kind_of(terrain.column_in(x + 8, z + 8, None).biome).map(|beached| {
        let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x5419);
        let template = (dice % library().len() as u64) as usize;
        let orient = Orient { turns: ((dice >> 16) % 4) as u8, mirror: false };
        let (template, orient) = if terrain.generation_profile() & 2 != 0 {
            selection(seed, chunk_x, chunk_z, beached)
        } else { (template, orient) };
        let (x, z) = if terrain.generation_profile() & 2 != 0 {
            frame_corner(x, z, &library()[template], orient.turns)
        } else { (x, z) };
        let woods = WOODS[((dice >> 24) % WOODS.len() as u64) as usize];
        let y = if terrain.generation_profile() & 2 != 0 && !beached {
            let t = &library()[template];
            let (tx, tz) = (chunk_x * 16, chunk_z * 16);
            let grid = (terrain.style() == super::terrain::Style::Vanilla)
                .then(|| terrain.relief_grid(tx, tz, tx + t.width - 1, tz + t.length - 1));
            ocean_height(tx, tz, t.width, t.length, &|wx, wz| terrain.column_in(wx, wz, grid.as_ref()).height)
        } else {
            let (sx, sz) = library()[template].footprint(orient);
            let mut heights = Vec::new();
            // Прежний профиль и береговые корабли сохраняют старое заглубление [проверить].
            for dx in (0..sx).step_by(2) {
                for dz in (0..sz).step_by(2) {
                    let height = terrain.column_at(x + dx, z + dz).height;
                    heights.push(if beached { height.max(super::terrain::SEA) } else { height });
                }
            }
            heights.sort_unstable();
            let median = heights.get(heights.len() / 2).copied().unwrap_or(super::terrain::SEA);
            placement_height(median, library()[template].height, beached, dice)
        };

        Ship { template, orient, origin: (x, y, z), woods, beached, chunk: (chunk_x, chunk_z) }
    });

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// minecraft.wiki/w/Heightmap: «OCEAN_FLOOR_WG». Контроль без структур
/// отделяет дно от кораллов и корабля. 39 естественных начал подтверждают
/// среднее первой свободной клетки по НЕповёрнутому размеру от TP, а не
/// медиану по повёрнутой рамке; диапазоны не включают дальний край size.
fn ocean_height(x: i32, z: i32, width: i32, length: i32, ground: &dyn Fn(i32, i32) -> i32) -> i32 {
    let mut sum = 0i64;
    for dx in 0..width {
        for dz in 0..length {
            sum += i64::from(ground(x + dx, z + dz)) + 1;
        }
    }
    (sum / (i64::from(width) * i64::from(length))) as i32
}

/// Дерево образца (дуб — основное, ель — второе) → дерево корабля.
fn rewood(text: &str, woods: (&str, &str)) -> String {
    if let Some(rest) = text.strip_prefix("oak_") {
        format!("{}_{}", woods.0, rest)
    } else if let Some(rest) = text.strip_prefix("spruce_") {
        format!("{}_{}", woods.1, rest)
    } else {
        text.to_string()
    }
}

fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text).or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text))).unwrap_or(super::AIR)
}

/// Таблица добычи по имени из образца.
#[cfg(not(seedmap_core))]
fn table(name: &str) -> &'static [crate::loot::Pool] {
    match name {
        "shipwreck_map" => &crate::loot::SHIPWRECK_MAP,
        "shipwreck_treasure" => &crate::loot::SHIPWRECK_TREASURE,
        _ => &crate::loot::SHIPWRECK_SUPPLY,
    }
}

/// Кладёт в сложенный чанк части ближних кораблей и зарытые сокровища.
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let seed = terrain.seed();

    for region_x in (chunk_x - 2).div_euclid(SPACING)..=(chunk_x + 1).div_euclid(SPACING) {
        for region_z in (chunk_z - 2).div_euclid(SPACING)..=(chunk_z + 1).div_euclid(SPACING) {
            if let Some(ship) = start_in(terrain, region_x, region_z) {
                paint(seed, &ship, chunk, chunk_x, chunk_z);
            }
        }
    }

    if let Some(y) = treasure_in(terrain, chunk_x, chunk_z) {
        bury(seed, chunk, chunk_x, chunk_z, y);
    }
}

#[cfg(not(seedmap_core))]
fn paint(seed: i64, ship: &Ship, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let template = &library()[ship.template];
    let (sx, sz) = ship.footprint();
    let (x0, y0, z0) = ship.origin;
    let (cx0, cz0) = (chunk_x * 16, chunk_z * 16);

    if x0 > cx0 + 15 || x0 + sx <= cx0 || z0 > cz0 + 15 || z0 + sz <= cz0 {
        return;
    }

    for (a, y, b, cell) in template.cells() {
        let Cell::Block(index) = cell else {
            continue;
        };
        let (dx, dz) = ship.orient.offset(a, b, template.width, template.length);
        let (x, wy, z) = (x0 + dx, y0 + y, z0 + dz);

        if x < cx0 || x > cx0 + 15 || z < cz0 || z > cz0 + 15 || !(super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT).contains(&wy) {
            continue;
        }

        let (lx, lz) = (x - cx0, z - cz0);
        let text = rewood(&template::orient_text(&template.texts[usize::from(index)], ship.orient), ship.woods);
        let wet = blocks::block_at_state(chunk.block(lx, wy, lz)) == Some("water") && text.contains("waterlogged=false");

        chunk.put_generated(lx, wy, lz, state_of(&if wet { text.replace("waterlogged=false", "waterlogged=true") } else { text.clone() }), true);

        if text.starts_with("chest") {
            let mut dice = crate::loot::Dice::new(template::roll(seed, x, z, 0x5A1F ^ (wy as u64)));
            let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

            chest.slots = crate::loot::fill(table(template.loot_at(a, y, b).unwrap_or("shipwreck_supply")), chest.slots.len(), &mut dice);
            chunk.put_generated_container(lx, wy, lz, chest);
        }
    }
}

/// Доля чанков пляжа с кладом (вики: «Probability of 1%»).
const TREASURE_SHARE: f64 = 0.01;

/// Глубина сундука под верхом земли клетки 9, 9 (замер оригинала, 39
/// кладов: на 1 блок ниже верха — 12, на 2 — 10, на 3 — 13, глубже — 4;
/// сверху всегда песок или гравий).
const TREASURE_DEPTH: [i32; 3] = [1, 2, 3];

/// Высота сундука клада в этом чанке, если клад есть.
pub fn treasure_in(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Option<i32> {
    use super::terrain::Biome;

    let chance = if terrain.generation_profile() & 2 != 0 {
        f64::from(template::placement_random(terrain.seed(), chunk_x, chunk_z, 10_387_320).float())
    } else { template::chance(terrain.seed(), chunk_x, 0, chunk_z, 0) };
    if chance >= TREASURE_SHARE {
        return None;
    }

    let column = terrain.column_at(chunk_x * 16 + 9, chunk_z * 16 + 9);

    let depth = TREASURE_DEPTH[(template::roll(terrain.seed(), chunk_x, chunk_z, 0xB0_7EA7) % 3) as usize];

    // Вики, Buried Treasure: «relative 9»; биом проверяем в клетке сундука.
    // Отдельного подтверждения иного смещения у границы пляжа нет.
    matches!(column.biome, Biome::Beach | Biome::SnowyBeach).then_some(column.height - depth)
}

/// Сундук клада: клетка 9, 9 чанка, смотрит на восток (вики).
#[cfg(not(seedmap_core))]
fn bury(seed: i64, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32, y: i32) {
    let wet = blocks::block_at_state(chunk.block(9, y, 9)) == Some("water");
    let text = format!("chest[facing=east,type=single,waterlogged={}]", wet);
    let mut dice = crate::loot::Dice::new(template::roll(seed, chunk_x, chunk_z, 0xB0_7EA6));
    let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

    chunk.put_generated(9, y, 9, state_of(&text), true);

    // Сверху — песок, если над сундуком пусто или вода (вики: «a gravel or
    // sand block covers it»).
    if matches!(blocks::block_at_state(chunk.block(9, y + 1, 9)), Some("air" | "water")) {
        chunk.put_generated(9, y + 1, 9, state_of("sand"), true);
    }
    chest.slots = crate::loot::fill(&crate::loot::BURIED_TREASURE, chest.slots.len(), &mut dice);
    chunk.put_generated_container(9, y, 9, chest);
}

/// Ближайший корабль (`locate structure shipwreck` / `shipwreck_beached`).
pub fn nearest(terrain: &super::terrain::Terrain, beached: bool, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| start_in(terrain, region_x, region_z).filter(|ship| ship.beached == beached).map(|ship| ship.chunk))
}

/// Ближайший клад (`locate structure buried_treasure`): чанк, по кольцам
/// чанков до `radius`.
pub fn nearest_treasure(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, radius: i32) -> Option<(i32, i32)> {
    for ring in 0..=radius {
        let mut best: Option<(i32, i32)> = None;

        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                let (x, z) = (chunk_x + dx, chunk_z + dz);
                let closer = best.is_none_or(|(bx, bz)| dx * dx + dz * dz < (bx - chunk_x).pow(2) + (bz - chunk_z).pow(2));

                if closer && treasure_in(terrain, x, z).is_some() {
                    best = Some((x, z));
                }
            }
        }

        if best.is_some() {
            return best;
        }
    }

    None
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;
    use crate::world::terrain::Biome;

    /// 20 образцов, сундуки с таблицами: в каждом 1–3 сундука, у каждого
    /// сундука записана таблица; у целого корабля — все три (вики: «up to 3
    /// chests … map chest, supply chest, treasure chest»).
    #[test]
    fn templates_have_their_chests() {
        let library = library();

        assert_eq!(library.len(), 20);

        for template in library {
            let chests: Vec<(i32, i32, i32)> = template.cells().filter(|(_, _, _, cell)| matches!(cell, Cell::Block(i) if template.block_name(*i) == "chest")).map(|(a, y, b, _)| (a, y, b)).collect();

            assert!((1..=3).contains(&chests.len()), "{}", template.name);

            for &(a, y, b) in &chests {
                assert!(template.loot_at(a, y, b).is_some(), "{} {a} {y} {b}", template.name);
            }

            if template.name.contains("full") || template.name.contains("with_mast") {
                let mut tables: Vec<&str> = chests.iter().filter_map(|&(a, y, b)| template.loot_at(a, y, b)).collect();

                tables.sort_unstable();
                assert_eq!(tables, ["shipwreck_map", "shipwreck_supply", "shipwreck_treasure"], "{}", template.name);
            }
        }
    }

    /// Дерево: дуб — основное, ель — второе.
    #[test]
    fn woods_are_swapped() {
        assert_eq!(rewood("oak_planks", ("dark_oak", "jungle")), "dark_oak_planks");
        assert_eq!(rewood("spruce_stairs[facing=east]", ("dark_oak", "jungle")), "jungle_stairs[facing=east]");
        assert_eq!(rewood("chest[facing=east]", ("dark_oak", "jungle")), "chest[facing=east]");
    }

    /// Вид по биому (вики): океаны — на дне, пляжи — на берегу.
    #[test]
    fn kinds_follow_the_biome() {
        assert_eq!(kind_of(Biome::DeepFrozenOcean), Some(false));
        assert_eq!(kind_of(Biome::WarmOcean), Some(false));
        assert_eq!(kind_of(Biome::SnowyBeach), Some(true));
        assert_eq!(kind_of(Biome::Plains), None);
        assert_eq!(kind_of(Biome::River), None);
    }

    /// Попытки: в своей области, сдвиг 0…19 (spacing 24 − separation 4).
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -5..5 {
            for region_z in -5..5 {
                let (x, z) = attempt(11, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }
    }

    /// Клад — в 1 % чанков (до проверки биома).
    #[test]
    fn treasure_share_is_one_percent() {
        let hits = (0..200).flat_map(|x| (0..200).map(move |z| (x, z))).filter(|&(x, z)| template::chance(5, x, 0, z, 0) < TREASURE_SHARE).count();

        assert!((300..500).contains(&hits), "{hits}");
    }

    /// Перепись кораблей нашего мира в рамках (как tools/measure/
    /// template_merge.py census shipwreck: дерево по виду блока) и сколько
    /// кладов в пляжах (`cargo test --release shipwreck_world -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn shipwreck_world() {
        use crate::world::{terrain, Chunk, Generator};

        const KINDS: [&str; 8] = ["planks", "stairs", "fence", "slab", "log", "trapdoor", "door", "chest"];
        let generator = Generator::Normal(terrain::Terrain::new(1_234_567_890, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let mut totals = [(0u32, [0u64; 8]); 2];

        for region_x in -10..10 {
            for region_z in -10..10 {
                let Some(ship) = start_in(terrain, region_x, region_z) else { continue };
                let entry = &mut totals[usize::from(ship.beached)];

                if entry.0 >= 30 {
                    continue;
                }

                entry.0 += 1;

                let (sx, sz) = ship.footprint();
                let (x0, y0, z0) = ship.origin;
                let height = library()[ship.template].height;

                for chunk_x in x0.div_euclid(16)..=(x0 + sx - 1).div_euclid(16) {
                    for chunk_z in z0.div_euclid(16)..=(z0 + sz - 1).div_euclid(16) {
                        let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                        for lx in 0..16 {
                            for lz in 0..16 {
                                let (x, z) = (chunk_x * 16 + lx, chunk_z * 16 + lz);

                                if !(x0..x0 + sx).contains(&x) || !(z0..z0 + sz).contains(&z) {
                                    continue;
                                }

                                for y in y0..y0 + height {
                                    let name = blocks::block_at_state(chunk.block(lx, y, lz)).unwrap_or("air");
                                    let kind = if name == "chest" { "chest" } else { name.rsplit('_').next().unwrap_or(name) };

                                    if let Some(at) = KINDS.iter().position(|known| *known == kind) {
                                        entry.1[at] += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for (beached, (count, sums)) in totals.iter().enumerate() {
            let line: Vec<String> = KINDS.iter().zip(sums).map(|(name, sum)| format!("{}={:.1}", name, *sum as f64 / f64::from((*count).max(1)))).collect();

            println!("{} {count} {}", if beached == 1 { "shipwreck_beached" } else { "shipwreck" }, line.join(" "));
        }

        let treasures = (-200..200).flat_map(|x| (-200..200).map(move |z| (x, z))).filter(|&(x, z)| treasure_in(terrain, x, z).is_some()).count();

        let beaches = (-200..200).flat_map(|x| (-200..200).map(move |z| (x, z))).filter(|&(x, z)| matches!(terrain.column_in(x * 16 + 9, z * 16 + 9, None).biome, terrain::Biome::Beach | terrain::Biome::SnowyBeach)).count();

        println!("кладов в 400×400 чанках: {treasures}, чанков пляжа {beaches}");
        assert!(totals[0].0 > 0);
    }

}

// vanilla-struct: beached BB Y59/61, верх берега вокруг рамки на 3–5 выше.
// Вики, Shipwreck: «in beaches»; точное распределение заглубления [проверить].
fn placement_height(surface:i32,height:i32,beached:bool,dice:u64) -> i32 {
    if beached {surface - (height/2-1).max(1) - ((dice>>32)%3) as i32} else {surface}
}

#[cfg(all(test, not(seedmap_core)))]
#[test]
fn beached_ship_is_partly_buried_and_preserves_its_loot() {
    assert_eq!(kind_of(super::terrain::Biome::Beach),Some(true));
    assert_eq!(kind_of(super::terrain::Biome::SnowyBeach),Some(true));
    for dice in [0,1_u64<<32,2_u64<<32] {
        let y=placement_height(64,9,true,dice);
        assert!((59..=61).contains(&y));
        assert!(y<64 && y+9>64);
        assert_eq!(placement_height(45,9,false,dice),45);
    }
    let i=library().iter().position(|t| t.name.contains("rightsideup_backhalf")).unwrap();
    let ship=Ship{template:i,orient:Orient::default(),origin:(-16,59,-16),woods:("oak","spruce"),beached:true,chunk:(-1,-1)};
    let mut chest_count=0;
    let (sx,sz)=ship.footprint();
    for cx in -1..=(-16+sx-1).div_euclid(16) { for cz in -1..=(-16+sz-1).div_euclid(16) {
        let mut c=super::Chunk::new();
        paint(1,&ship,&mut c,cx,cz);
        chest_count+=c.containers.len();
        assert!(c.containers.values().all(|c|c.slots.iter().any(Option::is_some)));
        let back=super::Chunk::from_bytes(&c.to_bytes()).unwrap();
        assert_eq!(back.containers.len(),c.containers.len());
    }}
    assert!(chest_count>0);
}

#[cfg(all(test, not(seedmap_core)))]
mod r2_tests {
    use super::*;

    #[test]
    fn measured_ship_templates_rotations_and_pivots() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/genaudit-26.3/overworld-pieces-r2.json")).unwrap();
        let rows = fixture["ships"].as_array().unwrap();
        assert_eq!(rows.len(), 107);
        for row in rows {
            let seed = row["seed"].as_i64().unwrap();
            let x = row["chunk"][0].as_i64().unwrap() as i32;
            let z = row["chunk"][1].as_i64().unwrap() as i32;
            let (index, orient) = selection(seed, x, z, row["beached"].as_bool().unwrap());
            let t = &library()[index];
            assert_eq!(t.name, row["template"].as_str().unwrap(), "{seed} {x}/{z}");
            assert_eq!(i64::from(orient.turns), row["turns"].as_i64().unwrap());
            let corner = frame_corner(x * 16, z * 16, t, orient.turns);
            assert_eq!(corner, (row["box"][0].as_i64().unwrap() as i32, row["box"][2].as_i64().unwrap() as i32));
            let (sx, sz) = t.footprint(orient);
            assert_eq!(i64::from(sx), row["box"][3].as_i64().unwrap() - row["box"][0].as_i64().unwrap() + 1);
            assert_eq!(i64::from(sz), row["box"][5].as_i64().unwrap() - row["box"][2].as_i64().unwrap() + 1);
        }
    }

    /// Проверка постановки в настоящем чанке, а не только выбора плана.
    #[test]
    fn rotated_ship_is_present_in_its_persisted_frame() {
        use super::super::{Chunk, Generator, terrain::{Terrain, Style}};
        let terrain = Terrain::new(1_792_932_773_345_722_119, Style::Vanilla).with_preview(true);
        let ship = start_in(&terrain, (-31i32).div_euclid(SPACING), 49i32.div_euclid(SPACING)).unwrap();
        assert_eq!(ship.chunk, (-31, 49));
        assert!(library()[ship.template].name.ends_with("sideways_backhalf"));
        let generator = Generator::Normal(terrain);
        let (sx, sz) = ship.footprint();
        let mut wood = 0;
        for cx in ship.origin.0.div_euclid(16)..=(ship.origin.0 + sx - 1).div_euclid(16) {
            for cz in ship.origin.2.div_euclid(16)..=(ship.origin.2 + sz - 1).div_euclid(16) {
                let chunk = Chunk::generated(&generator, cx, cz);
                for y in ship.origin.1..ship.origin.1 + library()[ship.template].height {
                    for z in 0..16 { for x in 0..16 {
                        if blocks::block_at_state(chunk.block(x,y,z)).is_some_and(|n| n.ends_with("_planks")) { wood += 1; }
                    }}
                }
            }
        }
        assert!(wood > 100, "{wood}");
    }
}

#[cfg(all(test, not(seedmap_core)))]
#[test]
fn ocean_ship_height_uses_the_unrotated_first_free_mean() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/genaudit-26.3/overworld-pieces-r3.json")).unwrap();
    for row in fixture["ship_ground_controls"].as_array().unwrap() {
        let x = row["position"][0].as_i64().unwrap() as i32;
        let z = row["position"][1].as_i64().unwrap() as i32;
        let width = row["size"][0].as_i64().unwrap() as i32;
        let length = row["size"][1].as_i64().unwrap() as i32;
        let bytes: Vec<_> = row["ground_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
            .map(|s| i32::from(u8::from_str_radix(std::str::from_utf8(s).unwrap(), 16).unwrap()) - 64).collect();
        assert_eq!(bytes.len(), (width * length) as usize);
        let height = ocean_height(x, z, width, length, &|wx, wz| bytes[((wx - x) * length + wz - z) as usize]);
        assert_eq!(i64::from(height), row["expected_y"].as_i64().unwrap(), "{row}");
    }
}
