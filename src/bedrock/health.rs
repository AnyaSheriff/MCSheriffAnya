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
}

/// Здоровье игрока Bedrock.
pub struct Health {
    pub vit: Vitals,
    hits: Vec<(f32, Source)>,
    sent: Option<Sent>,
}

/// Исход такта.
pub struct Outcome {
    pub packets: Vec<Vec<u8>>,
    pub died: Option<Source>,
    /// Виды урона, которые прошли в этот такт: остальным игрокам — «ранен».
    pub hurts: Vec<Source>,
}

impl Health {
    pub fn new(saved: Option<&crate::playerdata::SavedVitals>) -> Self {
        Self { vit: saved.map(Vitals::restored).unwrap_or_default(), hits: Vec::new(), sent: None }
    }

    /// Возрождение: всё заново.
    pub fn respawned(&mut self) {
        self.vit.respawned();
        self.hits.clear();
        self.sent = None;
    }

    /// Удар, который разберётся в ближайшем такте.
    pub fn hurt(&mut self, amount: f32, source: Source) {
        self.hits.push((amount, source));
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
            self.hits.push((damage, Source::Fall));
        }
    }

    /// Такт: удары, бездна, голод, лечение; изменившиеся атрибуты — клиенту.
    /// `mortal` — выживание или приключение, `immune` — творческий или
    /// наблюдатель (бьёт только бездна).
    pub fn tick(&mut self, runtime_id: u64, mortal: bool, immune: bool, difficulty: u8, feet_y: f32) -> Outcome {
        let mut out = Outcome { packets: Vec::new(), died: None, hurts: Vec::new() };

        if self.vit.dead {
            self.hits.clear();

            return out;
        }

        if feet_y < VOID_BELOW {
            self.hits.push((4.0, Source::OutOfWorld));
        }

        for (mut amount, source) in std::mem::take(&mut self.hits) {
            if source.scales_with_difficulty() {
                amount = survival::scaled(amount, difficulty);
            }

            if let Some(hit) = self.vit.hurt(amount, source, immune) {
                out.packets.push(hurt_event(runtime_id, source));
                out.hurts.push(source);

                if hit.died {
                    out.died = Some(source);
                    break;
                }
            }
        }

        if out.died.is_none() && mortal {
            self.vit.age = self.vit.age.wrapping_add(1);

            if difficulty == survival::PEACEFUL {
                if self.vit.health < survival::MAX_HEALTH && self.vit.age.is_multiple_of(20) {
                    self.vit.heal(1.0);
                }

                if self.vit.food < 20 && self.vit.age.is_multiple_of(10) {
                    self.vit.food += 1;
                }
            }

            self.vit.effects_tick();

            if let Some(amount) = self.vit.food_tick(difficulty, true)
                && let Some(hit) = self.vit.hurt(amount, Source::Starve, false)
            {
                out.packets.push(hurt_event(runtime_id, Source::Starve));
                out.hurts.push(Source::Starve);

                if hit.died {
                    out.died = Some(Source::Starve);
                }
            }
        }

        self.vit.tick_invulnerable();

        let now = Sent { health: self.vit.health, food: self.vit.food, saturation: self.vit.saturation };

        if self.sent != Some(now) {
            self.sent = Some(now);
            out.packets.push(attributes(runtime_id, &self.vit));
        }

        out
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
    const ALL: [Source; 27] = [
        Source::Arrow,
        Source::Cactus,
        Source::SweetBerryBush,
        Source::Drown,
        Source::Explosion,
        Source::Fall,
        Source::Generic,
        Source::GenericKill,
        Source::InFire,
        Source::InWall,
        Source::Lava,
        Source::OnFire,
        Source::OutOfWorld,
        Source::MobAttack,
        Source::PlayerExplosion,
        Source::PlayerAttack,
        Source::MaceSmash,
        Source::Starve,
        Source::Thorns,
        Source::Magic,
        Source::Wither,
        Source::Trident,
        Source::Fireworks,
        Source::OutsideBorder,
        Source::Lightning,
        Source::Fireball,
        Source::FlyIntoWall,
    ];

    ALL.iter().find(|source| source.type_id() == type_id).map_or(-1, |source| damage_cause(*source))
}

/// Причина урона Bedrock (ActorDamageCause) для вида урона. Сверено по BDS:
/// удар существа 2, огонь горения 7, без причины −1; остальные номера —
/// по порядку перечисления [проверить].
pub fn damage_cause(source: Source) -> i32 {
    match source {
        Source::MobAttack | Source::PlayerAttack | Source::MaceSmash => 2,
        Source::Cactus | Source::SweetBerryBush => 1,
        Source::Arrow | Source::Trident | Source::Thrown => 3,
        Source::InWall => 4,
        Source::Fall => 5,
        Source::InFire => 6,
        Source::OnFire => 7,
        Source::Lava => 8,
        Source::Drown => 9,
        Source::Explosion | Source::PlayerExplosion | Source::Fireworks | Source::Fireball => 11,
        Source::OutOfWorld | Source::OutsideBorder => 12,
        Source::Magic => 14,
        Source::Wither => 15,
        Source::Starve => 16,
        Source::Thorns => 18,
        Source::Lightning => 24,
        // [Bedrock: потом] причина «о стену на элитрах» не сверена.
        Source::Generic | Source::GenericKill | Source::FlyIntoWall => -1,
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
    let list: [(&str, f32, f32, f32, f32); 5] = [
        ("minecraft:movement", f32::MAX, 0.1, f32::MAX, 0.1),
        ("minecraft:health", survival::MAX_HEALTH, vit.health, survival::MAX_HEALTH, survival::MAX_HEALTH),
        ("minecraft:player.hunger", 20.0, vit.food as f32, 20.0, 20.0),
        ("minecraft:player.saturation", 20.0, vit.saturation, 20.0, 5.0),
        ("minecraft:player.exhaustion", 5.0, vit.exhaustion.min(5.0), 5.0, 0.0),
    ];
    let mut out = Out::packet(session::UPDATE_ATTRIBUTES);

    out.varint64(runtime_id).varint(list.len() as u32);

    for (name, max, value, default_max, default) in list {
        out.lf32(0.0).lf32(max).lf32(value).lf32(0.0).lf32(default_max).lf32(default).string(name).varint(0);
    }

    out.varint64(0); // такт
    out.bytes
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
