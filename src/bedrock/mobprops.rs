//! Свойства сущностей Bedrock (Sync Actor Property и поле `properties` в Add Actor).
//!
//! Клиент 1.26.13 у свиньи, коровы и курицы считает вариант по
//! `query.property('minecraft:climate_variant')` и `...sound_variant`; без
//! определений при входе он сыплет тысячами ошибок в журнал содержимого
//! («query.property called on an actor without a property component»).
//! Сверено по официальному BDS 1.26.13.1 (наблюдение по сети): при входе после
//! Start Game идёт по одному `sync_entity_property` (0xa5, NBT) на вид, а в
//! Add Actor свойства лежат целыми числами `(индекс, номер значения)` —
//! индекс в порядке списка `properties` из определения.

use super::codec::Out;
use crate::mob::Kind;

/// Sync Actor Property (протокол 944: `sync_entity_property` — 0xa5).
const SYNC_ACTOR_PROPERTY: u32 = 0xa5;

/// Теги сетевого NBT.
const TAG_INT: u8 = 3;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;

/// Тип свойства в определении: 0 — целое с границами, 1 — дробное,
/// 2 — флаг, 3 — перечисление (так у BDS).
const TYPE_ENUM: i32 = 3;

/// Вид значения свойства в определении BDS: перечисление (3), флаг (2), целое с границами (0).
enum Shape {
    Enum(&'static [&'static str]),
    Flag,
    Int(i32, i32),
}

/// Одно свойство: имя и вид значения.
struct EnumProperty {
    name: &'static str,
    shape: Shape,
}

const fn en(name: &'static str, values: &'static [&'static str]) -> EnumProperty {
    EnumProperty { name, shape: Shape::Enum(values) }
}

const CLIMATE: EnumProperty = en("minecraft:climate_variant", &["temperate", "warm", "cold"]);

/// Свойства вида в порядке индексов; пустой список — свойств нет.
fn properties(kind: Kind) -> &'static [EnumProperty] {
    const PIG: [EnumProperty; 2] = [CLIMATE, en("minecraft:sound_variant", &["default", "big", "mini"])];
    const COW: [EnumProperty; 2] = [CLIMATE, en("minecraft:sound_variant", &["default", "moody"])];
    const CHICKEN: [EnumProperty; 2] = [CLIMATE, en("minecraft:sound_variant", &["default", "picky"])];
    // Определения остальных видов — как их шлёт BDS 1.26.13.1 при входе (запись br-cases.js).
    const BEE: [EnumProperty; 1] = [EnumProperty { name: "minecraft:has_nectar", shape: Shape::Flag }];
    const GHAST: [EnumProperty; 1] = [EnumProperty { name: "minecraft:can_move", shape: Shape::Flag }];
    const ARMADILLO: [EnumProperty; 1] = [en("minecraft:armadillo_state", &["unrolled", "rolled_up", "rolled_up_peeking", "rolled_up_relaxing", "rolled_up_unrolling"])];
    const WOLF: [EnumProperty; 1] = [en("minecraft:sound_variant", &["default", "big", "cute", "grumpy", "mad", "puglin", "sad"])];
    const CAT: [EnumProperty; 1] = [en("minecraft:sound_variant", &["default", "royal"])];
    const CREAKING: [EnumProperty; 2] = [en("minecraft:creaking_state", &["neutral", "hostile_observed", "hostile_unobserved", "twitching", "crumbling"]), EnumProperty { name: "minecraft:creaking_swaying_ticks", shape: Shape::Int(0, 6) }];
    const COPPER: [EnumProperty; 3] = [en("minecraft:chest_interaction", &["none", "take", "take_fail", "put", "put_fail"]), EnumProperty { name: "minecraft:has_flower", shape: Shape::Flag }, en("minecraft:oxidation_level", &["unoxidized", "exposed", "weathered", "oxidized"])];

    match kind {
        Kind::Pig => &PIG,
        Kind::Cow => &COW,
        Kind::Chicken => &CHICKEN,
        Kind::Bee => &BEE,
        Kind::HappyGhast => &GHAST,
        Kind::Armadillo => &ARMADILLO,
        Kind::Wolf => &WOLF,
        Kind::Cat => &CAT,
        Kind::Creaking => &CREAKING,
        Kind::CopperGolem => &COPPER,
        _ => &[],
    }
}

/// Сетевой NBT: строка — длина varint и байты, число — zigzag varint.
fn name(out: &mut Out, tag: u8, key: &str) {
    out.u8(tag).string(key);
}

fn definition(kind: Kind, list: &[EnumProperty]) -> Vec<u8> {
    let mut out = Out::packet(SYNC_ACTOR_PROPERTY);

    out.u8(TAG_COMPOUND).string("");
    name(&mut out, TAG_LIST, "properties");
    out.u8(TAG_COMPOUND).zigzag32(list.len() as i32); // длина списка — int, то есть zigzag

    // Ключи компаунда — по алфавиту, как у BDS: enum | max, min | name, type.
    for property in list {
        match &property.shape {
            Shape::Enum(values) => {
                name(&mut out, TAG_LIST, "enum");
                out.u8(TAG_STRING).zigzag32(values.len() as i32);

                for value in *values {
                    out.string(value);
                }
            }
            Shape::Int(min, max) => {
                name(&mut out, TAG_INT, "max");
                out.zigzag32(*max);
                name(&mut out, TAG_INT, "min");
                out.zigzag32(*min);
            }
            Shape::Flag => {}
        }

        name(&mut out, TAG_STRING, "name");
        out.string(property.name);
        name(&mut out, TAG_INT, "type");
        out.zigzag32(match property.shape {
            Shape::Enum(_) => TYPE_ENUM,
            Shape::Flag => 2,
            Shape::Int(..) => 0,
        });
        out.u8(0); // конец компаунда свойства
    }

    name(&mut out, TAG_STRING, "type");
    out.string(kind.bedrock_name());
    out.u8(0); // конец корня
    out.bytes
}

/// Определения для всех видов с свойствами, которые мы показываем.
pub fn definitions() -> Vec<Vec<u8>> {
    ALL.iter().filter(|kind| !properties(**kind).is_empty()).map(|kind| definition(*kind, properties(*kind))).collect()
}

const ALL: [Kind; 10] = [Kind::Pig, Kind::Chicken, Kind::Cow, Kind::Bee, Kind::HappyGhast, Kind::Armadillo, Kind::Wolf, Kind::Cat, Kind::Creaking, Kind::CopperGolem];

/// Значения свойств для Add Actor: `(индекс, номер значения)`.
/// Климат пока всегда умеренный (у Java вариант зависит от биома) и голос
/// обычный [проверить]: у BDS голос случайный, у нас такого состояния нет.
pub fn values(kind: Kind) -> Vec<(u32, i32)> {
    // Начальные значения: у BDS все нули, кроме `can_move` счастливого гаста.
    (0..properties(kind).len() as u32).map(|index| (index, i32::from(kind == Kind::HappyGhast))).collect()
}

/// Поле `properties` Add Actor: целые, потом дробные.
pub fn write_values(out: &mut Out, kind: Kind) {
    let ints = values(kind);

    out.varint(ints.len() as u32);

    for (index, value) in ints {
        out.varint(index).zigzag32(value);
    }

    out.varint(0); // дробных нет
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Определение свиньи побайтово как у BDS: компаунд, список из двух
    /// свойств, у каждого enum/name/type, в конце имя вида.
    #[test]
    fn pig_definition_matches_bds_structure() {
        let packet = definition(Kind::Pig, properties(Kind::Pig));

        assert_eq!(&packet[..3], &[0xa5, 1, 10]); // id — varint из двух байт
        let text = String::from_utf8_lossy(&packet).to_string();

        for word in ["properties", "minecraft:climate_variant", "temperate", "cold", "minecraft:sound_variant", "mini", "minecraft:pig"] {
            assert!(text.contains(word), "нет {word}");
        }

        assert_eq!(*packet.last().expect("конец"), 0);
    }

    /// Определения BDS: флаг (type 2) без enum, целое (type 0) с max и min по алфавиту, перечисление (type 3).
    #[test]
    fn flag_int_and_enum_definitions_follow_bds() {
        let bee = String::from_utf8_lossy(&definition(Kind::Bee, properties(Kind::Bee))).to_string();

        assert!(bee.contains("minecraft:has_nectar") && !bee.contains("enum"));

        let creaking = definition(Kind::Creaking, properties(Kind::Creaking));
        let text = String::from_utf8_lossy(&creaking).to_string();

        assert!(text.contains("creaking_state") && text.contains("twitching") && text.contains("creaking_swaying_ticks"));
        let at = |needle: &[u8]| creaking.windows(needle.len()).position(|w| w == needle).expect("ключ");

        assert!(at(&[TAG_INT, 3, b'm', b'a', b'x']) < at(&[TAG_INT, 3, b'm', b'i', b'n']));
        assert_eq!(values(Kind::Creaking), vec![(0, 0), (1, 0)]);
        assert_eq!(values(Kind::HappyGhast), vec![(0, 1)]);
        assert_eq!(values(Kind::CopperGolem).len(), 3);
    }

    #[test]
    fn only_mobs_with_properties_are_defined() {
        assert_eq!(definitions().len(), 10);
        assert!(properties(Kind::Zombie).is_empty());
        assert!(values(Kind::Sheep).is_empty());
    }

    #[test]
    fn add_actor_values_are_two_ints_for_a_cow() {
        let mut out = Out::packet(0);

        write_values(&mut out, Kind::Cow);
        assert_eq!(&out.bytes[1..], &[2, 0, 0, 1, 0, 0]);

        let mut none = Out::packet(0);

        write_values(&mut none, Kind::Spider);
        assert_eq!(&none.bytes[1..], &[0, 0]);
    }
}
