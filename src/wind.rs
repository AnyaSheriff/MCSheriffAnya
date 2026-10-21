// Заряды ветра (вики, Wind Charge; Breeze): что делает взрыв ветра с
// блоками и обо что заряд ударяется.
//
// Кода игры здесь нет: правила — со страниц minecraft.wiki и из чёрного
// ящика (сценарии wind-charge-throw, breeze-charge-flight):
//
// - заряд летит по прямой — без тяжести и без сопротивления (breeze-charge-flight:
//   скорость 0,4 из z 1,5 о дверь на 6,8125 — ровно на 14-м ходу);
// - удар о блок — по лучу из центра заряда о настоящую форму блока (дверь
//   тонкая: 3/16), взрыв — на 0,25 от грани наружу (там же: центр 6,5625);
// - сила взрыва: у заряда игрока 1,2, у бриза 3 (пакет взрыва); толкает в
//   радиусе вдвое больше (вики: «2.4-block sphere», «6-block sphere»), ран
//   нет, блоки не рушатся, а «переключаются»: деревянные двери, люки и
//   калитки, рычаги, кнопки, колокола; свечи гаснут (вики, «Block
//   interactions»). Железные двери и люки — нет.

use crate::blocks;
use crate::tnt::Pos;
use crate::world::World;

/// Сила взрыва заряда игрока и бриза (чёрный ящик: «сила 1,2» и «сила 3»).
pub const CHARGE_POWER: f32 = 1.2;
pub const BREEZE_POWER: f32 = 3.0;

/// Толчок заряда игрока сильнее бризового на 22 % (вики, Wind Charge:
/// «22% more knockback») [проверить множитель бриза: 1].
pub const CHARGE_KNOCKBACK: f64 = 1.22;
pub const BREEZE_KNOCKBACK: f64 = 1.0;

/// Скорость брошенного заряда (wind-charge-throw: 1,5 по взгляду, разброс
/// как у снежка [проверить]).
pub const THROW_SPEED: f64 = 1.5;

/// Откат предмета после броска: 10 тактов (вики, Wind Charge: «0.5 seconds»).
pub const COOLDOWN: i32 = 10;

/// Урон прямого попадания (вики, Wind Charge: «1HP»; breeze-wind-charge:
/// здоровье 20 → 19).
pub const HIT_DAMAGE: f32 = 1.0;

/// Взрыв встаёт на 0,25 от грани, о которую ударился заряд.
const FACE_OFFSET: f64 = 0.25;

/// Свойство состояния и состояние с другим свойством.
fn prop(state: i32, key: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, key)
}

fn with(state: i32, key: &str, value: &str) -> Option<i32> {
    blocks::orientation_of(state)?.with(state, key, value)
}

/// Рамка внутри клетки: от угла (доли блока).
pub(crate) type Corners = ((f64, f64, f64), (f64, f64, f64));

const THIN: f64 = 3.0 / 16.0;

/// Обо что в клетке ударяется заряд: полный блок — куб, дверь и люк — своя
/// тонкая плита, прочее неполное — насквозь [проверить: ступени, плиты,
/// заборы и остальные формы].
fn boxes_with_catalog(state: i32, catalog: crate::gameplay_catalog::Catalog) -> Vec<Corners> {
    use crate::gameplay_catalog::SlabHalf;
    if let Ok(Some((half, _))) = catalog.slab_properties(state) {
        let (low, high) = match half { SlabHalf::Top => (0.5, 1.0), SlabHalf::Bottom => (0.0, 0.5), SlabHalf::Double => (0.0, 1.0) };
        return vec![((0.0, low, 0.0), (1.0, high, 1.0))];
    }
    boxes(state)
}

fn boxes(state: i32) -> Vec<Corners> {
    let Some(name) = blocks::block_at_state(state) else {
        return Vec::new();
    };

    if name.ends_with("_door") || name.ends_with("_trapdoor") {
        let value = |key: &str| prop(state, key).unwrap_or_default();
        let facing = value("facing");
        let open = value("open") == "true";
        let panel = |side: &str| -> Corners {
            match side {
                // Плита у той грани, куда «смотрит» сторона.
                "north" => ((0.0, 0.0, 0.0), (1.0, 1.0, THIN)),
                "south" => ((0.0, 0.0, 1.0 - THIN), (1.0, 1.0, 1.0)),
                "west" => ((0.0, 0.0, 0.0), (THIN, 1.0, 1.0)),
                _ => ((1.0 - THIN, 0.0, 0.0), (1.0, 1.0, 1.0)),
            }
        };
        let opposite = |side: &str| match side {
            "north" => "south",
            "south" => "north",
            "west" => "east",
            _ => "west",
        };

        if name.ends_with("_trapdoor") {
            if open {
                return vec![panel(opposite(facing))];
            }

            return vec![if value("half") == "top" { ((0.0, 1.0 - THIN, 0.0), (1.0, 1.0, 1.0)) } else { ((0.0, 0.0, 0.0), (1.0, THIN, 1.0)) }];
        }

        // Закрытая дверь, смотрящая на север, — плита у южной грани
        // (breeze-charge-flight: удар о z 6,8125). Открытая — повёрнута к
        // петле [проверить].
        if !open {
            return vec![panel(opposite(facing))];
        }

        let turned = match (facing, value("hinge") == "right") {
            ("north", false) | ("south", true) => "east",
            ("north", true) | ("south", false) => "west",
            ("east", false) | ("west", true) => "south",
            _ => "north",
        };

        return vec![panel(opposite(turned))];
    }

    if crate::mob::solid(state) {
        return vec![((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))];
    }

    Vec::new()
}

/// Где луч `from` → `to` входит в рамку: доля пути и нормаль грани.
fn enter(from: (f64, f64, f64), step: (f64, f64, f64), (low, high): Corners) -> Option<(f64, (i32, i32, i32))> {
    let (mut near, mut far) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut normal = (0, 0, 0);
    let axes = [(from.0, step.0, low.0, high.0), (from.1, step.1, low.1, high.1), (from.2, step.2, low.2, high.2)];

    for (axis, (start, delta, lo, hi)) in axes.into_iter().enumerate() {
        if delta.abs() < 1.0e-12 {
            if start < lo || start > hi {
                return None;
            }

            continue;
        }

        let (a, b) = ((lo - start) / delta, (hi - start) / delta);
        let (t0, t1) = if a < b { (a, b) } else { (b, a) };

        if t0 > near {
            near = t0;
            let sign = if delta > 0.0 { -1 } else { 1 };

            normal = match axis {
                0 => (sign, 0, 0),
                1 => (0, sign, 0),
                _ => (0, 0, sign),
            };
        }

        far = far.min(t1);
    }

    (near <= far && (0.0..=1.0).contains(&near)).then_some((near, normal))
}

/// Первый блок на пути центра заряда: доля пути, клетка, нормаль грани.
pub fn first_hit(world: &World, from: (f64, f64, f64), to: (f64, f64, f64)) -> Option<(f64, Pos, (i32, i32, i32))> {
    let step = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let (lo, hi) = ((from.0.min(to.0), from.1.min(to.1), from.2.min(to.2)), (from.0.max(to.0), from.1.max(to.1), from.2.max(to.2)));
    let mut best: Option<(f64, Pos, (i32, i32, i32))> = None;

    // Путь за такт короткий (до 1,5 блока): клеток вдоль него немного.
    for x in lo.0.floor() as i32..=hi.0.floor() as i32 {
        for y in lo.1.floor() as i32..=hi.1.floor() as i32 {
            for z in lo.2.floor() as i32..=hi.2.floor() as i32 {
                for (a, b) in boxes_with_catalog(world.get_block(x, y, z), world.catalog()) {
                    let corners = ((f64::from(x) + a.0, f64::from(y) + a.1, f64::from(z) + a.2), (f64::from(x) + b.0, f64::from(y) + b.1, f64::from(z) + b.2));

                    if let Some((part, normal)) = enter(from, step, corners)
                        && best.is_none_or(|(known, _, _)| part < known)
                    {
                        best = Some((part, (x, y, z), normal));
                    }
                }
            }
        }
    }

    best
}

/// Центр взрыва у грани: точка удара, сдвинутая наружу на 0,25.
pub fn burst_point(at: (f64, f64, f64), normal: (i32, i32, i32)) -> (f64, f64, f64) {
    (at.0 + f64::from(normal.0) * FACE_OFFSET, at.1 + f64::from(normal.1) * FACE_OFFSET, at.2 + f64::from(normal.2) * FACE_OFFSET)
}

/// Что взрыв ветра делает с блоком `pos` (вики, Wind Charge, «Block
/// interactions»). `touched` — двери, уже открытые этим взрывом: вторая
/// половина второй раз не переключает (wind-charge-throw: дверь открыта).
/// `griefing` — правило mobGriefing для заряда бриза: без него бриз
/// только звонит в колокола (вики, Breeze).
pub fn interact(world: &mut World, pos: Pos, touched: &mut Vec<Pos>, griefing: bool) {
    let state = world.get_block(pos.0, pos.1, pos.2);
    let Some(name) = blocks::block_at_state(state) else {
        return;
    };

    if name == "bell" {
        let facing = prop(state, "facing").unwrap_or_default();
        // Колокол качается в сторону, куда смотрит (wind-charge-throw:
        // смотрящий на север — событие 1/2) [проверить: у висячих].
        let face = match facing {
            "south" => 3,
            "west" => 4,
            "east" => 5,
            _ => 2,
        };

        crate::redstone::use_block_on(world, pos, face);
        return;
    }

    if !griefing {
        return;
    }

    let iron = name.starts_with("iron_");
    let door = name.ends_with("_door");

    if (door || name.ends_with("_trapdoor") || name.ends_with("_fence_gate")) && !iron {
        if door {
            let other = if prop(state, "half") == Some("lower") { (pos.0, pos.1 + 1, pos.2) } else { (pos.0, pos.1 - 1, pos.2) };

            if touched.contains(&other) {
                return;
            }

            touched.push(pos);
        }

        crate::redstone::use_block(world, pos);
        crate::redstone::settle(world);
        return;
    }

    if name == "lever" {
        crate::redstone::use_block(world, pos);
        crate::redstone::settle(world);
        return;
    }

    if name.ends_with("_button") {
        if prop(state, "powered") == Some("true") {
            return;
        }

        crate::redstone::use_block(world, pos);
        crate::redstone::settle(world);
        world.note_sound(pos.0, pos.1, pos.2, crate::redstone::button_click_on(name), 1.0, 1.0);
        return;
    }

    // Свечи (и на торте) гаснут.
    if (name.ends_with("candle") || name.ends_with("candle_cake"))
        && prop(state, "lit") == Some("true")
        && let Some(out) = with(state, "lit", "false")
    {
        world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.candle.extinguish", 1.0, 1.0);
        world.set_block(pos.0, pos.1, pos.2, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_door_facing_north_is_hit_on_its_south_plate() {
        let door = blocks::state_from_text("oak_door[facing=north,half=lower,open=false]").expect("дверь");
        let found = boxes(door);

        assert_eq!(found, vec![((0.0, 0.0, 1.0 - THIN), (1.0, 1.0, 1.0))]);
    }

    #[test]
    fn a_ray_enters_the_plate_from_the_north() {
        let plate = ((0.0, 0.0, 6.8125), (1.0, 2.0, 7.0));
        let (part, normal) = enter((0.5, 1.2, 6.5), (0.0, 0.0, 0.4), plate).expect("удар");

        assert!((part - 0.78125).abs() < 1.0e-9);
        assert_eq!(normal, (0, 0, -1));
        // Взрыв — на 0,25 наружу (breeze-charge-flight: 6,5625).
        assert_eq!(burst_point((0.5, 1.2, 6.8125), normal).2, 6.5625);
    }

    #[test]
    fn the_charge_knockback_is_stronger_than_the_breeze() {
        const { assert!(CHARGE_KNOCKBACK > BREEZE_KNOCKBACK) };
        assert_eq!(2.0 * CHARGE_POWER, 2.4);
    }
}

#[cfg(test)]
mod catalog_boxes_tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog,Profile,SlabHalf};
    #[test]
    fn preview_ray_boxes_match_selected_six_state_json() {
        let catalog = Catalog::new(Profile::Preview26_3);
        let source: serde_json::Value = serde_json::from_str(include_str!("gameplay_catalog/white-concrete-slab.json")).unwrap();
        for (index,half) in [SlabHalf::Top,SlabHalf::Bottom,SlabHalf::Double].into_iter().enumerate() {
            for (wet_index,wet) in [true,false].into_iter().enumerate() {
                let boxes = boxes_with_catalog(catalog.slab_state(half,wet).unwrap(),catalog);
                let (a,b)=boxes[0];
                let actual = serde_json::json!([[a.0,a.1,a.2,b.0,b.1,b.2]]);
                assert_eq!(actual,source["collision_by_state"][index*2+wet_index]);
            }
        }
    }
}
