//! Только reviewed37: wind binding BLOCKED, контекст не включён в Session.
//! Имена/ID взяты из обычного immutable network capture; wolf — canonical predicates.
use std::collections::{BTreeMap,BTreeSet};
use crate::{ids::RegistryKind,wire::{Cursor,Error,uint}};

pub const BLOCKED_WIND:&str="minecraft:blocks_wind_charge_explosions";
const LIMIT:usize=65536;
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Tags { groups:BTreeMap<String,BTreeMap<String,Vec<u32>>>,raw:Option<Vec<u8>> }
fn name(value:&str)->Result<(),Error>{
    if !value.starts_with("minecraft:")||value.len()<=10||value[10..].contains(':')||value.len()>256||!value.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b"_:/.".contains(&b)){return Err(Error::Invalid);}Ok(())
}
fn bounds()->Result<BTreeMap<String,usize>,Error>{
    let mut result=BTreeMap::new();
    for registry in super::reference_registries()?{if result.insert(registry.name,registry.entries.len()).is_some(){return Err(Error::Invalid);}}
    Ok(result)
}
fn member(registry:&str,id:u32,bounds:&BTreeMap<String,usize>)->Result<(),Error>{
    let kind=match registry {"minecraft:item"=>Some(RegistryKind::Item),"minecraft:block"=>Some(RegistryKind::Block),"minecraft:entity_type"=>Some(RegistryKind::Entity),_=>None};
    if let Some(kind)=kind{return super::validate_id(kind,768,id);}
    let count=bounds.get(registry).ok_or(Error::Invalid)?;
    if id as usize>=*count{return Err(Error::Invalid);}Ok(())
}
impl Tags {
    /// Payload без packetID; отдельная768schema с bound/duplicate/domain guards.
    pub fn decode(bytes:&[u8])->Result<Self,Error>{
        if bytes.len()>8*1024*1024{return Err(Error::TooLarge);}
        let mut input=Cursor::new(bytes);let count=input.uint()? as usize;
        if count>16{return Err(Error::TooLarge);}
        // count<=16: canonical header is exactly one byte; private splice relies on it.
        if bytes.len()-input.remaining()!=1{return Err(Error::Invalid);}
        let bounds=bounds()?;
        let mut groups=BTreeMap::new();
        for _ in 0..count {
            let registry=input.string()?.to_owned();name(&registry)?;
            if !matches!(registry.as_str(),"minecraft:item"|"minecraft:block"|"minecraft:entity_type"|"minecraft:enchantment"|"minecraft:worldgen/biome"|"minecraft:damage_type"|"minecraft:banner_pattern"){return Err(Error::Invalid);}
            let n=input.uint()? as usize;if n>LIMIT{return Err(Error::TooLarge);}
            let mut tags=BTreeMap::new();
            for _ in 0..n {
                let tag=input.string()?.to_owned();name(&tag)?;
                let n=input.uint()? as usize;if n>LIMIT{return Err(Error::TooLarge);}
                let mut ids=Vec::new();let mut seen=BTreeSet::new();
                for _ in 0..n {let id=input.uint()?;member(&registry,id,&bounds)?;if !seen.insert(id){return Err(Error::Invalid);}ids.push(id);}
                if tags.insert(tag,ids).is_some(){return Err(Error::Invalid);}
            }
            if groups.insert(registry,tags).is_some(){return Err(Error::Invalid);}
        }
        if input.remaining()!=0{return Err(Error::Invalid);}Ok(Self{groups,raw:Some(bytes.to_vec())})
    }
    pub fn encode(&self)->Vec<u8>{
        if let Some(raw)=&self.raw{return raw.clone();}
        fn string(out:&mut Vec<u8>,text:&str){uint(out,text.len() as u32);out.extend(text.as_bytes());}
        let mut out=Vec::new();uint(&mut out,self.groups.len() as u32);
        for (registry,tags) in &self.groups {string(&mut out,registry);uint(&mut out,tags.len() as u32);
            for (tag,ids) in tags {string(&mut out,tag);uint(&mut out,ids.len() as u32);for &id in ids{uint(&mut out,id);}}
        }out
    }
}
/// Неполный тип: нельзя превратить его в runtime closure без доказанного wind.
pub struct RequiredTags { reviewed:Tags }
impl RequiredTags {
    pub fn reviewed37()->Result<Self,Error>{
        let reviewed=Tags::decode(include_bytes!("../testdata/private768-reference/coherent37-tags.bin"))?;
        if reviewed.groups.values().map(BTreeMap::len).sum::<usize>()!=37||reviewed.groups.values().any(|g|g.contains_key(BLOCKED_WIND)){return Err(Error::Invalid);}
        Ok(Self{reviewed})
    }
    pub fn reviewed(&self)->&Tags{&self.reviewed}
    /// Не принимает raw/empty wind: в этом checkpoint нет VerifiedWind constructor.
    pub fn complete(&self)->Result<Tags,Error>{Err(Error::PayloadUnavailable("private768 unverified wind binding"))}
    /// Только offline review merge; canonical damage/banner payload неизменен.
    pub fn review_with_overrides(&self,canonical:&Tags)->Result<Tags,Error>{
        if canonical.groups.keys().any(|r|!matches!(r.as_str(),"minecraft:damage_type"|"minecraft:banner_pattern")){return Err(Error::Invalid);}
        let mut merged=self.reviewed.clone();
        for (r,tags) in &canonical.groups {if merged.groups.insert(r.clone(),tags.clone()).is_some(){return Err(Error::Invalid);}}
        let mut raw=Vec::new();uint(&mut raw,merged.groups.len() as u32);
        raw.extend(&self.reviewed.encode()[1..]);raw.extend(&canonical.encode()[1..]);merged.raw=Some(raw);
        Ok(merged)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn reviewed_closure_stays_blocked(){
        let ctx=RequiredTags::reviewed37().unwrap();assert_eq!(ctx.reviewed.encode(),include_bytes!("../testdata/private768-reference/coherent37-tags.bin"));
        assert!(ctx.complete().is_err());
        let doc:serde_json::Value=serde_json::from_str(include_str!("../testdata/private768-reference/required-tags-review.json")).unwrap();
        assert_eq!(doc["required"]["required_tag_count"],38);assert_eq!(doc["canonical_audit"]["symmetric_graph_pairs"],1764);assert_eq!(doc["canonical_audit"]["symmetric_graph_all_equal"],true);
        for binding in doc["required"]["required_tags"].as_array().unwrap(){
            let registry=binding["registry"].as_str().unwrap();let tag=binding["tag"].as_str().unwrap();
            if tag==BLOCKED_WIND{assert!(!ctx.reviewed.groups[registry].contains_key(tag));continue;}
            let members=binding["members"].as_array().unwrap();
            let expected:Vec<u32>=members.iter().filter(|m|registry!="minecraft:worldgen/biome"||matches!(m["name"].as_str().unwrap(),"minecraft:sparse_jungle"|"minecraft:savanna_plateau"|"minecraft:wooded_badlands")).map(|m|m["target_768_id"].as_u64().unwrap() as u32).collect();
            assert_eq!(ctx.reviewed.groups[registry][tag],expected);
        }
        for row in doc["canonical_audit"]["explicit_primary_items"].as_array().unwrap(){assert_eq!(row["equal_projected"],true);assert!(!row["known_777_only_omitted"].as_array().unwrap().is_empty());}
    }
    #[test]fn rejects_duplicate_tags_members_and_domains(){
        fn string(out:&mut Vec<u8>,s:&str){uint(out,s.len() as u32);out.extend(s.as_bytes());}
        fn payload(registry:&str,repeated_tag:bool,repeated_member:bool)->Vec<u8>{
            let mut out=vec![1];string(&mut out,registry);uint(&mut out,if repeated_tag{2}else{1});
            for _ in 0..if repeated_tag{2}else{1}{string(&mut out,"minecraft:test");uint(&mut out,if repeated_member{2}else{1});out.push(0);if repeated_member{out.push(0);}}
            out
        }
        assert!(Tags::decode(&payload("minecraft:item",false,false)).is_ok());
        assert!(Tags::decode(&payload("minecraft:item",true,false)).is_err());
        assert!(Tags::decode(&payload("minecraft:item",false,true)).is_err());
        assert!(Tags::decode(&payload("minecraft:unknown",false,false)).is_err());
        assert!(Tags::decode(&payload("minecraft:Item",false,false)).is_err());
    }
    #[test]fn override_bytes_and_guards(){
        let ctx=RequiredTags::reviewed37().unwrap();
        let actual=include_bytes!("../testdata/private768-reference/canonical-overrides-tags.bin");
        let preserved=Tags::decode(actual).unwrap();assert_eq!(preserved.encode(),actual);
        assert!(ctx.review_with_overrides(&preserved).unwrap().encode().ends_with(&actual[1..]));
        let mut groups=BTreeMap::new();
        groups.insert("minecraft:damage_type".to_owned(),BTreeMap::from([("minecraft:bypasses_armor".to_owned(),vec![])]));
        groups.insert("minecraft:banner_pattern".to_owned(),BTreeMap::from([("minecraft:pattern_item/flower".to_owned(),vec![0])]));
        let original=Tags{groups,raw:None};let before=original.encode();let merged=ctx.review_with_overrides(&original).unwrap();
        let recovered=Tags{groups:merged.groups.into_iter().filter(|(r,_)|original.groups.contains_key(r)).collect(),raw:None};assert_eq!(before,recovered.encode());
        let bytes=ctx.reviewed.encode();for n in 0..bytes.len(){assert!(Tags::decode(&bytes[..n]).is_err());}
        assert!(Tags::decode(&[255,255,255,255,127]).is_err());
        let mut overlong=vec![bytes[0]|128,0];overlong.extend(&bytes[1..]);assert!(Tags::decode(&overlong).is_err());
        let mut override_overlong=vec![actual[0]|128,0];override_overlong.extend(&actual[1..]);assert!(Tags::decode(&override_overlong).is_err());
        let mut trailing=bytes;trailing.push(0);assert!(Tags::decode(&trailing).is_err());
        assert!(member("minecraft:item",u32::MAX,&bounds().unwrap()).is_err());assert!(member("minecraft:unknown",0,&bounds().unwrap()).is_err());assert!(name("minecraft:Bad").is_err());assert!(name("minecraft:").is_err());assert!(name("minecraft:a:b").is_err());
        let mut duplicate=vec![2];duplicate.extend(&before[1..]);duplicate.extend(&before[1..]);assert!(Tags::decode(&duplicate).is_err());
    }
}
