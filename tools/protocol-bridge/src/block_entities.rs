//! Только наблюдавшиеся ID и явно проверенные схемы данных блоков.
//! Сложные компоненты предметов/текста нельзя пересылать как непрозрачный NBT.
use std::collections::BTreeMap;
use crate::{JavaVersion, registry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, UnknownType(u32), UnsupportedData, Invalid, Nbt(registry::Error) }

pub fn translate(from: JavaVersion, to: JavaVersion, kind: u32, data: &[u8]) -> Result<Option<(u32, Vec<u8>)>, Error> {
    if [from, to].iter().any(|version| !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3)) { return Err(Error::UnsupportedVersion); }
    if registry::nbt_length(data).map_err(Error::Nbt)? != data.len() { return Err(Error::Invalid); }
    let row = crate::block_entity_ids::OBSERVED.iter().find(|(old, new, _)| {
        if from == JavaVersion::V26_1_2 { *old == kind } else { *new == Some(kind) }
    }).ok_or(Error::UnknownType(kind))?;
    let target = if to == JavaVersion::V26_1_2 { Some(row.0) } else { row.1 };
    validate(row.0, data)?;
    // В777 кровать больше не имеет отдельной блок-сущности. Для обратного
    // направления MapChunk восстанавливает её по состоянию блока.
    Ok(target.map(|id| (id, data.to_vec())))
}

#[derive(Clone, Copy)]
struct Field<'a> { kind: u8, bytes: &'a [u8] }
fn compound(mut bytes: &[u8]) -> Result<BTreeMap<&str, Field<'_>>, Error> {
    let mut out = BTreeMap::new();
    loop {
        let (&kind, rest) = bytes.split_first().ok_or(Error::Invalid)?;
        if kind == 0 { return if rest.is_empty() { Ok(out) } else { Err(Error::Invalid) }; }
        if rest.len() < 2 { return Err(Error::Invalid); }
        let size = usize::from(u16::from_be_bytes([rest[0], rest[1]]));
        let (name, rest) = rest[2..].split_at_checked(size).ok_or(Error::Invalid)?;
        let name = std::str::from_utf8(name).map_err(|_| Error::Invalid)?;
        let size = registry::nbt_payload_length(kind, rest).map_err(Error::Nbt)?;
        if out.insert(name, Field { kind, bytes: &rest[..size] }).is_some() { return Err(Error::Invalid); }
        bytes = &rest[size..];
    }
}
fn boolean(field: Field<'_>) -> bool { field.kind == 1 && matches!(field.bytes, [0] | [1]) }
fn string(field: Field<'_>) -> Result<&str, Error> {
    if field.kind != 8 || field.bytes.len() < 2 { return Err(Error::UnsupportedData); }
    std::str::from_utf8(&field.bytes[2..]).map_err(|_| Error::UnsupportedData)
}
fn empty_items(field: Field<'_>) -> bool { field.kind == 9 && matches!(field.bytes, [0 | 10, 0, 0, 0, 0]) }
fn sign_side(field: Field<'_>) -> Result<(), Error> {
    if field.kind != 10 { return Err(Error::UnsupportedData); }
    for (key, field) in compound(field.bytes)? {
        let valid = match key {
            "has_glowing_text" => boolean(field),
            "color" => matches!(string(field)?, "white" | "orange" | "magenta" | "light_blue" | "yellow" | "lime" | "pink" | "gray" | "light_gray" | "cyan" | "purple" | "blue" | "brown" | "green" | "red" | "black"),
            // Только строковые компоненты. Rich text с событиями/предметами
            // требует отдельного переводчика и сюда не проходит.
            "messages" | "filtered_messages" => field.kind == 9 && field.bytes.starts_with(&[8, 0, 0, 0, 4]),
            _ => false,
        };
        if !valid { return Err(Error::UnsupportedData); }
    }
    Ok(())
}
fn validate(kind: u32, data: &[u8]) -> Result<(), Error> {
    if data == [0] || data == [10, 0] { return Ok(()); }
    if data.first() != Some(&10) { return Err(Error::UnsupportedData); }
    for (key, field) in compound(&data[1..])? {
        let valid = match (kind, key) {
            (7 | 8, "is_waxed") => boolean(field),
            (7 | 8, "front_text" | "back_text") => { sign_side(field)?; true }
            (15, "Levels") => field.kind == 3,
            (15, "primary_effect" | "secondary_effect") => matches!(string(field)?, "minecraft:speed" | "minecraft:haste" | "minecraft:resistance" | "minecraft:jump_boost" | "minecraft:strength" | "minecraft:regeneration"),
            (33 | 40, "Items") => empty_items(field),
            (40, "align_items_to_bottom") => boolean(field),
            (20, "components") => field.kind == 10 && field.bytes == [0],
            (22, "Age") => field.kind == 4,
            _ => false,
        };
        if !valid { return Err(Error::UnsupportedData); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};
    #[test]
    fn observed_ids_and_plain_sign_data_translate_both_directions() {
        for &(old, new, _) in crate::block_entity_ids::OBSERVED {
            assert_eq!(translate(OLD, NEW, old, &[0]).unwrap(), new.map(|id| (id, vec![0])));
            if let Some(new) = new { assert_eq!(translate(NEW, OLD, new, &[0]).unwrap(), Some((old, vec![0]))); }
        }
        let pairs: [(u32, &[u8], &[u8]); 3] = [
            (7, include_bytes!("../testdata/oak_sign-26.1.2.nbt"), include_bytes!("../testdata/oak_sign-26.3.nbt")),
            (8, include_bytes!("../testdata/oak_hanging_sign-26.1.2.nbt"), include_bytes!("../testdata/oak_hanging_sign-26.3.nbt")),
            (15, include_bytes!("../testdata/beacon-26.1.2.nbt"), include_bytes!("../testdata/beacon-26.3.nbt")),
        ];
        for (kind, old, new) in pairs {
            assert_eq!(translate(OLD, NEW, kind, old).unwrap(), Some((kind, new.to_vec())));
            assert_eq!(translate(NEW, OLD, kind, new).unwrap(), Some((kind, old.to_vec())));
        }
    }
    #[test]
    fn unsupported_nbt_and_malformed_lengths_never_pass_through() {
        assert_eq!(translate(OLD, NEW, 10, &[0]), Err(Error::UnknownType(10)));
        let sign = include_bytes!("../testdata/oak_sign-26.1.2.nbt");
        for n in 0..sign.len() { assert!(translate(OLD, NEW, 7, &sign[..n]).is_err()); }
        let mut extra = sign.to_vec(); extra.push(0); assert!(translate(OLD, NEW, 7, &extra).is_err());
        // Валидный, но неизвестный компонент; не выдаём его за совместимые данные.
        assert_eq!(translate(OLD, NEW, 1, &[10, 1, 0, 1, b'x', 1, 0]), Err(Error::UnsupportedData));
        let duplicate = [10, 1, 0, 8, b'i', b's', b'_', b'w', b'a', b'x', b'e', b'd', 0, 1, 0, 8, b'i', b's', b'_', b'w', b'a', b'x', b'e', b'd', 1, 0];
        assert_eq!(translate(OLD, NEW, 7, &duplicate), Err(Error::Invalid));
    }
}
