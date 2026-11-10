//! Private768: отдельная версия, typed соседнее ребро и reference registry context.
//! Pinned protocol.json «packet_position» / «Slot». Не alias payload769.
//! Незавершённые формы возвращают отказ; public admission не изменяется.
use std::collections::{BTreeMap,BTreeSet};
use crate::{State,Direction,JavaVersion,ids::{RegistryKind,IdTranslationError},wire::{Cursor,uint,Error}};
#[path="generated_private768.rs"]
mod tables;

pub const VERSION:JavaVersion=JavaVersion::V1_21_3;
const MAX_BYTES:usize=8*1024*1024;

/// Compile feature не равен допуску: нужен отдельный opt-in, loopback и primary777.
pub fn private_admission(protocol:i32,flag:bool,loopback:bool,primary:JavaVersion)->bool {
    protocol==768 && flag && loopback && primary==JavaVersion::V26_3
}

/// Точная статическая цель по именам/полным block-state properties, не числам.
pub fn map_id(kind:RegistryKind,source:u16,id:u32)->Result<u32,Error>{
    let (count,ranges)=tables::ranges(kind,source).ok_or(Error::Invalid)?;
    if id>=count{return Err(Error::Id(IdTranslationError::UnknownId(id)));}
    let &(_,_,offset)=ranges.iter().find(|&&(a,b,_)|a<=id&&id<=b).ok_or(Error::Invalid)?;
    if offset==65535{return Err(Error::Id(IdTranslationError::MissingTarget(id)));}
    u32::try_from(i64::from(id)+i64::from(offset)).map_err(|_|Error::Invalid)
}

pub fn packet_info(state:State,direction:Direction,source:u16,id:u32)->Result<(&'static str,u32,bool),Error>{
    let &(name,target,equal)=tables::packets(state,direction,source).and_then(|r|r.get(id as usize)).ok_or(Error::Invalid)?;
    if target==65535{return Err(Error::PayloadUnavailable(name));}
    Ok((name,u32::from(target),equal))
}

/// Полные12 RegistryData из обычного headless входа в original1.21.3. NBT768
/// не производится builder-ом769 и не считается общим только по schema equality.
pub fn reference_registries()->Result<Vec<crate::registry::Registry>,Error>{
    let raw=include_bytes!("../testdata/private768-reference/registry-network.bin");
    let mut input=Cursor::new(raw);
    if input.take(4)?!=b"R768"{return Err(Error::Invalid);}
    let count=u32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
    if count!=12{return Err(Error::Invalid);}
    let mut result=Vec::new();let mut names=BTreeSet::new();let mut entries=0;
    for _ in 0..count {
        let size=u32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?) as usize;
        if size>MAX_BYTES{return Err(Error::TooLarge);}
        let mut body=Cursor::new(input.take(size)?);
        if body.uint()?!=7{return Err(Error::Invalid);}
        let registry=crate::registry::Registry::decode(VERSION,body.rest).map_err(|_|Error::Invalid)?;
        if !names.insert(registry.name.clone())||registry.entries.iter().any(|r|r.data.is_none()){return Err(Error::Invalid);}
        entries+=registry.entries.len();result.push(registry);
    }
    if input.remaining()!=0||entries!=323{return Err(Error::Invalid);}
    Ok(result)
}

/// Число записей само по себе не доказывает совпадение ID: каждое отображение
/// строится по именам исходного и фактически отправляемого целевого реестра.
#[derive(Clone,Default)]
pub struct RegistryContext { maps:BTreeMap<String,Vec<Option<u32>>> }
impl RegistryContext {
    pub fn new(source:&[crate::registry::Registry],target:&[crate::registry::Registry])->Result<Self,Error>{
        let mut maps=BTreeMap::new();let mut targets=BTreeMap::new();
        for registry in target {if targets.insert(registry.name.as_str(),registry).is_some(){return Err(Error::Invalid);}}
        for registry in source {
            let target=targets.get(registry.name.as_str()).ok_or_else(||Error::RegistryUnavailable(registry.name.clone()))?;
            let names:BTreeMap<_,_>=target.entries.iter().enumerate().map(|(i,e)|(e.name.as_str(),i as u32)).collect();
            if names.len()!=target.entries.len(){return Err(Error::Invalid);}
            let mapping=registry.entries.iter().map(|e|names.get(e.name.as_str()).copied()).collect();
            if maps.insert(registry.name.clone(),mapping).is_some(){return Err(Error::Invalid);}
        }
        Ok(Self{maps})
    }
    pub fn map(&self,registry:&str,id:u32)->Result<u32,Error>{
        self.maps.get(registry).and_then(|v|v.get(id as usize)).copied().flatten()
            .ok_or_else(||Error::RegistryUnavailable(registry.into()))
    }
}

/// Только типизированные proven формы. В частности schema_equal НЕ открывает
/// generic копирование metadata/chunk/recipe с вложенными чужими ID.
pub fn play_edge(direction:Direction,source:u16,body:&[u8])->Result<Vec<u8>,Error>{
    if !matches!(source,768|769)||body.len()>MAX_BYTES{return Err(Error::Invalid);}
    let mut input=Cursor::new(body);let id=input.uint()?;
    let (name,target,_)=packet_info(State::Play,direction,source,id)?;
    let from=if source==768{crate::items::adjacent_slots::Layout::V768}else{crate::items::adjacent_slots::Layout::V769};
    let to=if source==768{crate::items::adjacent_slots::Layout::V769}else{crate::items::adjacent_slots::Layout::V768};
    let payload=match (direction,name) {
        (Direction::Clientbound,"window_items")=>crate::items::adjacent_slots::window_items(from,to,input.rest)?,
        (Direction::Clientbound,"set_slot")=>crate::items::adjacent_slots::set_slot(from,to,input.rest)?,
        (Direction::Clientbound,"position")=>crate::player_position::Position768::decode(input.rest)?.encode()?,
        (Direction::Clientbound,"player_info")=>if source==768{crate::player_info::from_768(input.rest)?}else{crate::player_info::from_769(input.rest)?},
        (_,"keep_alive")=>{let b=input.take(8)?.to_vec();finish(&input)?;b},
        (Direction::Serverbound,"pong")|(Direction::Clientbound,"ping")=>{let b=input.take(4)?.to_vec();finish(&input)?;b},
        (Direction::Clientbound,"chunk_batch_start")=>{finish(&input)?;Vec::new()},
        (Direction::Clientbound,"chunk_batch_finished")=>{let n=input.count(1024)?;finish(&input)?;let mut b=Vec::new();uint(&mut b,n as u32);b},
        (Direction::Serverbound,"teleport_confirm")=>{let n=input.uint()?;finish(&input)?;let mut b=Vec::new();uint(&mut b,n);b},
        (Direction::Serverbound,"chunk_batch_received")=>{
            let b=input.take(4)?;finish(&input)?;let value=f32::from_be_bytes(b.try_into().map_err(|_|Error::Invalid)?);
            if !value.is_finite()||value<=0.0{return Err(Error::Invalid);}b.to_vec()
        },
        (Direction::Serverbound,"position"|"position_look"|"look"|"flying")=>{
            let mut out=Vec::new();
            if matches!(name,"position"|"position_look"){for _ in 0..3{let b=input.take(8)?;if !f64::from_be_bytes(b.try_into().map_err(|_|Error::Invalid)?).is_finite(){return Err(Error::Invalid);}out.extend(b);}}
            if matches!(name,"look"|"position_look"){for _ in 0..2{let b=input.take(4)?;if !f32::from_be_bytes(b.try_into().map_err(|_|Error::Invalid)?).is_finite(){return Err(Error::Invalid);}out.extend(b);}}
            let flags=input.take(1)?[0];if flags&!3!=0{return Err(Error::Invalid);}out.push(flags);finish(&input)?;out
        },
        _=>return Err(Error::PayloadUnavailable(name)),
    };
    let mut out=Vec::new();uint(&mut out,target);out.extend(payload);Ok(out)
}
fn finish(input:&Cursor<'_>)->Result<(),Error>{if input.remaining()==0{Ok(())}else{Err(Error::Invalid)}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_version_never_admits_by_enum_or_feature_alone(){
        assert_eq!(VERSION.protocol(),768);assert_ne!(VERSION,JavaVersion::V1_21_4);
        assert!(crate::Negotiated::native_only(768,JavaVersion::V26_3).is_err());
        for (flag,local,primary) in [(false,true,JavaVersion::V26_3),(true,false,JavaVersion::V26_3),(true,true,JavaVersion::V26_1_2)]{assert!(!private_admission(768,flag,local,primary));}
        assert!(private_admission(768,true,true,JavaVersion::V26_3));
    }
    #[test]
    fn ordinary_reference_has_twelve_registries_and_actual323_nbt_values(){
        let regs=reference_registries().unwrap();assert_eq!(regs.len(),12);assert_eq!(regs.iter().map(|r|r.entries.len()).sum::<usize>(),323);
        let context=RegistryContext::new(&regs,&regs).unwrap();
        for reg in regs{for (id,e) in reg.entries.iter().enumerate(){assert!(e.data.is_some());assert_eq!(context.map(&reg.name,id as u32).unwrap(),id as u32);}assert!(context.map(&reg.name,u32::MAX).is_err());assert_eq!(reg.encode().unwrap()[0],7);}
    }
    #[test]
    fn missing_static_targets_are_errors_not_air(){
        for kind in [RegistryKind::Item,RegistryKind::Block,RegistryKind::BlockState,RegistryKind::Entity,RegistryKind::Component,RegistryKind::Particle]{assert!(map_id(kind,768,u32::MAX).is_err());}
        assert_eq!(map_id(RegistryKind::Item,768,1).unwrap(),1);
        assert_eq!(map_id(RegistryKind::Item,769,1).unwrap(),1);
        let count=(0..1385).filter(|&id|map_id(RegistryKind::Item,769,id).is_err()).count();assert_eq!(count,10);
    }
    #[test]
    fn login_profile_reuses_semantics_but_serializes_768_without_session_alias(){
        let value=crate::login::Success{uuid:[7;16],username:"Private768".into(),properties:Vec::new(),session_id:None};
        let bytes=value.encode(VERSION).unwrap();assert_eq!(crate::login::Success::decode(VERSION,&bytes).unwrap(),value);
        let mut invalid=value;invalid.session_id=Some([8;16]);assert!(invalid.encode(VERSION).is_err());
    }
}
