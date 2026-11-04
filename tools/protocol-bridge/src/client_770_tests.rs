//! Регрессии770: собственные таблицы, явные виды и строгие границы payload.
use crate::{JavaVersion as V, items, content::{ContentCodec,IdSpace}, ids::RegistryKind as K, wire::{self,Context}, entities::Tracker};
const LOW:V=V::V1_21_5;
const BASE:V=V::V26_1_2;
fn packet(name:&str,payload:&[u8])->Vec<u8>{
    let id=(0..256).find(|&id|crate::packet_layouts::packet_info(K::PlayClientbound,BASE,LOW,id).is_some_and(|r|r.name==name)).unwrap();
    let mut out=Vec::new();wire::uint(&mut out,id as u32);out.extend(payload);out
}
#[test]
fn all_canonical_content_has_explicit_770_views_without_changing_ids(){
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
fn profile770_keeps_partial_identity_and_properties_in_its_old_envelope(){
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:70,data:vec![0,1,1,b'x',0,0,0,0,0,0]}],removed:vec![]};
    for untrusted in [false,true] {
        let bytes=items::encode(LOW,Some(&stack),untrusted,Default::default()).unwrap();
        assert_eq!(items::decode(LOW,&bytes,untrusted,Default::default()).unwrap(),(Some(stack.clone()),bytes.len()));
        for n in 0..bytes.len(){assert!(items::decode(LOW,&bytes[..n],untrusted,Default::default()).is_err());}
    }
}
#[test]
fn spawn_and_velocity770_use_trailing_three_shorts_and_check_bounds(){
    let kind=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0=="cow").unwrap() as u32;
    let mut payload=vec![9];payload.extend([1;16]);wire::uint(&mut payload,kind);
    for v in [1.0f64,64.0,2.0]{payload.extend(v.to_be_bytes());}
    crate::entity_sync::push_velocity(&mut payload,0.5,0.0,-0.25);payload.extend([2,3,4,0]);
    let source=packet("spawn_entity",&payload);let bytes=crate::client_770::clientbound(&mut Tracker::default(),&source,Context::default()).unwrap().into_owned();
    assert_eq!(bytes.len(),1+1+16+1+24+3+1+6);
    for n in 0..source.len(){assert!(crate::client_770::clientbound(&mut Tracker::default(),&source[..n],Context::default()).is_err());}
    let mut bad=source.clone();bad.push(0);assert!(crate::client_770::clientbound(&mut Tracker::default(),&bad,Context::default()).is_err());
    let mut payload=vec![9];crate::entity_sync::push_velocity(&mut payload,0.5,0.0,-0.25);
    let bytes=crate::client_770::clientbound(&mut Tracker::default(),&packet("entity_velocity",&payload),Context::default()).unwrap().into_owned();assert_eq!(bytes.len(),8);
}
#[test]
fn removed_shelf_and_statue_are_visual_blocks_and_not_invalid_block_entities(){
    for kind in [40,48] {
        assert_eq!(crate::block_entities::translate(BASE,LOW,kind,&[10,0]),Ok(None));
        let mut payload=vec![0;8];wire::uint(&mut payload,kind);payload.extend([10,0]);
        let mut tracker=Tracker::default();
        if kind==48 {let mut state=vec![0;8];wire::uint(&mut state,crate::generated_payload_770::STATE_VIEWS.iter().find(|r|r.1==crate::generated_payload_770::REMOVED_BE_VIEWS.iter().find(|r|r.0==48).unwrap().1).unwrap().0);crate::client_770::clientbound(&mut tracker,&packet("block_change",&state),Context::default()).unwrap();}
        let bytes=crate::client_770::clientbound(&mut tracker,&packet("tile_entity_data",&payload),Context::default()).unwrap().into_owned();
        assert_eq!(bytes[0],0x08); // pinned770: block_change, не boss_bar09.
    }
    let data:serde_json::Value=serde_json::from_str(include_str!("../../data/blocks.json")).unwrap();
    let statue=data.as_array().unwrap().iter().find(|r|r["name"]=="copper_golem_statue").unwrap();
    let mut tracker=Tracker::default();
    let mut tile=vec![0;8];tile.extend([48,10,0]);let tile=packet("tile_entity_data",&tile);
    assert!(crate::client_770::clientbound(&mut tracker,&tile,Context::default()).is_err());
    for state in statue["minStateId"].as_u64().unwrap()..=statue["maxStateId"].as_u64().unwrap() {
        let mut payload=vec![0;8];wire::uint(&mut payload,state as u32);
        let view=crate::client_770::clientbound(&mut tracker,&packet("block_change",&payload),Context::default()).unwrap().into_owned();
        assert_eq!(crate::client_770::clientbound(&mut tracker,&tile,Context::default()).unwrap(),view);
    }
    for kind in [41,42,43,44,45] {assert_eq!(crate::block_entities::translate(BASE,LOW,kind,&[0]).unwrap().unwrap().0,kind-1);}
}
#[test]
fn particle770_parameters_follow_the_named_pinned_variants(){
    let target=|kind|ids_for_particle(kind);
    // tinted_leaves36 сохраняет цвет; flash42, effect16 и dragon_breath8 — void.
    for (kind,data,expected) in [(36,vec![0,255,0,0],vec![0,255,0,0]),(42,vec![0,255,0,0],vec![]),(16,vec![0,255,0,0,63,128,0,0],vec![]),(8,vec![63,0,0,0],vec![]),(46,vec![0,255,0,0,63,128,0,0],vec![])] {
        let mut bytes=vec![];wire::uint(&mut bytes,kind);bytes.extend(data);
        let mut out=Vec::new();crate::particles::particle(&mut wire::Cursor::new(&bytes),&mut out,BASE,LOW,Default::default()).unwrap();
        let mut value=vec![];wire::uint(&mut value,target(kind));value.extend(expected);assert_eq!(out,value,"{kind}");
    }
}
fn ids_for_particle(kind:u32)->u32{crate::ids::translate(K::Particle,BASE,LOW,kind).unwrap()}
#[test]
fn adjacent770_inventory_alias_and_main_hand_roundtrip_without_new_gameplay(){
    let upper=V::V1_21_6;
    let frame=|from,name:&str,payload:&[u8]| {
        let id=(0..256).find_map(|id|crate::packet_layouts::packet_info(K::PlayClientbound,BASE,from,id).filter(|r|r.name==name).map(|r|r.target_id.unwrap())).unwrap();
        let mut bytes=Vec::new();wire::uint(&mut bytes,id);bytes.extend(payload);bytes
    };
    let stack=items::Stack{item:crate::generated_payload_770::INPUT_ITEMS[0].1,count:3,added:vec![],removed:vec![]};
    let source=frame(upper,"set_cursor_item",&items::encode(upper,Some(&stack),false,Default::default()).unwrap());
    let lower=super::translate_play(crate::Direction::Clientbound,upper,&source,Context::default()).unwrap();
    assert_eq!(super::translate_play(crate::Direction::Clientbound,LOW,&lower,Context::default()).unwrap(),source);
    let player=crate::entity_layouts::PLAYER;
    let index=crate::generated_payload_772::LAYOUTS.iter().find(|r|r.0==player).unwrap().1.iter().position(|n|*n=="player_main_hand").unwrap() as u8;
    let source=frame(upper,"entity_metadata",&[42,index,0,1,255]);
    let context=Context{entity_kind:Some(player),..Default::default()};
    let lower=super::translate_play(crate::Direction::Clientbound,upper,&source,context).unwrap();
    assert_eq!(super::translate_play(crate::Direction::Clientbound,LOW,&lower,context).unwrap(),source);
}

#[test]
fn lava_chicken_disc_has_explicit_view_and_never_reconstructs_from_substitute(){
    let data:serde_json::Value=serde_json::from_str(include_str!("../../data/items.json")).unwrap();
    let id=|name:&str|data.as_array().unwrap().iter().find(|r|r["name"]==name).unwrap()["id"].as_u64().unwrap() as u32;
    let codec=ContentCodec::default();
    let view=codec.map(K::Item,IdSpace::Canonical,IdSpace::Wire(LOW),id("music_disc_lava_chicken")).unwrap();
    assert_eq!(codec.map(K::Item,IdSpace::Wire(LOW),IdSpace::Canonical,view),Ok(id("music_disc_13")));
    assert_ne!(view,codec.map(K::Item,IdSpace::Canonical,IdSpace::Wire(V::V1_21_8),id("music_disc_lava_chicken")).unwrap());
}

#[test]
fn attributes770_drop_only_display_fields_and_reject_waypoints(){
    let mut data=vec![1,3,1,b'x'];data.extend(2f64.to_be_bytes());data.extend([0,1,2,8,0,1,b'X']);
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:16,data}],removed:vec![]};
    let encoded=items::encode(LOW,Some(&stack),false,Default::default()).unwrap();
    let decoded=items::decode(LOW,&encoded,false,Default::default()).unwrap().0.unwrap();
    assert_eq!(decoded.added[0].data.last(),Some(&0));
    for n in 0..encoded.len(){assert!(items::decode(LOW,&encoded[..n],false,Default::default()).is_err());}
    assert_eq!(crate::ids::translate(K::Attribute,BASE,LOW,32),Ok(32));
    assert!(crate::ids::translate(K::Attribute,BASE,LOW,33).is_err());
}

#[test]
fn equippable770_consumes_shearing_tail_without_copying_unknown_sound_ids(){
    let mut data=vec![0,0,1,b'x',0,0,0,0,1,1,1,1,0,0,1,b'x',0];
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:32,data:data.clone()}],removed:vec![]};
    let encoded=items::encode(LOW,Some(&stack),false,Default::default()).unwrap();
    assert_eq!(&encoded[5..],&data[..12]);
    assert!(items::decode(LOW,&encoded,false,Default::default()).is_ok());
    for n in 0..encoded.len(){assert!(items::decode(LOW,&encoded[..n],false,Default::default()).is_err());}
    data[1]=1;let invalid=items::Stack{added:vec![items::Component{kind:32,data}],..stack};
    assert!(items::encode(LOW,Some(&invalid),false,Default::default()).is_err());
}

#[test]
fn command_tree770_accepts_core_parsers_and_checks_every_boundary(){
    let source=packet("declare_commands",&[3,0,1,1,1,1,2,1,b'x',6,0,1,b'y',5,2,0]);
    assert!(crate::client_770::clientbound(&mut Tracker::default(),&source,Context::default()).is_ok());
    for n in 0..source.len(){assert!(crate::client_770::clientbound(&mut Tracker::default(),&source[..n],Context::default()).is_err());}
    let mut invalid=source.clone();invalid.push(0);assert!(crate::client_770::clientbound(&mut Tracker::default(),&invalid,Context::default()).is_err());
}

#[test]
fn painting770_removes_direction_metadata_and_shifts_the_variant_index(){
    let kind=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0=="painting").unwrap() as u32;
    let context=Context{entity_kind:Some(kind),metadata_registry_counts:&[("minecraft:painting_variant",51)],..Default::default()};
    let source=packet("entity_metadata",&[9,8,12,2,9,34,25,255]);
    let bytes=wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&source,context,true).unwrap().into_owned();
    assert_eq!(&bytes[1..],&[9,8,30,25,255]);
    for n in 0..source.len(){assert!(wire::translate_inner(crate::State::Play,crate::Direction::Clientbound,BASE,LOW,&source[..n],context,true).is_err());}
}

#[test]
fn configuration770_does_not_tunnel_unavailable_password_dialogs(){
    for body in [&[0x11][..],&[0x12,10,0][..]] {
        assert!(crate::client_770::configuration(body).is_err());
    }
}

#[test]
fn limbo_configuration_reset_is_empty_and_directional() {
    let body=packet("start_configuration", &[]);
    let mapped=super::clientbound(&mut Tracker::default(), &body, Context::default()).unwrap().into_owned();
    let mut tail=body.clone();tail.push(0);
    assert!(super::clientbound(&mut Tracker::default(), &tail, Context::default()).is_err());
    assert!(!mapped.is_empty());
    let id=(0..256).find(|&id|crate::packet_layouts::packet_info(K::PlayServerbound,LOW,BASE,id).is_some_and(|r|r.name=="configuration_acknowledged")).unwrap();
    let mut ack=Vec::new();wire::uint(&mut ack,id as u32);
    assert_eq!(super::serverbound(&ack,Context::default()).unwrap().as_ref(), &[0x10]);
    ack.push(0);assert!(super::serverbound(&ack,Context::default()).is_err());
}
