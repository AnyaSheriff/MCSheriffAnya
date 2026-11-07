//! Настоящий strike_player→take_strike→tick; opaque777 baseline42352080533.
use super::*;
use crate::gameplay_catalog::{Catalog,Profile};
use crate::survival::Source;
use mcsa_protocol::{ids,wire,JavaVersion};

fn setup() -> (Shared,PlayerState,PlayerState) {
    let catalog=Catalog::new(Profile::Preview26_3);
    let shared=Shared::new(World::in_memory_with_catalog(catalog),World::in_memory_with_catalog(catalog),
        Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/protection-edges-runtime/unused.json")),crate::skins::Settings::default());
    let make=|byte,name: &str,z| {
        let mut p=PlayerState::new_for_catalog([byte;16],name.into(),None,None,0x7f,u64::from(byte),(0.5,64.0,z),catalog);
        p.entity_id=i32::from(byte);p.game_mode=GAME_MODE_SURVIVAL;p.awaiting_teleport=false;p.on_ground=true;
        shared.players.lock().unwrap().join(p.member(),p.entity_id).unwrap();p
    };
    let victim=make(81,"Defender",3.5);let attacker=make(82,"Attacker",1.5);(shared,victim,attacker)
}
fn chest(damage: i32,unbreaking: i32,unbreakable: bool) -> Stack {
    let mut data=crate::stackdata::Data {damage,unbreakable,..Default::default()};
    data.set_enchant(crate::stackdata::enchantment_number("thorns").unwrap(),7);
    if unbreaking>0 {data.set_enchant(crate::stackdata::enchantment_number("unbreaking").unwrap(),unbreaking);}
    Stack::new(blocks::item_named("diamond_chestplate").unwrap(),1).with_extras(data)
}
fn packet_id(version: JavaVersion,value: i32) -> i32 {
    if version==wire::INTERNAL {value} else {ids::translate(ids::RegistryKind::PlayClientbound,wire::INTERNAL,version,value as u32).unwrap() as i32}
}
async fn count(out: &Outgoing<'_>,wanted: i32) -> usize {
    let mut bytes=out.queued();let mut n=0;
    while !bytes.is_empty() {if read_packet(&mut bytes).await.unwrap().id==wanted {n+=1;}}n
}
async fn real_attack(breaking: bool,version: JavaVersion) {
    let (shared,mut victim,mut attacker)=setup();victim.inventory.set(6,Some(chest(if breaking {527} else {0},0,false)));
    vitals::update_equipment(&mut victim,0);publish_player(&shared,&victim);publish_player(&shared,&attacker);
    shared.ticks.send_replace(40);let from=shared.world_in(victim.dimension).lock().unwrap().watch_events(81);
    assert!(pvp::strike_player(&shared,&mut attacker,81,40));
    let (events,_)=shared.world_in(victim.dimension).lock().unwrap().events_since(from,81);
    let incoming=events.iter().find_map(|e|match e {WorldEvent::PlayerStrike(strike) if strike.victim==81=>Some(strike.clone()),_=>None}).unwrap();
    assert_eq!(incoming.thorns_slots,if breaking {Vec::new()} else {vec![6]});
    let mut target=Outgoing::capture_content(version,shared.catalog().content_codec());
    send_login(&mut target,Dimension::Overworld,GAME_MODE_SURVIVAL,81,2,2,false).await.unwrap();
    pvp::take_strike(&mut target,&shared,&mut victim,&incoming).await.unwrap();
    let mut actor=Outgoing::capture_content(version,shared.catalog().content_codec());
    send_login(&mut actor,Dimension::Overworld,GAME_MODE_SURVIVAL,82,2,2,false).await.unwrap();
    vitals::tick(&mut actor,&shared,&mut attacker).await.unwrap();
    if breaking {
        assert_eq!(victim.vit.health,19.0);assert!(victim.inventory.slot(6).is_none());
        assert_eq!(attacker.vit.health,20.0);assert_eq!(count(&actor,packet_id(version,0x19)).await,0);
        assert!(!events.iter().any(|e|matches!(e,WorldEvent::PlayerDamaged {entity_id:82,..})));
    } else {
        assert_eq!(victim.vit.health,f32::from_bits(0x419a6e98));
        assert_eq!(victim.inventory.slot(6).unwrap().extras().damage,3);
        assert!((15.0..=19.0).contains(&attacker.vit.health));
        assert_eq!(count(&actor,packet_id(version,0x19)).await,1);
    }
}

#[tokio::test]
async fn broken_thorns_piece_never_reflects_in_real_melee_both_wires() {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {real_attack(true,version).await;}
}
#[tokio::test]
async fn intact_thorns_control_reflects_and_wears_three_in_both_wires() {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {real_attack(false,version).await;}
}
#[test]
fn unbreaking_and_unbreakable_fates_are_not_treated_as_guaranteed_breaks() {
    let (_,mut victim,_)=setup();
    for (unbreaking,unbreakable) in [(3,false),(0,true)] {
        victim.inventory.set(6,Some(chest(527,unbreaking,unbreakable)));let gear=victim.gear();
        let after=pvp::thorns_after_armor_wear(Profile::Preview26_3,&gear,1.0,Source::PlayerAttack);
        assert_eq!(after.thorns,gear.view.thorns);assert_eq!(victim.inventory.slot(6),gear.worn[2]);
    }
}
#[test]
fn historical_profile_and_other_sources_keep_previous_thorns_view() {
    let (_,mut victim,_)=setup();victim.inventory.set(6,Some(chest(527,0,false)));let gear=victim.gear();
    for (profile,source) in [(Profile::Java26_1_2,Source::PlayerAttack),(Profile::Preview26_3,Source::Arrow),(Profile::Preview26_3,Source::MaceSmash)] {
        assert_eq!(pvp::thorns_after_armor_wear(profile,&gear,7.0,source).thorns,gear.view.thorns);
    }
}
