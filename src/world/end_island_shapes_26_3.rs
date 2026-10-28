// Формы 32 отдельных end_island, наблюдённые на официальном сервере 26.3.
// Команда place feature в пустом плоском обычном мире на Y120 измеряет
// геометрию. Естественное распределение форм внутри классов 4/5/6 [проверить].
// Собственные клетки/ответы/SHA: handoff/end-island-shapes-20261028.
// Измеренные столбцы непрерывны до относительной высоты 0; нижние слои
// входят в верхний.

use std::sync::OnceLock;

type Dense = [Option<(i8, i8)>; 13 * 13];

struct Models {
    classes: [Vec<Dense>; 3],
}

fn models() -> &'static Models {
    static MODELS: OnceLock<Models> = OnceLock::new();
    MODELS.get_or_init(|| {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/end-island-shapes-26.3.json"
        )).expect("собственная таблица форм островков читается");
        assert_eq!(data["schema"].as_u64(), Some(1));
        let mut classes: [Vec<Dense>; 3] = std::array::from_fn(|_| Vec::new());
        for model in data["models"].as_array().expect("формы присутствуют") {
            let radius = model["radius"].as_i64().expect("радиус формы");
            assert!((4..=6).contains(&radius));
            let mut dense = [None; 13 * 13];
            for value in model["columns"].as_array().expect("столбцы формы") {
                let fields = value.as_array().expect("четыре целых поля столбца");
                assert_eq!(fields.len(), 4);
                let x = fields[0].as_i64().expect("dx");
                let z = fields[1].as_i64().expect("dz");
                let bottom = fields[2].as_i64().expect("низ");
                let top = fields[3].as_i64().expect("верх");
                assert!((-radius..=radius).contains(&x) && (-radius..=radius).contains(&z));
                assert!((-6..=0).contains(&bottom) && top == 0);
                let at = ((z + 6) * 13 + x + 6) as usize;
                assert!(dense[at].is_none(), "столбец формы не повторяется");
                dense[at] = Some((bottom as i8, top as i8));
            }
            classes[(radius - 4) as usize].push(dense);
        }
        assert_eq!(classes.iter().map(Vec::len).collect::<Vec<_>>(), vec![10, 14, 8]);
        Models { classes }
    })
}

/// Число наблюдённых форм выбранного класса ширины; неизвестный класс даёт 0.
/// Выбор формы должен использовать отдельный RNG, не поток размещения.
pub fn model_count(radius: i32) -> u32 {
    match radius {
        4..=6 => models().classes[(radius - 4) as usize].len() as u32,
        _ => 0,
    }
}

/// Низ/верх относительно заданной высоты верхушки, включительно.
/// Вызов после однократного декодирования — доступ к плотному массиву 13×13.
pub fn column(radius: i32, index: u32, dx: i32, dz: i32) -> Option<(i32, i32)> {
    if !(4..=6).contains(&radius) || !(-6..=6).contains(&dx) || !(-6..=6).contains(&dz) {
        return None;
    }
    let dense = models().classes[(radius - 4) as usize].get(index as usize)?;
    dense[((dz + 6) * 13 + dx + 6) as usize].map(|(bottom, top)| (i32::from(bottom), i32::from(top)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn observations() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/end-island-shapes-observations-26.3.json"
        )).expect("отдельно сохранённые наблюдённые клетки")
    }

    fn measured_cells(plot: &serde_json::Value) -> BTreeSet<(i32, i32, i32)> {
        plot["cells"].as_array().unwrap().iter().map(|c| {
            (c[0].as_i64().unwrap() as i32, c[1].as_i64().unwrap() as i32, c[2].as_i64().unwrap() as i32)
        }).collect()
    }

    fn rendered(radius: i32, index: u32) -> BTreeSet<(i32, i32, i32)> {
        let mut cells = BTreeSet::new();
        for z in -8..=8 {
            for x in -8..=8 {
                if let Some((bottom, top)) = column(radius, index, x, z) {
                    for y in bottom..=top { cells.insert((x, y, z)); }
                }
            }
        }
        cells
    }

    #[test]
    fn every_model_reconstructs_independent_observed_cells_and_layers() {
        let data = observations();
        let mut next = [0; 3];
        for plot in data["observations"].as_array().unwrap() {
            let radius = plot["radius"].as_i64().unwrap() as i32;
            let index = &mut next[(radius - 4) as usize];
            let actual = rendered(radius, *index);
            *index += 1;
            let measured = measured_cells(plot);
            assert_eq!(actual, measured, "не совпала форма{}", plot["plot_index"]);
            assert_eq!(actual.len(), plot["volume"].as_u64().unwrap() as usize);
            let mut counts = BTreeMap::new();
            for &(_, y, _) in &actual { *counts.entry(y).or_insert(0usize) += 1; }
            let observed: BTreeMap<_, _> = plot["layers"].as_array().unwrap().iter()
                .map(|layer| (layer["dy"].as_i64().unwrap() as i32, layer["blocks"].as_u64().unwrap() as usize)).collect();
            assert_eq!(counts, observed);
        }
        assert_eq!(next, [model_count(4), model_count(5), model_count(6)]);
    }

    #[test]
    fn observations_have_contiguous_columns_and_nested_layers_with_measured_extents() {
        let data = observations();
        for plot in data["observations"].as_array().unwrap() {
            let cells = measured_cells(plot);
            let radius = plot["radius"].as_i64().unwrap() as i32;
            assert_eq!(cells.iter().map(|&(x, _, z)| x.abs().max(z.abs())).max(), Some(radius));
            for &(x, y, z) in &cells {
                assert!((-6..=0).contains(&y));
                for above in y..=0 { assert!(cells.contains(&(x, above, z))); }
            }
            // Есть клетки, отвергаемые прежним кругом радиуса4/5/6.
            assert!(cells.iter().any(|&(x, y, z)| y == 0 && x*x + z*z > radius*radius));
        }
    }

    #[test]
    fn shape_crossing_four_chunk_corners_keeps_every_measured_cell() {
        let data = observations();
        let mut next = [0; 3];
        for plot in data["observations"].as_array().unwrap() {
            let radius = plot["radius"].as_i64().unwrap() as i32;
            let index = next[(radius - 4) as usize];
            next[(radius - 4) as usize] += 1;
            let mut placed = BTreeSet::new();
            let mut occupied = BTreeSet::new();
            for cz in -1..=0 {
                for cx in -1..=0 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let (x, z) = (cx*16 + lx, cz*16 + lz);
                            if let Some((bottom, top)) = column(radius, index, x, z) {
                                occupied.insert((cx, cz));
                                for y in bottom..=top { assert!(placed.insert((x, y, z))); }
                            }
                        }
                    }
                }
            }
            assert_eq!(occupied.len(), 4);
            assert_eq!(placed, measured_cells(plot), "обрезка на отрицательной границе чанков");
        }
    }

    #[test]
    fn invalid_model_and_distant_coordinates_do_not_create_columns() {
        for radius in [i32::MIN, -1, 0, 3, 7, i32::MAX] {
            assert_eq!(model_count(radius), 0);
            assert_eq!(column(radius, 0, 0, 0), None);
        }
        for radius in 4..=6 {
            assert_eq!(column(radius, model_count(radius), 0, 0), None);
            assert_eq!(column(radius, u32::MAX, 0, 0), None);
            for (x, z) in [(7, 0), (0, -7), (i32::MIN, i32::MAX)] {
                assert_eq!(column(radius, 0, x, z), None);
            }
        }
    }
}
