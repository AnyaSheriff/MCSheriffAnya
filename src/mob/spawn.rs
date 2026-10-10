// Естественное появление животных в только что созданных чанках.
//
// Вики (Spawn, «Chunk generation»): животные появляются при создании чанка —
// с шансом 10 % на чанк выбирается вид по весу, и вся группа встаёт кучкой
// на траву со светом не ниже 9. У овцы вес 12 (группа 2–4), у свиньи 10
// (2–4), у курицы 10 (4), у коровы 8 (2–4). Здесь простая версия:
//
// - таблица биома учитывает кроликов и лис; прочие списки неполны [проверить];
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
        Kind::Rabbit => (2, 3),
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
#[cfg(test)]
pub fn for_new_chunk(view: &dyn View, chunk_x: i32, chunk_z: i32, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    for_new_chunk_in(view, chunk_x, chunk_z, "", rng, next_id)
}

/// Веса равнин и саванн — вики, Plains/Savanna, «Mobs». Отсутствующие
/// виды (броненосец, кролик) сохраняют долю, но пока не создаются.
pub fn equine_spawn(biome: &str) -> (u32, u32, u32, u32, u32) {
    match biome {
        "plains" | "sunflower_plains" => (5, 1, 1, 3, 40),
        "savanna" | "savanna_plateau" | "windswept_savanna" => (1, 1, 1, 1, 50),
        // [проверить] стадо ослов луга и вес остальных животных.
        "meadow" => (0, 1, 1, 2, 4),
        _ => (0, 0, 0, 0, 40),
    }
}

pub fn for_new_chunk_in(view: &dyn View, chunk_x: i32, chunk_z: i32, biome: &str, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    if rng.unit() >= CHUNK_CHANCE {
        return Vec::new();
    }

    let weights = passive_weights(biome);
    let total: u32 = weights.iter().map(|(_, weight)| weight).sum();
    // Кролики и лисы добавляют свой вес к прочим животным биома; в биомах без
    // прочих животных (пустыня, снежные равнины) остаются одни они.
    let wild: u32 = weights.iter().filter(|(kind, _)| matches!(kind, Kind::Rabbit | Kind::Fox)).map(|(_, weight)| weight).sum();
    let (horse_weight, donkey_weight, donkey_low, donkey_high, others) = equine_spawn(biome);
    let others = if total == wild { total } else { others + wild };
    if others + horse_weight + donkey_weight == 0 { return Vec::new(); }
    let mut pick = rng.below(others + horse_weight + donkey_weight);
    let chosen = if pick < horse_weight { Some(Kind::Horse) } else if pick < horse_weight + donkey_weight { Some(Kind::Donkey) } else { None };
    pick = pick.saturating_sub(horse_weight + donkey_weight);
    if chosen.is_none() && pick >= total { return Vec::new(); }
    let mut kind = Kind::Sheep;

    for (candidate, weight) in weights {
        if pick < weight {
            kind = candidate;
            break;
        }

        pick -= weight;
    }

    kind = chosen.unwrap_or(kind);
    let (low, high) = if kind == Kind::Horse { (2, 6) } else if kind == Kind::Donkey { (donkey_low, donkey_high) } else { group(kind) };
    let want = low + rng.below(high - low + 1);
    let (base_x, base_z) = (chunk_x * 16 + rng.below(16) as i32, chunk_z * 16 + rng.below(16) as i32);
    let mut mobs = Vec::new();
    let colour = wild_colour(rng);
    let coat = rng.below(7) as i32;

    for _ in 0..want * 4 {
        if mobs.len() as u32 >= want {
            break;
        }

        // Группа стоит кучкой в пределах чанка: разброс до 5 блоков от точки,
        // за край чанка не выходит.
        let x = (base_x + rng.below(11) as i32 - 5).clamp(chunk_x * 16, chunk_x * 16 + 15);
        let z = (base_z + rng.below(11) as i32 - 5).clamp(chunk_z * 16, chunk_z * 16 + 15);

        let Some(y) = animal_surface(view, x, z, kind) else {
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
        if super::horse::equine(kind) {
            super::horse::initialize(&mut mob, rng);
            mob.horse.variant = coat | ((rng.below(5) as i32) << 8);
            if rng.below(5) == 0 { mob.age = super::BABY_AGE; }
        }
        if super::wildlife::kind(kind) {
            super::wildlife::initialize(&mut mob, biome, rng);
            if kind == Kind::Fox && rng.below(20) == 0 { mob.age = super::BABY_AGE; }
            if kind == Kind::Rabbit && !mobs.is_empty() { mob.age = super::BABY_AGE; }
        }

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

/// Стая волков в новом чанке леса, тайги, рощи, плато саванны (вики, Wolf,
/// «Spawning»: стаей по 4, в саванне и бесплодных землях 4–8, в джунглях 2–4;
/// вариант — по биому). Доля среди животных — по весу вида против суммы весов
/// прочих (40) [проверить].
pub fn wolves_for_new_chunk(view: &dyn View, chunk_x: i32, chunk_z: i32, biome: &str, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    let Some((weight, low, high)) = super::tame::wolf_spawn(biome) else {
        return Vec::new();
    };

    if rng.unit() >= CHUNK_CHANCE * f64::from(weight) / f64::from(weight + 40) {
        return Vec::new();
    }

    let want = low + rng.below(high - low + 1);
    let (base_x, base_z) = (chunk_x * 16 + rng.below(16) as i32, chunk_z * 16 + rng.below(16) as i32);
    let variant = super::tame::wolf_variant_for(biome);
    let mut mobs = Vec::new();

    for _ in 0..want * 4 {
        if mobs.len() as u32 >= want {
            break;
        }

        let x = (base_x + rng.below(11) as i32 - 5).clamp(chunk_x * 16, chunk_x * 16 + 15);
        let z = (base_z + rng.below(11) as i32 - 5).clamp(chunk_z * 16, chunk_z * 16 + 15);

        let Some(y) = surface(view, x, z) else {
            continue;
        };

        if (y + 1..=y + 2).any(|above| solid(view.block(x, above, z))) {
            continue;
        }

        let mut wolf = Mob::new(next_id(), Kind::Wolf, (f64::from(x) + 0.5, f64::from(y + 1), f64::from(z) + 0.5));

        wolf.face((rng.unit() * 360.0) as f32);
        wolf.tame.variant = variant;
        mobs.push(wolf);
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
pub const HOSTILE: [(Kind, u32); 7] = [(Kind::Zombie, 95), (Kind::Skeleton, 100), (Kind::Creeper, 100), (Kind::Spider, 100), (Kind::Enderman, 10), (Kind::Witch, 5), (Kind::Slime, 100)];

/// Виды Обычного мира по биому и месту (вики, Husk: «Husks spawn ... in
/// deserts» вместо зомби, вес 80 против 19; Drowned: «rivers» вес 100,
/// «oceans» вес 5; в воде рождаются только утопленники) [проверить: веса
/// по вики, группа утопленника — один].
pub fn overworld_kinds(biome: &str, wet: bool) -> Vec<(Kind, u32)> {
    if wet {
        return match biome {
            "river" | "frozen_river" => vec![(Kind::Drowned, 100)],
            name if name.ends_with("ocean") => vec![(Kind::Drowned, 5)],
            _ => Vec::new(),
        };
    }

    if biome == "desert" {
        return HOSTILE.iter().map(|&(kind, weight)| if kind == Kind::Zombie { (Kind::Zombie, 19) } else { (kind, weight) }).chain([(Kind::Husk, 80)]).collect();
    }

    if super::skeleton::snowy_biome(biome) {
        return HOSTILE.iter().map(|&(kind, weight)| if kind == Kind::Skeleton { (kind, 20) } else { (kind, weight) }).chain([(Kind::Stray, 80)]).collect();
    }
    if matches!(biome, "swamp" | "mangrove_swamp") {
        return HOSTILE.iter().copied().chain([(Kind::Bogged, 50)]).collect();
    }
    HOSTILE.to_vec()
}

/// Годится ли клетка для утопленника: вода в ногах и над головой, пол — твёрдый.
pub fn wet_spot(view: &dyn View, x: i32, y: i32, z: i32) -> bool {
    super::is_water(view.block(x, y, z)) && super::is_water(view.block(x, y + 1, z)) && solid(view.block(x, y - 1, z))
}

/// Незер: виды и веса по биомам (вики, Mob spawning, «Nether»). Страйдеры
/// рождаются в лаве — естественного появления в лаве у нас пока нет
/// [проверить: Strider, «Spawning»].
pub fn nether_kinds(biome: &str) -> &'static [(Kind, u32)] {
    match biome {
        "soul_sand_valley" => &[(Kind::Skeleton, 20), (Kind::Ghast, 50), (Kind::Enderman, 1)],
        "crimson_forest" => &[(Kind::ZombifiedPiglin, 1), (Kind::Hoglin, 9), (Kind::Piglin, 5)],
        "warped_forest" => &[(Kind::Enderman, 1)],
        "basalt_deltas" => &[(Kind::Ghast, 40), (Kind::MagmaCube, 100)],
        _ => &[(Kind::Ghast, 50), (Kind::ZombifiedPiglin, 100), (Kind::MagmaCube, 2), (Kind::Enderman, 1), (Kind::Piglin, 15)],
    }
}

/// Крепость Незера: на кирпиче крепости (вики, Nether Fortress, «Mob spawning»).
pub const FORTRESS: [(Kind, u32); 5] = [(Kind::Blaze, 10), (Kind::ZombifiedPiglin, 5), (Kind::WitherSkeleton, 8), (Kind::Skeleton, 2), (Kind::MagmaCube, 3)];

/// Подводная крепость: стражи в воде её рамки (вики, Guardian, «Spawning»;
/// группа 2–4 [проверить]).
pub const MONUMENT: [(Kind, u32); 1] = [(Kind::Guardian, 1)];

/// Энд: одни эндермены (вики, Enderman, «Spawning»).
pub const END_HOSTILE: [(Kind, u32); 1] = [(Kind::Enderman, 10)];

/// Сколько мобов в группе не больше: ведьма одна (вики, Witch: «Group size: 1»),
/// прочие — до четырёх.
fn hostile_group(kind: Kind) -> u32 {
    match kind {
        Kind::Witch | Kind::Ghast | Kind::Enderman | Kind::Drowned => 1,
        // Вики: ифрит 2–3, иссушитель и скелет 5, хоглин 3–4, куб 4.
        Kind::Blaze => 3,
        Kind::WitherSkeleton => 5,
        _ => 4,
    }
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
    hostile_attempt_in(view, players, surface_of, &|_, _, _| kinds.to_vec(), 12, rng, next_id)
}

/// То же, но виды зависят от места (биом, крепость): `kinds_at(x, y, z)`; в
/// Незере попыток больше — свободных мест там мало.
#[allow(clippy::too_many_arguments)]
pub fn hostile_attempt_in(
    view: &dyn View,
    players: &[(f64, f64, f64)],
    surface_of: &dyn Fn(i32, i32) -> i32,
    kinds_at: &dyn Fn(i32, i32, i32) -> Vec<(Kind, u32)>,
    tries: u32,
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

    for _ in 0..tries {
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

        if spawnable(view, x, y, z, 2) || wet_spot(view, x, y, z) {
            found = Some((x, y, z));
            break;
        }

        // Дно реки или моря: воды над ним мало в выборе высоты — смотрим сразу
        // над дном.
        if wet_spot(view, x, top, z) {
            found = Some((x, top, z));
            break;
        }
    }

    let Some((x, y, z)) = found else {
        return Vec::new();
    };

    let kinds = kinds_at(x, y, z);

    if kinds.is_empty() {
        return Vec::new();
    }

    let kind = pick_hostile(&kinds, rng);
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

        let spot_ok = if kind == Kind::Guardian {
            super::is_water(view.block(gx, y, gz)) && super::is_water(view.block(gx, y + 1, gz))
        } else if kind == Kind::Drowned { wet_spot(view, gx, y, gz) } else { spawnable(view, gx, y, gz, block_height) };

        if !view.loaded(gx.div_euclid(16), gz.div_euclid(16)) || !spot_ok {
            continue;
        }

        // Вики, Stray: «directly under the sky»; болотнику небо не нужно.
        if kind == Kind::Stray && !view.open_sky(gx, y, gz) {
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

        // Вики, Drowned: 3 % — ракушка в левой руке (выпадает всегда); 10 % —
        // трезубец (10 из 16) или удочка (6 из 16) в основной.
        if each == Kind::Drowned {
            if rng.unit() > 0.9 {
                mob.h.hand = if rng.below(16) < 10 { super::hostile::HAND_TRIDENT } else { super::hostile::HAND_ROD };
            }

            mob.h.offhand_shell = rng.unit() < 0.03;
        }

        if super::slime::cube(each) {
            super::nether::set_size(&mut mob, super::nether::roll_size(rng));
        }

        if each == Kind::ZombieVillager {
            mob.villager.profession = rng.below(crate::villager::PROFESSIONS.len() as u32) as u8;
            mob.villager.finalized = true;
        }

        if super::slime::cube(each) && !fits(view, &mob) {
            continue;
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

/// Полный ящик куба: большой слизень должен помещаться и по ширине.
pub fn fits(view: &dyn View, mob: &Mob) -> bool {
    let (width, height) = mob.size();
    let half = width / 2.0;
    for x in (mob.x - half).floor() as i32..=(mob.x + half - 1.0e-7).floor() as i32 {
        for z in (mob.z - half).floor() as i32..=(mob.z + half - 1.0e-7).floor() as i32 {
            if !view.loaded(x.div_euclid(16), z.div_euclid(16)) {
                return false;
            }
            for y in mob.y.floor() as i32..=(mob.y + height - 1.0e-7).floor() as i32 {
                let state = view.block(x, y, z);
                if solid(state) || super::is_water(state) || super::is_lava(state) {
                    return false;
                }
            }
        }
    }
    true
}

// Биомные таблицы новых животных (вики, страницы мобов, «Spawning»).
// [проверить] Общий алгоритм размещения использует существующую модель чанков.
pub fn land_table(biome: &str) -> Vec<(Kind,u32,u32,u32)> {
    match biome {
        "snowy_slopes" | "jagged_peaks" | "frozen_peaks" => vec![(Kind::Goat,5,1,3)],
        "snowy_plains" | "ice_spikes" | "frozen_ocean" | "deep_frozen_ocean" => vec![(Kind::PolarBear,1,1,2)],
        "mushroom_fields" => vec![(Kind::Mooshroom,8,4,8)],
        "savanna" | "savanna_plateau" | "windswept_savanna" => vec![(Kind::Llama,8,4,6)],
        "windswept_hills" | "windswept_forest" | "windswept_gravelly_hills" => vec![(Kind::Llama,5,4,6)],
        "jungle" => vec![(Kind::Panda,1,1,2),(Kind::Ocelot,2,1,3)],
        "bamboo_jungle" => vec![(Kind::Panda,80,1,2),(Kind::Ocelot,2,1,3)],
        "sparse_jungle" => vec![(Kind::Ocelot,2,1,3)],
        _ => Vec::new(),
    }
}
pub fn land_for_new_chunk(view: &dyn View, cx: i32, cz: i32, biome: &str, rng: &mut Rng, next: &mut dyn FnMut()->i32) -> Vec<Mob> {
    let mut table=land_table(biome);
    let exclusive=matches!(biome,"snowy_slopes"|"jagged_peaks"|"frozen_peaks"|"snowy_plains"|"ice_spikes"|"frozen_ocean"|"deep_frozen_ocean"|"mushroom_fields");
    if !exclusive { table.extend(WEIGHTS.iter().map(|(k,w)| { let (a,b)=group(*k); (*k,*w,a,b) })); }
    if table.is_empty() || rng.unit()>=CHUNK_CHANCE { return Vec::new(); }
    let mut pick=rng.below(table.iter().map(|t| t.1).sum());
    let mut chosen=table[0];
    for t in table { if pick<t.1 { chosen=t; break; } pick-=t.1; }
    let (kind,_,low,high)=chosen;
    let count=low+rng.below(high-low+1);
    let mut result=Vec::new();
    for _ in 0..count*4 {
        if result.len()>=count as usize { break; }
        let x=cx*16+rng.below(16) as i32; let z=cz*16+rng.below(16) as i32;
        for y in (-63..319).rev() {
            let below=view.block(x,y-1,z);
            if !solid(below) { continue; }
            let block=blocks::block_at_state(below).unwrap_or("");
            let ground=match kind {
                Kind::Mooshroom=>block=="mycelium",
                Kind::Goat=>matches!(block,"grass_block"|"snow_block"|"stone"|"packed_ice"|"ice"),
                Kind::PolarBear=>matches!(block,"grass_block"|"snow_block"|"ice"|"packed_ice"),
                _=>block=="grass_block",
            };
            if !ground || solid(view.block(x,y,z)) || solid(view.block(x,y+1,z)) || (y+2..320).any(|yy| solid(view.block(x,yy,z))) { break; }
            let mut mob=Mob::new(next(),kind,(f64::from(x)+0.5,f64::from(y),f64::from(z)+0.5));
            super::land::initialize(&mut mob,rng);
            if kind==Kind::Sheep { mob.color=wild_colour(rng); }
            if kind==Kind::PolarBear && !result.is_empty() || matches!(kind,Kind::Panda|Kind::Goat|Kind::Llama) && rng.below(20)==0 { mob.age=super::BABY_AGE; }
            if fits(view,&mob) { result.push(mob); }
            break;
        }
    }
    result
}

/// Вики, страницы биомов, «Creature category». [проверить] веса горных
/// биомов и общий предел животных; волки пока остаются в старой ветке.
pub fn passive_weights(biome: &str) -> Vec<(Kind,u32)> {
    let biome=biome.trim_start_matches("minecraft:");
    let mut weights=match biome {
        "desert" | "snowy_plains" | "ice_spikes" | "grove" | "snowy_slopes" => Vec::new(),
        "meadow" => vec![(Kind::Sheep,2)],
        "cherry_grove" => vec![(Kind::Pig,1),(Kind::Sheep,2)],
        name if name.contains("ocean") || matches!(name,"river"|"frozen_river"|"beach"|"snowy_beach"|"stony_shore"|"mushroom_fields") => Vec::new(),
        _=>WEIGHTS.to_vec(),
    };
    let rabbit=match biome {
        "snowy_plains"|"ice_spikes"=>10,
        "meadow"=>2,
        "cherry_grove"=>3,
        "desert"|"flower_forest"|"taiga"|"snowy_taiga"|"old_growth_pine_taiga"|"old_growth_spruce_taiga"|"grove"|"snowy_slopes"=>4,
        _=>0,
    };
    if rabbit>0 { weights.push((Kind::Rabbit,rabbit)); }
    if matches!(biome,"taiga"|"snowy_taiga"|"old_growth_pine_taiga"|"old_growth_spruce_taiga"|"grove") { weights.push((Kind::Fox,8)); }
    weights
}

fn animal_surface(view: &dyn View,x: i32,z: i32,kind: Kind) -> Option<i32> {
    if !matches!(kind,Kind::Rabbit|Kind::Fox) { return surface(view,x,z); }
    for y in (-64..320).rev() {
        let state=view.block(x,y,z);
        let name=blocks::block_at_state(state).unwrap_or("");
        if solid(state) || super::is_water(state) || super::is_lava(state) {
            let floor=matches!(name,"grass_block"|"snow"|"snow_block")
                || (kind==Kind::Rabbit && name=="sand")
                || (kind==Kind::Fox && matches!(name,"podzol"|"coarse_dirt"));
            return (floor && view.light(x,y+1,z,15).0.max(view.light(x,y+1,z,15).1)>=9).then_some(y);
        }
    }
    None
}
