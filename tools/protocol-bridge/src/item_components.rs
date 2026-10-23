//! Сложные компоненты: каждый внутренний ID переводится отдельно.
//! Схемы из pinned protocol.json; обычные команды — complex-item-observations.json.
use std::collections::BTreeSet;
use crate::{JavaVersion,ids::{self,RegistryKind},items::{Reader,Error,uint,plain_text}};

fn string(input:&mut Reader<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    let n=input.count(32767)?;let bytes=input.take(n)?;std::str::from_utf8(bytes).map_err(|_|Error::Invalid)?;
    uint(out,n as u32);out.extend(bytes);Ok(())
}
fn skip_string(input:&mut Reader<'_>)->Result<(),Error>{
    let n=input.count(32767)?;std::str::from_utf8(input.take(n)?).map_err(|_|Error::Invalid)?;Ok(())
}
fn finite(input:&mut Reader<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    let bytes=input.take(8)?;let value=f64::from_be_bytes(bytes.try_into().map_err(|_|Error::Invalid)?);
    if !value.is_finite(){return Err(Error::Invalid);}out.extend(bytes);Ok(())
}

pub(crate) fn remap(kind:u32,input:&mut Reader<'_>,from:JavaVersion,to:JavaVersion)->Result<Vec<u8>,Error>{
    let mut out=Vec::new();
    match kind {
        16=>{
            let n=input.count(256)?;uint(&mut out,n as u32);
            for _ in 0..n {
                uint(&mut out,ids::translate(RegistryKind::Attribute,from,to,input.uint()?).map_err(Error::Id)?);
                string(input,&mut out)?;finite(input,&mut out)?;
                uint(&mut out,input.count(2)? as u32);uint(&mut out,input.count(10)? as u32);
                let display=if from==JavaVersion::V1_21_5 {0}else{input.count(2)?};
                if to!=JavaVersion::V1_21_5 {uint(&mut out,display as u32);}
                if display==2 {let text=input.nbt()?;plain_text(text)?;if to!=JavaVersion::V1_21_5 {out.extend(text);}}
            }
        }
        18=>{
            out.push(u8::from(input.boolean()?));let n=input.count(256)?;uint(&mut out,n as u32);let mut seen=BTreeSet::new();
            for _ in 0..n {
                let component=ids::translate(RegistryKind::Component,from,to,input.uint()?).map_err(Error::Id)?;
                if !seen.insert(component){return Err(Error::Invalid);}uint(&mut out,component);
            }
        }
        _=>return Err(Error::UnsupportedComponent(kind)),
    }Ok(out)
}

fn effect_detail(input:&mut Reader<'_>,depth:usize)->Result<(),Error>{
    if depth>32{return Err(Error::TooLarge);}input.count(255)?;
    if input.varint()? < -1{return Err(Error::Invalid);}
    for _ in 0..3{input.boolean()?;}
    if input.boolean()?{effect_detail(input,depth+1)?;}Ok(())
}
fn nonnegative_float(input:&mut Reader<'_>)->Result<(),Error>{
    let value=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
    if !value.is_finite() || value<0.0{return Err(Error::Invalid);}Ok(())
}

pub(crate) fn validate(kind:u32,input:&mut Reader<'_>)->Result<(),Error>{
    match kind {
        5=>{input.boolean()?;input.boolean()?;nonnegative_float(input)?;}
        7=>nonnegative_float(input)?,
        10=>skip_string(input)?,
        17=>{
            for _ in 0..input.count(1024)? {
                let value=f32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
                if !value.is_finite(){return Err(Error::Invalid);}
            }
            for _ in 0..input.count(1024)?{input.boolean()?;}
            for _ in 0..input.count(1024)?{skip_string(input)?;}
            let n=input.count(1024)?;input.take(n*4)?;
        }
        34=>(),
        23=>{input.uint()?;nonnegative_float(input)?;input.boolean()?;}
        51=>{
            //46обычных зелий и40эффектов проверены командами на775/777.
            if input.boolean()?{input.count(45)?;}
            if input.boolean()?{input.take(4)?;}
            let n=input.count(256)?;
            for _ in 0..n {input.count(39)?;effect_detail(input,0)?;}
            if input.boolean()?{skip_string(input)?;}
        }
        52=>nonnegative_float(input)?,
        54=>{
            let n=input.count(100)?;
            for _ in 0..n {skip_string(input)?;if input.boolean()?{skip_string(input)?;}}
        }
        55=>{
            skip_string(input)?;if input.boolean()?{skip_string(input)?;}skip_string(input)?;input.count(3)?;
            let n=input.count(100)?;
            for _ in 0..n {
                plain_text(input.nbt()?)?;
                if input.0.first()==Some(&0){input.take(1)?;}else{plain_text(input.nbt()?)?;}
            }
            input.boolean()?;
        }
        _=>return Err(Error::UnsupportedComponent(kind)),
    }Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{self,Component,Stack,Context};
    use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
    fn bytes(hex:&str)->Vec<u8>{hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect()}
    #[test]
    fn official_complex_items_match_and_roundtrip_including_untrusted_lengths() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
        let a=data["old"]["cases"].as_array().unwrap();let b=data["new"]["cases"].as_array().unwrap();assert_eq!(a.len(),54);assert_eq!(b.len(),54);
        for (old,new) in a.iter().zip(b) {
            assert_eq!(old["name"],new["name"]);let name=old["name"].as_str().unwrap();
            let a=bytes(old["slot_hex"].as_str().unwrap());let b=bytes(new["slot_hex"].as_str().unwrap());
            let (old,n)=items::decode(OLD,&a,false,Context::default()).unwrap_or_else(|e|panic!("{name}: {e:?}"));assert_eq!(n,a.len());
            let (new,n)=items::decode(NEW,&b,false,Context::default()).unwrap_or_else(|e|panic!("{name}: {e:?}"));assert_eq!(n,b.len());assert_eq!(old,new,"{name}");
            for (version,expected) in [(OLD,a),(NEW,b)] {
                assert_eq!(items::encode(version,old.as_ref(),false,Context::default()).unwrap(),expected,"{name}");
                let untrusted=items::encode(version,old.as_ref(),true,Context::default()).unwrap();
                let (decoded,n)=items::decode(version,&untrusted,true,Context::default()).unwrap();assert_eq!(decoded,old);assert_eq!(n,untrusted.len());
                for n in 0..expected.len(){assert!(items::decode(version,&expected[..n],false,Context::default()).is_err(),"{name} prefix{n}");}
            }
        }
    }
    #[test]
    fn all_observed_attribute_modifiers_preserve_meaning_and_new_ones_fail_downgrade() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/attribute-id-observations.json")).unwrap();
        let old=data["old"]["cases"].as_array().unwrap();let new=data["new"]["cases"].as_array().unwrap();
        assert_eq!(old.len(),35);assert_eq!(new.len(),40);
        for entry in old {
            let target=new.iter().find(|e|e["name"]==entry["name"]).unwrap();
            let a=bytes(entry["slot_hex"].as_str().unwrap());let b=bytes(target["slot_hex"].as_str().unwrap());
            let (a,_)=items::decode(OLD,&a,false,Context::default()).unwrap();let (b_decoded,_)=items::decode(NEW,&b,false,Context::default()).unwrap();
            assert_eq!(a,b_decoded,"{}",entry["name"]);assert_eq!(items::encode(NEW,a.as_ref(),false,Context::default()).unwrap(),b);
        }
        let unmatched=new.iter().filter(|e|!old.iter().any(|v|v["name"]==e["name"])).collect::<Vec<_>>();assert_eq!(unmatched.len(),5);
        for entry in unmatched {
            assert!(matches!(items::decode(NEW,&bytes(entry["slot_hex"].as_str().unwrap()),false,Context::default()),Err(Error::Id(crate::ids::IdTranslationError::MissingTarget(_)))));
        }
    }
    #[test]
    fn entity_attribute_packets_match_official_speed_health_and_scale() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/attribute-packet-observations.json")).unwrap();
        let old=data["old"]["attribute_updates"].as_array().unwrap();let new=data["new"]["attribute_updates"].as_array().unwrap();assert_eq!(old.len(),3);assert_eq!(new.len(),3);
        for (a,b) in old.iter().zip(new) {
            assert_eq!(a["name"],b["name"]);
            let a=bytes(a["packets"][0]["body"].as_str().unwrap());let b=bytes(b["packets"][0]["body"].as_str().unwrap());
            assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,OLD,NEW,&a).unwrap().as_ref(),b);
            assert_eq!(crate::wire::translate(crate::State::Play,crate::Direction::Clientbound,NEW,OLD,&b).unwrap().as_ref(),a);
        }
    }
    #[test]
    fn nested_component_and_attribute_ids_are_not_copied_blindly() {
        let mut attrs=vec![1];uint(&mut attrs,35);attrs.extend([1,b'a']);attrs.extend(1f64.to_be_bytes());attrs.extend([0,0,0]);
        let missing_attribute=Stack {item:1,count:1,added:vec![Component {kind:16,data:attrs}],removed:vec![]};
        assert!(items::encode(NEW,Some(&missing_attribute),false,Context::default()).is_err());
        let missing_tooltip=Stack {item:1,count:1,added:vec![Component {kind:18,data:vec![0,1,45]}],removed:vec![]};
        assert!(items::encode(NEW,Some(&missing_tooltip),false,Context::default()).is_err());
    }
    #[test]
    fn effect_recursion_unknown_ids_and_book_lengths_are_bounded() {
        assert!(validate(51,&mut Reader(&[1,46,0,0,0])).is_err());
        assert!(validate(51,&mut Reader(&[0,0,1,40,0,1,0,1,1,0,0])).is_err());
        let mut hidden=vec![0,0,1,0];
        for _ in 0..34{hidden.extend([0,1,0,1,1,1]);}hidden.extend([0,1,0,1,1,0,0]);
        assert_eq!(validate(51,&mut Reader(&hidden)),Err(Error::TooLarge));
        assert!(validate(54,&mut Reader(&[101])).is_err());
        assert!(validate(52,&mut Reader(&f32::NAN.to_be_bytes())).is_err());
        let infinite=[0,0,1,0,0,255,255,255,255,15,0,1,1,0,0];
        let mut input=Reader(&infinite);validate(51,&mut input).unwrap();assert!(input.0.is_empty());
    }
    #[test]
    #[ignore="экспорт наших54переведённых слотов для отдельного официального клиента"]
    fn export_complex_item_client_probe() {
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
        let mut packets=Vec::new();
        for case in data["old"]["cases"].as_array().unwrap() {
            let raw=bytes(case["slot_hex"].as_str().unwrap());let (stack,n)=items::decode(OLD,&raw,false,Context::default()).unwrap();assert_eq!(n,raw.len());
            let mut body=Vec::new();uint(&mut body,ids::translate(RegistryKind::PlayClientbound,OLD,NEW,0x6c).unwrap());uint(&mut body,0);
            body.extend(items::encode(NEW,stack.as_ref(),false,Context::default()).unwrap());
            packets.push(body.iter().map(|b|format!("{b:02x}")).collect::<String>());
        }
        let path=std::path::Path::new("tmp/complex-item-client-probe.json");std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path,serde_json::to_vec_pretty(&packets).unwrap()).unwrap();
    }
}
