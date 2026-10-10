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
    let damage = crate::survival::after_resistance(damage, view.resistance, source);

    if bypasses_armor(source) {
        return damage;
    }

    let points = view.protection + if source.is_projectile() { view.projectile_protection } else { 0 };

    after_protection(after_armor(damage, view.armor, view.toughness), points)
}

/// Предсказание исхода удара `damage` из точки `from` по жертве, которая
/// стоит в `at` и смотрит по `yaw`, на такте `now`.
pub fn outcome(view: &CombatView, damage: f32, source: Source, now: u64, victim: (f64, f64, f32), from: (f64, f64)) -> Outcome {
    if view.dead || damage <= 0.0 {
        return Outcome::Missed;
    }

    let up = view.blocking_since.is_some_and(|since| now >= since + u64::from(SHIELD_DELAY)) && now >= view.shield_disabled_until;

    if up && source != Source::SonicBoom && shield_covers(victim.2, (victim.0, victim.1), from) {
        return Outcome::Blocked;
    }

    let taken = final_damage(view, damage, source);

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
    if stack.extras().unbreakable {
        return 0;
    }

    info(stack.item).map_or(0, |info| info.max_damage)
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

    if let Some(custom) = &data.attributes {
        return custom.iter().filter(|m| group_matches(m.slot, place)).map(|m| (m.attribute, m.modifier())).collect();
    }

    let Some(info) = info(stack.item) else {
        return Vec::new();
    };

    info.mods
        .iter()
        .filter(|m| group_matches(m.slot, place))
        .map(|m| (m.attribute, Modifier { id: m.id.to_string(), amount: m.amount, operation: Operation::from_number(i32::from(m.operation)).unwrap_or(Operation::AddValue) }))
        .collect()
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
    let undead = matches!(kind, "zombie" | "skeleton" | "husk" | "stray" | "drowned" | "zombie_villager" | "zombified_piglin" | "wither_skeleton" | "phantom" | "wither" | "skeleton_horse" | "zombie_horse" | "bogged" | "parched");
    let arthropod = matches!(kind, "spider" | "cave_spider" | "silverfish" | "endermite" | "bee");

    (undead, arthropod)
}

/// Сила отдачи от удара сверх обычной (0,4): атрибут `attack_knockback`,
/// зачарование «отдача» (1 за уровень) и бег (1). Вики, Knockback.
pub fn extra_knockback(attack_knockback: f64, data: &Data, sprint_bonus: bool) -> f32 {
    attack_knockback as f32 + data.level("knockback") as f32 + if sprint_bonus { 1.0 } else { 0.0 }
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

/// Доля брони, которую оставляет «пробивание» (вики, Breach: «reduces the
/// effectiveness of armor by 15% per level») [проверить].
pub fn breach_scale(level: i32) -> f32 {
    (1.0 - 0.15 * level as f32).max(0.0)
}

// ---------------------------------------------------------------------------
// Броня
// ---------------------------------------------------------------------------

/// Виды урона, которые броня и её зачарования не смягчают (вики, Armor:
/// «Armor does not protect against»).
pub fn bypasses_armor(source: Source) -> bool {
    matches!(source, Source::SonicBoom | Source::Thorns | Source::Fall | Source::Drown | Source::Starve | Source::OutOfWorld | Source::GenericKill | Source::InWall | Source::Generic | Source::OutsideBorder)
}

/// Урон после брони и твёрдости (вики, Armor): `урон · (1 − min(20,
/// max(броня/5, броня − урон/(2 + твёрдость/4)))/25)`.
pub fn after_armor(damage: f32, armor: f32, toughness: f32) -> f32 {
    after_armor_breached(damage, armor, toughness, 1.0)
}

/// То же, но защита брони умножена на `scale` — «пробивание» булавы
/// (`breach_scale`).
pub fn after_armor_breached(damage: f32, armor: f32, toughness: f32, scale: f32) -> f32 {
    let defense = (armor / 5.0).max(armor - damage / (2.0 + toughness / 4.0)).min(20.0) * scale;

    damage * (1.0 - defense / 25.0)
}

/// Урон шипов при броске `roll` из [0; 1): от 1,5 до 4,5 и от уровня не
/// зависит (чёрный ящик, pvp-thorns: шипы III и VII дали 12 значений от 1,51
/// до 4,37; у VII ниже 3,5 — значит, на уровень не множится) [проверить:
/// точная форма распределения].
pub fn thorns_damage(_level: i32, roll: f32) -> f32 {
    3.0 * (0.5 + roll)
}

/// Очки защиты зачарований (EPF) одной детали от этого урона (вики,
/// Protection и др.): защита 1 за уровень от всего, что броня смягчает;
/// огнезащита и взрывоустойчивость и защита от снарядов — по 2 за уровень
/// от своего; лёгкость падения — 3 за уровень от падения.
pub fn protection_points(data: &Data, source: Source) -> i32 {
    let mut points = 0;

    if !bypasses_armor(source) {
        points += data.level("protection");
    }

    match source {
        Source::InFire | Source::Campfire | Source::Lava | Source::OnFire => points += 2 * data.level("fire_protection"),
        // Ракета — тоже взрыв (тег is_explosion; вики, Blast Protection) [проверить].
        Source::Explosion | Source::Fireworks => points += 2 * data.level("blast_protection"),
        Source::Arrow | Source::Trident => points += 2 * data.level("projectile_protection"),
        // Огненный шар — огонь и снаряд сразу (теги is_fire, is_projectile).
        Source::Fireball => points += 2 * data.level("fire_protection") + 2 * data.level("projectile_protection"),
        Source::Fall => points += 3 * data.level("feather_falling"),
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
        damage += (crit_roll * (damage / 2.0 + 2.0)).floor();
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
        // Пробивание II оставляет 70 % защиты: алмазная броня 20 против 10 урона.
        let whole = after_armor(10.0, 20.0, 8.0);
        let breached = after_armor_breached(10.0, 20.0, 8.0, breach_scale(2));

        assert!(breached > whole);
        assert!((breach_scale(2) - 0.7).abs() < 1e-6);
        assert_eq!(breach_scale(7), 0.0);
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
        assert_eq!(protection_points(&data, Source::Fall), 0);
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
    fn thorns_damage_stays_within_the_measured_range() {
        // Шипы III и VII у оригинала: от 1,51 до 4,37, от уровня не зависят.
        for level in [3, 7] {
            assert_eq!(thorns_damage(level, 0.0), 1.5);
            assert!((thorns_damage(level, 0.999) - 4.497).abs() < 1e-3);
        }
    }

    #[test]
    fn thorns_ignore_armor() {
        assert!(bypasses_armor(Source::Thorns));
    }
}
