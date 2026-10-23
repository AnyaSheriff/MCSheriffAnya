use super::*;
use crate::wire;
use crate::{ids::{self,RegistryKind as K}, items::{Component,Stack}};
use JavaVersion::{V1_21_9 as LOWER,V1_21_11 as UPPER};
fn packet(direction: Direction, version: JavaVersion, name: &str, payload: &[u8]) -> Vec<u8> {
    let to = if version == LOWER { UPPER } else { LOWER };
    let kind = if direction == Direction::Clientbound {K::PlayClientbound} else {K::PlayServerbound};
    let id = (0..256).find(|&id| crate::packet_layouts::packet_info(kind,version,to,id).is_some_and(|r| r.name == name)).unwrap();
    let mut out=Vec::new();uint(&mut out,id as u32);out.extend(payload);out
}
fn roundtrip(direction:Direction,name:&str,payload:&[u8]) {
    let body=packet(direction,LOWER,name,payload);
    let mapped=translate_play(direction,LOWER,&body,Context::default()).unwrap();
    assert_eq!(translate_play(direction,UPPER,&mapped,Context::default()).unwrap(),body,"{name}");
    for (from,bytes) in [(LOWER,&body),(UPPER,&mapped)] {
        for end in 0..bytes.len(){assert!(translate_play(direction,from,&bytes[..end],Context::default()).is_err(),"{name}: {end}");}
        let mut tail=bytes.clone();tail.push(0);assert!(translate_play(direction,from,&tail,Context::default()).is_err(),"{name}: tail");
    }
}
#[test]
fn inventory_slots_map_real_component_ids_and_bound_each_envelope() {
    // damage: canonical3; custom_name: canonical6→773/5. Не копия mapper ID774.
    let stack=Stack{item:1,count:7,added:vec![Component{kind:3,data:vec![2]},Component{kind:6,data:vec![8,0,1,b'x']}],removed:vec![19]};
    let lower=items::encode(LOWER,Some(&stack),false,Default::default()).unwrap();
    let upper=items::encode(UPPER,Some(&stack),false,Default::default()).unwrap();
    assert_ne!(lower,upper);
    assert_eq!(lower,[7,1,2,1,3,2,5,8,0,1,b'x',16]);
    assert_eq!(items::decode(LOWER,&lower,false,Default::default()).unwrap(),(Some(stack.clone()),lower.len()));
    for (name,mut prefix) in [("set_slot",vec![0,1,0,36]),("set_cursor_item",vec![]),("set_player_inventory",vec![36]),("entity_equipment",vec![42,0])] {
        prefix.extend(&lower);roundtrip(Direction::Clientbound,name,&prefix);
    }
    let mut window=vec![0,1,2];window.extend(&lower);window.push(0);window.extend(&lower);
    roundtrip(Direction::Clientbound,"window_items",&window);
    let mut creative=vec![0,36];creative.extend(items::encode(LOWER,Some(&stack),true,Default::default()).unwrap());
    roundtrip(Direction::Serverbound,"set_creative_slot",&creative);
    let template=items::encode_template(LOWER,&stack,Default::default()).unwrap();assert_eq!(template,lower);
    assert_eq!(items::decode_template(LOWER,&template,Default::default()).unwrap(),(stack,template.len()));
}
#[test]
fn predictions_remap_component_ids_and_reject_invalid_clicks() {
    let item=ids::translate(K::Item,wire::INTERNAL,LOWER,1).unwrap();
    let mut payload=vec![0,1,0,36,0,0,1,0,36,1];uint(&mut payload,item);payload.extend([1,1,5,0,0,0,42,0,0]);
    roundtrip(Direction::Serverbound,"window_click",&payload);
    let mut bad=payload;bad[5]=7;
    assert!(translate_play(Direction::Serverbound,LOWER,&packet(Direction::Serverbound,LOWER,"window_click",&bad),Context::default()).is_err());
    for value in [0,1,2] {roundtrip(Direction::Serverbound,"enchant_item",&[3,value]);}
    for value in [3,127,255] {
        for from in [LOWER,UPPER] {assert!(translate_play(Direction::Serverbound,from,&packet(Direction::Serverbound,from,"enchant_item",&[3,value]),Context::default()).is_err());}
    }
}
#[test]
fn recipes_and_item_particles_use_count_first_nested_templates() {
    // stonecutter ghost: input item1, result7×item1, station item1.
    let mut recipe=vec![0,3,2,1,3,7,1,0,0,2,1];
    roundtrip(Direction::Clientbound,"craft_recipe_response",&recipe);
    recipe=vec![0,0];roundtrip(Direction::Clientbound,"declare_recipes",&recipe);
    recipe=vec![0,0];roundtrip(Direction::Clientbound,"recipe_book_add",&recipe);
    let mut particle=vec![0,0];particle.extend([0;24+12+4]);particle.extend(1i32.to_be_bytes());
    uint(&mut particle,ids::translate(K::Particle,wire::INTERNAL,LOWER,47).unwrap());particle.extend([7,1,0,0]);
    roundtrip(Direction::Clientbound,"world_particles",&particle);
    let mut signed=vec![1,b'x'];signed.extend([0;16]);signed.extend([0,0,0,0,0,255]);
    roundtrip(Direction::Serverbound,"chat_command_signed",&signed);
}
#[test]
fn nested_official_fixture_items_preserve_canonical_stacks() {
    let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/nested-item-observations.json")).unwrap();
    for row in data["old"]["cases"].as_array().unwrap() {
        let raw:Vec<u8>=row["slot_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter().map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
        let (stack,n)=items::decode(wire::INTERNAL,&raw,false,Default::default()).unwrap();assert_eq!(n,raw.len());
        let bytes=items::encode(LOWER,stack.as_ref(),false,Default::default()).unwrap();
        assert_eq!(items::decode(LOWER,&bytes,false,Default::default()).unwrap(),(stack,bytes.len()),"{}",row["name"]);
        roundtrip(Direction::Clientbound,"set_cursor_item",&bytes);
    }
}
#[test]
fn unsupported_content_and_routes_still_fail_closed() {
    for kind in [5,7,8,30,38,39,40] {
        let stack=Stack{item:1,count:1,added:vec![Component{kind,data:vec![0]}],removed:vec![]};
        assert!(items::encode(LOWER,Some(&stack),false,Default::default()).is_err());
        let removed=Stack{added:vec![],removed:vec![kind],..stack};
        assert_eq!(items::encode(LOWER,Some(&removed),false,Default::default()).unwrap(),[1,1,0,0]);
    }
    for name in ["map_chunk","entity_metadata","spawn_entity","custom_payload"] {
        assert!(translate_play(Direction::Clientbound,LOWER,&packet(Direction::Clientbound,LOWER,name,&[0;32]),Context::default()).is_err());
    }
    for primary in [JavaVersion::V26_1_2,JavaVersion::V26_3] {assert!(crate::Negotiated::native_only(773,primary).is_err());}
    for from in [JavaVersion::V1_21_5,JavaVersion::V1_21_6,JavaVersion::V1_21_8] {
        assert!(translate_play(Direction::Clientbound,from,&[0],Context::default()).is_err());
        assert!(items::decode(from,&[0],false,Default::default()).is_err());
    }
}
#[test]
fn metadata_uses_separate_layouts_and_serializer_ids() {
    let item=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0=="item").unwrap() as u32;
    let field=crate::generated_payload_774::LAYOUTS.iter().find(|r|r.0==item).unwrap().1.iter().position(|&s|s=="item").unwrap() as u8;
    let stack=Stack{item:1,count:7,added:vec![Component{kind:6,data:vec![8,0,1,b'x']}],removed:vec![]};
    let mut payload=vec![42,field,7];payload.extend(items::encode(UPPER,Some(&stack),false,Default::default()).unwrap());payload.push(255);
    let body=packet(Direction::Clientbound,UPPER,"entity_metadata",&payload);
    let context=Context{entity_kind:Some(item),..Default::default()};
    let lower=translate_play(Direction::Clientbound,UPPER,&body,context).unwrap();
    assert_ne!(body,lower);
    assert_eq!(translate_play(Direction::Clientbound,LOWER,&lower,context).unwrap(),body);
    for (from,bytes) in [(LOWER,&lower),(UPPER,&body)] {
        for n in 0..bytes.len(){assert!(translate_play(Direction::Clientbound,from,&bytes[..n],context).is_err());}
        let mut tail=bytes.clone();tail.push(0);assert!(translate_play(Direction::Clientbound,from,&tail,context).is_err());
    }
    assert!(translate_play(Direction::Clientbound,UPPER,&body,Context::default()).is_err());
    let mannequin=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0=="mannequin").unwrap() as u32;
    let context=Context{entity_kind:Some(mannequin),..Default::default()};
    let body=packet(Direction::Clientbound,UPPER,"entity_metadata",&[42,17,37,0,0,0,0,0,0,0,0,255]);
    let lower=translate_play(Direction::Clientbound,UPPER,&body,context).unwrap();
    assert_eq!(lower[3],36);
    assert_eq!(translate_play(Direction::Clientbound,LOWER,&lower,context).unwrap(),body);
    for &(kind,_) in crate::generated_payload_773::LAYOUTS {
        let context=Context{entity_kind:Some(kind),..Default::default()};
        let body=packet(Direction::Clientbound,UPPER,"entity_metadata",&[42,255]);
        let lower=translate_play(Direction::Clientbound,UPPER,&body,context).unwrap();
        assert_eq!(translate_play(Direction::Clientbound,LOWER,&lower,context).unwrap(),body);
    }
}
#[test]
fn anger_duration_uses_explicit_snapshot_clock_and_correct_serializers() {
    for name in ["bee","wolf"] {
        let kind=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0==name).unwrap() as u32;
        let index=crate::generated_payload_773::LAYOUTS.iter().find(|r|r.0==kind).unwrap().1.iter().position(|&f|f=="remaining_anger_time").unwrap() as u8;
        let lower=packet(Direction::Clientbound,LOWER,"entity_metadata",&[42,index,1,40,255]);
        let context=Context{entity_kind:Some(kind),entity_clock:Some(1000),..Default::default()};
        let upper=translate_play(Direction::Clientbound,LOWER,&lower,context).unwrap();
        assert_eq!(translate_play(Direction::Clientbound,UPPER,&upper,context).unwrap(),&lower[..]);
        assert!(translate_play(Direction::Clientbound,LOWER,&lower,Context{entity_kind:Some(kind),..Default::default()}).is_err());
        assert!(translate_play(Direction::Clientbound,UPPER,&upper,Context{entity_kind:Some(kind),..Default::default()}).is_err());
        for end in 0..lower.len(){assert!(translate_play(Direction::Clientbound,LOWER,&lower[..end],context).is_err());}
    }
}
#[test]
fn malformed_slots_and_unsupported_routes_are_refused() {
    for untrusted in [false,true] {
        for raw in [vec![1,1,2,0,3,0,3,0],vec![1,1,1,1,3,0,3],vec![1,1,1,0,127,0],vec![128,1,1,0,0],vec![1,0,0,0]] {
            assert!(items::decode(LOWER,&raw,untrusted,Default::default()).is_err());
        }
    }
    let stack=Stack{item:1,count:1,added:vec![Component{kind:77,data:vec![0]}],removed:vec![]};
    assert!(items::encode(LOWER,Some(&stack),false,Default::default()).is_ok());
    let frame=packet(Direction::Clientbound,LOWER,"set_cursor_item",&[0]);
    let context=Context{items:items::Context{content:crate::content::ContentCodec::white_slab_preview(),..Default::default()},..Default::default()};
    assert!(translate_play(Direction::Clientbound,LOWER,&frame,context).is_err());
    for step in crate::adjacent::STEPS {
        let result=step.translate_play(Direction::Clientbound,LOWER,&frame,Context::default());
        assert_eq!(result.is_ok(),step==crate::adjacent::STEPS[0]);
    }
}
#[test]
fn recorded_complex_items_and_corrected_attributes_translate() {
    let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
    let mut accepted=0;
    for row in data["old"]["cases"].as_array().unwrap() {
        let raw:Vec<u8>=row["slot_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter().map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
        let (stack,n)=items::decode(wire::INTERNAL,&raw,false,Default::default()).unwrap();assert_eq!(n,raw.len());
        let name=row["name"].as_str().unwrap();
        for untrusted in [false,true] {
            let bytes=items::encode(LOWER,stack.as_ref(),untrusted,Default::default()).unwrap_or_else(|error|panic!("{name}: {error:?}"));
            assert_eq!(items::decode(LOWER,&bytes,untrusted,Default::default()).unwrap(),(stack.clone(),bytes.len()),"{name}");
            for n in 0..bytes.len(){assert!(items::decode(LOWER,&bytes[..n],untrusted,Default::default()).is_err(),"{name}: {n}");}
            if !untrusted {roundtrip(Direction::Clientbound,"set_cursor_item",&bytes);}
        }
        accepted+=1;
    }
    assert_eq!(accepted,54);
}

#[test]
fn explicit_773_content_views_preserve_the_canonical_source() {
    use crate::content::{ContentCodec,IdSpace};
    use crate::ids::RegistryKind;
    let codec=ContentCodec::default();
    for &(item,target) in crate::generated_payload_773::ITEM_VIEWS {
        let source=Stack{item,count:3,added:vec![],removed:vec![]};let before=source.clone();
        let wire=items::encode(LOWER,Some(&source),false,Default::default()).unwrap();
        assert_eq!(wire[0],3);
        assert_eq!(codec.map(RegistryKind::Item,IdSpace::Canonical,IdSpace::Wire(LOWER),item).unwrap(),target);
        let (view,n)=items::decode(LOWER,&wire,false,Default::default()).unwrap();assert_eq!(n,wire.len());
        assert_eq!(view.unwrap().item,crate::ids::translate(RegistryKind::Item,LOWER,wire::INTERNAL,target).unwrap());
        assert_eq!(source,before);
    }
    assert!(codec.map(RegistryKind::Item,IdSpace::Canonical,IdSpace::Wire(LOWER),u32::MAX).is_err());
}

#[test]
fn canonical_773_tracker_uses_explicit_entity_views_and_keeps_metadata_identity() {
    for &(kind,_,target) in crate::generated_payload_773::ENTITY_VIEWS {
        let mut tracker=crate::entities::Tracker::default();
        let mut spawn=vec![1,42];spawn.extend([4;16]);wire::uint(&mut spawn,kind);
        for value in [1.5f64,64.0,2.5]{spawn.extend(value.to_be_bytes());}spawn.extend([0;5]);
        let mapped=super::clientbound(&mut tracker,&spawn,Context::default()).unwrap();
        let mut input=wire::Cursor::new(&mapped);input.uint().unwrap();assert_eq!(input.uint().unwrap(),42);input.take(16).unwrap();assert_eq!(input.uint().unwrap(),target);
        let metadata=[0x63,42,0,0,1,255];
        let mapped=super::clientbound(&mut tracker,&metadata,Context::default()).unwrap();
        assert_eq!(&mapped[1..],&metadata[1..]);
        for end in 0..spawn.len(){assert!(super::clientbound(&mut tracker,&spawn[..end],Context::default()).is_err());}
    }
}

#[test]
fn main_hand_translates_byte_to_humanoid_arm_without_changing_its_value() {
    for hand in 0..=1 {
        let context=Context{entity_kind:Some(crate::entity_layouts::PLAYER),..Default::default()};
        let canonical=[0x63,42,15,42,hand,255];
        let lower=crate::wire::translate_inner(crate::State::Play,Direction::Clientbound,wire::INTERNAL,LOWER,&canonical,context,true).unwrap();
        assert_eq!(&lower[1..],&[42,15,0,hand,255]);
        let upper=translate_play(Direction::Clientbound,LOWER,&lower,context).unwrap();
        assert_eq!(&upper[1..],&[42,15,38,hand,255]);
        assert_eq!(translate_play(Direction::Clientbound,UPPER,&upper,context).unwrap(),&lower[..]);
        let mut invalid=canonical;invalid[4]=2;
        assert!(crate::wire::translate_inner(crate::State::Play,Direction::Clientbound,wire::INTERNAL,LOWER,&invalid,context,true).is_err());
    }
}
