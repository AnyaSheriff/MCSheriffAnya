//! Предметы в NBT полки/костра/хранилища: имена тоже требуют content remapper.
use std::collections::BTreeSet;
use super::{Error, Field, compound, string};
use crate::{JavaVersion, content::ContentCodec, generated_payload_776 as names, ids::RegistryKind, registry};

fn list(field: Field<'_>, kind: u8, limit: usize) -> Result<Vec<&[u8]>, Error> {
    if field.kind != 9 || field.bytes.len() < 5 { return Err(Error::UnsupportedData); }
    let count=i32::from_be_bytes(field.bytes[1..5].try_into().map_err(|_| Error::Invalid)?);
    if count<0 || count as usize>limit || (field.bytes[0]!=kind && !(count==0 && field.bytes[0]==0)) { return Err(Error::UnsupportedData); }
    let mut bytes=&field.bytes[5..];let mut rows=Vec::new();
    for _ in 0..count {
        let size=registry::nbt_payload_length(kind,bytes).map_err(Error::Nbt)?;
        rows.push(&bytes[..size]);bytes=&bytes[size..];
    }
    if !bytes.is_empty(){return Err(Error::Invalid);}Ok(rows)
}
fn item(data:&[u8],slots:Option<usize>)->Result<Option<u8>,Error>{
    let fields=compound(data)?;
    let id=fields.get("id").ok_or(Error::UnsupportedData)?;
    if !string(*id)?.starts_with("minecraft:"){return Err(Error::UnsupportedData);}
    let count=fields.get("count").ok_or(Error::UnsupportedData)?;
    if count.kind!=3 || !(1..=127).contains(&i32::from_be_bytes(count.bytes.try_into().map_err(|_|Error::Invalid)?)){return Err(Error::UnsupportedData);}
    let slot=if let Some(limit)=slots {
        let slot=fields.get("Slot").ok_or(Error::UnsupportedData)?;
        if slot.kind!=1 || usize::from(slot.bytes[0])>=limit{return Err(Error::UnsupportedData);}Some(slot.bytes[0])
    }else{None};
    if fields.len()!=if slots.is_some(){3}else{2}{return Err(Error::UnsupportedData);}Ok(slot)
}
pub(super) fn items(field:Field<'_>,slots:usize)->Result<bool,Error>{
    let mut seen=BTreeSet::new();
    for row in list(field,10,slots)? {
        if !seen.insert(item(row,Some(slots))?.ok_or(Error::Invalid)?){return Err(Error::Invalid);}
    }Ok(true)
}
pub(super) fn shared(field:Field<'_>)->Result<bool,Error>{
    if field.kind!=10{return Ok(false);}
    for (key,value) in compound(field.bytes)? {
        match key {
            "display_item" if value.kind==10=>{item(value.bytes,None)?;}
            "connected_players"=>{
                for uuid in list(value,11,128)? {
                    if uuid.len()!=20 || uuid[..4]!=4i32.to_be_bytes(){return Err(Error::UnsupportedData);}
                }
            }
            _=>return Ok(false),
        }
    }Ok(true)
}
pub(super) fn sherds(field:Field<'_>)->Result<bool,Error>{
    let rows=list(field,8,4)?;
    if rows.len()!=4{return Ok(false);}
    for row in rows {
        let name=string(Field{kind:8,bytes:row})?;
        if name!="minecraft:brick" && !(name.starts_with("minecraft:") && name.ends_with("_pottery_sherd")){return Ok(false);}
    }Ok(true)
}
fn table(version:JavaVersion)->Result<&'static [&'static str],Error>{
    match version {
        JavaVersion::V26_1_2=>Ok(names::ITEM_NAMES_775),JavaVersion::V26_2=>Ok(names::ITEM_NAMES_776),
        JavaVersion::V26_3=>Ok(names::ITEM_NAMES_777),_=>Err(Error::UnsupportedVersion),
    }
}
fn named(name:&str,from:JavaVersion,to:JavaVersion,content:ContentCodec)->Result<Vec<u8>,Error>{
    let bare=name.strip_prefix("minecraft:").ok_or(Error::UnsupportedData)?;
    let id=table(from)?.iter().position(|s|*s==bare).map(|id|id as u32).or_else(|| {
        if from==JavaVersion::V26_1_2 && content.is_preview() {
            crate::content::entries().find(|row|row.has_item() && row.name==bare).map(|row|row.item as u32)
        }else{None}
    }).ok_or(Error::UnsupportedData)?;
    let mapped=content.packet_id(RegistryKind::Item,from,to,id).map_err(|_|Error::UnsupportedData)?;
    let name=table(to)?.get(mapped as usize).copied().or_else(|| {
        if to==JavaVersion::V26_1_2 && content.is_preview(){crate::content::entries().find(|row|row.has_item() && row.item as u32==mapped).map(|row|row.name)}else{None}
    }).ok_or(Error::UnsupportedData)?;
    let name=format!("minecraft:{name}");let mut out=(name.len() as u16).to_be_bytes().to_vec();out.extend(name.as_bytes());Ok(out)
}
fn map_compound(mut data:&[u8],mut map:impl FnMut(&str,Field<'_>)->Result<Vec<u8>,Error>)->Result<Vec<u8>,Error>{
    let mut out=Vec::new();
    loop {
        let (&kind,rest)=data.split_first().ok_or(Error::Invalid)?;
        out.push(kind);
        if kind==0 {if !rest.is_empty(){return Err(Error::Invalid);}return Ok(out);}
        if rest.len()<2{return Err(Error::Invalid);}
        let size=usize::from(u16::from_be_bytes([rest[0],rest[1]]));
        let (header,rest)=rest.split_at_checked(size+2).ok_or(Error::Invalid)?;
        let key=std::str::from_utf8(&header[2..]).map_err(|_|Error::Invalid)?;
        out.extend(header);
        let size=registry::nbt_payload_length(kind,rest).map_err(Error::Nbt)?;
        out.extend(map(key,Field{kind,bytes:&rest[..size]})?);data=&rest[size..];
    }
}
pub(super) fn rewrite(kind:u32,data:&[u8],from:JavaVersion,to:JavaVersion,content:ContentCodec)->Result<Vec<u8>,Error>{
    if data.first()!=Some(&10) || !matches!(kind,33|40|45){return Ok(data.to_vec());}
    let map_item=|data:&[u8]|map_compound(data,|key,field| {
        if key=="id" {named(string(field)?,from,to,content)}else{Ok(field.bytes.to_vec())}
    });
    let mut out=vec![10];
    out.extend(map_compound(&data[1..],|key,field| {
        if matches!(kind,33|40) && key=="Items" {
            let rows=list(field,10,if kind==33{4}else{3})?;
            let mut bytes=field.bytes[..5].to_vec();
            for row in rows {bytes.extend(map_item(row)?);}Ok(bytes)
        }else if kind==45 && key=="shared_data" {
            map_compound(field.bytes,|key,field| {
                if key=="display_item" {map_item(field.bytes)}else{Ok(field.bytes.to_vec())}
            })
        }else{Ok(field.bytes.to_vec())}
    })?);Ok(out)
}
