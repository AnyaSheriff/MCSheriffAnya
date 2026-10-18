use super::*;
use crate::{Direction,State,wire};
use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
fn bytes(hex: &str) -> Vec<u8> { hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect() }
fn records() -> serde_json::Value { serde_json::from_str(include_str!("../testdata/particle-observations.json")).unwrap() }
fn translate(body: &[u8], from: JavaVersion, to: JavaVersion) -> Result<Vec<u8>,Error> {
    wire::translate(State::Play,Direction::Clientbound,from,to,body).map(|v|v.into_owned())
}
#[test]
fn official_world_particles_match_exactly_both_ways() {
    let data=records();
    assert_eq!(data["old"]["particles"].as_array().unwrap().len(),23);
    for (old,new) in data["old"]["particles"].as_array().unwrap().iter().zip(data["new"]["particles"].as_array().unwrap()) {
        assert_eq!(old["case"],new["case"]);
        let a=bytes(old["body"].as_str().unwrap()); let b=bytes(new["body"].as_str().unwrap());
        assert_eq!(translate(&a,OLD,NEW).unwrap(),b,"{}",old["case"]);
        assert_eq!(translate(&b,NEW,OLD).unwrap(),a,"{}",old["case"]);
    }
}
#[test]
fn observed_metadata_particles_preserve_colors_and_list_members() {
    let data=records();
    for (old,new) in data["old"]["metadata"].as_array().unwrap().iter().zip(data["new"]["metadata"].as_array().unwrap()) {
        let kind=crate::entity_layouts::LAYOUTS.iter().position(|e|e.0==old["type"].as_str().unwrap()).unwrap() as u32;
        let context=wire::Context {entity_kind:Some(kind),..Default::default()};
        let a=bytes(old["packets"].as_array().unwrap().last().unwrap()["body"].as_str().unwrap());
        let b=bytes(new["packets"].as_array().unwrap().last().unwrap()["body"].as_str().unwrap());
        for (from,to,source,target) in [(OLD,NEW,&a,&b),(NEW,OLD,&b,&a)] {
            let result=wire::translate_with_context(State::Play,Direction::Clientbound,from,to,source,context).unwrap();
            if kind == crate::entity_layouts::LAYOUTS.iter().position(|e|e.0=="cow").unwrap() as u32 {
                // Порядок двух эффектов различается между независимыми мирами.
                // Сортируем только наблюдение для сравнения; перевод порядок сохраняет.
                let normalize=|body: &[u8]| {
                    assert_eq!(&body[2..5],&[10,17,2]); assert_eq!(body.last(),Some(&255));
                    let mut particles=body[5..body.len()-1].as_chunks::<5>().0.iter().map(|v|v.to_vec()).collect::<Vec<_>>();
                    assert_eq!(particles.len(),2); particles.sort(); (body[..5].to_vec(),particles)
                };
                assert_eq!(normalize(&result),normalize(target));
                assert_eq!(wire::translate_with_context(State::Play,Direction::Clientbound,to,from,&result,context).unwrap().as_ref(),source);
            } else { assert_eq!(result.as_ref(),target); }
        }
    }
}
#[test]
fn observed_packets_reject_every_truncation_and_trailing_bytes() {
    let data=records();
    for (name,from,to) in [("old",OLD,NEW),("new",NEW,OLD)] {
        for entry in data[name]["particles"].as_array().unwrap() {
            let body=bytes(entry["body"].as_str().unwrap());
            for n in 0..body.len() { assert!(translate(&body[..n],from,to).is_err(),"{} prefix{n}",entry["case"]); }
            let mut extra=body; extra.push(0); assert!(translate(&extra,from,to).is_err());
        }
    }
}
#[test]
fn cannot_silently_downgrade_new_particles_or_randomization() {
    let data=records(); let base=bytes(data["new"]["particles"][0]["body"].as_str().unwrap());
    for id in [4,5,6,7,8,9,10,43,44,45,118,119,127,128] {
        let mut body=base.clone(); body[1]=id; assert!(translate(&body,NEW,OLD).is_err());
    }
    for mode in [1,2,3] { let mut body=base.clone(); *body.last_mut().unwrap()=mode; assert!(translate(&body,NEW,OLD).is_err()); }
    let mut axes=base.clone(); let pos=axes.len()-10; axes[pos..pos+4].copy_from_slice(&0.25f32.to_be_bytes());
    assert!(translate(&axes,NEW,OLD).is_err());
}
#[test]
fn malformed_numeric_fields_and_particle_lists_are_bounded() {
    let data=records(); let base=bytes(data["old"]["particles"][0]["body"].as_str().unwrap());
    for (pos,value) in [(3,f64::NAN.to_be_bytes().to_vec()),(27,f32::INFINITY.to_be_bytes().to_vec()),(39,f32::NAN.to_be_bytes().to_vec()),(43,(-1i32).to_be_bytes().to_vec())] {
        let mut body=base.clone(); body[pos..pos+value.len()].copy_from_slice(&value); assert!(translate(&body,OLD,NEW).is_err());
    }
    let mut invalid=base; invalid[1]=2; assert!(translate(&invalid,OLD,NEW).is_err());
    let context=wire::Context {entity_kind:Some(crate::entity_layouts::PLAYER),..Default::default()};
    let mut body=vec![0x63,1,10,17]; uint(&mut body,1025); body.push(255);
    assert!(wire::translate_with_context(State::Play,Direction::Clientbound,OLD,NEW,&body,context).is_err());
}
#[test]
fn item_template_has_id_before_count_and_reuses_component_validation() {
    let body=[0xe3,1,5,0,0]; // реальный775 red_wool x5, не Slot count/id
    let (stack,size)=items::decode_template(OLD,&body,items::Context::default()).unwrap();
    assert_eq!((stack.item,stack.count,size),(227,5,5));
    assert_eq!(items::encode_template(NEW,&stack,items::Context::default()).unwrap(),[0x88,2,5,0,0]);
    for bad in [vec![0,1,0,0],vec![1,0,0,0],vec![1,128,1,0,0],vec![1,1,1,0,127],vec![1,1,1,1,4,4]] {
        assert!(items::decode_template(OLD,&bad,items::Context::default()).is_err());
    }
}
#[test]
fn entity_vibration_form_and_empty_list_follow_pinned_schema() {
    // [проверить] Entity vibration пока проверена только по JSON, не живым сервером.
    let mut source=vec![48,1,37]; source.extend(1.5f32.to_be_bytes()); source.push(10);
    let mut out=Vec::new(); particle(&mut Cursor::new(&source),&mut out,OLD,NEW,items::Context::default()).unwrap();
    let mut expected=source.clone(); expected[0]=58; assert_eq!(out,expected);
    let mut bad=source; bad[1]=2;
    assert!(particle(&mut Cursor::new(&bad),&mut Vec::new(),OLD,NEW,items::Context::default()).is_err());
    let context=wire::Context {entity_kind:Some(crate::entity_layouts::PLAYER),..Default::default()};
    assert_eq!(wire::translate_with_context(State::Play,Direction::Clientbound,OLD,NEW,&[0x63,1,10,17,0,255],context).unwrap().as_ref(),[0x65,1,10,17,0,255]);
}
