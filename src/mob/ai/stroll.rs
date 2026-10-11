// Случайная прогулка по суше, отдельно от погони, приманки и паники.
// Вики, Cow (oldid=3454235): «avoid water unless following a player»;
// Mob AI (oldid=3455147): «preferring land». https://minecraft.wiki/w/Mob_AI
// [проверить] Вики не перечисляет вариант цели для каждого вида: список
// наземных видов и выбор десяти точек требуют сверки чёрным ящиком.

use super::{Kind, Rng, View, path, settle};
use path::Cell;

fn avoids_water(kind: Kind) -> bool {
    matches!(kind,
        Kind::Cow | Kind::Mooshroom | Kind::Pig | Kind::Sheep | Kind::Chicken
        | Kind::Rabbit | Kind::Fox | Kind::Wolf | Kind::Cat | Kind::Ocelot
        | Kind::Horse | Kind::Donkey | Kind::Mule | Kind::SkeletonHorse | Kind::ZombieHorse
        | Kind::Llama | Kind::TraderLlama | Kind::Camel | Kind::Goat | Kind::PolarBear
        | Kind::Panda | Kind::Armadillo | Kind::Sniffer
        | Kind::Zombie | Kind::ZombieVillager | Kind::Husk | Kind::Skeleton
        | Kind::Stray | Kind::Bogged | Kind::WitherSkeleton | Kind::Creeper
        | Kind::Spider | Kind::CaveSpider | Kind::Silverfish | Kind::Endermite
        | Kind::Enderman | Kind::Witch | Kind::ZombifiedPiglin | Kind::Piglin
        | Kind::PiglinBrute | Kind::Hoglin | Kind::Zoglin
        | Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner
        | Kind::Ravager | Kind::WanderingTrader)
}

fn land(view: &dyn View, (x, y, z): Cell) -> bool {
    path::standable(view, (x, y, z))
        && !path::water_cell(view, (x, y, z))
        && !super::super::is_water(view.block(x, y + 1, z))
}

pub(in crate::mob) fn route(kind: Kind, view: &dyn View, here: Cell, rng: &mut Rng) -> Vec<Cell> {
    let dry = avoids_water(kind);
    let escaping = dry && path::water_cell(view, here);

    for _ in 0..10 {
        let dx = rng.below(21) as i32 - 10;
        let dz = rng.below(21) as i32 - 10;
        // В воде ищем сверху: settle только опускает точку, иначе берег
        // выше ног утонувшего моба никогда не попадёт в кандидаты [проверить].
        let dy = if escaping { 7 } else { rng.below(15) as i32 - 7 };
        let target = settle(view, (here.0 + dx, here.1 + dy, here.2 + dz), here.1 + dy);
        if !path::standable(view, target) || dry && !land(view, target) {
            continue;
        }
        let route = path::find_for(view, here, target, path::MAX_NODES, kind);
        // A* может вернуть лишь часть пути: такая прогулка тоже не должна
        // кончаться в воде, даже если исходная цель была на другом берегу.
        if route.last().is_some_and(|&end| !dry || land(view, end)) {
            return route;
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests;
