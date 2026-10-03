// Звуки, частицы и подожжённый динамит для Bedrock: события мира, которые у
// Java уходят пакетами World Event, Sound Effect и Explosion, здесь
// переводятся в Level Event, Level Sound Event и Add Actor (описание
// протокола Mojang, minecraft-data Bedrock 1.26.10: packet_level_event,
// packet_level_sound_event, SoundType, MetadataDictionary).
//
// Названия звуков Java в номера SoundType переводятся короткой таблицей —
// только те звуки, что сервер сам рассылает и без которых игра глуха
// (взрыв, запал динамита, шипение). Звуки блоков, которые клиент играет по
// самому блоку, не нужны. [проверить] живым клиентом.

use super::codec::Out;

const LEVEL_EVENT: u32 = 0x19;
const LEVEL_SOUND_EVENT: u32 = 0x7b;
pub const SET_ENTITY_MOTION: u32 = 0x28;
const ADD_ACTOR: u32 = 0x0d;

/// LevelEvent: частицы роста (костная мука) и большой взрыв
/// (`add_particle_huge_explode` = 0x4000 | 16).
pub const PARTICLE_CROP_GROWTH: i32 = 2005;
pub const PARTICLE_HUGE_EXPLODE: i32 = 0x4000 | 16;

/// SoundType: Fizz, Explode, Ignite, Fuse.
pub const SOUND_FIZZ: u32 = 27;
pub const SOUND_EXPLODE: u32 = 48;
pub const SOUND_FUSE: u32 = 51;

/// Событие мира Java «костная мука» (World Event 1505).
const JAVA_BONE_MEAL: i32 = crate::farming::BONE_MEAL_EVENT;

/// Level Event: событие, место, число.
pub fn level_event(event: i32, (x, y, z): (f32, f32, f32), data: i32) -> Vec<u8> {
    let mut out = Out::packet(LEVEL_EVENT);

    out.zigzag32(event).vec3f(x, y, z).zigzag32(data);
    out.bytes
}

/// Level Sound Event: звук в месте, без сущности, с расстоянием.
pub fn level_sound(sound: u32, (x, y, z): (f32, f32, f32)) -> Vec<u8> {
    let mut out = Out::packet(LEVEL_SOUND_EVENT);

    out.varint(sound).vec3f(x, y, z).zigzag32(-1).string("").bool(false).bool(false).li64(-1);
    out.bytes
}

/// Номер SoundType для звука Java; None — такого звука у Bedrock-клиента
/// нет или он играет его сам.
pub fn sound_for(name: &str) -> Option<u32> {
    match name {
        "minecraft:entity.generic.explode" => Some(SOUND_EXPLODE),
        "minecraft:entity.tnt.primed" => Some(SOUND_FUSE),
        "minecraft:block.fire.extinguish" => Some(SOUND_FIZZ),
        _ => None,
    }
}

/// Пакеты для события мира Java (`WorldEvent::Level`); пусто — клиенту
/// нечего показывать.
pub fn level_packets(x: i32, y: i32, z: i32, effect: i32, data: i32) -> Vec<Vec<u8>> {
    let centre = (x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);

    match effect {
        JAVA_BONE_MEAL => vec![level_event(PARTICLE_CROP_GROWTH, centre, data)],
        _ => Vec::new(),
    }
}

/// Пакеты для взрыва: звук и вспышка в центре (у взрыва ветра — только
/// звук).
pub fn explosion_packets(blast: &crate::world::Blast) -> Vec<Vec<u8>> {
    let centre = (blast.centre.0 as f32, blast.centre.1 as f32, blast.centre.2 as f32);
    let mut packets = vec![level_sound(SOUND_EXPLODE, centre)];

    if !blast.wind {
        packets.push(level_event(PARTICLE_HUGE_EXPLODE, centre, 0));
    }

    packets
}

/// Set Entity Motion: скорость в блоках за такт.
pub fn motion(runtime_id: u64, (x, y, z): (f32, f32, f32)) -> Vec<u8> {
    let mut out = Out::packet(SET_ENTITY_MOTION);

    out.varint64(runtime_id).vec3f(x, y, z).varint64(0);
    out.bytes
}

/// Подожжённый динамит (Add Actor `minecraft:tnt`): запал в тактах — ключ
/// `fuse_length` (55, int), флаги — «подожжён» (10), сталкивается (48) и
/// тяжесть (49).
pub fn add_primed(entity_id: i32, (x, y, z): (f32, f32, f32), (vx, vy, vz): (f32, f32, f32), fuse: i32) -> Vec<u8> {
    const KEY_FLAGS: u32 = 0;
    const KEY_FUSE: u32 = 55;
    const TYPE_INT: u32 = 2;
    const TYPE_LONG: u32 = 7;
    let flags: i64 = (1 << 10) | (1 << 48) | (1 << 49);

    let mut out = Out::packet(ADD_ACTOR);

    out.zigzag64(i64::from(entity_id))
        .varint64(entity_id as u64)
        .string("minecraft:tnt")
        .vec3f(x, y, z)
        .vec3f(vx, vy, vz)
        .lf32(0.0)
        .lf32(0.0)
        .lf32(0.0)
        .lf32(0.0)
        .varint(0) // свойства-атрибуты
        .varint(2); // данные сущности
    out.varint(KEY_FLAGS).varint(TYPE_LONG).zigzag64(flags);
    out.varint(KEY_FUSE).varint(TYPE_INT).zigzag32(fuse);
    out.varint(0).varint(0); // свойства: целые, дробные
    out.varint(0); // связи
    out.bytes
}

/// Перемещение подожжённого динамита (Move Actor Absolute, без поворота).
pub fn move_primed(entity_id: i32, (x, y, z): (f32, f32, f32)) -> Vec<u8> {
    let mut out = Out::packet(0x12);

    out.varint64(entity_id as u64).u8(0).vec3f(x, y, z).u8(0).u8(0).u8(0);
    out.bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bedrock::codec::{In, packet_id};

    #[test]
    fn explosion_makes_a_sound_and_a_flash() {
        let blast = crate::world::Blast { centre: (1.0, 64.0, 2.0), power: 4.0, blocks: 3, pushed: Vec::new(), wind: false };
        let packets = explosion_packets(&blast);

        assert_eq!(packets.len(), 2);
        assert_eq!(packet_id(&packets[0]).map(|p| p.0), Some(LEVEL_SOUND_EVENT));

        let (id, body) = packet_id(&packets[1]).expect("пакет");
        let mut reader = In::new(body);

        assert_eq!(id, LEVEL_EVENT);
        assert_eq!(reader.zigzag32(), Some(PARTICLE_HUGE_EXPLODE));
        assert_eq!(explosion_packets(&crate::world::Blast { wind: true, ..blast }).len(), 1);
    }

    #[test]
    fn bone_meal_shows_green_particles() {
        let packets = level_packets(1, 2, 3, crate::farming::BONE_MEAL_EVENT, 15);
        let (_, body) = packet_id(&packets[0]).expect("пакет");

        assert_eq!(In::new(body).zigzag32(), Some(PARTICLE_CROP_GROWTH));
        assert!(level_packets(1, 2, 3, 1000, 0).is_empty());
    }

    #[test]
    fn primed_tnt_carries_its_fuse() {
        let packet = add_primed(7, (1.5, 64.0, 2.5), (0.0, 0.2, 0.0), 80);
        let (id, body) = packet_id(&packet).expect("пакет");
        let mut reader = In::new(body);

        assert_eq!(id, ADD_ACTOR);
        assert_eq!(reader.zigzag64(), Some(7));
        assert_eq!(reader.varint64(), Some(7));
        assert_eq!(reader.string().as_deref(), Some("minecraft:tnt"));
        // Запал 80 лежит в конце данных сущности (zigzag: 160 = 0xa0 0x01).
        assert!(packet.windows(5).any(|w| w == [55, 2, 0xa0, 0x01, 0]));
    }

    #[test]
    fn sounds_are_told_by_name() {
        assert_eq!(sound_for("minecraft:entity.generic.explode"), Some(SOUND_EXPLODE));
        assert_eq!(sound_for("minecraft:block.stone.hit"), None);
    }
}
