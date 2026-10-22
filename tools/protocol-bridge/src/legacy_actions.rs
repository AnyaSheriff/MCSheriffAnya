//! Входящие действия340 в общий формат775. Источник: закреплённые protocol.json.
//! Не создаёт игровые правила: проверку дальности, прав и движения делает ядро.
//! Окно высоты и текущий поворот передаёт соединение, а не глобальная настройка.
use crate::{legacy_chunk::HeightWindow, teleport::Pose, wire::{Cursor, Error, uint}};

#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub window: HeightWindow,
    /// Последняя согласованная поза в координатах сервера.
    pub pose: Pose,
}

fn string(out: &mut Vec<u8>, value: &str) { uint(out, value.len() as u32); out.extend(value.as_bytes()); }
fn number(input: &mut Cursor<'_>) -> Result<f64, Error> {
    let value = f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn angle(input: &mut Cursor<'_>) -> Result<f32, Error> {
    let value = f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}
fn bounded(input: &mut Cursor<'_>, maximum: u32) -> Result<u32, Error> {
    let value = input.uint()?;
    if value <= maximum { Ok(value) } else { Err(Error::Invalid) }
}

/// У340 порядок битов x/y/z; у775 x/z/y. Отрицательные x,z расширяются
/// со знаком до изменения окна; высота блока340 не может быть больше255.
pub fn block_position(packed: u64, window: HeightWindow) -> Result<u64, Error> {
    let x = (packed as i64) >> 38;
    let y = (packed >> 26) & 4095;
    let z = ((packed << 38) as i64) >> 38;
    let y = window.server_y(u8::try_from(y).map_err(|_| Error::Invalid)?);
    if !(-2048..=2047).contains(&y) { return Err(Error::Invalid); }
    Ok(((x as u64 & 0x3ffffff) << 38) | ((z as u64 & 0x3ffffff) << 12) | (y as u64 & 4095))
}

/// Ровно один входящий пакет; неподдержанное действие не становится пустым.
/// sequence=0 означает отсутствие старого клиентского предсказания; его ACK
/// не отправляется340. Номер транзакции инвентаря сюда НЕ подставляется.
pub fn translate(body: &[u8], context: Context) -> Result<Vec<u8>, Error> {
    if body.len() > 1_048_576 { return Err(Error::TooLarge); }
    let mut input = Cursor::new(body);
    let id = input.uint()?;
    let mut out = Vec::with_capacity(body.len() + 32);
    match id {
        0 => { out.push(0); uint(&mut out, input.varint()? as u32); }
        0x0b => { out.push(0x1c); out.extend(input.take(8)?); }
        0x0c..=0x0f => {
            out.push(match id { 0x0c => 0x21, 0x0d => 0x1e, 0x0e => 0x1f, _ => 0x20 });
            if matches!(id, 0x0d | 0x0e) {
                let pose = Pose { x: number(&mut input)?, y: number(&mut input)?, z: number(&mut input)?, ..context.pose };
                let pose = context.window.server_position(pose)?;
                for value in [pose.x, pose.y, pose.z] { out.extend(value.to_be_bytes()); }
            }
            if matches!(id, 0x0e | 0x0f) {
                for _ in 0..2 { out.extend(angle(&mut input)?.to_be_bytes()); }
            }
            // Новый horizontalCollision неизвестен340, это отсутствие
            // сообщения клиента, а не обход серверных проверок столкновений.
            out.push(u8::from(input.boolean()?));
        }
        0x04 => {
            out.push(0x0e);
            let locale = input.string()?;
            if locale.len() > 16 { return Err(Error::Invalid); }
            string(&mut out, locale); out.extend(input.take(1)?);
            uint(&mut out, bounded(&mut input, 2)?);
            out.push(u8::from(input.boolean()?));
            let skin = input.take(1)?[0];
            if skin & 128 != 0 { return Err(Error::Invalid); }
            out.push(skin); uint(&mut out, bounded(&mut input, 1)?);
            out.extend([0, 1, 0]); // фильтр текста, серверный список, частицы all
        }
        0x03 => { out.push(0x0c); uint(&mut out, bounded(&mut input, 1)?); }
        0x19 => {
            let action=bounded(&mut input,1)?; out.extend([0x32,action as u8]);
            if action==0 { string(&mut out,input.string()?); }
        }
        0x08 => { out.push(0x13); uint(&mut out, u32::from(input.take(1)?[0])); }
        0x1a => {
            out.push(0x35);
            let slot = input.take(2)?;
            if !(0..=8).contains(&i16::from_be_bytes(slot.try_into().map_err(|_| Error::Invalid)?)) { return Err(Error::Invalid); }
            out.extend(slot);
        }
        0x1d => { out.push(0x3f); uint(&mut out, bounded(&mut input, 1)?); }
        0x13 => {
            out.push(0x28);
            let flags = input.take(1)?[0];
            if flags & !15 != 0 { return Err(Error::Invalid); }
            // Старый клиент присылает скорости и creative/invulnerable;
            // право летать и скорости определяются сервером.
            for _ in 0..2 { if angle(&mut input)? < 0.0 { return Err(Error::Invalid); } }
            out.push(flags & 2);
        }
        0x15 => {
            let entity = input.uint()?; let action = bounded(&mut input, 8)?;
            let jump = bounded(&mut input, 100)?;
            if action < 2 {
                if jump != 0 { return Err(Error::Invalid); }
                out.extend([0x2b, if action == 0 {0x20} else {0}]);
            } else {
                out.push(0x2a); uint(&mut out,entity); uint(&mut out,action - 2); uint(&mut out,jump);
            }
        }
        0x14 => {
            out.push(0x29); uint(&mut out, bounded(&mut input, 6)?);
            let packed = u64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
            out.extend(block_position(packed, context.window)?.to_be_bytes());
            let face = input.take(1)?[0];
            if face > 5 && face != 255 { return Err(Error::Invalid); }
            out.push(face); out.push(0);
        }
        0x1f => {
            let packed = u64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
            let position = block_position(packed, context.window)?;
            let face = bounded(&mut input, 5)?;
            let hand = bounded(&mut input, 1)?;
            out.push(0x42); uint(&mut out, hand); out.extend(position.to_be_bytes()); uint(&mut out, face);
            for _ in 0..3 {
                let cursor = angle(&mut input)?;
                if !(0.0..=1.0).contains(&cursor) { return Err(Error::Invalid); }
                out.extend(cursor.to_be_bytes());
            }
            out.extend([0, 0, 0]); // insideBlock/worldBorderHit отсутствуют340
        }
        0x20 => {
            out.push(0x43); uint(&mut out, bounded(&mut input, 1)?); out.push(0);
            for value in [context.pose.yaw, context.pose.pitch] {
                if !value.is_finite() { return Err(Error::Invalid); }
                out.extend(value.to_be_bytes());
            }
        }
        0x0a => {
            let entity = input.uint()?;
            if input.uint()? != 1 { return Err(Error::PayloadUnavailable("legacy interact requires hit position")); }
            out.push(0x01); uint(&mut out, entity);
        }
        0x02 => {
            let message = input.string()?;
            if message.encode_utf16().count() > 256 { return Err(Error::TooLarge); }
            if let Some(command) = message.strip_prefix('/') { out.push(0x07); string(&mut out, command); }
            else {
                out.push(0x09); string(&mut out, message);
                out.extend([0; 22]); // unsigned: timestamp/salt/signature/offset/ack/checksum
            }
        }
        0x09 => {
            let channel = input.string()?;
            if channel != "MC|Brand" { return Err(Error::PayloadUnavailable("legacy plugin channel")); }
            let brand = input.string()?;
            if brand.len() > 32767 { return Err(Error::TooLarge); }
            out.push(0x16); string(&mut out, "minecraft:brand"); string(&mut out, brand);
        }
        _ => return Err(Error::PayloadUnavailable("legacy serverbound packet")),
    }
    if input.remaining() != 0 { return Err(Error::Invalid); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        Context { window: HeightWindow::new(-64).unwrap(), pose: Pose { x: 0.0, y: -1.0, z: 0.0, yaw: 32.0, pitch: -12.0 } }
    }
    #[test]
    fn old_entity_actions_have_explicit_native_numbers() {
        assert_eq!(translate(&[0x15,42,0,0],context()).unwrap(),[0x2b,0x20]);
        assert_eq!(translate(&[0x15,42,1,0],context()).unwrap(),[0x2b,0]);
        for action in 2..=8 { assert_eq!(translate(&[0x15,42,action,0],context()).unwrap(),[0x2a,42,action-2,0]); }
        for packet in [&[0x15,42,9,0][..], &[0x15,42,5,101], &[0x15,42,0,1]] {
            assert!(translate(packet,context()).is_err());
        }
    }
    #[test]
    fn actual_client_initial_packets_enter_the_native_format() {
        let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-client-actions.json")).unwrap();
        let packets = data["packets"].as_array().unwrap();
        assert_eq!(packets.len(), 6);
        for packet in packets {
            let hex = packet["body_hex"].as_str().unwrap();
            let bytes: Vec<u8> = hex.as_bytes().as_chunks::<2>().0.iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
            let native = translate(&bytes, context()).unwrap();
            let expected = match packet["name"].as_str().unwrap() {
                "keep_alive" => 0x1c, "settings" => 0x0e, "custom_payload" => 0x16,
                "position" => 0x1e, "position_look" => 0x1f, "teleport_confirm" => 0,
                other => panic!("unexpected observed packet {other}"),
            };
            assert_eq!(native[0], expected);
            if matches!(expected, 0x1e | 0x1f) {
                let old_y = f64::from_be_bytes(bytes[9..17].try_into().unwrap());
                let new_y = f64::from_be_bytes(native[9..17].try_into().unwrap());
                assert_eq!(new_y, old_y - 64.0);
            }
        }
    }
    #[test]
    fn old_position_uses_server_window_and_rejects_nonfinite_or_trailing_bytes() {
        let mut packet = vec![0x0e];
        for value in [-8.5f64, 1.25, -16.5] { packet.extend(value.to_be_bytes()); }
        for value in [30f32, -20.0] { packet.extend(value.to_be_bytes()); }
        packet.push(1);
        let translated = translate(&packet, context()).unwrap();
        assert_eq!(translated[0], 0x1f);
        assert_eq!(f64::from_be_bytes(translated[9..17].try_into().unwrap()), -62.75);
        for n in 0..packet.len() { assert!(translate(&packet[..n], context()).is_err()); }
        packet.push(0); assert!(translate(&packet, context()).is_err()); packet.pop();
        packet[9..17].copy_from_slice(&f64::NAN.to_be_bytes()); assert!(translate(&packet, context()).is_err());
    }
    #[test]
    fn block_bit_layout_and_negative_coordinates_are_not_reinterpreted() {
        let packed = ((-17i64 as u64 & 0x3ffffff) << 38) | (2 << 26) | (-31i64 as u64 & 0x3ffffff);
        let modern = block_position(packed, context().window).unwrap();
        assert_eq!((modern as i64) >> 38, -17);
        assert_eq!(((modern << 26) as i64) >> 38, -31);
        assert_eq!(((modern << 52) as i64) >> 52, -62);
        assert!(block_position(256 << 26, context().window).is_err());
    }
    #[test]
    fn attack_commands_and_abilities_keep_server_authority() {
        assert_eq!(translate(&[0x0a, 42, 1], context()).unwrap(), [1, 42]);
        assert!(translate(&[0x0a, 42, 2], context()).is_err());
        let mut command = vec![2]; string(&mut command, "/say 67");
        assert_eq!(translate(&command, context()).unwrap(), [7, 6, b's', b'a', b'y', b' ', b'6', b'7']);
        let mut abilities = vec![0x13, 15];
        abilities.extend(10f32.to_be_bytes()); abilities.extend(20f32.to_be_bytes());
        assert_eq!(translate(&abilities, context()).unwrap(), [0x28, 2]);
        assert!(translate(&[0x07], context()).is_err()); // транзакция не угадывается
    }
}
