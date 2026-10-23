//! Представление предметов/cow775→340. Только собственные пакеты ядра,
//! pinned protocol.json и собственные записи official340; не игровые правила.
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};
use crate::{legacy_chunk::HeightWindow, teleport::Pose, wire::{Cursor, Error, uint}};

const LIMIT: usize = 65_536;
const ITEM: u32 = 71;
const COW: u32 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Item, Cow }

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tracked { kind: Kind, pose: Pose }

#[derive(Clone, Debug, Default)]
pub(crate) struct Tracker {
    // Session::control клонирует состояние перед проверкой пакета; общая карта
    // не копируется целиком при каждой телепортации/смене abilities.
    entities: Arc<BTreeMap<u32, Tracked>>,
}

fn number(input: &mut Cursor<'_>) -> Result<f64, Error> {
    let value = f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn scalar(input: &mut Cursor<'_>) -> Result<f32, Error> {
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
pub(crate) fn velocity(input: &mut Cursor<'_>) -> Result<[f64; 3], Error> {
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
pub(crate) fn put_velocity(out: &mut Vec<u8>, values: [f64; 3]) -> Result<(), Error> {
    for value in values {
        // [проверить] Высокие скорости требуют отдельной пробы ограничения340.
        // Здесь не обрезаем неизвестный диапазон молча; обычный дроп в пределах.
        if !value.is_finite() || value.abs() > 3.9 { return Err(Error::PayloadUnavailable("legacy entity velocity range")); }
        out.extend(((value * 8000.0).trunc() as i16).to_be_bytes());
    }
    Ok(())
}

impl Tracker {
    pub(crate) fn clear(&mut self) { self.entities = Arc::default(); }

    pub(crate) fn living(&self, entity: u32) -> Result<(), Error> {
        if self.tracked(entity)?.kind == Kind::Cow { Ok(()) }
        else { Err(Error::PayloadUnavailable("legacy entity is not living")) }
    }

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
                    if player == Some(entity as i32) || self.entities.contains_key(&entity) { return Err(Error::Invalid); }
                    if self.entities.len() >= LIMIT { return Err(Error::TooLarge); }
                    let uuid = input.take(16)?;
                    let kind = match input.uint()? {
                        ITEM => Kind::Item, COW => Kind::Cow,
                        _ => return Err(Error::PayloadUnavailable("legacy entity kind")),
                    };
                    let mut pose = position(&mut input)?;
                    let speed = velocity(&mut input)?;
                    let pitch = input.take(1)?[0]; let yaw = input.take(1)?[0];
                    let head = input.take(1)?[0];
                    if input.varint()? != 0 { return Err(Error::PayloadUnavailable("legacy entity spawn data")); }
                    pose.yaw = f32::from(yaw) * 360.0 / 256.0; pose.pitch = f32::from(pitch) * 360.0 / 256.0;
                    out.push(if kind == Kind::Item { 0x00 } else { 0x03 });
                    uint(&mut out, entity); out.extend(uuid);
                    if kind == Kind::Item { out.push(2); } else { uint(&mut out, 92); }
                    put_position(&mut out, pose, window)?;
                    if kind == Kind::Item {
                        out.extend([pitch, yaw]);
                        // official340 item-no-gravity: objectData=1, включая обычный item.
                        out.extend(1i32.to_be_bytes()); put_velocity(&mut out, speed)?;
                    } else {
                        out.extend([yaw, pitch, head]); put_velocity(&mut out, speed)?;
                        // Actual340 client: пустой inline список, затем отдельная
                        // Metadata корректно показывают adult/baby (cow fixture).
                        out.push(255);
                    }
                    changed = Some((entity, Tracked { kind, pose }));
                }
                0x63 => {
                    let kind = self.tracked(entity)?.kind;
                    out.push(0x3c); uint(&mut out, entity);
                    metadata(&mut input, &mut out, kind, enchantments)?;
                }
                0x65 => {
                    if player != Some(entity as i32) { self.tracked(entity)?; }
                    out.push(0x3e); uint(&mut out, entity); put_velocity(&mut out, velocity(&mut input)?)?;
                }
                0x53 => {
                    self.living(entity)?;
                    out.push(0x36); uint(&mut out, entity); out.extend(input.take(1)?);
                }
                0x23 => {
                    let mut tracked = self.tracked(entity)?;
                    let mut pose = position(&mut input)?;
                    let speed = [number(&mut input)?, number(&mut input)?, number(&mut input)?];
                    pose.yaw = scalar(&mut input)?; pose.pitch = scalar(&mut input)?;
                    let ground = input.boolean()?;
                    out.push(0x4c); uint(&mut out, entity); put_position(&mut out, pose, window)?;
                    out.extend([angle(pose.yaw), angle(pose.pitch), u8::from(ground)]);
                    let mut motion = vec![0x3e]; uint(&mut motion, entity); put_velocity(&mut motion, speed)?;
                    extra.push(motion); tracked.pose = pose; changed = Some((entity, tracked));
                }
                0x35 | 0x36 | 0x38 => {
                    let mut tracked = self.tracked(entity)?;
                    let pose = &mut tracked.pose;
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
                    out.push(u8::from(input.boolean()?)); window.client_position(*pose)?;
                    changed = Some((entity, tracked));
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
        if let Some((entity, tracked)) = changed { Arc::make_mut(&mut self.entities).insert(entity, tracked); }
        for entity in removed { Arc::make_mut(&mut self.entities).remove(&entity); }
        extra.insert(0, out); Ok(extra)
    }

    fn item(&self, entity: u32) -> Result<Pose, Error> {
        let tracked = self.tracked(entity)?;
        if tracked.kind != Kind::Item { return Err(Error::PayloadUnavailable("legacy entity is not item")); }
        Ok(tracked.pose)
    }

    fn tracked(&self, entity: u32) -> Result<Tracked, Error> {
        self.entities.get(&entity).copied().ok_or(Error::PayloadUnavailable("legacy untracked entity"))
    }
}

fn metadata(input: &mut Cursor<'_>, out: &mut Vec<u8>, entity_kind: Kind, enchantments: &[&str]) -> Result<(), Error> {
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
                if input.boolean()? { return Err(Error::PayloadUnavailable("legacy entity custom name")); }
                out.extend([2, 3, 0]);
            }
            (3..=5, 8) => { out.extend([index, 6, u8::from(input.boolean()?)]); }
            (8, 7) if entity_kind == Kind::Item => {
                let (item, count) = crate::items::decode(crate::wire::INTERNAL, input.rest, false,
                    crate::items::Context { shared_enchantment_count: Some(enchantments.len() as u32), ..Default::default() }).map_err(Error::Item)?;
                input.take(count)?;
                out.extend([6, 5]); out.extend(crate::legacy_items::encode(item.as_ref(),
                    crate::legacy_items::Context { enchantments }).map_err(Error::LegacyItem)?);
            }
            (9, 3) if entity_kind == Kind::Cow => {
                let health = scalar(input)?;
                if health < 0.0 { return Err(Error::Invalid); }
                out.extend([7, 2]); out.extend(health.to_be_bytes());
            }
            (6, 20) | (10, 17) | (12, 1) if entity_kind == Kind::Cow => {
                // Собственный sender фильтрует standing/empty particles/arrow0
                // при spawn, но changed path может прислать сброс. У клиента340
                // это исходное состояние; ненулевые значения здесь ещё НЕ
                // принимались. Разбираем0, не угадывая индексы старых эффектов.
                if input.varint()? != 0 { return Err(Error::PayloadUnavailable("legacy cow nondefault metadata")); }
            }
            (15, 0) if entity_kind == Kind::Cow => {
                // NoAI: old adult1 против AI cow0, cross-entity observation.
                // [проверить] isolated toggle; прочие биты НЕ переносим наугад.
                let flags = input.take(1)?[0];
                if flags & !1 != 0 { return Err(Error::PayloadUnavailable("legacy cow flags")); }
                out.extend([11, 0, flags]);
            }
            (16, 8) if entity_kind == Kind::Cow =>
                out.extend([12, 6, u8::from(input.boolean()?)]),
            (17, 8) if entity_kind == Kind::Cow => {
                // AgeLocked управляет ростом на сервере. False не меняет
                // старую визуализацию, возраст уже передаётся полем12.
                if input.boolean()? { return Err(Error::PayloadUnavailable("legacy cow locked age")); }
            }
            _ => return Err(Error::PayloadUnavailable("legacy entity metadata field")),
        }
    }
}

#[cfg(test)]
#[path = "legacy_entity_tests.rs"]
mod tests;
