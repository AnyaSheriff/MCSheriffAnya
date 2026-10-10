// Иссушитель (вики, Wither): постройка из песка или почвы душ и трёх черепов
// (узор ищет mob/golem.rs), 220 тактов зарядки с ростом здоровья и полосой
// босса, взрыв силой 7, три головы со своими целями, черепа (чёрные и синие),
// броня на половине здоровья, ломка блоков после удара, лечение 1 в секунду,
// звезда Незера и 50 опыта.
//
// Кода игры здесь нет: правила — со страницы minecraft.wiki (Wither, Wither
// Skull) и с чёрного ящика (wither-summon, wither-skull-hit, wither-half-armor,
// wither-death-drop). Что не сверено, помечено «[проверить]».
//
// Что видно в записях оригинала:
// - построенный появляется в (x+0,5; y+0,55; z+0,5) нижнего блока, поворот 90°
//   у Т вдоль x и 0° вдоль z; здоровье 100 (треть), 19=220;
// - каждый такт зарядки 19 падает на 1, полоса — 1 − 19/220 (пакет сразу, в
//   ходе сущности); на каждом десятом такте жизни +10 здоровья (100 → 300);
// - на 220-м такте — взрыв силой 7 в глазах (y + 2,975), затем глобальное
//   событие мира 1023 в клетке иссушителя;
// - по вертикали скорость гаснет до 0,6 каждый такт (падение −0,047, −0,075, …);
// - без ИИ (NoAI) — ни зарядки, ни лечения, ни перемен полосы;
// - смерть — как у всех: событие 3, поза 7, через 19 тактов облачко 60 и уход
//   полосы вместе с сущностью.

use super::{ai, Kind, Mob, Rng, View};

/// Здоровье (вики, Wither: «300HP» на любой сложности).
pub const HEALTH: f32 = 300.0;

/// Опыт за убийство игроком (вики, Wither: «50»; wither-death-drop: шарики 37+7+3+3).
pub const XP: i32 = 50;

/// Такты зарядки после постройки (вики: «220 ticks»; wither-summon).
pub const INVUL: i32 = 220;

/// Лечение в зарядке: +10 на каждом десятом такте жизни (wither-summon:
/// 9=100, 110, …, 300).
const CHARGE_HEAL_EVERY: u64 = 10;
const CHARGE_HEAL: f32 = 10.0;

/// Взрыв в конце зарядки (вики: «blast power 7»; wither-summon: радиус 7).
pub const SPAWN_POWER: f32 = 7.0;

/// Взрыв черепа (вики, Wither Skull: «blast power 1»; wither-skull-hit).
pub const SKULL_POWER: f32 = 1.0;

/// Урон черепа: от иссушителя 8, без хозяина — магия 5 (вики, Wither Skull;
/// wither-skull-hit: Damage Event type=27, 20 → 15 и взрыв).
pub const SKULL_DAMAGE: f32 = 8.0;
pub const SKULL_DAMAGE_OWNERLESS: f32 = 5.0;

/// Хозяин черепа, убивший им цель, лечится на 5 (вики, Wither: «5 hearts»
/// [проверить: сердца или единицы]).
pub const KILL_HEAL: f32 = 5.0;

/// Иссушение от черепа: уровень II, 10 с на обычной и 40 с на сложной (вики;
/// wither-skull-hit: Effect id=19 amp=1 dur=200).
pub const SKULL_WITHER_AMPLIFIER: i32 = 1;

pub fn skull_wither_ticks(difficulty: u8) -> Option<i32> {
    match difficulty {
        2 => Some(200),
        3 => Some(800),
        _ => None,
    }
}

/// Сопротивление чёрного черепа и синего: синий летит медленнее (wither-skull-hit:
/// с 8,5 до грани 5 — 13 тактов при 0,73, чёрный — 9 тактов при 0,95).
pub const SKULL_DRAG: f64 = 0.95;
pub const DANGEROUS_DRAG: f64 = 0.73;

/// Синий череп считает прочность блоков не больше 0,8 (вики, Wither Skull:
/// «blue skulls … blast resistance of 0.8» [проверить формулировку]).
pub const DANGEROUS_RESISTANCE: f32 = 0.8;

/// Дальность стрельбы головами (вики: 20 блоков [проверить]).
const RANGE: f64 = 20.0;

/// Шанс за такт найти новую цель главной головой: 1 к 10.
const SEEK_CHANCE: u32 = 10;

/// Главная голова стреляет раз в 40 тактов (вики: «every 2 seconds»).
const MAIN_EVERY: i32 = 40;

/// Боковые головы: следующая проверка через 10–19 тактов, без цели — 15
/// проверок до синего черепа в случайную сторону на обычной и сложной (вики,
/// Wither: «9.5–17.2 seconds without firing» [проверить]).
const SIDE_EVERY: u64 = 10;
const SIDE_SPREAD: u32 = 10;
const IDLE_SHOTS: i32 = 15;

/// Синий череп главной головы — 1 к 1000 (вики: «0.1%»).
const MAIN_BLUE: u32 = 1000;

/// Ломка блоков: через 20 тактов после удара (вики: «the counter resets to 20
/// ticks»), область 3×4×3 вокруг.
const BREAK_DELAY: i32 = 20;

/// Вертикальная скорость гаснет до 0,6 за такт (wither-summon: падение −193,
/// −306, −372 … /4096 = 0,6·(v) и тяжесть).
const VERTICAL_DAMP: f64 = 0.6;

/// Лечение после зарядки: 1 раз в 20 тактов (вики: «1HP every 20 ticks»).
const REGEN_EVERY: u64 = 20;

/// Скорость полёта (вики: 0,6) и высота над целью [проверить].
const FLY_SPEED: f64 = 0.6;
const HOVER: f64 = 5.0;

/// Что иссушитель просит у такта (tick.rs, wither_deeds).
#[derive(Clone, Debug, PartialEq)]
pub enum Deed {
    /// Полоса босса: новая доля.
    Bar(f32),
    /// Взрыв конца зарядки и глобальное событие 1023 в клетке.
    Charged { at: (f64, f64, f64), cell: (i32, i32, i32) },
    /// Череп из головы `from` по направлению `dir`.
    Skull { from: (f64, f64, f64), dir: (f64, f64, f64), dangerous: bool },
    /// Ломка блоков вокруг (3×4×3 от ног).
    Break { feet: (i32, i32, i32) },
}

/// Состояние иссушителя.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Такты зарядки (свойство 19).
    pub invul: i32,
    /// Цели средней, левой и правой головы (свойства 16–18; 0 — нет).
    pub targets: [i32; 3],
    /// Такт следующей проверки боковых голов и сколько раз они были без цели.
    pub next_head: [u64; 2],
    pub idle: [i32; 2],
    /// Отсчёт до выстрела главной головы.
    pub main_cooldown: i32,
    /// Отсчёт до ломки блоков после удара.
    pub break_in: i32,
    /// Доля полосы, как она показана.
    pub bar: Option<f32>,
    /// Заказы такту.
    pub deeds: Vec<Deed>,
    /// Построен игроком (а не призван): атрибуты уходят на такт после
    /// появления, а не на третий (wither-summon против wither-half-armor).
    pub built: bool,
}

/// Череп скелета-иссушителя на полу или на стене.
pub fn is_skull(state: i32) -> bool {
    matches!(crate::blocks::block_at_state(state), Some("wither_skeleton_skull" | "wither_skeleton_wall_skull"))
}

/// Песок или почва душ (вики: «soul sand or soul soil»).
pub fn is_soul(state: i32) -> bool {
    matches!(crate::blocks::block_at_state(state), Some("soul_sand" | "soul_soil"))
}

/// Нежить (вики, Undead): иссушитель на неё не нападает, «Небесная кара» по ней бьёт.
pub fn undead(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Zombie | Kind::Husk | Kind::Drowned | Kind::ZombieVillager | Kind::ZombifiedPiglin | Kind::Skeleton | Kind::WitherSkeleton | Kind::Phantom | Kind::Wither
    )
}

/// Цель для иссушителя: живое и не нежить (вики: «attacks all mobs except
/// undead»; не цели — кристалл, узел поводка, снаряд шалкера).
fn prey(kind: Kind) -> bool {
    !undead(kind) && !matches!(kind, Kind::EndCrystal | Kind::LeashKnot | Kind::ShulkerBullet | Kind::EnderDragon)
}

/// Броня на половине здоровья: стрелы и трезубцы отскакивают (вики: «below
/// half health … immune to arrows»).
pub fn powered(mob: &Mob) -> bool {
    mob.kind == Kind::Wither && mob.vitals.health <= mob.vitals.max_health / 2.0
}

/// Только что построенный: треть здоровья и 220 тактов зарядки; поворот —
/// по оси Т (wither-summon: yaw 90° вдоль x, 0° вдоль z).
pub fn built(mob: &mut Mob, along_x: bool) {
    mob.wither.invul = INVUL;
    mob.wither.built = true;
    mob.wither.bar = Some(0.0);
    mob.vitals.health = HEALTH / 3.0;
    mob.y += 0.5;
    mob.yaw = if along_x { 90.0 } else { 0.0 };
}

/// Неуязвим ли к этому урону: в зарядке — ко всему, кроме /kill и пустоты;
/// всегда — к иссушению и утоплению (вики).
pub fn ignores(mob: &Mob, source: crate::survival::Source) -> bool {
    use crate::survival::Source;

    if matches!(source, Source::Wither | Source::Drown) {
        return true;
    }

    mob.wither.invul > 0 && !matches!(source, Source::GenericKill | Source::OutOfWorld)
}

/// Удар прошёл: через 20 тактов — ломка блоков вокруг.
pub fn on_hurt(mob: &mut Mob) {
    if mob.wither.invul == 0 {
        mob.wither.break_in = BREAK_DELAY;
    }
}

/// Доля полосы, как она показана: у призванного — полная (wither-half-armor:
/// add health=1), у построенного — пустая (wither-summon: add health=0).
pub fn bar(mob: &Mob) -> f32 {
    mob.wither.bar.unwrap_or(1.0)
}

fn set_bar(mob: &mut Mob, share: f32) {
    if bar(mob) != share {
        mob.wither.bar = Some(share);
        mob.wither.deeds.push(Deed::Bar(share));
    }
}

/// Поворот к цели не больше `step` градусов.
fn approach(from: f32, to: f32, step: f32) -> f32 {
    let delta = (to - from + 540.0).rem_euclid(360.0) - 180.0;

    // Дошла — ровно до цели, без остатка округления (wither-summon: головы
    // встают на 64 и 0 из 256, а не на 63 и −1).
    if delta.abs() <= step {
        return to;
    }

    from + delta.clamp(-step, step)
}

/// Место головы `index` (0 — средняя, 1 — левая, 2 — правая): средняя выше,
/// боковые — по 1,3 блока вбок от поворота тела [проверить].
pub fn head_at(mob: &Mob, index: usize) -> (f64, f64, f64) {
    if index == 0 {
        return (mob.x, mob.y + 3.0, mob.z);
    }

    let angle = f64::from(mob.yaw + 180.0 * (index as f32 - 1.0)).to_radians();

    (mob.x + angle.cos() * 1.3, mob.y + 2.2, mob.z + angle.sin() * 1.3)
}

/// Где цель: игрок или моб рядом.
fn place_of(ctx: &ai::Ctx, id: i32) -> Option<(f64, f64, f64, f64)> {
    if let Some(player) = ctx.players.iter().find(|player| player.id == id && player.targetable && !player.spectator) {
        return Some((player.x, player.y, player.z, player.eye));
    }

    ctx.mates.iter().find(|mate| mate.id == id && prey(mate.kind)).map(|mate| (mate.x, mate.y, mate.z, 0.5))
}

/// Ближайшая цель в пределах `range`.
fn nearest(mob: &Mob, ctx: &ai::Ctx, range: f64) -> Option<i32> {
    let near = |x: f64, y: f64, z: f64| {
        let d = (x - mob.x).powi(2) + (y - mob.y).powi(2) + (z - mob.z).powi(2);

        (d <= range * range).then_some(d)
    };
    let players = ctx.players.iter().filter(|player| player.targetable && !player.spectator).filter_map(|player| near(player.x, player.y, player.z).map(|d| (d, player.id)));
    let mates = ctx.mates.iter().filter(|mate| mate.id != mob.entity_id && prey(mate.kind)).filter_map(|mate| near(mate.x, mate.y, mate.z).map(|d| (d, mate.id)));

    players.chain(mates).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
}

/// Выстрел головы `index` в точку `at`.
fn shoot(mob: &mut Mob, index: usize, at: (f64, f64, f64), dangerous: bool) {
    let from = head_at(mob, index);
    let dir = (at.0 - from.0, at.1 - from.1, at.2 - from.2);
    let length = (dir.0 * dir.0 + dir.1 * dir.1 + dir.2 * dir.2).sqrt().max(1.0e-9);

    mob.wither.deeds.push(Deed::Skull { from, dir: (dir.0 / length, dir.1 / length, dir.2 / length), dangerous });

    if !mob.silent {
        mob.events.push(super::Event::Sound { name: "minecraft:entity.wither.shoot", volume: 3.0, pitch: 1.0 });
    }
}

/// Такт живого иссушителя с ИИ (без ИИ сюда не попадает: mod.rs).
pub fn tick(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
    ai::voice(mob, rng);
    mob.vy *= VERTICAL_DAMP;

    // Зарядка: стоит, лечится, полоса растёт; в конце — взрыв.
    if mob.wither.invul > 0 {
        mob.wither.invul -= 1;
        set_bar(mob, 1.0 - mob.wither.invul as f32 / INVUL as f32);

        if mob.wither.invul == 0 {
            mob.wither.deeds.push(Deed::Charged { at: (mob.x, mob.y + mob.eye(), mob.z), cell: mob.cell() });
        }

        if mob.ticks.is_multiple_of(CHARGE_HEAL_EVERY) {
            mob.vitals.heal(CHARGE_HEAL);
        }

        // Голова догоняет тело на 10° за такт (wither-summon: 11, 18, 25 … 64).
        mob.head_yaw = approach(mob.head_yaw, mob.yaw, 10.0);
        mob.physics(view, 0.0, false);

        return ai::Plan::default();
    }

    if mob.ticks.is_multiple_of(REGEN_EVERY) {
        mob.vitals.heal(1.0);
    }

    // Ломка блоков после удара.
    if mob.wither.break_in > 0 {
        mob.wither.break_in -= 1;

        if mob.wither.break_in == 0 {
            mob.wither.deeds.push(Deed::Break { feet: mob.cell() });
        }
    }

    // Главная голова: ближайшая цель, выстрел раз в 40 тактов.
    // Новую цель ищет с шансом 1 к 10 за такт, как другие враждебные
    // (mob/hostile.rs) [проверить у иссушителя].
    let kept = place_of(ctx, mob.wither.targets[0]).map(|_| mob.wither.targets[0]);
    let main = kept.or_else(|| if rng.below(SEEK_CHANCE) == 0 { nearest(mob, ctx, RANGE * 2.0) } else { None });

    mob.wither.targets[0] = main.unwrap_or(0);

    if mob.wither.main_cooldown > 0 {
        mob.wither.main_cooldown -= 1;
    }

    if let Some((x, y, z, eye)) = main.and_then(|id| place_of(ctx, id)) {
        let near = (x - mob.x).powi(2) + (y - mob.y).powi(2) + (z - mob.z).powi(2) <= RANGE * RANGE;

        if near && mob.wither.main_cooldown == 0 {
            mob.wither.main_cooldown = MAIN_EVERY;
            shoot(mob, 0, (x, y + eye * 0.5, z), rng.below(MAIN_BLUE) == 0);
        }

        let yaw = ((z - mob.z).atan2(x - mob.x).to_degrees() - 90.0) as f32;

        mob.yaw = approach(mob.yaw, yaw, 10.0);
        mob.head_yaw = approach(mob.head_yaw, yaw, 10.0);

        // Полёт: держится на 5 блоков выше цели (в броне — на её высоте),
        // подлетает, если дальше 3 блоков по горизонтали [проверить].
        let lift = if powered(mob) { 0.0 } else { HOVER };

        if mob.y < y + lift {
            mob.vy = mob.vy.max(0.0) + (FLY_SPEED - mob.vy) * FLY_SPEED;
        }

        let (dx, dz) = (x - mob.x, z - mob.z);
        let flat = dx * dx + dz * dz;

        if flat > 9.0 {
            let length = flat.sqrt();

            mob.vx += (dx / length * FLY_SPEED - mob.vx) * FLY_SPEED;
            mob.vz += (dz / length * FLY_SPEED - mob.vz) * FLY_SPEED;
        }
    } else {
        mob.head_yaw = approach(mob.head_yaw, mob.yaw, 10.0);
    }

    // Боковые головы: своя цель, выстрел раз в 10–19 тактов; без цели на
    // обычной и сложной — синий череп в случайную сторону.
    for side in 0..2 {
        if mob.ticks < mob.wither.next_head[side] {
            continue;
        }

        mob.wither.next_head[side] = mob.ticks + SIDE_EVERY + u64::from(rng.below(SIDE_SPREAD));

        let index = side + 1;
        let target = place_of(ctx, mob.wither.targets[index]).map(|_| mob.wither.targets[index]).or_else(|| nearest(mob, ctx, RANGE));

        mob.wither.targets[index] = target.unwrap_or(0);

        match target.and_then(|id| place_of(ctx, id)) {
            Some((x, y, z, eye)) => {
                mob.wither.idle[side] = 0;
                shoot(mob, index, (x, y + eye * 0.5, z), false);
            }
            None if ctx.difficulty >= 2 => {
                mob.wither.idle[side] += 1;

                if mob.wither.idle[side] > IDLE_SHOTS {
                    mob.wither.idle[side] = 0;

                    let at = (mob.x + (rng.unit() - 0.5) * 20.0, mob.y + (rng.unit() - 0.5) * 10.0, mob.z + (rng.unit() - 0.5) * 20.0);

                    shoot(mob, index, at, true);
                }
            }
            None => {}
        }
    }

    set_bar(mob, mob.vitals.health / mob.vitals.max_health);
    mob.physics(view, 0.0, false);

    ai::Plan::default()
}

/// Клетки, что ломаются вокруг (3×4×3 от ног): всё, кроме воздуха, жидкостей
/// и неразрушимого (вики: «except moving pistons, unbreakable blocks, and
/// liquids»).
pub fn break_cells(view: &dyn View, feet: (i32, i32, i32)) -> Vec<(i32, i32, i32)> {
    let mut cells = Vec::new();

    for dx in -1..=1 {
        for dy in 0..=3 {
            for dz in -1..=1 {
                let (x, y, z) = (feet.0 + dx, feet.1 + dy, feet.2 + dz);
                let state = view.block(x, y, z);

                if breakable(state) {
                    cells.push((x, y, z));
                }
            }
        }
    }

    cells
}

/// Ломает ли иссушитель этот блок.
pub fn breakable(state: i32) -> bool {
    let Some(name) = crate::blocks::block_at_state(state) else {
        return false;
    };

    !matches!(
        name,
        "air" | "cave_air" | "void_air" | "water" | "lava" | "bedrock" | "barrier" | "end_portal" | "end_portal_frame" | "end_gateway" | "command_block" | "chain_command_block" | "repeating_command_block" | "structure_block" | "jigsaw" | "moving_piston" | "light" | "reinforced_deepslate"
    )
}

/// Неразрушимое и для синего черепа (вики, Wither: теги «wither_immune»).
pub fn wither_immune(state: i32) -> bool {
    !breakable(state)
        && crate::blocks::block_at_state(state).is_some_and(|name| !matches!(name, "air" | "cave_air" | "void_air" | "water" | "lava"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_charge_heals_ten_every_ten_ticks_from_a_third() {
        let mut mob = Mob::new(1, Kind::Wither, (0.5, 0.05, 0.5));

        built(&mut mob, true);
        assert_eq!(mob.vitals.health, 100.0);
        assert_eq!(mob.wither.invul, 220);
        assert_eq!(mob.y, 0.55);
        assert_eq!(mob.yaw, 90.0);
    }

    #[test]
    fn heads_turn_ten_degrees_a_tick() {
        assert_eq!(approach(7.9, 90.0, 10.0), 17.9);
        assert_eq!(approach(85.0, 90.0, 10.0), 90.0);
        assert_eq!(approach(350.0, 10.0, 10.0), 360.0);
        assert_eq!(approach(4.712_3, 0.0, 10.0), 0.0);
    }

    #[test]
    fn undead_are_not_prey() {
        assert!(!prey(Kind::Zombie));
        assert!(!prey(Kind::Wither));
        assert!(prey(Kind::Cow));
        assert!(prey(Kind::IronGolem));
    }

    #[test]
    fn skull_wither_lasts_by_difficulty() {
        assert_eq!(skull_wither_ticks(1), None);
        assert_eq!(skull_wither_ticks(2), Some(200));
        assert_eq!(skull_wither_ticks(3), Some(800));
    }

    #[test]
    fn stuck_arrows_fall_out_one_at_a_time() {
        let mut mob = Mob::new(1, Kind::Wither, (0.5, 0.0, 0.5));

        mob.stick_arrow();
        mob.stick_arrow();
        assert_eq!((mob.arrows, mob.arrow_timer), (2, 560));
    }

    #[test]
    fn the_charge_ignores_everything_but_kill() {
        let mut mob = Mob::new(1, Kind::Wither, (0.5, 0.0, 0.5));

        built(&mut mob, false);
        assert!(ignores(&mob, crate::survival::Source::PlayerAttack));
        assert!(!ignores(&mob, crate::survival::Source::GenericKill));
        mob.wither.invul = 0;
        assert!(!ignores(&mob, crate::survival::Source::PlayerAttack));
        assert!(ignores(&mob, crate::survival::Source::Wither));
    }
}
