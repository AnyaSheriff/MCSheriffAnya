//! Состояние представления340: общий775 мир остаётся источником правил.
//! Здесь только явно разобранные служебные пакеты. Неизвестные отклоняются.
use crate::{legacy_chunk::HeightWindow, legacy_control::Packet, teleport::Pose, wire::{Cursor, Error, uint}};

#[derive(Clone, Debug)]
pub struct Session {
    pub window: HeightWindow,
    pub pose: Option<Pose>,
    pub dimension: Option<i32>,
    pub entity: Option<i32>,
    pub difficulty: u8,
    player_flags: u8,
    clock: Option<(i64, i64)>,
    modifiers: crate::legacy_attributes::Modifiers,
    pending_teleport: Option<i32>,
    recipes: crate::legacy_recipes::Book,
}

fn number(input: &mut Cursor<'_>) -> Result<f64, Error> {
    let value = f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn angle(input: &mut Cursor<'_>) -> Result<f32, Error> {
    let value = f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn string(out: &mut Vec<u8>, value: &str) { uint(out, value.len() as u32); out.extend(value.as_bytes()); }
fn long(input: &mut Cursor<'_>) -> Result<i64, Error> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let b = input.take(1)?[0];
        if shift == 63 && b & 254 != 0 { return Err(Error::Invalid); }
        value |= u64::from(b & 127) << shift;
        if b < 128 { return Ok(value as i64); }
    }
    Err(Error::Invalid)
}
fn put_long(out: &mut Vec<u8>, value: i64) {
    let mut value = value as u64;
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; } out.push(value as u8);
}
fn text(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    let size = crate::registry::nbt_length(input.rest).map_err(|_| Error::Invalid)?;
    let json = crate::legacy_text::json(input.take(size)?)?; string(out, &json); Ok(())
}
fn slot(input: &mut Cursor<'_>, out: &mut Vec<u8>, enchantments: &[&str]) -> Result<(), Error> {
    let (item, size) = crate::items::decode(crate::wire::INTERNAL, input.rest, false,
        crate::items::Context { shared_enchantment_count: Some(enchantments.len() as u32) }).map_err(Error::Item)?;
    input.take(size)?;
    out.extend(crate::legacy_items::encode(item.as_ref(), crate::legacy_items::Context { enchantments }).map_err(Error::LegacyItem)?);
    Ok(())
}
fn dimension(name: &str) -> Result<i32, Error> {
    match name { "minecraft:overworld" => Ok(0), "minecraft:the_nether" => Ok(-1), "minecraft:the_end" => Ok(1), _ => Err(Error::PayloadUnavailable("legacy custom dimension")) }
}
fn spawn(input: &mut Cursor<'_>) -> Result<(i32, u8, bool), Error> {
    input.uint()?; // Числовой dimension_type не используется как старый dimension.
    let dim = dimension(input.string()?)?;
    input.take(8)?;
    let mode = input.take(1)?[0];
    if mode > 3 { return Err(Error::Invalid); }
    let previous = input.take(1)?[0];
    if previous > 3 && previous != 255 { return Err(Error::Invalid); }
    input.boolean()?;
    let flat = input.boolean()?;
    if input.boolean()? { input.string()?; input.take(8)?; }
    input.uint()?; input.varint()?;
    Ok((dim, mode, flat))
}

pub fn block_position(packed: u64, window: HeightWindow) -> Result<u64, Error> {
    let x = (packed as i64) >> 38;
    let z = ((packed << 26) as i64) >> 38;
    let y = (((packed << 52) as i64) >> 52) as i32;
    let y = window.client_y(y)?;
    Ok(((x as u64 & 0x3ffffff) << 38) | (u64::from(y) << 26) | (z as u64 & 0x3ffffff))
}

impl Session {
    pub fn new(window: HeightWindow, difficulty: u8) -> Result<Self, Error> {
        if difficulty > 3 { return Err(Error::Invalid); }
        Ok(Self { window, pose: None, dimension: None, entity: None, difficulty, player_flags: 0, clock: None, modifiers: Default::default(), pending_teleport: None, recipes: Default::default() })
    }

    /// Только подтверждённые поля игрока. Для моба тот же индекс может
    /// означать другое: до появления в tracker такой пакет не принимается.
    pub fn player_metadata(&mut self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        use std::collections::BTreeMap;
        let mut input = Cursor::new(payload);
        let entity = input.uint()?;
        if Some(entity as i32) != self.entity { return Err(Error::PayloadUnavailable("legacy entity kind")); }
        let mut fields = BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut flags = self.player_flags;
        loop {
            let index = input.take(1)?[0];
            if index == 255 { break; }
            if !seen.insert(index) { return Err(Error::Invalid); }
            let kind = input.uint()?;
            let (old, value) = match (index, kind) {
                (0, 0) => { flags = input.take(1)?[0]; (0, flags) }
                (6, 20) => {
                    let pose = input.uint()?;
                    match pose { 0 => flags &= !2, 5 => flags |= 2, _ => return Err(Error::PayloadUnavailable("legacy player pose")) }
                    (0, flags)
                }
                (15, 42) => {
                    let hand = input.uint()?;
                    if hand > 1 { return Err(Error::Invalid); }
                    (14, hand as u8)
                }
                (16, 0) => { let skin = input.take(1)?[0]; if skin & 128 != 0 { return Err(Error::Invalid); } (13, skin) }
                _ => return Err(Error::PayloadUnavailable("legacy player metadata field")),
            };
            fields.insert(old, value);
        }
        if input.remaining() != 0 { return Err(Error::Invalid); }
        let mut out = vec![0x3c]; uint(&mut out, entity);
        for (index, value) in fields { out.extend([index, 0, value]); }
        out.push(255);
        self.player_flags = flags;
        Ok(out)
    }

    /// Пакеты новой организации отправки не являются игровыми действиями340.
    /// Команды остаются серверными, старый клиент запрашивает подсказки отдельно.
    pub fn clientbound(&mut self, body: &[u8], enchantments: &[&str]) -> Result<Vec<Vec<u8>>, Error> {
        if body.len() > 8 * 1024 * 1024 { return Err(Error::TooLarge); }
        let mut input = Cursor::new(body);
        let id = input.uint()?;
        let mut out = Vec::new();
        let mut extra = Vec::new();
        let mut clock = self.clock;
        match id {
            0x31 | 0x52 | 0x48 | 0x40 => return Ok(vec![self.control(body)?]),
            0x26 => {
                if input.rest.first() != Some(&13) { return Ok(vec![self.control(body)?]); }
                if input.take(5)? != [13, 0, 0, 0, 0] { return Err(Error::Invalid); }
            }
            // Дерево Brigadier и современные рецепты камнереза: этих пакетов
            // нет340. Это ещё не реализация старого tab-complete/recipe book.
            0x10 | 0x85 => { input.take(input.remaining())?; }
            0x0c => {}
            0x0b | 0x04 | 0x5f | 0x6f => { input.uint()?; }
            0x5e => { input.varint()?; input.varint()?; }
            0x7f => {
                if angle(&mut input)? != 20.0 || input.boolean()? { return Err(Error::PayloadUnavailable("legacy custom tick rate")); }
            }
            0x80 => { if input.uint()? != 0 { return Err(Error::PayloadUnavailable("legacy step tick")); } }
            0x2c => { out.push(0x1f); out.extend(input.take(8)?); }
            0x68 => {
                out.push(0x41); out.extend(angle(&mut input)?.to_be_bytes());
                uint(&mut out, input.uint()?); out.extend(angle(&mut input)?.to_be_bytes());
            }
            0x67 => { out.push(0x40); out.extend(angle(&mut input)?.to_be_bytes()); for _ in 0..2 { uint(&mut out, input.uint()?); } }
            0x69 => { out.push(0x3a); let slot = input.take(1)?[0]; if slot > 8 { return Err(Error::Invalid); } out.push(slot); }
            0x25 => {
                // Native ForgetLevelChunk: Z, X;340 UnloadChunk: X, Z.
                let z = input.take(4)?; let x = input.take(4)?;
                out.push(0x1d); out.extend(x); out.extend(z);
            }
            0x63 => return Ok(vec![self.player_metadata(input.rest)?]),
            0x83 => return Ok(self.modifiers.translate(input.rest)?.into_iter().collect()),
            0x4a..=0x4c => return Ok(vec![self.recipes.clientbound(id,input.rest)?]),
            0x82 => return Ok(vec![crate::legacy_advancements::translate(input.rest,enchantments)?]),
            0x46 => {
                let flags = input.take(1)?[0];
                let action = match flags { 13 | 45 => 0, 4 => 1, 16 => 2, 32 => 3, _ => return Err(Error::PayloadUnavailable("legacy player list action")) };
                let count = input.count(1024)?;
                out.extend([0x2e, action]); uint(&mut out, count as u32);
                for _ in 0..count {
                    out.extend(input.take(16)?);
                    if action == 0 {
                        string(&mut out, input.string()?);
                        let properties = input.count(1024)?; uint(&mut out, properties as u32);
                        for _ in 0..properties {
                            string(&mut out,input.string()?); string(&mut out,input.string()?);
                            let signed = input.boolean()?; out.push(u8::from(signed));
                            if signed { string(&mut out,input.string()?); }
                        }
                        uint(&mut out,input.uint()?);
                        if !input.boolean()? { return Err(Error::PayloadUnavailable("legacy hidden player list entry")); }
                        out.push(0); // latency не передавался в исходном Add.
                        let display = flags & 32 != 0 && input.boolean()?;
                        out.push(u8::from(display)); if display { text(&mut input,&mut out)?; }
                    } else if action == 3 {
                        let display = input.boolean()?; out.push(u8::from(display)); if display { text(&mut input,&mut out)?; }
                    } else { uint(&mut out,input.uint()?); }
                }
            }
            0x45 => {
                let count = input.count(1024)?; out.extend([0x2e,4]); uint(&mut out,count as u32); out.extend(input.take(count * 16)?);
            }
            0x12 => {
                let window = input.count(127)?; input.uint()?;
                let count = input.count(1024)?;
                out.extend([0x14,window as u8]); out.extend((count as i16).to_be_bytes());
                for _ in 0..count { slot(&mut input,&mut out,enchantments)?; }
                let mut cursor = vec![0x16,255,255,255]; slot(&mut input,&mut cursor,enchantments)?; extra.push(cursor);
            }
            0x14 => {
                let window = input.varint()?;
                if !(-2..=127).contains(&window) { return Err(Error::Invalid); }
                input.uint()?; out.extend([0x16,window as u8]); out.extend(input.take(2)?);
                slot(&mut input,&mut out,enchantments)?;
            }
            0x60 => { out.extend([0x16,255,255,255]); slot(&mut input,&mut out,enchantments)?; }
            0x20 | 0x79 => {
                out.push(if id == 0x20 {0x1a} else {0x0f}); text(&mut input,&mut out)?;
                if id == 0x79 { out.push(if input.boolean()? {2} else {1}); }
            }
            0x7a => { out.push(0x4a); text(&mut input,&mut out)?; text(&mut input,&mut out)?; }
            0x2b => {
                out.extend([0x38,3]);
                for _ in 0..4 { out.extend(number(&mut input)?.to_be_bytes()); }
                let speed = long(&mut input)?; if speed < 0 { return Err(Error::Invalid); } put_long(&mut out,speed);
                uint(&mut out,input.uint()?);
                let blocks = input.uint()?; let time = input.uint()?; uint(&mut out,time); uint(&mut out,blocks);
            }
            0x71 => {
                let age = i64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
                let count = input.count(1)?;
                let time = if count == 1 {
                    if input.uint()? != 0 { return Err(Error::PayloadUnavailable("legacy alternate clock")); }
                    let ticks = long(&mut input)?;
                    if angle(&mut input)? != 0.0 || angle(&mut input)? != 1.0 { return Err(Error::PayloadUnavailable("legacy clock rate")); }
                    if ticks < 0 { return Err(Error::PayloadUnavailable("legacy negative running time")); }
                    ticks
                } else {
                    let (previous, time) = clock.ok_or(Error::PayloadUnavailable("legacy initial clock"))?;
                    time.checked_add(age.checked_sub(previous).ok_or(Error::Invalid)?).ok_or(Error::Invalid)?
                };
                out.push(0x47); out.extend(age.to_be_bytes()); out.extend(time.to_be_bytes()); clock=Some((age,time));
            }
            0x61 => {
                let dim = dimension(input.string()?)?;
                if self.dimension != Some(dim) { return Err(Error::PayloadUnavailable("legacy spawn in another dimension")); }
                let packed = u64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
                angle(&mut input)?; angle(&mut input)?;
                out.push(0x46); out.extend(block_position(packed, self.window)?.to_be_bytes());
            }
            _ => return Err(Error::PayloadUnavailable("legacy clientbound packet")),
        }
        if input.remaining() != 0 { return Err(Error::Invalid); }
        self.clock = clock;
        if !out.is_empty() { extra.insert(0,out); }
        Ok(extra)
    }

    pub fn serverbound(&mut self, body: &[u8]) -> Result<Vec<u8>, Error> {
        let mut head = Cursor::new(body); let id = head.uint()?;
        if matches!(id, 0x12 | 0x17) { return self.recipes.serverbound(id,head.rest); }
        let pose = self.pose.ok_or(Error::PayloadUnavailable("legacy player pose"))?;
        let out = crate::legacy_actions::translate(body, crate::legacy_actions::Context { window: self.window, pose })?;
        let mut input = Cursor::new(&out);
        let id = input.uint()?;
        let mut next = pose;
        if id == 0 {
            let ack = input.varint()?;
            if self.pending_teleport == Some(ack) { self.pending_teleport = None; }
        }
        // Общий обработчик тоже игнорирует движение до правильного ACK.
        // Ранний/устаревший пакет не должен сдвигать базу относительного TP.
        if self.pending_teleport.is_some() { return Ok(out); }
        if matches!(id, 0x1e | 0x1f) {
            next.x = number(&mut input)?; next.y = number(&mut input)?; next.z = number(&mut input)?;
        }
        if matches!(id, 0x1f | 0x20) {
            next.yaw = angle(&mut input)?; next.pitch = angle(&mut input)?;
        }
        self.pose = Some(next);
        Ok(out)
    }

    /// Ошибка не меняет состояние. Пока обрабатывает только control; отсутствие
    /// старого аналога не означает разрешение терять любой новый пакет.
    pub fn control(&mut self, body: &[u8]) -> Result<Vec<u8>, Error> {
        if body.len() > 1_048_576 { return Err(Error::TooLarge); }
        let mut input = Cursor::new(body);
        let id = input.uint()?;
        let mut next = self.clone();
        let packet = match id {
            0x31 => {
                let entity = i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
                let hardcore = input.boolean()?;
                let count = input.count(1024)?;
                for _ in 0..count { input.string()?; }
                let maximum = input.uint()?.min(255) as u8;
                input.uint()?; input.uint()?;
                let reduced_debug = input.boolean()?;
                input.boolean()?; input.boolean()?;
                let (dim, mode, flat) = spawn(&mut input)?;
                input.boolean()?;
                next.dimension = Some(dim); next.entity = Some(entity); next.pose = None; next.pending_teleport = None;
                Packet::Join { entity, mode, hardcore, dimension: dim, difficulty: self.difficulty,
                    maximum, level_type: if flat { "flat" } else { "default" }.into(), reduced_debug }
            }
            0x52 => {
                let (dim, mode, flat) = spawn(&mut input)?;
                if input.take(1)?[0] & !3 != 0 { return Err(Error::Invalid); }
                next.dimension = Some(dim); next.pose = None; next.pending_teleport = None;
                Packet::Respawn { dimension: dim, difficulty: self.difficulty, mode,
                    level_type: if flat { "flat" } else { "default" }.into() }
            }
            0x48 => {
                let id = input.varint()?;
                let mut pose = Pose { x: number(&mut input)?, y: number(&mut input)?, z: number(&mut input)?, yaw: 0.0, pitch: 0.0 };
                let velocity = [number(&mut input)?, number(&mut input)?, number(&mut input)?];
                pose.yaw = angle(&mut input)?; pose.pitch = angle(&mut input)?;
                let flags = u32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
                if flags & !0x1ff != 0 { return Err(Error::Invalid); }
                // Ненулевая скорость требует отдельного Motion340, её нельзя
                // терять. До этого преобразования такой перенос закрыт.
                if velocity != [0.0; 3] { return Err(Error::PayloadUnavailable("legacy teleport velocity")); }
                let absolute = crate::legacy_control::resolve_teleport(pose, (flags & 31) as u8, self.pose)?;
                let pose = self.window.client_position(absolute)?;
                next.pose = Some(absolute);
                next.pending_teleport = Some(id);
                Packet::Teleport { pose, flags: 0, id }
            }
            0x40 => Packet::Abilities { flags: input.take(1)?[0], flying: angle(&mut input)?, walking: angle(&mut input)? },
            0x26 => Packet::GameState { reason: input.take(1)?[0], value: angle(&mut input)? },
            _ => return Err(Error::PayloadUnavailable("legacy control packet")),
        };
        if input.remaining() != 0 { return Err(Error::Invalid); }
        let (_, id, payload) = packet.encode()?;
        let mut out = Vec::with_capacity(payload.len() + 5); uint(&mut out, id); out.extend(payload);
        *self = next;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session { Session::new(HeightWindow::new(-64).unwrap(), 2).unwrap() }
    #[test]
    fn movement_before_matching_teleport_ack_does_not_change_relative_base() {
        let mut state = session();
        let pose = Pose { x: 1.0, y: 64.0, z: 2.0, yaw: 30.0, pitch: 4.0 };
        state.control(&teleport(pose,0)).unwrap();
        let mut movement = vec![0x0d];
        for value in [90f64,128.0,80.0] { movement.extend(value.to_be_bytes()); } movement.push(1);
        state.serverbound(&movement).unwrap(); assert_eq!(state.pose,Some(pose));
        state.serverbound(&[0,6]).unwrap(); assert_eq!(state.pending_teleport,Some(7));
        state.serverbound(&movement).unwrap(); assert_eq!(state.pose,Some(pose));
        state.serverbound(&[0,7]).unwrap(); assert_eq!(state.pending_teleport,None);
        state.serverbound(&movement).unwrap(); assert_eq!(state.pose.unwrap().x,90.0);
    }
    #[test]
    fn player_metadata_matches_official_skin_hand_sneak_and_effect_observations() {
        let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-player-metadata.json")).unwrap();
        let mut state = session();
        let entity = data["actor_entity"].as_u64().unwrap() as u32;
        state.entity = Some(entity as i32);
        for row in data["observations"].as_array().unwrap() {
            let mut native = Vec::new(); uint(&mut native, entity);
            native.extend(match row["stage"].as_str().unwrap() {
                "left-no-skin" => vec![15, 42, 0, 16, 0, 0],
                "right-all-skin" => vec![15, 42, 1, 16, 0, 127],
                "sneaking" => vec![0, 0, 2], "standing" => vec![0, 0, 0],
                "glowing" => vec![0, 0, 64], "invisible" => vec![0, 0, 96],
                other => panic!("unexpected stage {other}"),
            }); native.push(255);
            let encoded = state.player_metadata(&native).unwrap();
            let bytes: Vec<_> = row["hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
            assert_eq!(encoded[0], 0x3c); assert_eq!(encoded[1..], bytes);
        }
        let before = state.player_flags;
        let mut invalid = vec![]; uint(&mut invalid, entity); invalid.extend([0, 0, 2, 30, 0, 1, 255]);
        assert!(state.player_metadata(&invalid).is_err()); assert_eq!(state.player_flags, before);
        assert!(state.player_metadata(&[1, 0, 0, 0, 255]).is_err());
    }
    fn teleport(pose: Pose, flags: u32) -> Vec<u8> {
        let mut out = vec![0x48, 7];
        for v in [pose.x, pose.y, pose.z, 0.0, 0.0, 0.0] { out.extend(v.to_be_bytes()); }
        out.extend(pose.yaw.to_be_bytes()); out.extend(pose.pitch.to_be_bytes()); out.extend(flags.to_be_bytes()); out
    }
    #[test]
    fn absolute_and_relative_teleports_preserve_height_and_pose_atomically() {
        let mut state = session();
        let pose = Pose { x: -5.0, y: -60.0, z: 17.0, yaw: 20.0, pitch: 12.0 };
        let body = state.control(&teleport(pose, 0)).unwrap();
        let Packet::Teleport { pose: client, id, flags } = Packet::decode(crate::State::Play, u32::from(body[0]), &body[1..]).unwrap() else { panic!() };
        assert_eq!(client.y, 4.0); assert_eq!(id, 7); assert_eq!(flags, 0);
        let delta = Pose { x: 1.0, y: 2.0, z: -3.0, yaw: 5.0, pitch: 1.0 };
        state.control(&teleport(delta, 31)).unwrap();
        assert_eq!(state.pose.unwrap(), Pose { x: -4.0, y: -58.0, z: 14.0, yaw: 25.0, pitch: 13.0 });
        let before = state.pose;
        let mut broken = teleport(pose, 0); broken.push(0);
        assert!(state.control(&broken).is_err()); assert_eq!(state.pose, before);
        assert!(session().control(&teleport(delta, 1)).is_err());
    }
    #[test]
    fn packed_blocks_roundtrip_across_negative_coordinates_and_height_windows() {
        for origin in [-64, 0, 64] { for (x, z) in [(-17i64, -31i64), (0, 0), (33554431, -33554432)] {
            let window = HeightWindow::new(origin).unwrap();
            for y in [origin, origin + 255] {
                let modern = ((x as u64 & 0x3ffffff) << 38) | ((z as u64 & 0x3ffffff) << 12) | (y as u64 & 4095);
                assert_eq!(crate::legacy_actions::block_position(block_position(modern, window).unwrap(), window).unwrap(), modern);
            }
        } }
    }
}
