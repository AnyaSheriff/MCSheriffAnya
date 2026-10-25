// Серные пещеры 26.2 (профиль Preview26_3): сера и киноварь вместо камня в клетках биома,
// шипы на потолках и полах, мощная сера на дне водоёмов.
//
// Замер официального 26.3 (tools/measure/census_263.py, 25 тысяч чанков, 63 тысячи клеток
// биома): сера 1,49 млн и киноварь 1,17 млн блоков — около 66 % объёма клеток; шипов 9333
// (0,6 % от блоков серы), мощной серы 158. Разрезы столбцов: сплошные куски серы или киновари
// по 10–30 блоков, между ними глубинный сланец и камень. Тип зависит от места, а не от
// высоты, поэтому выбираем объёмным шумом. Источники (`rooted_sulfur_spring`) и гейзеры не
// ставим: [проверить] их вид и частота не сняты.

use super::terrain::Biome;
use super::{Chunk, Generator, CHUNK_SIZE, MIN_Y, WORLD_HEIGHT};

/// Доля исходной породы, остающаяся в клетках биома. Перепись двух семян
/// даёт 64–66 % серы и киновари от всего объёма биома, включая пустоты.
/// Прежние 12 % оставленной породы давали 71–74 % минералов [проверить].
const STONE_LEFT: f64 = 0.24;

/// Порог шума материала: ниже — сера, выше — киноварь. Подобран так, чтобы серы было
/// 56 % (замер: 1,49 против 1,17 млн блоков); проверяет `material_split_is_near_the_measurement`.
const MATERIAL_SPLIT: f64 = 0.04;

/// Шанс шипа на открытой грани: в переписи 26.3 2017–2731 шип на миллион
/// блоков биома. Прежние 5 % давали только 615–671 [проверить форму шипов].
/// Вики, Sulfur Caves: «sulfur spikes» — https://minecraft.wiki/w/Sulfur_Caves
const SPIKE_CHANCE: f64 = 0.168;

/// Попытка небольшого серного водоёма на клетку биома 4×4×4.
/// Частота своя [проверить]: цель переписи — около 30 блоков мощной серы
/// на миллион блоков биома, независимо от площади обычных водоносных слоёв.
const POOL_CHANCE: f64 = 0.0031;

fn mineral(name: &str) -> i32 {
    crate::gameplay_catalog::MINERALS.iter().find(|row| row.name == name).unwrap_or_else(|| panic!("{} в каталоге", name)).default_state
}

fn hash(seed: i64, x: i32, y: i32, z: i32, salt: u64) -> f64 {
    let mut value = (seed as u64) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (x as u32 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (y as u32 as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ (z as u32 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((value ^ (value >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// Выкладывает серные пещеры в чанке; без профиля Preview26_3 ничего не делает.
pub(super) fn decorate(generator: &Generator, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
    let Generator::Normal(terrain) = generator else {
        return;
    };

    if !terrain.preview() {
        return;
    }

    let number = Biome::SulfurCaves.number() as super::BiomeId;

    // Есть ли в чанке хоть одна клетка биома.
    if !chunk.biomes.contains(&number) {
        return;
    }

    let sulfur = mineral("sulfur");
    let cinnabar = mineral("cinnabar");
    let potent = mineral("potent_sulfur");
    let seed = terrain.seed();
    let stone: Vec<i32> = ["stone", "deepslate", "andesite", "diorite", "granite", "tuff"]
        .iter()
        .filter_map(|name| crate::blocks::state_by_name(name))
        .collect();
    let water = crate::fluids::state_of(crate::fluids::Kind::Water, crate::fluids::SOURCE);

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let (x, z) = (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);

            for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
                if chunk.biome(local_x, y, local_z) != number {
                    continue;
                }

                let state = chunk.block(local_x, y, local_z);

                if stone.contains(&state) {
                    if hash(seed, x, y, z, 1) < STONE_LEFT {
                        continue;
                    }

                    let pick = terrain.sulfur_noise(1, x, y, z);
                    chunk.put_generated(local_x, y, local_z, if pick < MATERIAL_SPLIT { sulfur } else { cinnabar }, true);
                }
            }
        }
    }

    sulfur_pools(chunk, seed, chunk_x, chunk_z, number, &stone, [sulfur, cinnabar, potent, water]);

    // Шипы — после минералов и водоёмов, когда стены уже сложены.
    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let (x, z) = (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);

            for y in MIN_Y + 1..MIN_Y + WORLD_HEIGHT - 1 {
                if chunk.biome(local_x, y, local_z) != number || chunk.block(local_x, y, local_z) != sulfur {
                    continue;
                }

                let above = chunk.block(local_x, y + 1, local_z);
                let below = chunk.block(local_x, y - 1, local_z);

                // Шип растёт вверх с пола или вниз с потолка: один, два или три блока.
                for (free, down, dy) in [(above, false, 1), (below, true, -1)] {
                    if free != super::AIR || hash(seed, x, y * 2 + i32::from(down), z, 3) >= SPIKE_CHANCE {
                        continue;
                    }

                    let length = 1 + (hash(seed, x, y, z, 4) * 3.0) as i32;

                    // Основание у опоры, кончик на дальнем конце [проверить: толщина по длине].
                    for step in 0..length {
                        let at = y + dy * (1 + step);

                        if chunk.block(local_x, at, local_z) != super::AIR {
                            break;
                        }

                        let thickness = if step == length - 1 {
                            crate::sulfur_spike::GEN_TIP
                        } else if step == 0 && length == 3 {
                            crate::sulfur_spike::GEN_BASE
                        } else {
                            crate::sulfur_spike::GEN_FRUSTUM
                        };

                        chunk.put_generated(local_x, at, local_z, crate::sulfur_spike::generated(thickness, down), false);
                    }
                }
            }
        }
    }
}

/// Вики, Sulfur Caves: «Potent sulfur is found only on the bottom of these pools».
/// https://minecraft.wiki/w/Sulfur_caves
/// Обычный аквифер не является серным водоёмом: прежний выбор любого мокрого
/// пола давал 137/163 вместо 30/30 блоков на миллион блоков биома двух семян.
/// Малые водоёмы целиком внутри клетки: стенки и дно замкнуты, порядок
/// соседних чанков не влияет. Размер и очертания свои [проверить].
fn sulfur_pools(chunk: &mut Chunk, seed: i64, cx: i32, cz: i32, biome: super::BiomeId, stone: &[i32], [sulfur, cinnabar, potent, water]: [i32; 4]) {
    let rock = |state| state == sulfur || state == cinnabar || stone.contains(&state);
    for x in (0..16).step_by(4) { for z in (0..16).step_by(4) {
        for y in (MIN_Y..MIN_Y + WORLD_HEIGHT - 4).step_by(4) {
            if chunk.biome(x, y, z) != biome
                || hash(seed, cx * 16 + x, y, cz * 16 + z, 5) >= POOL_CHANCE
            { continue; }
            // Водоём не прорезает существующую полость, руду или постройку.
            if !(x..x + 4).all(|px| (z..z + 4).all(|pz| (y + 1..=y + 3).all(|py| rock(chunk.block(px, py, pz))))) {
                continue;
            }
            for px in x..x + 4 { for pz in z..z + 4 {
                chunk.put_generated(px, y + 2, pz, sulfur, true);
                if (x + 1..=x + 2).contains(&px) && (z + 1..=z + 2).contains(&pz) {
                    chunk.put_generated(px, y + 1, pz, sulfur, true);
                    chunk.put_generated(px, y + 2, pz, water, true);
                    chunk.put_generated(px, y + 3, pz, super::AIR, true);
                }
            } }
            chunk.put_generated(x + 1, y + 1, z + 1, potent, true);
        }
    } }
}

#[cfg(test)]
mod tests {
    use super::super::terrain::{Style, Terrain};
    use super::*;

    /// Доля клеток биома среди всех клеток столбцов; печатается для сверки с замером
    /// 0,0016 (`cargo test --release sulfur_census -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn sulfur_census() {
        for seed in [42_i64, 1, 1_234_567_890] {
            let terrain = Terrain::new(seed, Style::Vanilla).with_preview(true);
            let (mut sulfur, mut total) = (0_u64, 0_u64);
            let mut by_depth = [0_u64; 8];

            for x in (-4096..4096).step_by(16) {
                for z in (-4096..4096).step_by(16) {
                    let column = terrain.column_in(x + 2, z + 2, None);
                    total += 96;

                    for layer in 0..96 {
                        let y = MIN_Y + layer * 4 + 2;

                        if terrain.sulfur_cell(&column, x + 2, y, z + 2) {
                            sulfur += 1;
                            by_depth[(((column.height - y) / 16) as usize).min(7)] += 1;
                        }
                    }
                }
            }

            println!("seed {seed}: серных клеток {sulfur} из {total} = {:.5} (замер 0.00160), по 16 блоков глубины {by_depth:?}", sulfur as f64 / total as f64);
        }
    }

    #[test]
    fn native_profile_has_no_sulfur_caves() {
        let terrain = Terrain::new(42, Style::Vanilla);

        for x in (-2048..2048).step_by(32) {
            let column = terrain.column_in(x, x / 2, None);

            for y in (-60..60).step_by(4) {
                assert!(!terrain.sulfur_cell(&column, x, y, x / 2));
            }
        }
    }

    /// Сера и киноварь делят объём как в замере: 56 против 44 с допуском в пять пунктов.
    #[test]
    fn material_split_is_near_the_measurement() {
        let terrain = Terrain::new(42, Style::Vanilla).with_preview(true);
        let (mut low, mut all) = (0, 0);

        for x in (-400..400).step_by(5) {
            for z in (-400..400).step_by(5) {
                for y in (-40..40).step_by(5) {
                    all += 1;
                    low += i32::from(terrain.sulfur_noise(1, x, y, z) < MATERIAL_SPLIT);
                }
            }
        }

        let share = f64::from(low) / f64::from(all);
        assert!((share - 0.56).abs() < 0.05, "серы {share}");
    }

    /// Координаты наши, эталон — плотности на блок серного биома оригинала.
    /// Для редкой мощной серы учитывается шум малого числа наблюдений.
    #[test]
    fn generated_sulfur_materials_match_per_biome_reference() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/batch-u/sulfur-reference.json"
        )).unwrap();
        let names = ["sulfur", "cinnabar", "sulfur_spike", "potent_sulfur"];
        let states = [mineral("sulfur"), mineral("cinnabar"), 0, mineral("potent_sulfur")];
        let mut total_counts = [0usize; 4];
        let mut total_expected = [0.0_f64; 4];
        for sample in reference["samples"].as_array().unwrap() {
            let generator = Generator::Normal(Terrain::new(sample["seed"].as_i64().unwrap(), Style::Vanilla).with_preview(true));
            let mut counts = [0usize; 4];
            let mut volume = 0usize;
            for at in sample["chunks"].as_array().unwrap() {
                let chunk = Chunk::generated(&generator, at[0].as_i64().unwrap() as i32, at[1].as_i64().unwrap() as i32);
                for y in MIN_Y..MIN_Y + WORLD_HEIGHT { for z in 0..16 { for x in 0..16 {
                    if chunk.biome(x, y, z) != Biome::SulfurCaves.number() as super::super::BiomeId { continue; }
                    volume += 1;
                    let state = chunk.block(x, y, z);
                    for i in 0..4 {
                        counts[i] += usize::from(if i == 2 { crate::sulfur_spike::is_spike(state) } else { state == states[i] });
                    }
                } } }
            }
            assert!(volume > 0);
            for i in 0..4 {
                total_counts[i] += counts[i];
                total_expected[i] += sample["official_per_million"][names[i]].as_f64().unwrap() * volume as f64 / 1e6;
            }
        }
        for i in 0..4 {
            let expected = total_expected[i];
            let tolerance = (expected * 0.2).max(3.0 * expected.sqrt());
            assert!((total_counts[i] as f64 - expected).abs() <= tolerance,
                "{} на двух семенах: {} / {expected}", names[i], total_counts[i]);
        }
    }
}
