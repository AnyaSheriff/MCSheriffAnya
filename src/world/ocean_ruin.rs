// Руины в океане (ocean ruins): холодные (каменный кирпич) и тёплые
// (песчаник) — где стоят и чем сложены.
//
// Где — по вики (Structure set «ocean_ruins»: spacing 20, separation 8;
// Cold/Warm Ocean Ruins, «Generation»): мир поделён на области 20×20
// чанков, в каждой одна попытка — чанк со сдвигом 0…11. Тёплые — в тёплом,
// умеренно тёплом и глубоком умеренно тёплом океане, холодные — в обычном,
// холодном и замёрзшем (и глубоких). 30 % — большие; у большой в 90 % вокруг
// ещё 4…8 малых («cluster»).
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/template_merge.py ruins,
// src/world/ocean_ruin.txt): руина — все её куски в одной рамке (у холодных
// три куска brick, cracked, mossy с целостью 0,8/0,9, 0,7, 0,5 — записи о
// строении), у клетки — блок большинства и доля экземпляров, где он стоит:
// при постановке клетка кладётся с этой долей. Воздуха руины не кладут
// (вики: «all air blocks … act as structure voids»). Гравий холодных и песок
// тёплых — подозрительные с долей замера (16 % и 9 % своих) [проверить: у
// нас нет кисти; блок-сущность без таблицы]. Сундуки — таблицы по месту
// (underwater_ruin_big/small), утопленники — по местам замера (сущности мира
// оригинала в рамках руин; ставит их такт мобов при сложении чанка).
//
// Высота (замер TPY против дна вокруг, 150 руин): малая — на дне (низ рамки
// на 1 выше верха дна), большая — на 1 ниже. Малые вокруг большой (замер 15
// больших): середины на 12…26 блоков от середины большой по большей оси.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::template::{self, Cell, Orient, Template};
use crate::blocks;

pub const SPACING: i32 = 20;
pub const SEPARATION: i32 = 8;
const SALT: u64 = 14_357_621;

/// Доля больших и доля скоплений у больших (вики).
const BIG_SHARE: f64 = 0.3;
const CLUSTER_SHARE: f64 = 0.9;

/// Подозрительные среди своего гравия (холодные) и песка (тёплые) — замер.
const SUSPICIOUS_COLD: f64 = 0.16;
const SUSPICIOUS_WARM: f64 = 0.09;

pub fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| template::parse(include_str!("ocean_ruin.txt")))
}

/// Тёплые ли руины для биома (None — руин нет).
pub fn warm_of(biome: super::terrain::Biome) -> Option<bool> {
    use super::terrain::Biome;

    match biome {
        Biome::WarmOcean | Biome::LukewarmOcean | Biome::DeepLukewarmOcean => Some(true),
        Biome::Ocean | Biome::DeepOcean | Biome::ColdOcean | Biome::DeepColdOcean | Biome::FrozenOcean | Biome::DeepFrozenOcean => Some(false),
        _ => None,
    }
}

/// Одна руина: образец, поворот, угол рамки.
#[derive(Clone, Copy, Debug)]
pub struct Ruin {
    pub template: usize,
    pub orient: Orient,
    pub origin: (i32, i32, i32),
}

impl Ruin {
    pub fn footprint(&self) -> (i32, i32) {
        library()[self.template].footprint(self.orient)
    }
}

/// Руины области: чанк старта, тёплые ли, руины (первая — главная).
#[derive(Clone, Debug)]
pub struct Site {
    pub chunk: (i32, i32),
    pub warm: bool,
    pub ruins: Vec<Ruin>,
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


/// Образцы вида: (тёплые ли, большие ли) → номера в библиотеке.
fn kinds(warm: bool, big: bool) -> Vec<usize> {
    let prefix = format!("{}_{}_", if warm { "warm" } else { "cold" }, if big { "big" } else { "small" });

    (0..library().len()).filter(|&at| library()[at].name.starts_with(&prefix)).collect()
}

/// Руины области обычного мира.
pub fn site_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Arc<Site>> {
    type Sites = Mutex<HashMap<(i64, u8, i32, i32), Option<Arc<Site>>>>;
    static CACHE: OnceLock<Sites> = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (terrain.seed(), terrain.generation_profile(), region_x, region_z);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let found = decide(terrain, region_x, region_z).map(Arc::new);

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found.clone());
    found
}

/// Высота дна под рамкой: срединный верх дна по рамке.
fn floor(terrain: &super::terrain::Terrain, x: i32, z: i32, sx: i32, sz: i32) -> i32 {
    let mut heights: Vec<i32> = [(0, 0), (sx - 1, 0), (0, sz - 1), (sx - 1, sz - 1), (sx / 2, sz / 2)].iter().map(|&(dx, dz)| terrain.column_at(x + dx, z + dz).height).collect();

    heights.sort_unstable();
    heights[heights.len() / 2]
}

/// minecraft.wiki/w/Ocean_Ruins: «clusters». Кандидаты относительно нижнего
/// угла большой рамки 16×16, порядок и поток сняты с 14 естественных начал.
/// Отсев пересечения использует пробную рамку 6×7 до выбора малого образца;
/// три дополнительных начала различают её с соседними размерами 5×6…8×7.
fn cluster_plans(random: &mut template::StructureRandom, warm: bool, x: i32, z: i32) -> Vec<(usize, Orient, i32, i32)> {
    let mut positions = vec![
        (random.below(8) - 15, random.below(7) + 17),
        (random.below(8) - 15, random.below(7) + 1),
        (random.below(8) - 15, random.below(5) - 12),
        (random.below(7) + 1, random.below(7) + 17),
        (random.below(7) + 1, random.below(3) - 12),
        (random.below(7) + 17, random.below(6) + 19),
        (random.below(7) + 17, random.below(7) + 1),
        (random.below(7) + 17, random.below(5) - 12),
    ];
    let count = 4 + random.below(5);
    let pool = kinds(warm, false);
    let mut out = Vec::new();
    for _ in 0..count {
        let index = random.below(positions.len() as i32) as usize;
        let (px, pz) = positions.remove(index);
        let turns = random.below(4) as u8;
        let (dx, dz, sx, sz) = match turns {
            0 => (0, 0, 6, 7),
            1 => (-6, 0, 7, 6),
            2 => (-5, -6, 6, 7),
            _ => (0, -5, 7, 6),
        };
        if px + dx <= 15 && px + dx + sx > 0 && pz + dz <= 15 && pz + dz + sz > 0 { continue; }
        let template = pool[random.below(pool.len() as i32) as usize];
        out.push((template, Orient { turns, mirror: false }, x + px, z + pz));
    }
    out
}

fn decide(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Site> {
    let seed = terrain.seed();
    let (chunk_x, chunk_z) = attempt_for(terrain, region_x, region_z);
    let (x, z) = (chunk_x * 16, chunk_z * 16);
    let warm = warm_of(terrain.column_in(x + 8, z + 8, None).biome)?;
    let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x0C3A);
    let unit = |shift: u32| ((dice >> shift) & 0xFFFF) as f64 / 65536.0;
    let preview = terrain.generation_profile() & 2 != 0;
    // minecraft.wiki/w/Ocean_Ruins: «30%»; последовательность вращение,
    // размер, образец подтверждена 57 сохранёнными началами 26.3.
    let mut random = template::StructureRandom::for_chunk(seed, chunk_x, chunk_z);
    let rotation = random.below(4) as u8;
    let big = if preview { f64::from(random.float()) < BIG_SHARE } else { unit(0) < BIG_SHARE };
    let pool = kinds(warm, big);

    if pool.is_empty() {
        return None;
    }

    let place = |template: usize, orient: Orient, x: i32, z: i32, big: bool| {
        let (sx, sz) = library()[template].footprint(orient);
        // Сохранённый TP — точка поворота (0, 0), а origin — низ рамки.
        let (x, z) = if preview {
            match orient.turns % 4 {
                0 => (x, z),
                1 => (x - sx + 1, z),
                2 => (x - sx + 1, z - sz + 1),
                _ => (x, z - sz + 1),
            }
        } else { (x, z) };
        let y = floor(terrain, x, z, sx, sz) + if big { -1 } else { 1 };

        Ruin { template, orient, origin: (x, y, z) }
    };
    let index = if preview { random.below(pool.len() as i32) as usize } else { ((dice >> 16) % pool.len() as u64) as usize };
    let turns = if preview { rotation } else { ((dice >> 24) % 4) as u8 };
    let main = place(pool[index], Orient { turns, mirror: false }, x, z, big);
    let mut ruins = vec![main];

    // Скопление малых вокруг большой: 4…8, середины на 12…26 блоков от
    // середины большой по большей оси (замер).
    let clustered = if preview { f64::from(random.float()) < CLUSTER_SHARE } else { unit(32) < CLUSTER_SHARE };
    if big && clustered && preview {
        for (template, orient, x, z) in cluster_plans(&mut random, warm, main.origin.0, main.origin.2) {
            ruins.push(place(template, orient, x, z, false));
        }
    } else if big && clustered {
        let small = kinds(warm, false);
        let (msx, msz) = main.footprint();
        let (cx, cz) = (x + msx / 2, z + msz / 2);
        let count = 4 + (dice >> 48) % 5;

        for at in 0..count {
            let roll = template::roll(seed, chunk_x, chunk_z, SALT ^ (0x51_0000 + at));
            let template = small[(roll % small.len().max(1) as u64) as usize];
            let orient = Orient { turns: ((roll >> 8) % 4) as u8, mirror: false };
            let (sx, sz) = library()[template].footprint(orient);
            let far = 12 + ((roll >> 16) % 15) as i32;
            let side = ((roll >> 24) % 53) as i32 - 26;
            let (dx, dz) = match (roll >> 32) % 4 {
                0 => (far, side),
                1 => (-far, side),
                2 => (side, far),
                _ => (side, -far),
            };

            ruins.push(place(template, orient, cx + dx - sx / 2, cz + dz - sz / 2, false));
        }
    }

    Some(Site { chunk: (chunk_x, chunk_z), warm, ruins })
}

fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text).or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text))).unwrap_or(super::AIR)
}

/// Таблица добычи по имени из образца.
#[cfg(not(seedmap_core))]
fn table(name: &str) -> &'static [crate::loot::Pool] {
    if name == "underwater_ruin_big" { &crate::loot::UNDERWATER_RUIN_BIG } else { &crate::loot::UNDERWATER_RUIN_SMALL }
}

/// Площадки, чьи руины могут задеть этот чанк (скопление — до 3 чанков).
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<Arc<Site>> {
    let mut out = Vec::new();

    for region_x in (chunk_x - 4).div_euclid(SPACING)..=(chunk_x + 4).div_euclid(SPACING) {
        for region_z in (chunk_z - 4).div_euclid(SPACING)..=(chunk_z + 4).div_euclid(SPACING) {
            let (x, z) = attempt_for(terrain, region_x, region_z);

            if (x - chunk_x).abs() > 4 || (z - chunk_z).abs() > 4 {
                continue;
            }

            if let Some(site) = site_in(terrain, region_x, region_z) {
                out.push(site);
            }
        }
    }

    out
}

#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    for site in near(terrain, chunk_x, chunk_z) {
        for ruin in &site.ruins {
            paint(terrain.seed(), site.warm, ruin, chunk, chunk_x, chunk_z);
        }
    }
}

#[cfg(not(seedmap_core))]
fn paint(seed: i64, warm: bool, ruin: &Ruin, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let template = &library()[ruin.template];
    let (sx, sz) = template.footprint(ruin.orient);
    let (x0, y0, z0) = ruin.origin;
    let (cx0, cz0) = (chunk_x * 16, chunk_z * 16);

    if x0 > cx0 + 15 || x0 + sx <= cx0 || z0 > cz0 + 15 || z0 + sz <= cz0 {
        return;
    }

    for (a, y, b, cell) in template.cells() {
        let Cell::Block(index) = cell else {
            continue;
        };
        let (dx, dz) = ruin.orient.offset(a, b, template.width, template.length);
        let (x, wy, z) = (x0 + dx, y0 + y, z0 + dz);

        if x < cx0 || x > cx0 + 15 || z < cz0 || z > cz0 + 15 || !(super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT).contains(&wy) {
            continue;
        }

        // Неполная целость: клетка — с долей замера.
        if template::chance(seed, x, wy, z, 0x0E5A) >= template.chance[usize::from(index)] {
            continue;
        }

        let (lx, lz) = (x - cx0, z - cz0);
        let mut text = template::orient_text(&template.texts[usize::from(index)], ruin.orient);
        let suspicious = if warm { SUSPICIOUS_WARM } else { SUSPICIOUS_COLD };

        if (text == "gravel" && !warm || text == "sand" && warm) && template::chance(seed, x, wy, z, 0x0E5B) < suspicious {
            text = format!("suspicious_{}", text);
        }

        let wet = blocks::block_at_state(chunk.block(lx, wy, lz)) == Some("water") && text.contains("waterlogged=false");

        chunk.put_generated(lx, wy, lz, state_of(&if wet { text.replace("waterlogged=false", "waterlogged=true") } else { text.clone() }), true);

        if text.starts_with("suspicious_") {
            chunk.archaeology.insert((lx,wy,lz),crate::archaeology::generated(if warm {"ocean_ruin_warm"} else {"ocean_ruin_cold"},template::roll(seed,x,z,0xA2C0 ^ wy as u64)));
        }
        if text.starts_with("chest") {
            let mut dice = crate::loot::Dice::new(template::roll(seed, x, z, 0x0E5C ^ (wy as u64)));
            let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

            chest.slots = crate::loot::fill(table(template.loot_at(a, y, b).unwrap_or("underwater_ruin_small")), chest.slots.len(), &mut dice);
            chunk.put_generated_container(lx, wy, lz, chest);
        }
    }
}

/// Утопленники руин, стоящие в чанке (их ставит такт мобов при сложении):
/// места замера, каждый — с долей замера.
pub fn drowned(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();

    for site in near(terrain, chunk_x, chunk_z) {
        for ruin in &site.ruins {
            let template = &library()[ruin.template];
            let (x0, y0, z0) = ruin.origin;

            for &((a, y, b), share) in &template.mobs {
                let (dx, dz) = ruin.orient.offset(a.clamp(0, template.width - 1), b.clamp(0, template.length - 1), template.width, template.length);
                let (x, wy, z) = (x0 + dx, y0 + y, z0 + dz);

                if x.div_euclid(16) != chunk_x || z.div_euclid(16) != chunk_z || template::chance(terrain.seed(), x, wy, z, 0x0E5D) >= share {
                    continue;
                }

                out.push((f64::from(x) + 0.5, f64::from(wy), f64::from(z) + 0.5));
            }
        }
    }

    out
}

/// Ближайшие руины вида (`locate structure ocean_ruin_cold/warm`).
pub fn nearest(terrain: &super::terrain::Terrain, warm: bool, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| site_in(terrain, region_x, region_z).filter(|site| site.warm == warm).map(|site| site.chunk))
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;
    use crate::world::terrain::Biome;

    /// Образцы: малых по 8 каждого вида, больших холодных 4, тёплых — что
    /// снято; у каждой клетки доля от 0,3 до 1.
    #[test]
    fn templates_parse() {
        assert_eq!(kinds(false, false).len(), 8);
        assert_eq!(kinds(true, false).len(), 8);
        assert_eq!(kinds(false, true).len(), 4);
        assert_eq!(kinds(true, true).len(), 4);

        for template in library() {
            assert!(template.chance.iter().all(|share| (0.3..=1.0).contains(share)), "{}", template.name);
        }
    }

    /// Вид по биому (вики).
    #[test]
    fn kinds_follow_the_biome() {
        assert_eq!(warm_of(Biome::WarmOcean), Some(true));
        assert_eq!(warm_of(Biome::DeepLukewarmOcean), Some(true));
        assert_eq!(warm_of(Biome::DeepFrozenOcean), Some(false));
        assert_eq!(warm_of(Biome::Ocean), Some(false));
        assert_eq!(warm_of(Biome::Beach), None);
    }

    /// Попытки: в своей области, сдвиг 0…11 (spacing 20 − separation 8).
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -5..5 {
            for region_z in -5..5 {
                let (x, z) = attempt(13, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }
    }

    /// Руины нашего мира: доля больших ~30 %, у больших — 4…8 малых в 90 %,
    /// перепись своих блоков (`cargo test --release ocean_ruin_world --
    /// --ignored --nocapture`, сверка с template_merge.py census).
    #[test]
    #[ignore]
    fn ocean_ruin_world() {
        use crate::world::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(1_234_567_890, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let (mut sites, mut big, mut clustered) = (0, 0, 0);
        let names = ["stone_bricks", "cracked_stone_bricks", "mossy_stone_bricks", "gravel", "suspicious_gravel", "sandstone", "sand", "suspicious_sand", "chest"];
        let mut counts = [[0u64; 9]; 2];
        let mut ruins = [0u32; 2];

        for region_x in -12..12 {
            for region_z in -12..12 {
                let Some(site) = site_in(terrain, region_x, region_z) else { continue };

                sites += 1;

                if library()[site.ruins[0].template].name.contains("_big_") {
                    big += 1;
                    clustered += usize::from(site.ruins.len() > 1);
                }

                if ruins[usize::from(site.warm)] >= 25 {
                    continue;
                }

                for ruin in &site.ruins {
                    ruins[usize::from(site.warm)] += 1;

                    let (sx, sz) = ruin.footprint();
                    let (x0, y0, z0) = ruin.origin;
                    let height = library()[ruin.template].height;

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

                                        if let Some(at) = names.iter().position(|known| *known == name) {
                                            counts[usize::from(site.warm)][at] += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        println!("площадок {sites}, больших {big}, со скоплением {clustered}");

        for warm in [false, true] {
            let n = f64::from(ruins[usize::from(warm)].max(1));
            let line: Vec<String> = names.iter().zip(counts[usize::from(warm)]).map(|(name, sum)| format!("{}={:.1}", name, sum as f64 / n)).collect();

            println!("{} руин {}: {}", if warm { "тёплые" } else { "холодные" }, ruins[usize::from(warm)], line.join(" "));
        }

        assert!(sites > 0);
    }
}

#[cfg(all(test, not(seedmap_core)))]
#[test]
fn measured_primary_ruin_variants_and_frames() {
    let f: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/genaudit-26.3/overworld-pieces-r2.json")).unwrap();
    let mut checked = 0;
    for row in f["ruins"].as_array().unwrap() {
        if !matches!(row["label"].as_str().unwrap(), "owner" | "second") { continue; }
        let t = super::terrain::Terrain::new(row["seed"].as_i64().unwrap(), super::terrain::Style::Vanilla).with_preview(true);
        let x = row["chunk"][0].as_i64().unwrap() as i32;
        let z = row["chunk"][1].as_i64().unwrap() as i32;
        let site = site_in(&t, x.div_euclid(SPACING), z.div_euclid(SPACING)).unwrap();
        let ruin = &site.ruins[0];
        assert_eq!(library()[ruin.template].name, row["template"].as_str().unwrap());
        assert_eq!(i64::from(ruin.orient.turns), row["turns"].as_i64().unwrap());
        assert_eq!((ruin.origin.0, ruin.origin.2), (row["box"][0].as_i64().unwrap() as i32, row["box"][2].as_i64().unwrap() as i32));
        checked += 1;
    }
    assert_eq!(checked, 48);
}

#[cfg(all(test, not(seedmap_core)))]
#[test]
fn measured_ruin_clusters_keep_candidate_order_and_overlap_rejection() {
    let f: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/genaudit-26.3/overworld-pieces-r2.json")).unwrap();
    let rows = f["clusters"].as_array().unwrap();
    assert_eq!(rows.len(), 14);
    for row in rows {
        let seed = row["seed"].as_i64().unwrap();
        let x = row["chunk"][0].as_i64().unwrap() as i32;
        let z = row["chunk"][1].as_i64().unwrap() as i32;
        let mut random = template::StructureRandom::for_chunk(seed, x, z);
        random.below(4);
        assert!(random.float() < BIG_SHARE as f32);
        random.below(4);
        assert!(random.float() < CLUSTER_SHARE as f32);
        let plans = cluster_plans(&mut random, row["warm"].as_bool().unwrap(),
            row["corner"][0].as_i64().unwrap() as i32, row["corner"][1].as_i64().unwrap() as i32);
        let measured = row["parts"].as_array().unwrap();
        assert_eq!(plans.len(), measured.len(), "{seed} {x}/{z}");
        for ((template, orient, px, pz), part) in plans.iter().zip(measured) {
            assert_eq!(library()[*template].name, part["template"].as_str().unwrap());
            assert_eq!(i64::from(orient.turns), part["turns"].as_i64().unwrap());
            assert_eq!((*px, *pz), (part["point"][0].as_i64().unwrap() as i32, part["point"][1].as_i64().unwrap() as i32));
        }
    }
}
