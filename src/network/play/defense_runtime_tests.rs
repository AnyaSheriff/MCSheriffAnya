//! Реальные PvP-обработчики и tick; golden-поля из opaque original42190823756.
use super::*;
use mcsa_protocol::{ids, wire, JavaVersion};
use crate::survival::Source;

fn shared() -> Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/defense-runtime/unused-ops.json")),crate::skins::Settings::default())
}
fn player() -> PlayerState {
    let mut p=PlayerState::new([81;16],"Defender".into(),None,None,0x7f,81,(0.5,64.0,3.5));
    p.entity_id=81;p.game_mode=GAME_MODE_SURVIVAL;p.awaiting_teleport=false;p.on_ground=true;p
}
fn strike(source: Source) -> crate::combat::PlayerStrike {
    crate::combat::PlayerStrike {kinetic:None,victim:81,attacker:82,projectile:None,attacker_uuid:[82;16],
        attacker_name:"Attacker".into(),from:(0.5,64.0,1.5),yaw:0.0,damage:7.0,source,extra_knockback:0.0,
        weapon:Some(Stack::new(blocks::item_named("diamond_sword").unwrap(),1)),disables_shield_ticks:0,
        breach_scale:1.0,fire_ticks:0,ignores_shield:false,thorns_slots:Vec::new(),effects:Vec::new()}
}
fn id(version: JavaVersion, value: i32) -> i32 {
    if version==wire::INTERNAL {value} else {ids::translate(ids::RegistryKind::PlayClientbound,wire::INTERNAL,version,value as u32).unwrap() as i32}
}
fn unhex(s: &str) -> Vec<u8> {
    let (pairs,rest)=s.as_bytes().as_chunks::<2>();assert!(rest.is_empty());
    pairs.iter().map(|b|u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()
}
fn fixture() -> serde_json::Value {serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/defense-777-observations.json")).unwrap()}
async fn selected(out: &Outgoing<'_>, wanted: i32) -> Vec<Vec<u8>> {
    let mut bytes=out.queued();let mut found=Vec::new();
    while !bytes.is_empty() {let p=read_packet(&mut bytes).await.unwrap();if p.id==wanted {found.push(p.payload().to_vec());}}
    found
}
async fn animation(yaw: f32, source: Source, expected: f32) {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {
        let shared=shared();let mut p=player();p.yaw=yaw;
        let mut out=Outgoing::capture_content(version,shared.catalog().content_codec());
        send_login(&mut out,Dimension::Overworld,GAME_MODE_SURVIVAL,81,2,2,false).await.unwrap();
        pvp::take_strike(&mut out,&shared,&mut p,&strike(source)).await.unwrap();
        let packets=selected(&out,id(version,0x2a)).await;
        let mut target=encode_varint(81);push_f32(&mut target,expected);assert_eq!(packets,vec![target]);
        assert!(p.vit.health<20.0);
        if yaw==0.0 && source==Source::PlayerAttack {
            assert_eq!(&packets[0][encode_varint(81).len()..],unhex(fixture()["rear_animation_fields_hex"].as_str().unwrap()));
        }
    }
}
#[tokio::test]
async fn rear_melee_animation_matches_recorded_signed_yaw_in_both_wires() {animation(0.0,Source::PlayerAttack,f32::from_be_bytes(unhex(fixture()["rear_animation_fields_hex"].as_str().unwrap()).try_into().unwrap())).await;}
#[tokio::test]
async fn front_melee_animation_matches_recorded_positive_yaw() {animation(-180.0,Source::PlayerAttack,f32::from_be_bytes(unhex(fixture()["front_animation_fields_hex"].as_str().unwrap()).try_into().unwrap())).await;}
#[tokio::test]
async fn thorns_keeps_previous_unsigned_angle() {animation(0.0,Source::Thorns,270.0).await;}

#[tokio::test]
async fn shield_axe_expiry_actual_tick_sends_recorded_zero_once() {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {
        let shared=shared();let mut p=player();p.yaw = -180.0;
        p.inventory.set(inventory::OFFHAND,Some(Stack::new(blocks::item_named("shield").unwrap(),1)));
        assert!(vitals::start_using(&shared,&mut p,1,0));shared.ticks.send_replace(10);
        let mut axe=strike(Source::PlayerAttack);axe.damage=9.0;axe.disables_shield_ticks=100;
        axe.weapon=Some(Stack::new(blocks::item_named("iron_axe").unwrap(),1));
        let mut out=Outgoing::capture_content(version,shared.catalog().content_codec());
        send_login(&mut out,Dimension::Overworld,GAME_MODE_SURVIVAL,81,2,2,false).await.unwrap();
        pvp::take_strike(&mut out,&shared,&mut p,&axe).await.unwrap();
        assert_eq!(p.net.shield_disabled_until,110);assert_eq!(p.vit.health,20.0);
        for now in [109,110,110,140] {shared.ticks.send_replace(now);vitals::tick(&mut out,&shared,&mut p).await.unwrap();
            if now==109 {assert_eq!(selected(&out,id(version,0x16)).await.len(),1);}}
        let data=fixture();let expected:Vec<Vec<u8>>=data["cooldowns_hex"].as_array().unwrap().iter().map(|v|unhex(v.as_str().unwrap())).collect();
        assert_eq!(selected(&out,id(version,0x16)).await,expected);assert_eq!(p.net.shield_disabled_until,0);
    }
}

#[tokio::test]
async fn shield_reuse_after_expiry_blocks_without_restoring_lost_health() {
    let shared=shared();let mut p=player();p.yaw = -180.0;
    p.inventory.set(inventory::OFFHAND,Some(Stack::new(blocks::item_named("shield").unwrap(),1)));
    assert!(vitals::start_using(&shared,&mut p,1,0));shared.ticks.send_replace(10);
    let mut axe=strike(Source::PlayerAttack);axe.damage=9.0;axe.disables_shield_ticks=100;
    let mut out=Outgoing::capture();pvp::take_strike(&mut out,&shared,&mut p,&axe).await.unwrap();
    shared.ticks.send_replace(45);assert!(!vitals::start_using(&shared,&mut p,1,45));
    pvp::take_strike(&mut out,&shared,&mut p,&strike(Source::PlayerAttack)).await.unwrap();assert_eq!(p.vit.health,13.0);
    shared.ticks.send_replace(130);vitals::tick(&mut out,&shared,&mut p).await.unwrap();
    assert!(vitals::start_using(&shared,&mut p,1,130));shared.ticks.send_replace(145);
    pvp::take_strike(&mut out,&shared,&mut p,&strike(Source::PlayerAttack)).await.unwrap();
    assert_eq!(p.vit.health,13.0);assert_eq!(p.inventory.slot(inventory::OFFHAND).unwrap().extras().damage,18);
}

#[tokio::test]
async fn extended_shield_deadline_does_not_emit_old_expiry() {
    let shared=shared();let mut p=player();let mut out=Outgoing::capture();p.net.shield_disabled_until=100;
    shared.ticks.send_replace(99);vitals::tick(&mut out,&shared,&mut p).await.unwrap();p.net.shield_disabled_until=150;
    shared.ticks.send_replace(100);vitals::tick(&mut out,&shared,&mut p).await.unwrap();
    assert!(selected(&out,0x16).await.is_empty());shared.ticks.send_replace(150);
    vitals::tick(&mut out,&shared,&mut p).await.unwrap();assert_eq!(selected(&out,0x16).await.len(),1);
}

#[tokio::test]
async fn shield_expiry_uses_published_tick_instead_of_later_live_clock() {
    let shared=shared();let mut p=player();let mut out=Outgoing::capture();p.net.shield_disabled_until=100;
    shared.ticks.send_replace(200);p.publication=Some(TrackingPublication {tick:99,events:None,players:Vec::new()});
    vitals::tick(&mut out,&shared,&mut p).await.unwrap();assert!(selected(&out,0x16).await.is_empty());
    p.publication.as_mut().unwrap().tick=100;vitals::tick(&mut out,&shared,&mut p).await.unwrap();
    assert_eq!(selected(&out,0x16).await.len(),1);assert_eq!(p.net.shield_disabled_until,0);
}
