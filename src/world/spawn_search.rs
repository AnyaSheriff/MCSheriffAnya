use super::terrain::{self, Climate, Column};

/// Исходная точка мира: тот же обход суши для сервера и карты.
pub fn find_preview(column_at: impl Fn(i32, i32) -> Column) -> (f64, f64, f64) {
    let (x, z) = climate_origin(|x,z| column_at(x,z).climate);
    // https://minecraft.wiki/w/World_spawn: «five-chunk square radius».
    // Сначала свой чанк, затем квадратные кольца; внутри — порядок X/Z.
    let centre = (x.div_euclid(16), z.div_euclid(16));
    for radius in 0_i32..=5 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius { continue; }
                for x in (centre.0+dx)*16..(centre.0+dx+1)*16 {
                    for z in (centre.1+dz)*16..(centre.1+dz+1)*16 {
                        let column = column_at(x,z);
                        if safe(&column) { return (f64::from(x)+0.5, f64::from(column.height+1), f64::from(z)+0.5); }
                    }
                }
            }
        }
    }
    find(column_at)
}

fn safe(column: &Column) -> bool {
    column.height >= terrain::SEA && !column.biome.is_ocean()
        && !matches!(column.biome, terrain::Biome::MushroomFields | terrain::Biome::River | terrain::Biome::FrozenRiver)
}

/// https://minecraft.wiki/w/World_spawn: «66 exact coordinates», «862 coordinates».
/// Климат ищется до проверки поверхности; ближайшая суша могла оказаться
/// руслом или краем моря. Диапазоны проверены на семенах переписи;
/// точный порядок финального обхода чанков оригинала [проверить].
fn climate_origin(climate_at: impl Fn(i32, i32) -> Climate) -> (i32, i32) {
    let mut best = (0,0);
    let mut score = fitness(climate_at(0,0),0,0);
    for (step, reach) in [(512,2048),(32,512)] {
        let centre = best;
        for radius in (step..=reach).step_by(step as usize) {
            let mut angle = 0.0_f64;
            while angle < std::f64::consts::TAU {
                let x = centre.0 + (angle.sin()*f64::from(radius)) as i32;
                let z = centre.1 + (angle.cos()*f64::from(radius)) as i32;
                let candidate = fitness(climate_at(x,z),x,z);
                if candidate < score { score = candidate; best = (x,z); }
                angle += f64::from(step)/f64::from(radius);
            }
        }
    }
    best
}

fn fitness(climate: Climate, x: i32, z: i32) -> f64 {
    let gap = |v: f64, low: f64, high: f64| (low-v).max(v-high).max(0.0);
    let penalty = gap(climate.continent,-0.11,1.0).powi(2)
        + gap(climate.temperature,-1.0,1.0).powi(2)
        + gap(climate.humidity,-1.0,1.0).powi(2)
        + gap(climate.erosion,-1.0,1.0).powi(2)
        + (0.16-climate.weirdness.abs()).max(0.0).powi(2);
    // https://minecraft.wiki/w/Tutorial:Adding_a_new_dimension: «spawn_target».
    // https://minecraft.wiki/w/Noise_settings: «any density function».
    // В 26.3 цель больше не ограничена старой биомной метрикой. Прежняя
    // нормировка делала малый выход за диапазон дешевле округления окружности.
    // Эффективный вес собственной модели проверен независимыми спавнами;
    // точные единицы целей плотности оригинала [проверить].
    let squared = f64::from(x).powi(2)+f64::from(z).powi(2);
    penalty * 1_000_000.0 + squared.powi(2)/(390_625.0 * 100_000_000.0)
}

pub fn find(column_at: impl Fn(i32, i32) -> Column) -> (f64, f64, f64) {
    const SEARCH: i32 = 3_000;
    const STEP: i32 = 16;

    // https://minecraft.wiki/w/Spawn: «world spawn».
    // Своя модель: выбираем ближайшую сушу, а не первый угол квадрата.
    // Прежний обход мог взять угол раньше более близкой середины стороны
    // следующего кольца; SEARCH при этом ограничивал ось, а не расстояние.
    let radius = SEARCH / STEP;
    let mut candidates = Vec::new();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let (x, z) = (dx * STEP, dz * STEP);
            if x*x + z*z <= SEARCH*SEARCH { candidates.push((x, z)); }
        }
    }
    candidates.sort_unstable_by_key(|&(x, z)| (x*x + z*z, z, x));
    for (x, z) in candidates {
        let column = column_at(x, z);
        // Вики, Mushroom Fields: «Players cannot spawn in mushroom fields».
        if column.height >= terrain::SEA && !column.biome.is_ocean()
            && column.biome != terrain::Biome::MushroomFields {
            return (f64::from(x) + 0.5, f64::from(column.height + 1), f64::from(z) + 0.5);
        }
    }

    // Суши не нашлось — появляемся над водой в начале координат.
    (0.5, f64::from(terrain::SEA + 1), 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::Biome;

    #[test]
    fn climate_search_has_the_documented_two_sampling_passes() {
        let samples = std::cell::Cell::new(0);
        let origin = climate_origin(|_,_| { samples.set(samples.get()+1); Column::flat(70).climate });
        assert_eq!(origin,(0,0));
        assert_eq!(samples.get(),66+862);
    }

    #[test]
    fn climate_search_discourages_river_noise_and_ocean_noise() {
        let ordinary = Climate { weirdness: -0.3, continent: 0.2, ..Column::flat(70).climate };
        assert!(fitness(ordinary,512,0) < fitness(Climate { weirdness:0.0, ..ordinary },0,0));
        assert!(fitness(ordinary,512,0) < fitness(Climate { continent:-0.5, ..ordinary },0,0));
    }

    #[test]
    fn owner_seed_spawn_is_on_dry_land_in_the_official_spawn_chunk() {
        let terrain = terrain::Terrain::new(1_792_932_773_345_722_119,terrain::Style::Vanilla).with_preview(true);
        let spawn = find_preview(|x,z|terrain.column_at(x,z));
        assert_eq!((spawn.0.floor() as i32,spawn.2.floor() as i32),(16,-48));
        assert!(safe(&terrain.column_at(16,-48)));
    }

    /// https://minecraft.wiki/w/World_spawn: «climate thresholds».
    /// Независимые сохранённые миры включают случаи, в которых прежний
    /// вес выбирал совсем другой чанк, и обычные контрольные семена.
    #[test]
    fn preview_spawn_matches_independent_black_box_worlds() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/batch-a4/spawn.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let seed = case["seed"].as_i64().unwrap();
            let terrain = terrain::Terrain::new(seed,terrain::Style::Vanilla).with_preview(true);
            let spawn = find_preview(|x,z|terrain.column_at(x,z));
            let x = spawn.0.floor() - case["vanilla"][0].as_f64().unwrap();
            let z = spawn.2.floor() - case["vanilla"][1].as_f64().unwrap();
            assert!(x.hypot(z) <= fixture["tolerance_blocks"].as_f64().unwrap(),
                "seed={seed}, spawn={spawn:?}, offset={}",x.hypot(z));
            assert!(safe(&terrain.column_at(spawn.0.floor() as i32,spawn.2.floor() as i32)));
            #[cfg(not(seedmap_core))]
            {
                let (x,y,z) = (spawn.0.floor() as i32,spawn.1 as i32-1,spawn.2.floor() as i32);
                let generator = crate::world::Generator::Normal(terrain);
                let chunk = crate::world::Chunk::generated(&generator,x.div_euclid(16),z.div_euclid(16));
                let support = crate::blocks::block_at_state(chunk.block(x.rem_euclid(16),y,z.rem_euclid(16)));
                assert!(!matches!(support,None | Some("air" | "cave_air" | "void_air" | "water" | "lava")),
                    "seed={seed}, spawn={spawn:?}, support={support:?}");
            }
        }
    }

    fn water() -> Column { Column { biome: Biome::Ocean, ..Column::flat(40) } }

    #[test]
    fn spawn_uses_distance_instead_of_square_corner_order() {
        let place = find(|x,z| if (x,z) == (-16,-16) || (x,z) == (0,-16) { Column::flat(70) } else { water() });
        assert_eq!(place, (0.5,71.0,-15.5));
    }

    #[test]
    fn spawn_does_not_stop_at_a_far_corner_before_a_nearer_next_ring() {
        let place = find(|x,z| if (x,z) == (-48,-48) || (x,z) == (0,-64) { Column::flat(70) } else { water() });
        assert_eq!(place, (0.5,71.0,-63.5));
    }

    #[test]
    fn spawn_rejects_flooded_land_and_high_ocean_biomes() {
        let place = find(|x,z| match (x,z) {
            (0,0) => Column::flat(terrain::SEA-1),
            (0,-16) => Column { biome: Biome::Ocean, ..Column::flat(100) },
            (-16,0) => Column::flat(terrain::SEA),
            _ => water(),
        });
        assert_eq!(place, (-15.5, f64::from(terrain::SEA+1), 0.5));
    }

    #[test]
    fn spawn_skips_mushroom_islands() {
        let place = find(|x,z| match (x,z) {
            (0,0) => Column { biome: Biome::MushroomFields, ..Column::flat(68) },
            (0,-16) => Column::flat(70),
            _ => water(),
        });
        assert_eq!(place,(0.5,71.0,-15.5));
    }

    #[test]
    fn spawn_fallback_is_bounded_by_the_search_radius() {
        let place = find(|x,z| { assert!(x*x+z*z <= 3_000*3_000); water() });
        assert_eq!(place, (0.5,f64::from(terrain::SEA+1),0.5));
    }
}
