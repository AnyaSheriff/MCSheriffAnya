use super::*;

// Прежний полный цикл: независимый оракул порядка лучей, клеток и RNG.
fn legacy_rays_with(world: &mut World, centre: (f64, f64, f64), power: f32, cap: Option<f32>, resist: bool) -> (Vec<Pos>, Vec<Pos>) {
    // Хеш — быстрый (`CoordHash`): у ядерного динамита клеток сотни тысяч.
    let mut seen: HashSet<Pos, CoordHash> = HashSet::default();
    let mut reached: HashSet<Pos, CoordHash> = HashSet::default();
    // Задетые клетки по порядку первого касания (и воздух): по ним ложится
    // огонь взрыва с огнём.
    let mut touched = Vec::new();
    let mut doomed = Vec::new();
    let edge = |n: i32| n == 0 || n == GRID - 1;
    let last = (GRID - 1) as f64;

    for i in 0..GRID {
        for j in 0..GRID {
            for k in 0..GRID {
                if !(edge(i) || edge(j) || edge(k)) {
                    continue;
                }

                let raw = (
                    i as f64 / last * 2.0 - 1.0,
                    j as f64 / last * 2.0 - 1.0,
                    k as f64 / last * 2.0 - 1.0,
                );
                let length = (raw.0 * raw.0 + raw.1 * raw.1 + raw.2 * raw.2).sqrt();
                let step = (raw.0 / length * RAY_STEP, raw.1 / length * RAY_STEP, raw.2 / length * RAY_STEP);

                let mut intensity = power * (0.7 + world.random_unit() * 0.6);
                let mut at = centre;

                while intensity > 0.0 {
                    let cell = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);

                    // За краем мира блоков нет — луч дальше не идёт.
                    if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&cell.1) {
                        break;
                    }

                    let state = world.get_block(cell.0, cell.1, cell.2);

                    if state != AIR && resist {
                        // Синий череп иссушителя: прочность не больше 0,8, кроме
                        // неразрушимого (mob/wither.rs).
                        let Ok(own_resistance) = world.catalog().blast_resistance(state) else {
                            // Неизвестный профильный блок нельзя считать воздухом или ломать.
                            break;
                        };
                        let resistance = match cap {
                            Some(cap) if !crate::mob::wither::wither_immune(state) => own_resistance.min(cap),
                            _ => own_resistance,
                        };

                        intensity -= (resistance + 0.3) * 0.3;
                    }

                    if intensity > 0.0 {
                        if reached.insert(cell) {
                            touched.push(cell);
                        }

                        if can_blow_up(world.catalog(), state) && seen.insert(cell) {
                            doomed.push(cell);
                        }
                    }

                    at = (at.0 + step.0, at.1 + step.1, at.2 + step.2);
                    intensity -= RAY_FADE;
                }
            }
        }
    }

    // Перемешать — Фишер–Йейтс на генераторе мира.
    for index in (1..doomed.len()).rev() {
        let other = world.random_bits() as usize % (index + 1);
        doomed.swap(index, other);
    }

    (doomed, touched)
}


#[test]
fn ordered_ray_steps_keep_all_1352_f64_bits_and_reuse_storage() {
    let actual = ray_steps();
    assert_eq!(actual.len(), 1352);
    assert!(std::ptr::eq(actual.as_ptr(), ray_steps().as_ptr()));
    let mut index = 0;
    for i in 0..16 {
        for j in 0..16 {
            for k in 0..16 {
                if i != 0 && i != 15 && j != 0 && j != 15 && k != 0 && k != 15 { continue; }
                let raw = (i as f64 / 15.0 * 2.0 - 1.0, j as f64 / 15.0 * 2.0 - 1.0, k as f64 / 15.0 * 2.0 - 1.0);
                let length = (raw.0 * raw.0 + raw.1 * raw.1 + raw.2 * raw.2).sqrt();
                let expected = (raw.0 / length * 0.3, raw.1 / length * 0.3, raw.2 / length * 0.3);
                assert_eq!(actual[index].0.to_bits(), expected.0.to_bits());
                assert_eq!(actual[index].1.to_bits(), expected.1.to_bits());
                assert_eq!(actual[index].2.to_bits(), expected.2.to_bits());
                index += 1;
            }
        }
    }
    assert_eq!(index, actual.len());
}

fn fixture(profile: crate::gameplay_catalog::Profile, scene: usize, height: i32, skip_random: usize) -> World {
    let mut world = World::in_memory_with_catalog(crate::gameplay_catalog::Catalog::new(profile));
    if scene != 0 {
        let material = blocks::state_by_name(if scene == 1 { "stone" } else { "water" }).unwrap();
        for x in -3..=3 {
            for y in height - 3..=height + 3 {
                if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) { continue; }
                for z in -3..=3 {
                    if (x, y, z) != (0, height, 0) { world.set_block_silently(x, y, z, material); }
                }
            }
        }
        if scene == 3 { world.set_block_silently(1, height, 0, blocks::state_by_name("bedrock").unwrap()); }
    }
    for _ in 0..skip_random { world.random_bits(); }
    world
}

#[test]
fn ordered_ray_steps_match_legacy_cells_shuffle_and_next_random() {
    use crate::gameplay_catalog::Profile;
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        // Воздух, камень, вода, вода с бедроком; нижняя и верхняя границы.
        for (scene, height) in [(0, 64), (1, 64), (2, 64), (3, 64), (1, MIN_Y), (2, MIN_Y + WORLD_HEIGHT - 1)] {
            for skip_random in [0, 7, 31] {
                for (power, cap, resist) in [(4.0, None, true), (4.0, Some(0.8), true), (1.2, None, false)] {
                    let mut actual = fixture(profile, scene, height, skip_random);
                    let mut expected = fixture(profile, scene, height, skip_random);
                    let centre = (-0.125, height as f64 + 0.06125, 0.5);
                    let result = rays_with(&mut actual, centre, power, cap, resist);
                    let reference = legacy_rays_with(&mut expected, centre, power, cap, resist);
                    assert_eq!(result, reference, "scene={scene} height={height} skip={skip_random} power={power} cap={cap:?} resist={resist}");
                    // После затухания и перемешивания поток RNG совпадает полностью.
                    for _ in 0..8 { assert_eq!(actual.random_bits(), expected.random_bits()); }
                }
            }
        }
    }
}
