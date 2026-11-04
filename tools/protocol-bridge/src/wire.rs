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
    MissingOnlineMode,
    MissingChunkContext,
    MissingEntityContext,
    Entity(crate::entities::Error),
    Item(crate::items::Error),
    Content(crate::content::Error),
    LegacyItem(crate::legacy_items::Error),
    Chunk(crate::chunk::Error),
    BlockEntity(crate::block_entities::Error),
    Light(crate::light::Error),
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

impl From<crate::content::Error> for Error {
    fn from(value: crate::content::Error) -> Self {
        match value { crate::content::Error::Id(error) => Self::Id(error), error => Self::Content(error) }
    }
}

/// Внутренний формат пока 775. Версия мира и версия клиента не должны менять
/// смысл числовых ID, которые производят существующие игровые обработчики.
pub const INTERNAL: JavaVersion = JavaVersion::V26_1_2;

#[derive(Clone, Copy, Default)]
pub struct Context {
    /// Последний подтверждённый wire772-вид статуи в той же точке.
    pub block_entity_view: Option<u32>,
    /// Часы снимка моба для абсолютной злости775 и оставшихся тактов773.
    pub entity_clock: Option<i64>,
    /// Последняя синхронизация часов для формата774 без clockUpdates.
    pub clock_774: Option<(i64,i64,bool)>,
    /// Настоящий server.properties online-mode; не результат проверки лицензии/AuthMe.
    pub online_mode: Option<bool>,
    pub chunk: Option<crate::chunk::Context>,
    pub items: crate::items::Context,
    /// Тип наблюдаемой сущности в пространстве775, установленный по AddEntity/Login.
    pub entity_kind: Option<u32>,
    /// Одинаковые порядок и содержимое динамических реестров обоих профилей.
    pub metadata_registry_counts: &'static [(&'static str, u32)],
}

pub fn translate(
    state: State, direction: Direction, from: JavaVersion, to: JavaVersion,
    body: &[u8],
) -> Result<Cow<'_, [u8]>, Error> {
    translate_with_context(state, direction, from, to, body, Context::default())
}

pub fn translate_with_context(
    state: State, direction: Direction, from: JavaVersion, to: JavaVersion,
    body: &[u8], context: Context,
) -> Result<Cow<'_, [u8]>, Error> {
    for version in [from, to] {
        if !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3) { return Err(Error::UnsupportedVersion(version)); }
    }
    translate_inner(state, direction, from, to, body, context, false)
}

pub(crate) fn translate_inner(
    state: State, direction: Direction, from: JavaVersion, to: JavaVersion,
    body: &[u8], context: Context, probe_776: bool,
) -> Result<Cow<'_, [u8]>, Error> {
    if probe_776 && !matches!((from,to),
        (JavaVersion::V26_1_2,JavaVersion::V26_2) | (JavaVersion::V26_2,JavaVersion::V26_1_2) | (JavaVersion::V26_1_2,JavaVersion::V1_21_11) | (JavaVersion::V1_21_11,JavaVersion::V26_1_2) | (JavaVersion::V26_1_2,JavaVersion::V1_21_4) | (JavaVersion::V1_21_4,JavaVersion::V26_1_2) | (JavaVersion::V26_1_2,JavaVersion::V1_21_5) | (JavaVersion::V1_21_5,JavaVersion::V26_1_2) | (JavaVersion::V26_1_2,JavaVersion::V1_21_6) | (JavaVersion::V26_1_2,JavaVersion::V1_21_8) | (JavaVersion::V1_21_6,JavaVersion::V26_1_2) | (JavaVersion::V1_21_8,JavaVersion::V26_1_2) | (JavaVersion::V26_1_2,JavaVersion::V1_21_9) | (JavaVersion::V1_21_9,JavaVersion::V26_1_2)) {
        return Err(Error::UnsupportedVersion(from));
    }
    if context.items.content.is_native_view() && (from != JavaVersion::V26_1_2 || to != JavaVersion::V26_1_2 || state != State::Play || direction != Direction::Clientbound) {
        return Err(Error::PayloadUnavailable("775 view is clientbound only"));
    }
    if context.items.content.is_preview() && !context.items.content.is_native_view() && (!probe_776 && !matches!((from,to),
        (JavaVersion::V26_1_2,JavaVersion::V26_3) | (JavaVersion::V26_3,JavaVersion::V26_1_2))) {
        return Err(Error::PayloadUnavailable("preview content requires explicit canonical/777 route"));
    }
    if body.is_empty() { return Err(Error::Truncated); }
    if body.len() > LIMIT { return Err(Error::TooLarge); }
    // Родной маршрут не копирует пакеты и сохраняет существующие проверки ядра.
    let native_view = from == to && context.items.content.is_native_view() && state == State::Play && direction == Direction::Clientbound;
    if from == to && !native_view { return Ok(Cow::Borrowed(body)); }
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
    // Сетевой blackbox777 через внешний ViaProxy: после Use Item On идёт
    // пустой Punch0x2e. Это взмах основной руки; отказ разрывал соединение
    // до отправки результата щелчка. Имена в таблице ID остаются разными.
    if state == State::Play && direction == Direction::Serverbound
        && from == JavaVersion::V26_3 && to == INTERNAL && id == 0x2e
    {
        if !cursor.rest.is_empty() { return Err(Error::Invalid); }
        return Ok(Cow::Owned(vec![0x3f, 0]));
    }
    let standard_id = if probe_776 { None } else { Some(ids::translate(registry, from, to, id)?) };
    let info = crate::packet_layouts::packet_info(registry, from, if native_view { JavaVersion::V26_3 } else { to }, id as usize).ok_or(Error::Invalid)?;
    let mapped = if probe_776 {
        if !probe_776_allowed(state, direction, info.name) { return Err(Error::PayloadUnavailable(info.name)); }
        info.target_id.ok_or(ids::IdTranslationError::MissingTarget(id))?
    } else { standard_id.ok_or(Error::Invalid)? };
    if !native_view { debug_assert_eq!(info.target_id, Some(mapped)); }
    let (name, equal) = (info.name, info.schema_equal);
    if native_view && !matches!(name, "trade_list" | "declare_recipes" | "recipe_book_add" | "craft_recipe_response" | "advancements" | "world_particles" | "spawn_entity" | "entity_metadata" | "set_slot" | "window_items" | "set_cursor_item" | "set_player_inventory" | "entity_equipment" | "map_chunk" | "tile_entity_data" | "block_change" | "block_action" | "multi_block_change" | "world_event") { return Ok(Cow::Borrowed(body)); }
    let mut out = Vec::with_capacity(body.len() + 8);
    uint(&mut out, mapped);
    match (state, direction, name) {
        (State::Play, Direction::Clientbound, "animation") if context.items.content.is_preview()
            && from == JavaVersion::V26_1_2 && to == JavaVersion::V26_3 => {
            let entity = cursor.uint()?;
            let kind = cursor.take(1)?[0];
            // Официальный777: cow-crit и cow-crit-fist, крит на +13 имеет
            // animation=1; pvp-sharpness имеет2 вместо внутреннего5.
            // Внутренний775 и прежний основной профиль сохраняют прежний enum.
            // bed-leave/bed-sleep-day777: взмах теперь отдельным пакетом
            // Swing Animation (ID из pinned JSON), пробуждение — animation0.
            // [проверить] Длительность взмаха при haste/mining_fatigue и вторая рука.
            if kind == 0 {
                out.clear();
                uint(&mut out, 0x7b);
                uint(&mut out, entity);
                out.extend_from_slice(&[0, 1, 6]);
            } else {
                uint(&mut out, entity);
                out.push(match kind { 2 => 0, 4 => 1, 5 => 2, other => other });
            }
        }
        (State::Play, Direction::Clientbound, "start_configuration") |
        (State::Play, Direction::Serverbound, "configuration_acknowledged") => {},
        (State::Play, Direction::Clientbound, "declare_commands") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5) => crate::client_770::commands(&mut cursor,&mut out)?,
        (State::Play, Direction::Clientbound, "difficulty") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5) => {out.push(cursor.count(3)? as u8);out.push(u8::from(cursor.boolean()?));}
        (State::Play, Direction::Clientbound, "game_state_change") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5) => {let reason=cursor.take(1)?[0];if reason>13{return Err(Error::Invalid);}out.push(reason);out.extend(cursor.take(4)?);}
        (State::Play, Direction::Clientbound, "recipe_book_settings") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5) => {for _ in 0..8 {out.push(u8::from(cursor.boolean()?));}}
        (State::Play, Direction::Clientbound, "player_info") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8) => {
            let flags=cursor.take(1)?[0];out.push(flags);
            let n=cursor.count(4096)?;uint(&mut out,n as u32);
            for _ in 0..n {
                out.extend(cursor.take(16)?);
                if flags&1!=0 {
                    string(&mut out,cursor.string()?);
                    let properties=cursor.count(256)?;uint(&mut out,properties as u32);
                    for _ in 0..properties {string(&mut out,cursor.string()?);string(&mut out,cursor.string()?);let signed=cursor.boolean()?;out.push(u8::from(signed));if signed{string(&mut out,cursor.string()?);}}
                }
                if flags&2!=0 {let present=cursor.boolean()?;out.push(u8::from(present));if present {out.extend(cursor.take(24)?);for _ in 0..2 {let n=cursor.count(8192)?;uint(&mut out,n as u32);out.extend(cursor.take(n)?);}}}
                if flags&4!=0 {uint(&mut out,cursor.count(3)? as u32);}
                if flags&8!=0 {out.push(u8::from(cursor.boolean()?));}
                if flags&16!=0 {uint(&mut out,cursor.uint()?);}
                if flags&32!=0 {let present=cursor.boolean()?;out.push(u8::from(present));if present {let n=crate::registry::nbt_length(cursor.rest).map_err(|_|Error::Invalid)?;let text=cursor.take(n)?;crate::items::plain_text(text).map_err(Error::Item)?;out.extend(text);}}
                if flags&128!=0 {uint(&mut out,cursor.uint()?);}
                if flags&64!=0 {out.push(u8::from(cursor.boolean()?));}
            }
        }
        (State::Play, Direction::Clientbound, "entity_velocity") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8) => {
            uint(&mut out,cursor.uint()?);let speed=crate::legacy_entities::velocity(&mut cursor)?;crate::legacy_entities::put_velocity(&mut out,speed)?;
        }
        (State::Play, Direction::Clientbound, "spawn_position") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8) => {
            cursor.string()?;out.extend(cursor.take(12)?);cursor.take(4)?;
        }
        (State::Play, Direction::Clientbound, "player_rotation") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8) => {
            for _ in 0..2 {out.extend(cursor.take(4)?);if cursor.boolean()? {return Err(Error::PayloadUnavailable("772 relative rotation"));}}
        }
        (State::Play, Direction::Clientbound, "open_book") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => uint(&mut out,cursor.count(1)? as u32),
        (State::Play, Direction::Serverbound, "chat_message") if from==JavaVersion::V1_21_4 => {
            string(&mut out,cursor.string()?);out.extend(cursor.take(16)?);
            let signed=cursor.boolean()?;out.push(u8::from(signed));if signed {out.extend(cursor.take(256)?);}
            uint(&mut out,cursor.uint()?);out.extend(cursor.take(3)?);out.push(0);
        }
        (State::Play, Direction::Serverbound, "chat_command_signed") if matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => {
            string(&mut out,cursor.string()?);out.extend(cursor.take(16)?);
            let n=cursor.count(64)?;uint(&mut out,n as u32);
            for _ in 0..n{string(&mut out,cursor.string()?);out.extend(cursor.take(256)?);}
            uint(&mut out,cursor.uint()?);out.extend(cursor.take(if from==JavaVersion::V1_21_4 {3}else{4})?);if from==JavaVersion::V1_21_4 {out.push(0);}
        }
        (State::Play, Direction::Serverbound, "use_entity") if matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => crate::client_774::interact(&mut cursor,&mut out)?,
        (State::Play, Direction::Clientbound, "update_time") if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => crate::client_774::time(&mut cursor,&mut out,context.clock_774)?,
        (State::Play, Direction::Serverbound, "arm_animation" | "client_command") if matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => uint(&mut out,cursor.count(1)? as u32),
        (State::Play, Direction::Serverbound, "entity_action") if matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => {uint(&mut out,cursor.uint()?);uint(&mut out,cursor.count(6)? as u32);uint(&mut out,cursor.count(100)? as u32);}
        (State::Play, Direction::Serverbound, "enchant_item") if matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11) => {uint(&mut out,cursor.uint()?);let n=if matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8|JavaVersion::V1_21_9) {cursor.count(2)? as u8}else{cursor.take(1)?[0]};if n>2{return Err(Error::Invalid);}uint(&mut out,u32::from(n));}

        (State::Play, Direction::Serverbound, "block_place" | "use_item") if probe_776 && (equal || matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11)) => {
            let hand = cursor.uint()?;
            if hand > 1 { return Err(Error::Invalid); }
            uint(&mut out, hand);
            if name == "block_place" {
                out.extend(cursor.take(8)?);
                let face = cursor.uint()?;
                if face > 5 { return Err(Error::Invalid); }
                uint(&mut out, face);
            } else { uint(&mut out, cursor.uint()?); }
            for _ in 0..if name == "block_place" { 3 } else { 2 } {
                let bytes = cursor.take(4)?;
                if !f32::from_be_bytes(bytes.try_into().map_err(|_| Error::Invalid)?).is_finite() { return Err(Error::Invalid); }
                out.extend(bytes);
            }
            if name == "block_place" {
                for _ in 0..2 { out.push(u8::from(cursor.boolean()?)); }
                uint(&mut out, cursor.uint()?);
            }
        }
        (State::Play, Direction::Serverbound, "use_entity") if probe_776 && equal => {
            uint(&mut out, cursor.uint()?);
            let hand = cursor.uint()?;
            if hand > 1 { return Err(Error::Invalid); }
            uint(&mut out, hand);
            let first = cursor.take(1)?[0]; out.push(first);
            if first != 0 {
                if first & 7 == 0 { return Err(Error::Invalid); }
                out.extend(cursor.take(5)?);
                if first & 4 != 0 { uint(&mut out, cursor.uint()?); }
            }
            out.push(u8::from(cursor.boolean()?));
        }
        (State::Play, Direction::Serverbound, "attack" | "client_command") if probe_776 && equal => {
            let value = cursor.uint()?;
            if name == "client_command" && value > 2 { return Err(Error::Invalid); }
            uint(&mut out, value);
        }
        (State::Play, Direction::Clientbound, "sync_entity_position" | "entity_teleport") if probe_776 && equal => {
            uint(&mut out, cursor.uint()?);
            for _ in 0..6 {
                let bytes = cursor.take(8)?;
                if !f64::from_be_bytes(bytes.try_into().map_err(|_| Error::Invalid)?).is_finite() { return Err(Error::Invalid); }
                out.extend(bytes);
            }
            for _ in 0..2 {
                let bytes = cursor.take(4)?;
                if !f32::from_be_bytes(bytes.try_into().map_err(|_| Error::Invalid)?).is_finite() { return Err(Error::Invalid); }
                out.extend(bytes);
            }
            if name == "entity_teleport" { out.extend(cursor.take(4)?); }
            out.push(u8::from(cursor.boolean()?));
        }
        (State::Play, Direction::Serverbound, "craft_recipe_request") if probe_776 && equal => {
            uint(&mut out, cursor.uint()?);
            uint(&mut out, cursor.uint()?);
            out.push(u8::from(cursor.boolean()?));
        }
        (State::Play, Direction::Serverbound, "set_beacon_effect" | "name_item" | "enchant_item" |
            "select_trade" | "set_slot_state" | "update_sign" | "select_bundle_item" |
            "edit_book" | "pick_item_from_block" | "pick_item_from_entity") if probe_776 && equal => {
            crate::actions::gui(name, &mut cursor, &mut out)?;
        }
        (State::Play, Direction::Serverbound, "block_dig") => {
            digging(&mut cursor, &mut out, from, to)?;
        }
        (State::Play, Direction::Serverbound, "window_click") => {
            crate::actions::click_with_context(&mut cursor,&mut out,from,to,context.items)?;
        }
        (State::Play, Direction::Clientbound, "trade_list") => {
            crate::trades::translate(&mut cursor,&mut out,from,to,context.items)?;
        }
        (State::Play, Direction::Clientbound, "open_window") => {
            uint(&mut out,cursor.uint()?);let menu=cursor.uint()?;
            // Обычные взаимодействия с блоками и жителем: menu/trading-observations.json.
            // Autocraft777: фактический OpenScreen menu7; для776 ещё не наблюдён.
            // wide777: двойной сундук5 и кафедра с книгой17.
            let observed_crafter = matches!(menu,5|7|17) && matches!((from, to),
                (JavaVersion::V26_1_2, JavaVersion::V26_3) | (JavaVersion::V26_3, JavaVersion::V26_1_2));
            if !matches!(menu,2|6|8..=16|18..=24) && !observed_crafter {
                return Err(Error::PayloadUnavailable("unverified menu type"));
            }
            uint(&mut out,menu);
            let n=crate::registry::nbt_length(cursor.rest).map_err(|_|Error::Invalid)?;out.extend(cursor.take(n)?);
        }
        (State::Play, Direction::Clientbound, "sound_effect" | "entity_sound_effect" | "explosion" | "world_event" | "entity_effect" | "remove_entity_effect" | "damage_event") => {
            crate::actions::effects(name,&mut cursor,&mut out,from,to,context)?;
        }
        (State::Play, Direction::Clientbound, "teams") if matches!((from,to),
            (JavaVersion::V26_1_2,JavaVersion::V26_3) | (JavaVersion::V26_3,JavaVersion::V26_1_2)) => {
            crate::actions::teams(&mut cursor,&mut out,from)?;
        }
        (State::Play, Direction::Clientbound, "declare_recipes" | "recipe_book_add" | "craft_recipe_response" | "advancements") => {
            crate::recipes::translate(name, &mut cursor, &mut out, from, to, context)?;
        }
        (State::Play, Direction::Clientbound, "world_particles") => {
            crate::particles::world(&mut cursor, &mut out, from, to, context.items)?;
        }
        (State::Play, Direction::Clientbound, "spawn_entity") => {
            if probe_776 { crate::entities::spawn_776(&mut cursor, &mut out, from, to, context.items.content)?; }
            else { crate::entities::spawn_with_content(&mut cursor, &mut out, from, to, context.items.content)?; }
        }
        (State::Play, Direction::Clientbound, "collect") if probe_776 => {
            for _ in 0..2 { uint(&mut out, cursor.uint()?); }
            let count = cursor.uint()?;
            if !(1..=127).contains(&count) { return Err(Error::Invalid); }
            uint(&mut out, count);
        }
        (State::Play, Direction::Clientbound, "entity_metadata") => {
            crate::entities::metadata(&mut cursor, &mut out, from, to, context)?;
        }
        (State::Play, Direction::Clientbound, "respawn") => {
            respawn(&mut cursor, &mut out, from, to)?;
        }
        (State::Play, Direction::Clientbound, "set_slot" | "window_items") => {
            uint(&mut out, cursor.varint()? as u32); // -1/-2 — специальные окна
            uint(&mut out, cursor.uint()?); // revision
            if name == "set_slot" {
                out.extend(cursor.take(2)?);
                item(&mut cursor, &mut out, from, to, false, context.items)?;
            } else {
                let count = cursor.count(4096)?; uint(&mut out, count as u32);
                for _ in 0..count { item(&mut cursor, &mut out, from, to, false, context.items)?; }
                item(&mut cursor, &mut out, from, to, false, context.items)?; // cursor
            }
        }
        (State::Play, Direction::Clientbound, "set_cursor_item" | "set_player_inventory") => {
            if name == "set_player_inventory" { uint(&mut out, cursor.uint()?); }
            item(&mut cursor, &mut out, from, to, false, context.items)?;
        }
        (State::Play, Direction::Clientbound, "entity_equipment") => {
            uint(&mut out, cursor.uint()?); let mut seen = 0u16;
            loop {
                let slot = cursor.take(1)?[0]; let index = slot & 127;
                if index > 7 || seen & (1 << index) != 0 { return Err(Error::Invalid); }
                seen |= 1 << index; out.push(slot);
                item(&mut cursor, &mut out, from, to, false, context.items)?;
                if slot & 128 == 0 { break; }
            }
        }
        (State::Play, Direction::Serverbound, "set_creative_slot") => {
            out.extend(cursor.take(2)?);
            item(&mut cursor, &mut out, from, to, true, context.items)?;
        }
        (State::Play, Direction::Clientbound, "map_chunk") => {
            let content = context.items.content;
            let context = context.chunk.ok_or(Error::MissingChunkContext)?;
            let payload = cursor.take(cursor.remaining())?;
            out.extend(if probe_776 {
                crate::chunk::translate_776_content(from, to, payload, context, content)
            } else { crate::chunk::translate_with_content(from, to, payload, context, content) }.map_err(Error::Chunk)?);
        }
        (State::Play, Direction::Clientbound, "tile_entity_data") => {
            let position=cursor.take(8)?;out.extend(position);
            let kind = cursor.uint()?;
            // В родном775 только горшок Preview несёт новую карту сторон.
            // Остальные блоки сохраняют прежний прямой путь без новых ограничений NBT.
            if native_view && kind != 42 { return Ok(Cow::Borrowed(body)); }
            let translated = crate::block_entities::translate_with_content(from, to, kind, cursor.take(cursor.remaining())?, context.items.content).map_err(Error::BlockEntity)?;
            if translated.is_none() && matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8) {
                // У визуального bookshelf/медного блока нет block entity.
                // Сначала валидируем прежний NBT, затем посылаем его явный вид.
                out.clear();
                let info=crate::packet_layouts::packet_info(RegistryKind::PlayClientbound,INTERNAL,to,8).ok_or(Error::Invalid)?;
                uint(&mut out,info.target_id.ok_or(Error::Invalid)?);out.extend(position);
                let state=if kind==48 {context.block_entity_view.ok_or(Error::PayloadUnavailable("772 statue view requires block state"))?}
                    else {let views=if to==JavaVersion::V1_21_4 {crate::generated_payload_769::REMOVED_BE_VIEWS}else if to==JavaVersion::V1_21_5 {crate::generated_payload_770::REMOVED_BE_VIEWS}else if to==JavaVersion::V1_21_6 {crate::generated_payload_771::REMOVED_BE_VIEWS}else{crate::generated_payload_772::REMOVED_BE_VIEWS};views.iter().find(|r|r.0==kind).ok_or(Error::Invalid)?.1};
                uint(&mut out,state);return Ok(Cow::Owned(out));
            }
            let (kind, data) = translated.ok_or(Error::PayloadUnavailable("removed block entity update"))?;
            uint(&mut out, kind);
            // Реальный776 требует compound в Update, хотя Chunk принимает End.
            // payload-actions-live775-v2: Expected non-null compound tag.
            if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_2) && data == [0] { out.extend([10,0]); }
            else { out.extend(data); }
        }
        (State::Play, Direction::Clientbound, "update_light") => {
            // Координаты — signed VarInt; копируем точные 32-битные значения.
            for _ in 0..2 { uint(&mut out, cursor.varint()? as u32); }
            let light = crate::light::Light::decode(from, cursor.take(cursor.remaining())?).map_err(Error::Light)?;
            out.extend(light.encode(to).map_err(Error::Light)?);
        }
        (State::Play, Direction::Clientbound, "login") => {
            play_login(&mut cursor, &mut out, from, to, context)?;
        }
        (State::Play, Direction::Clientbound, "rel_entity_move" | "entity_move_look") => {
            entity_move(&mut cursor, &mut out, from, to, name == "entity_move_look")?;
        }
        (State::Play, Direction::Clientbound, "entity_look") if from == JavaVersion::V26_3 || to == JavaVersion::V26_3 => {
            // Wiki Packets777/Update Entity Rotation и villager-claim:
            // On Ground теперь стоит до углов, а не после них.
            uint(&mut out, cursor.uint()?);
            let (angles, grounded) = if from == JavaVersion::V26_3 {
                let grounded = cursor.boolean()?;
                (cursor.take(2)?, grounded)
            } else {
                let angles = cursor.take(2)?;
                (angles, cursor.boolean()?)
            };
            if to == JavaVersion::V26_3 { out.push(u8::from(grounded)); }
            out.extend(angles);
            if to != JavaVersion::V26_3 { out.push(u8::from(grounded)); }
        }
        (State::Play, Direction::Clientbound, "block_change" | "block_action") => {
            let action = name == "block_action";
            out.extend(cursor.take(if action { 10 } else { 8 })?);
            let kind = if action { RegistryKind::Block } else { RegistryKind::BlockState };
            uint(&mut out, context.items.content.packet_id(kind, from, to, cursor.uint()?)?);
        }
        (State::Play, Direction::Clientbound, "multi_block_change") => {
            out.extend(cursor.take(8)?);
            let count = cursor.count(4096)?;
            uint(&mut out, count as u32);
            for _ in 0..count {
                let record = cursor.uint()?;
                let state = context.items.content.packet_id(RegistryKind::BlockState, from, to, record >> 12)?;
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
            tags(&mut cursor, &mut out, from, to, context.items.content)?;
        }
        (State::Configuration, Direction::Clientbound, "show_dialog") if probe_776 && equal => {
            // Собственный inline-dialog: реестровая ссылка здесь не допускается.
            let n = crate::registry::nbt_length(cursor.rest).map_err(|_| Error::Invalid)?;
            if cursor.rest.first() != Some(&10) { return Err(Error::Invalid); }
            out.extend(cursor.take(n)?);
        }
        (State::Configuration, Direction::Clientbound, "clear_dialog") if probe_776 && equal => {}
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

/// Только собственные ordinary-наблюдения: actual777-dig-observations.json.
/// Равная VarInt-схема не гарантирует равный смысл enum. Cancel/drop/swap проверены
/// отдельно: actual777-player-action-observations.json. Release/jab наблюдены
/// в actual777-use-action-observations.json; остальные закрыты, общего сдвига нет.
#[derive(Clone, Copy)]
enum DigAction { Start, Cancel, Finish, DropStack, DropOne, ReleaseUse, Swap, Jab }

fn digging(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion) -> Result<(), Error> {
    use JavaVersion::{V1_21_4 as Legacy, V1_21_5 as Previous, V1_21_6 as First, V1_21_8 as Lowest, V1_21_9 as Lower, V1_21_11 as Earlier, V26_1_2 as Old, V26_2 as Mid, V26_3 as New};
    let action = match (from, cursor.uint()?) {
        // official-actions-776: начало0 и завершение2 разрушили камень.
        // Остальные enum776 сняты обычным GUI: action-client-collector775-v3.
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid | New, 0) => DigAction::Start,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, 1) | (New, 2) => DigAction::Cancel,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, 2) | (New, 3) => DigAction::Finish,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, 3) | (New, 4) => DigAction::DropStack,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, 4) | (New, 5) => DigAction::DropOne,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, 5) | (New, 6) => DigAction::ReleaseUse,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, 6) | (New, 7) => DigAction::Swap,
        (Earlier | Old | Mid, 7) | (New, 8) => DigAction::Jab,
        _ => return Err(Error::PayloadUnavailable("unverified block_dig action")),
    };
    let status = match (to, action) {
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid | New, DigAction::Start) => 0,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, DigAction::Cancel) => 1,
        (New, DigAction::Cancel) => 2,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, DigAction::Finish) => 2,
        (New, DigAction::Finish) => 3,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, DigAction::DropStack) => 3,
        (New, DigAction::DropStack) => 4,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, DigAction::DropOne) => 4,
        (New, DigAction::DropOne) => 5,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, DigAction::ReleaseUse) => 5,
        (New, DigAction::ReleaseUse) => 6,
        (Legacy | Previous | First | Lowest | Lower | Earlier | Old | Mid, DigAction::Swap) => 6,
        (New, DigAction::Swap) => 7,
        (Earlier | Old | Mid, DigAction::Jab) => 7,
        (New, DigAction::Jab) => 8,
        _ => return Err(Error::UnsupportedVersion(to)),
    };
    let position = cursor.take(8)?;
    let face = cursor.take(1)?[0];
    if face > 5 { return Err(Error::Invalid); }
    let sequence = cursor.uint()?;
    uint(out, status);out.extend(position);out.push(face);uint(out, sequence);
    Ok(())
}

fn item(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion,
    untrusted: bool, context: crate::items::Context) -> Result<(), Error> {
    let (stack, size) = crate::items::decode_packet_slot(from, cursor.rest, untrusted, context).map_err(Error::Item)?;
    cursor.take(size)?;
    out.extend(crate::items::encode_packet_slot(to, stack.as_ref(), untrusted, context).map_err(Error::Item)?);
    Ok(())
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
            "select_bundle_item" | "enchant_item" | "set_slot_state" | "edit_book" |
            "pick_item_from_block" | "pick_item_from_entity" | "craft_recipe_request" |
            "recipe_book" | "displayed_recipe" | "advancement_tab" | "select_trade" |
            "update_command_block" | "update_command_block_minecart" | "update_jigsaw_block" | "update_structure_block" |
            "attack" | "query_block_nbt" | "set_difficulty" | "change_gamemode" |
            "message_acknowledgement" | "chat_command" | "chat_command_signed" | "chat_message" |
            "chat_session_update" | "chunk_batch_received" | "client_command" | "tick_end" |
            "settings" | "tab_complete" | "configuration_acknowledged" | "close_window" |
            "cookie_response" | "custom_payload" | "query_entity_nbt" | "use_entity" |
            "keep_alive" | "lock_difficulty" | "position" | "position_look" | "look" |
            "flying" | "vehicle_move" | "steer_boat" | "ping_request" | "abilities" |
            "entity_action" | "player_input" | "player_loaded" | "pong" |
            "name_item" | "resource_pack_receive" | "held_item_slot" | "update_sign" |
            "arm_animation" | "spectate" | "block_place" | "use_item" | "custom_click_action"),
        (State::Play, Direction::Clientbound) => matches!(name,
            "declare_commands" | "player_info" | "spawn_position" | "update_time" |
            "craft_progress_bar" | "set_cooldown" | "open_horse_window" | "recipe_book_remove" |
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

// Ограничения конкретной первой core-пробы. Равенство JSON дополнительно
// проверяется обычным dispatcher; содержательные ID идут через typed ветки.
// Никаких aliases776=775, непрозрачных RegistryData или silent skip.
fn probe_776_allowed(state: State, direction: Direction, name: &str) -> bool {
    // Остальные штатные ветки без статических ID проходят общий schema_equal.
    if state==State::Play && passthrough(state,direction,name){return true;}
    match (state, direction) {
        (State::Configuration, Direction::Clientbound) => matches!(name,
            "custom_payload" | "select_known_packs" | "tags" | "finish_configuration" | "disconnect" |
            "show_dialog" | "clear_dialog"),
        (State::Play, Direction::Serverbound) => matches!(name,
            "settings" | "custom_payload" | "keep_alive" | "pong" | "player_loaded" |
            "chunk_batch_received" | "tick_end" | "position" | "position_look" | "look" |
            "flying" | "player_input" | "abilities" | "entity_action" |
            "chat_command" | "chat_message" | "chat_session_update" | "message_acknowledgement" |
            "tab_complete" | "window_click" | "close_window" | "held_item_slot" |
            "recipe_book" | "displayed_recipe" | "craft_recipe_request" | "advancement_tab" | "arm_animation" |
            "set_beacon_effect" | "name_item" | "enchant_item" | "select_trade" | "set_slot_state" |
            "update_sign" | "select_bundle_item" | "edit_book" | "pick_item_from_block" | "pick_item_from_entity" |
            "attack" | "use_entity" | "client_command" | "block_place" | "use_item" | "block_dig" | "set_creative_slot"),
        (State::Play, Direction::Clientbound) => matches!(name,
            "login" | "position" | "abilities" | "declare_commands" | "declare_recipes" |
            "player_info" | "player_remove" | "entity_metadata" | "entity_destroy" |
            "spawn_entity" | "collect" | "rel_entity_move" | "entity_move_look" |
            "sync_entity_position" | "entity_teleport" | "entity_look" | "entity_head_rotation" |
            "entity_equipment" | "entity_velocity" | "animation" | "entity_status" |
            "damage_event" | "hurt_animation" | "death_combat_event" | "respawn" |
            "acknowledge_player_digging" | "block_break_animation" | "world_event" | "open_window" | "craft_progress_bar" |
            "initialize_world_border" | "set_ticking_state" | "step_tick" | "spawn_position" |
            "update_health" | "experience" | "game_state_change" | "update_time" |
            "update_view_position" | "update_view_distance" | "simulation_distance" |
            "map_chunk" | "update_light" | "unload_chunk" | "chunk_batch_start" |
            "chunk_batch_finished" | "window_items" | "set_slot" | "set_cursor_item" |
            "set_player_inventory" | "held_item_slot" | "close_window" |
            "keep_alive" | "ping" | "system_chat" | "tab_complete" | "kick_disconnect" |
            "recipe_book_settings" | "recipe_book_add" | "recipe_book_remove" | "craft_recipe_response" |
            "advancements" | "select_advancement_tab" | "entity_update_attributes" |
            "block_change" | "block_action" | "tile_entity_data" | "multi_block_change" | "sound_effect" | "entity_sound_effect" |
            "playerlist_header" | "trade_list" | "world_particles" | "explosion" | "entity_effect" | "remove_entity_effect"),
        _ => false,
    }
}

struct SpawnLayout {
    varint_modes: bool,
    join_online_mode: bool,
}

fn spawn_layout(version: JavaVersion) -> Result<SpawnLayout, Error> {
    match version {
        JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 => Ok(SpawnLayout { varint_modes: false, join_online_mode: false }),
        // Собственные Join/Respawn observations: 776 уже содержит onlineMode,
        // но режимы ещё byte/255(None), а dimensionType по-прежнему VarInt.
        // testdata/protocol-26.2-observations.json; публичный маршрут закрыт.
        JavaVersion::V26_2 => Ok(SpawnLayout { varint_modes: false, join_online_mode: true }),
        JavaVersion::V26_3 => Ok(SpawnLayout { varint_modes: true, join_online_mode: true }),
        _ => Err(Error::UnsupportedVersion(version)),
    }
}

/// Login Play: Wiki oldid3848030, закреплённые protocol.json и собственные
/// observations776. В 776/777 onlineMode идёт перед enforcesSecureChat.
/// Общие поля проверяем целиком:
/// добавлять байт к неразобранному пакету опасно при повреждённой структуре.
fn play_login(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, context: Context) -> Result<(), Error> {
    let source = spawn_layout(from)?;
    let target = spawn_layout(to)?;
    let common = cursor.rest;
    cursor.take(4)?; // entityId, не статический ID реестра
    cursor.boolean()?; // hardcore
    let worlds = cursor.count(1024)?;
    for _ in 0..worlds { cursor.string()?; }
    for _ in 0..3 { cursor.uint()?; } // maxPlayers, view, simulation
    for _ in 0..3 { cursor.boolean()?; } // debug, respawn screen, limited crafting
    out.extend(&common[..common.len() - cursor.remaining()]);
    spawn_info(cursor, out, from, to)?;
    let online = if source.join_online_mode { Some(cursor.boolean()?) } else { context.online_mode };
    let secure = cursor.boolean()?;
    if target.join_online_mode { out.push(u8::from(online.ok_or(Error::MissingOnlineMode)?)); }
    out.push(u8::from(secure));
    Ok(())
}

fn spawn_info(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion) -> Result<(), Error> {
    let source = spawn_layout(from)?;
    let target = spawn_layout(to)?;
    let common = cursor.rest;
    cursor.uint()?; // dimensionType — реестр, ранее отправленный этим сервером
    cursor.string()?; // dimension name
    cursor.take(8)?; // hashed seed
    out.extend(&common[..common.len() - cursor.remaining()]);
    // Wiki3848030 + probe_block_entities: JSON-кандидат ошибочно оставил
    // старые byte-поля. В777 оба режима VarInt; предыдущий =0(None) / ID+1.
    let mode = if source.varint_modes { cursor.uint()? } else { u32::from(cursor.take(1)?[0]) };
    if mode > 3 { return Err(Error::Invalid); }
    let previous = if source.varint_modes {
        match cursor.uint()? { 0 => None, value @ 1..=4 => Some(value - 1), _ => return Err(Error::Invalid) }
    } else {
        match cursor.take(1)?[0] { 255 => None, value @ 0..=3 => Some(u32::from(value)), _ => return Err(Error::Invalid) }
    };
    uint(out, mode); // 0..3 занимают один байт в обоих форматах.
    if target.varint_modes { uint(out, previous.map_or(0, |id| id + 1)); }
    else { out.push(previous.map_or(255, |id| id as u8)); }
    let common = cursor.rest;
    cursor.boolean()?; // debug world
    cursor.boolean()?; // flat world
    if cursor.boolean()? { cursor.string()?; cursor.take(8)?; } // death location
    cursor.uint()?; // portal cooldown
    cursor.varint()?; // sea level может быть отрицательным
    let common_len = common.len() - cursor.remaining();
    out.extend(&common[..common_len]);
    Ok(())
}

fn respawn(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion) -> Result<(), Error> {
    spawn_info(cursor, out, from, to)?;
    let flags = cursor.take(1)?[0];
    if flags & !3 != 0 { return Err(Error::Invalid); }
    out.push(flags);
    Ok(())
}

// 775/776: обычные i16 delta, затем yaw/pitch и onGround. 777: Delta с
// флагами/шагами, затем yaw/pitch. Не является общим alias протокола776.
fn move_angles_before_ground(version: JavaVersion) -> Result<bool, Error> {
    match version {
        JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 => Ok(true),
        JavaVersion::V26_3 => Ok(false),
        JavaVersion::V1_12_2 => Err(Error::UnsupportedVersion(version)),
    }
}

fn entity_move(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, rotation: bool) -> Result<(), Error> {
    let source_angles_before_ground = move_angles_before_ground(from)?;
    let target_angles_before_ground = move_angles_before_ground(to)?;
    uint(out, cursor.uint()?);
    let (delta, angles) = if source_angles_before_ground {
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
    if target_angles_before_ground && rotation {
        out.extend(&encoded[..6]);
        out.extend(angles.ok_or(Error::Invalid)?);
        out.push(encoded[6]);
    } else {
        out.extend(encoded);
        if let Some(angles) = angles { out.extend(angles); }
    }
    Ok(())
}

fn tags(cursor: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, content: crate::content::ContentCodec) -> Result<(), Error> {
    let count = cursor.count(1024)?;
    let destination=out;let mut groups=Vec::new();let out=&mut groups;let mut kept=count;
    for _ in 0..count {
        let start=out.len();
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
                uint(out, match registry { Some(kind) => content.packet_id(kind, from, to, id)?, None => id });
            }
        }
        if matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8|JavaVersion::V1_21_9) && name=="minecraft:timeline" {out.truncate(start);kept-=1;}
    }
    uint(destination,kept as u32);destination.extend(groups);
    Ok(())
}

pub(crate) struct Cursor<'a> { pub(crate) rest: &'a [u8] }
impl<'a> Cursor<'a> {
    pub(crate) fn new(rest: &'a [u8]) -> Self { Self { rest } }
    pub(crate) fn remaining(&self) -> usize { self.rest.len() }
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self.rest.split_at_checked(n).ok_or(Error::Truncated)?;
        self.rest = rest;
        Ok(value)
    }
    pub(crate) fn boolean(&mut self) -> Result<bool, Error> {
        match self.take(1)?[0] { 0 => Ok(false), 1 => Ok(true), _ => Err(Error::Invalid) }
    }
    pub(crate) fn varint(&mut self) -> Result<i32, Error> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf0 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return Ok(value as i32); }
        }
        Err(Error::Invalid)
    }
    pub(crate) fn uint(&mut self) -> Result<u32, Error> {
        self.varint()?.try_into().map_err(|_| Error::Invalid)
    }
    pub(crate) fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let value = self.uint()? as usize;
        if value > limit { return Err(Error::TooLarge); }
        Ok(value)
    }
    pub(crate) fn string(&mut self) -> Result<&'a str, Error> {
        let size = self.count(32_767)?;
        std::str::from_utf8(self.take(size)?).map_err(|_| Error::Invalid)
    }
}
pub(crate) fn uint(out: &mut Vec<u8>, mut value: u32) {
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
    fn native_preview_non_pot_block_updates_keep_their_direct_path() {
        let mut context = Context::default();
        context.items.content = crate::content::ContentCodec::white_slab_preview().native_view();
        // Непрозрачные данные другого блока не должны попасть в кодек горшка.
        let mut body = vec![0x06];
        body.extend([0; 8]);
        uint(&mut body, 1);
        body.extend([10, 12, 0, 1, b'x', 0, 0, 0, 1]);
        body.extend(7i64.to_be_bytes());
        body.push(0);
        assert!(matches!(translate_with_context(State::Play, Direction::Clientbound,
            OLD, OLD, &body, context).unwrap(), Cow::Borrowed(_)));
    }

    #[test]
    fn observed_double_chest_and_lectern_menus_roundtrip() {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../testdata/blackbox-menus-777.json")).unwrap();
        for row in fixture["menus"].as_array().unwrap() {
            let payload:Vec<u8>=row["payload_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
                .map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
            let mut old=vec![0x3b];old.extend(&payload);
            let new=cb(&old,OLD,NEW).unwrap();assert_eq!(&new[1..],payload);
            assert_eq!(cb(&new,NEW,OLD).unwrap(),old);
        }
    }

    #[test]
    fn observed_crafter_menu_roundtrips_without_opening_unobserved_menus_or_776() {
        // Настоящий OpenScreen official777, autocraft777-observation-02.
        let payload = b"\x01\x07\x0a\x08\x00\x09translate\x00\x11container.crafter\x00";
        let mut body = vec![0x3b]; body.extend(payload);
        let mapped = cb(&body, OLD, NEW).unwrap();
        assert_eq!(&mapped[1..], payload);
        assert_eq!(cb(&mapped, NEW, OLD).unwrap(), body);
        for menu in [0, 1, 3, 4, 25, 127] {
            let mut denied = body.clone(); denied[2] = menu;
            assert_eq!(cb(&denied, OLD, NEW), Err(Error::PayloadUnavailable("unverified menu type")));
        }
        assert_eq!(translate_inner(State::Play, Direction::Clientbound, OLD, JavaVersion::V26_2,
            &body, Context::default(), true), Err(Error::PayloadUnavailable("unverified menu type")));
        let mut truncated = body.clone(); truncated.pop();
        assert!(cb(&truncated, OLD, NEW).is_err());
        let mut trailing = body; trailing.push(0);
        assert!(cb(&trailing, OLD, NEW).is_err());
    }

    #[test]
    fn observed_light_data_translates_inside_negative_coordinate_updates() {
        use crate::light::Light;
        let cases: [(JavaVersion, &[u8], u8); 4] = [
            (OLD, include_bytes!("../testdata/light-26.1.2-placed.bin"), 14),
            (OLD, include_bytes!("../testdata/light-26.1.2-removed.bin"), 0),
            (NEW, include_bytes!("../testdata/light-26.3-placed.bin"), 14),
            (NEW, include_bytes!("../testdata/light-26.3-removed.bin"), 0),
        ];
        for (from, data, expected_level) in cases {
            let to = if from == OLD { NEW } else { OLD };
            let light = Light::decode(from, data).unwrap();
            let level = if light.empty_block.get(1).unwrap_or(&0) & 2 != 0 { 0 } else {
                assert_ne!(light.block[1] & 2, 0);
                let index = light.block[0].count_ones() + u32::from(light.block[1] & 1);
                light.block_arrays[index as usize][128] & 15
            };
            assert_eq!(level, expected_level);
            // Только обёртка синтетическая: payload наблюдался внутри MapChunk.
            let mut body = vec![if from == OLD { 0x30 } else { 0x31 }];
            body.extend([255, 255, 255, 255, 15, 255, 255, 255, 255, 15]);
            body.extend(data);
            let translated = cb(&body, from, to).unwrap();
            assert_eq!(translated[0], if to == OLD { 0x30 } else { 0x31 });
            assert_eq!(&translated[1..11], &body[1..11]);
            assert_eq!(Light::decode(to, &translated[11..]).unwrap(), light);
            assert_eq!(cb(&translated, to, from).unwrap(), body);
            for n in [1, 10, 12, body.len() - 1] { assert!(cb(&body[..n], from, to).is_err()); }
            let mut extra = body; extra.push(0);
            assert!(cb(&extra, from, to).is_err());
        }
    }

    #[test]
    fn actual_play_login_frames_translate_with_explicit_online_mode() {
        let old = include_bytes!("../testdata/play-login-26.1.2.bin");
        let new = include_bytes!("../testdata/play-login-26.3.bin");
        for original in [&old[..], &new[..]] {
            let from = if original[0] == 0x31 { OLD } else { NEW };
            let to = if from == OLD { NEW } else { OLD };
            let context = Context { online_mode: Some(false), ..Context::default() };
            let translated = translate_with_context(State::Play, Direction::Clientbound, from, to, original, context).unwrap();
            let restored = translate_with_context(State::Play, Direction::Clientbound, to, from, &translated, context).unwrap();
            assert_eq!(restored.as_ref(), original);
            for n in 1..original.len() {
                assert!(translate_with_context(State::Play, Direction::Clientbound, from, to, &original[..n], context).is_err());
            }
            let mut extra = original.to_vec(); extra.push(0);
            assert!(translate_with_context(State::Play, Direction::Clientbound, from, to, &extra, context).is_err());
        }
        assert_eq!(cb(old, OLD, NEW), Err(Error::MissingOnlineMode));
        for online in [false, true] {
            let mapped = translate_with_context(State::Play, Direction::Clientbound, OLD, NEW, old,
                Context { online_mode: Some(online), ..Context::default() }).unwrap();
            assert_eq!(mapped[0], 0x32);
            // Общие поля сохраняются, а previousMode меняет смысл (см. отдельный тест).
            assert_eq!(&mapped[1..6], &old[1..6]);
            assert_eq!(&mapped[mapped.len() - 2..], &[u8::from(online), 0]);
        }
        let mut bad = new.to_vec();
        let len = bad.len(); bad[len - 2] = 2;
        assert_eq!(cb(&bad, NEW, OLD), Err(Error::Invalid));
        let mut bad = new.to_vec(); bad[5] = 2; // hardcore — Bool
        assert_eq!(cb(&bad, NEW, OLD), Err(Error::Invalid));
        let mut bad = new.to_vec(); bad.splice(6..7, [0x81, 8]); // 1025 миров
        assert_eq!(cb(&bad, NEW, OLD), Err(Error::TooLarge));
    }

    struct ObservedSpawn {
        join: bool,
        body: Vec<u8>,
        previous_at: usize,
    }

    fn observed_776_spawn_packets() -> Vec<ObservedSpawn> {
        fn bytes(value: &serde_json::Value) -> Vec<u8> {
            let hex = value.as_str().unwrap();
            assert!(hex.len().is_multiple_of(2));
            (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect()
        }
        let observations: serde_json::Value = serde_json::from_str(
            include_str!("../testdata/protocol-26.2-observations.json")).unwrap();
        let session = &observations["sessions"];
        assert_eq!(session["version"], 776);
        let mut packets = vec![
            ObservedSpawn { join: true, body: bytes(&session["login_initial_hex"]), previous_at: 102 },
            ObservedSpawn { join: true, body: bytes(&session["login_after_switch_hex"]), previous_at: 102 },
        ];
        let respawns = session["respawns"].as_array().unwrap();
        assert_eq!(respawns.len(), 4);
        for (entry, previous_at) in respawns.iter().zip([32, 29, 31, 32]) {
            packets.push(ObservedSpawn { join: false, body: bytes(&entry["body"]), previous_at });
        }
        // Смещения относятся только к закреплённой записи, а не к общему codec.
        // Проверяем их независимо установленными парами mode/previous.
        for (packet, modes) in packets.iter().zip([[1, 255], [0, 1], [0, 1], [1, 0], [3, 1], [2, 3]]) {
            assert_eq!(packet.body[0], if packet.join { 0x31 } else { 0x52 });
            assert_eq!(packet.body[packet.previous_at - 1..=packet.previous_at], modes);
        }
        packets
    }

    /// Только payload: таблицы ID/реестров776 этим helper не включаются.
    fn spawn_payload(join: bool, body: &[u8], from: JavaVersion, to: JavaVersion, online_mode: Option<bool>) -> Result<Vec<u8>, Error> {
        let mut cursor = Cursor::new(body);
        let mut out = Vec::new();
        if join {
            play_login(&mut cursor, &mut out, from, to, Context { online_mode, ..Default::default() })?;
        } else {
            respawn(&mut cursor, &mut out, from, to)?;
        }
        if cursor.remaining() != 0 { return Err(Error::Invalid); }
        Ok(out)
    }

    /// Ожидаемое представление наблюдённого776 payload. 775/777 здесь
    /// синтетические проекции полей, не записи других серверов.
    fn expected_spawn(packet: &ObservedSpawn, version: JavaVersion) -> Vec<u8> {
        let mut payload = packet.body[1..].to_vec();
        if version == NEW {
            let previous = &mut payload[packet.previous_at - 1];
            *previous = if *previous == 255 { 0 } else { *previous + 1 };
        } else if version == OLD && packet.join {
            assert_eq!(payload.remove(payload.len() - 2), 0); // observed onlineMode=false
        }
        payload
    }

    #[test]
    fn observed_776_join_and_respawn_fields_translate_between_all_three_private_layouts() {
        let middle = JavaVersion::V26_2;
        for packet in observed_776_spawn_packets() {
            for from in [OLD, middle, NEW] {
                let source = expected_spawn(&packet, from);
                for to in [OLD, middle, NEW] {
                    // Для776/777 источник поля — пакет, даже если context иной.
                    let context = Some(from != OLD);
                    assert_eq!(spawn_payload(packet.join, &source, from, to, context).unwrap(),
                        expected_spawn(&packet, to), "{from:?}→{to:?}, join={}", packet.join);
                    for length in 0..source.len() {
                        assert!(spawn_payload(packet.join, &source[..length], from, to, context).is_err());
                    }
                    let mut extra = source.clone(); extra.push(0);
                    assert_eq!(spawn_payload(packet.join, &extra, from, to, context), Err(Error::Invalid));
                }
            }
        }
        // Настоящие прежние775/777 записи тоже проходят новый средний helper.
        for (from, body) in [
            (OLD, include_bytes!("../testdata/play-login-26.1.2-none.bin").as_slice()),
            (OLD, include_bytes!("../testdata/play-login-26.1.2-previous-creative.bin").as_slice()),
            (NEW, include_bytes!("../testdata/play-login-26.3-none.bin").as_slice()),
            (NEW, include_bytes!("../testdata/play-login-26.3-previous-creative.bin").as_slice()),
        ] {
            let converted = spawn_payload(true, &body[1..], from, middle, Some(false)).unwrap();
            assert_eq!(spawn_payload(true, &converted, middle, from, None).unwrap(), body[1..]);
        }
    }

    #[test]
    fn private_776_online_mode_requires_source_or_context_and_valid_booleans() {
        let middle = JavaVersion::V26_2;
        for packet in observed_776_spawn_packets().into_iter().filter(|packet| packet.join) {
            let old = expected_spawn(&packet, OLD);
            for to in [middle, NEW] {
                assert_eq!(spawn_payload(true, &old, OLD, to, None), Err(Error::MissingOnlineMode));
            }
            for from in [middle, NEW] {
                let mut source = expected_spawn(&packet, from);
                for online in [false, true] {
                    // online=true — синтетическая проверка Bool, не наблюдение
                    // настоящего online-mode сервера в этом наборе.
                    let at = source.len() - 2; source[at] = u8::from(online);
                    for to in [middle, NEW] {
                        let mapped = spawn_payload(true, &source, from, to, Some(!online)).unwrap();
                        assert_eq!(mapped[mapped.len() - 2], u8::from(online));
                    }
                    assert_eq!(spawn_payload(true, &source, from, OLD, None).unwrap(), old);
                }
                for at in [source.len() - 2, source.len() - 1] {
                    let mut invalid = source.clone(); invalid[at] = 2;
                    assert_eq!(spawn_payload(true, &invalid, from, OLD, None), Err(Error::Invalid));
                }
            }
        }
    }

    #[test]
    fn private_spawn_layout_checks_modes_flags_and_varint_dimension_type() {
        for packet in observed_776_spawn_packets() {
            for from in [OLD, JavaVersion::V26_2, NEW] {
                let source = expected_spawn(&packet, from);
                for (at, value) in [(packet.previous_at - 2, 4), (packet.previous_at - 1, if from == NEW { 5 } else { 254 })] {
                    let mut invalid = source.clone(); invalid[at] = value;
                    assert_eq!(spawn_payload(packet.join, &invalid, from, OLD, Some(false)), Err(Error::Invalid));
                }
                if !packet.join {
                    let mut invalid = source.clone();
                    *invalid.last_mut().unwrap() = 4;
                    assert_eq!(spawn_payload(false, &invalid, from, OLD, None), Err(Error::Invalid));
                    // Синтетический VarInt131 проверяет ширину поля; это не
                    // заявление о наличии такого dimensionType в registry.
                    let mut wide_dimension = source;
                    wide_dimension.splice(..1, [0x83, 1]);
                    let mapped = spawn_payload(false, &wide_dimension, from, JavaVersion::V26_2, None).unwrap();
                    assert_eq!(&mapped[..2], &[0x83, 1]);
                    assert_eq!(spawn_payload(false, &mapped, JavaVersion::V26_2, from, None).unwrap(), wide_dimension);
                }
            }
        }
    }

    #[test]
    fn private_776_spawn_helpers_do_not_open_the_public_protocol_route() {
        let middle = JavaVersion::V26_2;
        for from in [OLD, middle, NEW] {
            for to in [OLD, middle, NEW] {
                if from != middle && to != middle { continue; }
                for state in [State::Login, State::Configuration, State::Play] {
                    for direction in [Direction::Clientbound, Direction::Serverbound] {
                        assert_eq!(translate_with_context(state, direction, from, to, &[0],
                            Context { online_mode: Some(false), ..Default::default() }),
                            Err(Error::UnsupportedVersion(middle)));
                    }
                }
            }
        }
        for join in [false, true] {
            assert_eq!(spawn_payload(join, &[], JavaVersion::V1_12_2, middle, None),
                Err(Error::UnsupportedVersion(JavaVersion::V1_12_2)));
            assert_eq!(spawn_payload(join, &[], middle, JavaVersion::V1_12_2, None),
                Err(Error::UnsupportedVersion(JavaVersion::V1_12_2)));
        }
    }

    #[test]
    fn block_entity_updates_translate_types_and_keep_checked_nbt() {
        for (old_kind, new_kind, nbt) in [(27u8,26u8,&[0][..]), (7,7,&include_bytes!("../testdata/oak_sign-26.1.2.nbt")[..])] {
            let mut old = vec![6]; old.extend([255;8]); old.push(old_kind); old.extend(nbt);
            let mut expected = old.clone(); expected[9] = new_kind;
            assert_eq!(cb(&old, OLD, NEW).unwrap(),expected);
            assert_eq!(cb(&expected, NEW, OLD).unwrap(),old);
        }
        let mut bed = vec![6]; bed.extend([0;8]); bed.extend([25,0]);
        assert_eq!(cb(&bed,OLD,NEW),Err(Error::PayloadUnavailable("removed block entity update")));
    }

    #[test]
    fn previous_game_mode_uses_optional_varint_in_777_not_the_json_candidate_byte() {
        // Минимальный Login с единственным измерением m:o; expected строится
        // независимо от codec и проверяет все5 возможных предыдущих режимов.
        let mut prefix = vec![0x31, 0, 0, 0, 1, 0, 0, 20, 2, 2, 0, 1, 0, 0, 3, b'm', b':', b'o'];
        prefix.extend([0; 8]); prefix.push(1);
        for previous in [255u8, 0, 1, 2, 3] {
            let mut old = prefix.clone(); old.push(previous); old.extend([0, 0, 0, 0, 63, 0]);
            let mut expected = prefix.clone(); expected[0] = 0x32;
            expected.push(if previous == 255 { 0 } else { previous + 1 });
            expected.extend([0, 0, 0, 0, 63, 0, 0]);
            let new = translate_with_context(State::Play, Direction::Clientbound, OLD, NEW, &old,
                Context { online_mode: Some(false), ..Default::default() }).unwrap();
            assert_eq!(new.as_ref(), expected);
            assert_eq!(cb(&expected, NEW, OLD).unwrap(), old);
            let mut invalid = expected.clone(); invalid[prefix.len()] = 5;
            assert_eq!(cb(&invalid, NEW, OLD), Err(Error::Invalid));
            let mut extended = expected; extended.splice(prefix.len()..prefix.len()+1, [0x80 | if previous == 255 { 0 } else { previous + 1 }, 0]);
            assert_eq!(cb(&extended, NEW, OLD).unwrap(), old);
        }
        for data in [include_bytes!("../testdata/play-login-26.3-none.bin").as_slice(), include_bytes!("../testdata/play-login-26.3-previous-creative.bin").as_slice()] {
            let old = cb(data, NEW, OLD).unwrap();
            let restored = translate_with_context(State::Play, Direction::Clientbound, OLD, NEW, &old,
                Context { online_mode: Some(false), ..Default::default() }).unwrap();
            assert_eq!(restored.as_ref(), data);
        }
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
        assert_eq!(mapped[4],26); // movement_speed по официальному наблюдению.
        attributes[4] = 20; // mining_efficiency существует:20→24.
        assert_eq!(cb(&attributes,OLD,NEW).unwrap()[4],24);
        let mut new_attribute=mapped;new_attribute[4]=0; // Новый air_drag_modifier.
        assert_eq!(cb(&new_attribute, NEW, OLD), Err(Error::Id(ids::IdTranslationError::MissingTarget(0))));
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
        assert_eq!(cb(&[0x2d], OLD, NEW), Err(Error::MissingChunkContext));
        assert_eq!(cb(&[0x63], OLD, NEW), Err(Error::MissingEntityContext));
        assert_eq!(cb(&[], OLD, NEW), Err(Error::Truncated));
        assert_eq!(cb(&[255; 5], OLD, NEW), Err(Error::Invalid));
        assert!(matches!(cb(&[0], JavaVersion::V26_2, NEW), Err(Error::UnsupportedVersion(_))));
    }
}

#[cfg(test)]
#[path = "payload_776_tests.rs"]
mod payload_776_tests;

#[cfg(test)]
#[path = "initial_play_776_tests.rs"]
mod initial_play_776_tests;

#[cfg(test)]
#[path = "dig_action_tests.rs"]
mod dig_action_tests;

#[cfg(test)]
mod combat_777_tests {
    use super::*;

    #[test]
    fn observed_rotation_777_places_ground_before_angles() {
        const OLD: JavaVersion = JavaVersion::V26_1_2;
        const NEW: JavaVersion = JavaVersion::V26_3;
        let cb = |body:&[u8],from,to| translate(State::Play,Direction::Clientbound,from,to,body).map(|packet|packet.into_owned());
        let old = [0x38, 19, 0, 28, 1];
        let observed = [0x39, 19, 1, 0, 28];
        assert_eq!(cb(&old, OLD, NEW).unwrap(), observed);
        assert_eq!(cb(&observed, NEW, OLD).unwrap(), old);
        for n in 0..observed.len() {
            assert!(cb(&observed[..n], NEW, OLD).is_err());
        }
        assert!(cb(&[0x39,19,2,0,28], NEW, OLD).is_err());
    }

    #[test]
    fn observed_critical_animation_changes_only_for_the_263_primary_view() {
        let legacy = [0x02, 7, 4];
        let context = Context {
            items: crate::items::Context {
                content: crate::content::ContentCodec::white_slab_preview(),
                ..Default::default()
            },
            ..Default::default()
        };
        let modern = translate_with_context(State::Play, Direction::Clientbound,
            JavaVersion::V26_1_2, JavaVersion::V26_3, &legacy, context).unwrap();
        assert_eq!(modern.as_ref(), &[0x02, 7, 1]);
        // Прежний основной профиль не меняет ни775, ни его777-представление.
        for client in [JavaVersion::V26_1_2, JavaVersion::V26_3] {
            assert_eq!(translate(State::Play, Direction::Clientbound,
                JavaVersion::V26_1_2, client, &legacy).unwrap().as_ref(), legacy);
        }
        for end in 0..legacy.len() {
            assert!(translate_with_context(State::Play, Direction::Clientbound,
                JavaVersion::V26_1_2, JavaVersion::V26_3, &legacy[..end], context).is_err());
        }
        assert!(translate_with_context(State::Play, Direction::Clientbound,
            JavaVersion::V26_1_2, JavaVersion::V26_3, &[0x02,7,4,0], context).is_err());
    }
}

#[cfg(test)]
mod combat_777_enchanted_animation_tests {
    use super::*;

    #[test]
    fn observed_bed_swings_and_wake_use_separate_777_packets() {
        let context = Context {
            items: crate::items::Context {
                content: crate::content::ContentCodec::white_slab_preview(),
                ..Default::default()
            },
            ..Default::default()
        };
        // Сырые bed-leave777: entity1, swing 01000106 и wake 0100.
        for (legacy, observed) in [
            (&[0x02, 1, 0][..], &[0x7b, 1, 0, 1, 6][..]),
            (&[0x02, 1, 2][..], &[0x02, 1, 0][..]),
        ] {
            assert_eq!(translate_with_context(State::Play, Direction::Clientbound,
                JavaVersion::V26_1_2, JavaVersion::V26_3, legacy, context).unwrap().as_ref(), observed);
            for client in [JavaVersion::V26_1_2, JavaVersion::V26_3] {
                assert_eq!(translate(State::Play, Direction::Clientbound,
                    JavaVersion::V26_1_2, client, legacy).unwrap().as_ref(), legacy);
            }
            assert!(translate_with_context(State::Play, Direction::Clientbound,
                JavaVersion::V26_1_2, JavaVersion::V26_3, &legacy[..2], context).is_err());
            let mut extra = legacy.to_vec();
            extra.push(0);
            assert!(translate_with_context(State::Play, Direction::Clientbound,
                JavaVersion::V26_1_2, JavaVersion::V26_3, &extra, context).is_err());
        }
    }

    #[test]
    fn observed_sharpness_animation_uses_two_only_for_the_263_primary() {
        let legacy = [0x02, 7, 5];
        let context = Context {
            items: crate::items::Context {
                content: crate::content::ContentCodec::white_slab_preview(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(translate_with_context(State::Play, Direction::Clientbound,
            JavaVersion::V26_1_2, JavaVersion::V26_3, &legacy, context).unwrap().as_ref(), &[0x02,7,2]);
        for client in [JavaVersion::V26_1_2,JavaVersion::V26_3] {
            assert_eq!(translate(State::Play,Direction::Clientbound,
                JavaVersion::V26_1_2,client,&legacy).unwrap().as_ref(),legacy);
        }
    }

    #[test]
    fn observed_777_punch_becomes_a_main_hand_swing_without_aliasing_ids() {
        assert_eq!(translate(State::Play, Direction::Serverbound,
            JavaVersion::V26_3, INTERNAL, &[0x2e]).unwrap().as_ref(), &[0x3f, 0]);
        assert_eq!(translate(State::Play, Direction::Serverbound,
            JavaVersion::V26_3, INTERNAL, &[0x2e, 0]), Err(Error::Invalid));
        // Старый основной маршрут сохраняет обе руки без нового толкования.
        for hand in [0, 1] {
            let body = [0x3f, hand];
            assert_eq!(translate(State::Play, Direction::Serverbound,
                INTERNAL, INTERNAL, &body).unwrap().as_ref(), body);
        }
        assert_eq!(crate::packet_layouts::packet_info(RegistryKind::PlayServerbound,
            JavaVersion::V26_3, INTERNAL, 0x2e).unwrap().target_id, None);
    }
}
