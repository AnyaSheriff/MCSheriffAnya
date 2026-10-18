//! Действия и эффекты, содержащие числовые ссылки на игровые реестры.
use std::collections::BTreeSet;
use crate::{JavaVersion,ids::{self,RegistryKind},wire::{Cursor,Error,Context,uint}};
fn string(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{let value=input.string()?;uint(out,value.len() as u32);out.extend(value.as_bytes());Ok(())}
fn floats(input:&mut Cursor<'_>,out:&mut Vec<u8>,n:usize,double:bool)->Result<(),Error>{
    for _ in 0..n {let value=input.take(if double{8}else{4})?;
        let finite=if double{f64::from_be_bytes(value.try_into().map_err(|_|Error::Invalid)?).is_finite()}else{f32::from_be_bytes(value.try_into().map_err(|_|Error::Invalid)?).is_finite()};
        if !finite{return Err(Error::Invalid);}out.extend(value);
    }Ok(())
}

fn sound(input:&mut Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    let id=input.uint()?;
    if id!=0{return Err(Error::PayloadUnavailable("static sound registry"));}
    uint(out,0);string(input,out)?;let fixed=input.boolean()?;out.push(u8::from(fixed));if fixed{floats(input,out,1,false)?;}Ok(())
}
fn prediction(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion)->Result<(),Error>{
    let present=input.boolean()?;out.push(u8::from(present));if !present{return Ok(());}
    let item=ids::translate(RegistryKind::Item,from,to,input.uint()?)?;if item==0{return Err(Error::Invalid);}uint(out,item);
    let count=input.count(127)?;if count==0{return Err(Error::Invalid);}uint(out,count as u32);
    let mut seen=BTreeSet::new();
    let n=input.count(256)?;uint(out,n as u32);
    for _ in 0..n {
        let kind=ids::translate(RegistryKind::Component,from,to,input.uint()?)?;
        if !seen.insert(kind){return Err(Error::Invalid);}uint(out,kind);
        // Хэш непрозрачен и не доказывает равенства компонентов разных версий.
        // Ядро принудительно пересылает настоящий контейнер после такого щелчка.
        out.extend(input.take(4)?);
    }
    let n=input.count(256)?;uint(out,n as u32);
    for _ in 0..n {let kind=ids::translate(RegistryKind::Component,from,to,input.uint()?)?;if !seen.insert(kind){return Err(Error::Invalid);}uint(out,kind);}
    Ok(())
}
pub(crate) fn click(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion)->Result<(),Error>{
    uint(out,input.uint()?);uint(out,input.uint()?);out.extend(input.take(3)?);uint(out,input.count(6)? as u32);
    let n=input.count(128)?;uint(out,n as u32);let mut seen=BTreeSet::new();
    for _ in 0..n {
        let slot=input.take(2)?;let number=i16::from_be_bytes(slot.try_into().map_err(|_|Error::Invalid)?);
        if number<0 || !seen.insert(number){return Err(Error::Invalid);}out.extend(slot);prediction(input,out,from,to)?;
    }
    prediction(input,out,from,to)
}
pub(crate) fn effects(name:&str,input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context)->Result<(),Error>{
    match name {
        "entity_effect"|"remove_entity_effect"=>{
            uint(out,input.uint()?);
            // Обычные /effect give на775/777; записи effect-observations.json.
            // Новые неизвестные эффекты не подменяем эффектом с похожим номером.
            uint(out,input.count(39)? as u32);
            if name=="entity_effect" {
                uint(out,input.count(255)? as u32);
                let duration=input.varint()?;if duration < -1{return Err(Error::Invalid);}uint(out,duration as u32);
                let flags=input.take(1)?[0];if flags&!15!=0{return Err(Error::Invalid);}out.push(flags);
            }
        }
        "damage_event"=>{
            uint(out,input.uint()?);let kind=input.uint()?;
            let count=context.metadata_registry_counts.iter().find(|(name,_)|*name=="minecraft:damage_type")
                .ok_or_else(||Error::RegistryUnavailable("minecraft:damage_type".into()))?.1;
            if kind>=count{return Err(Error::Invalid);}uint(out,kind);
            uint(out,input.uint()?);uint(out,input.uint()?);
            let position=input.boolean()?;out.push(u8::from(position));if position{floats(input,out,3,true)?;}
        }
        "sound_effect"|"entity_sound_effect"=>{
            sound(input,out)?;uint(out,input.count(9)? as u32);
            if name=="sound_effect"{out.extend(input.take(12)?);}else{uint(out,input.uint()?);}
            floats(input,out,2,false)?;out.extend(input.take(8)?);
        }
        "explosion"=>{
            floats(input,out,3,true)?;floats(input,out,1,false)?;
            let count=input.take(4)?;if i32::from_be_bytes(count.try_into().map_err(|_|Error::Invalid)?)<0{return Err(Error::Invalid);}out.extend(count);
            let knockback=input.boolean()?;out.push(u8::from(knockback));if knockback{floats(input,out,3,true)?;}
            crate::particles::particle(input,out,from,to,context.items)?;sound(input,out)?;
            let n=input.count(4096)?;uint(out,n as u32);
            for _ in 0..n{crate::particles::particle(input,out,from,to,context.items)?;floats(input,out,2,false)?;uint(out,input.uint()?);}
        }
        "world_event"=>{
            let effect=input.take(4)?;let kind=i32::from_be_bytes(effect.try_into().map_err(|_|Error::Invalid)?);out.extend(effect);out.extend(input.take(8)?);
            let data=i32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
            let data=match kind {
                // Wiki Java Edition protocol/Packets, Level Event:
                // 2001/3008: «Block state ID» (слом и завершение археологии).
                2001|3008=>ids::translate(RegistryKind::BlockState,from,to,u32::try_from(data).map_err(|_|Error::Invalid)?)? as i32,
                // Цвета, направления и число частиц, а не ID содержимого.
                1500..=1505|2000|2002..=2004|2006..=2013|3000..=3007|3009=>data,
                _=>return Err(Error::PayloadUnavailable("unverified level event data")),
            };out.extend(data.to_be_bytes());out.push(u8::from(input.boolean()?));
        }
        _=>return Err(Error::Invalid),
    }Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
    #[test]
    fn nineteen_official_block_menus_match_in_both_directions() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/menu-observations.json")).unwrap();
        let bytes=|hex:&str|hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
        let a=data["old"]["menus"].as_array().unwrap();let b=data["new"]["menus"].as_array().unwrap();assert_eq!(a.len(),19);assert_eq!(b.len(),19);
        for (old,new) in a.iter().zip(b) {
            assert_eq!(old["block"],new["block"]);
            let old=bytes(old["body"].as_str().unwrap());let new=bytes(new["body"].as_str().unwrap());
            for (from,to,a,b) in [(OLD,NEW,&old,&new),(NEW,OLD,&new,&old)] {
                assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,from,to,a).unwrap().as_ref(),b);
                for n in 0..a.len(){assert!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,from,to,&a[..n]).is_err());}
            }
        }
    }
    #[test]
    fn all_forty_official_effects_translate_exactly_both_ways() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/effect-observations.json")).unwrap();
        let bytes=|hex:&str|hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
        let old=data["old"]["effects"].as_array().unwrap();let new=data["new"]["effects"].as_array().unwrap();
        assert_eq!(old.len(),40);assert_eq!(new.len(),40);
        for (a,b) in old.iter().zip(new) {
            assert_eq!(a["name"],b["name"]);assert_eq!(a["observed_ids"],b["observed_ids"]);
            let a=bytes(a["packets"][0]["body"].as_str().unwrap());let b=bytes(b["packets"][0]["body"].as_str().unwrap());
            assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,OLD,NEW,&a).unwrap().as_ref(),b);
            assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,NEW,OLD,&b).unwrap().as_ref(),a);
        }
    }
    #[test]
    fn effects_reject_unknown_ids_and_preserve_infinite_duration() {
        let body=[1,0,1,255,255,255,255,15,7];let mut result=Vec::new();
        effects("entity_effect",&mut Cursor::new(&body),&mut result,OLD,NEW,Context::default()).unwrap();assert_eq!(result,body);
        for body in [[1,40,0,1,0],[1,0,0,1,16]] {
            assert!(effects("entity_effect",&mut Cursor::new(&body),&mut Vec::new(),OLD,NEW,Context::default()).is_err());
        }
        let body=[1,0,0,0,0];
        assert!(effects("damage_event",&mut Cursor::new(&body),&mut Vec::new(),OLD,NEW,Context::default()).is_err());
        let context=Context {metadata_registry_counts:&[("minecraft:damage_type",1)],..Default::default()};
        let mut result=Vec::new();effects("damage_event",&mut Cursor::new(&body),&mut result,OLD,NEW,context).unwrap();assert_eq!(result,body);
        assert!(effects("damage_event",&mut Cursor::new(&[1,1,0,0,0]),&mut Vec::new(),OLD,NEW,context).is_err());
    }
    #[test]
    fn click_maps_items_and_components_but_does_not_trust_hashes() {
        let mut body=vec![0,7,0,36,0,0,1,0,36,1];
        uint(&mut body,100);body.extend([5,1,3]);body.extend(0x12345678i32.to_be_bytes());body.extend([0,0]);
        let mut result=Vec::new();let mut cursor=Cursor::new(&body);
        click(&mut cursor,&mut result,OLD,NEW).unwrap();assert!(cursor.rest.is_empty());
        let mut restored=Vec::new();click(&mut Cursor::new(&result),&mut restored,NEW,OLD).unwrap();assert_eq!(restored,body);
        for n in 0..body.len(){assert!(click(&mut Cursor::new(&body[..n]),&mut Vec::new(),OLD,NEW).is_err());}
        let mut duplicate=body.clone();duplicate[11]=2; // Две добавленные компоненты вместо одной.
        let added=duplicate[12..17].to_vec();duplicate.splice(17..17,added);
        assert!(click(&mut Cursor::new(&duplicate),&mut Vec::new(),OLD,NEW).is_err());
    }
    #[test]
    fn invalid_click_counts_modes_and_slots_fail_closed() {
        let good=[0,0,0,36,0,0,0,0];
        click(&mut Cursor::new(&good),&mut Vec::new(),OLD,NEW).unwrap();
        for (index,value) in [(5,7),(6,129),(7,2)] {
            let mut bad=good;bad[index]=value;
            assert!(click(&mut Cursor::new(&bad),&mut Vec::new(),OLD,NEW).is_err());
        }
        assert!(click(&mut Cursor::new(&[0,0,0,36,0,0,1,255,255,0,0]),&mut Vec::new(),OLD,NEW).is_err());
    }
    #[test]
    fn inline_sound_is_preserved_but_unknown_static_ids_are_rejected() {
        let name=b"minecraft:entity.item.pickup";let mut body=vec![0];uint(&mut body,name.len() as u32);body.extend(name);body.extend([0,0]);
        body.extend([0;12]);body.extend(1f32.to_be_bytes());body.extend(0.5f32.to_be_bytes());body.extend(42u64.to_be_bytes());
        let mut result=Vec::new();let mut cursor=Cursor::new(&body);
        effects("sound_effect",&mut cursor,&mut result,OLD,NEW,Context::default()).unwrap();assert_eq!(result,body);assert!(cursor.rest.is_empty());
        for n in 0..body.len(){assert!(effects("sound_effect",&mut Cursor::new(&body[..n]),&mut Vec::new(),OLD,NEW,Context::default()).is_err());}
        body[0]=1;assert!(effects("sound_effect",&mut Cursor::new(&body),&mut Vec::new(),OLD,NEW,Context::default()).is_err());
    }
    #[test]
    fn block_break_event_maps_state_instead_of_copying_its_number() {
        for event in [2001i32,3008] {
        let mut body=event.to_be_bytes().to_vec();body.extend([0;8]);body.extend(27i32.to_be_bytes());body.push(0);
        let mut result=Vec::new();effects("world_event",&mut Cursor::new(&body),&mut result,OLD,NEW,Context::default()).unwrap();
        assert_eq!(&result[12..16],&28i32.to_be_bytes());
        let mut restored=Vec::new();effects("world_event",&mut Cursor::new(&result),&mut restored,NEW,OLD,Context::default()).unwrap();assert_eq!(restored,body);
        }
    }
}
