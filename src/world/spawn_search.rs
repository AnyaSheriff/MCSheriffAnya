use super::terrain::{self, Column};

/// Исходная точка мира: тот же обход суши для сервера и карты.
pub fn find(column_at: impl Fn(i32, i32) -> Column) -> (f64, f64, f64) {
    /// Насколько далеко от начала координат искать сушу.
    const SEARCH: i32 = 3_000;

    /// Через сколько блоков проверять следующее место.
    const STEP: i32 = 16;

    let mut ring = 0;

    while ring * STEP <= SEARCH {
        let side = ring * STEP;

        // Обходим кольцо: его углы и стороны с тем же шагом.
        for step in -ring..=ring {
            let along = step * STEP;

            for (x, z) in [
                (along, -side),
                (along, side),
                (-side, along),
                (side, along),
            ] {
                let column = column_at(x, z);

                // Суша выше уровня моря и не голый лёд: на воде и на
                // отвесных пиках появляться незачем.
                if column.height >= terrain::SEA && !column.biome.is_ocean() {
                    return (x as f64 + 0.5, (column.height + 1) as f64, z as f64 + 0.5);
                }
            }
        }

        ring += 1;
    }

    // Суши не нашлось — появляемся над водой в начале координат.
    (0.5, (terrain::SEA + 1) as f64, 0.5)
}
