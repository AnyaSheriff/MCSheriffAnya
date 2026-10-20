//! Представление выпавших предметов775→340. Только собственные пакеты ядра,
//! pinned protocol.json и запись official-item-entities; не игровые правила.
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};
use crate::{legacy_chunk::HeightWindow, teleport::Pose, wire::{Cursor, Error, uint}};

const LIMIT: usize = 65_536;
const ITEM: u32 = 71;

#[derive(Clone, Debug, Default)]
pub(crate) struct Tracker {
    // Session::control клонирует состояние перед проверкой пакета; общая карта
    // не копируется целиком при каждой телепортации/смене abilities.
    items: Arc<BTreeMap<u32, Pose>>,
}

fn number(input: &mut Cursor<'_>) -> Result<f64, Error> {
    let value = f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn degrees(input: &mut Cursor<'_>) -> Result<f32, Error> {
    let value = f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn position(input: &mut Cursor<'_>) -> Result<Pose, Error> {
    Ok(Pose { x: number(input)?, y: number(input)?, z: number(input)?, yaw: 0.0, pitch: 0.0 })
}
fn put_position(out: &mut Vec<u8>, pose: Pose, window: HeightWindow) -> Result<(), Error> {
    let pose = window.client_position(pose)?;
    for value in [pose.x, pose.y, pose.z] { out.extend(value.to_be_bytes()); }
    Ok(())
}
fn angle(value: f32) -> u8 { ((f64::from(value).rem_euclid(360.0) * 256.0 / 360.0).floor() as u16 & 255) as u8 }

/// Обратная операция нашего entity_sync::push_velocity. Зарезервированный
/// компонент32767 и переполненный множитель не превращаются в числа.
fn velocity(input: &mut Cursor<'_>) -> Result<[f64; 3], Error> {
    let first = input.take(1)?[0];
    if first == 0 { return Ok([0.0; 3]); }
    if first & 7 == 0 { return Err(Error::Invalid); }
    let second = input.take(1)?[0];
    let tail = u32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
    let packed = u64::from(first) | (u64::from(second) << 8) | (u64::from(tail) << 16);
    let extra = if first & 4 != 0 { input.uint()? } else { 0 };
    let scale = u64::from(first & 3) + (u64::from(extra) << 2);
    if scale == 0 || scale > i32::MAX as u64 { return Err(Error::Invalid); }
    let mut values = [0.0; 3];
    for (index, value) in values.iter_mut().enumerate() {
        let part = (packed >> (3 + 15 * index)) & 0x7fff;
        if part > 32766 { return Err(Error::Invalid); }
        *value = (part as f64 / 32766.0 * 2.0 - 1.0) * scale as f64;
    }
    Ok(values)
}
fn put_velocity(out: &mut Vec<u8>, values: [f64; 3]) -> Result<(), Error> {
    for value in values {
        // [проверить] Высокие скорости требуют отдельной пробы ограничения340.
        // Здесь не обрезаем неизвестный диапазон молча; обычный дроп в пределах.
        if !value.is_finite() || value.abs() > 3.9 { return Err(Error::PayloadUnavailable("legacy item velocity range")); }
        out.extend(((value * 8000.0).trunc() as i16).to_be_bytes());
    }
    Ok(())
}

impl Tracker {
    pub(crate) fn clear(&mut self) { self.items = Arc::default(); }

    pub(crate) fn translate(&mut self, id: u32, payload: &[u8], window: HeightWindow,
        player: Option<i32>, enchantments: &[&str]) -> Result<Vec<Vec<u8>>, Error> {
        if payload.len() > 1_048_576 { return Err(Error::TooLarge); }
        let mut input = Cursor::new(payload);
        let mut out = Vec::new();
        let mut extra = Vec::new();
        let mut changed = None;
        let mut removed = Vec::new();
        if id == 0x4d {
            let count = input.count(LIMIT)?;
            out.push(0x32); uint(&mut out, count as u32);
            for _ in 0..count {
                let entity = input.uint()?;
                if player == Some(entity as i32) { return Err(Error::PayloadUnavailable("legacy removing own player")); }
                // Повторное удаление допустимо и не создаёт запись tracker.
                uint(&mut out, entity); removed.push(entity);
            }
        } else {
            let entity = input.uint()?;
            match id {
                0x01 => {
                    if player == Some(entity as i32) || self.items.contains_key(&entity) { return Err(Error::Invalid); }
                    if self.items.len() >= LIMIT { return Err(Error::TooLarge); }
                    let uuid = input.take(16)?;
                    if input.uint()? != ITEM { return Err(Error::PayloadUnavailable("legacy entity kind")); }
                    let mut pose = position(&mut input)?;
                    let speed = velocity(&mut input)?;
                    let pitch = input.take(1)?[0]; let yaw = input.take(1)?[0];
                    input.take(1)?; // headYaw не применяется к предмету.
                    if input.varint()? != 0 { return Err(Error::PayloadUnavailable("legacy item spawn data")); }
                    pose.yaw = f32::from(yaw) * 360.0 / 256.0; pose.pitch = f32::from(pitch) * 360.0 / 256.0;
                    out.push(0x00); uint(&mut out, entity); out.extend(uuid); out.push(2);
                    put_position(&mut out, pose, window)?; out.extend([pitch, yaw]);
                    // official340 item-no-gravity: objectData=1, включая обычный item.
                    out.extend(1i32.to_be_bytes()); put_velocity(&mut out, speed)?;
                    changed = Some((entity, pose));
                }
                0x63 => {
                    self.item(entity)?;
                    out.push(0x3c); uint(&mut out, entity);
                    metadata(&mut input, &mut out, enchantments)?;
                }
                0x65 => {
                    if player != Some(entity as i32) { self.item(entity)?; }
                    out.push(0x3e); uint(&mut out, entity); put_velocity(&mut out, velocity(&mut input)?)?;
                }
                0x23 => {
                    self.item(entity)?;
                    let mut pose = position(&mut input)?;
                    let speed = [number(&mut input)?, number(&mut input)?, number(&mut input)?];
                    pose.yaw = degrees(&mut input)?; pose.pitch = degrees(&mut input)?;
                    let ground = input.boolean()?;
                    out.push(0x4c); uint(&mut out, entity); put_position(&mut out, pose, window)?;
                    out.extend([angle(pose.yaw), angle(pose.pitch), u8::from(ground)]);
                    let mut motion = vec![0x3e]; uint(&mut motion, entity); put_velocity(&mut motion, speed)?;
                    extra.push(motion); changed = Some((entity, pose));
                }
                0x35 | 0x36 | 0x38 => {
                    let mut pose = self.item(entity)?;
                    out.push(match id { 0x35 => 0x26, 0x36 => 0x27, _ => 0x28 }); uint(&mut out, entity);
                    if id != 0x38 {
                        for value in [&mut pose.x, &mut pose.y, &mut pose.z] {
                            let bytes = input.take(2)?;
                            *value = (*value * 4096.0).round() / 4096.0
                                + f64::from(i16::from_be_bytes(bytes.try_into().map_err(|_| Error::Invalid)?)) / 4096.0;
                            out.extend(bytes);
                        }
                    }
                    if id != 0x35 {
                        let yaw = input.take(1)?[0]; let pitch = input.take(1)?[0];
                        pose.yaw = f32::from(yaw) * 360.0 / 256.0; pose.pitch = f32::from(pitch) * 360.0 / 256.0;
                        out.extend([yaw, pitch]);
                    }
                    out.push(u8::from(input.boolean()?)); window.client_position(pose)?;
                    changed = Some((entity, pose));
                }
                0x7c => {
                    self.item(entity)?;
                    let collector = input.uint()?;
                    // В этом инкременте наблюдён только сбор собственным игроком.
                    if player != Some(collector as i32) { return Err(Error::PayloadUnavailable("legacy item collector")); }
                    let count = input.uint()?; if count == 0 { return Err(Error::Invalid); }
                    out.push(0x4b); uint(&mut out, entity); uint(&mut out, collector); uint(&mut out, count);
                    // Collect не равен remove: частичный подбор оставляет сущность.
                }
                _ => return Err(Error::PayloadUnavailable("legacy entity packet")),
            }
        }
        if input.remaining() != 0 { return Err(Error::Invalid); }
        if let Some((entity, pose)) = changed { Arc::make_mut(&mut self.items).insert(entity, pose); }
        for entity in removed { Arc::make_mut(&mut self.items).remove(&entity); }
        extra.insert(0, out); Ok(extra)
    }

    fn item(&self, entity: u32) -> Result<Pose, Error> {
        self.items.get(&entity).copied().ok_or(Error::PayloadUnavailable("legacy untracked item"))
    }
}

fn metadata(input: &mut Cursor<'_>, out: &mut Vec<u8>, enchantments: &[&str]) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    loop {
        let index = input.take(1)?[0];
        if index == 255 { out.push(255); return Ok(()); }
        if !seen.insert(index) { return Err(Error::Invalid); }
        let kind = input.uint()?;
        match (index, kind) {
            (0, 0) => { out.extend([0, 0]); out.extend(input.take(1)?); }
            (1, 1) => { out.extend([1, 1]); uint(out, input.varint()? as u32); }
            (2, 6) => {
                if input.boolean()? { return Err(Error::PayloadUnavailable("legacy item custom name")); }
                out.extend([2, 3, 0]);
            }
            (3..=5, 8) => { out.extend([index, 6, u8::from(input.boolean()?)]); }
            (8, 7) => {
                let (item, count) = crate::items::decode(crate::wire::INTERNAL, input.rest, false,
                    crate::items::Context { shared_enchantment_count: Some(enchantments.len() as u32) }).map_err(Error::Item)?;
                input.take(count)?;
                out.extend([6, 5]); out.extend(crate::legacy_items::encode(item.as_ref(),
                    crate::legacy_items::Context { enchantments }).map_err(Error::LegacyItem)?);
            }
            _ => return Err(Error::PayloadUnavailable("legacy item metadata field")),
        }
    }
}

#[cfg(test)]
#[path = "legacy_entity_tests.rs"]
mod tests;
