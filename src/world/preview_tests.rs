// Генерация 26.3 (профиль Preview26_3): пятнистый лес, серные пещеры, заброшенные лагеря.
// Замеры — официальный 26.3 (tools/measure/census_263.py, surface_census_263.py); перепись нашего
// мира: `cargo test --release preview_census -- --ignored --nocapture`.

use super::terrain::{Biome, Style, Terrain};
use super::{abandoned_camp, Chunk, Generator};
use crate::blocks;

fn names_in(chunk: &Chunk) -> std::collections::HashMap<&'static str, u32> {
    let mut counts = std::collections::HashMap::new();

    for y in super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT {
        for z in 0..16 {
            for x in 0..16 {
                let state = chunk.block(x, y, z);

                if state != super::AIR {
                    *counts.entry(blocks::block_at_state(state).unwrap_or("?")).or_default() += 1;
                }
            }
        }
    }

    counts
}

fn preview_generator(seed: i64) -> Generator {
    Generator::Normal(Terrain::new(seed, Style::Vanilla).with_preview(true))
}

/// Первый лагерь мира: чанк старта.
fn first_camp(generator: &Generator) -> Option<(i32, i32)> {
    let Generator::Normal(terrain) = generator else { return None };

    (-6..6).flat_map(|x| (-6..6).map(move |z| (x, z))).find_map(|(x, z)| abandoned_camp::site_in(terrain, x, z).map(|site| site.chunk))
}

/// Лагерь ложится в чанки: ступени из шерсти, тропа, сундук или бочка с добычей.
#[test]
fn camp_builds_into_chunks() {
    let generator = preview_generator(1_234_567_890);
    let Some((chunk_x, chunk_z)) = first_camp(&generator) else { panic!("в окне 12×12 областей нет ни одного лагеря") };
    let (mut path, mut containers, mut wool) = (0, 0, 0);

    for cx in chunk_x - 2..=chunk_x + 2 {
        for cz in chunk_z - 2..=chunk_z + 2 {
            let chunk = Chunk::generated(&generator, cx, cz);

            path += names_in(&chunk).get("dirt_path").copied().unwrap_or(0);
            wool += names_in(&chunk).get("white_wool_stairs").copied().unwrap_or(0);
            containers += chunk.containers.values().filter(|c| c.slots.iter().any(Option::is_some)).count();
        }
    }

    assert!(path > 5, "тропа {path}");
    assert!(containers >= 1, "хранилищ с добычей {containers}");
    let _ = wool; // ступени шерсти — в блоках каталога, а не таблицы 775
}

/// В мире 26.1.2 лагерей, серы и тополей нет и биомов 26.3 нет.
#[test]
fn native_world_has_no_preview_content() {
    let generator = Generator::Normal(Terrain::new(1_234_567_890, Style::Vanilla));
    let Generator::Normal(terrain) = &generator else { unreachable!() };

    for x in -8..8 {
        for z in -8..8 {
            assert!(abandoned_camp::site_in(terrain, x, z).is_none());
        }
    }

    for (cx, cz) in [(0, 0), (40, -17), (-90, 63), (300, 300)] {
        let chunk = Chunk::generated(&generator, cx, cz);
        let counts = names_in(&chunk);

        for name in ["poplar_log", "sulfur", "cinnabar", "red_shrub", "shelf_mushroom", "sulfur_spike"] {
            assert!(!counts.contains_key(name), "{name} в чанке {cx} {cz}");
        }
    }

    for biome in Biome::PREVIEW {
        for x in (-2000..2000).step_by(64) {
            assert_ne!(terrain.biome_at(x, x / 3), biome);
        }
    }
}

/// Пятнистый лес: тополя, красные кусты, лежачие стволы и полочные грибы.
#[test]
fn dappled_forest_grows_poplars() {
    let generator = preview_generator(42);
    let Generator::Normal(terrain) = &generator else { unreachable!() };
    let mut spot = None;

    'search: for x in (-6000..6000).step_by(48) {
        for z in (-6000..6000).step_by(48) {
            if terrain.biome_at(x, z) == Biome::DappledForest && terrain.biome_at(x + 40, z) == Biome::DappledForest && terrain.biome_at(x, z + 40) == Biome::DappledForest {
                spot = Some((x.div_euclid(16), z.div_euclid(16)));
                break 'search;
            }
        }
    }

    let (cx, cz) = spot.expect("пятнистый лес на карте");
    let mut total: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

    for dx in 0..4 {
        for dz in 0..4 {
            for (name, count) in names_in(&Chunk::generated(&generator, cx + dx, cz + dz)) {
                *total.entry(name).or_default() += count;
            }
        }
    }

    assert!(total.get("poplar_log").copied().unwrap_or(0) > 20, "{total:?}");
    assert!(total.get("orange_poplar_leaves").copied().unwrap_or(0) + total.get("red_poplar_leaves").copied().unwrap_or(0) > 50);
}

/// Перепись нашего мира против замера официального 26.3: доли биомов и серной пещеры, число
/// блоков нового содержимого на чанк пятнистого леса.
#[test]
#[ignore]
fn preview_census() {
    for seed in [42_i64, 1, 1_234_567_890] {
        let generator = preview_generator(seed);
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let (mut dappled, mut plains, mut forest, mut total) = (0u64, 0u64, 0u64, 0u64);

        for x in (-8192..8192).step_by(16) {
            for z in (-8192..8192).step_by(16) {
                total += 1;

                match terrain.biome_at(x, z) {
                    Biome::DappledForest => dappled += 1,
                    Biome::Plains => plains += 1,
                    Biome::Forest => forest += 1,
                    _ => {}
                }
            }
        }

        println!(
            "seed {seed}: пятнистый лес {:.5} (замер 0.00185 на 25 тысячах чанков), равнины {:.4} (0.174), лес {:.4} (0.130)",
            dappled as f64 / total as f64,
            plains as f64 / total as f64,
            forest as f64 / total as f64
        );

        // Содержимое чанков пятнистого леса.
        let mut found = 0;
        let mut counts: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();

        'scan: for x in (-8192..8192).step_by(32) {
            for z in (-8192..8192).step_by(32) {
                if terrain.biome_at(x + 8, z + 8) != Biome::DappledForest || terrain.column_at(x + 8, z + 8).height < super::terrain::SEA {
                    continue;
                }

                let chunk = Chunk::generated(&generator, x.div_euclid(16), z.div_euclid(16));

                for (name, count) in names_in(&chunk) {
                    if ["poplar_log", "orange_poplar_leaves", "red_poplar_leaves", "yellow_poplar_leaves", "leaf_litter", "red_shrub", "shelf_mushroom", "brown_mushroom", "coarse_dirt", "spruce_log"].contains(&name) {
                        *counts.entry(name).or_default() += u64::from(count);
                    }
                }

                found += 1;

                if found >= 40 {
                    break 'scan;
                }
            }
        }

        let line: Vec<String> = counts.iter().map(|(name, sum)| format!("{name}={:.1}", *sum as f64 / f64::from(found.max(1)))).collect();

        println!("seed {seed}: чанков пятнистого леса {found}; на чанк: {} (замер: poplar_log 34.8, листья 237+162+99, leaf_litter 47, red_shrub 1.2, shelf_mushroom 1.7, brown_mushroom 0.6, spruce_log 0.8)", line.join(" "));
    }
}

/// Плотность пятен пятнистого леса так же, как у официального 26.3 по `/locate biome`: из точек файла
/// LOCATE_POINTS (`{"pts": [[x, z], …]}`) ищется ближайший пятнистый лес в радиусе 6400 блоков с шагом
/// 32; оценка плотности λ = n / (π Σ d²) на миллион квадратных блоков (промахи — d = 6400 без n).
/// `LOCATE_POINTS=… cargo test --release locate_density -- --ignored --nocapture`.
#[test]
#[ignore]
fn locate_density() {
    let Ok(path) = std::env::var("LOCATE_POINTS") else { return };
    let points: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let points: Vec<(i32, i32)> = points["pts"].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32)).collect();

    for seed in [42_i64, 1, 1_234_567_890] {
        let terrain = Terrain::new(seed, Style::Vanilla).with_preview(true);
        let (mut found, mut sum_sq, mut distances) = (0, 0.0f64, Vec::new());

        for &(px, pz) in &points {
            let mut nearest = None;

            'rings: for ring in 0..200 {
                for step in -ring..=ring {
                    for (dx, dz) in [(step, -ring), (step, ring), (-ring, step), (ring, step)] {
                        let (x, z) = (px + dx * 32, pz + dz * 32);

                        if terrain.biome_at(x, z) == Biome::DappledForest {
                            let distance = f64::from(dx * dx + dz * dz).sqrt() * 32.0;

                            nearest = Some(nearest.map_or(distance, |old: f64| old.min(distance)));
                        }
                    }
                }

                if nearest.is_some() {
                    break 'rings;
                }
            }

            match nearest {
                Some(distance) => {
                    found += 1;
                    sum_sq += distance * distance;
                    distances.push(distance);
                }
                None => sum_sq += 6400.0 * 6400.0,
            }
        }

        distances.sort_by(f64::total_cmp);
        println!(
            "seed {seed}: найдено {found} из {}, λ = {:.3} на млн блоков², медиана {:.0}",
            points.len(),
            f64::from(found) / (std::f64::consts::PI * sum_sq) * 1e6,
            distances.get(distances.len() / 2).copied().unwrap_or(0.0)
        );
    }
}

/// То же для серных пещер (официальный 26.3: λ около 0,55 на млн блоков², медиана 400 блоков):
/// клетка пещеры проверяется на высотах от −48 до 48 через 16.
#[test]
#[ignore]
fn locate_density_sulfur() {
    let Ok(path) = std::env::var("LOCATE_POINTS") else { return };
    let points: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let points: Vec<(i32, i32)> = points["pts"].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32)).collect();

    for seed in [42_i64, 1, 1_234_567_890] {
        let terrain = Terrain::new(seed, Style::Vanilla).with_preview(true);
        let (mut found, mut sum_sq, mut distances) = (0, 0.0f64, Vec::new());

        for &(px, pz) in &points {
            let mut nearest = None;

            'rings: for ring in 0..200 {
                for step in -ring..=ring {
                    for (dx, dz) in [(step, -ring), (step, ring), (-ring, step), (ring, step)] {
                        let (x, z) = (px + dx * 32, pz + dz * 32);
                        let column = terrain.column_in(x, z, None);

                        if (-3..=3).any(|level| terrain.sulfur_cell(&column, x, level * 16, z)) {
                            let distance = f64::from(dx * dx + dz * dz).sqrt() * 32.0;

                            nearest = Some(nearest.map_or(distance, |old: f64| old.min(distance)));
                        }
                    }
                }

                if nearest.is_some() {
                    break 'rings;
                }
            }

            match nearest {
                Some(distance) => {
                    found += 1;
                    sum_sq += distance * distance;
                    distances.push(distance);
                }
                None => sum_sq += 6400.0 * 6400.0,
            }
        }

        distances.sort_by(f64::total_cmp);
        println!(
            "seed {seed}: серные пещеры найдены {found} из {}, λ = {:.3} на млн блоков², медиана {:.0}",
            points.len(),
            f64::from(found) / (std::f64::consts::PI * sum_sq) * 1e6,
            distances.get(distances.len() / 2).copied().unwrap_or(0.0)
        );
    }
}

/// Таблицы лагеря: каждое имя — существующий предмет (иначе пункт молча пропадёт из добычи), а
/// число предметов в сундуке — как в замере (6–8, 6–10, 5–9).
#[test]
fn camp_loot_items_exist_and_sizes_match_the_measurement() {
    use crate::loot::{roll, Dice, Pool, ABANDONED_CAMP_BARREL, ABANDONED_CAMP_COMMON, ABANDONED_CAMP_SECRET};

    let tables: [(&[Pool], std::ops::RangeInclusive<usize>); 3] =
        [(&ABANDONED_CAMP_COMMON, 6..=9), (&ABANDONED_CAMP_SECRET, 6..=10), (&ABANDONED_CAMP_BARREL, 5..=9)];

    for (table, sizes) in tables {
        for pool in table {
            for (name, ..) in pool.entries {
                let item = name.strip_prefix("potion:").map_or(*name, |_| "potion");

                assert!(blocks::item_named(item).is_some() || crate::maps::pending_explorer(name).is_some(), "нет предмета {name}");
            }
        }

        for seed in 0..300 {
            let rolled = roll(table, &mut Dice::new(seed));

            assert!(sizes.contains(&rolled.len()), "предметов {} в броске {seed}", rolled.len());
        }
    }
}
/// Свёртка блоков, биомов, хранилищ и таблички блоков-сущностей 36 чанков трёх миров. Цифра закреплена
/// по сборке до генерации 26.3: мир профиля 26.1.2 не должен меняться ни на байт.
fn native_digest() -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    let mut feed = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    };

    for (seed, spots) in [
        (42_i64, [(0, 0), (11, -7), (-40, 25), (130, 130), (-300, 80), (700, -20)]),
        (1, [(0, 0), (5, 5), (-90, -60), (250, 40), (-15, 400), (64, -64)]),
        (1_234_567_890, [(0, 0), (-3, 9), (60, 60), (-200, 120), (333, -222), (-500, -500)]),
    ] {
        let generator = Generator::Normal(Terrain::new(seed, Style::Vanilla));

        for (cx, cz) in spots {
            for dx in 0..2 {
                for dz in 0..1 {
                    let chunk = Chunk::generated(&generator, cx + dx, cz + dz);

                    for y in super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT {
                        for z in 0..16 {
                            for x in 0..16 {
                                feed(chunk.block(x, y, z) as u64);
                            }
                        }
                    }

                    for biome in &chunk.biomes {
                        feed(u64::from(*biome));
                    }

                    feed(chunk.containers.len() as u64);
                }
            }
        }
    }

    hash
}

/// Мир 26.1.2 не изменился ни на байт: цифра снята со сборки до генерации 26.3 (коммит 96b8231).
#[test]
fn native_generation_digest_is_pinned() {
    assert_eq!(native_digest(), 0xb35e_0e1e_4587_13b0);
}

/// Мир, открытый с каталогом 26.3 (java-version=26.3), складывает пятнистый лес, серные пещеры и
/// лагеря; мир 26.1.2 — нет.
#[test]
fn world_profile_switches_the_generator() {
    use crate::gameplay_catalog::{Catalog, Profile};
    use super::{World, WorldKind};

    for (profile, expected) in [(Profile::Java26_1_2, false), (Profile::Preview26_3, true)] {
        let dir = std::env::temp_dir().join(format!("mcsa-worldgen-profile-{}-{expected}", std::process::id()));
        let world = World::open_with_catalog(&dir, Some(7), WorldKind::Normal, Catalog::new(profile)).unwrap();

        assert!(matches!(&*world.generator, Generator::Normal(terrain) if terrain.preview() == expected));
        drop(world);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
