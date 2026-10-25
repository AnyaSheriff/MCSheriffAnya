// Наблюдённые формы 26.3: только блоки из сохранённых region оригинала.
// Выбор целого образца сохраняет связь слоёв, ветвей и расстояний листвы;
// независимые вероятности клеток эту связь теряли. Повороты и отражения
// добавляют варианты, не меняя статистику. Прежний профиль сюда не входит.
use super::noise::Random;
use super::trees::{Axis, Part, Shape, TreeBlock};
use std::sync::OnceLock;

#[derive(serde::Deserialize)]
struct Model {
    species: String,
    trunk: i32,
    logs: Vec<(i32, i32, i32, u8)>,
    rows: Vec<(i32, i32, u32, i32)>,
}

fn models() -> &'static [Model] {
    static MODELS: OnceLock<Vec<Model>> = OnceLock::new();
    MODELS.get_or_init(|| serde_json::from_str(include_str!("tree_shapes_26_3.json")).expect("замеры крон 26.3"))
}

pub(super) fn select(shape: Shape, random: &mut Random) -> Option<usize> {
    let species = match shape {
        Shape::Oak => "oak",
        Shape::Fancy => "fancy_oak",
        Shape::Round => "birch",
        Shape::Swamp => "swamp_oak",
        Shape::Acacia => "acacia",
        Shape::Spruce => "spruce",
        Shape::Poplar => "natural_poplar",
        _ => return None,
    };
    select_named(species, random)
}

pub(super) fn select_sapling(shape: Shape, random: &mut Random) -> Option<usize> {
    if shape == Shape::Poplar { select_named("poplar", random) } else { select(shape, random) }
}

fn select_named(species: &str, random: &mut Random) -> Option<usize> {
    let start = models().iter().position(|m| m.species == species)?;
    let count = models()[start..].iter().take_while(|m| m.species == species).count();
    Some(start + ((random.next() >> 32) % count as u64) as usize)
}

pub(super) fn trunk(index: usize) -> i32 { models()[index].trunk }

pub(super) fn draw(index: usize, top: i32, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
    let model = &models()[index];
    let symmetry = (random.next() >> 32) % 8;
    for y in 1..=top { blocks.push((0, y, 0, Part::Log(Axis::Y))); }
    for &(x, y, z, axis) in &model.logs {
        if x == 0 && z == 0 && y <= 0 && axis == 1 { continue; }
        let (x, z) = transform(x, z, symmetry);
        let mut axis = match axis { 0 => Axis::X, 2 => Axis::Z, _ => Axis::Y };
        if !symmetry.is_multiple_of(2) { axis = match axis { Axis::X => Axis::Z, Axis::Z => Axis::X, Axis::Y => Axis::Y }; }
        blocks.push((x, y + top, z, Part::Log(axis)));
    }
    for &(y, z, mask, _) in &model.rows {
        for bit in 0..17 {
            if mask & (1 << bit) != 0 {
                let (x, z) = transform(bit - 8, z, symmetry);
                blocks.push((x, y + top, z, Part::Leaves));
            }
        }
    }
}

fn transform(mut x: i32, mut z: i32, symmetry: u64) -> (i32, i32) {
    if symmetry >= 4 { x = -x; }
    for _ in 0..symmetry % 4 { (x, z) = (-z, x); }
    (x, z)
}

/// Сохраняем состояния листвы, а не только её силуэт. Значения оригинала
/// могут ждать соседнего обновления; их нельзя заменять кратчайшим путём.
pub(super) fn distances(index: usize, top: i32, random: &mut Random) -> std::collections::HashMap<(i32,i32,i32),i32> {
    let symmetry = (random.next() >> 32) % 8;
    let mut result = std::collections::HashMap::new();
    for &(y,z,mask,distance) in &models()[index].rows {
        for bit in 0..17 {
            if mask & (1 << bit) != 0 {
                let (x,z)=transform(bit-8,z,symmetry);
                result.insert((x,y+top,z),distance);
            }
        }
    }
    result
}
