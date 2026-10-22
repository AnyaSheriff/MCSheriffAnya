use super::*;
use crate::wire::uint;

fn item_spawn(id: u32, kind: u32) -> Vec<u8> {
    // Synthetic775 packet: own writer + pinned schema, not captured776 item.
    let mut body = vec![1]; uint(&mut body, id); body.extend([0;16]); uint(&mut body, kind);
    for coordinate in [0.5f64,64.0,0.5] { body.extend(coordinate.to_be_bytes()); }
    body.extend([0,0,0,0,0]); // LP zero, pitch/yaw/headYaw, objectData0
    body
}
fn joined() -> Tracker {
    let mut tracker = Tracker::default();
    clientbound(&mut tracker, include_bytes!("../testdata/play-login-26.1.2.bin"),
        Context { online_mode: Some(false), ..Default::default() }).unwrap();
    tracker
}
fn item_kind() -> u32 {
    crate::entity_layouts::LAYOUTS.iter().position(|entry| entry.0 == "item").unwrap() as u32
}

#[test]
fn bounded_776_item_lifecycle_tracks_spawn_metadata_move_collect_remove() {
    let mut tracker=joined();let spawn=item_spawn(2,item_kind());
    assert_eq!(clientbound(&mut tracker,&spawn,Context::default()).unwrap().as_ref(),spawn);
    // Observed Slot stone3 envelope composed at pinned item metadata8/serializer7.
    let metadata=[0x63,2,8,7,3,1,0,0,255];
    assert_eq!(clientbound(&mut tracker,&metadata,Context::default()).unwrap().as_ref(),metadata);
    let movement=[0x35,2,0,0,0,0,0,0,0];
    assert_eq!(clientbound(&mut tracker,&movement,Context::default()).unwrap().as_ref(),movement);
    let collect=[0x7c,2,1,1];
    assert_eq!(clientbound(&mut tracker,&collect,Context::default()).unwrap().as_ref(),collect);
    // Collect animation does not remove tracking until destroy.
    assert!(clientbound(&mut tracker,&metadata,Context::default()).is_ok());
    assert!(clientbound(&mut tracker,&[0x4d,1,2],Context::default()).is_ok());
    for body in [&metadata[..],&movement[..],&collect[..]] {
        assert!(clientbound(&mut tracker,body,Context::default()).is_err());
    }
}

#[test]
fn bounded_776_failed_spawns_never_create_tracking_and_mobs_stay_closed() {
    let spawn=item_spawn(2,item_kind());
    for end in 0..spawn.len() {
        let mut tracker=joined();
        assert!(clientbound(&mut tracker,&spawn[..end],Context::default()).is_err());
        assert!(clientbound(&mut tracker,&[0x63,2,8,7,0,255],Context::default()).is_err());
    }
    for kind in [30,155,u32::MAX] {
        assert!(clientbound(&mut joined(),&item_spawn(2,kind),Context::default()).is_err());
    }
    let mut tail=spawn;tail.push(0);
    assert!(clientbound(&mut joined(),&tail,Context::default()).is_err());
    // Direct internal dispatcher cannot bypass item-only spawn guard either.
    assert!(wire::translate_inner(State::Play,Direction::Clientbound,OLD,MID,
        &item_spawn(2,30),Context::default(),true).is_err());
}

#[test]
fn bounded_776_item_metadata_and_controls_reject_bad_context_payloads() {
    let mut tracker=joined();
    clientbound(&mut tracker,&item_spawn(2,item_kind()),Context::default()).unwrap();
    for body in [vec![0x63,2,8,8,1,255],vec![0x63,2,8,7,128,1,1,0,0,255],
        vec![0x63,2,8,7,1,1,0,0,8,7,1,1,0,0,255], vec![0x63,2,254,0,1,255],
        vec![0x7c,2,1,0],vec![0x7c,2,1,128,1],vec![0x7c,99,1,1],
        vec![0x7c,2,99,1],vec![0x7c,2,1,1,0],vec![0x35,1,0,0,0,0,0,0,0],
        vec![0x35,2,0,0,0,0,0,0,2],vec![0x4d,1,2,0]] {
        assert!(clientbound(&mut tracker,&body,Context::default()).is_err(),"{body:?}");
    }
    let metadata=[0x63,2,8,7,1,1,0,0,255];
    assert!(clientbound(&mut tracker,&metadata,Context::default()).is_ok());
    // Removed/unknown nested component ID never passes as opaque metadata Slot.
    let unknown=[0x63,2,8,7,1,1,1,0,255,127,255];
    assert!(clientbound(&mut tracker,&unknown,Context::default()).is_err());
}

#[test]
fn observed_776_item_nogravity_slot_and_collect_roundtrip_exact_packets() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/item-lifecycle-26.2-observation.json")).unwrap();
    assert_eq!(fixture["protocol"],776);
    assert_eq!(fixture["source_sha256"],"d846e94b6e99fe18a72a1ae4d331c5bb34871ea20541815382f92eaeba588263");
    assert_eq!(fixture["item_kind"],item_kind());
    let context=Context { online_mode:Some(false), ..Default::default() };
    let mut forward=Tracker::default();
    let join=clientbound(&mut forward,include_bytes!("../testdata/play-login-26.1.2.bin"),context).unwrap().into_owned();
    let mut reverse=Tracker::default();
    reverse.translate_inner(MID,OLD,&join,context,true).unwrap();
    let packets=fixture["packets"].as_array().unwrap();
    assert_eq!(packets.len(),5);
    for packet in packets {
        let body: Vec<u8>=packet["body"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
            .map(|pair|u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect();
        match packet["name"].as_str().unwrap() {
            "spawn_entity"=>assert_eq!(packet["decoded"]["velocity_hex"],"00"),
            "entity_metadata"=>assert_eq!(body,[0x63,2,5,8,1,8,7,3,1,0,0,255]),
            "collect"=>assert_eq!(body,[0x7c,2,1,3]),
            "entity_destroy"=>assert_eq!(body,[0x4d,1,2]),
            _=>panic!("неожиданный пакет выбранного наблюдения"),
        }
        let native=reverse.translate_inner(MID,OLD,&body,context,true).unwrap().into_owned();
        assert_eq!(clientbound(&mut forward,&native,context).unwrap().as_ref(),body);
    }
    // Оба Tracker забыли настоящий item только после настоящего destroy.
    let metadata=[0x63,2,8,7,3,1,0,0,255];
    assert!(clientbound(&mut forward,&metadata,context).is_err());
    assert!(reverse.translate_inner(MID,OLD,&metadata,context,true).is_err());
}
