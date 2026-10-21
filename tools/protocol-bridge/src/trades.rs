//! Предложения жителей: предметы и компоненты оплаты переводятся по отдельности.
//! Источник схемы: pinned protocol.json, проверки обычными summon/use_entity.
use std::collections::BTreeSet;
use crate::{JavaVersion,ids::{self,RegistryKind},items,wire::{self,Cursor,uint}};

fn cost(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:items::Context)->Result<(),wire::Error>{
    let item=context.content.packet_id(RegistryKind::Item,from,to,input.uint()?)?;
    if item==0{return Err(wire::Error::Invalid);}uint(out,item);
    let count=input.count(127)?;if count==0{return Err(wire::Error::Invalid);}uint(out,count as u32);
    let count=input.count(256)?;uint(out,count as u32);let mut seen=BTreeSet::new();
    for _ in 0..count {
        let source=input.uint()?;
        let canonical=ids::translate(RegistryKind::Component,from,wire::INTERNAL,source)?;
        if !seen.insert(canonical){return Err(wire::Error::Invalid);}
        uint(out,ids::translate(RegistryKind::Component,from,to,source)?);
        let mut reader=items::Reader(input.rest);
        let body=items::component_bytes(canonical,&mut reader,from,to,context).map_err(wire::Error::Item)?;
        input.take(input.rest.len()-reader.0.len())?;out.extend(body);
    }Ok(())
}

pub(crate) fn translate(input:&mut Cursor<'_>,out:&mut Vec<u8>,from:JavaVersion,to:JavaVersion,context:items::Context)->Result<(),wire::Error>{
    uint(out,input.uint()?);let count=input.count(1024)?;uint(out,count as u32);
    for _ in 0..count {
        cost(input,out,from,to,context)?;
        let (stack,n)=items::decode_packet_slot(from,input.rest,false,context).map_err(wire::Error::Item)?;
        input.take(n)?;out.extend(items::encode_packet_slot(to,stack.as_ref(),false,context).map_err(wire::Error::Item)?);
        let second=input.boolean()?;out.push(u8::from(second));if second{cost(input,out,from,to,context)?;}
        out.push(u8::from(input.boolean()?));
        out.extend(input.take(16)?); // uses/maxUses/xp/specialPrice, включая отрицательную скидку.
        let multiplier=input.take(4)?;
        if !f32::from_be_bytes(multiplier.try_into().map_err(|_|wire::Error::Invalid)?).is_finite(){return Err(wire::Error::Invalid);}
        out.extend(multiplier);out.extend(input.take(4)?); // demand — знаковое число.
    }
    uint(out,input.count(5)? as u32);uint(out,input.varint()? as u32);
    out.push(u8::from(input.boolean()?));out.push(u8::from(input.boolean()?));Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{State,Direction,entity_layouts};
    use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
    fn fixture()->serde_json::Value{serde_json::from_str(include_str!("../testdata/trading-observations.json")).unwrap()}
    fn bytes(hex:&str)->Vec<u8>{hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect()}
    fn convert(body:&[u8],from:JavaVersion,to:JavaVersion)->Result<Vec<u8>,wire::Error>{
        let kind=entity_layouts::LAYOUTS.iter().position(|(name,_,_)|*name=="villager").unwrap() as u32;
        Ok(wire::translate_with_context(State::Play,Direction::Clientbound,from,to,body,wire::Context {entity_kind:Some(kind),..Default::default()})?.into_owned())
    }
    #[test]
    fn official_offers_match_both_directions_and_reject_truncation(){
        let data=fixture();
        for (a,b) in data["old"]["trades"].as_array().unwrap().iter().zip(data["new"]["trades"].as_array().unwrap()) {
            assert_eq!(a["name"],b["name"]);
            let packet=|case:&serde_json::Value|bytes(case["packets"].as_array().unwrap().iter().find(|p|p["name"]=="trade_list").unwrap()["body"].as_str().unwrap());
            let a=packet(a);let b=packet(b);
            assert_eq!(convert(&a,OLD,NEW).unwrap(),b);assert_eq!(convert(&b,NEW,OLD).unwrap(),a);
            for (version,target,body) in [(OLD,NEW,&a),(NEW,OLD,&b)]{
                for n in 0..body.len(){assert!(convert(&body[..n],version,target).is_err(),"prefix {n}");}
                let mut trailing=body.clone();trailing.push(0);assert!(convert(&trailing,version,target).is_err());
            }
        }
    }
    #[test]
    fn observed_villager_type_profession_and_level_ids_preserve_meaning(){
        let data=fixture();let types=["desert","jungle","plains","savanna","snow","swamp","taiga"];
        let professions=["none","armorer","butcher","cartographer","cleric","farmer","fisherman","fletcher","leatherworker","librarian","mason","nitwit","shepherd","toolsmith","weaponsmith"];
        let old=data["old"]["variants"].as_array().unwrap();let new=data["new"]["variants"].as_array().unwrap();assert_eq!(old.len(),25);assert_eq!(new.len(),25);
        for (a,b) in old.iter().zip(new){
            for key in ["type","profession","level"]{assert_eq!(a[key],b[key]);}
            let first=|case:&serde_json::Value|bytes(case["packets"][0]["body"].as_str().unwrap());let old=first(a);let new=first(b);
            assert_eq!(convert(&old,OLD,NEW).unwrap(),new);assert_eq!(convert(&new,NEW,OLD).unwrap(),old);
            for body in [&old,&new]{
                let mut input=Cursor::new(body);input.uint().unwrap();input.uint().unwrap();let mut seen=false;
                loop{let index=input.take(1).unwrap()[0];if index==255{break;}let kind=input.uint().unwrap();match (index,kind){
                    (9,3)=>{input.take(4).unwrap();},(15,0)|(20,8)=>{input.take(1).unwrap();},
                    (19,18)=>{assert_eq!(input.uint().unwrap() as usize,types.iter().position(|v|a["type"]==*v).unwrap());assert_eq!(input.uint().unwrap() as usize,professions.iter().position(|v|a["profession"]==*v).unwrap());assert_eq!(input.uint().unwrap() as u64,a["level"].as_u64().unwrap());seen=true;},
                    _=>panic!("unexpected observation {index}/{kind}"),
                }}assert!(seen);assert!(input.rest.is_empty());
            }
        }
    }
    #[test]
    fn merchant_menu_preserves_its_title_including_dynamic_uuid(){
        let data=fixture();for (side,from,to) in [("old",OLD,NEW),("new",NEW,OLD)]{
            for case in data[side]["trades"].as_array().unwrap(){
                let packet=case["packets"].as_array().unwrap().iter().find(|p|p["name"]=="open_window").unwrap();let body=bytes(packet["body"].as_str().unwrap());
                let mut input=Cursor::new(&body);input.uint().unwrap();input.uint().unwrap();assert_eq!(input.uint().unwrap(),19);
                let translated=convert(&body,from,to).unwrap();assert_eq!(&body[1..],&translated[1..]);assert_eq!(convert(&translated,to,from).unwrap(),body);
            }
        }
    }
    #[test]
    fn trade_prices_are_not_recomputed_and_invalid_costs_fail(){
        let data=fixture();let packet=&data["old"]["trades"][1]["packets"][1];assert_eq!(packet["name"],"trade_list");
        let mut body=bytes(packet["body"].as_str().unwrap());assert!(body.ends_with(&[2,0,1,1]));
        let offset=body.len()-16;assert_eq!(&body[offset..offset+4],&[0;4]);body[offset..offset+4].copy_from_slice(&(-3i32).to_be_bytes());
        let mapped=convert(&body,OLD,NEW).unwrap();assert_eq!(&mapped[mapped.len()-16..mapped.len()-12],&(-3i32).to_be_bytes());assert_eq!(convert(&mapped,NEW,OLD).unwrap(),body);
        for bad in [&[0x34,1,1,0][..],&[0x34,1,1,1,0],&[0x34,1,1,1,1,2,3,1,3,1],&[0x34,1,1,1,1,1,45]]{assert!(convert(bad,OLD,NEW).is_err());}
    }
    #[test]
    #[ignore="экспорт переведённых окон и предложений для отдельного официального клиента"]
    fn export_trading_client_probe(){
        let data=fixture();let mut packets=Vec::new();
        for case in data["old"]["trades"].as_array().unwrap(){
            for packet in case["packets"].as_array().unwrap(){packets.push(convert(&bytes(packet["body"].as_str().unwrap()),OLD,NEW).unwrap());}
        }
        // Отдельно проверяем принятие отрицательной скидки, не заявляя
        // совпадение правил её расчёта в двух официальных версиях.
        let case=&data["old"]["trades"][1];packets.push(convert(&bytes(case["packets"][0]["body"].as_str().unwrap()),OLD,NEW).unwrap());
        let mut body=bytes(case["packets"][1]["body"].as_str().unwrap());let offset=body.len()-16;body[offset..offset+4].copy_from_slice(&(-3i32).to_be_bytes());packets.push(convert(&body,OLD,NEW).unwrap());
        let packets=packets.iter().map(|body|body.iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>();
        std::fs::create_dir_all("tmp").unwrap();std::fs::write("tmp/trading-client-probe.json",serde_json::to_vec_pretty(&packets).unwrap()).unwrap();
    }
}
