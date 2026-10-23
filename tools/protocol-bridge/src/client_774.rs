//! Соседний typed-переход774↔775; публичный допуск задаётся отдельно.
use std::borrow::Cow;
use crate::{Direction, JavaVersion, State, entities::Tracker, wire::{self, Context, Error, Cursor, uint}};

pub fn configuration(body: &[u8]) -> Result<Cow<'_, [u8]>, Error> {
    wire::translate_inner(State::Configuration, Direction::Clientbound, wire::INTERNAL, JavaVersion::V1_21_11, body, Context::default(), true)
}
pub fn clientbound<'a>(tracker: &mut Tracker, body: &'a [u8], context: Context) -> Result<Cow<'a, [u8]>, Error> {
    tracker.translate_inner(wire::INTERNAL, JavaVersion::V1_21_11, body, context, true)
}
pub fn serverbound(body: &[u8], context: Context) -> Result<Cow<'_, [u8]>, Error> {
    wire::translate_inner(State::Play, Direction::Serverbound, JavaVersion::V1_21_11, wire::INTERNAL, body, context, true)
}

pub(crate) fn interact(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    let entity=input.uint()?; let action=input.count(2)?;
    let mut at=[0.0f64;3];
    if action==2 {for v in &mut at { *v=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?) as f64; if !v.is_finite() || v.abs()>64.0{return Err(Error::Invalid);} }}
    let hand=if action==1 {0} else {input.count(1)? as u32};
    let sneaking=input.boolean()?;
    out.clear(); uint(out,if action==1{1}else{0x1a});uint(out,entity);
    if action!=1 {
        uint(out,hand);
        let largest=at.into_iter().map(f64::abs).fold(0.0,f64::max);
        if largest<1.0/32766.0 {out.push(0);}else{
            let scale=largest.ceil() as u32;let mut packed=u64::from(scale&3)|if scale>3{4}else{0};
            for (i,v) in at.into_iter().enumerate(){packed|=((((v/f64::from(scale)*0.5+0.5)*32766.0).round() as u64)&0x7fff)<<(3+15*i);}
            out.push(packed as u8);out.push((packed>>8) as u8);out.extend(((packed>>16) as u32).to_be_bytes());if scale>3{uint(out,scale>>2);}
        }
        out.push(u8::from(sneaking));
    }
    Ok(())
}

pub(crate) fn time(input: &mut Cursor<'_>, out: &mut Vec<u8>, previous: Option<(i64,i64,bool)>) -> Result<(), Error> {
    let age=i64::from_be_bytes(input.take(8)?.try_into().map_err(|_|Error::Invalid)?);
    let n=input.count(1)?;let (mut time,mut running)=previous.map_or((age,true),|(last,t,run)|(t.saturating_add(if run{age.saturating_sub(last)}else{0}),run));
    if n==1 {
        if input.uint()?!=0{return Err(Error::Invalid);}
        let mut value=0u64;let mut done=false;
        for i in 0..10 {let b=input.take(1)?[0];if i==9 && b&254!=0{return Err(Error::Invalid);}value|=u64::from(b&127)<<(7*i);if b&128==0{done=true;break;}}
        if !done{return Err(Error::Invalid);}time=value as i64;
        let partial=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
        let rate=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
        if !partial.is_finite() || !matches!(rate,0.0|1.0){return Err(Error::PayloadUnavailable("774 clock rate"));}running=rate!=0.0;
    }
    out.extend(age.to_be_bytes());out.extend(time.to_be_bytes());out.push(u8::from(running));Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{items, ids::{self,RegistryKind}};
    #[test]
    fn interaction_preserves_attack_hand_hit_and_rejects_truncation_and_tail() {
        for body in [vec![0x19,42,1,0],vec![0x19,42,0,1,1],vec![0x19,42,2,0x3f,0,0,0,0,0,0,0,0,0,0,0,1,0]] {
            let mapped=serverbound(&body,Context::default()).unwrap();
            assert_eq!(&mapped[..2],if body[2]==1{&[1,42]}else{&[0x1a,42]});
            for end in 0..body.len(){assert!(serverbound(&body[..end],Context::default()).is_err());}
            let mut extra=body;extra.push(0);assert!(serverbound(&extra,Context::default()).is_err());
        }
    }
    #[test]
    fn relative_moves_keep_angles_before_ground_for_774() {
        for name in ["rel_entity_move","entity_move_look"] {
            let (id,info)=(0..256).find_map(|id|crate::packet_layouts::packet_info(RegistryKind::PlayClientbound,wire::INTERNAL,JavaVersion::V1_21_11,id).filter(|r|r.name==name).map(|r|(id,r))).unwrap();
            let mut body=vec![id as u8,42];for delta in [-100i16,200,-300]{body.extend(delta.to_be_bytes());}
            if name=="entity_move_look"{body.extend([30,240]);}body.push(1);
            let result=wire::translate_inner(State::Play,Direction::Clientbound,wire::INTERNAL,JavaVersion::V1_21_11,&body,Context::default(),true).unwrap();
            assert_eq!(result[0],info.target_id.unwrap() as u8);assert_eq!(&result[1..],&body[1..]);
            for end in 0..body.len(){assert!(wire::translate_inner(State::Play,Direction::Clientbound,wire::INTERNAL,JavaVersion::V1_21_11,&body[..end],Context::default(),true).is_err());}
        }
    }
    #[test]
    fn complex_items_roundtrip_and_774_templates_have_count_before_id() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
        for row in data["old"]["cases"].as_array().unwrap(){
            let raw:Vec<u8>=row["slot_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter().map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
            let (stack,n)=items::decode(wire::INTERNAL,&raw,false,Default::default()).unwrap();assert_eq!(n,raw.len());
            for untrusted in [false,true]{
                let encoded=items::encode(JavaVersion::V1_21_11,stack.as_ref(),untrusted,Default::default()).unwrap();
                assert_eq!(items::decode(JavaVersion::V1_21_11,&encoded,untrusted,Default::default()).unwrap(),(stack.clone(),encoded.len()),"{}",row["name"]);
                for end in 0..encoded.len(){assert!(items::decode(JavaVersion::V1_21_11,&encoded[..end],untrusted,Default::default()).is_err());}
            }
        }
        let stack=items::Stack{item:1,count:7,added:vec![],removed:vec![]};
        let bytes=items::encode_template(JavaVersion::V1_21_11,&stack,Default::default()).unwrap();
        assert_eq!(bytes,[7,ids::translate(RegistryKind::Item,wire::INTERNAL,JavaVersion::V1_21_11,1).unwrap() as u8,0,0]);
        assert_eq!(items::decode_template(JavaVersion::V1_21_11,&bytes,Default::default()).unwrap(),(stack,bytes.len()));
    }
    #[test]
    fn clock_keeps_set_time_across_empty_updates_and_stops_when_frozen() {
        let mut tracker=Tracker::default();
        let mut body=vec![0x71];body.extend(100i64.to_be_bytes());body.extend([1,0,0xa0,0x1f]);body.extend(0f32.to_be_bytes());body.extend(1f32.to_be_bytes());
        let result=clientbound(&mut tracker,&body,Context::default()).unwrap();assert_eq!(&result[9..17],4000i64.to_be_bytes());
        let mut body=vec![0x71];body.extend(120i64.to_be_bytes());body.push(0);
        let result=clientbound(&mut tracker,&body,Context::default()).unwrap();assert_eq!(&result[9..17],4020i64.to_be_bytes());
        for end in 0..body.len(){assert!(clientbound(&mut tracker,&body[..end],Context::default()).is_err());}
        let mut frozen=vec![0x71];frozen.extend(130i64.to_be_bytes());frozen.extend([1,0,0xa0,0x1f]);frozen.extend([0;8]);
        clientbound(&mut tracker,&frozen,Context::default()).unwrap();
        let mut later=vec![0x71];later.extend(150i64.to_be_bytes());later.push(0);
        let result=clientbound(&mut tracker,&later,Context::default()).unwrap();assert_eq!(&result[9..17],4000i64.to_be_bytes());assert_eq!(result[17],0);
    }
}

#[cfg(test)]
mod recipe_tests {
    use super::*;
    #[test]
    fn recipe_displays_use_774_discriminators_and_count_first_result() {
        let source=[0x3f,0,0,1,4,1,5,1,2,0,0,4,2];
        let result=clientbound(&mut Tracker::default(),&source,Context::default()).unwrap();
        assert_eq!(result.as_ref(),[61,0,0,1,2,1,3,2,1,0,0,2,2]);
        for end in 0..source.len(){assert!(clientbound(&mut Tracker::default(),&source[..end],Context::default()).is_err());}
        let set=[0x3f,0,0,1,6,3,1,2,5,1,2,0,0,4,2];
        let result=clientbound(&mut Tracker::default(),&set,Context::default()).unwrap();
        assert_eq!(result.as_ref(),[61,0,0,1,7,2,2,1,2,2,3,2,1,0,0,2,2]);
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::*;
    #[test]
    fn single_774_section_has_one_count_and_bounded_palettes() {
        let section=crate::chunk_sections::Section{blocks_present:4096,fluids_present:0,blocks:vec![1;4096],biomes:vec![0;64]};
        let encoded=crate::chunk_sections::encode(JavaVersion::V1_21_11,std::slice::from_ref(&section),1).unwrap();
        assert_eq!(encoded,[16,0,0,1,0,0]);
        assert_eq!(crate::chunk_sections::decode(JavaVersion::V1_21_11,&encoded,1,1).unwrap(),[section]);
        for end in 0..encoded.len(){assert!(crate::chunk_sections::decode(JavaVersion::V1_21_11,&encoded[..end],1,1).is_err());}
    }
}
