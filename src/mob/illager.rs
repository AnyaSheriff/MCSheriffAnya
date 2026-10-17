//! Разбойники (вики, Illager) и их спутники: разбойник с арбалетом, поборник
//! с топором, заклинатель (челюсти, вредины, «вололо»), вредина, иллюзионист,
//! разоритель и челюсти заклинателя.
//!
//! Сверено с оригиналом (чёрный ящик): pillager-crossbow, vindicator-axe,
//! evoker-fangs, evoker-vex, ravager-roar. Чего ящик не показал — по вики,
//! с пометкой [проверить].

use super::ai::{self, Ctx, Plan, Watcher};
use super::hostile::{self, Melee, Shot};
use super::{Event, Kind, Mob, Rng, View};
use crate::blocks;

/// Заклинания (свойство 17 у заклинателя и иллюзиониста, minecraft-data:
/// spell_casting): 0 — нет, 1 — вредины, 2 — челюсти, 3 — «вололо»
/// (чёрный ящик, evoker-fangs: 17=1 при призыве, 17=2 при челюстях).
pub const SPELL_NONE: u8 = 0;
pub const SPELL_VEX: u8 = 1;
pub const SPELL_FANGS: u8 = 2;
pub const SPELL_WOLOLO: u8 = 3;

/// «Вололо»: синяя овца (цвет 11) в 16 блоках становится красной (14);
/// заклинание 60 тактов, после него пауза 140 (вики, Evoker: «turns blue
/// sheep … red», нужен mob_griefing) [проверить: такты].
pub const WOLOLO_FROM: u8 = 11;
pub const WOLOLO_TO: u8 = 14;
pub const WOLOLO_RANGE: f64 = 16.0;
pub const WOLOLO_SPELL: i32 = 60;
pub const WOLOLO_COOLDOWN: i32 = 140;

/// От звука подготовки до самого заклинания (чёрный ящик, evoker-fangs:
/// prepare_summon в такте 2, вредины в 20; prepare_attack в 102, челюсти в 120).
pub const SPELL_WARMUP: i32 = 18;

/// Сколько длится заклинание целиком: призыв 100 тактов (evoker-fangs:
/// следующее заклинание через 100), челюсти — 41 (evoker-vex: 17=0 через 41).
pub const VEX_SPELL: i32 = 100;
pub const FANGS_SPELL: i32 = 41;

/// Перезарядка (вики, Evoker: челюсти 5 с, вредины 17 с). Заклинание
/// «вололо» (синих овец в красных) ещё не сделано [проверить].
pub const FANGS_COOLDOWN: i32 = 100;
pub const VEX_COOLDOWN: i32 = 340;

/// Вредин за раз и предел рядом (вики, Evoker: «three vexes», «8 … within 16»).
pub const VEXES: usize = 3;
pub const VEX_LIMIT: usize = 8;

/// Челюсти: линия из 16 через 1,25 блока; цель ближе 3 — два кольца, 5 и 8
/// (вики, Evoker; чёрный ящик, evoker-fangs: шаг 1,25, задержка +1 такт на
/// каждую следующую).
pub const FANG_LINE: usize = 16;
pub const FANG_STEP: f64 = 1.25;

/// Такты челюстей после появления (чёрный ящик, evoker-fangs и evoker-vex):
/// событие 4 через 1 + задержка, укус через 8 + задержка, исчезают через
/// 23 + задержка.
pub const FANG_SNAP: i32 = 1;
pub const FANG_BITE: i32 = 8;
pub const FANG_LIFE: i32 = 23;

/// Урон челюстей (вики, Evoker Fangs: лёгкая 4, обычная 6, трудная 9).
pub fn fang_damage(difficulty: u8) -> f32 {
    match difficulty {
        0 | 1 => 4.0,
        2 => 6.0,
        _ => 9.0,
    }
}

/// Арбалет разбойника: зарядка 25 тактов, звуки начала, середины и конца
/// (чёрный ящик, pillager-crossbow: 17=true в такте 0, loading_start в 5,
/// loading_middle в 13, loading_end в 24, 17=false в 25; свойства слежение
/// показывает на такт позже — отсюда счёт с 6).
fn loading_sound(held: u32, quick: i32) -> Option<(&'static str, f32)> {
    let total = crate::combat::crossbow_charge_ticks(quick);
    if held == total { return Some(("minecraft:item.crossbow.loading_end", 1.0)); }
    // У моба start/middle на такт позже player helper, конец без сдвига:
    // plain6/14/25, QuickIII3/10 (enchant-pillager-crossbow-*).
    held.checked_sub(1).and_then(|held| crate::combat::crossbow_loading_sound(held, quick))
}

/// Пауза заряженного до выстрела: 20 + 0…19 тактов (pillager-crossbow:
/// 34, 38, 38) [проверить: разброс]; после выстрела новая зарядка через 2.
pub const SHOT_DELAY: i32 = 20;
pub const RECHARGE_AFTER: i32 = 1;

/// Дальность выстрела разбойника: 8 блоков (вики, Pillager: «shoots from 8 blocks»).
pub const PILLAGER_RANGE: f64 = 8.0;

/// Поборник: следующий удар — через 25 тактов после прошлого, а не через
/// 20 (чёрный ящик, vindicator-axe: 59 → 84 и 40 → 65); холостой взмах на 5-м
/// такте, флаг «агрессивен» клиент видит снятым на 8-м (свойство уходит
/// тактом позже — снимается на 7-м). В третьем прогоне оригинала всё
/// позже на такт (+6, +9, +26) [проверить: от чего зависит].
pub const VINDICATOR_COOLDOWN: i32 = 25;
pub const VINDICATOR_EMPTY_SWING: i32 = 5;
pub const VINDICATOR_CALM: i32 = 7;

/// Оглушение разорителя после удара в щит и рёв после него (вики, Ravager:
/// «stunned for 2 seconds»; чёрный ящик, ravager-roar: звук рёва через 39
/// тактов после stunned, сам рёв — событие 69, урон и отброс — ещё через 10,
/// следующий укус — через 30 после него).
pub const STUN_TICKS: i32 = 39;
pub const ROAR_DELAY: i32 = 10;
pub const AFTER_ROAR: i32 = 30;

/// Удар разорителя в щит без оглушения отшвыривает игрока: у оригинала
/// скорость после такта 0,78…1,07 по горизонтали и 0,314 вверх, с
/// оглушением — обычный толчок 0,23…0,32 и 0,275 (чёрный ящик, ravager-roar:
/// 9 ударов в щит в двух прогонах). Множитель к обычным 0,218 —
/// [проверить: формула].
pub const RAVAGER_SHOVE: f64 = 4.3;
pub const RAVAGER_PAUSE: i32 = 10;
pub const RAVAGER_SHOVE_UP: f64 = 0.314;

/// Досягаемость укуса разорителя: его рамка, расширенная по горизонтали на
/// √2,04 − 0,6, должна задеть рамку цели (чёрный ящик, ravager-roar: укус
/// с 2,0 блока, а не с 3,9 по правилу «две ширины»).
pub const RAVAGER_REACH: f64 = 0.828_427;

/// К середине набега разбойник без цели идёт, пока дальше 8 блоков (вики,
/// Raid: «raiders … move toward the village») [проверить: расстояние].
pub const RAID_NEAR: f64 = 8.0;

/// Урон рёва разорителя (вики, Ravager: «Roar 4 / 6 / 9»).
pub fn roar_damage(difficulty: u8) -> f32 {
    match difficulty {
        0 | 1 => 4.0,
        2 => 6.0,
        _ => 9.0,
    }
}

/// Вызов в мир: вид, место, задержка, поворот.
pub type Summon = (Kind, (f64, f64, f64), i32, f32);

/// Состояние разбойника.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Текущее заклинание и сколько тактов ему ещё идти.
    pub spell: u8,
    pub spell_left: i32,

    /// Перезарядки заклинаний.
    pub fangs_cooldown: i32,
    pub vex_cooldown: i32,

    /// Разбойник: такт зарядки (0 — не заряжает), заряжен ли и сколько ждать
    /// до выстрела.
    pub charge: i32,
    pub charged: bool,
    pub delay: i32,

    /// Челюсти: задержка до броска, возраст, хозяин уже не нужен (номер — h.owner).
    pub warmup: i32,
    pub age: i32,

    /// Вредина: тактов до начала урона (0 — живёт без срока), рывок к цели.
    pub life: i32,
    pub charging: bool,

    /// Разоритель: такты оглушения, в конце — рёв.
    pub stun: i32,

    /// Вредина: такты блуждания до следующего рывка.
    pub wander: i32,

    /// Капитан патруля или набега: зловещий флаг на голове (вики, Pillager).
    pub captain: bool,

    /// Празднует победу набега (свойство 16).
    pub celebrating: bool,

    /// Набег, в котором участвует (0 — ни в каком; raid.rs), и его середина.
    pub raid: u32,
    pub raid_center: Option<(f64, f64, f64)>,

    /// Игрок, ударивший последним (для героя деревни).
    pub hit_by: i32,

    /// Патруль: куда идёт (raid.rs, вики, Pillager «Patrols»).
    pub patrol: Option<(f64, f64, f64)>,

    /// Разоритель: тактов до самого рёва (после звука).
    pub roar: i32,

    /// Заклинатель: ближняя синяя овца, кого перекрасить этим тактом и пауза «вололо».
    pub sheep: Option<i32>,
    pub wololo_target: Option<i32>,
    pub recolor: Option<i32>,
    pub wololo_cooldown: i32,

    /// Поборник: такт после удара, считая с 1 (0 — не считает), и снятый флаг
    /// «агрессивен» (свойство 15).
    pub since_hit: i32,
    pub calm: bool,

    /// Разоритель: тактов стоять после укуса.
    pub pause: i32,

    /// Разбойник: износ арбалета (по 1 за выстрел) и счётчик перемен
    /// снаряжения для слежения (network/play/mobs.rs).
    pub wear: i32,
    pub gear: u32,

    /// Кого вызвать в мир этим тактом: вредин и челюсти (вид, место, задержка, поворот).
    pub summon: Vec<Summon>,
}

/// Разбойник ли (вики, Raid: участники набега).
pub fn raider(kind: Kind) -> bool {
    matches!(kind, Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner | Kind::Ravager)
}

/// Здоровье (вики: разбойник, поборник, заклинатель 24; иллюзионист 32;
/// вредина 14; разоритель 100).
pub fn max_health(kind: Kind) -> f32 {
    match kind {
        Kind::Pillager | Kind::Vindicator | Kind::Evoker => 24.0,
        Kind::Illusioner => 32.0,
        Kind::Vex => 14.0,
        Kind::Ravager => 100.0,
        _ => 1.0,
    }
}

/// Скорость шага (чёрный ящик: атрибуты разбойника и поборника 0,35,
/// заклинателя 0,5, вредины 0,7, разорителя 0,3).
pub fn speed(kind: Kind) -> f64 {
    match kind {
        Kind::Pillager | Kind::Vindicator => 0.35,
        Kind::Evoker | Kind::Illusioner => 0.5,
        Kind::Vex => 0.7,
        Kind::Ravager => 0.3,
        _ => 0.0,
    }
}

/// Голос (вики, Sounds; minecraft-data: sounds.json).
pub fn sounds(kind: Kind) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Pillager => ("minecraft:entity.pillager.ambient", "minecraft:entity.pillager.hurt", "minecraft:entity.pillager.death"),
        Kind::Vindicator => ("minecraft:entity.vindicator.ambient", "minecraft:entity.vindicator.hurt", "minecraft:entity.vindicator.death"),
        Kind::Evoker => ("minecraft:entity.evoker.ambient", "minecraft:entity.evoker.hurt", "minecraft:entity.evoker.death"),
        Kind::Illusioner => ("minecraft:entity.illusioner.ambient", "minecraft:entity.illusioner.hurt", "minecraft:entity.illusioner.death"),
        Kind::Vex => ("minecraft:entity.vex.ambient", "minecraft:entity.vex.hurt", "minecraft:entity.vex.death"),
        Kind::Ravager => ("minecraft:entity.ravager.ambient", "minecraft:entity.ravager.hurt", "minecraft:entity.ravager.death"),
        _ => ("", "", ""),
    }
}

/// Что в руке у призванного (вики: разбойник — арбалет, поборник — железный
/// топор, вредина — железный меч, иллюзионист — лук).
pub fn hand_item(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Pillager => Some("crossbow"),
        Kind::Vindicator => Some("iron_axe"),
        Kind::Vex => Some("iron_sword"),
        Kind::Illusioner => Some("bow"),
        _ => None,
    }
}

/// Урон в ближнем бою (вики: поборник с топором 7,5 / 13 / 19,5; вредина с
/// мечом 5,5 / 9 / 13,5; разоритель 7 / 12 / 18; чёрный ящик, vindicator-axe:
/// 20 → 7 на обычной).
pub fn melee_damage(kind: Kind, difficulty: u8) -> f32 {
    let (easy, normal, hard) = match kind {
        Kind::Vindicator => (7.5, 13.0, 19.5),
        Kind::Vex => (5.5, 9.0, 13.5),
        Kind::Ravager => (7.0, 12.0, 18.0),
        _ => (2.0, 3.0, 4.5),
    };

    match difficulty {
        0 | 1 => easy,
        2 => normal,
        _ => hard,
    }
}

/// Добыча (вики): разбойник — арбалет 8,5 %, капитан — зловещий флаг и
/// зловещая бутылка; поборник — 0–1 изумруд (игрок) и топор 8,5 %;
/// заклинатель — тотем бессмертия всегда и 0–1 изумруд (игрок);
/// разоритель — седло; иллюзионист — лук 8,5 %.
pub fn loot(mob: &Mob, give: &mut dyn FnMut(&str, i32, i32, &mut Rng), rng: &mut Rng) {
    let looting = mob.looting.max(0);
    let hand_chance = |rng: &mut Rng| mob.equipment.is_none() && rng.unit() < f64::from(super::DEFAULT_DROP) + 0.01 * f64::from(looting);

    match mob.kind {
        Kind::Pillager => {
            if mob.by_player && hand_chance(rng) {
                give("crossbow", 1, 1, rng);
            }

            captain_loot(mob, give, rng);
        }
        Kind::Vindicator => {
            captain_loot(mob, give, rng);

            if mob.by_player {
                give("emerald", 0, 1 + looting, rng);

                if hand_chance(rng) {
                    give("iron_axe", 1, 1, rng);
                }
            }
        }
        Kind::Evoker => {
            give("totem_of_undying", 1, 1, rng);

            if mob.by_player {
                give("emerald", 0, 1 + looting, rng);
            }
        }
        Kind::Illusioner => {
            if mob.by_player && hand_chance(rng) {
                give("bow", 1, 1, rng);
            }
        }
        Kind::Ravager if mob.by_player => give("saddle", 1, 1, rng),
        _ => {}
    }
}

/// Капитан роняет зловещий флаг, а вне набега — и зловещую бутылку уровня
/// 1…5 (вики, Pillager: «Always drops an Ominous Bottle and an Ominous
/// Banner … level is randomized from 1 to 5»; «Captains participating in a
/// raid don't drop bottles»). Сам флаг — с узором (stackdata), бутылка — с
/// уровнем: их кладёт loot по именам `ominous_banner` и `ominous_bottle:N`.
fn captain_loot(mob: &Mob, give: &mut dyn FnMut(&str, i32, i32, &mut Rng), rng: &mut Rng) {
    if !mob.illager.captain {
        return;
    }

    if mob.equipment.is_none() { give("ominous_banner", 1, 1, rng); }

    if mob.illager.raid == 0 {
        let level = rng.below(5);

        give(OMINOUS_BOTTLES[level as usize], 1, 1, rng);
    }
}

/// Имена бутылок по уровню для `loot` (mob/mod.rs разбирает приставку).
pub const OMINOUS_BOTTLES: [&str; 5] = ["ominous_bottle:0", "ominous_bottle:1", "ominous_bottle:2", "ominous_bottle:3", "ominous_bottle:4"];

/// Мысль разбойника за такт (после голоса и паузы удара в hostile::think).
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, mut plan: Plan) -> Plan {
    if mob.kind == Kind::Vex {
        return vex(mob, ctx, rng, plan);
    }

    let here = mob.cell();
    let grounded = mob.on_ground || super::is_water(view.block(here.0, here.1, here.2));

    // Заклинание идёт и без цели: до конца стоит и смотрит.
    if matches!(mob.kind, Kind::Evoker | Kind::Illusioner) {
        mob.illager.fangs_cooldown = (mob.illager.fangs_cooldown - 1).max(0);
        mob.illager.vex_cooldown = (mob.illager.vex_cooldown - 1).max(0);
        mob.illager.wololo_cooldown = (mob.illager.wololo_cooldown - 1).max(0);
    }

    // Поборник после удара: через 5 тактов — взмах вхолостую, через 8 —
    // флаг «агрессивен» снят (чёрный ящик, vindicator-axe: оба прогона
    // одинаковы) [проверить: отчего так у оригинала].
    if mob.kind == Kind::Vindicator && mob.illager.since_hit > 0 {
        mob.illager.since_hit += 1;

        match mob.illager.since_hit - 1 {
            VINDICATOR_EMPTY_SWING => mob.events.push(Event::Swing),
            VINDICATOR_CALM => {
                mob.illager.calm = true;
                mob.illager.since_hit = 0;
            }
            _ => {}
        }
    }

    if mob.kind == Kind::Ravager && (ravager_stunned(mob) || ravager_roar(mob, ctx, &mut plan)) {
        return plan;
    }

    // Разоритель после укуса стоит (чёрный ящик, ravager-roar: скорость
    // гаснет 10 тактов, ход — лишь через 16; укусы без оглушения — раз в 29
    // тактов, а не в 20) [проверить: точный срок].
    if mob.kind == Kind::Ravager && mob.illager.pause > 0 {
        mob.illager.pause -= 1;
        mob.brain.stop();

        return plan;
    }

    let target = hostile::choose_target(mob, view, ctx, rng);

    if mob.illager.spell != SPELL_NONE {
        cast(mob, view, ctx, target.as_ref(), rng);

        return plan;
    }

    let Some(target) = target else {
        mob.illager.charge = 0;
        mob.illager.calm = false;

        // В набеге — к середине деревни, в патруле — к его точке.
        let goal = mob.illager.raid_center.filter(|_| mob.illager.raid != 0).or(mob.illager.patrol);

        if let Some(goal) = goal
            && grounded
            && hostile::dist2(mob, goal.0, goal.1, goal.2) > RAID_NEAR * RAID_NEAR
        {
            return walk_to(mob, view, goal, plan);
        }

        if mob.illager.raid == 0 {
            mob.illager.patrol = None;
        }

        // Без цели заклинатель перекрашивает синих овец.
        if mob.kind == Kind::Evoker && mob.illager.wololo_cooldown == 0 && let Some(sheep) = mob.illager.sheep {
            begin(mob, SPELL_WOLOLO, WOLOLO_SPELL, "minecraft:entity.evoker.prepare_wololo");
            mob.illager.wololo_cooldown = WOLOLO_COOLDOWN + WOLOLO_SPELL;
            mob.illager.wololo_target = Some(sheep);

            return plan;
        }

        return hostile::idle_walk(mob, view, ctx, rng, plan, grounded);
    };

    let (dx, dz) = (target.x - mob.x, target.z - mob.z);
    let flat = (dx * dx + dz * dz).sqrt();
    let dist_sq = hostile::dist2(mob, target.x, target.y, target.z);
    let sight = hostile::line_of_sight(mob, view, &target);

    match mob.kind {
        Kind::Pillager => pillager(mob, view, &target, &mut plan, dist_sq, sight, rng, grounded),
        Kind::Ravager => ravager(mob, view, ctx, &target, &mut plan, dist_sq, flat, rng, grounded),
        Kind::Vindicator => {
            hostile::melee(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded);

            if let Some(melee) = plan.melee.as_mut() {
                melee.damage = melee_damage(mob.kind, ctx.difficulty);
                mob.h.cooldown = VINDICATOR_COOLDOWN;
                mob.illager.since_hit = 1;
            }
        }
        Kind::Evoker => {
            ai::look_at_point(mob, target.x, target.y + target.eye, target.z);
            start_spell(mob, ctx, &target, sight);
        }
        Kind::Illusioner => illusioner(mob, view, &target, &mut plan, dist_sq, sight, grounded),
        _ => {}
    }

    plan
}

/// Идёт к точке кусками до 16 блоков (середина набега, точка патруля).
fn walk_to(mob: &mut Mob, view: &dyn View, goal: (f64, f64, f64), plan: Plan) -> Plan {
    mob.brain.repath -= 1;

    if !mob.brain.walking() || mob.brain.repath <= 0 {
        mob.brain.repath = 20;

        let here = mob.cell();
        let (dx, dz) = (goal.0 - mob.x, goal.2 - mob.z);
        let length = (dx * dx + dz * dz).sqrt().max(1.0e-6);
        let step = length.min(16.0);
        let cell = ((mob.x + dx / length * step).floor() as i32, here.1, (mob.z + dz / length * step).floor() as i32);
        let cell = ai::settle(view, cell, here.1);

        mob.brain.go(super::path::find(view, here, cell, super::path::MAX_NODES), 1.0);
    }

    ai::walk(mob, plan)
}

/// Что разоритель ломает на ходу (вики, Ravager, «Block breaking»).
pub fn ravager_breaks(name: &str) -> bool {
    name.ends_with("_leaves") || matches!(name, "beetroots" | "carrots" | "potatoes" | "wheat" | "pitcher_crop" | "torchflower_crop")
}

/// Клетки рамки разорителя, расширенной на 0,2 (вики, Ravager) [проверить: запас].
pub fn ravager_reach_cells((x, y, z): (f64, f64, f64)) -> Vec<(i32, i32, i32)> {
    let half = Kind::Ravager.width() / 2.0 + 0.2;
    let mut cells = Vec::new();

    for bx in (x - half).floor() as i32..=(x + half).floor() as i32 {
        for by in y.floor() as i32..=(y + Kind::Ravager.height()).floor() as i32 {
            for bz in (z - half).floor() as i32..=(z + half).floor() as i32 {
                cells.push((bx, by, bz));
            }
        }
    }

    cells
}

/// Иллюзионист: стреляет из лука с 15 блоков раз в 40 тактов (вики,
/// Illusioner: «shoots arrows»; без заклинаний слепоты и двойников)
/// [проверить: такты, заклинания].
fn illusioner(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, grounded: bool) {
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    if dist_sq <= 15.0 * 15.0 && sight {
        mob.brain.stop();

        if mob.h.cooldown == 0 {
            mob.h.cooldown = 40;
            mob.events.push(Event::Sound { name: "minecraft:entity.skeleton.shoot", volume: 1.0, pitch: 1.0 });
            plan.shot = Some(Shot { from: (mob.x, mob.y + mob.eye() - 0.1, mob.z), aim: (target.x, target.y + 0.6, target.z), speed: hostile::ARROW_SPEED, trident: false, crossbow: false });
        }
    } else if grounded {
        hostile::approach(mob, view, target, 1.0);
        *plan = ai::walk(mob, std::mem::take(plan));
    }
}

/// Разоритель: подходит и кусает; досягаемость — по рамкам (RAVAGER_REACH).
#[allow(clippy::too_many_arguments)]
fn ravager(mob: &mut Mob, view: &dyn View, ctx: &Ctx, target: &Watcher, plan: &mut Plan, dist_sq: f64, flat: f64, rng: &mut Rng, grounded: bool) {
    let reach = mob.kind.width() / 2.0 + RAVAGER_REACH + 0.3;
    let close = (target.x - mob.x).abs() <= reach && (target.z - mob.z).abs() <= reach && (target.y - mob.y).abs() < 2.2;

    if !close {
        // Подход — как у прочих: путь к цели и шаг.
        if grounded {
            hostile::approach(mob, view, target, 1.0);
        }

        *plan = ai::walk(mob, std::mem::take(plan));
        let _ = dist_sq;

        return;
    }

    mob.brain.stop();
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    if mob.h.cooldown == 0 && hostile::line_of_sight(mob, view, target) {
        mob.h.cooldown = hostile::MELEE_COOLDOWN;

        let (ux, uz) = if flat > 1.0e-6 { ((target.x - mob.x) / flat, (target.z - mob.z) / flat) } else { (0.0, 1.0) };

        // Событие 4 и звук укуса (чёрный ящик, ravager-roar).
        mob.events.push(Event::Swing);
        mob.events.push(Event::Status(4));
        mob.events.push(Event::Sound { name: "minecraft:entity.ravager.attack", volume: 1.0, pitch: 1.0 });
        mob.illager.pause = RAVAGER_PAUSE;
        // Удар в щит без оглушения отшвыривает игрока сильнее: толчок
        // длиннее единицы (network/play/vitals.rs, mob_hit).
        let shove = if ravager_blocked(mob, target, rng) == Some(false) { RAVAGER_SHOVE } else { 1.0 };

        plan.melee = Some(Melee { target: target.id, damage: melee_damage(Kind::Ravager, ctx.difficulty), push: (ux * shove, uz * shove), wither: false, hunger: false });
    }
}

/// Разбойник: подходит на 8 блоков, заряжает арбалет 25 тактов, ждёт
/// 20–39 и стреляет (чёрный ящик, pillager-crossbow).
#[allow(clippy::too_many_arguments)]
fn pillager(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, rng: &mut Rng, grounded: bool) {
    let Some(crossbow) = super::equipment::slot(mob, 0).filter(|stack| blocks::item_name(stack.item) == Some("crossbow")) else {
        mob.illager.charge = 0;
        mob.illager.charged = false;
        return;
    };
    let data = crossbow.extras();
    let quick = data.level("quick_charge");
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    let in_range = dist_sq <= PILLAGER_RANGE * PILLAGER_RANGE && sight;

    // Видит цель меньше пяти тактов — ещё идёт к ней (чёрный ящик,
    // pillager-crossbow: из 7 блоков подошёл на 1,1) [проверить].
    mob.h.see_time = if sight { mob.h.see_time + 1 } else { 0 };

    if in_range && mob.h.see_time >= 5 {
        mob.brain.stop();
    } else if grounded {
        hostile::approach(mob, view, target, 1.0);
        *plan = ai::walk(mob, std::mem::take(plan));
    }

    let state = &mut mob.illager;

    if state.charged {
        state.delay -= 1;

        if state.delay <= 0 && in_range {
            state.charged = false;
            state.charge = -RECHARGE_AFTER;
            let count = if data.level("multishot") > 0 { 3 } else { 1 };
            if crate::combat::max_damage(&crossbow) > 0 {
                state.wear += crate::combat::wear(&crossbow, count, false, || rng.unit() as f32);
            }
            state.gear += 1;
            mob.events.push(Event::Sound { name: "minecraft:item.crossbow.shoot", volume: 1.0, pitch: 1.0 });
            if count == 3 {
                for extra in [0.63, 0.43] {
                    let pitch = 1.0 / (rng.unit() as f32 * 0.5 + 1.8) + extra;
                    mob.events.push(Event::Sound { name: "minecraft:item.crossbow.shoot", volume: 1.0, pitch });
                }
            }
            let broken = crate::combat::max_damage(&crossbow) > 0 && state.wear >= crate::combat::max_damage(&crossbow);
            plan.shot = Some(Shot {
                from: (mob.x, mob.y + mob.eye() - 0.1, mob.z),
                aim: (target.x, target.y + 0.6, target.z),
                speed: hostile::ARROW_SPEED,
                trident: false,
                crossbow: true,
            });
            if broken {
                super::equipment::set_slot(mob, 0, None);
                mob.events.push(Event::Status(47));
                mob.events.push(Event::Sound { name: "minecraft:entity.item.break", volume: 0.8, pitch: 0.8 });
            }
        }

        return;
    }

    // Пауза после выстрела до новой зарядки.
    if state.charge < 0 {
        state.charge += 1;

        if state.charge < 0 {
            return;
        }
    }

    state.charge += 1;

    if let Some((name, volume)) = loading_sound((state.charge - 1) as u32, quick) {
        let pitch = if name == "minecraft:item.crossbow.loading_end" { 1.0 / (rng.unit() as f32 * 0.5 + 1.0) + 0.2 } else { 1.0 };
        mob.events.push(Event::Sound { name, volume, pitch });
    }

    if mob.illager.charge > crate::combat::crossbow_charge_ticks(quick) as i32 {
        mob.illager.charge = 0;
        mob.illager.charged = true;
        mob.illager.gear += 1;
        mob.illager.delay = SHOT_DELAY + rng.below(20) as i32;
    }
}

/// Заряжает ли разбойник арбалет (свойство 17 и «рука занята» 8=1).
pub fn charging(mob: &Mob) -> bool {
    mob.kind == Kind::Pillager && mob.illager.charge > 0 && !mob.illager.charged
}

/// Заклинатель выбирает заклинание: сперва вредин (если их рядом меньше
/// восьми и перезарядка прошла), потом челюсти; «вололо» — вне боя.
fn start_spell(mob: &mut Mob, ctx: &Ctx, target: &Watcher, sight: bool) {
    if mob.kind == Kind::Evoker && mob.illager.vex_cooldown == 0 {
        let vexes = ctx.mates.iter().filter(|mate| mate.kind == Kind::Vex && (mate.x - mob.x).powi(2) + (mate.y - mob.y).powi(2) + (mate.z - mob.z).powi(2) <= 256.0).count();

        if vexes < VEX_LIMIT {
            begin(mob, SPELL_VEX, VEX_SPELL, "minecraft:entity.evoker.prepare_summon");
            mob.illager.vex_cooldown = VEX_COOLDOWN;

            return;
        }
    }

    if mob.kind == Kind::Evoker && mob.illager.fangs_cooldown == 0 && sight {
        begin(mob, SPELL_FANGS, FANGS_SPELL, "minecraft:entity.evoker.prepare_attack");
        mob.illager.fangs_cooldown = FANGS_COOLDOWN + FANGS_SPELL;
        mob.illager.summon.clear();
        let _ = target;
    }
}

fn begin(mob: &mut Mob, spell: u8, length: i32, sound: &'static str) {
    mob.illager.spell = spell;
    mob.illager.spell_left = length;
    mob.brain.stop();
    mob.events.push(Event::Sound { name: sound, volume: 1.0, pitch: 1.0 });
}

/// Ход заклинания: через 18 тактов — само заклинание (звук cast_spell).
fn cast(mob: &mut Mob, view: &dyn View, ctx: &Ctx, target: Option<&Watcher>, rng: &mut Rng) {
    mob.brain.stop();
    mob.illager.spell_left -= 1;

    let elapsed = match mob.illager.spell {
        SPELL_VEX => VEX_SPELL,
        SPELL_WOLOLO => WOLOLO_SPELL,
        _ => FANGS_SPELL,
    } - mob.illager.spell_left;

    if let Some(target) = target {
        ai::look_at_point(mob, target.x, target.y + target.eye, target.z);
    }

    if elapsed == SPELL_WARMUP {
        match mob.illager.spell {
            SPELL_VEX => {
                let (x, y, z) = mob.cell();

                for _ in 0..VEXES {
                    let at = (f64::from(x + rng.below(5) as i32 - 2) + 0.5, f64::from(y + 1), f64::from(z + rng.below(5) as i32 - 2) + 0.5);

                    mob.illager.summon.push((Kind::Vex, at, 0, 0.0));
                }
            }
            SPELL_FANGS => {
                if let Some(target) = target {
                    fangs_spell(mob, view, target);
                }
            }
            SPELL_WOLOLO => mob.illager.recolor = mob.illager.wololo_target.take(),
            _ => {}
        }

        mob.events.push(Event::Sound { name: "minecraft:entity.evoker.cast_spell", volume: 1.0, pitch: 1.0 });
    }

    let _ = ctx;

    if mob.illager.spell_left <= 0 {
        mob.illager.spell = SPELL_NONE;
        mob.illager.spell_left = 0;
    }
}

/// Челюсти: линия к цели или два кольца, если цель ближе трёх блоков.
fn fangs_spell(mob: &mut Mob, view: &dyn View, target: &Watcher) {
    let low = mob.y.min(target.y);
    let high = mob.y.max(target.y) + 1.0;
    let angle = (target.z - mob.z).atan2(target.x - mob.x);

    if hostile::dist2(mob, target.x, target.y, target.z) < 9.0 {
        for index in 0..5 {
            let turn = angle + f64::from(index) * std::f64::consts::PI * 0.4;

            place_fang(mob, view, mob.x + turn.cos() * 1.5, mob.z + turn.sin() * 1.5, low, high, 0, turn);
        }

        for index in 0..8 {
            let turn = angle + f64::from(index) * std::f64::consts::PI * 2.0 / 8.0 + std::f64::consts::PI * 2.0 / 5.0;

            place_fang(mob, view, mob.x + turn.cos() * 2.5, mob.z + turn.sin() * 2.5, low, high, 3, turn);
        }

        return;
    }

    for index in 0..FANG_LINE {
        let step = FANG_STEP * (index as f64 + 1.0);

        place_fang(mob, view, mob.x + angle.cos() * step, mob.z + angle.sin() * step, low, high, index as i32, angle);
    }
}

/// Одна челюсть: на полу между низом и верхом (сверху вниз), иначе не ставится.
#[allow(clippy::too_many_arguments)]
fn place_fang(mob: &mut Mob, view: &dyn View, x: f64, z: f64, low: f64, high: f64, warmup: i32, angle: f64) {
    let (cx, cz) = (x.floor() as i32, z.floor() as i32);
    let mut y = high.floor() as i32;

    while f64::from(y) >= low.floor() - 1.0 {
        if super::solid(view.block(cx, y - 1, cz)) && !super::solid(view.block(cx, y, cz)) {
            let yaw = (angle.to_degrees() - 90.0) as f32;

            mob.illager.summon.push((Kind::EvokerFangs, (x, f64::from(y), z), warmup, yaw));

            return;
        }

        y -= 1;
    }
}

/// Такт челюстей: событие 4 (клиент играет пасть и звук), укус всех в рамке,
/// уход (чёрный ящик, evoker-fangs).
pub fn fangs(mob: &mut Mob, ctx: &Ctx) -> Plan {
    let mut plan = Plan::default();

    mob.illager.age += 1;

    let age = mob.illager.age - mob.illager.warmup;

    if age == FANG_SNAP {
        mob.events.push(Event::Status(4));
    }

    if age == FANG_BITE {
        // Рамка челюстей 0,5 × 0,8, расширенная на 0,2 (вики, Evoker Fangs) [проверить].
        let half = 0.25 + 0.2;

        for player in ctx.players.iter().filter(|player| player.targetable && !player.spectator) {
            if (player.x - mob.x).abs() <= half + 0.3 && player.y <= mob.y + 1.0 && player.y + 1.8 >= mob.y - 0.2 && (player.z - mob.z).abs() <= half + 0.3 {
                let (dx, dz) = (player.x - mob.x, player.z - mob.z);
                let length = (dx * dx + dz * dz).sqrt();
                let push = if length > 1.0e-6 { (dx / length, dz / length) } else { (0.0, 0.0) };

                plan.melee = Some(Melee { target: player.id, damage: fang_damage(ctx.difficulty), push, wither: false, hunger: false });
                break;
            }
        }
    }

    if age >= FANG_LIFE {
        mob.h.despawn = true;
    }

    plan
}

/// Вредина: летит к цели, рывком бьёт; со сроком — после него теряет по 1
/// здоровья в секунду (вики, Vex).
fn vex(mob: &mut Mob, ctx: &Ctx, rng: &mut Rng, mut plan: Plan) -> Plan {
    if mob.illager.life > 0 {
        mob.illager.life -= 1;

        if mob.illager.life == 0 {
            mob.illager.life = -1;
        }
    }

    if mob.illager.life < 0 && mob.ticks.is_multiple_of(20) {
        mob.hurt(1.0, crate::survival::Source::Generic, None);
    }

    let target = mob.h.target.and_then(|id| ctx.players.iter().find(|player| player.id == id && player.targetable && !player.spectator)).copied().or_else(|| {
        ctx.players
            .iter()
            .filter(|player| player.targetable && !player.spectator)
            .filter(|player| (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2) <= 16.0 * 16.0)
            .min_by(|a, b| hostile::dist2(mob, a.x, a.y, a.z).total_cmp(&hostile::dist2(mob, b.x, b.y, b.z)))
            .copied()
    });

    let Some(target) = target else {
        mob.h.target = None;
        mob.illager.charging = false;

        return plan;
    };

    mob.h.target = Some(target.id);
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    // Рывок — к глазам цели (вики, Vex: «charges at the player») [проверить].
    let (dx, dy, dz) = (target.x - mob.x, target.y + target.eye - mob.y, target.z - mob.z);
    let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-6);

    // Рывок: с шансом 1/7 за такт, когда не блуждает; после рывка — 40–79
    // тактов блуждания возле цели (чёрный ящик, evoker-fangs: из трёх вредин
    // за 240 тактов рывков два) [проверить: блуждание по вики, Vex].
    if mob.illager.wander > 0 {
        mob.illager.wander -= 1;
    } else if !mob.illager.charging && rng.below(7) == 0 {
        mob.illager.charging = true;
        mob.illager.charge = 0;
        mob.events.push(Event::Sound { name: "minecraft:entity.vex.charge", volume: 1.0, pitch: 1.0 });
    }

    if mob.illager.charging {
        mob.illager.charge += 1;

        if mob.illager.charge > 40 {
            stop_charge(mob, rng);
        }
    }

    let speed = if mob.illager.charging { 1.0 } else { 0.25 };
    let (aim_x, aim_y, aim_z) = if mob.illager.charging || mob.illager.wander == 0 { (dx, dy, dz) } else { (dx + 4.0, dy + 3.0, dz) };
    let aim = (aim_x * aim_x + aim_y * aim_y + aim_z * aim_z).sqrt().max(1.0e-6);

    mob.vx = aim_x / aim * speed * 0.1 + mob.vx * 0.9;
    mob.vy = aim_y / aim * speed * 0.1 + mob.vy * 0.9;
    mob.vz = aim_z / aim * speed * 0.1 + mob.vz * 0.9;

    // Бьёт, лишь коснувшись рамкой рамки цели: у оригинала из семи рывков
    // попали два (чёрный ящик, evoker-fangs), а не каждый.
    let touches = (target.x - mob.x).abs() < (0.6 + Kind::Vex.width()) / 2.0
        && (target.z - mob.z).abs() < (0.6 + Kind::Vex.width()) / 2.0
        && mob.y < target.y + 1.8
        && mob.y + Kind::Vex.height() > target.y;

    if mob.illager.charging && touches && mob.h.cooldown == 0 {
        mob.h.cooldown = hostile::MELEE_COOLDOWN;
        stop_charge(mob, rng);
        mob.events.push(Event::Swing);
        plan.melee = Some(Melee { target: target.id, damage: melee_damage(Kind::Vex, ctx.difficulty), push: (dx / length, dz / length), wither: false, hunger: false });
    }

    plan
}

fn stop_charge(mob: &mut Mob, rng: &mut Rng) {
    mob.illager.charging = false;
    mob.illager.charge = 0;
    mob.illager.wander = 40 + rng.below(40) as i32;
}

/// Полёт вредины: сквозь блоки, без тяжести (вики, Vex: «can fly through blocks»).
pub fn vex_fly(mob: &mut Mob) {
    mob.x += mob.vx;
    mob.y += mob.vy;
    mob.z += mob.vz;
    mob.on_ground = false;
}

/// Удар разорителя пришёлся в щит игрока, что смотрит на него: оглушение на
/// 39 тактов с шансом 1/2 (вики, Ravager: «50% chance … stunned»; чёрный
/// ящик, ravager-roar: 3 оглушения на 5 ударов в щит).
/// Возвращает None, если щит удар не принял, иначе — оглушён ли разоритель.
fn ravager_blocked(mob: &mut Mob, target: &Watcher, rng: &mut Rng) -> Option<bool> {
    let shield = target.holding.contains(&Some("shield"));

    if !shield || !crate::combat::shield_covers(target.look.0, (target.x, target.z), (mob.x, mob.z)) {
        return None;
    }

    let stunned = rng.below(2) == 0;

    if stunned {
        mob.illager.stun = STUN_TICKS;
        mob.events.push(Event::Sound { name: "minecraft:entity.ravager.stunned", volume: 1.0, pitch: 1.0 });
        mob.events.push(Event::Status(39));
    }

    Some(stunned)
}

/// Оглушённый разоритель стоит; в конце — рёв: урон и отброс всех рядом,
/// кроме разбойников (вики, Ravager: «roar … 6 damage … knockback»).
fn ravager_stunned(mob: &mut Mob) -> bool {
    if mob.illager.stun <= 0 {
        return false;
    }

    mob.brain.stop();
    mob.illager.stun -= 1;

    if mob.illager.stun == 0 {
        mob.events.push(Event::Sound { name: "minecraft:entity.ravager.roar", volume: 1.0, pitch: 1.0 });
        mob.illager.roar = ROAR_DELAY;
    }

    true
}

/// Рёв после оглушения: через 10 тактов после звука — событие 69, урон и
/// отброс всех в 4 блоках, кроме разбойников (вики, Ravager; чёрный ящик,
/// ravager-roar: 69 и щит в такте 92 при звуке в 82).
fn ravager_roar(mob: &mut Mob, ctx: &Ctx, plan: &mut Plan) -> bool {
    if mob.illager.roar <= 0 {
        return false;
    }

    mob.brain.stop();
    mob.illager.roar -= 1;

    if mob.illager.roar == 0 {
        mob.events.push(Event::Status(69));
        mob.h.cooldown = AFTER_ROAR;

        for player in ctx.players.iter().filter(|player| player.targetable && !player.spectator) {
            if hostile::dist2(mob, player.x, player.y, player.z) <= 4.0 * 4.0 {
                let (dx, dz) = (player.x - mob.x, player.z - mob.z);
                let length = (dx * dx + dz * dz).sqrt().max(1.0e-6);

                plan.melee = Some(Melee { target: player.id, damage: roar_damage(ctx.difficulty), push: (dx / length, dz / length), wither: false, hunger: false });
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_follow_the_wiki() {
        assert_eq!(max_health(Kind::Ravager), 100.0);
        assert_eq!(max_health(Kind::Vex), 14.0);
        assert_eq!(melee_damage(Kind::Vindicator, 2), 13.0);
        assert_eq!(fang_damage(2), 6.0);
        assert!(raider(Kind::Pillager) && !raider(Kind::Vex));
    }

    #[test]
    fn fang_line_is_sixteen_long() {
        struct Flat;

        impl View for Flat {
            fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
                if y < 0 { 1 } else { 0 }
            }

            fn loaded(&self, _x: i32, _z: i32) -> bool {
                true
            }
        }

        let mut evoker = Mob::new(1, Kind::Evoker, (7.5, 0.0, 0.5));
        let target = Watcher { id: 2, x: 0.5, y: 0.0, z: 0.5, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false };

        fangs_spell(&mut evoker, &Flat, &target);

        assert_eq!(evoker.illager.summon.len(), FANG_LINE);
        // Чёрный ящик, evoker-fangs: первая челюсть в 1,25 блока к цели.
        assert!((evoker.illager.summon[0].1 .0 - 6.25).abs() < 1.0e-9);
        assert_eq!(evoker.illager.summon[15].2, 15);
    }

    struct CrossbowFloor;
    impl View for CrossbowFloor {
        fn block(&self, _: i32, y: i32, _: i32) -> i32 { if y < 0 { 1 } else { 0 } }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }
    fn crossbow_target() -> Watcher {
        Watcher { id: 2, x: 0.5, y: 0.0, z: 6.5, eye: 1.62, holding: [None, None], spectator: false, targetable: true,
            look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false }
    }
    fn crossbow(quick: i32, multi: i32, unbreaking: i32) -> crate::inventory::Stack {
        let mut data = crate::stackdata::Data::default();
        for (name, level) in [("quick_charge", quick), ("multishot", multi), ("unbreaking", unbreaking)] {
            data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), level);
        }
        crate::inventory::Stack::new(blocks::item_named("crossbow").unwrap(), 1).with_extras(data)
    }

    #[test]
    fn actual_pillager_charge_keeps_plain_phase_and_shortens_quick_charge() {
        for (quick, expected) in [(0, vec![7, 15, 26]), (3, vec![4, 11])] {
            let mut mob = Mob::new(1, Kind::Pillager, (0.5, 0.0, 0.5));
            super::super::equipment::set_slot(&mut mob, 0, Some(crossbow(quick, 0, 0)));
            let mut rng = Rng::new(42);
            let mut sounds = Vec::new();
            for tick in 1..=26 {
                let mut plan = Plan::default();
                pillager(&mut mob, &CrossbowFloor, &crossbow_target(), &mut plan, 36.0, true, &mut rng, false);
                for event in std::mem::take(&mut mob.events) {
                    if let Event::Sound { name, .. } = event {
                        sounds.push(tick);
                        if quick > 0 { assert_ne!(name, "minecraft:item.crossbow.loading_middle"); }
                    }
                }
                if mob.illager.charged { assert_eq!(tick, if quick == 0 { 26 } else { 11 }); break; }
            }
            assert_eq!(sounds, expected);
            assert!(mob.illager.charged);
            assert!((20..40).contains(&mob.illager.delay));
        }
    }

    #[test]
    fn pillager_multishot_wears_each_arrow_and_unbreaking_protects_it() {
        for level in [0, 3] {
            let mut mob = Mob::new(1, Kind::Pillager, (0.5, 0.0, 0.5));
            super::super::equipment::set_slot(&mut mob, 0, Some(crossbow(0, 1, level)));
            let mut rng = Rng::new(42);
            for _ in 0..50 {
                mob.illager.charged = true; mob.illager.delay = 1;
                let mut plan = Plan::default();
                pillager(&mut mob, &CrossbowFloor, &crossbow_target(), &mut plan, 36.0, true, &mut rng, false);
                assert!(plan.shot.unwrap().crossbow);
                assert_eq!(std::mem::take(&mut mob.events).iter().filter(|event| matches!(event, Event::Sound { name: "minecraft:item.crossbow.shoot", .. })).count(), 3);
            }
            if level == 0 { assert_eq!(mob.illager.wear, 150); }
            else { assert!((15..70).contains(&mob.illager.wear)); }
        }
        let mut mob = Mob::new(1, Kind::Pillager, (0.5, 0.0, 0.5));
        let stack = crossbow(0, 1, 0);
        let mut data = stack.extras(); data.unbreakable = true;
        super::super::equipment::set_slot(&mut mob, 0, Some(stack.with_extras(data)));
        mob.illager.charged = true; mob.illager.delay = 1;
        let mut plan = Plan::default();
        pillager(&mut mob, &CrossbowFloor, &crossbow_target(), &mut plan, 36.0, true, &mut Rng::new(1), false);
        assert!(plan.shot.is_some());
        assert_eq!(mob.illager.wear, 0);
        let mut data = stack.extras(); data.damage = crate::combat::max_damage(&stack) - 1;
        super::super::equipment::set_slot(&mut mob, 0, Some(stack.with_extras(data)));
        mob.illager.charged = true; mob.illager.delay = 1;
        let mut plan = Plan::default();
        pillager(&mut mob, &CrossbowFloor, &crossbow_target(), &mut plan, 36.0, true, &mut Rng::new(1), false);
        assert!(plan.shot.is_some());
        assert!(super::super::equipment::slot(&mob, 0).is_none());
        let mut next = Plan::default();
        pillager(&mut mob, &CrossbowFloor, &crossbow_target(), &mut next, 36.0, true, &mut Rng::new(1), false);
        assert!(next.shot.is_none());
        assert!(!charging(&mob));
    }

    #[test]
    fn pillager_charges_for_25_ticks() {
        let mut pillager = Mob::new(1, Kind::Pillager, (0.5, 0.0, 0.5));

        pillager.illager.charge = 1;
        assert!(charging(&pillager));
        pillager.illager.charged = true;
        assert!(!charging(&pillager));
    }
}
