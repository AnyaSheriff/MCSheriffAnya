// Вики, Mob spawning: «must not collide with any liquid»; обводнённые
// блоки тоже содержат воду. Проверяем весь ящик, включая соседние колонки.
use crate::{blocks, fluids};
use super::{Kind, Mob, View, solid};

pub fn has_fluid(state: i32) -> bool {
    super::super::is_water(state) || fluids::source_at(state).is_some()
}

/// Водное размещение, а не умение плавать: лягушки и черепахи — на суше.
/// Вики, Mob spawning / Frog / Turtle, «Spawning».
pub fn in_water(kind: Kind) -> bool {
    matches!(kind, Kind::Cod | Kind::Salmon | Kind::TropicalFish | Kind::Pufferfish
        | Kind::Squid | Kind::GlowSquid | Kind::Dolphin | Kind::Axolotl
        | Kind::Tadpole | Kind::Guardian | Kind::ElderGuardian | Kind::Drowned)
}

fn clear(view: &dyn View, at: (f64, f64, f64), size: (f64, f64), allowed: Option<fluids::Kind>) -> bool {
    let (width, height) = size;
    let half = width / 2.0;
    for x in (at.0 - half).floor() as i32..=(at.0 + half - 1.0e-7).floor() as i32 {
        for z in (at.2 - half).floor() as i32..=(at.2 + half - 1.0e-7).floor() as i32 {
            if !view.loaded(x.div_euclid(16), z.div_euclid(16)) { return false; }
            for y in at.1.floor() as i32..=(at.1 + height - 1.0e-7).floor() as i32 {
                let state = view.block(x, y, z);
                if solid(state) { return false; }
                let fluid = if super::super::is_water(state) { Some(fluids::Kind::Water) }
                    else { fluids::source_at(state).map(|(kind, _)| kind) };
                if fluid.is_some() && fluid != allowed { return false; }
            }
        }
    }
    true
}

pub fn dry(view: &dyn View, at: (f64, f64, f64), size: (f64, f64)) -> bool {
    clear(view, at, size, None)
}

pub fn dry_kind(view: &dyn View, kind: Kind, at: (f64, f64, f64)) -> bool {
    dry(view, at, (kind.width(), kind.height()))
}

/// Правило жидкости не подменяет проверку биома, пола или глубины воды.
pub fn fits(view: &dyn View, mob: &Mob) -> bool {
    let allowed = if in_water(mob.kind) { Some(fluids::Kind::Water) }
        else if mob.kind == Kind::Strider { Some(fluids::Kind::Lava) } else { None };
    clear(view, (mob.x, mob.y, mob.z), mob.size(), allowed)
}

/// Спавнеры: сохраняем прежнюю проверку пола для сухопутных [проверить:
/// Monster Spawner допускает появление части видов в воздухе].
pub fn spawner(view: &dyn View, mob: &Mob) -> bool {
    let (x, y, z) = mob.cell();
    let medium = if in_water(mob.kind) { super::super::is_water(view.block(x, y, z)) }
        else if mob.kind == Kind::Strider { super::super::is_lava(view.block(x, y, z)) }
        else { blocks::solid_top(view.block(x, y - 1, z)) };
    medium && fits(view, mob)
}
