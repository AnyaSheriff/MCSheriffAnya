// Вики, Cow: «flees for a few seconds»; Chicken: «flee around».
// https://minecraft.wiki/w/Cow?oldid=3454235
// https://minecraft.wiki/w/Chicken?oldid=3509347
// Паника от урона выбирает случайную цель, а уход от замеченного врага —
// точку прочь от него (Mob fleeing). Это разные цели.

use super::{Doing, Kind, Mob, Rng, View, path, settle};
use crate::survival::Source;

// [проверить] ±5 по горизонтали, ±4 по вертикали и десять попыток:
// в проверенной редакции Mob AI (3455147) точных диапазонов нет.
const HORIZONTAL: i32 = 5;
const VERTICAL: i32 = 4;

pub(in crate::mob) fn tick(mob: &mut Mob) {
    mob.brain.panic = (mob.brain.panic - 1).max(0);
}

/// Вики, Java Edition 24w21a: «predefined damage source types».
/// https://minecraft.wiki/w/Damage_type_tag_(Java_Edition)?oldid=3498996#panic_causes
fn environmental(source: Source) -> bool {
    matches!(source, Source::Cactus | Source::Freeze | Source::HotFloor
        | Source::InFire | Source::Lava | Source::Lightning | Source::OnFire)
}

pub(in crate::mob) fn on_hurt(mob: &Mob, source: Source) -> bool {
    let passive = matches!(source, Source::PlayerAttack | Source::MaceSmash | Source::Spear
        | Source::Arrow | Source::DragonBreath | Source::Explosion | Source::Fireball
        | Source::LargeFireball | Source::Fireworks | Source::IndirectMagic | Source::Magic
        | Source::MobAttack | Source::MobProjectile | Source::PlayerExplosion | Source::SonicBoom
        | Source::Sting | Source::Thrown | Source::Trident | Source::UnattributedFireball
        | Source::WindCharge | Source::Wither | Source::WitherSkull) || environmental(source);
    match mob.kind {
        // Вики, 24w21a: «Pandas, wolves and adult polar bears» — только
        // environmental; детёныш белого медведя не отвечает на удар.
        Kind::Wolf | Kind::Panda => environmental(source),
        Kind::PolarBear if !mob.is_baby() => environmental(source),
        Kind::Cow | Kind::Mooshroom | Kind::Sheep | Kind::Pig | Kind::Chicken
        | Kind::Horse | Kind::Donkey | Kind::Mule | Kind::Llama | Kind::TraderLlama
        | Kind::Rabbit | Kind::Fox | Kind::Cat | Kind::Ocelot | Kind::PolarBear
        | Kind::Goat | Kind::Camel | Kind::Turtle | Kind::Strider | Kind::Sniffer
        | Kind::Parrot | Kind::Allay | Kind::Axolotl | Kind::Frog | Kind::Tadpole => passive,
        _ => false,
    }
}

pub(in crate::mob) fn burning(mob: &mut Mob) {
    if mob.vitals.fire > 0 && on_hurt(mob, Source::OnFire) {
        if !mob.brain.is_panicking() { mob.brain.panic(1); }
        // Горит и между тиками урона; не бросать путь каждый такт.
        mob.brain.panic = mob.brain.panic.max(1);
        mob.brain.eating = 0;
    }
}

pub(super) fn route(mob: &Mob, view: &dyn View, rng: &mut Rng) -> Vec<path::Cell> {
    let here = mob.cell();
    // Огонь: ищем доступную воду. [проверить] Радиус и порядок выбора воды.
    if mob.vitals.fire > 0 {
        let mut water = Vec::new();
        for dx in -HORIZONTAL..=HORIZONTAL {
            for dz in -HORIZONTAL..=HORIZONTAL {
                for dy in -VERTICAL..=VERTICAL {
                    let at = (here.0 + dx, here.1 + dy, here.2 + dz);
                    if super::super::is_water(view.block(at.0, at.1, at.2)) && path::standable(view, at) {
                        water.push((dx * dx + dy * dy + dz * dz, at));
                    }
                }
            }
        }
        water.sort_unstable();
        for (_, at) in water {
            let route = path::find_for(view, here, at, path::MAX_NODES, mob.kind);
            if route.last() == Some(&at) { return route; }
        }
    }
    for _ in 0..10 {
        let dx = rng.below((2 * HORIZONTAL + 1) as u32) as i32 - HORIZONTAL;
        let dz = rng.below((2 * HORIZONTAL + 1) as u32) as i32 - HORIZONTAL;
        let dy = rng.below((2 * VERTICAL + 1) as u32) as i32 - VERTICAL;
        let at = (here.0 + dx, here.1 + dy, here.2 + dz);
        if at == here || !path::standable(view, at) { continue; }
        let route = path::find_for(view, here, at, path::MAX_NODES, mob.kind);
        if !route.is_empty() { return route; }
    }
    Vec::new()
}

pub(super) fn flee_route(mob: &Mob, view: &dyn View, rng: &mut Rng) -> Vec<path::Cell> {
    let here = mob.cell();
    let (ax, az) = mob.fright.unwrap_or((mob.x, mob.z));
    let (dx, dz) = (mob.x - ax, mob.z - az);
    let len = dx.hypot(dz);
    let (ux, uz) = if len > 1.0e-3 { (dx / len, dz / len) } else {
        let angle = rng.unit() * std::f64::consts::TAU;
        (angle.cos(), angle.sin())
    };
    let jitter = (rng.unit() - 0.5) * 4.0;
    let target = (here.0 + (ux * 6.0 + jitter).round() as i32, here.1, here.2 + (uz * 6.0 - jitter).round() as i32);
    path::find_for(view, here, settle(view, target, here.1), path::MAX_NODES, mob.kind)
}

/// Наземные животные прерывают сидение, еду и бой при панике от среды.
pub(in crate::mob) fn active(mob: &Mob) -> bool {
    !mob.brain.fleeing && (mob.brain.panic > 0 || mob.brain.doing == Doing::Panic && mob.brain.walking())
}
