//! Остальные компоненты, которые реально пишет encode_components ядра775.
//! Порядок динамических реестров берётся из общего Configuration соединения.
use std::collections::BTreeSet;
use crate::{JavaVersion, content::IdSpace, ids::RegistryKind, items::{self,Context,Error,Reader,fields,nbt_string,plain_text,uint}};

fn text(input:&mut Reader<'_>)->Result<(),Error>{plain_text(input.nbt()?) }
fn string(input:&mut Reader<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    let n=input.count(32767)?;let value=input.take(n)?;std::str::from_utf8(value).map_err(|_|Error::Invalid)?;uint(out,n as u32);out.extend(value);Ok(())
}
fn skip_string(input:&mut Reader<'_>)->Result<(),Error>{string(input,&mut Vec::new())}
fn float(input:&mut Reader<'_>)->Result<(),Error>{let value=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);if !value.is_finite() || value<0.0{return Err(Error::Invalid);}Ok(())}
fn holder(input:&mut Reader<'_>)->Result<bool,Error>{Ok(input.count(4096)?==0)}
fn explosion(input:&mut Reader<'_>)->Result<(),Error>{
    input.count(4)?;for _ in 0..2 {let n=input.count(256)?;input.take(n*4)?;}input.boolean()?;input.boolean()?;Ok(())
}
type NbtField<'a>=(String,u8,&'a[u8]);
fn compound(data:&[u8])->Result<Vec<NbtField<'_>>,Error>{
    if data.first()!=Some(&10){return Err(Error::Invalid);}fields(&data[1..])
}
fn bucket(data:&[u8],bee:bool)->Result<(),Error>{
    for (key,kind,value) in compound(data)? {
        match (key.as_str(),kind) {
            ("Age"|"HuntingCooldown",3)=>(),
            ("InLove"|"CannotEnterHiveTicks",3) if bee=>(),
            ("NoAI"|"Silent",1) if matches!(value,[0]|[1])=>(),
            ("HasNectar"|"HasStung",1) if bee && matches!(value,[0]|[1])=>(),
            ("Health",5) if f32::from_be_bytes(value.try_into().map_err(|_|Error::Invalid)?).is_finite()=>(),
            ("id",8) if bee && nbt_string(value)?=="minecraft:bee"=>(),
            ("CustomName",8) if bee=>{nbt_string(value)?;}
            ("UUID",11) if bee && value.len()==20 && value[..4]==4i32.to_be_bytes()=>(),
            _=>return Err(Error::UnsupportedComponent(if bee{77}else{59})),
        }
    }Ok(())
}

pub(crate) fn remap(kind:u32,input:&mut Reader<'_>,context:Context,spaces:(IdSpace,IdSpace))->Result<Vec<u8>,Error>{
    // Наблюдение payload-actions-instrument775: у776 instrument — holder без bool.
    // Внутренний writer775 хранит прежнюю bool+holder форму. JSON здесь неточен.
    if kind==61 {
        let legacy=!matches!(spaces.0,IdSpace::Wire(JavaVersion::V1_21_4 | JavaVersion::V26_2));
        if legacy && !input.boolean()? {return Err(Error::UnsupportedComponent(61));}
        let direct=input.0;
        if holder(input)? {
            if input.uint()?!=0{return Err(Error::UnsupportedComponent(61));}
            skip_string(input)?;if input.boolean()?{float(input)?;}float(input)?;float(input)?;text(input)?;
        }
        let mut out=Vec::new();
        if !matches!(spaces.1,IdSpace::Wire(JavaVersion::V1_21_4 | JavaVersion::V26_2)){out.push(1);}
        out.extend(&direct[..direct.len()-input.0.len()]);return Ok(out);
    }
    if kind==77 {
        // payload-actions-bees775 и774-client775-v6: тип сущности перед compound.
        // Общий JSON этого VarInt не содержит; принимаем только пчелу ядра.
        let bee=crate::entity_layouts::LAYOUTS.iter().position(|row|row.0=="bee").ok_or(Error::Invalid)? as u32;
        let wire_id=|space|match space {IdSpace::Wire(version @ (JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_2))=>crate::ids::translate(RegistryKind::Entity,JavaVersion::V26_1_2,version,bee).map(Some).map_err(Error::Id),_=>Ok(None)};
        let source_type=wire_id(spaces.0)?;let target_type=wire_id(spaces.1)?;
        let count=input.count(3)?;let mut out=Vec::new();uint(&mut out,count as u32);
        for _ in 0..count {
            if let Some(wire)=source_type && input.uint()?!=wire {return Err(Error::UnsupportedComponent(77));}
            let nbt=input.nbt()?;bucket(nbt,true)?;
            if let Some(wire)=target_type {uint(&mut out,wire);}
            // У обычного клиента id может быть вынесен в VarInt; внутреннее
            // сохранение пчелы по-прежнему получает именованный id.
            if target_type.is_none() && !compound(nbt)?.iter().any(|(key,_,_)|key=="id") {
                out.extend([10,8,0,2,b'i',b'd',0,13]);out.extend(b"minecraft:bee");out.extend(&nbt[1..]);
            }else{out.extend(nbt);}
            uint(&mut out,input.uint()?);uint(&mut out,input.uint()?);
        }return Ok(out);
    }
    if kind==56 && (matches!(spaces.0,IdSpace::Wire(JavaVersion::V1_21_4)) || matches!(spaces.1,IdSpace::Wire(JavaVersion::V1_21_4))) {
        return crate::client_769::trim(input,spaces);
    }
    let start=input.0;
    match kind {
        53=>{for _ in 0..input.count(32)? {input.count(39)?;input.uint()?;}}
        56=>{
            if holder(input)? {
                skip_string(input)?;
                for _ in 0..input.count(256)? {skip_string(input)?;skip_string(input)?;}text(input)?;
            }
            if holder(input)? {skip_string(input)?;text(input)?;input.boolean()?;}
        }
        57=>{for (_,kind,value) in compound(input.nbt()?)? {if kind!=8{return Err(Error::Invalid);}nbt_string(value)?;}}
        59=>bucket(input.nbt()?,false)?,
        66=>{
            let data=input.nbt()?;
            if data.len()<6 || data[..2]!=[9,8]{return Err(Error::Invalid);}
            let count=i32::from_be_bytes(data[2..6].try_into().map_err(|_|Error::Invalid)?);
            if !(0..=1024).contains(&count){return Err(Error::TooLarge);}
            let mut rest=&data[6..];
            for _ in 0..count {
                let size=crate::registry::nbt_payload_length(8,rest).map_err(Error::Nbt)?;nbt_string(&rest[..size])?;rest=&rest[size..];
            }
            if !rest.is_empty(){return Err(Error::Invalid);}
        }
        68=>explosion(input)?,
        69=>{input.count(255)?;for _ in 0..input.count(256)?{explosion(input)?;}}
        72=>{for _ in 0..input.count(256)? {if holder(input)? {skip_string(input)?;skip_string(input)?;}input.count(15)?;}}
        74=>{
            let count=input.count(4)?;let mut out=Vec::new();uint(&mut out,count as u32);
            for _ in 0..count{uint(&mut out,items::content_id(context,RegistryKind::Item,spaces.0,spaces.1,input.uint()?)?);}return Ok(out);
        }
        76=>{
            let mut seen=BTreeSet::new();
            for _ in 0..input.count(256)? {
                let n=input.count(32767)?;let key=std::str::from_utf8(input.take(n)?).map_err(|_|Error::Invalid)?;
                if !seen.insert(key){return Err(Error::Invalid);}skip_string(input)?;
            }
        }
        86|88|89|90|104=>{input.count(match kind{86=>2,88=>11,104=>4,_=>15})?;}
        _=>return Err(Error::UnsupportedComponent(kind)),
    }
    Ok(start[..start.len()-input.0.len()].to_vec())
}

pub(crate) fn supported_pair(from:JavaVersion,to:JavaVersion)->bool{
    matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_1_2|JavaVersion::V26_2) && matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_1_2|JavaVersion::V26_2)
}

/// Equippable770 заканчивается на equipOnInteract, shearable появился771.
pub(crate) fn equippable_770(input:&mut Reader<'_>,from:JavaVersion,to:JavaVersion,context:Context,spaces:(IdSpace,IdSpace))->Result<Vec<u8>,Error>{
    let mut out=Vec::new();let slot=input.count(if from==JavaVersion::V1_21_4 {6}else{7})?;uint(&mut out,if to==JavaVersion::V1_21_4 && slot==7 {6}else{slot as u32});
    let sound=input.uint()?;if sound!=0{return Err(Error::UnsupportedComponent(32));}uint(&mut out,sound);
    if sound==0 {string(input,&mut out)?;let range=input.boolean()?;out.push(u8::from(range));if range {let data=input.0;float(input)?;out.extend(&data[..4]);}}
    for _ in 0..2 {let present=input.boolean()?;out.push(u8::from(present));if present {string(input,&mut out)?;}}
    let present=input.boolean()?;out.push(u8::from(present));
    if present {let n=input.count(4097)?;uint(&mut out,n as u32);if n==0 {string(input,&mut out)?;}else{for _ in 1..n {uint(&mut out,context.content.map(RegistryKind::Entity,spaces.0,spaces.1,input.uint()?).map_err(Error::Content)?);}}}
    for _ in 0..3 {out.push(u8::from(input.boolean()?));}
    let interact=if from==JavaVersion::V1_21_4 {false}else{input.boolean()?};
    if to!=JavaVersion::V1_21_4 {out.push(u8::from(interact));}
    let mut tail=Vec::new();
    if !matches!(from,JavaVersion::V1_21_4 | JavaVersion::V1_21_5) {tail.push(u8::from(input.boolean()?));let sound=input.uint()?;if sound!=0{return Err(Error::UnsupportedComponent(32));}uint(&mut tail,sound);if sound==0 {string(input,&mut tail)?;let range=input.boolean()?;tail.push(u8::from(range));if range {let data=input.0;float(input)?;tail.extend(&data[..4]);}}}
    // [проверить] У770 нет shearingSound; выбранный именованный звук
    // заполняет только выключенное поле shearable=false, не игровые правила.
    else {tail.extend([0,0]);uint(&mut tail,26);tail.extend(b"minecraft:entity.sheep.shear");tail.push(0);}
    if !matches!(to,JavaVersion::V1_21_4 | JavaVersion::V1_21_5) {out.extend(tail);}Ok(out)
}
