// Здоровье и голод игрока Bedrock: счёт общий с Java (survival.rs), здесь —
// пакеты Bedrock. Названия и порядок полей — документация Mojang к r/26_u1
// (UpdateAttributesPacket, ActorEventPacket, DeathInfoPacket, RespawnPacket,
// enum ActorEvent, PlayerActionType).
//
// Порядок смерти и возрождения (по описанию пакетов; живой клиент ещё не
// проверен [проверить]):
//   1. здоровье в атрибутах уходит клиенту — на нуле он сам рисует экран
//      смерти; следом ActorEvent DEATH и DeathInfo с сообщением;
//   2. клиент по кнопке «возродиться» шлёт Player Action «Respawn» (или сам
//      Respawn с состоянием «ClientReadyToSpawn»);
//   3. сервер переносит его к точке возрождения, возвращает здоровье и
//      отвечает Respawn «ReadyToSpawn».

use super::codec::Out;
use super::session;
use crate::survival::{self, Source, Vitals};
use crate::world;

pub const ACTOR_EVENT: u32 = 0x1b;
pub const RESPAWN: u32 = 0x2d;
pub const DEATH_INFO: u32 = 0xbd;

/// ActorEvent: HURT — индекс 2, DEATH — 3 (enum ActorEvent).
const EVENT_HURT: u8 = 2;
const EVENT_DEATH: u8 = 3;

/// Состояния RespawnPacket (uint8).
pub const STATE_READY: u8 = 1;
pub const STATE_CLIENT_READY: u8 = 2;

/// PlayerActionType «Respawn»: в перечислении документации он восьмой, но
/// первым там стоит «Unknown» (−1) — так же, как ChangeDimensionAck = 14.
pub const ACTION_RESPAWN: i32 = 7;

/// Ниже этой высоты — бездна (как у Java: дно мира минус 64).
const VOID_BELOW: f32 = world::MIN_Y as f32 - 64.0;

/// Что клиент уже знает об атрибутах.
#[derive(Clone, Copy, PartialEq)]
struct Sent {
    health: f32,
    food: i32,
    saturation: f32,
    exhaustion: f32,
    absorption: f32,
    max_health: f32,
    xp: i32,
}

impl Sent {
    /// То же, что видит клиент: без изнурения, которое он считает сам.
    fn same_visible(&self, other: &Sent) -> bool {
        Sent { exhaustion: other.exhaustion, ..*self } == *other
    }
}

/// Здоровье игрока Bedrock.
pub struct Health {
    pub vit: Vitals,
    hits: Vec<(f32, Source, bool, Vec<survival::Effect>)>,
    sent: Option<Sent>,
}

/// Исход такта.
pub struct Outcome {
    pub packets: Vec<Vec<u8>>,
    pub died: Option<Source>,
    /// Виды урона, которые прошли в этот такт: остальным игрокам — «ранен».
    pub hurts: Vec<Source>,
    pub slots: Vec<i32>,
}

pub struct Rules {
    pub mortal: bool,
    pub immune: bool,
    pub difficulty: u8,
    pub feet_y: f32,
    pub natural_regeneration: bool,
}

/// Вики, Totem of Undying: «held in either hand». Сроки эффектов — Java,
/// поскольку Bedrock здесь играет в том же мире, а не по отдельным правилам.
fn revive(vit: &mut Vitals, inventory: &mut crate::inventory::Inventory) -> Option<i32> {
    let totem = crate::blocks::item_named("totem_of_undying")?;
    let slot = [inventory.selected_slot(), crate::inventory::OFFHAND].into_iter()
        .find(|slot| inventory.slot(*slot).is_some_and(|s| s.item == totem))?;
    inventory.set(slot, crate::inventory::take_one(inventory.slot(slot)));
    vit.dead = false; vit.health = 1.0; vit.remove_effects(None); vit.absorption = 0.0;
    for (kind, amplifier, ticks_left) in [(survival::EffectKind::Regeneration, 1, 900),
        (survival::EffectKind::Absorption, 1, 100), (survival::EffectKind::FireResistance, 0, 800)] {
        vit.give_effect_shown(survival::Effect { kind, amplifier, ticks_left, ambient: false, visible: true, show_icon: true });
    }
    Some(slot)
}

impl Health {
    pub fn new(saved: Option<&crate::playerdata::SavedVitals>) -> Self {
        let mut vit = saved.map(Vitals::restored).unwrap_or_default();
        if vit.xp_seed == 0 { vit.xp_seed = crate::enchanting::new_seed(0); }
        Self { vit, hits: Vec::new(), sent: None }
    }

    /// Возрождение: всё заново.
    pub fn respawned(&mut self) {
        self.vit.respawned();
        self.hits.clear();
        self.sent = None;
    }

    /// Удар, который разберётся в ближайшем такте.
    pub fn hurt(&mut self, amount: f32, source: Source) {
        self.hits.push((amount, source, false, Vec::new()));
    }

    /// Урон после брони/сложности из общего тела: второй раз не ослабляем.
    #[cfg(test)]
    pub fn hurt_final(&mut self, amount: f32, source: Source) {
        self.hurt_with_effects(amount, source, Vec::new());
    }

    /// Движение по вводу клиента: прыжок, бег, падение. `dy` — сдвиг ног по
    /// высоте, `run` — путь по земле, `soft`/`multiplier` — среда и блок
    /// внизу (survival::soft_block, landing_multiplier).
    pub fn moved(&mut self, dy: f64, on_ground: bool, run: f64, jumped: bool, soft: bool, multiplier: f32) {
        if self.vit.dead {
            return;
        }

        if jumped {
            self.vit.jumped();
        }

        if on_ground && self.vit.sprinting && run > 0.0 {
            self.vit.ran(run);
        }

        if let Some(damage) = self.vit.land(dy, on_ground, soft, multiplier) {
            self.hits.push((damage, Source::Fall, false, Vec::new()));
        }
    }

    pub fn hurt_with_effects(&mut self, amount: f32, source: Source, effects: Vec<survival::Effect>) {
        self.hits.push((amount, source, true, effects));
    }

    /// Такт: удары, бездна, голод, лечение; изменившиеся атрибуты — клиенту.
    /// `mortal` — выживание или приключение, `immune` — творческий или
    /// наблюдатель (бьёт только бездна).
    #[cfg(test)]
    pub fn tick(&mut self, runtime_id: u64, mortal: bool, immune: bool, difficulty: u8, feet_y: f32) -> Outcome {
        self.step(runtime_id, Rules { mortal, immune, difficulty, feet_y, natural_regeneration: true },
            &mut crate::inventory::Inventory::new(), &crate::attributes::Attributes::player())
    }

    pub fn step(&mut self, runtime_id: u64, rules: Rules, inventory: &mut crate::inventory::Inventory, attrs: &crate::attributes::Attributes) -> Outcome {
        let mut out = Outcome { packets: Vec::new(), died: None, hurts: Vec::new(), slots: Vec::new() };
        if self.vit.dead { self.hits.clear(); return out; }
        if rules.feet_y < VOID_BELOW { self.hits.push((4.0, Source::OutOfWorld, false, Vec::new())); }

        for (amount, source, final_damage, effects) in std::mem::take(&mut self.hits) {
            let before = out.hurts.len();
            self.apply_hit(runtime_id, (amount, source, final_damage), &rules, inventory, attrs, &mut out);
            if out.hurts.len() != before && out.died.is_none() {
                for effect in effects { self.vit.give_effect_shown(effect); }
            }
            if out.died.is_some() { break; }
        }
        if out.died.is_none() {
            self.vit.age = self.vit.age.wrapping_add(1);
            // Эффекты тикают и в творческом режиме, как у Java.
            for (amount, source) in self.vit.effects_tick().hurt {
                self.apply_hit(runtime_id, (amount, source, false), &rules, inventory, attrs, &mut out);
                if out.died.is_some() { break; }
            }
        }
        if out.died.is_none() && rules.mortal {
            if rules.difficulty == survival::PEACEFUL {
                if self.vit.health < self.vit.max_health && self.vit.age.is_multiple_of(20) { self.vit.heal(1.0); }
                if self.vit.food < 20 && self.vit.age.is_multiple_of(10) { self.vit.food += 1; }
            }
            if let Some(amount) = self.vit.food_tick(rules.difficulty, rules.natural_regeneration) {
                self.apply_hit(runtime_id, (amount, Source::Starve, false), &rules, inventory, attrs, &mut out);
            }
        }
        self.vit.tick_invulnerable();
        let now = Sent { health: self.vit.health, food: self.vit.food, saturation: self.vit.saturation,
            exhaustion: self.vit.exhaustion, absorption: self.vit.absorption, max_health: self.vit.max_health, xp: self.vit.xp };
        // BDS (br-record: effects/hunger/absorption_xp): клиенту уходят только
        // изменившиеся атрибуты, а голод-«усталость» (exhaustion) он считает сам —
        // её каждый такт не шлём.
        if let Some(packet) = attributes_since(runtime_id, &self.vit, self.sent.as_ref()) { out.packets.push(packet); }
        if self.sent.as_ref().is_none_or(|prev| !prev.same_visible(&now)) { self.sent = Some(now); }
        out
    }

    fn apply_hit(&mut self, id: u64, (mut amount, source, final_damage): (f32, Source, bool), rules: &Rules,
        inventory: &mut crate::inventory::Inventory, attrs: &crate::attributes::Attributes, out: &mut Outcome) {
        if !final_damage {
            if source.scales_with_difficulty() { amount = survival::scaled(amount, rules.difficulty); }
            let points = crate::combat::ARMOR_SLOTS.iter().filter_map(|(slot, _)| inventory.slot(*slot))
                .map(|s| crate::combat::protection_points(&s.extras(), source)).sum();
            amount = crate::combat::mitigated_damage(amount, attrs.value(crate::attributes::ARMOR) as f32,
                attrs.value(crate::attributes::ARMOR_TOUGHNESS) as f32, points, source, 1.0);
        }
        if let Some(hit) = self.vit.hurt(amount, source, rules.immune) {
            out.packets.push(hurt_event(id, source)); out.hurts.push(source);
            if hit.died {
                if !source.bypasses_invulnerability() && let Some(slot) = revive(&mut self.vit, inventory) {
                    out.slots.push(slot);
                    // ActorEvent TALISMAN_ACTIVATE = 65 (Mojang enum, не индекс массива).
                    out.packets.push(actor_event(id, 65));
                } else { out.died = Some(source); }
            }
        }
    }

}

/// ActorEvent: номер сущности, событие, данные.
pub fn actor_event(runtime_id: u64, event: u8) -> Vec<u8> {
    actor_event_data(runtime_id, event, 0)
}

/// Actor Event с данными: у «ранен» (2) это причина урона (ActorDamageCause);
/// BDS шлёт 2 («удар существа») при ударе и −1 («без причины») при команде
/// `damage` (чёрный ящик Bedrock, tmp/br-sounds).
pub fn actor_event_data(runtime_id: u64, event: u8, data: i32) -> Vec<u8> {
    let mut out = Out::packet(ACTOR_EVENT);

    out.varint64(runtime_id).u8(event).zigzag32(data);
    out.bytes
}

/// Событие «ранен» с причиной урона вида `source`.
pub fn hurt_event(runtime_id: u64, source: Source) -> Vec<u8> {
    actor_event_data(runtime_id, EVENT_HURT, damage_cause(source))
}

/// Причина урона по номеру вида из Damage Event (`Source::type_id`).
pub fn damage_cause_of_type(type_id: i32) -> i32 {
    const ALL: &[Source] = &[
        Source::BadRespawnPoint,
        Source::Cramming,
        Source::DragonBreath,
        Source::EnderPearl,
        Source::FallingAnvil,
        Source::FallingBlock,
        Source::FallingStalactite,
        Source::Freeze,
        Source::HotFloor,
        Source::MobProjectile,
        Source::Spear,
        Source::Spit,
        Source::Stalagmite,
        Source::UnattributedFireball,
        Source::Arrow,
        Source::Cactus,
        Source::SweetBerryBush,
        Source::Drown,
        Source::DryOut,
        Source::Explosion,
        Source::Fall,
        Source::Generic,
        Source::GenericKill,
        Source::InFire,
        Source::Campfire,
        Source::InWall,
        Source::Lava,
        Source::OnFire,
        Source::OutOfWorld,
        Source::MobAttack,
        Source::MobAttackNoAggro,
        Source::Sting,
        Source::PlayerExplosion,
        Source::PlayerAttack,
        Source::MaceSmash,
        Source::Starve,
        Source::Thorns,
        Source::Magic,
        Source::IndirectMagic,
        Source::WitherSkull,
        Source::SonicBoom,
        Source::Wither,
        Source::Trident,
        Source::Fireworks,
        Source::OutsideBorder,
        Source::Lightning,
        Source::Fireball,
        Source::LargeFireball,
        Source::FlyIntoWall,
        Source::Thrown,
        Source::WindCharge,
    ];

    ALL.iter().find(|source| source.type_id() == type_id).map_or(-1, |source| damage_cause(*source))
}

/// ActorDamageCause: Mojang r/26_u1, SharedTypes::Legacy::ActorDamageCause.
/// Несовпадающие виды Java сведены к ближайшей причине [проверить].
pub fn damage_cause(source: Source) -> i32 {
    match source {
        Source::MobAttack | Source::Sting | Source::PlayerAttack => 2,
        Source::MobAttackNoAggro => 30,
        Source::MaceSmash => 34,
        Source::Cactus | Source::SweetBerryBush => 1,
        Source::Arrow | Source::Trident | Source::Thrown | Source::WindCharge |
        Source::MobProjectile | Source::Spear | Source::Spit => 3,
        Source::InWall | Source::Cramming => 4,
        Source::Fall | Source::EnderPearl => 5,
        Source::InFire => 6,
        Source::Campfire => 32,
        Source::OnFire => 7,
        Source::Lava => 8,
        Source::Drown => 9,
        Source::Explosion | Source::BadRespawnPoint => 10,
        Source::PlayerExplosion | Source::Fireball | Source::LargeFireball | Source::UnattributedFireball => 11,
        Source::OutOfWorld | Source::OutsideBorder => 12,
        Source::Magic | Source::IndirectMagic | Source::WitherSkull | Source::DragonBreath => 14,
        Source::Wither => 15,
        Source::Starve => 16,
        Source::FallingAnvil => 17,
        Source::Thorns => 18,
        Source::FallingBlock => 19,
        Source::FlyIntoWall => 21,
        Source::HotFloor => 22,
        Source::Fireworks => 23,
        Source::Lightning => 24,
        Source::Freeze => 27,
        Source::FallingStalactite => 28,
        Source::Stalagmite => 29,
        Source::SonicBoom => 31,
        Source::DryOut | Source::Generic | Source::GenericKill => -1,
    }
}

pub fn death_event(runtime_id: u64) -> Vec<u8> {
    actor_event(runtime_id, EVENT_DEATH)
}

/// DeathInfo: ключ сообщения и его подстановки (имя игрока).
pub fn death_info(source: Source, name: &str) -> Vec<u8> {
    let mut out = Out::packet(DEATH_INFO);

    out.string(source.death_key()).varint(1).string(name);
    out.bytes
}

/// Respawn: место (глаза), состояние, номер сущности — в порядке
/// порядковых номеров полей документации (Position, State, Runtime Id).
pub fn respawn_packet(position: (f32, f32, f32), state: u8, runtime_id: u64) -> Vec<u8> {
    let mut out = Out::packet(RESPAWN);

    out.vec3f(position.0, position.1, position.2).u8(state).varint64(runtime_id);
    out.bytes
}

/// Все атрибуты игрока: скорость шага, здоровье, еда, насыщение, изнурение.
/// Записи: наименьшее, наибольшее, текущее, и те же три «по умолчанию».
pub fn attributes(runtime_id: u64, vit: &Vitals) -> Vec<u8> {
    let list = entries(vit);

    write_entries(runtime_id, &list.iter().collect::<Vec<_>>())
}

/// Атрибут пакета Update Attributes: имя, наибольшее, текущее, «по умолчанию»
/// и модификаторы (id, имя, величина, операция, операнд).
struct Entry {
    name: &'static str,
    max: f32,
    value: f32,
    default_max: f32,
    default: f32,
    modifiers: Vec<(&'static str, &'static str, f32, i32, i32)>,
}

const HEALTH_BOOST_ID: &str = "5d6f0ba2-1186-46ac-b896-c61c5cee99cc";

/// Предел впитывания у BDS — 16 (чёрный ящик br-record, absorption_xp:
/// `absorption=12[0,16]` при Absorption III).
const ABSORPTION_MAX: f32 = 16.0;

fn entries(vit: &Vitals) -> [Entry; 7] {
    let (level, bar) = survival::level_of(vit.xp);
    let plain = |name, max, value, default_max, default| Entry { name, max, value, default_max, default, modifiers: Vec::new() };
    // Здоровье сверх 20 от Health Boost у BDS — модификатор «HealthBoost» (операция 0, операнд 1).
    let mut health = plain("minecraft:health", vit.max_health, vit.health, survival::MAX_HEALTH, survival::MAX_HEALTH);

    if vit.max_health != survival::MAX_HEALTH {
        health.modifiers.push((HEALTH_BOOST_ID, "HealthBoost", vit.max_health - survival::MAX_HEALTH, 0, 1));
    }

    [
        health,
        plain("minecraft:player.hunger", 20.0, vit.food as f32, 20.0, 20.0),
        plain("minecraft:player.saturation", 20.0, vit.saturation, 20.0, 5.0),
        plain("minecraft:player.exhaustion", 5.0, vit.exhaustion.min(5.0), 5.0, 0.0),
        plain("minecraft:absorption", ABSORPTION_MAX.max(vit.absorption), vit.absorption, ABSORPTION_MAX, 0.0),
        plain("minecraft:player.level", 24791.0, level as f32, 24791.0, 0.0),
        plain("minecraft:player.experience", 1.0, bar, 1.0, 0.0),
    ]
}

fn write_entries(runtime_id: u64, list: &[&Entry]) -> Vec<u8> {
    let mut out = Out::packet(session::UPDATE_ATTRIBUTES);

    out.varint64(runtime_id).varint(list.len() as u32);

    for entry in list {
        out.lf32(0.0).lf32(entry.max).lf32(entry.value).lf32(0.0).lf32(entry.default_max).lf32(entry.default)
            .string(entry.name).varint(entry.modifiers.len() as u32);

        for (id, name, amount, operation, operand) in &entry.modifiers {
            out.string(id).string(name).lf32(*amount).li32(*operation).li32(*operand).bool(false);
        }
    }

    out.varint64(0); // такт
    out.bytes
}

/// Только изменившиеся с прошлой отправки: у BDS здоровье, голод, насыщение,
/// впитывание, опыт и уровень уходят каждое отдельно (br-record: hunger,
/// absorption_xp); `None` — добавить нечего. Без прошлой отправки — все.
fn attributes_since(runtime_id: u64, vit: &Vitals, prev: Option<&Sent>) -> Option<Vec<u8>> {
    let Some(prev) = prev else { return Some(attributes(runtime_id, vit)); };
    let list = entries(vit);
    let (level, bar) = survival::level_of(vit.xp);
    let (old_level, old_bar) = survival::level_of(prev.xp);
    let changed = [
        vit.health != prev.health || vit.max_health != prev.max_health,
        vit.food != prev.food,
        vit.saturation != prev.saturation,
        false, // изнурение клиент считает сам
        vit.absorption != prev.absorption,
        level != old_level,
        bar != old_bar || level != old_level,
    ];
    let picked: Vec<&Entry> = list.iter().zip(changed).filter(|(_, yes)| *yes).map(|(entry, _)| entry).collect();

    if picked.is_empty() { None } else { Some(write_entries(runtime_id, &picked)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sources_translate_by_type_without_a_missing_variant() {
        // Отдельные Java варианты большого/малого fireball имеют один id.
        let sources = [Source::Freeze, Source::EnderPearl, Source::WindCharge, Source::SonicBoom,
            Source::Thorns, Source::IndirectMagic, Source::MobAttackNoAggro, Source::FallingAnvil,
            Source::Stalagmite, Source::FallingStalactite, Source::MaceSmash, Source::Campfire];
        for (source, cause) in sources.into_iter().zip([27, 5, 3, 31, 18, 14, 30, 17, 29, 28, 34, 32]) {
            assert_eq!(damage_cause(source), cause);
            assert_eq!(damage_cause_of_type(source.type_id()), cause);
        }
    }

    #[test]
    fn attributes_include_exhaustion_absorption_maximum_and_experience() {
        let mut vit = Vitals::new(); vit.max_health = 40.0; vit.health = 32.0; vit.absorption = 8.0; vit.xp = 100; vit.exhaustion = 3.0;
        let bytes = attributes(300, &vit);
        let (id, body) = crate::bedrock::codec::packet_id(&bytes).unwrap();
        let mut r = crate::bedrock::codec::In::new(body);
        assert_eq!(id, session::UPDATE_ATTRIBUTES); assert_eq!((r.varint64(), r.varint()), (Some(300), Some(7)));
        let mut entries = std::collections::HashMap::new();
        for _ in 0..7 {
            assert_eq!(r.lf32(), Some(0.0)); let max = r.lf32().unwrap(); let value = r.lf32().unwrap();
            for _ in 0..3 { assert!(r.lf32().unwrap().is_finite()); }
            entries.insert(r.string().unwrap(), (max, value));
            for _ in 0..r.varint().unwrap() { r.string().unwrap(); r.string().unwrap(); r.lf32().unwrap(); r.li32().unwrap(); r.li32().unwrap(); r.bool().unwrap(); }
        }
        assert_eq!((r.varint64(), r.remaining()), (Some(0), 0));
        assert_eq!(entries["minecraft:health"], (40.0, 32.0));
        assert_eq!(entries["minecraft:absorption"].1, 8.0);
        assert_eq!(entries["minecraft:player.exhaustion"].1, 3.0);
        assert_eq!(entries["minecraft:player.level"].1, survival::level_of(100).0 as f32);
        assert_eq!(entries["minecraft:player.experience"].1, survival::level_of(100).1);
        let mut health = Health::new(None); health.tick(1, false, true, survival::NORMAL, 64.0);
        // BDS не шлёт изнурение (клиент считает его сам) и не шлёт неизменившееся.
        health.vit.exhaustion = 1.0;
        assert!(health.tick(1, false, true, survival::NORMAL, 64.0).packets.is_empty());
        health.vit.food = 17;
        let packets = health.tick(1, false, true, survival::NORMAL, 64.0).packets;
        assert_eq!(packets.len(), 1);
        let (_, body) = crate::bedrock::codec::packet_id(&packets[0]).unwrap();
        let mut r = crate::bedrock::codec::In::new(body);
        assert_eq!((r.varint64(), r.varint()), (Some(1), Some(1)));
        for _ in 0..6 { r.lf32().unwrap(); }
        assert_eq!(r.string().as_deref(), Some("minecraft:player.hunger"));
    }

    #[test]
    fn poison_and_wither_damage_are_not_discarded_and_creative_effects_expire() {
        for kind in [survival::EffectKind::Poison, survival::EffectKind::Wither] {
            let mut health = Health::new(None);
            health.vit.give_effect_shown(survival::Effect { kind, amplifier: 0, ticks_left: 100, ambient: false, visible: true, show_icon: true });
            health.vit.food = 0; health.vit.saturation = 0.0;
            let mut hurts = Vec::new();
            for _ in 0..100 { hurts.extend(health.tick(1, true, false, survival::NORMAL, 64.0).hurts); }
            assert!(!hurts.is_empty(), "{kind:?}");
            assert!(health.vit.health < 20.0);
        }
        let mut health = Health::new(None);
        health.vit.give_effect_shown(survival::Effect { kind: survival::EffectKind::Speed, amplifier: 0, ticks_left: 1, ambient: true, visible: false, show_icon: false });
        health.tick(1, false, true, survival::NORMAL, 64.0);
        assert!(health.vit.effects.is_empty());
    }

    #[test]
    fn lethal_hit_consumes_one_totem_but_void_does_not() {
        let mut inventory = crate::inventory::Inventory::new();
        let totem = crate::blocks::item_named("totem_of_undying").unwrap();
        inventory.set(crate::inventory::OFFHAND, Some(crate::inventory::Stack::new(totem, 1)));
        let mut health = Health::new(None); health.hurt(100.0, Source::SonicBoom);
        let rules = || Rules { mortal: true, immune: false, difficulty: survival::NORMAL, feet_y: 64.0, natural_regeneration: false };
        let out = health.step(7, rules(), &mut inventory, &crate::attributes::Attributes::player());
        assert!(out.died.is_none()); assert!(!health.vit.dead); assert!(health.vit.health > 0.0);
        assert_eq!(out.slots, [crate::inventory::OFFHAND]); assert!(inventory.slot(crate::inventory::OFFHAND).is_none());
        assert!(out.packets.contains(&actor_event(7, 65))); assert!(health.vit.has_effect(survival::EffectKind::FireResistance));
        inventory.set(crate::inventory::OFFHAND, Some(crate::inventory::Stack::new(totem, 1)));
        health.vit.invulnerable = 0;
        health.hurt(100.0, Source::OutOfWorld);
        assert!(health.step(7, rules(), &mut inventory, &crate::attributes::Attributes::player()).died.is_some());
        assert!(inventory.slot(crate::inventory::OFFHAND).is_some());
    }

    #[test]
    fn enchanting_seed_is_initialized_restored_and_survives_respawn() {
        let health = Health::new(None); assert_ne!(health.vit.xp_seed, 0);
        let saved = health.vit.saved();
        let mut restored = Health::new(Some(&saved));
        assert_eq!(restored.vit.xp_seed, health.vit.xp_seed);
        restored.respawned(); assert_eq!(restored.vit.xp_seed, health.vit.xp_seed);
    }

    #[test]
    fn final_pvp_damage_is_not_scaled_twice_or_armored_twice() {
        let mut health = Health::new(None); let mut inventory = crate::inventory::Inventory::new();
        let mut attrs = crate::attributes::Attributes::player(); attrs.set_base(crate::attributes::ARMOR, 20.0);
        health.hurt_final(6.0, Source::Arrow);
        health.step(1, Rules { mortal: true, immune: false, difficulty: survival::EASY, feet_y: 64.0, natural_regeneration: false }, &mut inventory, &attrs);
        assert_eq!(health.vit.health, 14.0);
    }

    #[test]
    fn hurt_events_carry_the_damage_cause() {
        // Снято с BDS: удар существа — 2, команда `damage` — −1, горение — 7.
        assert_eq!(damage_cause(Source::PlayerAttack), 2);
        assert_eq!(damage_cause(Source::MobAttack), 2);
        assert_eq!(damage_cause(Source::Generic), -1);
        assert_eq!(damage_cause(Source::OnFire), 7);
        assert_eq!(damage_cause_of_type(Source::PlayerAttack.type_id()), 2);
        assert_eq!(damage_cause_of_type(Source::Fall.type_id()), 5);
        assert_eq!(damage_cause_of_type(-5), -1);

        let packet = hurt_event(7, Source::PlayerAttack);
        let (id, body) = crate::bedrock::codec::packet_id(&packet).expect("пакет");
        let mut reader = crate::bedrock::codec::In::new(body);

        assert_eq!(id, ACTOR_EVENT);
        assert_eq!((reader.varint64(), reader.u8(), reader.zigzag32()), (Some(7), Some(EVENT_HURT), Some(2)));
    }

    #[test]
    fn a_fall_hurts_once_on_landing() {
        let mut health = Health::new(None);

        health.moved(-8.0, false, 0.0, false, false, 1.0);
        health.moved(0.0, true, 0.0, false, false, 1.0);

        let out = health.tick(1, true, false, survival::NORMAL, 64.0);

        assert_eq!(health.vit.health, 15.0); // ceil(8 − 3)
        assert_eq!(out.packets.len(), 2); // событие удара и атрибуты
        assert!(out.died.is_none());
    }

    #[test]
    fn the_void_kills_a_survival_player_and_hurts_a_creative_one() {
        let mut survivor = Health::new(None);
        let mut out = survivor.tick(1, true, false, survival::NORMAL, -200.0);

        assert!(out.died.is_none() && survivor.vit.health == 16.0);

        for _ in 0..80 {
            if out.died.is_some() {
                break;
            }

            out = survivor.tick(1, true, false, survival::NORMAL, -200.0);
        }

        assert_eq!(out.died, Some(Source::OutOfWorld));

        let mut flyer = Health::new(None);

        flyer.tick(1, false, true, survival::NORMAL, -200.0);
        assert_eq!(flyer.vit.health, 16.0);

        flyer.hurt(5.0, Source::Lava);
        flyer.tick(1, false, true, survival::NORMAL, 64.0);
        assert_eq!(flyer.vit.health, 16.0); // лава творческого не берёт
    }

    #[test]
    fn attributes_are_sent_once_and_again_on_change() {
        let mut health = Health::new(None);

        assert_eq!(health.tick(1, true, false, survival::NORMAL, 64.0).packets.len(), 1);
        assert!(health.tick(1, true, false, survival::NORMAL, 64.0).packets.is_empty());

        health.vit.food = 10;
        assert_eq!(health.tick(1, true, false, survival::NORMAL, 64.0).packets.len(), 1);
    }

    #[test]
    fn packets_have_the_documented_layout() {
        // ActorEvent: номер сущности (varuint64), событие (byte), данные (varint32 со знаком).
        assert_eq!(actor_event(5, EVENT_HURT), [ACTOR_EVENT as u8, 5, 2, 0]);
        assert_eq!(death_event(5), [ACTOR_EVENT as u8, 5, 3, 0]);

        // Respawn: три float, состояние, номер сущности.
        let packet = respawn_packet((1.0, 2.0, 3.0), STATE_READY, 7);

        assert_eq!(packet.len(), 2 + 12 + 1 + 1 - 1);
        assert_eq!(packet[packet.len() - 2], STATE_READY);
        assert_eq!(packet[packet.len() - 1], 7);
    }

    #[test]
    fn hunger_starves_a_player_without_food() {
        let mut health = Health::new(None);

        health.vit.food = 0;
        health.vit.saturation = 0.0;

        let mut hurt = 0;

        for _ in 0..200 {
            if health.tick(1, true, false, survival::NORMAL, 64.0).packets.iter().any(|p| p[..2] == [ACTOR_EVENT as u8, 1]) {
                hurt += 1;
            }
        }

        assert!(hurt >= 2, "{hurt}");
    }
    #[test]
    fn tipped_arrow_resistance_applies_after_impact() {
        let mut health = Health::new(None);
        let effect = survival::Effect { kind: survival::EffectKind::Resistance, amplifier: 2, ticks_left: 100, ambient: false, visible: true, show_icon: true };
        health.hurt_with_effects(10.0,Source::Arrow,vec![effect]);
        health.tick(1,true,false,survival::NORMAL,64.0);
        assert_eq!(health.vit.health,10.0);
        assert!(health.vit.effects.iter().any(|e| e.kind == effect.kind));
    }

}
