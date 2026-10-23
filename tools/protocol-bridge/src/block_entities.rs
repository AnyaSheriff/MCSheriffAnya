//! Только наблюдавшиеся ID и явно проверенные схемы данных блоков.
//! Сложные компоненты предметов/текста нельзя пересылать как непрозрачный NBT.
use std::collections::BTreeMap;
use crate::{JavaVersion, registry};
#[path="block_entity_items.rs"]
mod item_data;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, UnknownType(u32), UnsupportedData, Invalid, Nbt(registry::Error) }

pub fn translate(from: JavaVersion, to: JavaVersion, kind: u32, data: &[u8]) -> Result<Option<(u32, Vec<u8>)>, Error> {
    translate_with_content(from,to,kind,data,Default::default())
}

pub(crate) fn translate_with_content(from:JavaVersion,to:JavaVersion,kind:u32,data:&[u8],content:crate::content::ContentCodec)->Result<Option<(u32,Vec<u8>)>,Error>{
    if matches!(to,JavaVersion::V1_21_5|JavaVersion::V1_21_6) || matches!(from,JavaVersion::V1_21_5|JavaVersion::V1_21_6) {
        if [from,to].iter().any(|v|matches!(v,JavaVersion::V1_12_2)){return Err(Error::UnsupportedVersion);}
        let canonical=if matches!(from,JavaVersion::V1_21_5|JavaVersion::V1_21_6) && kind>=40 {kind+1}else{kind};
        if registry::nbt_length(data).map_err(Error::Nbt)? != data.len(){return Err(Error::Invalid);}
        if canonical==48 {
            if !matches!(data,[0]|[10,0]){return Err(Error::UnsupportedData);}
        } else {
            if !crate::block_entity_ids::OBSERVED.iter().any(|r|r.0==canonical){return Err(Error::UnknownType(canonical));}
            validate(canonical,data)?;
        }
        let data=item_data::rewrite(canonical,data,from,to,content)?;
        if matches!(to,JavaVersion::V1_21_5|JavaVersion::V1_21_6) && matches!(canonical,40|48){return Ok(None);}
        return Ok(Some((if matches!(to,JavaVersion::V1_21_5|JavaVersion::V1_21_6) && canonical>40 {canonical-1}else{canonical},data)));
    }
    if to==JavaVersion::V1_21_8 || from==JavaVersion::V1_21_8 {
        let canonical=if from==JavaVersion::V1_21_8 && kind>=40 {kind+1}else{kind};
        let data=translate_with_content(JavaVersion::V26_1_2,JavaVersion::V1_21_9,canonical,data,content)?.ok_or(Error::UnsupportedData)?.1;
        if to==JavaVersion::V1_21_8 && matches!(canonical,40|48){return Ok(None);}
        return Ok(Some((if to==JavaVersion::V1_21_8 && canonical>40 {canonical-1}else{canonical},data)));
    }
    // Кровать удалена уже в26.2 (minecraft.wiki/w/Java_Edition_26.2).
    // Наблюдённые775/777 имена и порядок проверяются отдельно клиентом776;
    // новые типы вне OBSERVED этим не разрешаются.
    if [from, to].iter().any(|version| !matches!(version, JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3)) { return Err(Error::UnsupportedVersion); }
    if matches!(from, JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11) || matches!(to, JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11) {
        if !matches!((from,to),(JavaVersion::V26_1_2,JavaVersion::V1_21_11)|(JavaVersion::V1_21_11,JavaVersion::V26_1_2)|(JavaVersion::V26_1_2,JavaVersion::V1_21_6) | (JavaVersion::V26_1_2,JavaVersion::V1_21_8)|(JavaVersion::V1_21_6,JavaVersion::V26_1_2) | (JavaVersion::V1_21_8,JavaVersion::V26_1_2)|(JavaVersion::V26_1_2,JavaVersion::V1_21_9)|(JavaVersion::V1_21_9,JavaVersion::V26_1_2)) { return Err(Error::UnsupportedVersion); }
        if kind==48 { if !matches!(data,[0]|[10,0]) { return Err(Error::UnsupportedData); } return Ok(Some((kind,data.to_vec()))); }
        if !crate::block_entity_ids::OBSERVED.iter().any(|r|r.0==kind){return Err(Error::UnknownType(kind));}
        if registry::nbt_length(data).map_err(Error::Nbt)? != data.len(){return Err(Error::Invalid);}
        validate(kind,data)?;
        return Ok(Some((kind,item_data::rewrite(kind,data,from,to,content)?)));
    }
    if registry::nbt_length(data).map_err(Error::Nbt)? != data.len() { return Err(Error::Invalid); }
    // golem-statue-2026-10-02-0835.md: оригинал775 прислал48 {}.
    // В776 кровать удалена; отдельная живая проба проверяет47 и смену позы.
    if (from==JavaVersion::V26_1_2 && to==JavaVersion::V26_2 && kind==48)
        || (from==JavaVersion::V26_2 && to==JavaVersion::V26_1_2 && kind==47) {
        if !matches!(data,[0]|[10,0]){return Err(Error::UnsupportedData);}
        return Ok(Some((if to==JavaVersion::V26_2{47}else{48},data.to_vec())));
    }
    let row = crate::block_entity_ids::OBSERVED.iter().find(|(old, new, _)| {
        if from == JavaVersion::V26_1_2 { *old == kind } else { *new == Some(kind) }
    }).ok_or(Error::UnknownType(kind))?;
    let target = if to == JavaVersion::V26_1_2 { Some(row.0) } else { row.1 };
    validate(row.0, data)?;
    // В777 кровать больше не имеет отдельной блок-сущности. Для обратного
    // направления MapChunk восстанавливает её по состоянию блока.
    let data=item_data::rewrite(row.0,data,from,to,content)?;
    Ok(target.map(|id| (id,data)))
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
fn spawn_data(field: Field<'_>) -> Result<bool, Error> {
    if field.kind != 10 { return Ok(false); }
    for (key, value) in compound(field.bytes)? {
        if key != "entity" || value.kind != 10 { return Ok(false); }
        for (key, value) in compound(value.bytes)? {
            match key {
                "id" => {
                    let name = string(value)?.strip_prefix("minecraft:").ok_or(Error::UnsupportedData)?;
                    // Только общие типы: новых мобов нельзя пропускать по строке.
                    if !crate::entity_layouts::LAYOUTS[..157].iter().any(|row| row.0 == name) { return Ok(false); }
                }
                "NoAI" | "Silent" if boolean(value) => (),
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}
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
            (33 | 40, "Items") => item_data::items(field,if kind==33{4}else{3})?,
            (40, "align_items_to_bottom") => boolean(field),
            (20, "components") => field.kind == 10 && field.bytes == [0],
            (22, "Age") => field.kind == 4,
            (9, "MaxNearbyEntities" | "RequiredPlayerRange" | "SpawnCount" | "MaxSpawnDelay" |
                "SpawnRange" | "Delay" | "MinSpawnDelay") => field.kind == 2,
            (9, "SpawnData") | (44, "spawn_data") => spawn_data(field)?,
            (45, "shared_data") => item_data::shared(field)?,
            (42, "sherds") => item_data::sherds(field)?,
            (32, "joint") => matches!(string(field)?, "rollable" | "aligned"),
            (32, "name" | "pool" | "target") => string(field)? == "minecraft:empty",
            (32, "final_state") => string(field)? == "minecraft:air",
            (32, "placement_priority" | "selection_priority") => field.kind == 3,
            (21, "metadata" | "author" | "name") => field.kind == 8,
            (21, "mirror") => matches!(string(field)?, "NONE" | "LEFT_RIGHT" | "FRONT_BACK"),
            (21, "rotation") => matches!(string(field)?, "NONE" | "CLOCKWISE_90" | "CLOCKWISE_180" | "COUNTERCLOCKWISE_90"),
            (21, "mode") => matches!(string(field)?, "SAVE" | "LOAD" | "CORNER" | "DATA"),
            (21, "ignoreEntities" | "powered" | "showair" | "strict" | "showboundingbox") => boolean(field),
            (21, "seed") => field.kind == 4,
            (21, "posX" | "posY" | "posZ" | "sizeX" | "sizeY" | "sizeZ") => field.kind == 3,
            (21, "integrity") => field.kind == 5 && f32::from_be_bytes(field.bytes.try_into().map_err(|_| Error::Invalid)?).is_finite(),
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
    fn observed_types_and_validated_data_use_the_post_bed_776_table() {
        for &(old, new, _) in crate::block_entity_ids::OBSERVED {
            assert_eq!(translate(OLD, JavaVersion::V26_2, old, &[0]).unwrap(), new.map(|id| (id, vec![0])));
            if let Some(new) = new {
                assert_eq!(translate(JavaVersion::V26_2, OLD, new, &[0]).unwrap(), Some((old, vec![0])));
            }
        }
        for (from, to) in [(OLD, JavaVersion::V26_2), (JavaVersion::V26_2, OLD)] {
            assert_eq!(translate(from, to, 1, &[10, 1, 0, 1, b'x', 1, 0]), Err(Error::UnsupportedData));
            assert!(translate(from, to, 1, &[10]).is_err());
            assert!(translate(from, to, 1, &[0, 0]).is_err());
        }
        for (kind, data) in [(7, &include_bytes!("../testdata/oak_sign-26.1.2.nbt")[..]),
                             (8, &include_bytes!("../testdata/oak_hanging_sign-26.1.2.nbt")[..]),
                             (15, &include_bytes!("../testdata/beacon-26.1.2.nbt")[..])] {
            assert_eq!(translate(OLD, JavaVersion::V26_2, kind, data).unwrap(), Some((kind, data.to_vec())));
            for end in 0..data.len() { assert!(translate(OLD, JavaVersion::V26_2, kind, &data[..end]).is_err()); }
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
