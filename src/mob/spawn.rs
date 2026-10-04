// Естественное появление животных в только что созданных чанках.
//
// Вики (Spawn, «Chunk generation»): животные появляются при создании чанка —
// с шансом 10 % на чанк выбирается вид по весу, и вся группа встаёт кучкой
// на траву со светом не ниже 9. У овцы вес 12 (группа 2–4), у свиньи 10
// (2–4), у курицы 10 (4), у коровы 8 (2–4). Здесь простая версия:
//
// - биом не учитывается [проверить: у оригинала виды и веса зависят от биома,
//   в пустынях и океанах животных нет];
// - свет — «над клеткой небо открыто» (снизу вверх нет твёрдого) [проверить:
//   листва рассеивает свет по одному за блок, а не закрывает целиком];
// - овцы получают цвет по вики (Sheep, «Spawning»: белая 81,836 %, чёрная,
//   серая и светло-серая по 5 %, коричневая 3 %, розовая 0,164 %).

use super::{Kind, Mob, Rng, View, solid};
use crate::blocks;

/// Шанс группы на новый чанк (вики, Spawn: «10 % chance per chunk»).
pub const CHUNK_CHANCE: f64 = 0.1;

/// Виды и веса (вики, Spawn, «Plains»).
const WEIGHTS: [(Kind, u32); 4] = [(Kind::Sheep, 12), (Kind::Pig, 10), (Kind::Chicken, 10), (Kind::Cow, 8)];

/// Размер группы: (наименьшая, наибольшая).
fn group(kind: Kind) -> (u32, u32) {
    match kind {
        Kind::Chicken => (4, 4),
        _ => (2, 4),
    }
}

/// Цвет новой овцы по весам вики (доли из 10000).
pub fn wild_colour(rng: &mut Rng) -> u8 {
    let roll = rng.below(10_000);

    // Белая 81,836 %; чёрная 5 %, серая 5 %, светло-серая 5 %, коричневая 3 %,
    // розовая 0,164 %. Номера цветов — как в `mob::WOOL`.
    match roll {
        0..500 => 15,
        500..1000 => 7,
        1000..1500 => 8,
        1500..1800 => 12,
        1800..1816 => 6,
        _ => 0,
    }
}

/// Верхняя твёрдая клетка колонки и есть ли над ней открытое небо: верх поверхности.
fn surface(view: &dyn View, x: i32, z: i32) -> Option<i32> {
    let mut y = 319;

    while y > -64 {
        let state = view.block(x, y, z);

        if solid(state) || super::is_water(state) || super::is_lava(state) {
            return (blocks::block_at_state(state) == Some("grass_block")).then_some(y);
        }

        y -= 1;
    }

    None
}

/// Группа животных для нового чанка (пустая — не повезло или негде).
pub fn for_new_chunk(view: &dyn View, chunk_x: i32, chunk_z: i32, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    if rng.unit() >= CHUNK_CHANCE {
        return Vec::new();
    }

    let total: u32 = WEIGHTS.iter().map(|(_, weight)| weight).sum();
    let mut pick = rng.below(total);
    let mut kind = Kind::Sheep;

    for (candidate, weight) in WEIGHTS {
        if pick < weight {
            kind = candidate;
            break;
        }

        pick -= weight;
    }

    let (low, high) = group(kind);
    let want = low + rng.below(high - low + 1);
    let (base_x, base_z) = (chunk_x * 16 + rng.below(16) as i32, chunk_z * 16 + rng.below(16) as i32);
    let mut mobs = Vec::new();
    let colour = wild_colour(rng);

    for _ in 0..want * 4 {
        if mobs.len() as u32 >= want {
            break;
        }

        // Группа стоит кучкой в пределах чанка: разброс до 5 блоков от точки,
        // за край чанка не выходит.
        let x = (base_x + rng.below(11) as i32 - 5).clamp(chunk_x * 16, chunk_x * 16 + 15);
        let z = (base_z + rng.below(11) as i32 - 5).clamp(chunk_z * 16, chunk_z * 16 + 15);

        let Some(y) = surface(view, x, z) else {
            continue;
        };

        // Над травой должно быть место в два блока и открытое небо.
        if (y + 1..=y + 2).any(|above| solid(view.block(x, above, z))) || (y + 3..320).any(|above| solid(view.block(x, above, z))) {
            continue;
        }

        let at = (f64::from(x) + 0.5, f64::from(y + 1), f64::from(z) + 0.5);
        let mut mob = Mob::new(next_id(), kind, at);
        let yaw = (rng.unit() * 360.0) as f32;

        mob.face(yaw);

        if kind == Kind::Sheep {
            mob.color = colour;
        }

        if kind == Kind::Chicken {
            mob.egg_timer = 6000 + rng.below(6001) as i32;
        }

        mobs.push(mob);
    }

    mobs
}

// ---------------------------------------------------------------------------
// Враждебные: появление в темноте
// ---------------------------------------------------------------------------
//
// Вики (Mob spawning, «Java Edition: hostile»): попытка выбирает случайное
// место рядом с игроком, не ближе 24 блоков ни к одному игроку и не дальше
// 128; под ногами — твёрдый блок с полной верхней гранью, в клетке и над ней
// воздух; блочный свет 0, а небесный не больше случайного числа от 0 до 7
// («a spawn only occurs if the light level is less than or equal to a random
// number between 0 and 7»); всего враждебных — не больше 70. Группа — до 4
// мобов в пределах 5 блоков. Веса видов: зомби 95, скелет 100, крипер 100,
// паук 100. Что не сверено: доля попыток за такт и вклад биомов [проверить].

/// Предел враждебных на измерение (вики, Mob spawning: «70»).
pub const MONSTER_CAP: usize = 70;

/// Ближе этого к игроку не появляются, и дальше него — тоже (вики).
pub const MIN_DISTANCE: f64 = 24.0;
pub const MAX_DISTANCE: f64 = 128.0;

/// Виды и веса враждебных: эндермен 10, ведьма 5 (вики, Enderman и Witch:
/// «10⁄515», «5»).
pub const HOSTILE: [(Kind, u32); 6] = [(Kind::Zombie, 95), (Kind::Skeleton, 100), (Kind::Creeper, 100), (Kind::Spider, 100), (Kind::Enderman, 10), (Kind::Witch, 5)];

/// Энд: одни эндермены (вики, Enderman, «Spawning»).
pub const END_HOSTILE: [(Kind, u32); 1] = [(Kind::Enderman, 10)];

/// Сколько мобов в группе не больше: ведьма одна (вики, Witch: «Group size: 1»),
/// прочие — до четырёх.
fn hostile_group(kind: Kind) -> u32 {
    if kind == Kind::Witch { 1 } else { 4 }
}

/// Годится ли клетка для появления: воздух в ногах и над головой, пол — целый.
pub fn spawnable(view: &dyn View, x: i32, y: i32, z: i32, height: i32) -> bool {
    let free = |cy: i32| {
        let state = view.block(x, cy, z);

        !solid(state) && !super::is_water(state) && !super::is_lava(state)
    };

    (0..height).all(|dy| free(y + dy)) && blocks::solid_top(view.block(x, y - 1, z))
}

/// Ставит вид по весам.
fn pick_hostile(kinds: &[(Kind, u32)], rng: &mut Rng) -> Kind {
    let total: u32 = kinds.iter().map(|(_, weight)| weight).sum();
    let mut roll = rng.below(total);

    for &(kind, weight) in kinds {
        if roll < weight {
            return kind;
        }

        roll -= weight;
    }

    Kind::Zombie
}

/// Свет, нужный при появлении: блочный 0 и небесный не выше случайного 0…7.
pub fn dark_enough(sky: u8, block: u8, rng: &mut Rng) -> bool {
    block == 0 && u32::from(sky) <= rng.below(8)
}

/// Одна попытка появления: место и группа; свет проверяет вызывающий (его
/// считает мир, `dark_enough`). `players` — где игроки.
pub fn hostile_attempt(
    view: &dyn View,
    players: &[(f64, f64, f64)],
    surface_of: &dyn Fn(i32, i32) -> i32,
    kinds: &[(Kind, u32)],
    rng: &mut Rng,
    next_id: &mut dyn FnMut() -> i32,
) -> Vec<Mob> {
    if players.is_empty() {
        return Vec::new();
    }

    let centre = players[rng.below(players.len() as u32) as usize];
    let reach = MAX_DISTANCE.min(96.0) as u32;
    let low = crate::world::MIN_Y + 1;

    // Оригинал пробует место в каждом чанке каждый такт; здесь за вызов —
    // десяток случайных мест, первое годное берётся (вики: высота случайна
    // от дна до поверхности, поэтому пещеры тоже в счёт).
    let mut found = None;

    for _ in 0..12 {
        let x = centre.0.floor() as i32 + rng.below(reach * 2 + 1) as i32 - reach as i32;
        let z = centre.2.floor() as i32 + rng.below(reach * 2 + 1) as i32 - reach as i32;

        if !view.loaded(x.div_euclid(16), z.div_euclid(16)) {
            continue;
        }

        let top = surface_of(x, z);

        if top <= low {
            continue;
        }

        let y = low + rng.below((top - low + 1) as u32) as i32;

        if spawnable(view, x, y, z, 2) {
            found = Some((x, y, z));
            break;
        }
    }

    let Some((x, y, z)) = found else {
        return Vec::new();
    };

    let kind = pick_hostile(kinds, rng);
    let (width, height) = (kind.width(), kind.height());
    let block_height = height.ceil() as i32;
    let far_enough = |px: f64, py: f64, pz: f64| {
        players.iter().all(|player| (player.0 - px).powi(2) + (player.1 - py).powi(2) + (player.2 - pz).powi(2) >= MIN_DISTANCE * MIN_DISTANCE)
            && players.iter().any(|player| (player.0 - px).powi(2) + (player.2 - pz).powi(2) < (MAX_DISTANCE - 8.0).powi(2))
    };
    let _ = width;
    let mut group = Vec::new();

    for _ in 0..hostile_group(kind) {
        // Разброс пачки: сумма двух равномерных — «треугольный», до 5 блоков.
        let (gx, gz) = (x + rng.below(6) as i32 - rng.below(6) as i32, z + rng.below(6) as i32 - rng.below(6) as i32);

        if !view.loaded(gx.div_euclid(16), gz.div_euclid(16)) || !spawnable(view, gx, y, gz, block_height) {
            continue;
        }

        let at = (f64::from(gx) + 0.5, f64::from(y), f64::from(gz) + 0.5);

        if !far_enough(at.0, at.1, at.2) {
            continue;
        }

        // Вики, Zombie Villager: «5% chance when regular zombies spawn»; у зомби-жителя
        // случайная профессия [проверить: тип по биому].
        let each = if kind == Kind::Zombie && rng.below(100) < 5 { Kind::ZombieVillager } else { kind };
        let mut mob = Mob::new(next_id(), each, at);

        if each == Kind::ZombieVillager {
            mob.villager.profession = rng.below(crate::villager::PROFESSIONS.len() as u32) as u8;
            mob.villager.finalized = true;
        }

        mob.face((rng.unit() * 360.0) as f32);
        group.push(mob);
    }

    group
}

/// Выбор вида по весам — для проверок.
#[cfg(test)]
pub fn pick_hostile_for_test(kinds: &[(Kind, u32)], rng: &mut Rng) -> Kind {
    pick_hostile(kinds, rng)
}
