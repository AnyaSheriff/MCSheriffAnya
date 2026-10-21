use super::*;
use JavaVersion::{V26_1_2 as OLD,V26_2 as MID,V26_3 as NEW};

fn packet(status:u32,face:u8,sequence:u32)->Vec<u8>{
    let mut out=vec![0x29];uint(&mut out,status);
    out.extend(0x0000008000000041u64.to_be_bytes());out.push(face);uint(&mut out,sequence);out
}
fn sb(body:&[u8],from:JavaVersion,to:JavaVersion)->Result<Cow<'_,[u8]>,Error>{
    translate(State::Play,Direction::Serverbound,from,to,body)
}
fn hex(value:&str)->Vec<u8>{
    value.as_bytes().as_chunks::<2>().0.iter().map(|c|u8::from_str_radix(std::str::from_utf8(c).unwrap(),16).unwrap()).collect()
}

#[test]
fn actual_777_start_finish_payloads_map_to_native_mining_without_tool_drop(){
    let fixture:serde_json::Value=serde_json::from_str(include_str!("../testdata/actual777-dig-observations.json")).unwrap();
    assert_eq!(fixture["protocol"],777);
    assert_eq!(fixture["source_sha256"],"2c9622a8ec5740665f4018175f54d8cf47d2ca88e6833b42f700d1bb6f151d21");
    for case in fixture["observations"].as_array().unwrap(){
        assert_eq!(case["air"],true);
        assert!(!case["drops"].as_array().unwrap().is_empty());
        for event in case["dig"].as_array().unwrap(){
            let mut raw=vec![0x29];raw.extend(hex(event["payload_hex"].as_str().unwrap()));
            let status=event["status"].as_u64().unwrap();
            assert!(status==0||status==3);
            let expected=packet(if status==3{2}else{0},event["face"].as_u64().unwrap() as u8,event["sequence"].as_u64().unwrap() as u32);
            let mapped=sb(&raw,NEW,OLD).unwrap();
            assert_eq!(mapped.as_ref(),expected);
            assert_eq!(sb(&mapped,OLD,NEW).unwrap().as_ref(),raw);
            if status==3{assert_ne!(mapped[1],3,"native3 is DROP_STACK, not FINISH");}
        }
    }
}

#[test]
fn unverified_cross_version_actions_are_denied_in_both_directions(){
    for (from,to,verified) in [(OLD,NEW,&[0,1,2,3,4,5,6,7][..]),(NEW,OLD,&[0,2,3,4,5,6,7,8][..])]{
        for status in (0..=255).chain([u32::MAX]){
            if verified.contains(&status){continue;}
            assert!(sb(&packet(status,1,17),from,to).is_err(),"unverified status {status}");
        }
    }
}

#[test]
fn typed_action_payload_requires_face_sequence_and_exact_end(){
    for (from,to,status) in [(OLD,NEW,0),(OLD,NEW,1),(OLD,NEW,2),(OLD,NEW,3),(OLD,NEW,4),(OLD,NEW,5),(OLD,NEW,6),(OLD,NEW,7),
                             (NEW,OLD,0),(NEW,OLD,2),(NEW,OLD,3),(NEW,OLD,4),(NEW,OLD,5),(NEW,OLD,6),(NEW,OLD,7),(NEW,OLD,8)]{
        let valid=packet(status,1,300);
        for end in 0..valid.len(){assert!(sb(&valid[..end],from,to).is_err(),"truncated at {end}");}
        for face in [6,127,255]{assert!(sb(&packet(status,face,300),from,to).is_err());}
        let mut extra=valid.clone();extra.push(0);assert!(sb(&extra,from,to).is_err());
        let mut negative=packet(status,1,0);negative.pop();negative.extend([255,255,255,255,15]);
        assert!(sb(&negative,from,to).is_err());
    }
}

#[test]
fn native_775_passthrough_and_unverified_776_denial_are_unchanged(){
    for status in [0,1,2,3,4,5,6,7,u32::MAX]{
        let body=packet(status,255,0);
        assert!(matches!(sb(&body,OLD,OLD),Ok(Cow::Borrowed(original)) if std::ptr::eq(original.as_ptr(),body.as_ptr())));
    }
    let body=packet(3,1,4);
    assert!(matches!(sb(&body,MID,OLD),Err(Error::UnsupportedVersion(MID))));
    assert!(super::translate_inner(State::Play,Direction::Serverbound,MID,OLD,&body,Context::default(),true).is_err());
}

#[test]
fn actual_777_cancel_drop_swap_map_selected_payloads_without_guessing_other_actions(){
    let fixture:serde_json::Value=serde_json::from_str(include_str!("../testdata/actual777-player-action-observations.json")).unwrap();
    assert_eq!(fixture["protocol"],777);
    assert_eq!(fixture["source_sha256"],"51ec98869afd72c16b4003044288cc71db71ba5985b17f72497041f1fcff12cf");
    assert_eq!(fixture["statuses"],serde_json::json!({"start":0,"finish":3,"cancel":2,"drop_one":5,"drop_stack":4,"swap":7}));
    assert_eq!(fixture["visual_review_status"],"PASS");
    assert_eq!(fixture["observations"].as_array().unwrap().len(),4);
    for case in fixture["observations"].as_array().unwrap(){
        assert_eq!(case["positive_console_predicates"].as_array().unwrap().len(),2);
        match case["case"].as_str().unwrap(){
            "cancel"=>{assert_eq!(case["target_retained"],true);assert!(case["drops"].as_array().unwrap().is_empty());},
            "drop-one"=>{assert_eq!(case["drops"][0]["slot"],serde_json::json!({"item":1,"count":1}));assert_eq!(case["server_hands"]["mainhand"],serde_json::json!({"item":1,"count":6}));},
            "drop-stack"=>{assert_eq!(case["drops"][0]["slot"],serde_json::json!({"item":62,"count":11}));assert!(case["server_hands"]["mainhand"].is_null());},
            "swap"=>{assert!(case["drops"].as_array().unwrap().is_empty());assert_eq!(case["server_hands"]["mainhand"],serde_json::json!({"item":1022,"count":5}));assert_eq!(case["server_hands"]["offhand"],serde_json::json!({"item":1018,"count":3}));},
            other=>panic!("unverified fixture case {other}"),
        }
        for event in case["actions"].as_array().unwrap(){
            let mut raw=vec![0x29];raw.extend(hex(event["payload_hex"].as_str().unwrap()));
            let status=event["status"].as_u64().unwrap();
            let native=match status{0=>0,2=>1,4=>3,5=>4,7=>6,_=>panic!("unverified action {status}")};
            let mapped=sb(&raw,NEW,OLD).unwrap();
            let mut expected=raw.clone();expected[1]=native;
            assert_eq!(mapped.as_ref(),expected);
            assert_eq!(sb(&mapped,OLD,NEW).unwrap().as_ref(),raw);
        }
    }
}

#[test]
fn actual_777_bow_release_and_spear_jab_map_only_observed_payloads(){
    let fixture:serde_json::Value=serde_json::from_str(include_str!("../testdata/actual777-use-action-observations.json")).unwrap();
    assert_eq!(fixture["protocol"],777);
    assert_eq!(fixture["source_sha256"],"1a647a569081c25b36c9c32a81f14bda1cd400dbf712fb852bc9616fa7abcf23");
    assert_eq!(fixture["statuses"],serde_json::json!({"release_use":6,"jab":8}));
    assert_eq!(fixture["visual_review_status"],"PASS");
    assert_eq!(fixture["observations"].as_array().unwrap().len(),2);
    for case in fixture["observations"].as_array().unwrap(){
        assert_eq!(case["positive_console_predicates"].as_array().unwrap().len(),2);
        let event=&case["action"];
        let (status,native)=match case["case"].as_str().unwrap(){
            "bow"=>{
                assert_eq!(case["use_item"].as_array().unwrap().len(),1);
                assert_eq!(case["use_item"][0]["hand"],0);
                assert_eq!(case["acknowledgements"].as_array().unwrap().len(),1);
                assert_eq!(case["acknowledgements"][0]["sequence"],case["use_item"][0]["sequence"]);
                let use_time=case["use_item"][0]["elapsed"].as_f64().unwrap();
                let action_time=event["elapsed"].as_f64().unwrap();
                assert!(action_time-use_time>=1.2);
                assert!(action_time>=case["input"]["release_requested"].as_f64().unwrap());
                assert_eq!(case["arrow_spawns"].as_array().unwrap().len(),1);
                assert_eq!(case["arrow_spawns"][0]["entity_type"],fixture["fixtures"]["arrow_entity"]);
                assert_eq!(case["hand_updates"][0]["mainhand"],serde_json::json!({"item":1008,"count":1,"damage":1}));
                assert_eq!(case["hand_updates"][1]["offhand"],serde_json::json!({"item":1009,"count":2}));
                (6,5)
            },
            "spear"=>{
                assert!(case["use_item"].as_array().unwrap().is_empty());
                assert!(case["acknowledgements"].as_array().unwrap().is_empty());
                assert!(case["arrow_spawns"].as_array().unwrap().is_empty());
                assert!(case["displacement"]["delta_x"].as_f64().unwrap()>0.3);
                assert_eq!(case["hand_updates"][0]["mainhand"]["damage"],1);
                assert_eq!(case["hand_updates"][0]["mainhand"]["item"],1450);
                (8,7)
            },
            other=>panic!("unverified fixture case {other}"),
        };
        assert_eq!(event["status"],status);
        assert_eq!(event["target"],serde_json::json!([0,0,0]));
        assert_eq!(event["face"],0);assert_eq!(event["sequence"],0);
        let mut raw=vec![0x29];raw.extend(hex(event["payload_hex"].as_str().unwrap()));
        let mut expected=raw.clone();expected[1]=native;
        let mapped=sb(&raw,NEW,OLD).unwrap();
        assert_eq!(mapped.as_ref(),expected);
        assert_eq!(sb(&mapped,OLD,NEW).unwrap().as_ref(),raw);
    }
    // Неизвестная777единица не выводится из соседних enum-значений.
    assert!(sb(&packet(1,0,0),NEW,OLD).is_err());
}
