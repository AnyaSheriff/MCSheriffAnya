//! Летящее и падающее для клиента Bedrock: брошенные зелья, оки Края,
//! ракеты фейерверка, падающие блоки и шарики опыта (Add Actor с их именами
//! Bedrock и данными сущности).
//!
//! Сверено по официальному BDS 1.26.13.1 (наблюдение по сети, запись
//! tmp/br-entities/bds-proj.jsonl, `summon` оператором):
//!
//! - `minecraft:splash_potion` — флаги «невидим», «сталкивается», «падает»;
//!   `potion_aux_value` (ключ 36, short) — вид зелья для цвета; поворот 0;
//! - `minecraft:falling_block` — флаги «сталкивается», «падает»; `variant`
//!   (ключ 2) — номер состояния блока; скорость как у Java;
//! - `minecraft:fireworks_rocket` — флаги «сталкивается», «падает»; при
//!   взрыве — Actor Event `firework_particles` (25) и удаление;
//! - `minecraft:xp_orb` — `experience_value` (ключ 15, int), со скоростью.
//!
//! Лишние значения по умолчанию (границы, звуки) клиент берёт у себя.

use super::codec::Out;
use super::mobdata::{self, Value};

const ADD_ACTOR: u32 = 0x0d;

/// Ключи данных сущности.
const KEY_FLAGS: u32 = 0;
const KEY_VARIANT: u32 = 2;
const KEY_EXPERIENCE_VALUE: u32 = 15;
const KEY_POTION_AUX_VALUE: u32 = 36;

/// Биты флагов (MetadataFlags1): невидим, сталкивается, падает.
const FLAG_INVISIBLE: u32 = 5;
const FLAG_HAS_COLLISION: u32 = 48;
const FLAG_AFFECTED_BY_GRAVITY: u32 = 49;

/// Actor Event «фейерверк взорвался» (`firework_particles`, запись BDS).
pub const EVENT_FIREWORK_PARTICLES: u8 = 25;

/// Одна показываемая сущность: номер, имя Bedrock, место с поворотом
/// (x, y, z, рыскание, наклон, голова), скорость и данные.
#[derive(Clone, Debug, PartialEq)]
pub struct Thing {
    pub id: i32,
    pub name: &'static str,
    pub place: [f32; 6],
    pub motion: [f32; 3],
    pub data: Vec<(u32, Value)>,
}

fn plain_flags(extra: &[u32]) -> Value {
    let mut bits: i64 = (1 << FLAG_HAS_COLLISION) | (1 << FLAG_AFFECTED_BY_GRAVITY);

    for bit in extra {
        bits |= 1 << bit;
    }

    Value::Long(bits)
}

/// Номер зелья Bedrock для цвета снаряда (`potion_aux_value`) по имени зелья
/// Java. Порядок Bedrock: water 0, mundane 1, long mundane 2, thick 3,
/// awkward 4, night vision 5 … [проверить] живым клиентом.
pub fn bedrock_potion(name: &str) -> i16 {
    const ORDER: [&str; 43] = [
        "water", "mundane", "long_mundane", "thick", "awkward", "night_vision", "long_night_vision", "invisibility", "long_invisibility",
        "leaping", "long_leaping", "strong_leaping", "fire_resistance", "long_fire_resistance", "swiftness", "long_swiftness",
        "strong_swiftness", "slowness", "long_slowness", "water_breathing", "long_water_breathing", "healing", "strong_healing",
        "harming", "strong_harming", "poison", "long_poison", "strong_poison", "regeneration", "long_regeneration",
        "strong_regeneration", "strength", "long_strength", "strong_strength", "weakness", "long_weakness", "wither", "turtle_master",
        "long_turtle_master", "strong_turtle_master", "slow_falling", "long_slow_falling", "strong_slowness",
    ];

    ORDER.iter().position(|known| *known == name).map_or(0, |index| index as i16)
}

/// Брошенное зелье: взрывное или оседающее.
pub fn potion(id: i32, lingering: bool, potion_name: &str, at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Thing {
    Thing {
        id,
        name: if lingering { "minecraft:lingering_potion" } else { "minecraft:splash_potion" },
        place: [at.0 as f32, at.1 as f32, at.2 as f32, 0.0, 0.0, 0.0],
        motion: [velocity.0 as f32, velocity.1 as f32, velocity.2 as f32],
        data: vec![(KEY_FLAGS, plain_flags(&[FLAG_INVISIBLE])), (KEY_POTION_AUX_VALUE, Value::Short(bedrock_potion(potion_name)))],
    }
}

/// Око Края в полёте. Имя — `minecraft:eye_of_ender_signal` [проверить]:
/// `summon` у BDS такого имени не принял.
pub fn eye(id: i32, at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Thing {
    Thing {
        id,
        name: "minecraft:eye_of_ender_signal",
        place: [at.0 as f32, at.1 as f32, at.2 as f32, 0.0, 0.0, 0.0],
        motion: [velocity.0 as f32, velocity.1 as f32, velocity.2 as f32],
        data: vec![(KEY_FLAGS, plain_flags(&[]))],
    }
}

/// Ракета фейерверка.
pub fn rocket(id: i32, at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Thing {
    Thing {
        id,
        name: "minecraft:fireworks_rocket",
        place: [at.0 as f32, at.1 as f32, at.2 as f32, 0.0, 0.0, 0.0],
        motion: [velocity.0 as f32, velocity.1 as f32, velocity.2 as f32],
        data: vec![(KEY_FLAGS, plain_flags(&[]))],
    }
}

/// Падающий блок (песок, гравий, наковальня…): `block` — номер состояния у
/// клиента Bedrock.
pub fn falling_block(id: i32, block: u32, at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Thing {
    Thing {
        id,
        name: "minecraft:falling_block",
        place: [at.0 as f32, at.1 as f32, at.2 as f32, 0.0, 0.0, 0.0],
        motion: [velocity.0 as f32, velocity.1 as f32, velocity.2 as f32],
        data: vec![(KEY_FLAGS, plain_flags(&[])), (KEY_VARIANT, Value::Int(block as i32))],
    }
}

/// Шарик опыта.
pub fn orb(id: i32, value: i32, at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Thing {
    Thing {
        id,
        name: "minecraft:xp_orb",
        place: [at.0 as f32, at.1 as f32, at.2 as f32, 0.0, 0.0, 0.0],
        motion: [velocity.0 as f32, velocity.1 as f32, velocity.2 as f32],
        data: vec![(KEY_FLAGS, plain_flags(&[])), (KEY_EXPERIENCE_VALUE, Value::Int(value))],
    }
}

/// Add Actor для сущности: без атрибутов, связей и свойств.
pub fn add_packet(thing: &Thing) -> Vec<u8> {
    let [x, y, z, yaw, pitch, head] = thing.place;
    let mut out = Out::packet(ADD_ACTOR);

    out.zigzag64(i64::from(thing.id))
        .varint64(thing.id as u64)
        .string(thing.name)
        .vec3f(x, y, z)
        .vec3f(thing.motion[0], thing.motion[1], thing.motion[2])
        .lf32(pitch)
        .lf32(yaw)
        .lf32(head)
        .lf32(yaw)
        .varint(0); // свойства-атрибуты
    mobdata::write(&mut out, &thing.data);
    out.varint(0).varint(0); // свойства: целые, дробные
    out.varint(0); // связи
    out.bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_splash_potion_matches_the_bds_fields() {
        let thing = potion(7, false, "swiftness", (1.0, 2.0, 3.0), (0.0, 0.1, 0.0));

        assert_eq!(thing.name, "minecraft:splash_potion");
        assert_eq!(thing.data[0], (0, Value::Long((1 << 5) | (1 << 48) | (1 << 49))));
        assert_eq!(thing.data[1], (36, Value::Short(14)));
    }

    #[test]
    fn a_falling_block_carries_its_state_as_variant() {
        let thing = falling_block(3, 1234, (0.5, 70.0, 0.5), (0.0, -0.04, 0.0));

        assert_eq!(thing.data[1], (2, Value::Int(1234)));
    }

    #[test]
    fn an_orb_carries_its_value() {
        assert_eq!(orb(1, 5, (0.0, 0.0, 0.0), (0.0, 0.0, 0.0)).data[1], (15, Value::Int(5)));
    }

    #[test]
    fn add_packet_has_name_motion_and_data() {
        let bytes = add_packet(&rocket(9, (1.0, 2.0, 3.0), (0.0, 0.5, 0.0)));

        assert_eq!(bytes[0], 0x0d);
        assert!(String::from_utf8_lossy(&bytes).contains("minecraft:fireworks_rocket"));
        // Конец пакета: данные (две записи: флаги и ничего больше → одна), свойства, связи.
        assert_eq!(&bytes[bytes.len() - 3..], &[0, 0, 0]);
    }

    #[test]
    fn unknown_potions_fall_back_to_water() {
        assert_eq!(bedrock_potion("nonsense"), 0);
        assert_eq!(bedrock_potion("healing"), 21);
    }
}
