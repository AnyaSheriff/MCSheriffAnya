// Слизень и общие правила прыгающих кубов. Источники: minecraft.wiki/w/Slime
// («Size», «Behavior», «Spawning», «Drops», «Sounds»), Magma Cube;
// minecraft-data 26.1 entities.json. Кода игры и чужих ядер здесь нет.
// Сверено с оригиналом чёрным ящиком: slime-split, slime-hop, slime-attack.

use super::ai::{Ctx, Plan, Watcher};
use super::{Event, Kind, Mob, Rng, View, hostile};

pub fn cube(kind: Kind) -> bool {
    matches!(kind, Kind::Slime | Kind::MagmaCube)
}

/// Вики: «health is equal to their size squared»; Size в NBT на единицу меньше.
pub fn set_size(mob: &mut Mob, size: i32) {
    mob.nether.size = size.clamp(1, 127);
    mob.vitals.max_health = (mob.nether.size * mob.nether.size) as f32;
    mob.vitals.health = mob.vitals.max_health;
    mob.attr_changes += 1;

    // Замер slime-hop: после появления прыжок на 3-м такте (приземление на 1-м).
    if mob.kind == Kind::Slime {
        mob.nether.hop = 2;
    }
}

pub fn speed(size: i32) -> f64 {
    f64::from(0.2f32 + 0.1f32 * size.max(1) as f32)
}

pub fn damage(size: i32) -> f32 {
    if size <= 1 { 0.0 } else { size as f32 }
}

/// Вики: «2–4 smaller slimes», каждый вдвое меньше; маленький не делится.
pub fn split(parent: &Mob, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    if !cube(parent.kind) || parent.nether.size <= 1 || parent.h.despawn || !parent.gone() {
        return Vec::new();
    }

    let count = 2 + rng.below(3);
    // Замер slime-split и magma-cube-stand: дети стоят на ±ширина/4 от центра по
    // каждой оси (размер 2: ±0,26; 3: ±0,39; 4: ±0,52), ширина — 0,52·размер.
    let spread = parent.size().0 / 2.0;
    let mut children = Vec::new();

    for i in 0..count {
        let at = (parent.x + (f64::from(i % 2) - 0.5) * spread, parent.y + 0.5, parent.z + (f64::from(i / 2) - 0.5) * spread);
        let mut child = Mob::new(next_id(), parent.kind, at);

        set_size(&mut child, parent.nether.size / 2);
        child.no_ai = parent.no_ai;
        child.silent = parent.silent;
        child.nether.persistence_required = parent.nether.persistence_required;
        child.face((rng.unit() * 360.0) as f32);
        children.push(child);
    }

    children
}

/// Случайный размер при появлении; местная сложность увеличивает долю больших
/// (вики, Slime). [проверить] Пока равные доли, как у существующего Magma Cube.
pub fn roll_size(rng: &mut Rng) -> i32 {
    1 << rng.below(3)
}

/// Вики, Slime: задержка 10–29 тактов на земле, при атаке втрое короче;
/// у магмового куба вчетверо длиннее. Маленький слизень не атакует.
pub fn jump_delay(kind: Kind, attacking: bool, rng: &mut Rng) -> i32 {
    let delay = (10 + rng.below(20) as i32) * if kind == Kind::MagmaCube { 4 } else { 1 };
    let delay = if attacking { delay / 3 } else { delay };

    // Замер slime-hop / slime-idle: у слизня от приземления до прыжка всегда
    // нечётное число тактов — 2·счёт + 1 (21–59 без цели, 7–19 в погоне).
    // [проверить] У магмового куба то же — записей прыжков нет.
    if kind == Kind::Slime { 2 * delay + 1 } else { delay }
}

/// Общая мысль: без прокладки пути; слизень прыгает прямо к видимой цели.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();
    let target = hostile::choose_target(mob, view, ctx, rng);
    let attacking = target.is_some() && (mob.kind == Kind::MagmaCube || mob.nether.size > 1);
    let (x, y, z) = mob.cell();
    let fluid = super::is_water(view.block(x, y, z)) || super::is_lava(view.block(x, y, z));

    if let Some(target) = target {
        super::ai::look_at_point(mob, target.x, target.y + target.eye, target.z);
        let yaw = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;
        mob.yaw = super::ai::turn(mob.yaw, yaw, 90.0);
        mob.head_yaw = mob.yaw;
    } else if mob.on_ground || fluid {
        mob.nether.wander_wait -= 1;
        if mob.nether.wander_wait <= 0 {
            mob.nether.wander_wait = 40 + rng.below(60) as i32;
            mob.face((rng.unit() * 360.0) as f32);
        }
    }

    if fluid {
        // Вики, Slime: «can swim». [проверить] Плавание — шанс 80 % за такт.
        if rng.unit() < 0.8 {
            if mob.kind == Kind::MagmaCube && super::is_lava(view.block(x, y, z)) {
                mob.vy = 0.22 + 0.05 * f64::from(mob.nether.size);
            } else {
                mob.vy += 0.04;
            }
        }
        plan.speed = 1.2;
    } else if mob.on_ground {
        mob.nether.hop -= 1;
        if mob.nether.hop <= 0 {
            mob.nether.hop = jump_delay(mob.kind, attacking, rng);
            plan.jump = true;
            plan.speed = 1.0;
            let name = if mob.kind == Kind::MagmaCube { "minecraft:entity.magma_cube.jump" } else { sound(mob.nether.size, "jump") };
            mob.events.push(Event::SoundAt { name, volume: mob.sound_volume(), pitch: hop_pitch(mob, rng, 0.8), at: (mob.x, mob.y, mob.z) });
        }
    } else {
        plan.speed = 1.0;
    }

    plan
}

/// Контакт, а не взмах оружием: любой игрок, включая не выбранную цель.
/// Интервал задаёт не ИИ, а неуязвимость игрока после урона (вики, Slime).
/// [проверить] Луч видимости и границы ящика для присевшего игрока.
pub fn contact(mob: &Mob, view: &dyn View, player: &Watcher) -> Option<hostile::Melee> {
    // Замер slime-attack: слизень с NoAI у игрока не бьёт ни разу (оригинал, 80 тактов).
    if mob.no_ai || mob.is_dead() || mob.h.despawn || !player.targetable || player.spectator {
        return None;
    }
    let damage = if mob.kind == Kind::MagmaCube { super::nether::cube_damage(mob.nether.size) } else { damage(mob.nether.size) };
    let (width, height) = mob.size();
    let (dx, dy, dz) = (player.x - mob.x, player.y - mob.y, player.z - mob.z);
    let overlap = dx.abs() < width / 2.0 + 0.3 && dz.abs() < width / 2.0 + 0.3 && dy < height && dy + player.eye + 0.18 > 0.0;
    let reach = 0.6 * f64::from(mob.nether.size);
    if damage == 0.0 || !overlap || dx * dx + dy * dy + dz * dz >= reach * reach
        || !hostile::sees(view, (mob.x, mob.y + mob.eye(), mob.z), (player.x, player.y + player.eye, player.z))
    {
        return None;
    }
    let flat = (dx * dx + dz * dz).sqrt();
    let push = if flat > 1.0e-6 { (dx / flat, dz / flat) } else { (0.0, 1.0) };
    Some(hostile::Melee { target: player.id, damage, push, wither: false, hunger: false })
}

pub fn sound(size: i32, event: &str) -> &'static str {
    match (size <= 1, event) {
        (true, "jump") => "minecraft:entity.slime.jump_small",
        (false, "jump") => "minecraft:entity.slime.jump",
        (true, "squish") => "minecraft:entity.slime.squish_small",
        (false, "squish") => "minecraft:entity.slime.squish",
        (true, "hurt") => "minecraft:entity.slime.hurt_small",
        (false, "hurt") => "minecraft:entity.slime.hurt",
        (true, "death") => "minecraft:entity.slime.death_small",
        (false, "death") => "minecraft:entity.slime.death",
        _ => "minecraft:entity.slime.attack",
    }
}

fn hop_pitch(mob: &Mob, rng: &mut Rng, factor: f32) -> f32 {
    let base = (rng.unit() - rng.unit()) as f32 * 0.2 + 1.0;

    // Замер slime-hop (оригинал, три записи): у слизня размера ≥2 прыжок
    // ~0,8 (0,71–0,93), приземление ~1,25 (1,06–1,47) — base·0,8 и base/0,8.
    if mob.kind == Kind::Slime && mob.nether.size > 1 {
        return base * if factor < 1.0 { 0.8 } else { 1.25 };
    }

    base * if mob.nether.size <= 1 { 1.4 } else { 0.8 } * factor
}

pub fn landed(mob: &mut Mob, rng: &mut Rng) {
    // [проверить] Частицы item_slime при приземлении рисует клиент по on_ground;
    // не дублируем их пакетами. Серверный Poof при исчезновении — общий.
    let name = if mob.kind == Kind::MagmaCube {
        if mob.nether.size <= 1 { "minecraft:entity.magma_cube.squish_small" } else { "minecraft:entity.magma_cube.squish" }
    } else {
        sound(mob.nether.size, "squish")
    };
    mob.events.push(Event::Sound { name, volume: mob.sound_volume(), pitch: hop_pitch(mob, rng, 1.0) });
}

/// Формула slime chunk из вики, «Java Edition»: переполнения int происходят
/// ДО приведения к long; z² умножается на 4392871 уже как long.
pub fn is_slime_chunk(seed: i64, x: i32, z: i32) -> bool {
    let mixed = seed
        .wrapping_add(i64::from(x.wrapping_mul(x).wrapping_mul(4_987_142)))
        .wrapping_add(i64::from(x.wrapping_mul(5_947_611)))
        .wrapping_add(i64::from(z.wrapping_mul(z)).wrapping_mul(4_392_871))
        .wrapping_add(i64::from(z.wrapping_mul(389_711))) ^ 987_234_911;
    // java.util.Random: 48-битный LCG и nextInt(10) с отбрасыванием хвоста.
    let mut state = (mixed as u64 ^ 0x5DEECE66D) & ((1 << 48) - 1);
    loop {
        state = state.wrapping_mul(0x5DEECE66D).wrapping_add(11) & ((1 << 48) - 1);
        let bits = (state >> 17) as i32;
        let value = bits % 10;
        if bits.wrapping_sub(value).wrapping_add(9) >= 0 {
            return value == 0;
        }
    }
}

/// Вики, Moon: полный месяц, убывающий, четверть, серп, новолуние, обратно.
pub fn moon_brightness(clock: i64) -> f64 {
    [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75][clock.div_euclid(24_000).rem_euclid(8) as usize]
}

/// Вики: «below Y=40», «between Y=50 and Y=70», свет 0…7 и фаза Луны.
pub fn can_spawn(seed: i64, at: (i32, i32, i32), biome: &str, light: u8, clock: i64, rng: &mut Rng) -> bool {
    if matches!(biome, "mushroom_fields" | "deep_dark") {
        return false;
    }
    let (x, y, z) = at;
    if y < 40 {
        return is_slime_chunk(seed, x.div_euclid(16), z.div_euclid(16)) && rng.below(10) == 0;
    }
    matches!(biome, "swamp" | "mangrove_swamp") && y > 50 && y < 70
        && rng.unit() < 0.5 && rng.unit() < moon_brightness(clock) && u32::from(light) <= rng.below(8)
}

/// Добыча: базовые 0–2 и округлённая равномерная добавка Looting 0…уровень.
pub fn slimeballs(looting: i32, rng: &mut Rng) -> i32 {
    rng.below(3) as i32 + (rng.unit() * f64::from(looting.max(0))).round() as i32
}
