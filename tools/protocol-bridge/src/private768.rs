//! Private768: отдельная версия, typed соседнее ребро и reference registry context.
//! Pinned protocol.json «packet_position» / «Slot». Не alias payload769.
//! Незавершённые формы возвращают отказ; public admission не изменяется.
use std::collections::{BTreeMap,BTreeSet};
use crate::{State,Direction,JavaVersion,ids::{RegistryKind,IdTranslationError},wire::{self,Cursor,uint,Error}};
#[path="generated_private768.rs"]
mod tables;
#[path="private768_play.rs"]
mod play;
#[path="private768_tags.rs"]
pub mod required_tags;

pub const VERSION:JavaVersion=JavaVersion::V1_21_3;
const MAX_BYTES:usize=8*1024*1024;

/// Compile feature не равен допуску: нужен отдельный opt-in, loopback и primary777.
pub fn private_admission(protocol:i32,flag:bool,loopback:bool,primary:JavaVersion)->bool {
    protocol==768 && flag && loopback && primary==JavaVersion::V26_3
}

/// Точная статическая цель по именам/полным block-state properties, не числам.
pub fn validate_id(kind:RegistryKind,source:u16,id:u32)->Result<(),Error>{
    let (count,_)=tables::ranges(kind,source).ok_or(Error::Invalid)?;
    if id>=count{return Err(Error::Id(IdTranslationError::UnknownId(id)));}Ok(())
}
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
            let Some(target)=targets.get(registry.name.as_str()) else {
                if maps.insert(registry.name.clone(),vec![None;registry.entries.len()]).is_some(){return Err(Error::Invalid);}
                continue;
            };
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
        (Direction::Serverbound,"settings")=>{let raw=input.rest.to_vec();settings(&mut input)?;raw},
        (Direction::Serverbound,"player_input")=>{let flags=input.take(1)?[0];if flags&128!=0{return Err(Error::Invalid);}finish(&input)?;vec![flags]},
        (Direction::Serverbound,"held_item_slot")=>{let bytes=input.take(2)?;let slot=i16::from_be_bytes(bytes.try_into().map_err(|_|Error::Invalid)?);if !(0..=8).contains(&slot){return Err(Error::Invalid);}finish(&input)?;bytes.to_vec()},
        (Direction::Clientbound,"held_item_slot")=>{let slot=if source==769{input.count(8)?}else{usize::from(input.take(1)?[0])};if slot>8{return Err(Error::Invalid);}finish(&input)?;vec![slot as u8]},
        (Direction::Clientbound,"set_cursor_item"|"set_player_inventory")=>{
            let mut out=Vec::new();if name=="set_player_inventory"{uint(&mut out,input.count(45)? as u32);}
            let stack=if source==768 {
                if input.boolean()?{let (stack,n)=crate::items::adjacent_slots::decode(from,input.rest).map_err(Error::Item)?;input.take(n)?;Some(stack.ok_or(Error::Invalid)?)}else{None}
            }else{let(stack,n)=crate::items::adjacent_slots::decode(from,input.rest).map_err(Error::Item)?;input.take(n)?;stack};finish(&input)?;
            if source==769{out.push(u8::from(stack.is_some()));if let Some(stack)=stack{out.extend(crate::items::adjacent_slots::encode(to,Some(&stack)).map_err(Error::Item)?);}}
            else{out.extend(crate::items::adjacent_slots::encode(to,stack.as_ref()).map_err(Error::Item)?);}out
        },
        (_,"keep_alive")=>{let b=input.take(8)?.to_vec();finish(&input)?;b},
        (Direction::Serverbound,"pong")|(Direction::Clientbound,"ping")=>{let b=input.take(4)?.to_vec();finish(&input)?;b},
        (Direction::Serverbound,"tick_end")|(Direction::Clientbound,"chunk_batch_start")=>{finish(&input)?;Vec::new()},
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
    fn packet(state:State,direction:Direction,source:u16,name:&str,payload:&[u8])->Vec<u8>{
        let id=tables::packets(state,direction,source).unwrap().iter().position(|r|r.0==name).unwrap();
        let mut body=Vec::new();uint(&mut body,id as u32);body.extend(payload);body
    }
    #[test]
    fn option_slot_and_held_slot_are_not_769_byte_aliases(){
        let body=packet(State::Play,Direction::Clientbound,769,"set_cursor_item",&[5,1,0,0]);
        let converted=play_edge(Direction::Clientbound,769,&body).unwrap();
        let mut reader=Cursor::new(&converted);reader.uint().unwrap();assert_eq!(reader.rest,[1,5,1,0,0]);
        assert_eq!(play_edge(Direction::Clientbound,768,&converted).unwrap(),body);
        let bad=packet(State::Play,Direction::Clientbound,768,"set_cursor_item",&[1,0]);
        assert!(play_edge(Direction::Clientbound,768,&bad).is_err());
        let held=packet(State::Play,Direction::Clientbound,769,"held_item_slot",&[9]);assert!(play_edge(Direction::Clientbound,769,&held).is_err());
    }
    #[test]
    fn phase_ack_and_reference_dimensions_are_explicit(){
        let regs=reference_registries().unwrap();let mut session=Session::new(&regs).unwrap();
        assert!(session.login_ack(&[]).is_err());session.login_success().unwrap();assert!(session.login_ack(&[0]).is_err());session.login_ack(&[]).unwrap();
        let ack=packet(State::Configuration,Direction::Serverbound,768,"finish_configuration",&[]);
        assert!(session.configuration_serverbound(&ack).is_err());
        let dimensions=play::dimension_context(&regs).unwrap();
        let dimension=regs.iter().find(|r|r.name=="minecraft:dimension_type").unwrap();
        let index=dimension.entries.iter().position(|e|e.name=="minecraft:overworld").unwrap();
        assert_eq!(dimensions[index],(-64,24));
        let malformed=packet(State::Configuration,Direction::Serverbound,768,"settings",&[0]);
        assert!(session.configuration_serverbound(&malformed).is_err());
        assert!(session.skin_parts.is_none());
    }

    #[test]
    fn serverbound_requires_phase_and_unknown_or_duplicate_ack_is_atomic(){
        let regs=reference_registries().unwrap();let mut session=Session::new(&regs).unwrap();
        let packet=packet(State::Play,Direction::Serverbound,768,"teleport_confirm",&[7]);
        assert!(session.serverbound(&packet,Default::default()).is_err());
        session.phase=Phase::Play;session.pending_teleports.insert(8);
        assert!(session.serverbound(&packet,Default::default()).is_err());assert!(session.pending_teleports.contains(&8));
        session.pending_teleports.insert(7);assert_eq!(session.serverbound(&packet,Default::default()).unwrap().teleport.unwrap().id,7);
        assert!(session.serverbound(&packet,Default::default()).is_err());assert!(session.pending_teleports.contains(&8));
    }
    #[test]
    fn configuration_tags_use_static_and_dynamic_domains_with_no_zero_fallback(){
        let regs=reference_registries().unwrap();let mut session=Session::new(&regs).unwrap();session.phase=Phase::Configuration;
        assert_eq!(session.tag_id("minecraft:item",836).unwrap(),827);
        assert_eq!(session.tag_id("minecraft:block",1).unwrap(),1);
        assert_eq!(session.tag_id("minecraft:entity_type",0).unwrap(),0);
        assert!(session.tag_id("minecraft:item",u32::MAX).is_err());
        assert!(session.tag_id("minecraft:unknown",0).is_err());
        let canonical=crate::ids::translate(RegistryKind::Item,JavaVersion::V1_21_4,wire::INTERNAL,836).unwrap();
        let mut body=vec![13,1];put_string(&mut body,"minecraft:item");uint(&mut body,1);put_string(&mut body,"minecraft:example");uint(&mut body,1);uint(&mut body,canonical);
        let converted=session.configuration_clientbound(&body).unwrap();let mut reader=Cursor::new(&converted);
        assert_eq!(reader.uint().unwrap(),13);assert_eq!(reader.uint().unwrap(),1);assert_eq!(reader.string().unwrap(),"minecraft:item");
        assert_eq!(reader.uint().unwrap(),1);assert_eq!(reader.string().unwrap(),"minecraft:example");assert_eq!(reader.uint().unwrap(),1);assert_eq!(reader.uint().unwrap(),827);assert_eq!(reader.remaining(),0);
    }

}

pub struct Inbound {pub body:Vec<u8>,pub teleport:Option<crate::teleport::Confirmation>,pub chunk_ack:bool}

#[derive(Clone,Copy,PartialEq)]
enum Phase { Login,Configuration,Play }

/// Контекст одного соединения. После split обе половины транспорта разделяют
/// это состояние; неподдержанный CB не фиксирует частичный tracker/inventory.
#[derive(Clone)]
pub struct Session {
    phase:Phase,
    login_success:bool,
    offered:bool,
    replied:bool,
    finished:bool,
    registry_sent:BTreeSet<String>,
    registries:std::sync::Arc<Vec<crate::registry::Registry>>,
    mapping:std::sync::Arc<RegistryContext>,
    self_entity:Option<u32>,
    dimension:Option<u32>,
    dimensions:std::sync::Arc<Vec<(i32,usize)>>,
    skin_parts:Option<u8>,
    pending_teleports:BTreeSet<i32>,
    pending_chunk_batches:u32,
    chunk_write_failed:bool,
}
impl Session {
    pub fn new(source:&[crate::registry::Registry])->Result<Self,Error>{
        let registries=reference_registries()?;
        let mapping=RegistryContext::new(source,&registries)?;
        let dimensions=play::dimension_context(&registries)?;
        Ok(Self{phase:Phase::Login,login_success:false,offered:false,replied:false,finished:false,
            registry_sent:Default::default(),registries:std::sync::Arc::new(registries),mapping:std::sync::Arc::new(mapping),self_entity:None,dimension:None,dimensions:std::sync::Arc::new(dimensions),skin_parts:None,pending_teleports:Default::default(),pending_chunk_batches:0,chunk_write_failed:false})
    }
    pub fn login_success(&mut self)->Result<(),Error>{
        if self.phase!=Phase::Login||self.login_success{return Err(Error::Invalid);}self.login_success=true;Ok(())
    }
    pub fn login_ack(&mut self,payload:&[u8])->Result<(),Error>{
        if self.phase!=Phase::Login||!self.login_success||!payload.is_empty(){return Err(Error::Invalid);}
        self.phase=Phase::Configuration;Ok(())
    }
    pub fn registry_sent(&mut self,registry:&crate::registry::Registry)->Result<(),Error>{
        if self.phase!=Phase::Configuration||!self.replied||registry.version!=VERSION||!self.registries.iter().any(|r|r==registry)
            ||!self.registry_sent.insert(registry.name.clone()){return Err(Error::Invalid);}Ok(())
    }
    pub fn configuration_serverbound(&mut self,body:&[u8])->Result<(),Error>{
        if self.phase!=Phase::Configuration{return Err(Error::Invalid);}
        let mut input=Cursor::new(body);let id=input.uint()?;
        let (name,_,_)=packet_info(State::Configuration,Direction::Serverbound,768,id)?;
        match name {
            "settings"=>{self.skin_parts=Some(settings(&mut input)?);},
            "custom_payload"=>{brand(&mut input)?;},
            "select_known_packs"=>{
                if !self.offered||self.replied{return Err(Error::Invalid);}
                let packs=packs(&mut input)?;
                if !packs.is_empty()&&packs!=[("minecraft".into(),"core".into(),"1.21.3".into())]{return Err(Error::Invalid);}
                self.replied=true;
            },
            "finish_configuration"=>{
                finish(&input)?;if !self.finished{return Err(Error::Invalid);}self.phase=Phase::Play;
            },
            "keep_alive"=>{input.take(8)?;finish(&input)?;},
            "pong"=>{input.take(4)?;finish(&input)?;},
            _=>return Err(Error::PayloadUnavailable(name)),
        }Ok(())
    }
    pub fn configuration_clientbound(&mut self,canonical:&[u8])->Result<Vec<u8>,Error>{
        if self.phase!=Phase::Configuration{return Err(Error::Invalid);}
        let intermediate=crate::client_769::configuration(canonical)?;
        let mut input=Cursor::new(&intermediate);let id=input.uint()?;
        let (name,target,_)=packet_info(State::Configuration,Direction::Clientbound,769,id)?;
        let mut out=Vec::new();uint(&mut out,target);
        match name {
            "custom_payload"=>{brand_copy(&mut input,&mut out)?;},
            "feature_flags"=>{let n=input.count(1024)?;uint(&mut out,n as u32);for _ in 0..n{copy_string(&mut input,&mut out)?;}finish(&input)?;},
            "select_known_packs"=>{
                if self.offered{return Err(Error::Invalid);}
                if packs(&mut input)?!=[("minecraft".into(),"core".into(),"1.21.4".into())]{return Err(Error::Invalid);}
                uint(&mut out,1);for s in ["minecraft","core","1.21.3"]{put_string(&mut out,s);}self.offered=true;
            },
            "tags"=>{
                let n=input.count(1024)?;uint(&mut out,n as u32);
                for _ in 0..n {
                    let registry=input.string()?;put_string(&mut out,registry);let n=input.count(65536)?;uint(&mut out,n as u32);
                    for _ in 0..n{copy_string(&mut input,&mut out)?;let n=input.count(65536)?;uint(&mut out,n as u32);
                        for _ in 0..n{uint(&mut out,self.tag_id(registry,input.uint()?)?);}}
                }finish(&input)?;
            },
            "finish_configuration"=>{
                finish(&input)?;if self.registry_sent.len()!=12||!self.replied||self.finished{return Err(Error::Invalid);}self.finished=true;
            },
            _=>return Err(Error::PayloadUnavailable(name)),
        }Ok(out)
    }
    pub fn ensure_play(&self)->Result<(),Error>{if self.phase==Phase::Play{Ok(())}else{Err(Error::Invalid)}}
    pub fn serverbound(&mut self,body:&[u8],context:wire::Context)->Result<Inbound,Error>{
        self.ensure_play()?;let mut candidate=self.clone();
        let mut input=Cursor::new(body);let id=input.uint()?;
        let (name,_,_)=packet_info(State::Play,Direction::Serverbound,768,id)?;
        let translated=if name=="chunk_batch_received" {
            let value=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);finish(&input)?;
            if !value.is_finite()||value<=0.0||candidate.chunk_write_failed||candidate.pending_chunk_batches==0{return Err(Error::Invalid);}
            candidate.pending_chunk_batches-=1;
            // Это проверенный control ACK: он не является игровым действием775.
            Inbound{body:Vec::new(),teleport:None,chunk_ack:true}
        }else if name=="teleport_confirm" {
            let confirmation=crate::teleport::Confirmation::decode(VERSION,input.rest).map_err(|_|Error::Invalid)?;
            if !candidate.pending_teleports.remove(&confirmation.id){return Err(Error::PayloadUnavailable("private768 unknown/duplicate teleport ACK"));}
            let mut body=vec![0];uint(&mut body,confirmation.id as u32);Inbound{body,teleport:Some(confirmation),chunk_ack:false}
        }else{Inbound{body:serverbound(body,context)?,teleport:None,chunk_ack:false}};
        *self=candidate;Ok(translated)
    }
    fn tag_id(&self,registry:&str,id:u32)->Result<u32,Error>{
        match registry {
            "minecraft:block"=>map_id(RegistryKind::Block,769,id),
            "minecraft:item"=>map_id(RegistryKind::Item,769,id),
            "minecraft:entity_type"=>map_id(RegistryKind::Entity,769,id),
            _=>self.mapping.map(registry,id),
        }
    }
    /// Одна настоящая пачка содержит один полностью проверенный чанк.
    /// До read-loop ACK не ждём; это учёт контекста, не full backpressure.
    pub fn chunk_batch(&mut self,tracker:&mut crate::entities::Tracker,body:&[u8],context:wire::Context)->Result<[Vec<u8>;3],Error>{
        self.ensure_play()?;
        if self.chunk_write_failed||self.pending_chunk_batches>=65_536{return Err(Error::TooLarge);}
        let mut header=Cursor::new(body);if header.uint()?!=0x2d{return Err(Error::Invalid);}
        let mut next=self.clone();let mut entities=tracker.clone();
        let chunk=next.clientbound_inner(&mut entities,body,context)?;
        // Закреплённое ребро769→768 проверяет имена, ID и payload каждого маркера.
        let start=play_edge(Direction::Clientbound,769,&[13])?;
        let finished=play_edge(Direction::Clientbound,769,&[12,1])?;
        next.pending_chunk_batches+=1;
        *self=next;*tracker=entities;
        Ok([start,chunk,finished])
    }
    pub fn chunk_write_failed(&mut self){self.chunk_write_failed=true;}
    pub fn clientbound(&mut self,tracker:&mut crate::entities::Tracker,body:&[u8],context:wire::Context)->Result<Vec<u8>,Error>{
        let mut header=Cursor::new(body);
        if matches!(header.uint()?,0x0b|0x0c|0x2d){return Err(Error::PayloadUnavailable("private768 chunk requires atomic batch producer"));}
        self.clientbound_inner(tracker,body,context)
    }
    fn clientbound_inner(&mut self,tracker:&mut crate::entities::Tracker,body:&[u8],context:wire::Context)->Result<Vec<u8>,Error>{
        if self.phase!=Phase::Play{return Err(Error::Invalid);}
        let mut candidate=tracker.clone();
        let intermediate=crate::client_769::clientbound(&mut candidate,body,context)?;
        let mut next=self.clone();
        let output=next.encode_768(&intermediate,context)?;
        *self=next;*tracker=candidate;Ok(output)
    }
}

fn serverbound(body:&[u8],context:wire::Context)->Result<Vec<u8>,Error>{
    let intermediate=play_edge(Direction::Serverbound,768,body)?;
    Ok(crate::client_769::serverbound(&intermediate,context)?.into_owned())
}
fn put_string(out:&mut Vec<u8>,value:&str){uint(out,value.len() as u32);out.extend(value.as_bytes());}
fn copy_string(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{put_string(out,input.string()?);Ok(())}
fn packs(input:&mut Cursor<'_>)->Result<Vec<(String,String,String)>,Error>{
    let n=input.count(16)?;let mut values=Vec::new();
    for _ in 0..n{values.push((input.string()?.into(),input.string()?.into(),input.string()?.into()));}finish(input)?;Ok(values)
}
fn brand(input:&mut Cursor<'_>)->Result<(),Error>{
    if input.string()?!="minecraft:brand"{return Err(Error::PayloadUnavailable("private768 custom channel"));}input.string()?;finish(input)
}
fn brand_copy(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    if input.string()?!="minecraft:brand"{return Err(Error::PayloadUnavailable("private768 custom channel"));}
    put_string(out,"minecraft:brand");copy_string(input,out)?;finish(input)
}
fn settings(input:&mut Cursor<'_>)->Result<u8,Error>{
    input.string()?;let distance=input.take(1)?[0];if !(2..=32).contains(&distance){return Err(Error::Invalid);}
    input.count(2)?;input.boolean()?;let skin=input.take(1)?[0];if skin&!127!=0{return Err(Error::Invalid);}
    input.count(1)?;input.boolean()?;input.boolean()?;input.count(2)?;finish(input)?;Ok(skin)
}

#[cfg(test)]
mod chunk_batch_tests {
    use super::*;
    fn fixture()->(Session,crate::entities::Tracker,Vec<u8>,wire::Context){
        let regs=reference_registries().unwrap();let mut session=Session::new(&regs).unwrap();session.phase=Phase::Play;
        let dimensions=regs.iter().find(|r|r.name=="minecraft:dimension_type").unwrap();
        let dimension=dimensions.entries.iter().position(|e|e.name=="minecraft:overworld").unwrap();
        session.dimension=Some(dimension as u32);
        let biomes=regs.iter().find(|r|r.name=="minecraft:worldgen/biome").unwrap().entries.len() as u32;
        let context=wire::Context{chunk:Some(crate::chunk::Context{min_y:-64,sections:24,shared_biome_count:biomes}),..Default::default()};
        let sections=vec![crate::chunk_sections::Section{blocks_present:0,fluids_present:0,blocks:vec![0;4096],biomes:vec![0;64]};24];
        let bytes=crate::chunk_sections::encode(wire::INTERNAL,&sections,biomes).unwrap();
        let mut body=vec![0x2d];body.extend([0;8]);body.push(0); // Чанк0,0, без heightmaps.
        uint(&mut body,bytes.len() as u32);body.extend(bytes);body.push(0); // Без block entities.
        let light=crate::light::Light{sky:vec![],block:vec![],empty_sky:vec![],empty_block:vec![],sky_arrays:vec![],block_arrays:vec![]};
        body.extend(light.encode(wire::INTERNAL).unwrap());
        (session,Default::default(),body,context)
    }
    fn ack(value:f32)->Vec<u8>{let mut b=vec![9];b.extend(value.to_be_bytes());b}
    #[test]
    fn genuine_one_chunk_has_typed_markers_and_one_contextual_ack(){
        let(mut session,mut tracker,body,context)=fixture();
        let rows=session.chunk_batch(&mut tracker,&body,context).unwrap();
        assert_eq!(rows[0],[13]);assert_eq!(rows[2],[12,1]);
        assert_eq!(packet_info(State::Play,Direction::Clientbound,768,u32::from(rows[1][0])).unwrap().0,"map_chunk");
        assert_eq!(session.pending_chunk_batches,1);
        let control=session.serverbound(&ack(1.0),Default::default()).unwrap();
        assert!(control.chunk_ack&&control.body.is_empty()&&control.teleport.is_none());
        assert_eq!(session.pending_chunk_batches,0);
        assert!(session.serverbound(&ack(1.0),Default::default()).is_err());
    }
    #[test]
    fn ack_without_batch_nan_infinite_nonpositive_trailing_and_truncated_fail_atomically(){
        let(mut session,_,_,_)=fixture();
        assert!(session.serverbound(&ack(1.0),Default::default()).is_err());
        for v in [0.0,-0.0,-1.0,f32::NAN,f32::INFINITY,f32::NEG_INFINITY] {
            session.pending_chunk_batches=1;
            assert!(session.serverbound(&ack(v),Default::default()).is_err());assert_eq!(session.pending_chunk_batches,1);
        }
        for mut b in [vec![9],vec![9,0,0,0],ack(1.0)]{
            if b.len()==5{b.push(0);}
            assert!(session.serverbound(&b,Default::default()).is_err());assert_eq!(session.pending_chunk_batches,1);
        }
    }
    #[test]
    fn failed_encoder_cannot_publish_partial_batch_or_ack_credit(){
        let(mut session,mut tracker,body,context)=fixture();
        for n in 0..body.len(){assert!(session.chunk_batch(&mut tracker,&body[..n],context).is_err());assert_eq!(session.pending_chunk_batches,0);}
        assert!(session.clientbound(&mut tracker,&body,context).is_err());
        assert!(session.clientbound(&mut tracker,&[0x0c],context).is_err());
        assert!(session.clientbound(&mut tracker,&[0x0b,1],context).is_err());
        assert_eq!(session.pending_chunk_batches,0);
    }
    #[test]
    fn failed_transport_is_terminal_for_chunk_and_ack_context(){
        let(mut session,mut tracker,body,context)=fixture();
        session.chunk_batch(&mut tracker,&body,context).unwrap();session.chunk_write_failed();
        assert!(session.serverbound(&ack(1.0),Default::default()).is_err());assert_eq!(session.pending_chunk_batches,1);
        assert!(session.chunk_batch(&mut tracker,&body,context).is_err());assert_eq!(session.pending_chunk_batches,1);
    }
    #[test]
    fn pending_cap_and_new_session_have_distinct_ack_context(){
        let(mut session,mut tracker,body,context)=fixture();session.pending_chunk_batches=65_536;
        assert!(session.chunk_batch(&mut tracker,&body,context).is_err());assert_eq!(session.pending_chunk_batches,65_536);
        let(mut fresh,_,_,_)=fixture();assert!(fresh.serverbound(&ack(1.0),Default::default()).is_err());
    }
}
