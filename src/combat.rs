// Бой: свойства предметов, урон ближнего боя, броня, зачарования, щит, лук.
//
// Здесь только счёт — без сети и без состояния игроков; подключения
// (network/play/combat.rs) берут числа отсюда. Источники: вики (Damage,
// Attack cooldown / Melee attack, Armor, Shield, Knockback, страницы
// зачарований, Bow, Arrow), таблица предметов combat_table.rs (отчёты штатного
// генератора) и сверка с оригиналом чёрным ящиком (сценарии pvp-*).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::attributes::{Attributes, Modifier, Operation};
use crate::combat_table::{ItemInfo, ITEMS};
use crate::inventory::{self, Inventory, Stack};
use crate::players::CombatView;
use crate::stackdata::Data;
use crate::survival::Source;

/// Чем кончится удар по игроку, если судить по тому, что он опубликовал:
/// щит держит удар, иначе — сколько здоровья он потеряет (после брони,
/// зачарований и неуязвимости); None — удар не пройдёт.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Blocked,
    Taken(f32),
    Missed,
}

/// Урон после брони и зачарований защиты по опубликованному состоянию.
pub fn final_damage(view: &CombatView, damage: f32, source: Source) -> f32 {
    final_damage_breached(view, damage, source, 1.0)
}

/// Тот же расчёт с пробиванием: сопротивление применяется после брони,
/// как у получателя удара. Вики, Breach: «15 percentage points» [проверить].
fn final_damage_breached(view: &CombatView, damage: f32, source: Source, breach: f32) -> f32 {
    let points = view.protection + if source.is_projectile() { view.projectile_protection } else { 0 };

    crate::survival::after_resistance(mitigated_damage(damage, view.armor, view.toughness, points, source, breach), view.resistance, source)
}

/// Предсказание исхода удара `damage` из точки `from` по жертве, которая
/// стоит в `at` и смотрит по `yaw`, на такте `now`.
pub fn outcome(view: &CombatView, damage: f32, source: Source, now: u64, victim: (f64, f64, f32), from: (f64, f64)) -> Outcome {
    outcome_breached(view, damage, source, now, victim, from, 1.0)
}

/// Предсказание должно учитывать те же чары, что и фактический удар.
pub fn outcome_breached(view: &CombatView, damage: f32, source: Source, now: u64, victim: (f64, f64, f32), from: (f64, f64), breach: f32) -> Outcome {
    if view.dead || damage <= 0.0 {
        return Outcome::Missed;
    }

    let up = view.blocking_since.is_some_and(|since| now >= since + u64::from(SHIELD_DELAY)) && now >= view.shield_disabled_until;

    if up && shield_blocks(source) && shield_covers(victim.2, (victim.0, victim.1), from) {
        return Outcome::Blocked;
    }

    let taken = if breach == 1.0 { final_damage(view, damage, source) } else { final_damage_breached(view, damage, source, breach) };

    if view.invulnerable > 10 {
        if taken <= view.last_hurt {
            return Outcome::Missed;
        }

        return Outcome::Taken(taken - view.last_hurt);
    }

    Outcome::Taken(taken)
}

/// Удар по игроку, который нападающий отдаёт жертве через журнал мира
/// (`WorldEvent::PlayerStrike`): щит, броню, отдачу и износ считает
/// подключение жертвы — у него её инвентарь и состояние.
#[derive(Clone, Debug)]
pub struct PlayerStrike {
    pub victim: i32,
    pub attacker: i32,
    pub attacker_uuid: [u8; 16],
    pub attacker_name: String,
    /// Где стоял нападающий и куда смотрел (градусы).
    pub from: (f64, f64, f64),
    pub yaw: f32,
    /// Урон до щита и брони.
    pub damage: f32,
    pub source: Source,
    /// Сверх обычной отдачи 0,4: зачарование «отдача», бег, `attack_knockback`.
    pub extra_knockback: f32,
    /// Чем ударили (для сообщения о смерти) и во что: топор отключает щит.
    pub weapon: Option<Stack>,
    pub disables_shield_ticks: u32,
    /// «Пробивание» булавы: доля защиты брони, которая остаётся (1 — вся).
    pub breach_scale: f32,
    /// Зачарование «заговорённое лезвие»: столько тактов горит жертва.
    pub fire_ticks: i32,
    /// Стрела «пробивания» минует щит (вики, Piercing: «ignores shields»)
    /// и не застревает в теле.
    pub ignores_shield: bool,
    /// Слоты брони, чьи шипы сработали при этом ударе (бросок делает нападающий).
    pub thorns_slots: Vec<i32>,
    /// Эффекты наконечника — только после успешного попадания.
    pub effects: Vec<crate::survival::Effect>,
}

/// Свойства предмета по номеру.
pub fn info(item: i32) -> Option<&'static ItemInfo> {
    static BY_NUMBER: OnceLock<HashMap<i32, &'static ItemInfo>> = OnceLock::new();

    BY_NUMBER.get_or_init(|| ITEMS.iter().map(|info| (info.item, info)).collect()).get(&item).copied()
}

/// Наибольший износ стопки (0 — не изнашивается).
pub fn max_damage(stack: &Stack) -> i32 {
    let data = stack.extras();
    if data.unbreakable { return 0; }
    configured_durability(stack.item, &data)
}

/// Прочность для починки: неизнашиваемость не должна запрещать старые рецепты.
pub(crate) fn max_durability(stack: &Stack) -> i32 {
    configured_durability(stack.item, &stack.extras())
}

fn configured_durability(item: i32, data: &crate::stackdata::Data) -> i32 {
    if let Some(max) = data.plugin_meta.as_ref().and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok()).and_then(|value| value.get("maxDamage").and_then(serde_json::Value::as_i64)).filter(|value| (1..=i32::MAX as i64).contains(value)) { return max as i32; }
    info(item).map_or(0, |info| info.max_damage)
}

// ---------------------------------------------------------------------------
// Экипировка и атрибуты
// ---------------------------------------------------------------------------

/// Места экипировки игрока в инвентаре: шлем, нагрудник, штаны, ботинки.
pub const ARMOR_SLOTS: [(i32, u8); 4] = [(5, 7), (6, 6), (7, 5), (8, 4)];

/// Подходит ли группа мест модификатора месту экипировки.
fn group_matches(group: u8, place: u8) -> bool {
    match group {
        0 => true,
        3 => place == 1 || place == 2,
        8 => (4..=7).contains(&place),
        _ => group == place,
    }
}

/// Модификаторы, которые стопка даёт на месте `place` (1 — основная рука,
/// 2 — вторая, 4–7 — обувь…шлем): свои из довесков или, если их нет, по
/// умолчанию из таблицы. Предмет с местом надевания даёт своё, только надетым
/// туда (нагрудник в руке ничего не даёт).
pub fn granted(stack: &Stack, place: u8) -> Vec<(i32, Modifier)> {
    let data = stack.extras();

    let mut modifiers: Vec<_> = if let Some(custom) = &data.attributes {
        custom.iter().filter(|m| group_matches(m.slot, place)).map(|m| (m.attribute, m.modifier())).collect()
    } else {
        info(stack.item).into_iter().flat_map(|info| info.mods.iter())
            .filter(|m| group_matches(m.slot, place))
            .map(|m| (m.attribute, Modifier { id: m.id.to_string(), amount: m.amount, operation: Operation::from_number(i32::from(m.operation)).unwrap_or(Operation::AddValue) }))
            .collect()
    };

    // Вики, Respiration/Aqua Affinity/Depth Strider/Swift Sneak/Efficiency:
    // «oxygen bonus», «400%», «⅓», «15%», «(Level^2)+1».
    // Чары независимы от собственного attribute_modifiers компонента.
    // [проверить]: точные ID/порядок модификаторов в пакетах vanilla.
    // [Bedrock: потом] отображение новых атрибутов в Bedrock-пакетах.
    // Имя модификатора у оригинала — `enchantment.<чары>/<место>` (чёрный ящик,
    // itemaudit-enchantment-attributes: respiration/head, depth_strider/feet,
    // efficiency/mainhand, swift_sneak/legs, fire_protection/chest).
    let place_name = match place {
        1 => "mainhand",
        2 => "offhand",
        4 => "feet",
        5 => "legs",
        6 => "chest",
        _ => "head",
    };

    for (name, slot, attribute, per_level, operation) in [
        ("respiration", 7, crate::attributes::OXYGEN_BONUS, 1.0, Operation::AddValue),
        ("aqua_affinity", 7, crate::attributes::SUBMERGED_MINING_SPEED, 4.0, Operation::AddMultipliedTotal),
        ("depth_strider", 4, crate::attributes::WATER_MOVEMENT_EFFICIENCY, 1.0 / 3.0, Operation::AddValue),
        ("swift_sneak", 5, crate::attributes::SNEAKING_SPEED, 0.15, Operation::AddValue),
    ] {
        let level = data.level(name);
        if place == slot && level > 0 {
            modifiers.push((attribute, Modifier::new(&format!("minecraft:enchantment.{name}/{place_name}"), f64::from(level) * per_level, operation)));
        }
    }
    let efficiency = data.level("efficiency");
    // Вики, Sweeping Edge: «level / (level + 1)»; sword-sweep-neighbours:
    // модификатор sweeping_edge/mainhand = 0,75, в том числе после износа.
    let sweeping = data.level("sweeping_edge");
    if place == 1 && sweeping > 0 {
        modifiers.push((crate::attributes::SWEEPING_DAMAGE_RATIO, Modifier::new("minecraft:enchantment.sweeping_edge/mainhand", f64::from(sweeping as f32 / (sweeping as f32 + 1.0)), Operation::AddValue)));
    }
    if place == 1 && efficiency > 0 {
        modifiers.push((crate::attributes::MINING_EFFICIENCY, Modifier::new("minecraft:enchantment.efficiency/mainhand", f64::from(efficiency).powi(2) + 1.0, Operation::AddValue)));
    }
    // Вики, Fire Protection/Blast Protection: «15 × level»,
    // «stacking with multiple armor pieces». По одному ID на слот,
    // иначе нагрудник затёр бы бонус ботинок [проверить: ID пакета].
    if (4..=7).contains(&place) {
        for (name, attribute, amount, operation) in [
            ("fire_protection", crate::attributes::BURNING_TIME, -0.15, Operation::AddMultipliedBase),
            ("blast_protection", crate::attributes::EXPLOSION_KNOCKBACK_RESISTANCE, 0.15, Operation::AddValue),
        ] {
            let level = data.level(name);
            if level > 0 {
                // Сумма считается во float, как у оригинала (−0,15·4 = −0,6000000238…).
                modifiers.push((attribute, Modifier::new(&format!("minecraft:enchantment.{name}/{place_name}"), f64::from(amount as f32 * level as f32), operation)));
            }
        }
    }
    modifiers
}

/// Переставляет модификаторы от предметов: снимает прежние (`applied`) и
/// ставит те, что дают рука, вторая рука и броня сейчас.
pub fn refresh_equipment(attributes: &mut Attributes, applied: &mut Vec<(i32, String)>, inventory: &Inventory) {
    for (attribute, id) in applied.drain(..) {
        attributes.remove_modifier(attribute, &id);
    }

    // Порядок — как у оригинала обходятся места экипировки: рука, вторая
    // рука, ботинки, штаны, нагрудник, шлем (чёрный ящик, pvp-armor: части
    // брони идут в пакете ботинки, штаны, нагрудник, шлем).
    let mut places: Vec<(Stack, u8)> = Vec::new();

    if let Some(stack) = inventory.held() {
        places.push((stack, 1));
    }

    if let Some(stack) = inventory.slot(inventory::OFFHAND) {
        places.push((stack, 2));
    }

    for (slot, place) in ARMOR_SLOTS.iter().rev() {
        if let Some(stack) = inventory.slot(*slot) {
            places.push((stack, *place));
        }
    }

    for (stack, place) in places {
        for (attribute, modifier) in granted(&stack, place) {
            applied.push((attribute, modifier.id.clone()));
            attributes.set_modifier(attribute, modifier);
        }
    }
}

// ---------------------------------------------------------------------------
// Удар в ближнем бою
// ---------------------------------------------------------------------------

/// Заряд руки от 0 до 1: `(такты + 0,5) / (20 / скорость удара)`. Вики,
/// Melee attack: «attack strength»; чёрный ящик (cow-hit): такты считаются
/// от прошлого удара.
pub fn charge(ticks_since: u32, attack_speed: f64) -> f32 {
    if attack_speed <= 0.0 {
        return 1.0;
    }

    let period = (20.0 / attack_speed) as f32;

    ((ticks_since as f32 + 0.5) / period).clamp(0.0, 1.0)
}

/// Доля основного урона при заряде: `0,2 + заряд²·0,8`.
pub fn base_scale(charge: f32) -> f32 {
    0.2 + charge * charge * 0.8
}

/// Порог полного заряда: только выше него работают крит, взмах и рывок
/// (вики, Melee attack).
pub const FULL_CHARGE: f32 = 0.9;

/// Прибавка урона от зачарований оружия по цели (вики, Sharpness, Smite,
/// Bane of Arthropods): острота 0,5 + 0,5·уровень; небесная кара и бич
/// членистоногих 2,5 за уровень против своих целей.
pub fn enchant_bonus(data: &Data, undead: bool, arthropod: bool) -> f32 {
    let mut bonus = 0.0;
    let sharp = data.level("sharpness");

    if sharp > 0 {
        bonus += 0.5 + 0.5 * sharp as f32;
    }

    if undead {
        bonus += 2.5 * data.level("smite") as f32;
    }

    if arthropod {
        bonus += 2.5 * data.level("bane_of_arthropods") as f32;
    }

    bonus
}

/// Нежить и членистоногие по имени вида (вики, Smite, Bane of Arthropods):
/// зачарования оружия бьют по ним сильнее. Виды, которых здесь нет, — ни то
/// ни другое; при добавлении враждебных мобов список пополняется.
pub fn mob_traits(kind: &str) -> (bool, bool) {
    let undead = matches!(kind, "zombie" | "skeleton" | "husk" | "stray" | "drowned" | "zombie_villager" | "zombified_piglin" | "wither_skeleton" | "phantom" | "wither" | "skeleton_horse" | "zombie_horse" | "bogged" | "parched" | "zoglin" | "camel_husk" | "zombie_nautilus");
    let arthropod = matches!(kind, "spider" | "cave_spider" | "silverfish" | "endermite" | "bee");

    (undead, arthropod)
}

/// Вики, Impaling (Java): «both ... projectile and ... melee»; drowned не
/// aquatic, игрок под дождём также не получает прибавку [проверить].
pub fn impaling_bonus(data: &Data, kind: &str) -> f32 {
    if matches!(kind, "axolotl" | "dolphin" | "guardian" | "elder_guardian" | "nautilus" | "zombie_nautilus" | "squid" | "glow_squid" | "tadpole" | "turtle" | "cod" | "salmon" | "pufferfish" | "tropical_fish") {
        2.5 * data.level("impaling") as f32
    } else { 0.0 }
}

/// Вики, Shield: магия, падение, огонь окружения, молния и шипы обходят щит.
pub fn shield_blocks(source: Source) -> bool {
    matches!(source, Source::PlayerAttack | Source::MaceSmash | Source::MobAttack | Source::MobAttackNoAggro
        | Source::Arrow | Source::Trident | Source::Thrown | Source::Fireball | Source::LargeFireball | Source::WitherSkull
        | Source::WindCharge | Source::Fireworks | Source::Explosion | Source::PlayerExplosion | Source::Thorns)
}

/// Сила отдачи от удара сверх обычной (0,4): атрибут `attack_knockback`,
/// зачарование «отдача» (1 за уровень) и бег (1). Вики, Knockback.
pub fn extra_knockback(attack_knockback: f64, data: &Data, sprint_bonus: bool) -> f32 {
    attack_knockback as f32 + data.level("knockback") as f32 + if sprint_bonus { 1.0 } else { 0.0 }
}

/// Добавка от «отбрасывания» стрелы до трения: 0,6 за уровень по горизонтали
/// и 0,1 вверх (вики, Punch; чёрный ящик pvp-bow-punch). PlayerStrike кодирует
/// уровень как 1,2L. Сопротивление уменьшает только горизонтальную добавку.
pub fn punch_impulse(extra_knockback: f32, dir: (f64, f64), resistance: f64) -> (f64, f64, f64) {
    if extra_knockback <= 0.0 { return (0.0, 0.0, 0.0); }
    let push = f64::from(extra_knockback) * 0.5 * (1.0 - resistance).max(0.0);
    (dir.0 * push, 0.1, dir.1 * push)
}

/// Итог удара по цели.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    /// Урон до брони.
    pub damage: f32,
    pub crit: bool,
    /// Рука заряжена выше порога.
    pub full: bool,
    pub charge: f32,
}

/// Считает урон удара: основа умножается на долю заряда, критический удар
/// (только при полном заряде и `crit_ok`) умножает основу ещё на 1,5, а
/// прибавка от зачарований масштабируется зарядом (вики, Melee attack).
/// `bonus` — прибавка к основе до крита: сокрушающий удар булавы (вики,
/// Mace: «both the mace's base damage and its smash attack damage are
/// increased by 50%»; чёрный ящик, pvp-mace: (6 + 4·1,5925)·1,5 = 18,556).
pub fn strike(attack_damage: f64, bonus: f32, enchant: f32, charge: f32, crit_ok: bool) -> Strike {
    let full = charge > FULL_CHARGE;
    let crit = full && crit_ok;
    let mut base = attack_damage as f32 * base_scale(charge) + bonus;

    if crit {
        base *= 1.5;
    }

    Strike { damage: base + enchant * charge, crit, full, charge }
}

// ---------------------------------------------------------------------------
// Булава
// ---------------------------------------------------------------------------

/// С какого пролёта удар булавой сокрушающий (вики, Mace: «falling 1.5
/// blocks or more»).
pub const SMASH_FALL: f32 = 1.5;

/// Сокрушает ли удар: булава в руке, игрок в пролёте не меньше 1,5 блока и
/// не на земле (вики, Mace). [проверить: полёт на крыльях не считается]
pub fn smash_ready(mace: bool, fall: f32, on_ground: bool) -> bool {
    mace && fall >= SMASH_FALL && !on_ground
}

/// Прибавка сокрушающего удара за пролёт `fall` блоков (вики, Mace): 4 за
/// каждый из первых трёх блоков, 2 за каждый из следующих пяти, дальше по 1;
/// доли блока считаются (чёрный ящик, pvp-mace: пролёт 1,5925 дал 6,37).
/// «Плотность» прибавляет 0,5 за уровень на каждый блок пролёта
/// (вики, Density) [проверить].
pub fn smash_bonus(fall: f32, density: i32) -> f32 {
    let first = fall.min(3.0);
    let second = (fall - 3.0).clamp(0.0, 5.0);
    let rest = (fall - 8.0).max(0.0);

    4.0 * first + 2.0 * second + rest + 0.5 * density as f32 * fall
}

/// Звук сокрушающего удара: по стоящей на земле жертве — «о землю», при
/// пролёте больше пяти блоков — тяжёлый; по жертве в воздухе — «в воздухе»
/// (вики, Mace: «fall of greater than five blocks»; чёрный ящик, pvp-mace).
/// [проверить: тяжёлый и воздушный]
pub fn smash_sound(fall: f32, victim_on_ground: bool) -> &'static str {
    if !victim_on_ground {
        "minecraft:item.mace.smash_air"
    } else if fall > 5.0 {
        "minecraft:item.mace.smash_ground_heavy"
    } else {
        "minecraft:item.mace.smash_ground"
    }
}

/// Сколько частиц просит событие мира 2013 «сокрушающий удар» под жертвой
/// (чёрный ящик, pvp-mace и pvp-mace-heavy: 750 при пролёте 1,6 и 4,8).
pub const SMASH_PARTICLES: i32 = 750;

/// Кодирование пробивания для PlayerStrike: 1 − 0,15L; при расчёте брони
/// вычитается 1 − scale процентных пунктов защиты (вики, Breach).
pub fn breach_scale(level: i32) -> f32 {
    (1.0 - 0.15 * level as f32).max(0.0)
}

// ---------------------------------------------------------------------------
// Броня
// ---------------------------------------------------------------------------

/// Вики, Damage type tag (Java Edition), bypasses_armor: 19 видов.
/// Огонь блока, костёр и лава бронёй смягчаются; горение — нет.
/// [проверить] сверка с 26.1.2 без выгрузки тегов оригинала.
pub fn bypasses_armor(source: Source) -> bool {
    matches!(source, Source::Cramming | Source::DragonBreath | Source::Drown
        | Source::EnderPearl | Source::Fall | Source::FlyIntoWall | Source::Freeze
        | Source::Generic | Source::GenericKill | Source::InWall | Source::IndirectMagic
        | Source::Magic | Source::OnFire | Source::OutOfWorld | Source::OutsideBorder
        | Source::SonicBoom | Source::Stalagmite | Source::Starve | Source::Wither)
}

/// Вики, Protection: «falling or magical damage» уменьшаются чарами,
/// хотя саму броню обходят. Голод, пустота, /kill и звуковой удар
/// хранителя обходят оба слоя (тег bypasses_enchantments).
pub fn bypasses_enchantments(source: Source) -> bool {
    matches!(source, Source::Starve | Source::OutOfWorld | Source::GenericKill | Source::SonicBoom)
}

/// Общий путь для PvP, окружения, зелий и /damage. Сопротивление и
/// поглощение применяет Vitals::hurt после этого расчёта [проверить].
pub fn mitigated_damage(damage: f32, armor: f32, toughness: f32, points: i32, source: Source, breach: f32) -> f32 {
    let damage = if bypasses_armor(source) { damage } else if breach == 1.0 { after_armor(damage, armor, toughness) } else { after_armor_breached(damage, armor, toughness, breach) };
    if bypasses_enchantments(source) { damage } else { after_protection(damage, points) }
}

/// Урон после брони и твёрдости (вики, Armor): `урон · (1 − min(20,
/// max(броня/5, броня − урон/(2 + твёрдость/4)))/25)`.
pub fn after_armor(damage: f32, armor: f32, toughness: f32) -> f32 {
    after_armor_breached(damage, armor, toughness, 1.0)
}

/// Вики, Breach: «15 percentage points lower per level». Вычитаются
/// процентные пункты, а не доля защиты; без брони урон не растёт [проверить].
pub fn after_armor_breached(damage: f32, armor: f32, toughness: f32, scale: f32) -> f32 {
    let defense = (armor / 5.0).max(armor - damage / (2.0 + toughness / 4.0)).clamp(0.0, 20.0) / 25.0;

    damage * (1.0 - (defense - (1.0 - scale)).max(0.0))
}

/// Вики, Thorns: «1 ... 5 ... not restricted to integer values»;
/// с 1.21 уровни >10 также не меняют величину урона [проверить: RNG parity].
pub fn thorns_damage(_level: i32, roll: f32) -> f32 {
    1.0 + 4.0 * roll
}

/// Вики, Bane of Arthropods: Slowness IV, 1–(1+0.5L) секунд.
/// Накладывается после принятого удара, только на членистоногих.
pub fn bane_effect(data: &Data, kind: &str, roll: f32) -> Option<crate::survival::Effect> {
    let level = data.level("bane_of_arthropods");
    (level > 0 && mob_traits(kind).1).then_some(crate::survival::Effect {
        kind: crate::survival::EffectKind::Slowness, amplifier: 3,
        ticks_left: ((1.0 + 0.5 * level as f32 * roll) * 20.0) as i32,
        ambient: false, visible: true, show_icon: true,
    })
}

/// Очки защиты зачарований (EPF) одной детали от этого урона (вики,
/// Protection и др.): защита 1 за уровень, кроме bypasses_enchantments;
/// огнезащита и взрывоустойчивость и защита от снарядов — по 2 за уровень
/// от своего; лёгкость падения — 3 за уровень от падения.
pub fn protection_points(data: &Data, source: Source) -> i32 {
    let mut points = 0;

    if !bypasses_enchantments(source) {
        points += data.level("protection");
    }

    match source {
        Source::InFire | Source::Campfire | Source::Lava | Source::OnFire | Source::HotFloor => points += 2 * data.level("fire_protection"),
        // Ракета — тоже взрыв (тег is_explosion; вики, Blast Protection) [проверить].
        Source::Explosion | Source::PlayerExplosion | Source::BadRespawnPoint | Source::Fireworks => points += 2 * data.level("blast_protection"),
        Source::Arrow | Source::Trident | Source::MobProjectile | Source::Thrown | Source::WindCharge | Source::LargeFireball | Source::WitherSkull | Source::Spit => points += 2 * data.level("projectile_protection"),
        // Огненный шар — огонь и снаряд сразу (теги is_fire, is_projectile).
        Source::Fireball | Source::UnattributedFireball => points += 2 * data.level("fire_protection") + 2 * data.level("projectile_protection"),
        Source::Fall | Source::EnderPearl | Source::Stalagmite => points += 3 * data.level("feather_falling"),
        _ => {}
    }

    points
}

/// Урон после зачарований защиты: сумма очков зажимается в 20, каждое очко
/// снимает 4 % (вики, Armor: «Enchantments»).
pub fn after_protection(damage: f32, points: i32) -> f32 {
    damage * (1.0 - points.clamp(0, 20) as f32 / 25.0)
}

/// Снижение отдачи взрыва и прочего здесь не считается.
///
/// Прочность, которую теряет каждая надетая деталь от удара `damage`: 1 за
/// каждые 4 единицы, не меньше 1 (вики, Armor: «Durability»).
pub fn armor_wear(damage: f32) -> i32 {
    ((damage / 4.0) as i32).max(1)
}

/// Износ предмета с учётом «прочности»: каждая единица урона проходит с
/// шансом. Инструмент и оружие — 1/(уровень+1); броня — 0,6 + 0,4/(уровень+1)
/// (вики, Unbreaking). `roll` даёт случайные числа [0; 1).
pub fn wear(stack: &Stack, amount: i32, armor: bool, mut roll: impl FnMut() -> f32) -> i32 {
    let level = stack.extras().level("unbreaking");

    if level <= 0 {
        return amount;
    }

    let chance = if armor { 0.6 + 0.4 / (level as f32 + 1.0) } else { 1.0 / (level as f32 + 1.0) };

    (0..amount).filter(|_| roll() < chance).count() as i32
}

/// Сколько прочности у стопки осталось; `None` — не изнашивается.
pub fn durability_left(stack: &Stack) -> Option<i32> {
    let max = max_damage(stack);

    (max > 0).then(|| max - stack.extras().damage)
}

/// Добавляет износ. None — предмет сломался (износ дошёл до наибольшего).
pub fn add_damage(stack: Stack, amount: i32) -> Option<Stack> {
    let max = max_damage(&stack);

    if max <= 0 || amount <= 0 {
        return Some(stack);
    }

    let mut data = stack.extras();

    data.damage += amount;

    if data.damage >= max {
        return None;
    }

    Some(stack.with_extras(data))
}

/// Один шаг починки от шарика, а не от /xp (вики, Mending: «two durability
/// per point of experience»). Выбирается только повреждённая вещь в руке,
/// второй руке или броне; целые вещи и рюкзак не участвуют [проверить].
/// Возвращает изменённый слот и не потраченный на починку опыт.
pub fn mend(inventory: &mut Inventory, points: i32, roll: f32) -> (Option<i32>, i32) {
    if points <= 0 {
        return (None, 0);
    }

    let candidates: Vec<(i32, Stack)> = [inventory.selected_slot(), inventory::OFFHAND, 5, 6, 7, 8]
        .into_iter()
        .filter_map(|slot| inventory.slot(slot).map(|stack| (slot, stack)))
        .filter(|(_, stack)| max_damage(stack) > 0 && stack.extras().damage > 0 && stack.extras().level("mending") > 0)
        .collect();

    if candidates.is_empty() {
        return (None, points);
    }

    let index = ((roll.clamp(0.0, 1.0) * candidates.len() as f32) as usize).min(candidates.len() - 1);
    let (slot, stack) = candidates[index];
    let mut data = stack.extras();
    let repaired = data.damage.min(points.saturating_mul(2));
    data.damage -= repaired;
    inventory.set(slot, Some(stack.with_extras(data)));

    (Some(slot), points - repaired / 2)
}

/// Остаток того же шарика сначала чинит остальные повреждённые вещи.
/// Вики, Mending: «remaining experience ... used to repair another item».
/// Один пункт износа чинится без целого XP; цикл всё равно завершается,
/// поскольку выбранная вещь либо становится целой, либо расходует весь XP.
pub fn mend_all(inventory: &mut Inventory, mut points: i32, mut roll: impl FnMut() -> f32) -> (Vec<i32>, i32) {
    let mut changed = Vec::new();
    while points > 0 {
        let (slot, remaining) = mend(inventory, points, roll());
        let Some(slot) = slot else { break; };
        changed.push(slot);
        points = remaining;
    }
    (changed, points.max(0))
}

/// Вики, Curse of Vanishing: «does not drop when the player dies».
/// keepInventory сохраняет и проклятые вещи; ручной выброс не затрагивается.
pub fn death_drops(inventory: &mut Inventory, keep: bool) -> Vec<Stack> {
    if keep {
        return Vec::new();
    }

    let mut drops = Vec::new();
    for slot in 0..inventory::SLOTS as i32 {
        if let Some(stack) = inventory.slot(slot) {
            inventory.set(slot, None);
            if stack.extras().level("vanishing_curse") == 0 {
                drops.push(stack);
            }
        }
    }
    drops.extend(inventory.take_retained().into_iter().filter(|stack|stack.extras().level("vanishing_curse")==0));
    drops
}

// ---------------------------------------------------------------------------
// Отдача
// ---------------------------------------------------------------------------

/// Скорость цели после отдачи силой `strength` от ударившего в направлении
/// `(dx, dz)` — от ударившего к цели, единичное. Вики, Knockback: скорость
/// цели вдвое гасится, вычитается направление · сила; вертикаль на земле —
/// вдвое гасится и растёт на силу, но не выше 0,4. Стойкость (0…1) ослабляет
/// силу: `сила · (1 − стойкость)`.
pub fn knockback_velocity(velocity: (f64, f64, f64), dir: (f64, f64), strength: f64, resistance: f64, on_ground: bool) -> (f64, f64, f64) {
    let strength = strength * (1.0 - resistance.clamp(0.0, 1.0));

    if strength <= 0.0 {
        return velocity;
    }

    let vy = if on_ground { (velocity.1 / 2.0 + strength).min(0.4) } else { velocity.1 };

    (velocity.0 / 2.0 + dir.0 * strength, vy, velocity.2 / 2.0 + dir.1 * strength)
}

// ---------------------------------------------------------------------------
// Щит
// ---------------------------------------------------------------------------

/// Тактов до того, как поднятый щит начинает держать удар: 0,25 с
/// (компонент `blocks_attacks`, `block_delay_seconds`).
pub const SHIELD_DELAY: u32 = 5;

/// Угол защиты щита в каждую сторону от взгляда, градусы (вики, Shield:
/// полукруг перед игроком).
pub const SHIELD_HALF_ANGLE: f64 = 90.0;

/// Держит ли щит удар из точки `from`: угол между взглядом и направлением
/// на источник не больше 90° (вики, Shield). `yaw` — взгляд в градусах.
pub fn shield_covers(yaw: f32, (x, z): (f64, f64), from: (f64, f64)) -> bool {
    let (dx, dz) = (from.0 - x, from.1 - z);
    let length = dx.hypot(dz);

    if length < 1.0e-9 {
        return true;
    }

    let rad = f64::from(yaw).to_radians();
    let look = (-rad.sin(), rad.cos());
    let cos = (look.0 * dx + look.1 * dz) / length;

    cos >= SHIELD_HALF_ANGLE.to_radians().cos() - 1.0e-9
}

/// Износ щита от заблокированного удара: при уроне не меньше порога 3 —
/// `1 + 1·урон` (округление вниз), иначе ничего (компонент `blocks_attacks`,
/// `item_damage`: base 1, factor 1, threshold 3).
pub fn shield_wear(blocked: f32) -> i32 {
    if blocked < 3.0 { 0 } else { (1.0 + blocked) as i32 }
}

// ---------------------------------------------------------------------------
// Лук
// ---------------------------------------------------------------------------

/// Сила натяжения по времени в тактах (вики, Bow): `f = t/20`,
/// `f = (f² + 2f)/3`, не больше 1; меньше 0,1 — выстрела нет.
pub fn bow_power(ticks: u32) -> f32 {
    let t = ticks as f32 / 20.0;
    let power = (t * t + t * 2.0) / 3.0;

    power.min(1.0)
}

/// Скорость стрелы: 3 блока за такт при полном натяжении (вики, Arrow).
pub fn arrow_speed(power: f32) -> f64 {
    f64::from(power) * 3.0
}

/// Урон стрелы при попадании: `ceil(скорость · (2 + сила·0,5 + 0,5))`, где
/// сила — уровень «силы» (вики, Arrow, Power: +25 % за уровень + 1 к уровню).
/// Критическая полностью натянутая стрела добавляет случайные 0…(урон/2 + 1)
/// (`crit_roll` в [0; 1)). Возвращает дробный урон: округление вверх — у
/// вызывающего.
pub fn arrow_damage(speed: f64, power_level: i32, crit: bool, crit_roll: f32) -> f32 {
    let mut base = 2.0 + if power_level > 0 { 0.5 * power_level as f64 + 0.5 } else { 0.0 };

    base *= speed;

    let mut damage = base.ceil() as f32;

    if crit {
        damage += (crit_roll * ((damage / 2.0).floor() + 2.0)).floor();
    }

    damage
}

// ---------------------------------------------------------------------------
// Арбалет
// ---------------------------------------------------------------------------

/// Скорость стрелы из арбалета: 3,15 блока за такт (вики, Arrow: «3.15 for
/// crossbows»).
pub const CROSSBOW_SPEED: f64 = 3.15;

/// Сколько тактов арбалет заряжается: 25 (вики, Crossbow: 1,25 с), «быстрая
/// перезарядка» снимает по 5 за уровень (1 с, 0,75 с, 0,5 с).
pub fn crossbow_charge_ticks(quick_charge: i32) -> u32 {
    (25 - 5 * quick_charge.clamp(0, 5)).max(0) as u32
}

/// Звук такта зарядки арбалета: имя и громкость. Такты от начала натяжения:
/// начало — когда натянуто на 0,2 (громкость 0,5), середина — на 0,5 (0,5;
/// у «быстрой перезарядки» своё начало и нет середины), конец — на 1 (1,0).
/// Снято чёрным ящиком: crossbow-shoot (+5, +13, +25), crossbow-quick-charge
/// II (+3, +15).
pub fn crossbow_loading_sound(held: u32, quick_charge: i32) -> Option<(&'static str, f32)> {
    let total = crossbow_charge_ticks(quick_charge) as f32;

    if held == (total * 0.2).ceil() as u32 {
        return Some(match quick_charge {
            0 => ("minecraft:item.crossbow.loading_start", 0.5),
            1 => ("minecraft:item.crossbow.quick_charge_1", 0.5),
            2 => ("minecraft:item.crossbow.quick_charge_2", 0.5),
            _ => ("minecraft:item.crossbow.quick_charge_3", 0.5),
        });
    }

    if held == total as u32 {
        return Some(("minecraft:item.crossbow.loading_end", 1.0));
    }

    (quick_charge == 0 && held == (total * 0.5).ceil() as u32).then_some(("minecraft:item.crossbow.loading_middle", 0.5))
}

/// Угол стрел «залпа» вокруг средней: ±10° по горизонтали (вики, Multishot:
/// «10° apart horizontally»).
pub const MULTISHOT_ANGLE: f32 = 10.0;

/// Имя предмета по номеру без пространства имён.
pub fn item_name(item: i32) -> Option<&'static str> {
    crate::blocks::item_name(item).map(|name| name.strip_prefix("minecraft:").unwrap_or(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::{ARMOR, ARMOR_TOUGHNESS, ATTACK_DAMAGE, ATTACK_SPEED};
    use crate::stackdata;

    fn stack_of(name: &str) -> Stack {
        Stack::new(crate::blocks::item_named(name).expect("предмет есть"), 1)
    }

    #[test]
    fn unbreakable_items_keep_their_repair_limit() {
        let sword=stack_of("diamond_sword");let mut data=sword.extras();data.unbreakable=true;let sword=sword.with_extras(data);
        assert_eq!(max_damage(&sword),0);assert_eq!(max_durability(&sword),1561);
    }

    #[test]
    fn a_diamond_sword_in_hand_gives_seven_damage_and_the_wiki_speed() {
        let mut attributes = Attributes::player();
        let mut applied = Vec::new();
        let mut inventory = Inventory::new();

        inventory.set(inventory::FIRST_HOTBAR, Some(stack_of("diamond_sword")));
        refresh_equipment(&mut attributes, &mut applied, &inventory);

        assert_eq!(attributes.value(ATTACK_DAMAGE), 7.0);
        assert!((attributes.value(ATTACK_SPEED) - 1.6).abs() < 1e-6);

        // убрали меч — вернулась основа
        inventory.set(inventory::FIRST_HOTBAR, None);
        refresh_equipment(&mut attributes, &mut applied, &inventory);

        assert_eq!(attributes.value(ATTACK_DAMAGE), 1.0);
        assert_eq!(attributes.value(ATTACK_SPEED), 4.0);
    }

    #[test]
    fn a_trident_with_one_durability_left_is_known() {
        let trident = Stack::new(crate::blocks::item_named("trident").expect("трезубец есть"), 1);

        // Прочность трезубца 250 (вики, Trident).
        assert_eq!(durability_left(&trident), Some(250));
        assert_eq!(durability_left(&add_damage(trident, 249).expect("ещё цел")), Some(1));
        assert_eq!(durability_left(&Stack::new(crate::blocks::item_named("stone").expect("камень есть"), 1)), None);
    }

    #[test]
    fn the_crossbow_charges_faster_with_quick_charge() {
        assert_eq!(crossbow_charge_ticks(0), 25);
        assert_eq!(crossbow_charge_ticks(1), 20);
        assert_eq!(crossbow_charge_ticks(3), 10);
        // Звуки зарядки (чёрный ящик, crossbow-shoot, crossbow-quick-charge).
        assert_eq!(crossbow_loading_sound(5, 0), Some(("minecraft:item.crossbow.loading_start", 0.5)));
        assert_eq!(crossbow_loading_sound(13, 0), Some(("minecraft:item.crossbow.loading_middle", 0.5)));
        assert_eq!(crossbow_loading_sound(25, 0), Some(("minecraft:item.crossbow.loading_end", 1.0)));
        assert_eq!(crossbow_loading_sound(3, 2), Some(("minecraft:item.crossbow.quick_charge_2", 0.5)));
        assert_eq!(crossbow_loading_sound(15, 2), Some(("minecraft:item.crossbow.loading_end", 1.0)));
        assert_eq!(crossbow_loading_sound(8, 2), None);
        assert_eq!(crossbow_loading_sound(12, 0), None);
        // Критическая стрела арбалета: ceil(3,15 · 2) = 7 плюс случайная добавка
        // (чёрный ящик: у оригинала урон 7…11 в пяти записях).
        assert_eq!(arrow_damage(CROSSBOW_SPEED, 0, true, 0.0), 7.0);
        assert_eq!(arrow_damage(CROSSBOW_SPEED, 0, true, 0.8), 11.0);
    }

    #[test]
    fn mace_smash_matches_the_black_box() {
        // pvp-mace: пролёт 1,5925 блока, крит: (6 + 6,37)·1,5 = 18,556.
        let fall = 1.5925;
        let hit = strike(6.0, smash_bonus(fall, 0), 0.0, 1.0, true);

        assert!((hit.damage - 18.556).abs() < 0.01, "{}", hit.damage);
        assert!(hit.crit);

        // Ступени прибавки: 3 блока по 4, 5 блоков по 2, дальше по 1.
        assert_eq!(smash_bonus(3.0, 0), 12.0);
        assert_eq!(smash_bonus(8.0, 0), 22.0);
        assert_eq!(smash_bonus(10.0, 0), 24.0);
        // «Плотность» I: ещё 0,5 за блок.
        assert_eq!(smash_bonus(4.0, 1), 16.0);

        assert!(smash_ready(true, 1.5, false));
        assert!(!smash_ready(true, 1.4, false));
        assert!(!smash_ready(true, 3.0, true));
        assert!(!smash_ready(false, 3.0, false));

        assert_eq!(smash_sound(1.6, true), "minecraft:item.mace.smash_ground");
        assert_eq!(smash_sound(5.5, true), "minecraft:item.mace.smash_ground_heavy");
        assert_eq!(smash_sound(5.5, false), "minecraft:item.mace.smash_air");
    }

    #[test]
    fn breach_weakens_armor() {
        // Защита 70 % превращается в 40 %, а не в 49 %.
        let whole = after_armor(10.0, 20.0, 8.0);
        let breached = after_armor_breached(10.0, 20.0, 8.0, breach_scale(2));

        assert!((whole - 3.0).abs() < 1e-5);
        assert!((breached - 6.0).abs() < 1e-5);
        assert!((after_armor_breached(6.0, 20.0, 8.0, breach_scale(4)) - 5.16).abs() < 1e-5);
        assert_eq!(after_armor_breached(6.0, 2.0, 0.0, breach_scale(4)), 6.0);
        assert_eq!(after_armor_breached(6.0, 0.0, 0.0, breach_scale(4)), 6.0);
        assert!((breach_scale(2) - 0.7).abs() < 1e-6);
        assert_eq!(breach_scale(7), 0.0);
    }

    #[test]
    fn breach_prediction_agrees_with_victim_armor_then_resistance() {
        let view = CombatView { armor: 20.0, toughness: 8.0, protection: 4, resistance: 1, ..Default::default() };
        let damage = after_protection(after_armor_breached(6.0, 20.0, 8.0, breach_scale(4)), 4);
        let expected = crate::survival::after_resistance(damage, 1, Source::PlayerAttack);
        assert_eq!(outcome_breached(&view, 6.0, Source::PlayerAttack, 0, (0.0, 0.0, 0.0), (0.0, 2.0), breach_scale(4)), Outcome::Taken(expected));
        let mut mob = crate::mob::Mob::new(42, crate::mob::Kind::Zombie, (0.0, 0.0, 0.0));
        assert_eq!(mob.hurt_breached(6.0, Source::PlayerAttack, None, breach_scale(4)).unwrap().taken, 6.0);
    }

    fn mending_item(name: &str, damage: i32) -> Stack {
        let mut data = Data { damage, ..Default::default() };
        data.set_enchant(stackdata::enchantment_number("mending").unwrap(), 1);
        Stack::new(crate::blocks::item_named(name).unwrap(), 1).with_extras(data)
    }

    #[test]
    fn mending_selects_only_damaged_equipped_items() {
        let mut inventory = Inventory::new();
        inventory.set(inventory::FIRST_MAIN, Some(mending_item("diamond_pickaxe", 100)));
        inventory.set(5, Some(mending_item("diamond_helmet", 0)));
        inventory.set(inventory::OFFHAND, Some(mending_item("diamond_sword", 20)));
        assert_eq!(mend(&mut inventory, 7, 0.0), (Some(inventory::OFFHAND), 0));
        assert_eq!(inventory.slot(inventory::OFFHAND).unwrap().extras().damage, 6);
        assert_eq!(inventory.slot(inventory::FIRST_MAIN).unwrap().extras().damage, 100);
        assert_eq!(mend(&mut inventory, 7, 0.9), (Some(inventory::OFFHAND), 4));
        assert_eq!(mend(&mut inventory, 3, 0.0), (None, 3));
    }

    #[test]
    fn mending_uses_each_equipped_slot_and_rounds_spent_xp_down() {
        for slot in [inventory::FIRST_HOTBAR, inventory::OFFHAND, 5, 6, 7, 8] {
            let mut inventory = Inventory::new();
            inventory.set(slot, Some(mending_item("diamond_sword", 3)));
            assert_eq!(mend(&mut inventory, 7, 0.5), (Some(slot), 6));
            assert_eq!(inventory.slot(slot).unwrap().extras().damage, 0);
        }
        let mut inventory = Inventory::new();
        inventory.set(5, Some(mending_item("diamond_helmet", 10)));
        inventory.set(6, Some(mending_item("diamond_chestplate", 10)));
        assert_eq!(mend(&mut inventory, 1, 0.0).0, Some(5));
        assert_eq!(mend(&mut inventory, 1, 0.999).0, Some(6));
        assert_eq!(mend(&mut inventory, 0, 0.0), (None, 0));
    }

    #[test]
    fn mending_spends_one_orb_across_equipment_before_granting_xp() {
        let mut inventory = Inventory::new();
        inventory.set(5, Some(mending_item("diamond_helmet", 3)));
        inventory.set(6, Some(mending_item("diamond_chestplate", 10)));
        inventory.set(7, Some(mending_item("diamond_leggings", 1)));
        inventory.set(inventory::FIRST_MAIN, Some(mending_item("diamond_pickaxe", 100)));
        assert_eq!(mend_all(&mut inventory, 7, || 0.0), (vec![5, 6, 7], 1));
        for slot in [5, 6, 7] { assert_eq!(inventory.slot(slot).unwrap().extras().damage, 0); }
        assert_eq!(inventory.slot(inventory::FIRST_MAIN).unwrap().extras().damage, 100);
        assert_eq!(mend_all(&mut inventory, 7, || 0.0), (vec![], 7));
        assert_eq!(mend_all(&mut inventory, -1, || panic!("нет броска без опыта")), (vec![], 0));
    }

    #[test]
    fn mending_stops_when_orb_runs_out_and_selects_again_after_repair() {
        let mut inventory = Inventory::new();
        inventory.set(5, Some(mending_item("diamond_helmet", 2)));
        inventory.set(6, Some(mending_item("diamond_chestplate", 10)));
        inventory.set(7, Some(mending_item("diamond_leggings", 10)));
        let mut rolls = [0.0, 0.999].into_iter();
        assert_eq!(mend_all(&mut inventory, 3, || rolls.next().unwrap()), (vec![5, 7], 0));
        assert_eq!(inventory.slot(5).unwrap().extras().damage, 0);
        assert_eq!(inventory.slot(6).unwrap().extras().damage, 10);
        assert_eq!(inventory.slot(7).unwrap().extras().damage, 6);
    }

    #[test]
    fn vanishing_removes_all_carried_cursed_items_only_on_death() {
        let mut inventory = Inventory::new();
        let plain = Stack::new(crate::blocks::item_named("stone").unwrap(), 3);
        let mut data = Data::default();
        data.set_enchant(stackdata::enchantment_number("vanishing_curse").unwrap(), 1);
        let cursed = mending_item("diamond_sword", 10).with_extras(data);
        for slot in [5, inventory::FIRST_MAIN, inventory::FIRST_HOTBAR, inventory::OFFHAND] {
            inventory.set(slot, Some(cursed));
        }
        inventory.set(10, Some(plain));
        assert!(death_drops(&mut inventory, true).is_empty());
        assert_eq!(inventory.slot(5), Some(cursed));
        assert_eq!(death_drops(&mut inventory, false), vec![plain]);
        assert!(inventory.slots().iter().all(Option::is_none));
    }

    #[test]
    fn armor_gives_points_only_in_its_slot() {
        let mut attributes = Attributes::player();
        let mut applied = Vec::new();
        let mut inventory = Inventory::new();

        // нагрудник в руке ничего не даёт
        inventory.set(inventory::FIRST_HOTBAR, Some(stack_of("diamond_chestplate")));
        refresh_equipment(&mut attributes, &mut applied, &inventory);

        assert_eq!(attributes.value(ARMOR), 0.0);

        for (name, slot) in [("diamond_helmet", 5), ("diamond_chestplate", 6), ("diamond_leggings", 7), ("diamond_boots", 8)] {
            inventory.set(slot, Some(stack_of(name)));
        }

        refresh_equipment(&mut attributes, &mut applied, &inventory);

        assert_eq!(attributes.value(ARMOR), 20.0);
        assert_eq!(attributes.value(ARMOR_TOUGHNESS), 8.0);
    }

    #[test]
    fn custom_modifiers_replace_the_defaults() {
        let (name, data) = stackdata::parse_item_spec(
            "diamond_sword[attribute_modifiers=[{type:\"minecraft:attack_damage\",id:\"my:dmg\",amount:20,operation:\"add_value\",slot:\"mainhand\"}]]",
        )
        .expect("разобралось");
        let sword = stack_of(&name).with_extras(data);
        let mut attributes = Attributes::player();
        let mut applied = Vec::new();
        let mut inventory = Inventory::new();

        inventory.set(inventory::FIRST_HOTBAR, Some(sword));
        refresh_equipment(&mut attributes, &mut applied, &inventory);

        // 1 + 20; скорость осталась основой — своих модификаторов на неё нет
        assert_eq!(attributes.value(ATTACK_DAMAGE), 21.0);
        assert_eq!(attributes.value(ATTACK_SPEED), 4.0);
    }

    #[test]
    fn the_charge_follows_the_attack_speed() {
        // мечу нужно 12,5 такта: сразу после удара заряд мал, через 13 — полный
        assert!(charge(0, 1.6) < 0.05);
        assert_eq!(charge(13, 1.6), 1.0);
        // кулаку — 5 тактов
        assert_eq!(charge(5, 4.0), 1.0);
        assert!((base_scale(0.0) - 0.2).abs() < 1e-6);
        assert!((base_scale(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_full_crit_multiplies_only_the_base_damage() {
        // меч 7, острота V (+3), полный заряд, крит: 7·1,5 + 3
        let hit = strike(7.0, 0.0, 3.0, 1.0, true);

        assert!(hit.crit && hit.full);
        assert!((hit.damage - 13.5).abs() < 1e-5);

        // не полный заряд — крита нет, и прибавка масштабируется зарядом
        let weak = strike(7.0, 0.0, 3.0, 0.5, true);

        assert!(!weak.crit && !weak.full);
        assert!((weak.damage - (7.0 * 0.4 + 1.5)).abs() < 1e-5);
    }

    #[test]
    fn enchant_bonuses_follow_the_wiki() {
        let mut data = Data::default();

        data.set_enchant(stackdata::enchantment_number("sharpness").unwrap(), 5);
        data.set_enchant(stackdata::enchantment_number("smite").unwrap(), 2);

        assert_eq!(enchant_bonus(&data, false, false), 3.0);
        assert_eq!(enchant_bonus(&data, true, false), 8.0);
        assert_eq!(enchant_bonus(&Data::default(), true, true), 0.0);
    }

    #[test]
    fn armor_formula_matches_the_wiki_table() {
        // алмазная броня (20 / 8): удар 7 — снижение 20 %·… по формуле
        let reduced = after_armor(7.0, 20.0, 8.0);
        let defense = 20.0f32 - 7.0 / (2.0 + 2.0);

        assert!((reduced - 7.0 * (1.0 - defense / 25.0)).abs() < 1e-5);
        assert!(reduced < 7.0);
        // без брони урон не меняется
        assert_eq!(after_armor(7.0, 0.0, 0.0), 7.0);
        // броня 20, урон 10, твёрдости нет: защита max(4, 20 − 10/2) = 15 → 40 % урона
        assert!((after_armor(10.0, 20.0, 0.0) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn protection_enchants_cap_at_twenty_points() {
        assert!((after_protection(10.0, 4) - 8.4).abs() < 1e-5);
        assert!((after_protection(10.0, 99) - 2.0).abs() < 1e-5);

        let mut data = Data::default();

        data.set_enchant(stackdata::enchantment_number("protection").unwrap(), 4);
        data.set_enchant(stackdata::enchantment_number("fire_protection").unwrap(), 4);

        assert_eq!(protection_points(&data, Source::PlayerAttack), 4);
        assert_eq!(protection_points(&data, Source::Lava), 12);
        assert_eq!(protection_points(&data, Source::Fall), 4);
    }

    #[test]
    fn armor_bypass_still_allows_protection_and_feather_falling() {
        let mut data = Data::default();
        data.set_enchant(stackdata::enchantment_number("protection").unwrap(), 4);
        data.set_enchant(stackdata::enchantment_number("feather_falling").unwrap(), 4);
        assert_eq!(protection_points(&data, Source::Fall), 16);
        assert!((mitigated_damage(10.0, 20.0, 8.0, 16, Source::Fall, 1.0) - 3.6).abs() < 1e-5);
        for source in [Source::Magic, Source::IndirectMagic, Source::Wither, Source::OnFire, Source::FlyIntoWall] {
            assert!((mitigated_damage(10.0, 20.0, 8.0, 4, source, 1.0) - 8.4).abs() < 1e-5, "{source:?}");
        }
        for source in [Source::Starve, Source::OutOfWorld, Source::GenericKill] {
            assert_eq!(protection_points(&data, source), 0);
            assert_eq!(mitigated_damage(10.0, 20.0, 8.0, 20, source, 1.0), 10.0);
        }
        data.set_enchant(stackdata::enchantment_number("blast_protection").unwrap(), 4);
        assert_eq!(protection_points(&data, Source::PlayerExplosion), 12);
    }

    #[test]
    fn equipment_enchantments_apply_only_in_their_slots_and_are_removed() {
        use crate::attributes::{MINING_EFFICIENCY, OXYGEN_BONUS, SNEAKING_SPEED, SUBMERGED_MINING_SPEED, WATER_MOVEMENT_EFFICIENCY};
        let mut inventory = Inventory::new();
        for (slot, item, enchantment, level) in [(5, "diamond_helmet", "respiration", 3), (7, "diamond_leggings", "swift_sneak", 3), (8, "diamond_boots", "depth_strider", 3), (inventory::FIRST_HOTBAR, "diamond_pickaxe", "efficiency", 5)] {
            let mut data = Data::default();
            data.set_enchant(stackdata::enchantment_number(enchantment).unwrap(), level);
            if slot == 5 { data.set_enchant(stackdata::enchantment_number("aqua_affinity").unwrap(), 1); }
            inventory.set(slot, Some(stack_of(item).with_extras(data)));
        }
        let mut attributes = Attributes::player();
        let mut applied = Vec::new();
        refresh_equipment(&mut attributes, &mut applied, &inventory);
        for (id, value) in [(OXYGEN_BONUS, 3.0), (SUBMERGED_MINING_SPEED, 1.0), (SNEAKING_SPEED, 0.75), (WATER_MOVEMENT_EFFICIENCY, 1.0), (MINING_EFFICIENCY, 26.0)] {
            assert!((attributes.value(id) - value).abs() < 1e-6, "{id}");
        }
        let pickaxe = inventory.held();
        inventory.set(inventory::OFFHAND, pickaxe);
        for slot in [5, 7, 8, inventory::FIRST_HOTBAR] { inventory.set(slot, None); }
        refresh_equipment(&mut attributes, &mut applied, &inventory);
        for (id, value) in [(OXYGEN_BONUS, 0.0), (SUBMERGED_MINING_SPEED, 0.2), (SNEAKING_SPEED, 0.3), (WATER_MOVEMENT_EFFICIENCY, 0.0), (MINING_EFFICIENCY, 0.0)] {
            assert!((attributes.value(id) - value).abs() < 1e-6, "{id}");
        }
    }

    #[test]
    fn armor_wear_is_a_quarter_of_the_damage_and_at_least_one() {
        assert_eq!(armor_wear(1.0), 1);
        assert_eq!(armor_wear(7.9), 1);
        assert_eq!(armor_wear(8.0), 2);
        assert_eq!(armor_wear(20.0), 5);
    }

    #[test]
    fn tools_wear_out_and_break() {
        let sword = stack_of("wooden_sword");
        let max = max_damage(&sword);

        assert_eq!(max, 59);

        let worn = add_damage(sword, 58).expect("ещё цел");

        assert_eq!(worn.extras().damage, 58);
        assert_eq!(add_damage(worn, 1), None);
        // то, что не изнашивается, не ломается
        assert_eq!(add_damage(stack_of("stone"), 5), Some(stack_of("stone")));
    }

    #[test]
    fn unbreaking_lowers_the_wear() {
        let mut data = Data::default();

        data.set_enchant(stackdata::enchantment_number("unbreaking").unwrap(), 3);

        let tool = stack_of("iron_pickaxe").with_extras(data.clone());
        let boots = stack_of("iron_boots").with_extras(data);

        // всегда 0,5: инструмент (шанс 0,25) не изнашивается, броня (шанс 0,7) — тоже
        assert_eq!(wear(&tool, 4, false, || 0.5), 0);
        assert_eq!(wear(&boots, 4, true, || 0.5), 4);
        assert_eq!(wear(&boots, 4, true, || 0.9), 0);
        assert_eq!(wear(&stack_of("iron_pickaxe"), 4, false, || 0.99), 4);
    }

    #[test]
    fn knockback_halves_the_speed_and_caps_the_lift() {
        let v = knockback_velocity((0.0, 0.0, 0.0), (0.0, 1.0), 0.4, 0.0, true);

        assert_eq!(v, (0.0, 0.4, 0.4));

        let v = knockback_velocity((0.2, 0.3, 0.0), (1.0, 0.0), 0.4, 0.0, true);

        assert!((v.0 - 0.5).abs() < 1e-9 && (v.1 - 0.4).abs() < 1e-9);
        // в воздухе вертикаль не трогается
        assert_eq!(knockback_velocity((0.0, -0.5, 0.0), (0.0, 1.0), 0.4, 0.0, false).1, -0.5);
        // полная стойкость гасит всё
        assert_eq!(knockback_velocity((0.1, 0.0, 0.0), (0.0, 1.0), 0.4, 1.0, true), (0.1, 0.0, 0.0));
    }

    #[test]
    fn the_shield_covers_the_front_half() {
        // игрок в нуле смотрит на +z (yaw 0)
        assert!(shield_covers(0.0, (0.0, 0.0), (0.0, 3.0)));
        assert!(shield_covers(0.0, (0.0, 0.0), (3.0, 0.0)));
        assert!(!shield_covers(0.0, (0.0, 0.0), (0.0, -3.0)));
        // смотрит на -x (yaw 90): источник на +x — позади
        assert!(!shield_covers(90.0, (0.0, 0.0), (3.0, 0.0)));
        assert!(shield_covers(90.0, (0.0, 0.0), (-3.0, 0.0)));
    }

    #[test]
    fn the_shield_wears_only_from_hard_hits() {
        assert_eq!(shield_wear(2.9), 0);
        assert_eq!(shield_wear(3.0), 4);
        assert_eq!(shield_wear(7.0), 8);
    }

    #[test]
    fn the_bow_pulls_like_the_wiki() {
        assert!((bow_power(20) - 1.0).abs() < 1e-6);
        assert_eq!(bow_power(40), 1.0);
        // 10 тактов — треть плюс четверть
        assert!((bow_power(10) - (0.25 + 1.0) / 3.0).abs() < 1e-6);
        assert!((arrow_speed(1.0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn arrow_damage_scales_with_speed_power_and_crit() {
        // полный лук: 2·3 = 6
        assert_eq!(arrow_damage(3.0, 0, false, 0.0), 6.0);
        // сила I: (2 + 1)·3 = 9
        assert_eq!(arrow_damage(3.0, 1, false, 0.0), 9.0);
        // крит добавляет до урона/2 + 2
        assert_eq!(arrow_damage(3.0, 0, true, 0.0), 6.0);
        assert_eq!(arrow_damage(3.0, 0, true, 0.99), 6.0 + (0.99f32 * 5.0).floor());
    }

    #[test]
    fn thorns_damage_uses_wiki_range_at_all_levels() {
        // Вики, Thorns: диапазон 1–5; прежняя выборка не определяла границы.
        for level in [1, 3, 7, 11, 255] {
            assert_eq!(thorns_damage(level, 0.0), 1.0);
            assert!((thorns_damage(level, 0.999) - 4.996).abs() < 1e-3);
        }
    }

    #[test]
    fn armor_tags_distinguish_burning_from_contact_fire() {
        for source in [Source::Thorns,Source::InFire,Source::Campfire,Source::Lava,Source::HotFloor,Source::Trident,Source::Cactus] {
            assert!(!bypasses_armor(source), "{source:?}");
        }
        for source in [Source::OnFire,Source::Magic,Source::IndirectMagic,Source::Freeze,Source::Cramming,Source::DragonBreath,Source::EnderPearl,Source::Stalagmite] {
            assert!(bypasses_armor(source), "{source:?}");
        }
    }
}

/// Источник команды /damage (вики, Commands/damage: «by», «from», «at»).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageOrigin {
    pub direct: i32,
    pub cause: i32,
    pub at: Option<(f64, f64, f64)>,
    pub entity_at: Option<(f64, f64, f64)>,
    pub player: Option<(String, [u8; 16])>,
}

#[cfg(test)]
mod itemaudit2_tests {
    use super::*;

    #[test]
    fn punch_impulse_scales_with_level_and_horizontal_resistance() {
        assert_eq!(punch_impulse(0.0, (0.0, 1.0), 0.0), (0.0, 0.0, 0.0));
        for level in [1, 2] {
            for resistance in [0.0, 0.5, 1.0] {
                let impulse = punch_impulse(level as f32 * 1.2, (0.0, 1.0), resistance);
                assert_eq!(impulse.0, 0.0);
                assert_eq!(impulse.1, 0.1);
                assert!((impulse.2 - 0.6 * f64::from(level) * (1.0 - resistance)).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn impaling_covers_java_aquatic_targets_but_not_drowned_or_players() {
        let mut data = Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("impaling").unwrap(), 5);
        for name in ["axolotl", "dolphin", "guardian", "elder_guardian", "nautilus", "zombie_nautilus", "squid", "glow_squid", "tadpole", "turtle", "cod", "salmon", "pufferfish", "tropical_fish"] {
            assert_eq!(impaling_bonus(&data, name), 12.5, "{name}");
        }
        for name in ["drowned", "player", "zombie", "cow"] { assert_eq!(impaling_bonus(&data, name), 0.0); }
        assert_eq!(impaling_bonus(&Data::default(), "guardian"), 0.0);
    }

    #[test]
    fn shield_damage_types_exclude_magic_lightning_and_environment() {
        for source in [Source::Magic, Source::IndirectMagic, Source::Lightning, Source::Fall, Source::Lava, Source::OnFire, Source::Wither] {
            assert!(!shield_blocks(source), "{source:?}");
        }
        for source in [Source::Arrow, Source::Trident, Source::PlayerAttack, Source::MaceSmash, Source::Fireball, Source::Explosion, Source::Thorns] {
            assert!(shield_blocks(source));
        }
    }
    #[test]
    fn enchantment_modifiers_are_named_by_enchantment_and_slot_like_the_original() {
        // Чёрный ящик, itemaudit-enchantment-attributes: «…fire_protection/chest», «…depth_strider/feet».
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("fire_protection").unwrap(), 4);
        data.set_enchant(crate::stackdata::enchantment_number("depth_strider").unwrap(), 3);
        let chest = Stack::new(crate::blocks::item_named("diamond_chestplate").unwrap(), 1).with_extras(data.clone());
        let boots = Stack::new(crate::blocks::item_named("diamond_boots").unwrap(), 1).with_extras(data);
        let ids = |stack: &Stack, place: u8| granted(stack, place).into_iter().map(|(_, m)| m.id).collect::<Vec<_>>();

        assert!(ids(&chest, 6).iter().any(|id| id == "minecraft:enchantment.fire_protection/chest"));
        assert!(ids(&boots, 4).iter().any(|id| id == "minecraft:enchantment.depth_strider/feet"));
        assert!(ids(&boots, 4).iter().any(|id| id == "minecraft:enchantment.fire_protection/feet"));
    }

}

#[cfg(test)]
#[path = "combat_audit_tests.rs"]
mod itemaudit4_tests;
