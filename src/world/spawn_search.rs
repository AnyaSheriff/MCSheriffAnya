use super::terrain::{self, Column};

/// Исходная точка мира: тот же обход суши для сервера и карты.
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
