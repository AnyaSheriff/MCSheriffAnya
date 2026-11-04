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
    MODELS.get_or_init(|| {
        let mut models: Vec<Model> = serde_json::from_str(include_str!("tree_shapes_26_3.json")).expect("замеры крон 26.3");
        // minecraft.wiki/w/Dark_oak: «thick 2×2 trunks». Сохраняем все
        // четыре столбца и крону целиком: 32 изолированные площадки 26.3.
        models.extend(serde_json::from_str::<Vec<Model>>(include_str!(
            "../../testdata/world-profile/genaudit-26.3/dark-oak-models-v2.json")).expect("замеры тёмного дуба 26.3"));
        // minecraft.wiki/w/Cherry: «curved branches». Целые формы, включая
        // оси ветвей; 32 отдельные площадки вишни 26.3.
        models.extend(serde_json::from_str::<Vec<Model>>(include_str!(
            "../../testdata/world-profile/genaudit-26.3/batch-t3/crown-models.json")).expect("замеры крон T3"));
        models
    })
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
        Shape::DarkOak => "dark_oak",
        Shape::Cherry => "cherry",
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
    let wide = model.species == "dark_oak";
    if !wide { for y in 1..=top { blocks.push((0, y, 0, Part::Log(Axis::Y))); } }
    for &(x, y, z, axis) in &model.logs {
        if !wide && x == 0 && z == 0 && y <= 0 && axis == 1 { continue; }
        let (x, z) = transform(x, z, symmetry, wide);
        let mut axis = match axis { 0 => Axis::X, 2 => Axis::Z, _ => Axis::Y };
        if !symmetry.is_multiple_of(2) { axis = match axis { Axis::X => Axis::Z, Axis::Z => Axis::X, Axis::Y => Axis::Y }; }
        blocks.push((x, y + top, z, Part::Log(axis)));
    }
    for &(y, z, mask, _) in &model.rows {
        for bit in 0..17 {
            if mask & (1 << bit) != 0 {
                let (x, z) = transform(bit - 8, z, symmetry, wide);
                blocks.push((x, y + top, z, Part::Leaves));
            }
        }
    }
}

fn transform(mut x: i32, mut z: i32, symmetry: u64, wide: bool) -> (i32, i32) {
    let center = i32::from(wide);
    if symmetry >= 4 { x = center - x; }
    for _ in 0..symmetry % 4 { (x, z) = (center - z, x); }
    (x, z)
}

/// Целые группы грибов у основания естественного тополя: 96 деревьев
/// селектора, включая 61 без грибов. minecraft.wiki/w/Shelf_Mushroom:
/// «Both small and large shelf mushrooms generate attached to poplar logs».
pub(super) fn poplar_shelf(random: &mut Random, blocks: &mut Vec<TreeBlock>) {
    type Mushroom = (i32,i32,i32,usize,usize);
    static MODELS: OnceLock<Vec<Vec<Mushroom>>> = OnceLock::new();
    let models = MODELS.get_or_init(|| serde_json::from_str(include_str!(
        "../../testdata/world-profile/genaudit-26.3/poplar-shelf-models-v2.json")).expect("грибы естественных тополей 26.3"));
    let model = &models[((random.next() >> 32) % models.len() as u64) as usize];
    let symmetry = (random.next() >> 32) % 8;
    const STATES: [[&str;4];2] = [
        ["shelf_mushroom[age=0,facing=north]","shelf_mushroom[age=0,facing=south]",
         "shelf_mushroom[age=0,facing=west]","shelf_mushroom[age=0,facing=east]"],
        ["shelf_mushroom[age=1,facing=north]","shelf_mushroom[age=1,facing=south]",
         "shelf_mushroom[age=1,facing=west]","shelf_mushroom[age=1,facing=east]"],
    ];
    for &(x,y,z,age,face) in model {
        let (x,z) = transform(x,z,symmetry,false);
        let (fx,fz) = [(0,-1),(0,1),(-1,0),(1,0)][face];
        let (fx,fz) = transform(fx,fz,symmetry,false);
        if !blocks.iter().any(|b| (b.0,b.1,b.2) == (x-fx,y,z-fz) && matches!(b.3,Part::Log(_)))
            || blocks.iter().any(|b| (b.0,b.1,b.2) == (x,y,z)) { continue; }
        let face = match (fx,fz) { (0,-1) => 0, (0,1) => 1, (-1,0) => 2, _ => 3 };
        blocks.push((x,y,z,Part::Block(STATES[age][face])));
    }
}

/// Сохраняем состояния листвы, а не только её силуэт. Значения оригинала
/// могут ждать соседнего обновления; их нельзя заменять кратчайшим путём.
pub(super) fn distances(index: usize, top: i32, random: &mut Random) -> std::collections::HashMap<(i32,i32,i32),i32> {
    let symmetry = (random.next() >> 32) % 8;
    let mut result = std::collections::HashMap::new();
    let wide = models()[index].species == "dark_oak";
    for &(y,z,mask,distance) in &models()[index].rows {
        for bit in 0..17 {
            if mask & (1 << bit) != 0 {
                let (x,z)=transform(bit-8,z,symmetry,wide);
                result.insert((x,y+top,z),distance);
            }
        }
    }
    result
}
