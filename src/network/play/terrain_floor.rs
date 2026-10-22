//! Ограниченная сверка пола: только новый проход вниз через известный верх.
//! Обычные записи777 FIRST12: отклоняется весь шаг, включая прежнюю высоту
//! в воздухе; client on_ground не выбирает решение. Это не общий решатель физики.
use super::*;
use crate::gameplay_catalog::{Catalog, SlabHalf};

const HALF: f64 = 0.3;
// Край основания: как у собственной проверки лодки; точный допуск [проверить].
const EDGE_EPSILON: f64 = 1.0e-7;
// Ordinary stone24: глубина1e-7 принята, 1e-5/1e-4/.001/.01/.0625
// отклонены при обоих ground flags. Выбранная малая граница согласуется с
// этими точками; точный порог между ними и другие формы [проверить].
const VERTICAL_EPSILON: f64 = 1.0e-7;
// Граница объёма работы, а не измеренный предел скорости vanilla [проверить].
const MAX_DESCENT: f64 = 4.0;

type Point = (f64, f64, f64);

#[derive(Debug, PartialEq, Eq)]
enum Crossing { Clear, Blocked, Contact, Uncovered, ExistingOverlap }

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Decision { Legacy, Reject, PreserveGround }

/// None — форма вне этого среза; Some(None) — известный воздух.
fn shape(catalog: Catalog, state: i32) -> Option<Option<(f64, f64)>> {
    if let Ok(Some((half, false))) = catalog.slab_properties(state) {
        return Some(Some(match half {
            SlabHalf::Bottom => (0.0, 0.5),
            SlabHalf::Top => (0.5, 1.0),
            SlabHalf::Double => (0.0, 1.0),
        }));
    }
    // Цельные блоки серы и киновари — полный куб (стенки, шипы и прочее — вне этого среза).
    if catalog.block_entry(state).ok().flatten().is_some_and(|row| matches!(row.form, crate::gameplay_catalog::BlockForm::Full | crate::gameplay_catalog::BlockForm::PotentSulfur)) {
        return Some(Some((0.0, 1.0)));
    }
    let name = blocks::block_at_state(state)?;
    if matches!(name, "air" | "cave_air" | "void_air") { return Some(None); }
    if name == "stone" { return Some(Some((0.0, 1.0))); }
    let colored = |suffix| name.strip_suffix(suffix).is_some_and(|color| matches!(color,
        "white" | "orange" | "magenta" | "light_blue" | "yellow" | "lime" | "pink" | "gray"
        | "light_gray" | "cyan" | "purple" | "blue" | "brown" | "green" | "red" | "black"));
    if colored("_wool") { Some(Some((0.0, 1.0))) }
    else if colored("_carpet") { Some(Some((0.0, 1.0 / 16.0))) }
    else { None }
}

/// Вся затронутая площадь и высота должны быть загружены. Ни генерации, ни
/// представления неизвестного чанка воздухом; неизвестная форма оставляет старый путь.
fn crossing(world: &World, from: Point, to: Point, height: f64) -> Crossing {
    let low_x = (from.0 - HALF + EDGE_EPSILON).floor() as i32;
    let high_x = (from.0 + HALF - EDGE_EPSILON).floor() as i32;
    let low_z = (from.2 - HALF + EDGE_EPSILON).floor() as i32;
    let high_z = (from.2 + HALF - EDGE_EPSILON).floor() as i32;
    let mut blocked = false;
    let mut overlap = false;
    let mut contact = false;
    for x in low_x..=high_x {
        for z in low_z..=high_z {
            for y in to.1.floor() as i32..=(from.1 + height - VERTICAL_EPSILON).floor() as i32 {
                let Some(state) = world.get_block_loaded(x, y, z) else { return Crossing::Uncovered; };
                let Some(bounds) = shape(world.catalog(), state) else { return Crossing::Uncovered; };
                let Some((bottom, top)) = bounds else { continue; };
                let bottom = f64::from(y) + bottom;
                let top = f64::from(y) + top;
                overlap |= from.1 < top - VERTICAL_EPSILON && from.1 + height > bottom + VERTICAL_EPSILON;
                blocked |= from.1 >= top - VERTICAL_EPSILON && to.1 < top - VERTICAL_EPSILON;
                // Сохраняем ground лишь у малого шага с уже известного верха.
                // Диапазон начальной высоты и прочие формы — геометрическое
                // ограничение этого среза, не отдельное ordinary наблюдение.
                contact |= (from.1 - top).abs() <= VERTICAL_EPSILON
                    && to.1 < top && to.1 >= top - VERTICAL_EPSILON;
            }
        }
    }
    if overlap { Crossing::ExistingOverlap }
    else if blocked { Crossing::Blocked }
    else if contact { Crossing::Contact }
    else { Crossing::Clear }
}

pub(super) fn classify(shared: &Shared, player: &PlayerState, to: Point) -> Decision {
    let from = (player.x, player.y, player.z);
    if !matches!(player.game_mode, GAME_MODE_SURVIVAL | GAME_MODE_ADVENTURE)
        || player.creative_flying || player.vit.dead || player.glide.active
        || player.net.spin_pose || player.net.spin_since.is_some()
        || player.net.in_water || player.net.in_lava
        || from.0 != to.0 || from.2 != to.2 || to.1 >= from.1 || from.1 - to.1 > MAX_DESCENT
    { return Decision::Legacy; }
    let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    match crossing(&world, from, to, if player.sneaking { 1.5 } else { 1.8 }) {
        Crossing::Blocked => Decision::Reject,
        // Ordinary24 проверял уже стоящего игрока: это сохранение ground,
        // не вычисление приземления из воздуха или неподвижного пакета.
        Crossing::Contact if player.on_ground => Decision::PreserveGround,
        _ => Decision::Legacy,
    }
}

#[cfg(test)]
#[path = "terrain_floor_tests.rs"]
mod tests;
