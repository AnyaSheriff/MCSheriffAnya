// Лошади: правила по minecraft.wiki (Horse, Donkey, Mule, Skeleton Horse,
// Tutorial:Horses). Кода игры нет. [проверить] такты, распределения и движение
// требуют чёрного ящика; сценарии подготовлены, но не запускались.

use super::{Event, Kind, Mob, Rng, tame};
use crate::inventory::Stack;

pub const KINDS: [Kind; 5] = [Kind::Horse, Kind::Donkey, Kind::Mule, Kind::SkeletonHorse, Kind::ZombieHorse];

pub fn equine(kind: Kind) -> bool { KINDS.contains(&kind) }
pub fn undead(kind: Kind) -> bool { matches!(kind, Kind::SkeletonHorse | Kind::ZombieHorse) }
pub fn pack(kind: Kind) -> bool { matches!(kind, Kind::Donkey | Kind::Mule) }
pub fn fertile(kind: Kind) -> bool { matches!(kind, Kind::Horse | Kind::Donkey) }
pub fn compatible(a: Kind, b: Kind) -> bool { fertile(a) && fertile(b) }

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub tamed: bool,
    pub temper: u8,
    pub variant: i32,
    pub speed: f64,
    pub jump: f64,
    pub chest: bool,
    // Два места снаряжения, затем 15 мест сундука. Довески предметов сохраняются.
    pub slots: [Option<Stack>; 17],
    pub rider: Option<([u8; 16], i32)>,
    pub buck: bool,
    pub jump_charge: f64,
    pub input: u8,
    pub rearing: u8,
    pub eating: u8,
    pub trap: bool,
    pub trap_age: i32,
    pub last_move_tick: u64,
    pub move_budget: f64,
    pub rider_fall: f32,
    // Какие атрибуты заданы данными призыва: 1 — здоровье, 2 — скорость, 4 — прыжок.
    pub explicit: u8,
    // Сколько раз менялось снаряжение — повод послать его заново.
    pub gear_version: u32,
    // Шагов под седоком (звук шага: первые пять — «step_wood», дальше галоп).
    pub gallop: u32,
    // Снаряжение, заданное данными, выпадает не всегда (8,5 %); положенное игроком — всегда.
    pub gear_by_data: bool,
    pub air_ticks: u32,
}

impl Default for State {
    fn default() -> Self {
        Self { tamed: false, temper: 0, variant: 0, speed: 0.225, jump: 0.7,
            chest: false, slots: [None; 17], rider: None, buck: false,
            jump_charge: 0.0, input: 0, rearing: 0, eating: 0, trap: false,
            trap_age: 0, last_move_tick: 0, move_budget: 0.0, rider_fall: 0.0, explicit: 0, gear_version: 0, gallop: 0, gear_by_data: false, air_ticks: 0 }
    }
}

impl State {
    pub fn saddled(&self) -> bool { self.slots[0].is_some() }
    pub fn flags(&self) -> u8 {
        (u8::from(self.tamed) << 1) | (u8::from(self.rearing > 0) << 5) | (u8::from(self.eating > 0) << 6)
    }
    pub fn inventory(&self) -> Vec<Option<Stack>> { self.slots[..if self.chest { 17 } else { 2 }].to_vec() }
}

// /summon с данными не бросает кости (чёрный ящик, probe-horse: у лошади,
// осла и мула здоровье 53 и без правок видна лишь скорость, у скелетной
// лошади 15 и 0,2, у зомби-лошади 25 и 0,225): стартовые значения видов.
pub fn defaults(mob: &mut Mob) {
    if !equine(mob.kind) { return; }
    mob.vitals.max_health = match mob.kind { Kind::SkeletonHorse => 15.0, Kind::ZombieHorse => 25.0, _ => 53.0 };
    mob.vitals.health = mob.vitals.max_health;
    mob.horse.speed = match mob.kind { Kind::Donkey | Kind::Mule => 0.175, Kind::SkeletonHorse => 0.2, _ => 0.225 };
    mob.horse.jump = 0.7;
}

// Вики: здоровье 15–30 (середина чаще), скорость 0,1125–0,3375,
// прыжок 0,4–1; у осла/мула 0,175 и 0,5. [проверить] распределения.
pub fn initialize(mob: &mut Mob, rng: &mut Rng) {
    if !equine(mob.kind) { return; }
    mob.vitals.max_health = if undead(mob.kind) { 15.0 } else { (15 + rng.below(8) + rng.below(9)) as f32 };
    mob.vitals.health = mob.vitals.max_health;
    mob.horse.speed = if pack(mob.kind) { 0.175 } else if undead(mob.kind) { 0.2 } else { 0.1125 + (rng.unit() + rng.unit() + rng.unit()) * 0.075 };
    mob.horse.jump = if pack(mob.kind) { 0.5 } else { 0.4 + (rng.unit() + rng.unit() + rng.unit()) * 0.2 };
    // Брошенные кости уходят клиенту при появлении (чёрный ящик, probe-ride: три атрибута на +0).
    mob.horse.explicit = 7;
}

pub fn wild_variant(rng: &mut Rng) -> i32 { rng.below(7) as i32 | ((rng.below(5) as i32) << 8) }
pub fn variant(value: i32) -> i32 { (value & 255).clamp(0, 6) | (((value >> 8) & 255).clamp(0, 4) << 8) }

// Одобренная таблица combat_table.rs: броня и прочность материала 26.1.2.
pub fn armor(item: &str) -> f32 {
    if !item.ends_with("_horse_armor") { return 0.0; }
    crate::blocks::item_named(item).map_or(0.0, |id| equipment_attribute(id, crate::attributes::ARMOR))
}

fn equipment_attribute(item: i32, attribute: i32) -> f32 {
    crate::combat::info(item).and_then(|info| info.mods.iter().find(|m| m.attribute == attribute)).map_or(0.0, |m| m.amount as f32)
}

pub fn toughness(mob: &Mob) -> f32 {
    if mob.kind != Kind::Horse { return 0.0; }
    mob.horse.slots[1].map_or(0.0, |s| equipment_attribute(s.item, crate::attributes::ARMOR_TOUGHNESS))
}

pub fn can_parent(mob: &Mob) -> bool {
    fertile(mob.kind) && mob.horse.tamed && mob.horse.rider.is_none() && mob.age == 0 && mob.vitals.health >= mob.vitals.max_health
}

pub fn may_place(kind: Kind, slot: usize, item: i32) -> bool {
    let name = crate::blocks::item_name(item).unwrap_or("");
    match slot { 0 => name == "saddle", 1 => kind == Kind::Horse && armor(name) > 0.0, _ => pack(kind) }
}

pub fn equipment_changed(mob: &mut Mob, old: [Option<Stack>; 17]) {
    if mob.horse.slots[0] != old[0] && mob.horse.saddled() { sound(mob, "minecraft:entity.horse.saddle", 1.0); }
    if mob.horse.slots[1] != old[1] && mob.horse.slots[1].is_some() { sound(mob, "minecraft:entity.horse.armor", 1.0); }
    if mob.horse.slots[..2] != old[..2] { mob.horse.gear_version = mob.horse.gear_version.wrapping_add(1); mob.horse.gear_by_data = false; }
}

pub fn sound(mob: &mut Mob, name: &'static str, volume: f32) {
    if !mob.silent { mob.events.push(Event::Sound { name, volume, pitch: if mob.is_baby() { 1.5 } else { 1.0 } }); }
}

pub fn angry(mob: &mut Mob) {
    mob.horse.rearing = 20;
    let name = match mob.kind { Kind::Donkey => "minecraft:entity.donkey.angry", Kind::Mule => "minecraft:entity.mule.angry", _ => "minecraft:entity.horse.angry" };
    sound(mob, name, 0.8);
}

// Вики, Mule, «Food»: здоровье, секунды роста, темперамент.
pub fn food(item: &str) -> Option<(f32, i32, u8)> {
    Some(match item {
        "sugar" => (1.0, 30, 3), "wheat" => (2.0, 20, 3), "apple" => (3.0, 60, 3),
        "golden_carrot" => (4.0, 60, 5), "golden_apple" | "enchanted_golden_apple" => (10.0, 240, 10),
        "hay_block" => (20.0, 180, 0), _ => return None,
    })
}

pub fn feed(mob: &mut Mob, item: &str) -> bool {
    if undead(mob.kind) || (item == "hay_block" && !mob.horse.tamed && !mob.is_baby()) { return false; }
    let Some((heal, growth, temper)) = food(item) else { return false; };
    let love = fertile(mob.kind) && mob.horse.tamed && mob.age == 0 && mob.love == 0
        && matches!(item, "golden_carrot" | "golden_apple" | "enchanted_golden_apple");
    let useful = mob.vitals.health < mob.vitals.max_health || mob.is_baby() || love || (!mob.horse.tamed && mob.horse.temper < 100 && temper > 0);
    if !useful { return false; }
    mob.vitals.health = (mob.vitals.health + heal).min(mob.vitals.max_health);
    if mob.is_baby() { mob.age = (mob.age + growth * 20).min(0); }
    mob.horse.temper = mob.horse.temper.saturating_add(temper).min(100);
    if love { mob.love = super::LOVE_TICKS; mob.events.push(Event::Hearts); }
    mob.horse.eating = 30;
    sound(mob, match mob.kind { Kind::Donkey => "minecraft:entity.donkey.eat", Kind::Mule => "minecraft:entity.mule.eat", _ => "minecraft:entity.horse.eat" }, 1.0);
    true
}

pub fn tame_attempt(mob: &mut Mob, roll: u32) -> bool {
    let Some(who) = mob.horse.rider else { return false; };
    if mob.horse.tamed || undead(mob.kind) { return false; }
    if roll < u32::from(mob.horse.temper) {
        tame::tame(mob, who);
        mob.tame.ordered_sit = false;
        mob.horse.tamed = true;
        mob.events.push(Event::Status(7));
        true
    } else {
        mob.horse.temper = mob.horse.temper.saturating_add(5).min(100);
        mob.horse.buck = true;
        mob.events.push(Event::Status(6));
        angry(mob);
        false
    }
}

pub fn tick(mob: &mut Mob, rng: &mut Rng) {
    if undead(mob.kind) {
        mob.vitals.effects.retain(|e| !matches!(e.kind, crate::survival::EffectKind::Poison | crate::survival::EffectKind::Regeneration));
    }
    for (amount, source) in mob.vitals.effects_tick().hurt { mob.hurt(amount, source, None); }
    if mob.is_dead() { return; }
    if !mob.on_ground { mob.horse.air_ticks = mob.horse.air_ticks.saturating_add(1); } else { mob.horse.air_ticks = 0; }
    mob.horse.rearing = mob.horse.rearing.saturating_sub(1);
    mob.horse.eating = mob.horse.eating.saturating_sub(1);
    if !mob.no_ai && mob.horse.rider.is_some() && !mob.horse.tamed && !mob.horse.buck && rng.below(50) == 0 {
        // [проверить] частота попыток; порог 0..99 и +5 — вики, Taming.
        tame_attempt(mob, rng.below(100));
    }
    if !mob.no_ai && mob.ticks.is_multiple_of(900) && mob.vitals.health < mob.vitals.max_health {
        mob.vitals.health = (mob.vitals.health + 1.0).min(mob.vitals.max_health);
    }
}

// Вики, Tutorial:Horses: «averaging the two parent horses … random variance».
// [проверить] величина разброса, отражение у границ — гипотеза для чёрного ящика.
pub fn inherit(a: f64, b: f64, min: f64, max: f64, rng: &mut Rng) -> f64 {
    let a = a.clamp(min, max);
    let b = b.clamp(min, max);
    let value = (a + b) / 2.0 + ((a - b).abs() + (max - min) * 0.3) * ((rng.unit() + rng.unit() + rng.unit()) / 3.0 - 0.5);
    if value > max { 2.0 * max - value } else if value < min { 2.0 * min - value } else { value }
}

pub fn child(a: &Mob, b: &Mob, id: i32, rng: &mut Rng) -> Mob {
    let kind = if a.kind == b.kind { a.kind } else { Kind::Mule };
    let mut baby = Mob::new(id, kind, (a.x, a.y, a.z));
    baby.age = super::BABY_AGE;
    baby.vitals.max_health = inherit(f64::from(a.vitals.max_health), f64::from(b.vitals.max_health), 15.0, 30.0, rng) as f32;
    baby.vitals.health = baby.vitals.max_health;
    baby.horse.speed = inherit(a.horse.speed, b.horse.speed, 0.1125, 0.3375, rng);
    baby.horse.jump = inherit(a.horse.jump, b.horse.jump, 0.4, 1.0, rng);
    baby.horse.explicit = 7;
    if kind == Kind::Horse {
        // Вики, Horse: масть 4/9 + 4/9 + 1/9; отметины 2/5 + 2/5 + 1/5.
        let coat = match rng.below(9) { 0..=3 => a.horse.variant & 255, 4..=7 => b.horse.variant & 255, _ => rng.below(7) as i32 };
        let marks = match rng.below(5) { 0..=1 => a.horse.variant >> 8, 2..=3 => b.horse.variant >> 8, _ => rng.below(5) as i32 };
        baby.horse.variant = coat | (marks << 8);
    }
    baby
}

// Полная шкала от 90; иначе 40–80 % прыжка. [проверить] на пакетах клиента.
pub fn charge(value: i32) -> f64 { if value >= 90 { 1.0 } else { 0.4 + 0.4 * f64::from(value.clamp(0, 90)) / 90.0 } }

// Вики, Mule: «safely fall 6 blocks», «half … rounded up».
pub fn fall_damage(distance: f32) -> f32 { ((distance - 6.0 - 1.0e-4) * 0.5).ceil().max(0.0) }

pub fn seat(mob: &Mob) -> (f64, f64, f64) {
    // Высота ног седока над ногами зверя (чёрный ящик, probe-ride).
    let up = match mob.kind { Kind::Horse => 0.844, Kind::Donkey => 0.512, Kind::Mule => 0.612, _ => 0.719 };
    (mob.x, mob.y + up, mob.z)
}

pub fn apply_data(mob: &mut Mob, data: &crate::snbt::Value) {
    if let Some(v) = data.get("Tame").and_then(|v| v.as_f64()) { mob.horse.tamed = v != 0.0; }
    if let Some(v) = data.get("InLove").and_then(|v| v.as_f64()) { mob.love = if fertile(mob.kind) { v.clamp(0.0, 600.0) as i32 } else { 0 }; }
    if let Some(v) = data.get("Temper").and_then(|v| v.as_f64()) { mob.horse.temper = v.clamp(0.0, 100.0) as u8; }
    if let Some(v) = data.get("Variant").and_then(|v| v.as_f64()) { mob.horse.variant = variant(v as i32); }
    if let Some(v) = data.get("ChestedHorse").and_then(|v| v.as_f64()) { mob.horse.chest = pack(mob.kind) && v != 0.0; }
    if let Some(v) = data.get("SkeletonTrap").and_then(|v| v.as_f64()) { mob.horse.trap = mob.kind == Kind::SkeletonHorse && v != 0.0; }
    if let Some(v) = data.get("SkeletonTrapTime").and_then(|v| v.as_f64()) { mob.horse.trap_age = v.max(0.0) as i32; }
    for (slot, key, modern) in [(0, "SaddleItem", "saddle"), (1, "ArmorItem", "body")] {
        if let Some(v) = data.get(key).or_else(|| data.get("equipment").and_then(|v| v.get(modern))) {
            mob.horse.slots[slot] = stack(v).filter(|s| may_place(mob.kind, slot, s.item)).map(|s| s.with_count(1));
        }
    }
    if let Some(crate::snbt::Value::List(list)) = data.get("Items") {
        for v in list {
            if let Some(slot) = v.get("Slot").and_then(|v| v.as_f64()).map(|v| v as usize)
                && (2..17).contains(&slot) && mob.horse.chest { mob.horse.slots[slot] = stack(v); }
        }
    }
    if let Some(crate::snbt::Value::List(list)) = data.get("attributes").or_else(|| data.get("Attributes")) {
        for v in list {
            let name = v.get("id").or_else(|| v.get("Name")).and_then(|v| v.as_str()).unwrap_or("").trim_start_matches("minecraft:");
            if let Some(base) = v.get("base").or_else(|| v.get("Base")).and_then(|v| v.as_f64()).filter(|n| n.is_finite()) {
                match name {
                    "max_health" => { mob.vitals.max_health = base.clamp(1.0, 1024.0) as f32; mob.horse.explicit |= 1; }
                    "movement_speed" => { mob.horse.speed = base.clamp(0.0, 1024.0); mob.horse.explicit |= 2; }
                    "jump_strength" => { mob.horse.jump = base.clamp(0.0, 32.0); mob.horse.explicit |= 4; }
                    _ => {}
                }
            }
        }
    }
    if let Some(crate::snbt::Value::List(list)) = data.get("Owner") && list.len() == 4 {
        let mut uuid = [0; 16];
        for (i, n) in list.iter().enumerate() { uuid[i * 4..i * 4 + 4].copy_from_slice(&(n.as_f64().unwrap_or(0.0) as i32).to_be_bytes()); }
        mob.tame.owner = Some(uuid);
    }
    mob.vitals.health = mob.vitals.health.min(mob.vitals.max_health);
    if let Some(v) = data.get("Health").and_then(|v| v.as_f64()).filter(|v| v.is_finite()) { mob.vitals.health = (v as f32).clamp(0.0, mob.vitals.max_health); }
    if data.get("SaddleItem").is_some() || data.get("equipment").is_some() || data.get("ArmorItem").is_some() { mob.horse.gear_by_data = true; }
    if data.get("attributes").or_else(|| data.get("Attributes")).is_some() { mob.attr_changes = mob.attr_changes.wrapping_add(1); }
}

fn stack(data: &crate::snbt::Value) -> Option<Stack> {
    let item = crate::blocks::item_named(data.get("id")?.as_str()?.trim_start_matches("minecraft:"))?;
    let count = data.get("count").or_else(|| data.get("Count")).and_then(|v| v.as_f64()).unwrap_or(1.0) as i32;
    if count <= 0 { return None; }
    let mut stack = Stack::new(item, count.min(crate::blocks::stack_size(item)));
    if let Some(crate::snbt::Value::Map(fields)) = data.get("components") {
        // Один разбор довесков с /give: имена, чары и атрибуты не теряются.
        let fields = fields.iter().map(|(key, value)| format!("{key}={}", snbt_text(value))).collect::<Vec<_>>().join(",");
        let spec = format!("{}[{fields}]", crate::blocks::item_name(item)?);
        if let Ok((_, extras)) = crate::stackdata::parse_item_spec(&spec) { stack = stack.with_extras(extras); }
    }
    Some(stack)
}

fn snbt_text(value: &crate::snbt::Value) -> String {
    use crate::snbt::Value;
    match value {
        Value::Num(n) => n.to_string(),
        Value::Str(s) => serde_json::to_string(s).expect("строка SNBT"),
        Value::List(list) => format!("[{}]", list.iter().map(snbt_text).collect::<Vec<_>>().join(",")),
        Value::Map(fields) => format!("{{{}}}", fields.iter().map(|(k, v)| format!("{}:{}", serde_json::to_string(k).expect("ключ SNBT"), snbt_text(v))).collect::<Vec<_>>().join(",")),
    }
}

pub fn write(mob: &Mob, out: &mut Vec<u8>) {
    tame::write(&mob.tame, out);
    out.extend_from_slice(&mob.vitals.max_health.to_be_bytes());
    out.extend_from_slice(&mob.horse.speed.to_be_bytes());
    out.extend_from_slice(&mob.horse.jump.to_be_bytes());
    out.extend_from_slice(&mob.horse.variant.to_be_bytes());
    out.extend_from_slice(&[u8::from(mob.horse.tamed), mob.horse.temper, u8::from(mob.horse.chest), u8::from(mob.horse.trap), u8::from(mob.silent)]);
    out.extend_from_slice(&mob.horse.trap_age.to_be_bytes());
    for slot in mob.horse.slots {
        out.push(u8::from(slot.is_some()));
        if let Some(s) = slot {
            out.extend_from_slice(&s.stored_item().to_be_bytes());
            out.extend_from_slice(&s.count.to_be_bytes());
            s.write_extras(out);
        }
    }
}

pub fn read(mob: &mut Mob, bytes: &mut &[u8]) -> Option<()> {
    mob.tame = tame::read(bytes)?;
    let mut take = |count: usize| { let (head, rest) = bytes.split_at_checked(count)?; *bytes = rest; Some(head) };
    let health = f32::from_be_bytes(take(4)?.try_into().ok()?);
    let speed = f64::from_be_bytes(take(8)?.try_into().ok()?);
    let jump = f64::from_be_bytes(take(8)?.try_into().ok()?);
    if !health.is_finite() || health <= 0.0 || !speed.is_finite() || speed < 0.0 || !jump.is_finite() || jump < 0.0 { return None; }
    mob.vitals.max_health = health;
    mob.horse.speed = speed;
    mob.horse.jump = jump;
    mob.horse.variant = variant(i32::from_be_bytes(take(4)?.try_into().ok()?));
    mob.horse.tamed = take(1)?[0] != 0;
    mob.horse.temper = take(1)?[0].min(100);
    mob.horse.chest = take(1)?[0] != 0 && pack(mob.kind);
    mob.horse.trap = take(1)?[0] != 0 && mob.kind == Kind::SkeletonHorse;
    mob.silent = take(1)?[0] != 0;
    mob.horse.trap_age = i32::from_be_bytes(take(4)?.try_into().ok()?);
    for slot in &mut mob.horse.slots {
        if take(1)?[0] == 0 { continue; }
        let stored = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let count = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let extras = Stack::read_extras(stored, &mut take)?;
        let mut s = Stack::from_stored(stored, count);
        if let Some(extras) = extras { s = s.with_extras(extras); }
        *slot = (count > 0).then_some(s);
    }
    Some(())
}

/// Проверка клиентского движения без доверия флагу земли и без прохождения
/// сквозь блоки. [проверить] допуски для задержек пакетов, воды и эффектов.
pub fn drive(mob: &mut Mob, view: &dyn super::View, to: (f64, f64, f64), look: (f32, f32), _ground: bool) -> bool {
    if mob.is_dead() || !mob.horse.tamed || !mob.horse.saddled() || mob.horse.rider.is_none() { return false; }
    if !to.0.is_finite() || !to.1.is_finite() || !to.2.is_finite() || !look.0.is_finite() || !look.1.is_finite()
        || to.0.abs() > 30_000_000.0 || to.2.abs() > 30_000_000.0 || to.1.abs() > 20_000_000.0
        || !view.loaded((to.0.floor() as i32).div_euclid(16), (to.2.floor() as i32).div_euclid(16)) { return false; }
    let delta = (to.0 - mob.x, to.1 - mob.y, to.2 - mob.z);
    let elapsed = mob.ticks.saturating_sub(mob.horse.last_move_tick).min(5) as f64;
    // Скорость атрибута соответствует примерно 43 блокам/с на единицу.
    // Кнопка sprint не добавляет лошади множитель бега игрока.
    let per_tick = mob.horse.speed * 2.2 + 0.15;
    let budget = (mob.horse.move_budget + elapsed * per_tick).min(per_tick * 5.0);
    let distance = delta.0.hypot(delta.2);
    if distance > budget + 1.0e-6 || delta.1.abs() > 10.0 { return false; }
    // Подъём не требует заряда прыжка: оригинал сверяет ход клиента лишь с
    // препятствиями и скоростью (чёрный ящик, horse-jump: ход вверх при
    // заряде 0 принят без сверки места).
    let mut candidate = mob.clone();
    if candidate.horse.air_ticks > 200 && delta.1 >= -0.001 { return false; }
    candidate.step(view, delta.0, delta.1, delta.2);
    if (candidate.x - to.0).abs() + (candidate.y - to.1).abs() + (candidate.z - to.2).abs() > 0.0625 { return false; }
    // Нулевая вертикаль тоже может означать стояние: проверяем опору ящика.
    let (width, height) = candidate.size();
    candidate.on_ground = super::clip_y(view, (candidate.x - width / 2.0, candidate.y, candidate.z - width / 2.0), width / 2.0, height, -1.0e-5) > -1.0e-5;
    if candidate.on_ground {
        // Приземление после прыжка со звуком (чёрный ящик, horse-jump: land 0,40).
        if !mob.on_ground && mob.horse.jump_charge > 0.0 {
            candidate.horse.jump_charge = 0.0;
            sound(&mut candidate, "minecraft:entity.horse.land", 0.4);
        }
        candidate.horse.air_ticks = 0;
        if candidate.vitals.fall > 0.0 {
            let damage = fall_damage(candidate.vitals.fall);
            candidate.vitals.fall = 0.0;
            if damage > 0.0 { candidate.hurt(damage, crate::survival::Source::Fall, None); candidate.horse.rider_fall += damage; }
        }
    }
    candidate.face(look.0);
    candidate.pitch = look.1 * 0.5;
    // Скорость седла сервер не разглашает: ход ведёт клиент (чёрный ящик,
    // horse-saddle-ride: ни одной скорости под седоком).
    candidate.vx = 0.0;
    candidate.vy = 0.0;
    candidate.vz = 0.0;
    candidate.horse.last_move_tick = mob.ticks;
    candidate.horse.move_budget = (budget - distance).max(0.0);
    // Шаг — за каждые 1/0,6 блока пути (чёрный ящик, horse-saddle-ride:
    // первый звук после 1,675 блока, второй через 1,625). Под седоком первые
    // пять шагов — «step_wood» на любом полу, дальше «gallop» через два шага
    // на третий (horse-tame-run: пять step_wood, затем gallop). Громкость 0,15.
    if candidate.on_ground { candidate.move_dist += distance * 0.6; }
    if candidate.on_ground && candidate.move_dist > candidate.next_step {
        candidate.next_step = candidate.move_dist.floor() + 1.0;
        candidate.horse.gallop += 1;
        if candidate.horse.gallop <= 5 { sound(&mut candidate, "minecraft:entity.horse.step_wood", 0.15); }
        else if candidate.horse.gallop.is_multiple_of(3) { sound(&mut candidate, "minecraft:entity.horse.gallop", 0.15); }
    }
    *mob = candidate;
    true
}

/// Приводит общее сидение и состояние моба в соответствие. Возвращает урон
/// седоку при падении; отключившийся игрок не остаётся водителем.
pub fn sync_riding(items: &mut crate::items::Items, riding: &mut crate::riding::Riding, owners: &[([u8; 16], i32)], view: &dyn super::View) -> Vec<(i32, f32)> {
    let mut hurt = Vec::new();
    for mob in items.mobs_mut().iter_mut().filter(|mob| equine(mob.kind)) {
        if mob.horse.rider_fall > 0.0 && let Some((_, rider)) = mob.horse.rider {
            hurt.push((rider, mob.horse.rider_fall));
        }
        mob.horse.rider_fall = 0.0;
        let underwater = !undead(mob.kind) && super::is_water(view.block(mob.x.floor() as i32, (mob.y + 1.6).floor() as i32, mob.z.floor() as i32));
        if mob.is_dead() || mob.h.despawn || mob.horse.buck || underwater {
            riding.clear_vehicle(mob.entity_id);
            mob.horse.buck = false;
        }
        mob.horse.rider = riding.driver(mob.entity_id).and_then(|id| owners.iter().find(|(_, who)| *who == id).copied());
    }
    let seats: Vec<(i32, (f64, f64, f64))> = items.mobs().iter().filter(|m| equine(m.kind) && !m.is_dead())
        .flat_map(|m| riding.passengers(m.entity_id).iter().map(|id| (*id, seat(m))).collect::<Vec<_>>()).collect();
    for (id, at) in seats {
        if let Some(rider) = items.mobs_mut().iter_mut().find(|m| m.entity_id == id) {
            if rider.is_dead() { riding.dismount(id); } else {
                (rider.x, rider.y, rider.z) = at;
                rider.vitals.fall = 0.0;
                rider.vx = 0.0; rider.vy = 0.0; rider.vz = 0.0;
            }
        } else if !owners.iter().any(|(_, who)| *who == id) { riding.dismount(id); }
    }
    hurt
}

/// Вики, Skeleton Horse: четыре всадника при приближении на 10 блоков,
/// неприведённая в действие ловушка исчезает через 18000 тактов.
/// [проверить] чары снаряжения и разлёт всадников.
pub fn traps(items: &mut crate::items::Items, riding: &mut crate::riding::Riding, players: &[super::ai::Watcher], rng: &mut Rng, next: &mut dyn FnMut() -> i32) {
    let mut triggered = Vec::new();
    for mob in items.mobs_mut().iter_mut().filter(|m| m.kind == Kind::SkeletonHorse && m.horse.trap && !m.is_dead()) {
        mob.horse.trap_age += 1;
        if mob.horse.trap_age >= 18000 { mob.h.despawn = true; continue; }
        if !mob.no_ai && players.iter().any(|p| !p.spectator && (p.x - mob.x).powi(2) + (p.y - mob.y).powi(2) + (p.z - mob.z).powi(2) <= 100.0) {
            mob.horse.trap = false;
            mob.horse.tamed = true;
            mob.age = 0;
            triggered.push((mob.entity_id, (mob.x, mob.y, mob.z)));
        }
    }
    for (first, at) in triggered {
        let mut bolt = crate::lightning::Bolt::new(next(), at);
        bolt.visual_only = true;
        items.add_bolt(bolt);
        for index in 0..4 {
            let id = if index == 0 { first } else {
                let mut horse = Mob::new(next(), Kind::SkeletonHorse, at);
                initialize(&mut horse, rng);
                horse.horse.tamed = true;
                horse.vx = (rng.unit() - 0.5) * 0.6;
                horse.vz = (rng.unit() - 0.5) * 0.6;
                let id = horse.entity_id;
                items.add_mob(horse);
                id
            };
            let mut skeleton = Mob::new(next(), Kind::Skeleton, (at.0, at.1 + 1.2, at.2));
            skeleton.h.bow = true;
            skeleton.h.helmet = true;
            skeleton.persistent_tag = true;
            riding.mount(skeleton.entity_id, id, 1);
            items.add_mob(skeleton);
        }
    }
}

#[cfg(test)]
mod tests;
