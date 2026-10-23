//! Появление и metadata сущностей775↔777. Индексы взяты только из pinned JSON;
//! значения проверяются отдельными наблюдениями официальных серверов.
use std::{borrow::Cow, collections::BTreeMap};
use crate::{JavaVersion, Direction, State, ids::{self, RegistryKind}, entity_layouts, items, registry};
use crate::wire::{self, Cursor, uint};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnknownEntity(u32), UnknownKind(u32), UnknownField(u8), MissingField(&'static str), Serializer(u32), MissingRegistry(&'static str) }

fn failure(error: Error) -> wire::Error { wire::Error::Entity(error) }

/// Учёт принадлежит одному соединению. Изменяется только после успешного
/// перевода целого пакета; неизвестный entityId не считается свиньёй/игроком.
#[derive(Default)]
pub struct Tracker {
    entities: BTreeMap<u32, u32>,
    player: Option<u32>,
    dimension: Option<String>,
    clock_774: Option<(i64,i64,bool)>,
}

fn supported_payload(from: JavaVersion, to: JavaVersion) -> Result<(), wire::Error> {
    for version in [from, to] {
        if !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3) {
            return Err(wire::Error::UnsupportedVersion(version));
        }
    }
    Ok(())
}

impl Tracker {
    /// Перевод может требовать нескольких пакетов: абсолютная позиция777
    /// отделена от скорости. Родной маршрут остаётся одним borrowed-пакетом.
    pub fn translate_packets<'a>(&mut self,from:JavaVersion,to:JavaVersion,body:&'a [u8],context:wire::Context)->Result<Vec<Cow<'a,[u8]>>,wire::Error>{
        supported_payload(from, to)?;
        if from==JavaVersion::V26_1_2 && to==JavaVersion::V26_3 && body.first()==Some(&0x23){
            return Ok(crate::entity_sync::upgrade(body)?.into_iter().map(Cow::Owned).collect());
        }
        Ok(vec![self.translate(from,to,body,context)?])
    }

    pub fn translate<'a>(&mut self, from: JavaVersion, to: JavaVersion, body: &'a [u8], context: wire::Context) -> Result<Cow<'a, [u8]>, wire::Error> {
        supported_payload(from, to)?;
        self.translate_inner(from, to, body, context, false)
    }

    pub(crate) fn translate_inner<'a>(&mut self, from: JavaVersion, to: JavaVersion, body: &'a [u8], mut context: wire::Context, probe_776: bool) -> Result<Cow<'a, [u8]>, wire::Error> {
        if from == to && !context.items.content.is_native_view() { return wire::translate_with_context(State::Play, Direction::Clientbound, from, to, body, context); }
        let mut input = Cursor::new(body);
        let packet = input.uint()?;
        if !probe_776 { ids::translate(RegistryKind::PlayClientbound, from, to, packet)?; }
        let name = crate::packet_layouts::packet_info(RegistryKind::PlayClientbound, from, if context.items.content.is_native_view() { JavaVersion::V26_3 } else { to }, packet as usize).ok_or(wire::Error::Invalid)?.name;
        let mut spawn = None;
        let mut removed = Vec::new();
        let mut dimension = None;
        let mut player = None;
        match name {
            "spawn_entity" => {
                let id = input.uint()?; input.take(16)?;
                let raw = input.uint()?;
                let kind = match context.items.content.entity_layout_base(raw).filter(|_| from == wire::INTERNAL) {
                    Some(base) => entity_layouts::LAYOUTS.iter().position(|entry| entry.0 == base).expect("раскладка лодки") as u32,
                    None => ids::translate(RegistryKind::Entity, from, wire::INTERNAL, raw)?,
                };
                if !self.entities.contains_key(&id) && self.entities.len() >= 65_536 { return Err(wire::Error::TooLarge); }
                if probe_776 {
                    let target = if entity_layouts::LAYOUTS[kind as usize].0 == "cushion" {
                        entity_layouts::LAYOUTS.iter().position(|row| row.0 == "armor_stand").ok_or(wire::Error::Invalid)? as u32
                    } else if to==JavaVersion::V1_21_11 && entity_layouts::LAYOUTS[kind as usize].0=="sulfur_cube" {entity_layouts::LAYOUTS.iter().position(|row|row.0=="slime").ok_or(wire::Error::Invalid)? as u32} else { kind };
                    metadata_layout(to, target)?;
                }
                spawn = Some((id, kind));
            }
            "entity_metadata" => {
                let id = input.uint()?;
                context.entity_kind = Some(*self.entities.get(&id).ok_or_else(|| failure(Error::UnknownEntity(id)))?);
            }
            "collect" if probe_776 => {
                let item = input.uint()?;
                let collector = input.uint()?;
                let count = input.uint()?;
                if self.entities.get(&item) != Some(&probe_776_item_kind()) {
                    return Err(failure(Error::UnknownEntity(item)));
                }
                if self.entities.get(&collector) != Some(&entity_layouts::PLAYER) { return Err(failure(Error::UnknownEntity(collector))); }
                if !(1..=127).contains(&count) || input.remaining() != 0 { return Err(wire::Error::Invalid); }
            }
            "rel_entity_move" | "entity_move_look" | "sync_entity_position" | "entity_teleport" if probe_776 => {
                let id = input.uint()?;
                if !self.entities.contains_key(&id) {
                    return Err(failure(Error::UnknownEntity(id)));
                }
            }
            "entity_destroy" => {
                let count = input.count(65_536)?;
                for _ in 0..count { removed.push(input.uint()?); }
                if input.remaining() != 0 { return Err(wire::Error::Invalid); }
            }
            "login" => {
                let id = i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| wire::Error::Invalid)?);
                player = Some(u32::try_from(id).map_err(|_| wire::Error::Invalid)?);
                input.boolean()?;
                for _ in 0..input.count(1024)? { input.string()?; }
                for _ in 0..3 { input.uint()?; }
                for _ in 0..3 { input.boolean()?; }
                input.uint()?; dimension = Some(input.string()?.to_owned());
            }
            "respawn" => { input.uint()?; dimension = Some(input.string()?.to_owned()); }
            _ => (),
        }
        context.clock_774=self.clock_774;
        let translated = if probe_776 {
            wire::translate_inner(State::Play, Direction::Clientbound, from, to, body, context, true)?
        } else { wire::translate_with_context(State::Play, Direction::Clientbound, from, to, body, context)? };
        if name=="update_time" && to==JavaVersion::V1_21_11 {
            let mut clock=Cursor::new(&translated);clock.uint()?;
            let age=i64::from_be_bytes(clock.take(8)?.try_into().map_err(|_|wire::Error::Invalid)?);
            let time=i64::from_be_bytes(clock.take(8)?.try_into().map_err(|_|wire::Error::Invalid)?);
            self.clock_774=Some((age,time,clock.boolean()?));
        }
        if let Some(dimension) = dimension {
            // В том же измерении остальные сущности остаются известны клиенту.
            if player.is_some() || self.dimension.as_ref() != Some(&dimension) { self.entities.clear(); }
            self.dimension = Some(dimension);
        }
        if let Some(id) = player { self.player = Some(id); }
        if let Some(id) = self.player { self.entities.insert(id, entity_layouts::PLAYER); }
        if let Some((id, kind)) = spawn { self.entities.insert(id, kind); }
        for id in removed { self.entities.remove(&id); }
        Ok(translated)
    }
}

// Внутренняя идентичность по своей закреплённой таблице, не wire ID776.
fn probe_776_item_kind() -> u32 {
    entity_layouts::LAYOUTS.iter().position(|entry| entry.0 == "item").expect("item layout") as u32
}

pub(crate) fn spawn_776(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, content: crate::content::ContentCodec) -> Result<(), wire::Error> {
    let mut header = Cursor::new(input.rest);
    header.uint()?; header.take(16)?;
    let raw = header.uint()?;
    let kind = match content.entity_layout_base(raw).filter(|_| from == wire::INTERNAL) {
        Some(name) => entity_layouts::LAYOUTS.iter().position(|row| row.0 == name).ok_or(wire::Error::Invalid)? as u32,
        None => ids::translate(RegistryKind::Entity, from, wire::INTERNAL, raw)?,
    };
    // Проверяется именно раскладка целевого заменителя, а не wire ID.
    let kind = if entity_layouts::LAYOUTS[kind as usize].0 == "cushion" {
        entity_layouts::LAYOUTS.iter().position(|row| row.0 == "armor_stand").ok_or(wire::Error::Invalid)? as u32
    } else if to==JavaVersion::V1_21_11 && entity_layouts::LAYOUTS[kind as usize].0=="sulfur_cube" {entity_layouts::LAYOUTS.iter().position(|row|row.0=="slime").ok_or(wire::Error::Invalid)? as u32} else { kind };
    metadata_layout(to, kind)?;
    spawn_with_content(input, out, from, to, content)
}

pub(crate) fn spawn_with_content(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, content: crate::content::ContentCodec) -> Result<(), wire::Error> {
    uint(out, input.uint()?); out.extend(input.take(16)?);
    let kind = input.uint()?;
    let extension = content.entity_layout_base(kind).filter(|_| from == wire::INTERNAL);
    uint(out, if extension.is_some() { content.packet_id(RegistryKind::Entity, from, to, kind)? }
        else { ids::translate(RegistryKind::Entity, from, to, kind)? });
    for _ in 0..3 {
        let bytes = input.take(8)?;
        if !f64::from_be_bytes(bytes.try_into().map_err(|_| wire::Error::Invalid)?).is_finite() { return Err(wire::Error::Invalid); }
        out.extend(bytes);
    }
    // LP Vec3: нулевой вектор — один байт; иначе48бит и необязательный VarInt.
    let first = input.take(1)?[0]; out.push(first);
    if first != 0 {
        if first & 7 == 0 { return Err(wire::Error::Invalid); }
        out.extend(input.take(5)?);
        if first & 4 != 0 { uint(out, input.uint()?); }
    }
    out.extend(input.take(3)?); // pitch/yaw/headYaw
    let data = input.varint()?;
    let old_kind = if extension.is_some() { u32::MAX } else { ids::translate(RegistryKind::Entity, from, wire::INTERNAL, kind)? };
    uint(out, if old_kind == entity_layouts::FALLING_BLOCK {
        content.packet_id(RegistryKind::BlockState, from, to, u32::try_from(data).map_err(|_| wire::Error::Invalid)?)?
    } else { data as u32 }); // направление либо динамический entityId, не ID контента
    Ok(())
}

fn registry_id(input: &mut Cursor<'_>, out: &mut Vec<u8>, context: wire::Context, name: &'static str, holder: bool) -> Result<(), wire::Error> {
    let limit = context.metadata_registry_counts.iter().find(|(key,_)| *key == name).ok_or_else(|| failure(Error::MissingRegistry(name)))?.1;
    let id = input.uint()?;
    let index = if holder { id.checked_sub(1).ok_or_else(|| failure(Error::Serializer(34)))? } else { id };
    if index >= limit { return Err(wire::Error::Invalid); }
    uint(out, id); Ok(())
}

#[path = "entity_layouts_776.rs"]
mod layouts_776;

fn metadata_layout(version: JavaVersion, kind: u32) -> Result<&'static [&'static str], wire::Error> {
    let (_, old, new) = entity_layouts::LAYOUTS.get(kind as usize).ok_or_else(|| failure(Error::UnknownKind(kind)))?;
    match version {
        JavaVersion::V26_1_2 => Ok(old),
        JavaVersion::V26_2 => crate::generated_payload_776::LAYOUTS.iter().find(|entry| entry.0 == kind)
            .map(|entry| entry.1).ok_or(wire::Error::PayloadUnavailable("unverified 26.2 metadata entity")),
        JavaVersion::V26_3 => Ok(new),
        JavaVersion::V1_21_11 => crate::generated_payload_774::LAYOUTS.iter().find(|r|r.0==kind).map(|r|r.1).ok_or_else(||failure(Error::UnknownKind(kind))),
        JavaVersion::V1_21_9 => crate::generated_payload_773::LAYOUTS.iter().find(|r|r.0==kind).map(|r|r.1).ok_or_else(||failure(Error::UnknownKind(kind))),
        JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_12_2 => Err(wire::Error::UnsupportedVersion(version)),
    }
}

pub(crate) fn metadata(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, context: wire::Context) -> Result<(), wire::Error> {
    let kind = context.entity_kind.ok_or(wire::Error::MissingEntityContext)?;
    let source = metadata_layout(from, kind)?;
    let substitute = if context.items.content.is_native_view() || (context.items.content.is_preview() && matches!(to,JavaVersion::V1_21_11|JavaVersion::V26_2)) {
        match entity_layouts::LAYOUTS.get(kind as usize).map(|r| r.0) {
            Some("cushion") => Some("armor_stand"),
            Some("sulfur_cube") if to != JavaVersion::V26_2 => Some("slime"), _ => None,
        }
    } else { None };
    let target_kind = substitute.and_then(|name| entity_layouts::LAYOUTS.iter().position(|r| r.0 == name)).map_or(kind, |n| n as u32);
    let target = metadata_layout(to, target_kind)?;
    let has_776 = from == JavaVersion::V26_2 || to == JavaVersion::V26_2;
    uint(out, input.uint()?);
    let mut seen = [false;255];
    loop {
        let index = input.take(1)?[0];
        if index == 255 { out.push(255); break; }
        if std::mem::replace(&mut seen[index as usize], true) { return Err(wire::Error::Invalid); }
        let field = source.get(index as usize).ok_or_else(|| failure(Error::UnknownField(index)))?;
        let start = out.len();
        let target_index = target.iter().position(|name| name == field);
        if target_index.is_none() && substitute.is_none() && !(to==JavaVersion::V1_21_11 && matches!(*field,"age_locked"|"sound_variant"|"villager_finalized")) { return Err(failure(Error::MissingField(field))); }
        out.push(target_index.unwrap_or(0) as u8);
        let raw_serializer = input.uint()?;
        let serializer = if from==JavaVersion::V1_21_9 {u32::from(*crate::generated_payload_773::SERIALIZERS_FROM.get(raw_serializer as usize).ok_or(wire::Error::Invalid)?)} else if from==JavaVersion::V1_21_11 {u32::from(*crate::generated_payload_774::SERIALIZERS_FROM.get(raw_serializer as usize).ok_or(wire::Error::Invalid)?)} else {raw_serializer};
        if has_776 {
            // Пять наблюдённых типов сохраняют строгую проверку serializer
            // поля; прочие раскладки закреплены JSON, значения разбираются
            // по совпадающему дискриминатору entityMetadataEntry775/776.
            if layouts_776::LAYOUTS.iter().any(|row| row.0 == kind) {
                let expected = layouts_776::SERIALIZERS.iter().find(|entry| entry.0 == *field)
                    .ok_or_else(|| failure(Error::UnknownField(index)))?.1;
                if serializer != expected { return Err(failure(Error::Serializer(serializer))); }
            }
        }
        let target_serializer=if to==JavaVersion::V1_21_9 {*crate::generated_payload_773::SERIALIZERS_TO.get(serializer as usize).ok_or(wire::Error::Invalid)? as u32} else if to==JavaVersion::V1_21_11 {*crate::generated_payload_774::SERIALIZERS_TO.get(serializer as usize).ok_or(wire::Error::Invalid)? as u32} else {serializer};
        if target_serializer==u32::from(u16::MAX) && target_index.is_some(){return Err(failure(Error::Serializer(serializer)));}
        uint(out, target_serializer);
        match serializer {
            0 => out.extend(input.take(1)?),
            1 => uint(out, input.varint()? as u32),
            2 => { // signed VarLong, включая отрицательные значения.
                for n in 0..10 {
                    let byte = input.take(1)?[0];
                    if n == 9 && byte & 254 != 0 { return Err(wire::Error::Invalid); }
                    out.push(byte);
                    if byte & 128 == 0 { break; }
                }
            }
            3 | 9 | 39 | 40 => {
                let n = match serializer { 3=>1, 40=>4, _=>3 };
                for _ in 0..n {
                    let value = input.take(4)?;
                    if !f32::from_be_bytes(value.try_into().map_err(|_| wire::Error::Invalid)?).is_finite() { return Err(wire::Error::Invalid); }
                    out.extend(value);
                }
            }
            4 => { let value = input.string()?; uint(out,value.len() as u32); out.extend(value.as_bytes()); }
            5 | 6 => {
                let present = serializer == 5 || input.boolean()?;
                if serializer == 6 { out.push(u8::from(present)); }
                if present {
                    let length = registry::nbt_length(input.rest).map_err(|e| wire::Error::Item(items::Error::Nbt(e)))?;
                    let value = input.take(length)?;
                    items::plain_text(value).map_err(wire::Error::Item)?; out.extend(value);
                }
            }
            7 => {
                let (stack,size) = items::decode_packet_slot(from,input.rest,false,context.items).map_err(wire::Error::Item)?;
                input.take(size)?;
                out.extend(items::encode_packet_slot(to,stack.as_ref(),false,context.items).map_err(wire::Error::Item)?);
            }
            8 => out.push(u8::from(input.boolean()?)),
            10 => out.extend(input.take(8)?),
            11 => { let present = input.boolean()?; out.push(u8::from(present)); if present { out.extend(input.take(8)?); } }
            12 => uint(out,input.count(5)? as u32),
            13 => {
                // Переименование в living_entity_reference не поменяло эту
                // форму: pinned JSON + оба реальных пакета владельца волка.
                let present = input.boolean()?; out.push(u8::from(present));
                if present { out.extend(input.take(16)?); }
            }
            14 | 15 => {
                let value = input.uint()?;
                // OptionalBlockState:0означает отсутствие, остальные — прямой
                // stateID. Это НЕ OptionalUnsignedInt с добавлением единицы.
                uint(out, if serializer == 15 && value == 0 { 0 }
                    else { context.items.content.packet_id(RegistryKind::BlockState,from,to,value)? });
            }
            16 => crate::particles::particle(input,out,from,to,context.items)?,
            17 => {
                let count = input.count(1024)?;
                uint(out,count as u32);
                for _ in 0..count { crate::particles::particle(input,out,from,to,context.items)?; }
            }
            18 => {
                // Все 7 типов, 15 профессий и уровни 1–5 записаны обычными
                // summon на обеих версиях (trading-observations.json).
                uint(out,input.count(6)? as u32);uint(out,input.count(14)? as u32);
                let level=input.count(5)?;if level==0{return Err(wire::Error::Invalid);}uint(out,level as u32);
            }
            19 => uint(out,input.uint()?),
            20 => uint(out,input.count(17)? as u32),
            21..=32 => {
                let name = ["minecraft:cat_variant","minecraft:cat_sound_variant","minecraft:cow_variant","minecraft:cow_sound_variant",
                    "minecraft:wolf_variant","minecraft:wolf_sound_variant","minecraft:frog_variant","minecraft:pig_variant",
                    "minecraft:pig_sound_variant","minecraft:chicken_variant","minecraft:chicken_sound_variant","minecraft:zombie_nautilus_variant"][(serializer-21) as usize];
                registry_id(input,out,context,name,false)?;
            }
            33 => {
                let present = input.boolean()?; out.push(u8::from(present));
                if present { let name = input.string()?; uint(out,name.len() as u32); out.extend(name.as_bytes()); out.extend(input.take(8)?); }
            }
            34 => registry_id(input,out,context,"minecraft:painting_variant",true)?,
            41 if has_776 || matches!(from,JavaVersion::V1_21_9|JavaVersion::V1_21_11) || matches!(to,JavaVersion::V1_21_9|JavaVersion::V1_21_11) => {
                let start = input.rest;
                crate::profile::read(input)?;
                out.extend(&start[..start.len() - input.rest.len()]);
            }
            35..=38 | 42 => { let max = match serializer {35=>6,36=>3,37=>3,38=>3,_=>1}; uint(out,input.count(max)? as u32); }
            _ => return Err(failure(Error::Serializer(serializer))),
        }
        // Поля отсутствующего типа не посылаются заменителю; серверная metadata сохраняется.
        if target_index.is_none() { out.truncate(start); }
    }
    Ok(())
}

#[cfg(test)]
#[path = "entity_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "metadata_776_tests.rs"]
mod metadata_776_tests;
