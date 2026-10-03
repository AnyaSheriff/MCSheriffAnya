//! Данные сущности мобов для клиента Bedrock (поле `metadata` в Add Actor и
//! пакет Set Actor Data): детёныш, цвет овцы, остриженность, любовь,
//! профессия и тип жителя.
//!
//! Сверено по официальному BDS 1.26.13.1 (наблюдение по сети, запись
//! tmp/br-entities/bds-mobs*.jsonl): детёныш — флаг `baby` (бит 11) и
//! `scale` 0,5 (ключ 38); у взрослого `scale` 1; цвет шерсти — `color`
//! (ключ 3, байт, порядок красок как у Java); у жителя `variant` (ключ 2) —
//! профессия, `skin_id` (ключ 104) — тип, у детёныша жителя `max_trade_tier`
//! 0. Лишнего BDS шлёт ещё шестьдесят значений по умолчанию (границы,
//! звуки…): клиент берёт их из собственных определений, поэтому здесь только
//! то, что различает мобов одного вида. Номера ключей и битов — описание
//! протокола minecraft-data (MetadataDictionary, MetadataFlags1).

use super::codec::Out;
use crate::mob::{Kind, Mob};

/// Ключи данных сущности.
const KEY_FLAGS: u32 = 0;
const KEY_VARIANT: u32 = 2;
const KEY_COLOR: u32 = 3;
const KEY_NAMETAG: u32 = 4;
const KEY_SCALE: u32 = 38;
const KEY_ALWAYS_SHOW_NAMETAG: u32 = 81;
const KEY_TRADE_TIER: u32 = 101;
const KEY_MAX_TRADE_TIER: u32 = 102;
const KEY_SKIN_ID: u32 = 104;

/// Виды значений.
const TYPE_BYTE: u32 = 0;
const TYPE_SHORT: u32 = 1;
const TYPE_INT: u32 = 2;
const TYPE_FLOAT: u32 = 3;
const TYPE_STRING: u32 = 4;
const TYPE_LONG: u32 = 7;

/// Биты флагов (MetadataFlags1).
const FLAG_INLOVE: u32 = 7;
const FLAG_BABY: u32 = 11;
const FLAG_CAN_SHOW_NAMETAG: u32 = 14;
const FLAG_CAN_CLIMB: u32 = 19;
const FLAG_WALKER: u32 = 22;
const FLAG_SITTING: u32 = 24;
const FLAG_TAMED: u32 = 28;
const FLAG_SHEARED: u32 = 31;
const FLAG_BREATHING: u32 = 35;
const FLAG_HAS_COLLISION: u32 = 48;
const FLAG_AFFECTED_BY_GRAVITY: u32 = 49;

/// Что клиенту видно у моба: из этого собираются флаги и значения. Сравнением
/// двух таких снимков решается, слать ли Set Actor Data.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub kind: Kind,
    pub baby: bool,
    pub in_love: bool,
    pub sheared: bool,
    pub color: u8,
    pub sitting: bool,
    pub tamed: bool,
    /// Профессия, тип и уровень жителя (номера Java).
    pub profession: u8,
    pub village_type: u8,
    pub level: u8,
    pub name: Option<String>,
}

impl Look {
    pub fn of(mob: &Mob) -> Self {
        Self {
            kind: mob.kind,
            baby: mob.is_baby(),
            in_love: mob.love > 0,
            sheared: mob.sheared,
            color: mob.color,
            // Ни приручения, ни «сидит» у наших мобов нет (волков и кошек
            // в ядре нет).
            sitting: false,
            tamed: false,
            profession: mob.villager.profession,
            village_type: mob.villager.kind,
            level: mob.villager.level,
            name: None,
        }
    }
}

/// Профессия жителя Java (по имени из `villager::PROFESSIONS`) в номер
/// профессии Bedrock (`variant`): farmer — 1, librarian — 5 сняты с BDS
/// (`minecraft:become_farmer`, `become_librarian`), остальные — по порядку
/// вики Bedrock [проверить].
pub fn bedrock_profession(java: u8) -> i32 {
    match crate::villager::PROFESSIONS.get(usize::from(java)).copied().unwrap_or("none") {
        "farmer" => 1,
        "fisherman" => 2,
        "shepherd" => 3,
        "fletcher" => 4,
        "librarian" => 5,
        "cartographer" => 6,
        "cleric" => 7,
        "armorer" => 8,
        "weaponsmith" => 9,
        "toolsmith" => 10,
        "butcher" => 11,
        "leatherworker" => 12,
        "mason" => 13,
        "nitwit" => 14,
        _ => 0,
    }
}

/// Тип жителя по биому (Java: desert 0, jungle 1, plains 2, savanna 3, snow 4,
/// swamp 5, taiga 6) в `skin_id` Bedrock: plains 0, desert 1, jungle 2,
/// остальные те же [проверить].
pub fn bedrock_village_type(java: u8) -> i32 {
    match java {
        0 => 1,
        1 => 2,
        2 => 0,
        other => i32::from(other),
    }
}

/// Значение данных сущности.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Byte(i8),
    Short(i16),
    Int(i32),
    Float(f32),
    Str(String),
    Long(i64),
}

/// Флаги живого моба: то, что BDS ставит всем (лазит, сталкивается, падает,
/// дышит), «ходит» у наземных, и состояния из `look`.
pub fn flags(look: &Look) -> i64 {
    let mut bits: i64 = (1 << FLAG_CAN_CLIMB) | (1 << FLAG_BREATHING) | (1 << FLAG_HAS_COLLISION) | (1 << FLAG_AFFECTED_BY_GRAVITY);
    let mut set = |on: bool, bit: u32| {
        if on {
            bits |= 1 << bit;
        }
    };

    set(!matches!(look.kind, Kind::EndCrystal | Kind::EnderDragon | Kind::ShulkerBullet), FLAG_WALKER);
    set(look.baby, FLAG_BABY);
    set(look.in_love, FLAG_INLOVE);
    set(look.sheared, FLAG_SHEARED);
    set(look.sitting, FLAG_SITTING);
    set(look.tamed, FLAG_TAMED);
    set(look.name.is_some(), FLAG_CAN_SHOW_NAMETAG);

    bits
}

/// Значения данных сущности для моба: ключ и значение.
pub fn entries(look: &Look) -> Vec<(u32, Value)> {
    let mut list = vec![(KEY_FLAGS, Value::Long(flags(look))), (KEY_SCALE, Value::Float(if look.baby { 0.5 } else { 1.0 }))];

    match look.kind {
        Kind::Sheep => list.push((KEY_COLOR, Value::Byte(look.color.min(15) as i8))),
        Kind::Villager => {
            list.push((KEY_VARIANT, Value::Int(bedrock_profession(look.profession))));
            list.push((KEY_SKIN_ID, Value::Int(bedrock_village_type(look.village_type))));
            list.push((KEY_TRADE_TIER, Value::Int(i32::from(look.level.max(1)) - 1)));
            list.push((KEY_MAX_TRADE_TIER, Value::Int(if look.baby { 0 } else { 4 })));
        }
        _ => {}
    }

    if let Some(name) = &look.name {
        list.push((KEY_NAMETAG, Value::Str(name.clone())));
        list.push((KEY_ALWAYS_SHOW_NAMETAG, Value::Byte(1)));
    }

    list
}

/// Пишет словарь: число записей и сами записи.
pub fn write(out: &mut Out, list: &[(u32, Value)]) {
    out.varint(list.len() as u32);

    for (key, value) in list {
        out.varint(*key);

        match value {
            Value::Byte(byte) => out.varint(TYPE_BYTE).u8(*byte as u8),
            Value::Short(short) => out.varint(TYPE_SHORT).li16(*short),
            Value::Int(int) => out.varint(TYPE_INT).zigzag32(*int),
            Value::Float(float) => out.varint(TYPE_FLOAT).lf32(*float),
            Value::Str(text) => out.varint(TYPE_STRING).string(text),
            Value::Long(long) => out.varint(TYPE_LONG).zigzag64(*long),
        };
    }
}

/// Set Actor Data: номер, значения, свойства (целые, дробные), такт.
pub fn set_data_packet(entity_id: i32, list: &[(u32, Value)]) -> Vec<u8> {
    let mut out = Out::packet(super::session::SET_ENTITY_DATA);

    out.varint64(entity_id as u64);
    write(&mut out, list);
    out.varint(0).varint(0); // свойства: целые, дробные
    out.varint64(0); // такт
    out.bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(kind: Kind) -> Look {
        Look { kind, baby: false, in_love: false, sheared: false, color: 0, sitting: false, tamed: false, profession: 0, village_type: 2, level: 1, name: None }
    }

    /// Набор флагов взрослой коровы побайтово как у BDS: лазит, ходит, дышит,
    /// сталкивается, падает (запись bds-mobs3.jsonl: корова после Set Actor Data, 844459294588928).
    #[test]
    fn adult_flags_follow_bds() {
        assert_eq!(flags(&look(Kind::Cow)), 844_459_294_588_928);
    }

    #[test]
    fn a_baby_has_the_flag_and_half_scale() {
        let mut baby = look(Kind::Sheep);

        baby.baby = true;
        assert_ne!(flags(&baby) & (1 << 11), 0);
        assert!(entries(&baby).contains(&(KEY_SCALE, Value::Float(0.5))));
        assert!(entries(&look(Kind::Sheep)).contains(&(KEY_SCALE, Value::Float(1.0))));
    }

    #[test]
    fn sheep_carry_colour_and_shearing() {
        let mut sheep = look(Kind::Sheep);

        sheep.color = 14;
        sheep.sheared = true;
        assert!(entries(&sheep).contains(&(KEY_COLOR, Value::Byte(14))));
        assert_ne!(flags(&sheep) & (1 << 31), 0);
        assert!(!entries(&look(Kind::Cow)).iter().any(|(key, _)| *key == KEY_COLOR));
    }

    #[test]
    fn villager_profession_maps_to_bedrock() {
        assert_eq!(bedrock_profession(5), 1); // farmer
        assert_eq!(bedrock_profession(9), 5); // librarian
        assert_eq!(bedrock_profession(0), 0);

        let mut villager = look(Kind::Villager);

        villager.profession = 5;
        assert!(entries(&villager).contains(&(KEY_VARIANT, Value::Int(1))));
    }

    #[test]
    fn dictionary_bytes_are_key_type_value() {
        let mut out = Out::packet(0);

        write(&mut out, &[(KEY_SCALE, Value::Float(0.5)), (KEY_COLOR, Value::Byte(3))]);
        assert_eq!(&out.bytes[1..], &[2, 38, 3, 0, 0, 0, 0x3f, 3, 0, 3]);
    }
}
