//! Реальные обработчики; наблюдения opaque777 CI42226930283, не формулы-копии.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile};
use crate::survival::Source;
use mcsa_protocol::{ids, wire, JavaVersion};

fn setup(profile: Profile) -> (Shared, PlayerState) {
    let catalog=Catalog::new(profile);
    let shared=Shared::new(World::in_memory_with_catalog(catalog),World::in_memory_with_catalog(catalog),
        Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/armor-enchants-runtime/unused.json")),crate::skins::Settings::default());
    let mut p=PlayerState::new_for_catalog([81;16],"Defender".into(),None,None,0x7f,81,(0.5,64.0,3.5),catalog);
    p.entity_id=81;p.game_mode=GAME_MODE_SURVIVAL;p.awaiting_teleport=false;p.on_ground=true;
    shared.players.lock().unwrap().join(p.member(),p.entity_id).unwrap();
    (shared,p)
}
fn observer(shared: &Shared) -> PlayerState {
    let mut p=PlayerState::new_for_catalog([82;16],"Observer".into(),None,None,0x7f,82,(0.5,64.0,1.5),shared.catalog());
    p.entity_id=82;p.game_mode=GAME_MODE_SURVIVAL;p.awaiting_teleport=false;p
}
fn publish(shared: &Shared, player: &PlayerState) {
    publish_player(shared,player);
    let mut players=shared.players.lock().unwrap();
    players.set_sneaking(&player.uuid,player.sneaking);
    players.set_skin_parts(&player.uuid,player.skin_parts);
    players.publish_tick();
}
fn chest(damage: i32) -> Stack {
    Stack::new(blocks::item_named("iron_chestplate").unwrap(),1)
        .with_extras(crate::stackdata::Data {damage,..Default::default()})
}
fn strike() -> crate::combat::PlayerStrike {
    crate::combat::PlayerStrike {kinetic:None,victim:81,attacker:82,projectile:None,attacker_uuid:[82;16],
        attacker_name:"Observer".into(),from:(0.5,64.0,1.5),yaw:0.0,damage:7.0,source:Source::PlayerAttack,
        extra_knockback:0.0,weapon:None,disables_shield_ticks:0,breach_scale:1.0,fire_ticks:0,
        ignores_shield:false,thorns_slots:Vec::new(), armor_wear_plan: None,effects:Vec::new()}
}
fn packet_id(version: JavaVersion, value: i32) -> i32 {
    if version==wire::INTERNAL {value} else {ids::translate(ids::RegistryKind::PlayClientbound,wire::INTERNAL,version,value as u32).unwrap() as i32}
}
async fn packets(out: &Outgoing<'_>) -> Vec<PacketInfo> {
    let mut bytes=out.queued();let mut result=Vec::new();
    while !bytes.is_empty() {result.push(read_packet(&mut bytes).await.unwrap());}
    result
}
async fn flags(out: &Outgoing<'_>, version: JavaVersion) -> Vec<u8> {
    packets(out).await.into_iter().filter(|p|p.id==packet_id(version,SET_ENTITY_DATA))
        .filter_map(|p| {let b=p.payload();(b.len()>=4&&b[..3]==[81,0,0]).then(||b[3])}).collect()
}

#[tokio::test]
async fn breaking_chestplate_is_removed_before_same_hit_mitigation_and_self_event() {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {
        let (shared,mut p)=setup(Profile::Preview26_3);p.inventory.set(6,Some(chest(239)));
        vitals::update_equipment(&mut p,0);assert_eq!(p.attrs.value(crate::attributes::ARMOR),6.0);
        let mut out=Outgoing::capture_content(version,shared.catalog().content_codec());
        send_login(&mut out,Dimension::Overworld,GAME_MODE_SURVIVAL,81,2,2,false).await.unwrap();
        let mut changes=shared.world_in(p.dimension).lock().unwrap().watch_events(81);
        pvp::take_strike(&mut out,&shared,&mut p,&strike()).await.unwrap();
        assert_eq!(p.vit.health,13.0);assert!(p.inventory.slot(6).is_none());
        assert_eq!(p.attrs.value(crate::attributes::ARMOR),0.0);
        let before=packets(&out).await;
        let event=before.iter().position(|p|p.id==packet_id(version,0x22)).unwrap();
        let damage=before.iter().position(|p|p.id==packet_id(version,0x19)).unwrap();
        assert!(event<damage);assert_eq!(before[event].payload(),[0,0,0,81,50]);
        send_pending_world_events(&mut out,&shared,&mut p,&mut changes).await.unwrap();
        assert_eq!(packets(&out).await.iter().filter(|p|p.id==packet_id(version,0x22)).count(),1);
        p.vit.invulnerable=0;shared.ticks.send_replace(20);
        pvp::take_strike(&mut out,&shared,&mut p,&strike()).await.unwrap();assert_eq!(p.vit.health,6.0);
    }
}

#[tokio::test]
async fn unbroken_chestplate_wears_once_and_keeps_observed_two_hit_health() {
    let (shared,mut p)=setup(Profile::Preview26_3);p.inventory.set(6,Some(chest(200)));
    vitals::update_equipment(&mut p,0);let mut out=Outgoing::capture();
    pvp::take_strike(&mut out,&shared,&mut p,&strike()).await.unwrap();
    assert_eq!(p.vit.health,f32::from_bits(0x415b3334));assert_eq!(p.inventory.slot(6).unwrap().extras().damage,201);
    p.vit.invulnerable=0;shared.ticks.send_replace(20);
    pvp::take_strike(&mut out,&shared,&mut p,&strike()).await.unwrap();
    assert_eq!(p.vit.health,f32::from_bits(0x40eccccf));assert_eq!(p.inventory.slot(6).unwrap().extras().damage,202);
}

#[tokio::test]
async fn rejected_melee_does_not_wear_or_break_chestplate() {
    for mode in [GAME_MODE_SURVIVAL,GAME_MODE_CREATIVE,GAME_MODE_SPECTATOR] {
        let (shared,mut p)=setup(Profile::Preview26_3);p.inventory.set(6,Some(chest(239)));
        vitals::update_equipment(&mut p,0);p.game_mode=mode;
        if mode==GAME_MODE_SURVIVAL {assert!(p.vit.hurt(7.0,Source::PlayerAttack,false).is_some());}
        let health=p.vit.health;let mut out=Outgoing::capture();
        pvp::take_strike(&mut out,&shared,&mut p,&strike()).await.unwrap();
        assert_eq!(p.vit.health,health);assert_eq!(p.inventory.slot(6).unwrap().extras().damage,239);
        assert!(!packets(&out).await.iter().any(|p|p.id==0x22));
    }
}

#[tokio::test]
async fn historical_profile_keeps_previous_breaking_hit_damage() {
    let (shared,mut p)=setup(Profile::Java26_1_2);p.inventory.set(6,Some(chest(239)));
    vitals::update_equipment(&mut p,0);let mut out=Outgoing::capture();
    pvp::take_strike(&mut out,&shared,&mut p,&strike()).await.unwrap();
    assert_eq!(p.vit.health,f32::from_bits(0x415b3334));assert!(p.inventory.slot(6).is_none());
}

#[tokio::test]
async fn published_fire_starts_and_extinguishes_for_other_player_without_speed_repeat() {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {
        let (shared,mut p)=setup(Profile::Preview26_3);let viewer=observer(&shared);let mut seen=Seen::new();
        publish(&shared,&p);let mut out=Outgoing::capture_content(version,shared.catalog().content_codec());
        send_login(&mut out,Dimension::Overworld,GAME_MODE_SURVIVAL,82,2,2,false).await.unwrap();
        send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Moves).await.unwrap();
        for burning in [true,false] {
            p.net.fire_flag=burning;publish(&shared,&p);
            send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Swings).await.unwrap();
            send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Moves).await.unwrap();
        }
        assert_eq!(flags(&out,version).await,vec![0,1,0]);
        assert!(!packets(&out).await.iter().any(|p|p.id==packet_id(version,0x83)));
    }
}

#[tokio::test]
async fn burning_survives_sprint_crouch_and_skin_updates() {
    let (shared,mut p)=setup(Profile::Preview26_3);let viewer=observer(&shared);let mut seen=Seen::new();
    let mut out=Outgoing::capture();publish(&shared,&p);
    send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Moves).await.unwrap();
    p.net.fire_flag=true;p.vit.sprinting=true;p.sneaking=true;publish(&shared,&p);
    send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Swings).await.unwrap();
    send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Moves).await.unwrap();
    let before_skin=flags(&out,wire::INTERNAL).await.len();
    p.skin_parts^=1;publish(&shared,&p);
    send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Moves).await.unwrap();
    let got=flags(&out,wire::INTERNAL).await;assert_eq!(got.len(),before_skin+1);assert!(got[1..].iter().all(|f|*f==11));
}

#[tokio::test]
async fn newly_visible_burning_player_has_fire_in_first_metadata() {
    let (shared,mut p)=setup(Profile::Preview26_3);p.net.fire_flag=true;publish(&shared,&p);
    let viewer=observer(&shared);let mut out=Outgoing::capture();let mut seen=Seen::new();
    send_other_moves(&mut out,&shared,&viewer,&mut seen,Phase::Moves).await.unwrap();
    assert_eq!(flags(&out,wire::INTERNAL).await,vec![1]);
}

#[tokio::test]
async fn command_armor_publishes_gear_and_sound_before_return() {
    let (shared,mut p)=setup(Profile::Preview26_3);let mut out=Outgoing::capture();publish(&shared,&p);
    let from=shared.world_in(p.dimension).lock().unwrap().watch_events(81);
    apply_command_slot(&mut out,&shared,&mut p,6,Some(chest(200))).await.unwrap();
    assert_eq!(shared.players.lock().unwrap().gear(&p.uuid).worn[2],Some(chest(200)));
    let (events,_)=shared.world_in(p.dimension).lock().unwrap().events_since(from,81);
    assert!(events.iter().any(|e|matches!(e,WorldEvent::Sound(sound) if sound.name=="minecraft:item.armor.equip_iron")));
}
