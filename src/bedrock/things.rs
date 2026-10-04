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
//! - `minecraft:fireworks_rocket` — флаги «сталкивается», «падает»; данные
//!   фейерверка — составной тег в ключе 16 (у BDS он зовётся
//!   `minecart_display_block`): `{Fireworks:{Explosions:[…], Flight:b}}`,
//!   копия тега предмета; у выпущенной предметом ракеты тег есть (пустой
//!   список взрывов и `Flight` 1 у простой), у вызванной `summon` — пустой
//!   составной тег. При взрыве — Actor Event `firework_particles` (25) и
//!   удаление. Взрыв описан так же, как у предмета в творческом меню:
//!   `FireworkColor` и `FireworkFade` — байтовые массивы номеров красок
//!   (0 чёрный … 15 белый, порядок красок Bedrock), `FireworkFlicker`
//!   (мерцание), `FireworkTrail` (след), `FireworkType` (0 малый шар,
//!   1 большой, 2 звезда, 3 крипер, 4 вспышка — как у Java);
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
const KEY_FIREWORKS: u32 = 16;
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

/// Краски Bedrock (номер у `FireworkColor`) и цвет взрыва у Java: порядок
/// Bedrock — чёрная, красная, зелёная, коричневая, синяя, пурпурная,
/// бирюзовая, светло-серая, серая, розовая, лаймовая, жёлтая, голубая,
/// сиреневая, оранжевая, белая; цвета — minecraft.wiki, Dye, «firework
/// color»; номера сверены с предметами творческого меню BDS (чёрная — 0,
/// серая — 8, светло-серая — 7).
const DYES: [i32; 16] = [
    0x1D1D21, 0xB02E26, 0x5E7C16, 0x835432, 0x3C44AA, 0x8932B8, 0x169C9C, 0x9D9D97, 0x474F52, 0xF38BAA, 0x80C71F, 0xFED83D, 0x3AB3DA,
    0xC74EBD, 0xF9801D, 0xF9FFFE,
];

/// Номер краски Bedrock для цвета взрыва Java (ближайшая из шестнадцати:
/// у Bedrock в звезде только краски).
pub fn bedrock_dye(rgb: i32) -> u8 {
    let (r, g, b) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);

    (0..16u8)
        .min_by_key(|&index| {
            let dye = DYES[usize::from(index)];
            let (dr, dg, db) = ((dye >> 16) - r, ((dye >> 8) & 0xff) - g, (dye & 0xff) - b);

            dr * dr + dg * dg + db * db
        })
        .unwrap_or(0)
}

/// Как пишутся имена, числа и длины в NBT: сеть (varint, zigzag) или
/// little-endian (предметы, файлы мира).
#[derive(Clone, Copy, PartialEq)]
pub enum NbtStyle {
    Network,
    Little,
}

struct Writer {
    style: NbtStyle,
    bytes: Vec<u8>,
}

impl Writer {
    fn tag(&mut self, kind: u8, name: &str) {
        self.bytes.push(kind);
        self.text(name);
    }

    fn text(&mut self, text: &str) {
        match self.style {
            NbtStyle::Network => self.varint(text.len() as u32),
            NbtStyle::Little => self.bytes.extend_from_slice(&(text.len() as u16).to_le_bytes()),
        }

        self.bytes.extend_from_slice(text.as_bytes());
    }

    fn varint(&mut self, mut value: u32) {
        while value >= 0x80 {
            self.bytes.push((value & 0x7f) as u8 | 0x80);
            value >>= 7;
        }

        self.bytes.push(value as u8);
    }

    /// Длина или счётчик: int (сеть — zigzag-varint).
    fn count(&mut self, value: i32) {
        match self.style {
            NbtStyle::Network => self.varint(((value << 1) ^ (value >> 31)) as u32),
            NbtStyle::Little => self.bytes.extend_from_slice(&value.to_le_bytes()),
        }
    }

    fn byte(&mut self, name: &str, value: u8) {
        self.tag(1, name);
        self.bytes.push(value);
    }

    fn int(&mut self, name: &str, value: i32) {
        self.tag(3, name);
        self.count(value);
    }

    fn bytes_array(&mut self, name: &str, values: &[u8]) {
        self.tag(7, name);
        self.count(values.len() as i32);
        self.bytes.extend_from_slice(values);
    }
}

/// Взрыв как составной тег (поля по алфавиту, как у BDS); без корня.
fn write_explosion(w: &mut Writer, explosion: &crate::stackdata::Explosion) {
    let dyes = |colors: &[i32]| colors.iter().map(|rgb| bedrock_dye(*rgb)).collect::<Vec<u8>>();

    w.bytes_array("FireworkColor", &dyes(&explosion.colors));
    w.bytes_array("FireworkFade", &dyes(&explosion.fade_colors));
    w.byte("FireworkFlicker", u8::from(explosion.twinkle));
    w.byte("FireworkTrail", u8::from(explosion.trail));
    w.byte("FireworkType", explosion.shape);
    w.bytes.push(0);
}

/// Составной тег `Fireworks` ракеты: `{Explosions:[…], Flight:b}`; без корня,
/// с началом «тип, имя».
fn write_fireworks(w: &mut Writer, fireworks: &crate::stackdata::Fireworks) {
    w.tag(10, "Fireworks");
    w.tag(9, "Explosions");

    if fireworks.explosions.is_empty() {
        w.bytes.push(0);
        w.count(0);
    } else {
        w.bytes.push(10);
        w.count(fireworks.explosions.len() as i32);

        for explosion in &fireworks.explosions {
            write_explosion(w, explosion);
        }
    }

    w.byte("Flight", fireworks.flight as u8);
    w.bytes.push(0);
}

/// Данные ракеты для клиента: корень `{Fireworks:{…}}`. Для сущности — сетевой
/// NBT (ключ 16), для предмета — little-endian (`item_nbt`).
pub fn rocket_tag(fireworks: &crate::stackdata::Fireworks, style: NbtStyle) -> Vec<u8> {
    let mut w = Writer { style, bytes: Vec::new() };

    w.tag(10, "");
    write_fireworks(&mut w, fireworks);
    w.bytes.push(0);
    w.bytes
}

/// Данные звезды: `FireworksItem` (взрыв) и `customColor`
/// (первый цвет, ARGB), как у звёзд творческого меню BDS.
pub fn star_tag(explosion: &crate::stackdata::Explosion, style: NbtStyle) -> Vec<u8> {
    let mut w = Writer { style, bytes: Vec::new() };

    w.tag(10, "");
    w.tag(10, "FireworksItem");
    write_explosion(&mut w, explosion);
    w.int("customColor", explosion.colors.first().map_or(0, |rgb| rgb | 0xFF00_0000u32 as i32));
    w.bytes.push(0);
    w.bytes
}

/// Ракета фейерверка.
pub fn rocket(id: i32, at: (f64, f64, f64), velocity: (f64, f64, f64), fireworks: &crate::stackdata::Fireworks) -> Thing {
    Thing {
        id,
        name: "minecraft:fireworks_rocket",
        place: [at.0 as f32, at.1 as f32, at.2 as f32, 0.0, 0.0, 0.0],
        motion: [velocity.0 as f32, velocity.1 as f32, velocity.2 as f32],
        data: vec![(KEY_FLAGS, plain_flags(&[])), (KEY_FIREWORKS, Value::Nbt(rocket_tag(fireworks, NbtStyle::Network)))],
    }
}

/// Set Actor Data с одними данными ракеты — после события взрыва (BDS).
pub fn rocket_data_packet(id: i32, fireworks: &crate::stackdata::Fireworks) -> Vec<u8> {
    mobdata::set_data_packet(id, &[(KEY_FIREWORKS, Value::Nbt(rocket_tag(fireworks, NbtStyle::Network)))])
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
        let bytes = add_packet(&rocket(9, (1.0, 2.0, 3.0), (0.0, 0.5, 0.0), &crate::stackdata::Fireworks::default()));

        assert_eq!(bytes[0], 0x0d);
        assert!(String::from_utf8_lossy(&bytes).contains("minecraft:fireworks_rocket"));
        // Конец пакета: данные ракеты (составной тег), свойства, связи.
        assert_eq!(&bytes[bytes.len() - 3..], &[0, 0, 0]);
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Простая ракета: тег как у BDS (запись bds-fw2-*.jsonl, набор данных
    /// ракеты после `/give firework_rocket` и пуска): `{Fireworks:{Explosions:[],
    /// Flight:1}}` — пустой список с типом «конец».
    #[test]
    fn a_plain_rocket_tag_matches_bds() {
        let tag = rocket_tag(&crate::stackdata::Fireworks::default(), NbtStyle::Network);

        let mut want = vec![0x0a, 0x00, 0x0a, 0x09];

        want.extend_from_slice(b"Fireworks");
        want.extend_from_slice(&[0x09, 0x0a]);
        want.extend_from_slice(b"Explosions");
        want.extend_from_slice(&[0x00, 0x00, 0x01, 0x06]);
        want.extend_from_slice(b"Flight");
        want.extend_from_slice(&[0x01, 0x00, 0x00]);

        assert_eq!(hex(&tag), hex(&want));
    }

    #[test]
    fn dyes_follow_the_bedrock_order() {
        assert_eq!(bedrock_dye(0x1D1D21), 0);
        assert_eq!(bedrock_dye(0x474F52), 8);
        assert_eq!(bedrock_dye(0x9D9D97), 7);
        assert_eq!(bedrock_dye(0xF9FFFE), 15);
        assert_eq!(bedrock_dye(0xB02E26), 1);
        // Не краска — ближайшая: почти красный.
        assert_eq!(bedrock_dye(0xFF0000), 1);
    }

    #[test]
    fn an_explosion_is_written_like_the_creative_menu_rocket() {
        let explosion = crate::stackdata::Explosion { shape: 2, colors: vec![0x9D9D97, 0xB02E26], fade_colors: vec![0xF9FFFE], trail: true, twinkle: false };
        let fireworks = crate::stackdata::Fireworks { flight: 2, explosions: vec![explosion] };
        let tag = rocket_tag(&fireworks, NbtStyle::Little);
        let has = |needle: &[u8]| tag.windows(needle.len()).any(|w| w == needle);

        // Цвета — байтовый массив [7, 1], затухание [15], след 1, форма 2,
        // полёт 2 (little-endian: длина — int32).
        assert!(has(&[7, 13, 0, b'F', b'i', b'r', b'e', b'w', b'o', b'r', b'k', b'C', b'o', b'l', b'o', b'r', 2, 0, 0, 0, 7, 1]));
        assert!(has(&[7, 12, 0, b'F', b'i', b'r', b'e', b'w', b'o', b'r', b'k', b'F', b'a', b'd', b'e', 1, 0, 0, 0, 15]));
        assert!(has(&[1, 13, 0, b'F', b'i', b'r', b'e', b'w', b'o', b'r', b'k', b'T', b'r', b'a', b'i', b'l', 1]));
        assert!(has(&[1, 12, 0, b'F', b'i', b'r', b'e', b'w', b'o', b'r', b'k', b'T', b'y', b'p', b'e', 2]));
        assert!(has(&[1, 6, 0, b'F', b'l', b'i', b'g', b'h', b't', 2]));
    }

    #[test]
    fn a_star_carries_its_first_colour_as_custom_colour() {
        let explosion = crate::stackdata::Explosion { shape: 0, colors: vec![0x1D1D21], ..Default::default() };
        let tag = star_tag(&explosion, NbtStyle::Little);

        // customColor: int 0xFF1D1D21 = -14869215 (звезда чёрного цвета у BDS).
        assert!(tag.windows(4).any(|w| w == (-14_869_215i32).to_le_bytes()));
    }

    #[test]
    fn unknown_potions_fall_back_to_water() {
        assert_eq!(bedrock_potion("nonsense"), 0);
        assert_eq!(bedrock_potion("healing"), 21);
    }
}
