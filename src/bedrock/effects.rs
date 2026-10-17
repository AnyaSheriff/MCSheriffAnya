// Звуки, частицы и подожжённый динамит для Bedrock: события мира, которые у
// Java уходят пакетами World Event, Sound Effect и Explosion, здесь
// переводятся в Level Event, Level Sound Event и Add Actor (описание
// протокола Mojang, minecraft-data Bedrock 1.26.10: packet_level_event,
// packet_level_sound_event, SoundType, MetadataDictionary).
//
// Названия звуков Java в номера SoundType переводятся таблицей, снятой с
// официального BDS 1.26.13 (tmp/br-sounds: два зонда, действующий и
// смотрящий). Что BDS НЕ шлёт, того тут нет: удар, боль и смерть мобов,
// шаги, поломку и постановку блоков клиент Bedrock играет сам — по
// ActorEvent и по самому Update Block. Шлёт он: звуки дверей, люков,
// калиток, рычагов, кнопок, плит, сундуков и бочек (и тому, кто щёлкнул,
// тоже), ноту, выстрел и попадание стрелы, бросок зелья, звук выхода
// из еды (Burp), запуск ракеты; удар игрока — всем, кроме бьющего.

use super::codec::Out;

const LEVEL_EVENT: u32 = 0x19;
const LEVEL_SOUND_EVENT: u32 = 0x7b;
pub const SET_ENTITY_MOTION: u32 = 0x28;
const ADD_ACTOR: u32 = 0x0d;

/// LevelEvent: частицы роста (костная мука).
pub const PARTICLE_CROP_GROWTH: i32 = 2005;

/// SoundType: Fizz, Explode, Ignite, Fuse.
pub const SOUND_FIZZ: u32 = 27;
pub const SOUND_EXPLODE: u32 = 48;
pub const SOUND_FUSE: u32 = 51;
/// Торг: житель согласен и отказывает (HaggleYes, HaggleNo; BDS играет первый
/// при каждой сделке, tmp/br-trade).
pub const SOUND_HAGGLE_YES: u32 = 122;
pub const SOUND_HAGGLE_NO: u32 = 123;

pub const SOUND_BOW: u32 = 21;
pub const SOUND_GLASS: u32 = 127;
pub const SOUND_THROW: u32 = 40;
pub const SOUND_ATTACK_NODAMAGE: u32 = 42;
pub const SOUND_ATTACK_STRONG: u32 = 43;
pub const SOUND_LAUNCH: u32 = 56;
pub const SOUND_BOW_HIT: u32 = 63;
pub const SOUND_CHEST_OPEN: u32 = 67;
pub const SOUND_CHEST_CLOSED: u32 = 68;
pub const SOUND_SHULKER_OPEN: u32 = 69;
pub const SOUND_SHULKER_CLOSED: u32 = 70;
pub const SOUND_ENDER_CHEST_OPEN: u32 = 71;
pub const SOUND_ENDER_CHEST_CLOSED: u32 = 72;
pub const SOUND_NOTE: u32 = 81;
pub const SOUND_PISTON_IN: u32 = 83;
pub const SOUND_PISTON_OUT: u32 = 84;
pub const SOUND_BURP: u32 = 89;
pub const SOUND_SHIELD_BLOCK: u32 = 255;
pub const SOUND_BARREL_OPEN: u32 = 270;
pub const SOUND_BARREL_CLOSE: u32 = 271;
pub const SOUND_PRESSURE_PLATE_OFF: u32 = 448;
pub const SOUND_PRESSURE_PLATE_ON: u32 = 449;
pub const SOUND_BUTTON_OFF: u32 = 450;
pub const SOUND_BUTTON_ON: u32 = 451;
pub const SOUND_DOOR_OPEN: u32 = 452;
pub const SOUND_DOOR_CLOSE: u32 = 453;
pub const SOUND_TRAPDOOR_OPEN: u32 = 454;
pub const SOUND_TRAPDOOR_CLOSE: u32 = 455;
pub const SOUND_FENCE_GATE_OPEN: u32 = 456;
pub const SOUND_FENCE_GATE_CLOSE: u32 = 457;

/// LevelEvent: частицы блока при поломке (BDS шлёт их и тому, кто сломал),
/// всплеск зелья и взрыв (`particle_explosion`; у BDS в данных 4 для
/// динамита мощности 4).
pub const EVENT_PARTICLE_DESTROY: i32 = 2001;
pub const EVENT_PARTICLE_SPLASH: i32 = 2002;
pub const EVENT_PARTICLE_EXPLOSION: i32 = 2025;

/// Событие мира Java «костная мука» (World Event 1505).
const JAVA_BONE_MEAL: i32 = crate::farming::BONE_MEAL_EVENT;

/// События мира Java: 2001 — блок сломан (данные — состояние блока),
/// 2002 — зелье разбилось (данные — цвет).
const JAVA_BLOCK_BREAK: i32 = 2001;
/// Наковальня (вики, World Event): сломалась 1029, работает 1030, упала 1031.
/// У Bedrock — LevelEvent 1020 `sound_anvil_break`, 1021 `sound_anvil_use`,
/// 1022 `sound_anvil_fall`.
const JAVA_ANVIL_BREAK: i32 = 1029;
const JAVA_ANVIL_USE: i32 = 1030;
const JAVA_ANVIL_LAND: i32 = 1031;
const EVENT_ANVIL_BREAK: i32 = 1020;
const EVENT_ANVIL_USE: i32 = 1021;
const EVENT_ANVIL_FALL: i32 = 1022;
const JAVA_POTION_SPLASH: i32 = 2002;

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

/// Что ещё идёт в пакете звука, кроме номера.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extra {
    /// Ничего (−1).
    None,
    /// Номер блока Bedrock в этом месте (так шлёт BDS звуки блоков).
    Block,
    /// Звук игрока (удар, бросок): вид существа «игрок»; бьющему не шлётся —
    /// свой удар клиент играет сам.
    Player,
    /// Нота: инструмент и высота (0–24); место — угол блока, как у BDS.
    Note(i32, i32),
}

/// Звук Bedrock для звука Java.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    pub id: u32,
    pub extra: Extra,
}

impl Cue {
    const fn new(id: u32, extra: Extra) -> Cue {
        Cue { id, extra }
    }
}

/// Номер SoundType для звука Java; None — такого звука у Bedrock-клиента
/// нет или он играет его сам (боль и смерть мобов, шаги, еда, звук
/// постановки блока): BDS их не шлёт.
pub fn cue_for(name: &str, pitch: f32) -> Option<Cue> {
    let short = name.strip_prefix("minecraft:")?;

    match short {
        // Mojang r/26_u1, SoundType: имена не зависят от Java registry id.
        "item.crossbow.loading_start" => return Some(Cue::new(245, Extra::None)),
        "item.crossbow.loading_middle" => return Some(Cue::new(246, Extra::None)),
        "item.crossbow.loading_end" => return Some(Cue::new(247, Extra::None)),
        "item.crossbow.shoot" => return Some(Cue::new(248, Extra::None)),
        "item.crossbow.quick_charge_1" => return Some(Cue::new(249, Extra::None)),
        "item.crossbow.quick_charge_2" => return Some(Cue::new(250, Extra::None)),
        "item.crossbow.quick_charge_3" => return Some(Cue::new(251, Extra::None)),
        "block.respawn_anchor.charge" => return Some(Cue::new(308, Extra::None)),
        "block.respawn_anchor.deplete" => return Some(Cue::new(309, Extra::None)),
        "block.respawn_anchor.set_spawn" => return Some(Cue::new(310, Extra::None)),
        "entity.player.attack.crit" => return Some(Cue::new(569, Extra::Player)),
        "item.trident.hit" => return Some(Cue::new(178, Extra::None)),
        "item.trident.return" => return Some(Cue::new(179, Extra::None)),
        "item.trident.riptide_1" => return Some(Cue::new(180, Extra::None)),
        "item.trident.riptide_2" => return Some(Cue::new(181, Extra::None)),
        "item.trident.riptide_3" => return Some(Cue::new(182, Extra::None)),
        "item.trident.throw" => return Some(Cue::new(183, Extra::None)),
        "item.trident.thunder" => return Some(Cue::new(184, Extra::None)),
        "item.trident.hit_ground" => return Some(Cue::new(185, Extra::None)),
        "entity.warden.sonic_boom" => return Some(Cue::new(436, Extra::None)),
        "entity.warden.sonic_charge" => return Some(Cue::new(437, Extra::None)),
        "entity.wind_charge.throw" => return Some(Cue::new(438, Extra::None)),
        "entity.wind_charge.wind_burst" => return Some(Cue::new(509, Extra::None)),
        "item.mace.smash_air" => return Some(Cue::new(514, Extra::None)),
        "item.mace.smash_ground" => return Some(Cue::new(515, Extra::None)),
        "item.mace.smash_ground_heavy" => return Some(Cue::new(520, Extra::None)),
        "event.mob_effect.bad_omen" => return Some(Cue::new(523, Extra::None)),
        "event.mob_effect.raid_omen" => return Some(Cue::new(524, Extra::None)),
        "event.mob_effect.trial_omen" => return Some(Cue::new(525, Extra::None)),
        "entity.generic.explode" => return Some(Cue::new(SOUND_EXPLODE, Extra::None)),
        "entity.tnt.primed" => return Some(Cue::new(SOUND_FUSE, Extra::None)),
        "block.fire.extinguish" => return Some(Cue::new(SOUND_FIZZ, Extra::None)),
        // minecraft-data 1.26.10, SoundType; вики, Block of Copper: «Waxing».
        // [проверить] расстояние и extra_data на живом клиенте.
        "block.bell.use" => return Some(Cue::new(258, Extra::Block)),
        "block.campfire.crackle" => return Some(Cue::new(259, Extra::Block)),
        "item.honeycomb.wax_on" => return Some(Cue::new(339, Extra::Block)),
        "item.axe.wax_off" => return Some(Cue::new(340, Extra::Block)),
        "item.axe.scrape" => return Some(Cue::new(341, Extra::Block)),
        "item.brush.brushing.sand" => return Some(Cue::new(462, Extra::Block)),
        "item.brush.brushing.gravel" => return Some(Cue::new(462, Extra::Block)),
        "item.brush.brushing.sand.complete" => return Some(Cue::new(463, Extra::Block)),
        "item.brush.brushing.gravel.complete" => return Some(Cue::new(463, Extra::Block)),
        "block.sponge.absorb" => return Some(Cue::new(534, Extra::Block)),
        "block.bubble_column.upwards_ambient" => return Some(Cue::new(214, Extra::Block)),
        "block.bubble_column.whirlpool_ambient" => return Some(Cue::new(215, Extra::Block)),
        "block.bubble_column.bubble_pop" => return Some(Cue::new(216, Extra::Block)),
        "block.bubble_column.upwards_inside" => return Some(Cue::new(217, Extra::Block)),
        "block.bubble_column.whirlpool_inside" => return Some(Cue::new(218, Extra::Block)),
        "block.trial_spawner.open_shutter" => return Some(Cue::new(484, Extra::Block)),
        "block.trial_spawner.eject_item" => return Some(Cue::new(485, Extra::Block)),
        "block.trial_spawner.detect_player" => return Some(Cue::new(486, Extra::Block)),
        "block.trial_spawner.spawn_mob" => return Some(Cue::new(487, Extra::Block)),
        "block.trial_spawner.close_shutter" => return Some(Cue::new(488, Extra::Block)),
        "block.trial_spawner.ambient" => return Some(Cue::new(489, Extra::Block)),
        "block.trial_spawner.ambient_ominous" => return Some(Cue::new(517, Extra::Block)),
        "block.vault.open_shutter" => return Some(Cue::new(500, Extra::Block)),
        "block.vault.close_shutter" => return Some(Cue::new(501, Extra::Block)),
        "block.vault.eject_item" => return Some(Cue::new(502, Extra::Block)),
        "block.vault.insert_item" => return Some(Cue::new(503, Extra::Block)),
        "block.vault.insert_item_fail" => return Some(Cue::new(504, Extra::Block)),
        "block.vault.ambient" => return Some(Cue::new(505, Extra::Block)),
        "block.vault.activate" => return Some(Cue::new(506, Extra::Block)),
        "block.vault.deactivate" => return Some(Cue::new(507, Extra::Block)),
        "block.vault.reject_rewarded_player" => return Some(Cue::new(530, Extra::Block)),


        // Звук не определяет силу: сервер использует общий Java cooldown.
        "entity.player.attack.strong" | "entity.player.attack.weak" | "entity.player.attack.sweep" | "entity.player.attack.knockback" => {
            return Some(Cue::new(SOUND_ATTACK_STRONG, Extra::Player));
        }
        "entity.player.attack.nodamage" => return Some(Cue::new(SOUND_ATTACK_NODAMAGE, Extra::Player)),
        "item.shield.block" => return Some(Cue::new(SOUND_SHIELD_BLOCK, Extra::None)),
        "entity.arrow.shoot" => return Some(Cue::new(SOUND_BOW, Extra::None)),
        "entity.arrow.hit" => return Some(Cue::new(SOUND_BOW_HIT, Extra::None)),
        "entity.splash_potion.throw" | "entity.lingering_potion.throw" => return Some(Cue::new(SOUND_THROW, Extra::Player)),
        "entity.player.burp" => return Some(Cue::new(SOUND_BURP, Extra::None)),
        // Торг жителя (tmp/br-trade).
        "entity.villager.yes" => return Some(Cue::new(SOUND_HAGGLE_YES, Extra::None)),
        "entity.villager.no" => return Some(Cue::new(SOUND_HAGGLE_NO, Extra::None)),
        "entity.firework_rocket.launch" => return Some(Cue::new(SOUND_LAUNCH, Extra::None)),
        "block.lever.click" => {
            // Один звук у Java: высота 0,6 — включили, 0,5 — выключили.
            return Some(Cue::new(if pitch >= 0.55 { SOUND_BUTTON_ON } else { SOUND_BUTTON_OFF }, Extra::Block));
        }
        "block.piston.extend" => return Some(Cue::new(SOUND_PISTON_OUT, Extra::None)),
        "block.piston.contract" => return Some(Cue::new(SOUND_PISTON_IN, Extra::None)),
        _ => {}
    }

    let rest = short.strip_prefix("block.")?;
    let (what, action) = rest.rsplit_once('.')?;

    if let Some(instrument) = what.strip_prefix("note_block") {
        return note_cue(instrument.trim_start_matches('.'), action, pitch);
    }

    let open = match action {
        "open" | "click_on" => true,
        "close" | "click_off" => false,
        _ => return None,
    };
    let pick = |on: u32, off: u32| Some(Cue::new(if open { on } else { off }, Extra::Block));

    if action.starts_with("click_") {
        return match what {
            w if w.ends_with("pressure_plate") => pick(SOUND_PRESSURE_PLATE_ON, SOUND_PRESSURE_PLATE_OFF),
            w if w.ends_with("button") => pick(SOUND_BUTTON_ON, SOUND_BUTTON_OFF),
            _ => None,
        };
    }

    match what {
        w if w.ends_with("trapdoor") => pick(SOUND_TRAPDOOR_OPEN, SOUND_TRAPDOOR_CLOSE),
        w if w.ends_with("fence_gate") => pick(SOUND_FENCE_GATE_OPEN, SOUND_FENCE_GATE_CLOSE),
        w if w.ends_with("door") => pick(SOUND_DOOR_OPEN, SOUND_DOOR_CLOSE),
        "ender_chest" => pick(SOUND_ENDER_CHEST_OPEN, SOUND_ENDER_CHEST_CLOSED),
        w if w.ends_with("shulker_box") => pick(SOUND_SHULKER_OPEN, SOUND_SHULKER_CLOSED),
        "barrel" => pick(SOUND_BARREL_OPEN, SOUND_BARREL_CLOSE),
        // Сундук, сундук-ловушка и медные сундуки звучат одинаково.
        w if w.contains("chest") => pick(SOUND_CHEST_OPEN, SOUND_CHEST_CLOSED),
        _ => None,
    }
}

/// Нота: звук Java назван по инструменту, высота — множитель 2^((нота−12)/12).
/// У Bedrock — один звук Note, в данных инструмент и нота. Номера
/// инструментов — порядок из документации Bedrock [проверить].
fn note_cue(instrument: &str, action: &str, pitch: f32) -> Option<Cue> {
    let instrument = if action.is_empty() || instrument.is_empty() { action } else { instrument };
    let index = match instrument {
        "harp" => 0,
        "basedrum" => 1,
        "snare" => 2,
        "hat" => 3,
        "bass" => 4,
        "flute" => 5,
        "bell" => 6,
        "guitar" => 7,
        "chime" => 8,
        "xylophone" => 9,
        "iron_xylophone" => 10,
        "cow_bell" => 11,
        "didgeridoo" => 12,
        "bit" => 13,
        "banjo" => 14,
        "pling" => 15,
        _ => return None,
    };
    let note = (12.0 * pitch.max(0.01).log2()).round() as i32 + 12;

    Some(Cue::new(SOUND_NOTE, Extra::Note(index, note.clamp(0, 24))))
}

/// Звук открытия или закрытия хранилища с крышкой (сундук, бочка, шалкер).
pub fn is_lid_sound(name: &str) -> bool {
    cue_for(name, 1.0).is_some_and(|cue| {
        matches!(
            cue.id,
            SOUND_CHEST_OPEN
                | SOUND_CHEST_CLOSED
                | SOUND_SHULKER_OPEN
                | SOUND_SHULKER_CLOSED
                | SOUND_ENDER_CHEST_OPEN
                | SOUND_ENDER_CHEST_CLOSED
                | SOUND_BARREL_OPEN
                | SOUND_BARREL_CLOSE
        )
    })
}

/// Level Sound Event с данными и видом существа (`entity` — например
/// «minecraft:player»; пусто — звук самого мира).
pub fn level_sound_with(sound: u32, (x, y, z): (f32, f32, f32), extra: i32, entity: &str) -> Vec<u8> {
    let mut out = Out::packet(LEVEL_SOUND_EVENT);

    out.varint(sound).vec3f(x, y, z).zigzag32(extra).string(entity).bool(false).bool(false).li64(-1);
    out.bytes
}

/// Block Event «звук» (0) — нота: BDS шлёт его перед звуком Note.
pub fn note_block_event((x, y, z): (i32, i32, i32), note: i32) -> Vec<u8> {
    let mut out = Out::packet(super::session::BLOCK_EVENT);

    out.block_position(x, y, z).zigzag32(0).zigzag32(note);
    out.bytes
}

/// Звук Java как пакеты Bedrock (`block` — номер блока Bedrock в месте
/// звука, если известен). Пусто — у Bedrock такого звука нет.
pub fn sound_packets(name: &str, pitch: f32, at: (f32, f32, f32), block: Option<i32>) -> Vec<Vec<u8>> {
    let Some(cue) = cue_for(name, pitch) else {
        return Vec::new();
    };

    match cue.extra {
        Extra::None => vec![level_sound_with(cue.id, at, -1, "")],
        Extra::Block => vec![level_sound_with(cue.id, at, block.unwrap_or(-1), "")],
        Extra::Player => vec![level_sound_with(cue.id, at, -1, "minecraft:player")],
        Extra::Note(instrument, note) => {
            let corner = (at.0.floor(), at.1.floor(), at.2.floor());

            vec![
                note_block_event((corner.0 as i32, corner.1 as i32, corner.2 as i32), note),
                level_sound_with(cue.id, corner, (instrument << 8) | note, ""),
            ]
        }
    }
}

/// Completed Using Item: еда доедена (предмет Bedrock и способ 1 — «eat»).
pub fn completed_using_item(item: i16, method: i32) -> Vec<u8> {
    let mut out = Out::packet(0x8e);

    out.li16(item).li32(method);
    out.bytes
}

/// Пакеты для события мира Java (`WorldEvent::Level`); пусто — клиенту
/// нечего показывать.
pub fn level_packets(x: i32, y: i32, z: i32, effect: i32, data: i32) -> Vec<Vec<u8>> {
    let centre = (x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);

    match effect {
        JAVA_BONE_MEAL => vec![level_event(PARTICLE_CROP_GROWTH, centre, data)],
        // World Event Java: wax_on/off/scrape → отдельные LevelEvent Bedrock.
        3003 => vec![level_event(2030, centre, 0)],
        3004 => vec![level_event(2031, centre, 0)],
        3005 => vec![level_event(2032, centre, 0)],
        1009 => vec![level_sound(SOUND_FIZZ, centre)],
        // Упавшая наковальня: BDS шлёт LevelEvent 1022 с данными 0 в клетке,
        // куда она встала (чёрный ящик, tmp/br-polish/bds-af*.jsonl: место —
        // угол блока). Работа и поломка — те же имена событий Bedrock,
        // у BDS не снято [проверить].
        JAVA_ANVIL_LAND => vec![level_event(EVENT_ANVIL_FALL, (x as f32, y as f32, z as f32), 0)],
        JAVA_ANVIL_USE => vec![level_event(EVENT_ANVIL_USE, (x as f32, y as f32, z as f32), 0)],
        JAVA_ANVIL_BREAK => vec![level_event(EVENT_ANVIL_BREAK, (x as f32, y as f32, z as f32), 0)],
        // Блок сломан: частицы с его номером Bedrock (звук поломки клиент
        // играет по ним сам; BDS шлёт их и тому, кто сломал).
        JAVA_BLOCK_BREAK => vec![level_event(EVENT_PARTICLE_DESTROY, centre, super::world::bedrock_state(data) as i32)],
        // Зелье разбилось: всплеск цвета зелья (данные — цвет RGB, у BDS —
        // ARGB с непрозрачным альфа) и звук стекла.
        JAVA_POTION_SPLASH => vec![
            level_event(EVENT_PARTICLE_SPLASH, centre, data | (0xffu32 << 24) as i32),
            level_sound_with(SOUND_GLASS, centre, -1, ""),
        ],
        _ => Vec::new(),
    }
}

/// Пакеты для взрыва: звук и вспышка в центре (у взрыва ветра — только
/// звук).
pub fn explosion_packets(blast: &crate::world::Blast) -> Vec<Vec<u8>> {
    let centre = (blast.centre.0 as f32, blast.centre.1 as f32, blast.centre.2 as f32);
    if blast.wind {
        // LevelEvent WindExplosion/BreezeWindExplosion из документации 944.
        let breeze = blast.gust == crate::world::Gust::Breeze;
        return vec![level_sound(if breeze { 493 } else { 509 }, centre),
            level_event(if breeze { 3610 } else { 3614 }, centre, 0)];
    }
    let mut packets = vec![level_sound(SOUND_EXPLODE, centre)];

    if !blast.wind {
        // BDS: `particle_explosion`, в данных 4 у динамита мощности 4.
        packets.push(level_event(EVENT_PARTICLE_EXPLOSION, centre, blast.power as i32));
    }

    packets
}

/// События боевых частиц. SonicExplosion — 2039; LegacyEvent | Crit —
/// 0x4003 (Mojang r/26_u1). SweepAttack у Bedrock отсутствует: показываем
/// искры крита в точке взмаха, без изменения урона [проверить].
pub fn combat_particles(kind: i32, block: Option<i32>, at: (f64, f64, f64)) -> Option<Vec<u8>> {
    let at = (at.0 as f32, at.1 as f32, at.2 as f32);
    match kind {
        28 => Some(level_event(2039, at, 0)),
        67 => Some(level_event(0x4003, at, 0)),
        1 => Some(level_event(9815, at, super::world::bedrock_state(block?) as i32)),
        26 => Some(level_event(3610, at, 0)),
        27 => Some(level_event(3614, at, 0)),
        _ => None,
    }
}

/// minecraft-data 1.26.10, packet_spawn_particle_effect; wiki Particles
/// (Bedrock Edition): «soul_particle». Вид и траектория на BDS [проверить].
pub fn soul_particle(dimension: u8, at: (f64, f64, f64)) -> Vec<u8> {
    let mut out = Out::packet(118);
    out.u8(dimension).zigzag64(-1).vec3f(at.0 as f32, at.1 as f32, at.2 as f32)
        .string("minecraft:soul_particle").bool(false);
    out.bytes
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
    fn soul_particle_carries_dimension_and_unbound_entity() {
        let packet = soul_particle(2, (1.5, 70.1, 4.5));
        let (id, data) = packet_id(&packet).unwrap();
        assert_eq!(id, 118);
        let mut r = In::new(data);
        assert_eq!(r.u8(), Some(2));
        assert_eq!(r.zigzag64(), Some(-1));
        for v in [1.5, 70.1, 4.5] { assert_eq!(r.lf32(), Some(v)); }
        assert_eq!(r.string().as_deref(), Some("minecraft:soul_particle"));
        assert_eq!(r.bool(), Some(false));
    }

    #[test]
    fn wind_and_combat_particles_have_documented_events() {
        for (gust, sound, particle) in [(crate::world::Gust::Breeze, 493, 3610), (crate::world::Gust::Charge, 509, 3614), (crate::world::Gust::Mace, 509, 3614)] {
            let blast = crate::world::Blast { centre: (1.0,2.0,3.0), power: 1.0, blocks: 0, pushed: Vec::new(), wind: true, gust };
            let packets = explosion_packets(&blast);
            let (id, body) = packet_id(&packets[0]).unwrap(); let mut r = In::new(body);
            assert_eq!(id, LEVEL_SOUND_EVENT); assert_eq!(r.varint(), Some(sound));
            for value in [1.0,2.0,3.0] { assert_eq!(r.lf32(), Some(value)); }
            assert_eq!(r.zigzag32(), Some(-1)); assert_eq!(r.string().as_deref(), Some(""));
            assert_eq!((r.bool(), r.bool()), (Some(false), Some(false))); assert_eq!(r.take(8), Some((-1i64).to_le_bytes().as_slice())); assert_eq!(r.remaining(), 0);
            let (id, body) = packet_id(&packets[1]).unwrap(); let mut r = In::new(body);
            assert_eq!(id, LEVEL_EVENT); assert_eq!(r.zigzag32(), Some(particle));
            for value in [1.0,2.0,3.0] { assert_eq!(r.lf32(), Some(value)); }
            assert_eq!((r.zigzag32(), r.remaining()), (Some(0), 0));
        }
        for (kind, expected) in [(28,2039), (67,0x4003), (26,3610), (27,3614)] {
            let bytes = combat_particles(kind, None, (1.0,2.0,3.0)).unwrap();
            let (_, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
            assert_eq!(r.zigzag32(), Some(expected));
            for value in [1.0,2.0,3.0] { assert_eq!(r.lf32(), Some(value)); }
            assert_eq!((r.zigzag32(), r.remaining()), (Some(0), 0));
        }
        assert!(combat_particles(1, None, (0.0,0.0,0.0)).is_none());
        let bytes = combat_particles(1, Some(0), (0.0,0.0,0.0)).unwrap();
        let (_, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!(r.zigzag32(), Some(9815)); for _ in 0..3 { assert_eq!(r.lf32(), Some(0.0)); }
        assert_eq!(r.zigzag32(), Some(super::super::world::bedrock_state(0) as i32)); assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn block_action_events_decode_with_bedrock_ids() {
        for (java,bedrock) in [(3003,2030),(3004,2031),(3005,2032)] {
            let packets = level_packets(-2,70,3,java,0); assert_eq!(packets.len(),1);
            let mut r = In::new(&packets[0]); assert_eq!(r.varint(),Some(LEVEL_EVENT));
            assert_eq!(r.zigzag32(),Some(bedrock));
            assert_eq!((r.lf32(),r.lf32(),r.lf32()),(Some(-1.5),Some(70.5),Some(3.5)));
            assert_eq!(r.zigzag32(),Some(0)); assert_eq!(r.remaining(),0);
        }
        for (name,id) in [("minecraft:block.bell.use",258),("minecraft:item.brush.brushing.sand",462),
            ("minecraft:block.sponge.absorb",534),("minecraft:item.axe.scrape",341)] {
            let packets = sound_packets(name,1.0,(1.0,2.0,3.0),Some(55)); assert_eq!(packets.len(),1);
            let mut r = In::new(&packets[0]); assert_eq!(r.varint(),Some(LEVEL_SOUND_EVENT)); assert_eq!(r.varint(),Some(id));
            assert_eq!((r.lf32(),r.lf32(),r.lf32()),(Some(1.0),Some(2.0),Some(3.0)));
            assert_eq!(r.zigzag32(),Some(55)); assert_eq!(r.string().as_deref(),Some(""));
            assert_eq!((r.bool(),r.bool()),(Some(false),Some(false))); assert_eq!(r.take(8),Some([255;8].as_slice())); assert_eq!(r.remaining(),0);
        }
    }

    #[test]
    fn explosion_makes_a_sound_and_a_flash() {
        let blast = crate::world::Blast { centre: (1.0, 64.0, 2.0), power: 4.0, blocks: 3, pushed: Vec::new(), wind: false, gust: crate::world::Gust::Mace };
        let packets = explosion_packets(&blast);

        assert_eq!(packets.len(), 2);
        assert_eq!(packet_id(&packets[0]).map(|p| p.0), Some(LEVEL_SOUND_EVENT));

        let (id, body) = packet_id(&packets[1]).expect("пакет");
        let mut reader = In::new(body);

        assert_eq!(id, LEVEL_EVENT);
        assert_eq!(reader.zigzag32(), Some(EVENT_PARTICLE_EXPLOSION));
        assert_eq!(explosion_packets(&crate::world::Blast { wind: true, ..blast }).len(), 2);
    }

    #[test]
    fn bone_meal_shows_green_particles() {
        let packets = level_packets(1, 2, 3, crate::farming::BONE_MEAL_EVENT, 15);
        let (_, body) = packet_id(&packets[0]).expect("пакет");

        assert_eq!(In::new(body).zigzag32(), Some(PARTICLE_CROP_GROWTH));
        assert!(level_packets(1, 2, 3, 1000, 0).is_empty());

        // Упавшая наковальня — LevelEvent 1022 на углу блока, данные 0 (BDS).
        let landed = level_packets(1, 2, 3, 1031, 0);

        assert_eq!(landed.len(), 1);

        let (id, body) = packet_id(&landed[0]).expect("пакет");

        assert_eq!(id, LEVEL_EVENT);
        assert_eq!(In::new(body).zigzag32(), Some(1022));
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
        let id = |name: &str, pitch: f32| cue_for(name, pitch).map(|cue| cue.id);

        assert_eq!(id("minecraft:entity.generic.explode", 1.0), Some(SOUND_EXPLODE));
        assert_eq!(id("minecraft:block.stone.hit", 1.0), None);
        // Боль, смерть, шаги и постановка блока — у клиента свои.
        assert_eq!(id("minecraft:entity.zombie.hurt", 1.0), None);
        assert_eq!(id("minecraft:entity.player.hurt", 1.0), None);
        assert_eq!(id("minecraft:block.stone.place", 1.0), None);
    }

    #[test]
    fn blocks_that_open_sound_like_bds() {
        let id = |name: &str, pitch: f32| cue_for(name, pitch).map(|cue| cue.id);

        assert_eq!(id("minecraft:block.wooden_door.open", 1.0), Some(SOUND_DOOR_OPEN));
        assert_eq!(id("minecraft:block.copper_door.close", 1.0), Some(SOUND_DOOR_CLOSE));
        assert_eq!(id("minecraft:block.cherry_wood_door.open", 1.0), Some(SOUND_DOOR_OPEN));
        assert_eq!(id("minecraft:block.bamboo_wood_trapdoor.open", 1.0), Some(SOUND_TRAPDOOR_OPEN));
        assert_eq!(id("minecraft:block.iron_trapdoor.close", 1.0), Some(SOUND_TRAPDOOR_CLOSE));
        assert_eq!(id("minecraft:block.fence_gate.close", 1.0), Some(SOUND_FENCE_GATE_CLOSE));
        assert_eq!(id("minecraft:block.nether_wood_fence_gate.open", 1.0), Some(SOUND_FENCE_GATE_OPEN));
        assert_eq!(id("minecraft:block.lever.click", 0.6), Some(SOUND_BUTTON_ON));
        assert_eq!(id("minecraft:block.lever.click", 0.5), Some(SOUND_BUTTON_OFF));
        assert_eq!(id("minecraft:block.stone_button.click_off", 1.0), Some(SOUND_BUTTON_OFF));
        assert_eq!(id("minecraft:block.wooden_pressure_plate.click_on", 0.8), Some(SOUND_PRESSURE_PLATE_ON));
        assert_eq!(id("minecraft:block.copper_chest_oxidized.open", 1.0), Some(SOUND_CHEST_OPEN));
        assert_eq!(id("minecraft:block.chest.close", 1.0), Some(SOUND_CHEST_CLOSED));
        assert_eq!(id("minecraft:block.ender_chest.open", 1.0), Some(SOUND_ENDER_CHEST_OPEN));
        assert_eq!(id("minecraft:block.barrel.open", 1.0), Some(SOUND_BARREL_OPEN));
        assert_eq!(id("minecraft:block.barrel.close", 1.0), Some(SOUND_BARREL_CLOSE));
        assert!(is_lid_sound("minecraft:block.barrel.close"));
        assert!(!is_lid_sound("minecraft:block.wooden_door.open"));
    }

    #[test]
    fn combat_and_use_sounds_follow_bds() {
        let cue = |name: &str| cue_for(name, 1.0);

        // В протоколе 944 критический удар имеет отдельный SoundType 569.
        for name in ["strong", "weak", "sweep", "knockback"] {
            assert_eq!(cue(&format!("minecraft:entity.player.attack.{name}")), Some(Cue { id: SOUND_ATTACK_STRONG, extra: Extra::Player }));
        }

        assert_eq!(cue("minecraft:entity.player.attack.nodamage").map(|cue| cue.id), Some(SOUND_ATTACK_NODAMAGE));
        assert_eq!(cue("minecraft:entity.player.attack.crit").map(|cue| cue.id), Some(569));
        assert_eq!(cue("minecraft:entity.arrow.shoot").map(|cue| cue.id), Some(SOUND_BOW));
        assert_eq!(cue("minecraft:entity.arrow.hit").map(|cue| cue.id), Some(SOUND_BOW_HIT));
        assert_eq!(cue("minecraft:entity.player.burp").map(|cue| cue.id), Some(SOUND_BURP));
        assert_eq!(cue("minecraft:entity.splash_potion.throw"), Some(Cue { id: SOUND_THROW, extra: Extra::Player }));
        assert_eq!(cue("minecraft:entity.generic.eat"), None);
    }

    /// Разбор пакета звука: номер, место, данные, вид существа.
    fn read_sound(packet: &[u8]) -> (u32, (f32, f32, f32), i32, String) {
        let (id, body) = packet_id(packet).expect("пакет");
        let mut reader = In::new(body);
        let sound = reader.varint().expect("звук");
        let at = (f32::from_le_bytes(reader.take(4).unwrap().try_into().unwrap()), f32::from_le_bytes(reader.take(4).unwrap().try_into().unwrap()), f32::from_le_bytes(reader.take(4).unwrap().try_into().unwrap()));
        let extra = reader.zigzag32().expect("данные");
        let entity = reader.string().expect("существо");

        assert_eq!(id, LEVEL_SOUND_EVENT);
        assert_eq!(reader.take(1 + 1 + 8).map(<[u8]>::len), Some(10), "два флага и номер сущности");
        assert_eq!(reader.remaining(), 0);

        (sound, at, extra, entity)
    }

    #[test]
    fn door_sound_carries_the_block_and_the_place() {
        let packets = sound_packets("minecraft:block.wooden_door.open", 1.0, (2.5, 63.5, 0.5), Some(5321));

        assert_eq!(packets.len(), 1);
        assert_eq!(read_sound(&packets[0]), (SOUND_DOOR_OPEN, (2.5, 63.5, 0.5), 5321, String::new()));
        assert!(sound_packets("minecraft:block.stone.hit", 1.0, (0.0, 0.0, 0.0), None).is_empty());
    }

    #[test]
    fn attack_sound_names_the_player() {
        let packets = sound_packets("minecraft:entity.player.attack.strong", 1.0, (0.5, 63.0, 0.5), None);

        assert_eq!(read_sound(&packets[0]), (SOUND_ATTACK_STRONG, (0.5, 63.0, 0.5), -1, "minecraft:player".to_string()));
    }

    #[test]
    fn note_block_sends_an_event_then_the_note() {
        // Высота 2^((нота−12)/12): 1,0 — нота 12; арфа — инструмент 0.
        let packets = sound_packets("minecraft:block.note_block.pling", 2.0, (3.5, 63.5, -1.5), None);

        assert_eq!(packets.len(), 2);

        let (id, body) = packet_id(&packets[0]).expect("пакет");
        let mut reader = In::new(body);

        assert_eq!(id, crate::bedrock::session::BLOCK_EVENT);
        assert_eq!((reader.zigzag32(), reader.zigzag32(), reader.zigzag32()), (Some(3), Some(63), Some(-2)));
        assert_eq!((reader.zigzag32(), reader.zigzag32()), (Some(0), Some(24)));
        assert_eq!(read_sound(&packets[1]), (SOUND_NOTE, (3.0, 63.0, -2.0), (15 << 8) | 24, String::new()));
        assert_eq!(note_cue("", "harp", 1.0).map(|cue| cue.extra), Some(Extra::Note(0, 12)));
        assert_eq!(note_cue("imitate", "zombie", 1.0), None);
    }

    #[test]
    fn eating_is_completed_with_the_item_and_the_method() {
        let packet = completed_using_item(285, 1);
        let (id, body) = packet_id(&packet).expect("пакет");

        assert_eq!(id, 0x8e);
        assert_eq!(body, [0x1d, 0x01, 1, 0, 0, 0]);
    }

    #[test]
    fn broken_block_and_splash_show_particles() {
        let (_, body) = packet_id(&level_packets(1, 2, 3, 2001, 1)[0]).map(|(id, body)| (id, body.to_vec())).expect("пакет");
        let mut reader = In::new(&body);

        assert_eq!(reader.zigzag32(), Some(EVENT_PARTICLE_DESTROY));

        let splash = level_packets(1, 2, 3, 2002, 0x3c38c6);

        assert_eq!(splash.len(), 2);

        let (_, body) = packet_id(&splash[0]).expect("пакет");
        let mut reader = In::new(body);

        assert_eq!(reader.zigzag32(), Some(EVENT_PARTICLE_SPLASH));
        reader.take(12);
        assert_eq!(reader.zigzag32(), Some(0xff3c38c6u32 as i32));
        assert_eq!(read_sound(&splash[1]).0, SOUND_GLASS);
    }

    #[test]
    fn villagers_haggle() {
        assert_eq!(cue_for("minecraft:entity.villager.yes", 1.0).map(|cue| cue.id), Some(122));
        assert_eq!(cue_for("minecraft:entity.villager.no", 1.0).map(|cue| cue.id), Some(123));
    }
}
