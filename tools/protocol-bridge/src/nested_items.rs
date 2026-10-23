//! Вложенные ItemStackTemplate: ID идёт перед количеством (не обычный Slot).
//! Каждый уровень переводит свои компоненты; глубина и размеры ограничены.
use std::collections::BTreeSet;
use crate::content::IdSpace;
use crate::{JavaVersion,ids::{self,RegistryKind},items::{self,Reader,Context,Error,uint}};

fn template(input:&mut Reader<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:Context,depth:usize,spaces:(IdSpace,IdSpace))->Result<(),Error>{
    let old_count = if from == JavaVersion::V1_21_11 { Some(input.count(127)? as u32) } else { None };
    if old_count == Some(0) { return Err(Error::Invalid); }
    let item=items::content_id(context,RegistryKind::Item,spaces.0,spaces.1,input.uint()?)?;
    if item==0{return Err(Error::Invalid);}
    let count=match old_count {Some(n)=>n,None=>input.count(127)? as u32};if count==0{return Err(Error::Invalid);}
    if to==JavaVersion::V1_21_11 {uint(out,count);uint(out,item);}else{uint(out,item);uint(out,count);}
    let added=input.count(256)?;let removed=input.count(256)?;uint(out,added as u32);uint(out,removed as u32);
    let mut seen=BTreeSet::new();
    for _ in 0..added {
        let source=input.uint()?;let kind=ids::translate(RegistryKind::Component,from,JavaVersion::V26_1_2,source).map_err(Error::Id)?;
        if !seen.insert(kind){return Err(Error::Invalid);}
        uint(out,ids::translate(RegistryKind::Component,from,to,source).map_err(Error::Id)?);
        out.extend(items::component_bytes_at_spaces(kind,input,from,to,context,depth,spaces)?);
        if out.len()>items::LIMIT{return Err(Error::TooLarge);}
    }
    for _ in 0..removed {
        let source=input.uint()?;let kind=ids::translate(RegistryKind::Component,from,JavaVersion::V26_1_2,source).map_err(Error::Id)?;
        if !seen.insert(kind){return Err(Error::Invalid);}
        uint(out,ids::translate(RegistryKind::Component,from,to,source).map_err(Error::Id)?);
    }Ok(())
}

#[cfg(test)]
pub(crate) fn remap(kind:u32,input:&mut Reader<'_>,from:JavaVersion,to:JavaVersion,context:Context,depth:usize)->Result<Vec<u8>,Error>{
    remap_with_spaces(kind,input,from,to,context,depth,(IdSpace::Wire(from),IdSpace::Wire(to)))
}

pub(crate) fn remap_with_spaces(kind:u32,input:&mut Reader<'_>,from:JavaVersion,to:JavaVersion,context:Context,depth:usize,spaces:(IdSpace,IdSpace))->Result<Vec<u8>,Error>{
    if depth>=32{return Err(Error::TooLarge);}let mut out=Vec::new();
    match kind {
        25=>template(input,&mut out,from,to,context,depth+1,spaces)?,
        49|50|75=>{
            let count=input.count(256)?;uint(&mut out,count as u32);
            for _ in 0..count {
                let present=if kind!=75 {true} else if from==JavaVersion::V1_21_11 {if input.0.first()==Some(&0){input.take(1)?;false}else{true}} else {input.boolean()?};
                if kind==75 && to!=JavaVersion::V1_21_11 {out.push(u8::from(present));}
                if !present && to==JavaVersion::V1_21_11 {out.push(0);}
                if present{template(input,&mut out,from,to,context,depth+1,spaces)?;}
                if out.len()>items::LIMIT{return Err(Error::TooLarge);}
            }
        }
        _=>return Err(Error::UnsupportedComponent(kind)),
    }Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Stack,Component};
    use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
    fn bytes(hex:&str)->Vec<u8>{hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect()}
    fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../testdata/nested-item-observations.json")).unwrap()}
    #[test]
    fn official_nested_items_match_exactly_and_reject_all_truncated_prefixes(){
        let data=fixture();let old=data["old"]["cases"].as_array().unwrap();let new=data["new"]["cases"].as_array().unwrap();assert_eq!(old.len(),9);assert_eq!(new.len(),9);
        for (a,b) in old.iter().zip(new){
            assert_eq!(a["name"],b["name"]);let a=bytes(a["slot_hex"].as_str().unwrap());let b=bytes(b["slot_hex"].as_str().unwrap());
            let (stack,n)=items::decode(OLD,&a,false,Context::default()).unwrap();assert_eq!(n,a.len());
            let (other,n)=items::decode(NEW,&b,false,Context::default()).unwrap();assert_eq!(n,b.len());assert_eq!(stack,other);
            for (version,expected) in [(OLD,a),(NEW,b)]{
                assert_eq!(items::encode(version,stack.as_ref(),false,Context::default()).unwrap(),expected);
                let untrusted=items::encode(version,stack.as_ref(),true,Context::default()).unwrap();
                let (decoded,n)=items::decode(version,&untrusted,true,Context::default()).unwrap();assert_eq!(n,untrusted.len());assert_eq!(decoded,stack);
                for n in 0..expected.len(){assert!(items::decode(version,&expected[..n],false,Context::default()).is_err());}
            }
        }
    }
    #[test]
    fn attributes_inside_nested_templates_use_observed_target_ids(){
        let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/attribute-id-observations.json")).unwrap();
        let template=|case:&serde_json::Value|{
            let raw=bytes(case["slot_hex"].as_str().unwrap());let mut input=Reader(&raw);let count=input.uint().unwrap();let item=input.uint().unwrap();
            let mut out=vec![1];uint(&mut out,item);uint(&mut out,count);out.extend(input.0);out
        };
        for old in data["old"]["cases"].as_array().unwrap(){
            let new=data["new"]["cases"].as_array().unwrap().iter().find(|v|v["name"]==old["name"]).unwrap();
            let source=template(old);let expected=template(new);
            let mut input=Reader(&source);assert_eq!(remap(50,&mut input,OLD,NEW,Context::default(),0).unwrap(),expected);assert!(input.0.is_empty());
        }
    }
    #[test]
    fn nesting_depth_counts_and_duplicate_components_are_bounded(){
        let mut body=vec![0];
        for _ in 0..31{let mut outer=vec![1,1,1,1,0,50];outer.extend(body);body=outer;}
        let stack=|body|Stack {item:1,count:1,added:vec![Component {kind:50,data:body}],removed:vec![]};
        assert!(items::encode(NEW,Some(&stack(body.clone())),false,Context::default()).is_ok());
        let mut too_deep=vec![1,1,1,1,0,50];too_deep.extend(body);
        assert_eq!(items::encode(NEW,Some(&stack(too_deep)),false,Context::default()),Err(Error::TooLarge));
        for bad in [vec![1,0,1,0,0],vec![1,1,0,0,0],vec![1,1,1,2,0,3,0,3,0],vec![1,1,1,1,1,3,0,3]]{
            assert_eq!(items::encode(NEW,Some(&stack(bad)),false,Context::default()),Err(Error::Invalid));
        }
        assert_eq!(items::encode(NEW,Some(&stack(vec![129,2])),false,Context::default()),Err(Error::TooLarge));
        let malformed=Stack {item:1,count:1,added:vec![Component {kind:22,data:vec![8,0,0]}],removed:vec![]};
        assert_eq!(items::encode(NEW,Some(&malformed),false,Context::default()),Err(Error::Invalid));
    }
    #[test]
    #[ignore="экспорт переведённых вложенных предметов для официального клиента"]
    fn export_nested_item_client_probe(){
        let data=fixture();let mut packets=Vec::new();
        for case in data["old"]["cases"].as_array().unwrap(){
            let raw=bytes(case["slot_hex"].as_str().unwrap());let (stack,n)=items::decode(OLD,&raw,false,Context::default()).unwrap();assert_eq!(n,raw.len());
            let mut body=Vec::new();uint(&mut body,ids::translate(RegistryKind::PlayClientbound,OLD,NEW,0x6c).unwrap());uint(&mut body,0);
            body.extend(items::encode(NEW,stack.as_ref(),false,Context::default()).unwrap());packets.push(body.iter().map(|b|format!("{b:02x}")).collect::<String>());
        }
        std::fs::create_dir_all("tmp").unwrap();std::fs::write("tmp/nested-item-client-probe.json",serde_json::to_vec_pretty(&packets).unwrap()).unwrap();
    }
}
