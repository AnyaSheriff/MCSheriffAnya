//! Направленный перевод несжатых тел пакетов. Неизвестный payload не проходит
//! только потому, что совпало его имя. Источники: закреплённые protocol.json,
//! Wiki oldid=3848030 («Update Entity Position»); чужих реализаций здесь нет.
use std::borrow::Cow;
use crate::{Direction, JavaVersion, State};
use crate::ids::{self, RegistryKind};
use crate::movement::{Delta, Motion};

const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedVersion(JavaVersion),
    Invalid,
    Truncated,
    TooLarge,
    Id(ids::IdTranslationError),
    PayloadUnavailable(&'static str),
    Movement(crate::movement::Error),
    RegistryUnavailable(String),
}

impl From<ids::IdTranslationError> for Error {
    fn from(value: ids::IdTranslationError) -> Self { Self::Id(value) }
}
impl From<crate::movement::Error> for Error {
    fn from(value: crate::movement::Error) -> Self { Self::Movement(value) }
}

/// Внутренний формат пока 775. Версия мира и версия клиента не должны менять
/// смысл числовых ID, которые производят существующие игровые обработчики.
pub const INTERNAL: JavaVersion = JavaVersion::V26_1_2;

pub fn translate(
    state: State, direction: Direction, from: JavaVersion, to: JavaVersion,
    body: &[u8],
) -> Result<Cow<'_, [u8]>, Error> {
    for version in [from, to] {
        if version == JavaVersion::V26_2 { return Err(Error::UnsupportedVersion(version)); }
    }
    if body.is_empty() { return Err(Error::Truncated); }
    if body.len() > LIMIT { return Err(Error::TooLarge); }
    // Родной маршрут не копирует пакеты и сохраняет существующие проверки ядра.
    if from == to { return Ok(Cow::Borrowed(body)); }
    let registry = match (state, direction) {
        (State::Login, Direction::Clientbound) => RegistryKind::LoginClientbound,
        (State::Login, Direction::Serverbound) => RegistryKind::LoginServerbound,
        (State::Configuration, Direction::Clientbound) => RegistryKind::ConfigurationClientbound,
        (State::Configuration, Direction::Serverbound) => RegistryKind::ConfigurationServerbound,
        (State::Play, Direction::Clientbound) => RegistryKind::PlayClientbound,
        (State::Play, Direction::Serverbound) => RegistryKind::PlayServerbound,
    };
    let mut cursor = Cursor::new(body);
    let id = cursor.uint()?;
    let mapped = ids::translate(registry, from, to, id)?;
    let (name, equal) = crate::generated_ids::packet_info(registry, from == JavaVersion::V26_3, id as usize)
        .ok_or(Error::Invalid)?;
    let mut out = Vec::with_capacity(body.len() + 8);
    uint(&mut out, mapped);
    match (state, direction, name) {
        (State::Play, Direction::Clientbound, "rel_entity_move" | "entity_move_look") => {
            entity_move(&mut cursor, &mut out, from, to, name == "entity_move_look")?;
        }
        (State::Play, Direction::Clientbound, "block_change" | "block_action") => {
            let action = name == "block_action";
            out.extend(cursor.take(if action { 10 } else { 8 })?);
            let kind = if action { RegistryKind::Block } else { RegistryKind::BlockState };
            uint(&mut out, ids::translate(kind, from, to, cursor.uint()?)?);
        }
        (State::Play, Direction::Clientbound, "multi_block_change") => {
            out.extend(cursor.take(8)?);
            let count = cursor.count(4096)?;
            uint(&mut out, count as u32);
            for _ in 0..count {
                let record = cursor.uint()?;
                let state = ids::translate(RegistryKind::BlockState, from, to, record >> 12)?;
                uint(&mut out, (state << 12) | (record & 4095));
            }
        }
        (State::Play, Direction::Clientbound, "entity_update_attributes") => {
            uint(&mut out, cursor.uint()?);
            let count = cursor.count(1024)?;
            uint(&mut out, count as u32);
            for _ in 0..count {
                uint(&mut out, ids::translate(RegistryKind::Attribute, from, to, cursor.uint()?)?);
                out.extend(cursor.take(8)?);
                let modifiers = cursor.count(1024)?;
                uint(&mut out, modifiers as u32);
                for _ in 0..modifiers {
                    string(&mut out, cursor.string()?);
                    out.extend(cursor.take(8)?);
                    let operation = cursor.take(1)?[0];
                    if operation > 2 { return Err(Error::Invalid); }
                    out.push(operation);
                }
            }
        }
        (State::Configuration | State::Play, Direction::Clientbound, "tags") => {
            tags(&mut cursor, &mut out, from, to)?;
        }
        (State::Configuration, Direction::Clientbound, "select_known_packs") => {
            let count = cursor.count(1024)?;
            uint(&mut out, count as u32);
            for _ in 0..count {
                let namespace = cursor.string()?;
                let id = cursor.string()?;
                let version = cursor.string()?;
                string(&mut out, namespace);
                string(&mut out, id);
                string(&mut out, if namespace == "minecraft" && id == "core" && version == from.name() { to.name() } else { version });
            }
        }
        _ if equal && passthrough(state, direction, name) => {
            out.extend(cursor.take(cursor.remaining())?);
        }
        _ => return Err(Error::PayloadUnavailable(name)),
    }
    if cursor.remaining() != 0 { return Err(Error::Invalid); }
    if out.len() > LIMIT { return Err(Error::TooLarge); }
    Ok(Cow::Owned(out))
}

/// Здесь только пакеты без статических ID контента. Динамические ID сущностей,
/// контейнеров и отправленных самим сервером реестров принадлежат соединению.
/// Равенство полных схем проверяется дополнительно сгенерированной таблицей.
fn passthrough(state: State, direction: Direction, name: &str) -> bool {
    match (state, direction) {
        (State::Login, _) => false, // LoginSuccess требует нового sessionId от сервера.
        (State::Configuration, Direction::Clientbound) => matches!(name,
            "cookie_request" | "custom_payload" | "disconnect" | "finish_configuration" |
            "keep_alive" | "ping" | "reset_chat" | "remove_resource_pack" | "add_resource_pack" |
            "store_cookie" | "transfer" | "feature_flags" | "custom_report_details" |
            "server_links" | "clear_dialog" | "show_dialog" | "code_of_conduct"),
        (State::Configuration, Direction::Serverbound) => matches!(name,
            "settings" | "cookie_response" | "custom_payload" | "finish_configuration" |
            "keep_alive" | "pong" | "resource_pack_receive" | "select_known_packs" |
            "custom_click_action" | "accept_code_of_conduct"),
        (State::Play, Direction::Serverbound) => matches!(name,
            "attack" | "query_block_nbt" | "set_difficulty" | "change_gamemode" |
            "message_acknowledgement" | "chat_command" | "chat_command_signed" | "chat_message" |
            "chat_session_update" | "chunk_batch_received" | "client_command" | "tick_end" |
            "settings" | "tab_complete" | "configuration_acknowledged" | "close_window" |
            "cookie_response" | "custom_payload" | "query_entity_nbt" | "use_entity" |
            "keep_alive" | "lock_difficulty" | "position" | "position_look" | "look" |
            "flying" | "vehicle_move" | "steer_boat" | "ping_request" | "abilities" |
            "block_dig" | "entity_action" | "player_input" | "player_loaded" | "pong" |
            "name_item" | "resource_pack_receive" | "held_item_slot" | "update_sign" |
            "arm_animation" | "spectate" | "block_place" | "use_item" | "custom_click_action"),
        (State::Play, Direction::Clientbound) => matches!(name,
            "bundle_delimiter" | "animation" | "acknowledge_player_digging" | "block_break_animation" |
            "boss_bar" | "difficulty" | "chunk_batch_finished" | "chunk_batch_start" | "clear_titles" |
            "tab_complete" | "close_window" | "cookie_request" | "chat_suggestions" |
            "custom_payload" | "hide_message" | "kick_disconnect" | "entity_status" |
            "unload_chunk" | "game_state_change" | "hurt_animation" | "initialize_world_border" |
            "keep_alive" | "low_disk_space_warning" | "entity_look" | "vehicle_move" |
            "open_book" | "open_sign_entity" | "ping" | "ping_response" | "abilities" |
            "end_combat_event" | "enter_combat_event" | "death_combat_event" | "player_remove" |
            "face_player" | "position" | "player_rotation" | "recipe_book_settings" |
            "entity_destroy" | "reset_score" | "remove_resource_pack" | "add_resource_pack" |
            "entity_head_rotation" | "select_advancement_tab" | "server_data" | "action_bar" |
            "world_border_center" | "world_border_lerp_size" | "world_border_size" |
            "world_border_warning_delay" | "world_border_warning_reach" | "camera" |
            "update_view_position" | "update_view_distance" | "scoreboard_display_objective" |
            "attach_entity" | "entity_velocity" | "experience" | "update_health" | "held_item_slot" |
            "scoreboard_objective" | "set_passengers" | "teams" | "scoreboard_score" |
            "simulation_distance" | "set_title_subtitle" | "set_title_text" | "set_title_time" |
            "start_configuration" | "stop_sound" | "store_cookie" | "system_chat" |
            "playerlist_header" | "collect" | "entity_teleport" | "set_ticking_state" |
            "step_tick" | "transfer" | "set_projectile_power" | "custom_report_details" |
            "server_links" | "clear_dialog" | "show_dialog"),
    }
}

fn entity_move(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, rotation: bool) -> Result<(), Error> {
    uint(out, cursor.uint()?);
    let (delta, angles) = if from == JavaVersion::V26_1_2 {
        let bytes = cursor.take(6)?;
        let movement = [i16::from_be_bytes([bytes[0], bytes[1]]), i16::from_be_bytes([bytes[2], bytes[3]]), i16::from_be_bytes([bytes[4], bytes[5]])];
        // В старом Position+Rotation onGround стоит ПОСЛЕ yaw/pitch.
        let angles = if rotation { Some([cursor.take(1)?[0], cursor.take(1)?[0]]) } else { None };
        let on_ground = match cursor.take(1)?[0] { 0 => false, 1 => true, _ => return Err(Error::Invalid) };
        (Delta { on_ground, motion: Motion::Linear(movement) }, angles)
    } else {
        let (delta, used) = Delta::decode_prefix(from, cursor.rest)?;
        cursor.take(used)?;
        let angles = if rotation { Some([cursor.take(1)?[0], cursor.take(1)?[0]]) } else { None };
        (delta, angles)
    };
    let encoded = delta.encode(to)?;
    if to == JavaVersion::V26_1_2 && rotation {
        out.extend(&encoded[..6]);
        out.extend(angles.ok_or(Error::Invalid)?);
        out.push(encoded[6]);
    } else {
        out.extend(encoded);
        if let Some(angles) = angles { out.extend(angles); }
    }
    Ok(())
}

fn tags(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion) -> Result<(), Error> {
    let count = cursor.count(1024)?;
    uint(out, count as u32);
    for _ in 0..count {
        let name = cursor.string()?;
        string(out, name);
        let registry = match name {
            "minecraft:block" => Some(RegistryKind::Block),
            "minecraft:item" => Some(RegistryKind::Item),
            "minecraft:entity_type" => Some(RegistryKind::Entity),
            // Порядок этих динамических реестров задаёт само ядро в Registry Data.
            "minecraft:damage_type" | "minecraft:banner_pattern" | "minecraft:timeline" => None,
            _ => return Err(Error::RegistryUnavailable(name.into())),
        };
        let count = cursor.count(65_536)?;
        uint(out, count as u32);
        for _ in 0..count {
            string(out, cursor.string()?);
            let count = cursor.count(65_536)?;
            uint(out, count as u32);
            for _ in 0..count {
                let id = cursor.uint()?;
                uint(out, match registry { Some(kind) => ids::translate(kind, from, to, id)?, None => id });
            }
        }
    }
    Ok(())
}

struct Cursor<'a> { rest: &'a [u8] }
impl<'a> Cursor<'a> {
    fn new(rest: &'a [u8]) -> Self { Self { rest } }
    fn remaining(&self) -> usize { self.rest.len() }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self.rest.split_at_checked(n).ok_or(Error::Truncated)?;
        self.rest = rest;
        Ok(value)
    }
    fn uint(&mut self) -> Result<u32, Error> {
        let mut value = 0;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf8 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return Ok(value); }
        }
        Err(Error::Invalid)
    }
    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let value = self.uint()? as usize;
        if value > limit { return Err(Error::TooLarge); }
        Ok(value)
    }
    fn string(&mut self) -> Result<&'a str, Error> {
        let size = self.count(32_767)?;
        std::str::from_utf8(self.take(size)?).map_err(|_| Error::Invalid)
    }
}
fn uint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; }
    out.push(value as u8);
}
fn string(out: &mut Vec<u8>, value: &str) { uint(out, value.len() as u32); out.extend(value.as_bytes()); }

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};

    fn cb(body: &[u8], from: JavaVersion, to: JavaVersion) -> Result<Vec<u8>, Error> {
        translate(State::Play, Direction::Clientbound, from, to, body).map(Cow::into_owned)
    }
    #[test]
    fn rotation_and_ground_are_not_swapped_by_the_movement_adapter() {
        for rotation in [false, true] {
            for ground in [0, 1] {
                let mut old = vec![if rotation { 0x36 } else { 0x35 }, 0x82, 1, 0x80, 0, 0, 17, 0x7f, 0xff];
                if rotation { old.extend([0xfe, 0x32]); }
                old.push(ground);
                let mut expected = vec![if rotation { 0x37 } else { 0x36 }, 0x82, 1, ground, 0x80, 0, 0, 17, 0x7f, 0xff];
                if rotation { expected.extend([0xfe, 0x32]); }
                assert_eq!(cb(&old, OLD, NEW).unwrap(), expected);
                assert_eq!(cb(&expected, NEW, OLD).unwrap(), old);
                for n in 1..old.len() { assert!(cb(&old[..n], OLD, NEW).is_err()); }
                let mut extra = old.clone(); extra.push(0);
                assert_eq!(cb(&extra, OLD, NEW), Err(Error::Invalid));
            }
        }
        let mut timed = vec![0x36, 2];
        timed.extend(&include_bytes!("../testdata/entity-delta-26.3.bin")[..8]);
        assert_eq!(cb(&timed, NEW, OLD), Err(Error::Movement(crate::movement::Error::RequiresTimedDelivery)));
    }

    #[test]
    fn block_and_attribute_ids_translate_without_silent_fallback() {
        let mut body = vec![8]; body.extend([0; 8]); uint(&mut body, 1000);
        let mapped = cb(&body, OLD, NEW).unwrap();
        let mut expected = vec![8]; expected.extend([0; 8]);
        uint(&mut expected, ids::translate(RegistryKind::BlockState, OLD, NEW, 1000).unwrap());
        assert_eq!(mapped, expected);
        assert_eq!(cb(&mapped, NEW, OLD).unwrap(), body);
        let mut attributes = vec![0x83, 1, 1, 1, 22]; // packet131, entity1, count1, movement_speed22
        attributes.extend(0.1f64.to_be_bytes()); attributes.push(1);
        string(&mut attributes, "mcsa:test"); attributes.extend(0.5f64.to_be_bytes()); attributes.push(2);
        let mapped = cb(&attributes, OLD, NEW).unwrap();
        assert_eq!(cb(&mapped, NEW, OLD).unwrap(), attributes);
        attributes[4] = 20; // удалённый mining_efficiency: нельзя отправить как movement_speed.
        assert_eq!(cb(&attributes, OLD, NEW), Err(Error::Id(ids::IdTranslationError::MissingTarget(20))));
    }

    #[test]
    fn configuration_tags_translate_content_but_keep_owned_dynamic_ids() {
        let mut body = vec![13, 2];
        string(&mut body, "minecraft:item"); body.push(1); string(&mut body, "mcsa:probe"); body.push(1); uint(&mut body, 1400);
        string(&mut body, "minecraft:damage_type"); body.push(1); string(&mut body, "mcsa:damage"); body.extend([1, 9]);
        let mapped = translate(State::Configuration, Direction::Clientbound, OLD, NEW, &body).unwrap();
        assert_eq!(mapped[0], 14);
        assert_eq!(translate(State::Configuration, Direction::Clientbound, NEW, OLD, &mapped).unwrap(), body);
        let mut packs = vec![14, 1];
        for value in ["minecraft", "core", "26.1.2"] { string(&mut packs, value); }
        let mapped = translate(State::Configuration, Direction::Clientbound, OLD, NEW, &packs).unwrap();
        assert_eq!(mapped[0], 15); assert!(mapped.ends_with(b"26.3"));
    }

    #[test]
    fn native_bytes_borrow_but_unimplemented_cross_version_payloads_fail() {
        let body = [0x2c, 0, 0, 0, 0, 0, 0, 0, 7];
        assert!(matches!(translate(State::Play, Direction::Clientbound, OLD, OLD, &body), Ok(Cow::Borrowed(_))));
        assert_eq!(cb(&body, OLD, NEW).unwrap()[0], 0x2d);
        for (id, name) in [(0x2d, "map_chunk"), (0x14, "set_slot"), (0x63, "entity_metadata"), (0x31, "login")] {
            assert_eq!(cb(&[id], OLD, NEW), Err(Error::PayloadUnavailable(name)));
        }
        assert_eq!(cb(&[], OLD, NEW), Err(Error::Truncated));
        assert_eq!(cb(&[255; 5], OLD, NEW), Err(Error::Invalid));
        assert!(matches!(cb(&[0], JavaVersion::V26_2, NEW), Err(Error::UnsupportedVersion(_))));
    }
}
