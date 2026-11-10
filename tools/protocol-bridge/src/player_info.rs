//! Ограниченное тело PlayerInfo768↔769, без packet ID и без допуска клиента.
//! Pinned JSON: testdata/player-info-768-769-layouts.json, «update_list_order».
//! Поля профиля/chat/plain text разбираются общим кодеком прежнего пути769–772.
//! Непредставимый showHat не превращается в list_order и не пропускается молча.
use crate::wire::{Cursor, Error, uint};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_PLAYERS: usize = 4096;
const MAX_PROPERTIES: usize = 256;
const MAX_KEY_BYTES: usize = 8192;

fn string(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    let value = input.string()?;
    uint(out, value.len() as u32);
    out.extend(value.as_bytes());
    Ok(())
}

fn scalar(input: &mut Cursor<'_>, out: &mut Vec<u8>, signed: bool) -> Result<(), Error> {
    let value = if signed { input.varint()? as u32 } else { input.uint()? };
    uint(out, value);
    Ok(())
}

/// Общий прежний разбор: вызывающий слой задаёт смысл двух старших битов.
/// У нового ребра latency/listPriority имеют полный signed VarInt из JSON;
/// прежний native writer сохраняет собственные ограничения без изменения.
fn entries(input: &mut Cursor<'_>, out: &mut Vec<u8>, flags: u8, list_order: u8, hat: u8, signed_values: bool) -> Result<(), Error> {
    let count = input.count(MAX_PLAYERS)?;
    uint(out, count as u32);
    for _ in 0..count {
        out.extend(input.take(16)?);
        if flags & 1 != 0 {
            string(input, out)?;
            let count = input.count(MAX_PROPERTIES)?;
            uint(out, count as u32);
            for _ in 0..count {
                string(input, out)?;
                string(input, out)?;
                let signed = input.boolean()?;
                out.push(u8::from(signed));
                if signed { string(input, out)?; }
            }
        }
        if flags & 2 != 0 {
            let present = input.boolean()?;
            out.push(u8::from(present));
            if present {
                out.extend(input.take(24)?);
                for _ in 0..2 {
                    let count = input.count(MAX_KEY_BYTES)?;
                    uint(out, count as u32);
                    out.extend(input.take(count)?);
                }
            }
        }
        if flags & 4 != 0 { uint(out, input.count(3)? as u32); }
        if flags & 8 != 0 { out.push(u8::from(input.boolean()?)); }
        if flags & 16 != 0 { scalar(input, out, signed_values)?; }
        if flags & 32 != 0 {
            let present = input.boolean()?;
            out.push(u8::from(present));
            if present {
                let length = crate::registry::nbt_length(input.rest).map_err(|_| Error::Invalid)?;
                let text = input.take(length)?;
                crate::items::plain_text(text).map_err(Error::Item)?;
                out.extend(text);
            }
        }
        if flags & list_order != 0 { scalar(input, out, signed_values)?; }
        if flags & hat != 0 { out.push(u8::from(input.boolean()?)); }
    }
    Ok(())
}

/// Тот же формат, который прежний wire builder отправляет769–772.
/// Границу всего пакета и завершающий хвост по-прежнему проверяет wire.
pub(crate) fn modern(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    let flags = input.take(1)?[0];
    out.push(flags);
    entries(input, out, flags, 0x80, 0x40, false)
}

/// Только старый list_order0x40 становится новым0x80; showHat не выдумывается.
pub fn from_768(payload: &[u8]) -> Result<Vec<u8>, Error> {
    translate(payload, false)
}

/// Без доказанного старого представления showHat отклоняется даже для false.
pub fn from_769(payload: &[u8]) -> Result<Vec<u8>, Error> {
    translate(payload, true)
}

fn translate(payload: &[u8], from_new: bool) -> Result<Vec<u8>, Error> {
    if payload.len() > MAX_BYTES { return Err(Error::TooLarge); }
    let mut input = Cursor::new(payload);
    let flags = input.take(1)?[0];
    if from_new {
        if flags & 0x40 != 0 { return Err(Error::PayloadUnavailable("768 player info showHat requires context")); }
    } else if flags & 0x80 != 0 { return Err(Error::Invalid); }
    let mapped = (flags & 0x3f) | if from_new { (flags & 0x80) >> 1 } else { (flags & 0x40) << 1 };
    let mut out = Vec::with_capacity(payload.len());
    out.push(mapped);
    entries(&mut input, &mut out, flags, if from_new { 0x80 } else { 0x40 }, 0, true)?;
    if input.remaining() != 0 { return Err(Error::Invalid); }
    // Прежний parser канонизирует VarInt. Новое соседнее ребро вместо этого
    // отказывает нестандартной записи: после флага каждый исходный байт сохранён.
    if out[1..] != payload[1..] { return Err(Error::Invalid); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Vec<u8> {
        let mut out = vec![8];
        out.extend((value.len() as u16).to_be_bytes());
        out.extend(value.as_bytes());
        out
    }

    fn put_string(out: &mut Vec<u8>, value: &str) {
        uint(out, value.len() as u32); out.extend(value.as_bytes());
    }

    fn fixture(flags: u8, modern: bool, optional: bool) -> Vec<u8> {
        let mut out = vec![flags, 2];
        for id in [11, 42] {
            out.extend([id; 16]);
            if flags & 1 != 0 {
                put_string(&mut out, "ViaProbe");
                out.push(1);
                put_string(&mut out, "textures"); put_string(&mut out, "synthetic-value");
                out.push(u8::from(optional));
                if optional { put_string(&mut out, "synthetic-signature"); }
            }
            if flags & 2 != 0 {
                out.push(u8::from(optional));
                if optional {
                    out.extend([55; 16]); out.extend(1234i64.to_be_bytes());
                    out.extend([3, 1, 2, 3, 2, 4, 5]);
                }
            }
            if flags & 4 != 0 { out.push(2); }
            if flags & 8 != 0 { out.push(1); }
            if flags & 16 != 0 { uint(&mut out, 150); }
            if flags & 32 != 0 {
                out.push(u8::from(optional));
                if optional { out.extend(text("Тест")); }
            }
            let list_order = if modern { 0x80 } else { 0x40 };
            if flags & list_order != 0 { uint(&mut out, 300); }
            if modern && flags & 0x40 != 0 { out.push(1); }
        }
        out
    }

    #[test]
    fn pinned_action_bits_and_common_nested_layouts_match_the_contract() {
        let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/player-info-768-769-layouts.json")).unwrap();
        let old = &data["versions"]["768"];
        let new = &data["versions"]["769"];
        let flags = |value: &serde_json::Value| value["player_info"][1][0]["type"][1]["flags"].as_array().unwrap().clone();
        let a = flags(old); let b = flags(new);
        assert_eq!(&a[..6], &b[..6]);
        assert_eq!(a[6], "update_list_order");
        assert_eq!(b[6], "update_hat"); assert_eq!(b[7], "update_list_order");
        assert_eq!(old["game_profile"], new["game_profile"]);
        assert_eq!(old["chat_session"], new["chat_session"]);
        for value in [old, new] {
            assert!(value["source"]["url"].as_str().unwrap().contains("/924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/"));
        }
    }

    #[test]
    fn independent_json_field_order_fixtures_preserve_every_nonflag_byte() {
        let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/player-info-768-769-layouts.json")).unwrap();
        let bytes = |hex: &str| hex.as_bytes().as_chunks::<2>().0.iter().map(|part|
            u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap()).collect::<Vec<_>>();
        let cases = data["synthetic_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 4);
        for row in cases {
            let old = bytes(row["body768_hex"].as_str().unwrap());
            let new = bytes(row["body769_hex"].as_str().unwrap());
            assert_eq!(from_768(&old).unwrap(), new);
            assert_eq!(from_769(&new).unwrap(), old);
            assert_eq!(&new[1..], &old[1..]);
        }
    }

    #[test]
    fn every_old_action_combination_preserves_rows_uuid_chat_and_text_bytes() {
        for flags in 0..=0x7f {
            for optional in [false, true] {
                let old = fixture(flags, false, optional);
                let new = from_768(&old).unwrap();
                assert_eq!(new[0], (flags & 0x3f) | ((flags & 0x40) << 1));
                assert_eq!(&new[1..], &old[1..]);
                assert_eq!(from_769(&new).unwrap(), old);
            }
        }
    }

    #[test]
    fn adjacent_signed_latency_and_priority_keep_all_i32_bits() {
        for latency in [i32::MIN, -1, 0, i32::MAX] {
            for priority in [i32::MIN, -1, 0, i32::MAX] {
                let mut old = vec![0x50, 1]; old.extend([9; 16]);
                uint(&mut old, latency as u32); uint(&mut old, priority as u32);
                let new = from_768(&old).unwrap();
                assert_eq!(new[0], 0x90);
                assert_eq!(&new[1..], &old[1..]);
                assert_eq!(from_769(&new).unwrap(), old);
            }
        }
    }

    #[test]
    fn shared_modern_parser_retains_hat_and_list_order_without_a_route_change() {
        for flags in 0..=u8::MAX {
            let body = fixture(flags, true, true);
            let mut input = Cursor::new(&body);
            let mut out = Vec::new();
            modern(&mut input, &mut out).unwrap();
            assert_eq!(input.remaining(), 0);
            assert_eq!(out, body);
        }
    }

    #[test]
    fn hat_and_unknown_old_flag_refuse_instead_of_changing_the_wrong_field() {
        for flags in [0x40, 0x80, 0xc0, 0xff] {
            let old = fixture(flags, false, true);
            if flags & 0x80 != 0 { assert_eq!(from_768(&old), Err(Error::Invalid)); }
            if flags & 0x40 != 0 {
                let new = fixture(flags, true, true);
                assert_eq!(from_769(&new), Err(Error::PayloadUnavailable("768 player info showHat requires context")));
            }
        }
        assert_eq!(from_769(&[0x40, 0]), Err(Error::PayloadUnavailable("768 player info showHat requires context")));
    }

    #[test]
    fn truncated_trailing_oversized_and_noncanonical_fields_are_rejected() {
        let body = fixture(0x7f, false, true);
        for length in 0..body.len() { assert!(from_768(&body[..length]).is_err()); }
        let new = from_768(&body).unwrap();
        for length in 0..new.len() { assert!(from_769(&new[..length]).is_err()); }
        let mut trailing = body; trailing.push(0);
        assert_eq!(from_768(&trailing), Err(Error::Invalid));
        assert_eq!(from_768(&vec![0; MAX_BYTES + 1]), Err(Error::TooLarge));
        // Ноль игроков/приоритет должны иметь ровно каноническую VarInt-запись.
        assert_eq!(from_768(&[0, 0x80, 0]), Err(Error::Invalid));
        let mut priority = vec![0x40, 1]; priority.extend([0; 16]); priority.extend([0x80, 0]);
        assert_eq!(from_768(&priority), Err(Error::Invalid));
        for value in [vec![0x80], vec![0xff; 6], vec![0xff,0xff,0xff,0xff,0x10]] {
            let mut body = vec![0]; body.extend(value); assert!(from_768(&body).is_err());
        }
    }

    #[test]
    fn counts_lengths_boolean_modes_and_utf8_are_strictly_bounded() {
        let mut players = vec![0]; uint(&mut players, (MAX_PLAYERS + 1) as u32);
        assert_eq!(from_768(&players), Err(Error::TooLarge));
        let one = |flags| { let mut out = vec![flags, 1]; out.extend([0; 16]); out };
        let mut properties = one(1); put_string(&mut properties, "X"); uint(&mut properties, (MAX_PROPERTIES + 1) as u32);
        assert_eq!(from_768(&properties), Err(Error::TooLarge));
        let mut key = one(2); key.push(1); key.extend([0; 24]); uint(&mut key, (MAX_KEY_BYTES + 1) as u32);
        assert_eq!(from_768(&key), Err(Error::TooLarge));
        let mut string = one(1); uint(&mut string, 32768);
        assert_eq!(from_768(&string), Err(Error::TooLarge));
        let mut utf8 = one(1); utf8.extend([1, 255, 0]);
        assert_eq!(from_768(&utf8), Err(Error::Invalid));
        let mut mode = one(4); mode.push(4);
        assert_eq!(from_768(&mode), Err(Error::TooLarge));
        for (flag, invalid) in [(2, 2), (8, 2), (32, 2)] {
            let mut body = one(flag); body.push(invalid);
            assert_eq!(from_768(&body), Err(Error::Invalid));
        }
        let mut signature = one(1); put_string(&mut signature, "X"); signature.push(1);
        put_string(&mut signature, "A"); put_string(&mut signature, "B"); signature.push(2);
        assert_eq!(from_768(&signature), Err(Error::Invalid));
    }

    #[test]
    fn unsupported_nested_text_is_not_forwarded_as_plain_text() {
        // Анонимный compound с text и неизвестным полем: весь пакет отклоняется.
        let mut nbt = vec![10, 8, 0, 4]; nbt.extend(b"text"); nbt.extend([0, 1, b'X']);
        nbt.extend([8, 0, 7]); nbt.extend(b"unknown"); nbt.extend([0, 1, b'Y', 0]);
        let mut body = vec![0x20, 1]; body.extend([0; 16]); body.push(1); body.extend(nbt);
        assert_eq!(from_768(&body), Err(Error::Item(crate::items::Error::UnsupportedComponent(6))));
    }
}
