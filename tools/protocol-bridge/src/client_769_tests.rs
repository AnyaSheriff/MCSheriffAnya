//! Регрессии769: собственные таблицы, явные виды и строгие границы payload.
use crate::{JavaVersion as V, items, content::{ContentCodec,IdSpace}, ids::RegistryKind as K, wire::{self,Context}};
const LOW:V=V::V1_21_4;
const BASE:V=V::V26_1_2;
fn packet(name:&str,payload:&[u8])->Vec<u8>{
    let id=(0..256).find(|&id|crate::packet_layouts::packet_info(K::PlayClientbound,BASE,LOW,id).is_some_and(|r|r.name==name)).unwrap();
    let mut out=Vec::new();wire::uint(&mut out,id as u32);out.extend(payload);out
}
#[test]
fn all_canonical_content_has_explicit_769_views_without_changing_ids(){
    let codec=ContentCodec::default();
    for (kind,count) in [(K::Item,1506),(K::Block,1168),(K::BlockState,29873),(K::Entity,157)] {
        for id in 0..count {
            let view=codec.map(kind,IdSpace::Canonical,IdSpace::Wire(LOW),id).unwrap();
            assert!(codec.map(kind,IdSpace::Wire(LOW),IdSpace::Canonical,view).is_ok(),"{kind:?}/{id}");
            assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Canonical,id),Ok(id));
        }
    }
    assert_eq!(items::decode(LOW,&[0],false,Default::default()).unwrap(),(None,1));
}
#[test]
fn tooltip_booleans_are_typed_and_untrusted769_has_no_length_envelope(){
    for (kind,data,context) in [(4,vec![],items::Context::default()),(13,vec![1,0,2],items::Context{shared_enchantment_count:Some(3),..Default::default()}),(44,vec![0,0,255,0],items::Context::default())] {
        let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind,data}],removed:vec![]};
        let wire=items::encode(LOW,Some(&stack),false,context).unwrap();
        assert_eq!(wire.last(),Some(&1));
        assert_eq!(items::encode(LOW,Some(&stack),true,context).unwrap(),wire);
        assert_eq!(items::decode(LOW,&wire,true,context).unwrap(),(Some(stack),wire.len()));
        for n in 1..wire.len(){assert!(items::decode(LOW,&wire[..n],true,context).is_err());}
        let mut bad=wire;*bad.last_mut().unwrap()=2;assert!(items::decode(LOW,&bad,false,context).is_err());
    }
}
#[test]
fn chat769_adds_only_the_later_checksum_and_rejects_a_tail(){
    let id=crate::generated_client_chain::packets(K::PlayServerbound,LOW,V::V1_21_5).unwrap().iter().position(|r|r.0=="chat_message").unwrap();
    let mut source=vec![id as u8,1,b'x'];source.extend([0;16]);source.extend([0,0,0,0,0]);
    let result=super::serverbound(&source,Context::default()).unwrap();assert_eq!(result.len(),source.len()+1);
    source.push(0);assert!(super::serverbound(&source,Context::default()).is_err());
}
#[test]
fn dropped_variants_still_parse_the_complete_metadata_value(){
    let kind=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0=="cow").unwrap() as u32;
    let layout=crate::entity_layouts::LAYOUTS[kind as usize].1;
    let index=layout.iter().position(|n|*n=="variant").unwrap() as u8;
    let body=packet("entity_metadata",&[42,index,23,0,255]);
    let context=Context{entity_kind:Some(kind),metadata_registry_counts:&[("minecraft:cow_variant",3)],..Default::default()};
    let translated=wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&body,context,true).unwrap();
    assert_eq!(&translated[1..],&[42,255]);
    let mut tail=body;tail.push(0);assert!(wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&tail,context,true).is_err());
}
#[test]
fn palettes769_include_explicit_word_counts_and_reject_truncated_lengths(){
    use crate::chunk_sections::{self,Section};
    for distinct in [1,17,300] {
        let section=Section{blocks_present:4096,fluids_present:0,blocks:(0..4096).map(|n|1+n%distinct).collect(),biomes:vec![0;64]};
        let bytes=chunk_sections::encode(LOW,std::slice::from_ref(&section),1).unwrap();
        if distinct==1 {assert_eq!(bytes,&[16,0,0,1,0,0,0,0]);}
        assert_eq!(chunk_sections::decode(LOW,&bytes,1,1).unwrap(),vec![section]);
        for n in [0,2,bytes.len()-1] {assert!(chunk_sections::decode(LOW,&bytes[..n],1,1).is_err());}
        let mut tail=bytes;tail.push(0);assert!(chunk_sections::decode(LOW,&tail,1,1).is_err());
    }
}

#[test]
fn attributes769_skip_camera_and_waypoints_without_shifting_attack(){
    // attack_damage, entity_interaction_range, max_health, movement_speed,
    // water_movement_efficiency: закреплённые имена775 без camera_distance.
    for (canonical,older) in [(2,2),(9,8),(19,18),(22,21),(32,31)] {
        assert_eq!(crate::ids::translate(K::Attribute,BASE,LOW,canonical),Ok(older));
        assert_eq!(crate::ids::translate(K::Attribute,LOW,BASE,older),Ok(canonical));
    }
    for id in [8,33,34] {assert!(crate::ids::translate(K::Attribute,BASE,LOW,id).is_err());}
    assert!(crate::ids::translate(K::Attribute,LOW,BASE,32).is_err());
}

#[test]
fn advancements769_have_no_trailing_show_flag(){
    let source=packet("advancements",&[1,0,0,0,1]);
    let translated=wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&source,Context::default(),true).unwrap();
    assert_eq!(&translated[1..],&[1,0,0,0]);
    for n in 0..source.len() {assert!(wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&source[..n],Context::default(),true).is_err());}
    let mut tail=source;tail.push(0);assert!(wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&tail,Context::default(),true).is_err());
}

#[test]
fn full_slot_click769_keeps_the_connections_enchantment_bounds(){
    let context=Context{items:items::Context{shared_enchantment_count:Some(1),..Default::default()},..Default::default()};
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:13,data:vec![1,0,1]}],removed:vec![]};
    let id=crate::generated_client_chain::packets(K::PlayServerbound,LOW,V::V1_21_5).unwrap().iter().position(|r|r.0=="window_click").unwrap();
    let mut source=vec![id as u8,0,0,0,0,0,0,1,0,0];source.extend(items::encode(LOW,Some(&stack),false,context.items).unwrap());source.push(0);
    let translated=super::serverbound(&source,context).unwrap();
    assert_eq!(&translated[1..],&[0,0,0,0,0,0,1,0,0,0,0]);
    assert!(super::serverbound(&source,Context::default()).is_err());
    let wrong=Context{items:items::Context{shared_enchantment_count:Some(0),..Default::default()},..Default::default()};
    assert!(super::serverbound(&source,wrong).is_err());
    source.push(0);assert!(super::serverbound(&source,context).is_err());
}
