//! Подушка 26.3 (cushion): неподвижная сущность 0,999 × 0,249 без ИИ и столкновений.
//!
//! Вики (Cushion, 26.3): ставится на любую поверхность (в том числе верх плит и ступеней) по сетке
//! блоков; ломается, если убрать опору или толкнуть поршнем; на ней можно сидеть (использовать);
//! любой урон ломает её сразу, а предмет выпадает всегда; цвет — по умолчанию белый.
//! Здесь: размещение, поломка без опоры/по удару, предмет по цвету. Сидение — `riding` (capacity 1).
//! [проверить] у официального сервера: звук и частицы поломки, толкание поршнем, разрушение лавой
//! «через несколько секунд», запрет ломать в режиме приключения, перекраска красителем, топливо в печи.
use super::{Kind, Mob, newmobs};
use crate::survival::{Hit, Source};

/// Предмет подушки по номеру цвета (порядок красителей, как у овец).
pub fn item_name(color: u8) -> String {
    format!("{}_cushion", newmobs::COLOURS[usize::from(color.min(15))])
}

/// Номер цвета по предмету «<цвет>_cushion».
pub fn color_of_item(name: &str) -> Option<u8> {
    let colour = name.strip_suffix("_cushion")?;
    newmobs::COLOURS.iter().position(|c| *c == colour).map(|i| i as u8)
}

/// Любой урон ломает подушку сразу; предмет выпадает (см. `Mob::die`).
pub fn hurt(mob: &mut Mob, _amount: f32, _source: Source) -> Option<Hit> {
    if mob.is_dead() { return None; }
    mob.vitals.health = 0.0;
    mob.vitals.dead = true;
    mob.die();
    Some(Hit { taken: 0.0, absorbed: 0.0, resisted: 0.0, died: true })
}

/// Опора под подушкой: блок с коробкой столкновения под её низом (верх плиты или ступени тоже годится).
pub fn supported(view: &dyn super::View, mob: &Mob) -> bool {
    let (x, z) = (mob.x.floor() as i32, mob.z.floor() as i32);
    crate::blocks::has_box(view.block(x, (mob.y - 0.001).floor() as i32, z))
}

/// Поставить подушку из предмета на клетку `pos` (над кликнутым блоком), по высоте верха опоры.
/// Возвращает false, если клетка занята блоком или другой подушкой.
pub fn place(world: &mut crate::world::World, items: &mut crate::items::Items, shared: &crate::shared::Shared, support: (i32, i32, i32), color: u8) -> bool {
    let (x, y, z) = support;
    // Верх опоры: нижняя плита — +0,5, остальное — +1 [проверить] ступени, снег и прочие формы.
    let state = world.get_block(x, y, z);
    let slab = crate::blocks::orientation_of(state).is_some_and(|block| block.family == crate::blocks::Family::Slab && block.value(state, "type") == Some("bottom"));
    let top = if slab { 0.5 } else { 1.0 };
    let at = (f64::from(x) + 0.5, f64::from(y) + top, f64::from(z) + 0.5);
    if !slab && crate::blocks::has_box(world.get_block(x, y + 1, z)) { return false; }
    if items.mobs().iter().any(|m| m.kind == Kind::Cushion && !m.is_dead() && (m.x - at.0).abs() < 0.999 && (m.z - at.2).abs() < 0.999 && (m.y - at.1).abs() < 0.249) { return false; }
    let mut mob = Mob::new(shared.next_entity_id(), Kind::Cushion, at);
    mob.color = color;
    items.add_mob(mob);
    true
}
