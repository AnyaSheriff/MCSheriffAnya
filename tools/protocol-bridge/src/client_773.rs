//! Клиентский маршрут773 через общие typed codec774/776 и отдельные таблицы773.
//! Злость переводится по часам снимка моба; неизвестные формы закрыты.
//! Источник: client-chain-sources.json; равные Slot envelopes не заменяют codec.
use crate::{Direction, JavaVersion, ids::RegistryKind, items, wire::{Context, Cursor, Error, uint}};

pub fn configuration(body: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    crate::wire::translate_inner(crate::State::Configuration,Direction::Clientbound,crate::wire::INTERNAL,JavaVersion::V1_21_9,body,Context::default(),true)
}
pub fn clientbound<'a>(tracker: &mut crate::entities::Tracker, body: &'a [u8], context: Context) -> Result<std::borrow::Cow<'a,[u8]>,Error> {
    tracker.translate_inner(crate::wire::INTERNAL,JavaVersion::V1_21_9,body,context,true)
}
pub fn serverbound(body: &[u8],context: Context) -> Result<std::borrow::Cow<'_,[u8]>,Error> {
    crate::wire::translate_inner(crate::State::Play,Direction::Serverbound,JavaVersion::V1_21_9,crate::wire::INTERNAL,body,context,true)
}

fn slot(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, untrusted: bool, context: items::Context) -> Result<(), Error> {
    let (stack, count) = items::decode_packet_slot(from, input.rest, untrusted, context).map_err(Error::Item)?;
    input.take(count)?;
    out.extend(items::encode_packet_slot(to, stack.as_ref(), untrusted, context).map_err(Error::Item)?);
    Ok(())
}

/// Только соседний inventory/metadata/recipe/particle шаг. Неподдержанные объекты и
/// компоненты не заменяются молча, а generic wire/admission не открывается.
pub fn translate_play(direction: Direction, from: JavaVersion, body: &[u8], context: Context) -> Result<Vec<u8>, Error> {
    let to = match from {
        JavaVersion::V1_21_9 => JavaVersion::V1_21_11,
        JavaVersion::V1_21_11 => JavaVersion::V1_21_9,
        _ => return Err(Error::UnsupportedVersion(from)),
    };
    translate_selected(direction,from,to,body,context)
}

pub(crate) fn translate_selected(direction:Direction,from:JavaVersion,to:JavaVersion,body:&[u8],context:Context)->Result<Vec<u8>,Error> {
    if context.items.content.is_preview() { return Err(Error::PayloadUnavailable("773 adjacent wire content only")); }
    if body.len() > 8 * 1024 * 1024 { return Err(Error::TooLarge); }
    let mut input = Cursor::new(body);
    let kind = match direction { Direction::Clientbound => RegistryKind::PlayClientbound, Direction::Serverbound => RegistryKind::PlayServerbound };
    let id = input.uint()?;
    let info = crate::packet_layouts::packet_info(kind, from, to, id as usize).ok_or(Error::Invalid)?;
    let mut out = Vec::with_capacity(body.len());
    uint(&mut out, info.target_id.ok_or(crate::ids::IdTranslationError::MissingTarget(id))?);
    match (direction, info.name) {
        (Direction::Clientbound, "set_slot" | "window_items") => {
            uint(&mut out, input.varint()? as u32);
            uint(&mut out, input.uint()?);
            if info.name == "set_slot" {
                out.extend(input.take(2)?);
                slot(&mut input, &mut out, from, to, false, context.items)?;
            } else {
                let count = input.count(4096)?;
                uint(&mut out, count as u32);
                for _ in 0..count { slot(&mut input, &mut out, from, to, false, context.items)?; }
                slot(&mut input, &mut out, from, to, false, context.items)?;
            }
        }
        (Direction::Clientbound, "set_cursor_item" | "set_player_inventory") => {
            if info.name == "set_player_inventory" { uint(&mut out, input.uint()?); }
            slot(&mut input, &mut out, from, to, false, context.items)?;
        }
        (Direction::Clientbound, "entity_equipment") => {
            uint(&mut out, input.uint()?);
            let mut seen = 0u16;
            loop {
                let slot_id = input.take(1)?[0];
                let index = slot_id & 127;
                if index > 7 || seen & (1 << index) != 0 { return Err(Error::Invalid); }
                seen |= 1 << index;
                out.push(slot_id);
                slot(&mut input, &mut out, from, to, false, context.items)?;
                if slot_id & 128 == 0 { break; }
            }
        }
        (Direction::Serverbound, "set_creative_slot") => {
            out.extend(input.take(2)?);
            slot(&mut input, &mut out, from, to, true, context.items)?;
        }
        (Direction::Serverbound, "window_click") => crate::actions::click_with_context(&mut input, &mut out, from, to, context.items)?,
        (Direction::Serverbound, "enchant_item") => {
            uint(&mut out, input.uint()?);
            let selected = if from == JavaVersion::V1_21_9 { input.count(2)? as u8 } else {
                let value = input.take(1)?[0];
                if value > 2 { return Err(Error::Invalid); }
                value
            };
            // VarInt0..2 и i8 имеют одну и ту же однобайтовую запись.
            out.push(selected);
        }
        (Direction::Serverbound, "chat_command_signed") => {
            let command = input.string()?;
            uint(&mut out, command.len() as u32); out.extend(command.as_bytes());
            out.extend(input.take(16)?);
            let count = input.count(64)?; uint(&mut out, count as u32);
            for _ in 0..count {
                let name = input.string()?;
                uint(&mut out, name.len() as u32); out.extend(name.as_bytes());
                out.extend(input.take(256)?);
            }
            uint(&mut out, input.uint()?); out.extend(input.take(4)?);
        }
        (Direction::Clientbound, "entity_metadata") => crate::entities::metadata(&mut input, &mut out, from, to, context)?,
        (Direction::Clientbound, "trade_list") => crate::trades::translate(&mut input, &mut out, from, to, context.items)?,
        (Direction::Clientbound, "declare_recipes" | "recipe_book_add" | "craft_recipe_response" | "advancements") => crate::recipes::translate(info.name, &mut input, &mut out, from, to, context)?,
        (Direction::Clientbound, "world_particles") => crate::particles::world(&mut input, &mut out, from, to, context.items)?,
        (Direction::Clientbound, "explosion") => crate::actions::effects(info.name, &mut input, &mut out, from, to, context)?,
        _ => return Err(Error::PayloadUnavailable(info.name)),
    }
    if input.remaining() != 0 { return Err(Error::Invalid); }
    if out.len() > 8 * 1024 * 1024 { return Err(Error::TooLarge); }
    Ok(out)
}

#[cfg(test)]
#[path = "client_773_tests.rs"]
mod tests;
