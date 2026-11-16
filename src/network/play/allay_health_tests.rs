//! Фактические HP тихони: журнал и настоящие пакеты777, без сокетов.
use super::*;
use crate::{gameplay_catalog::{Catalog,Profile}, mob::{Kind,Mob,Rng,View}};
use mcsa_protocol::{JavaVersion,ids::{self,RegistryKind}};

fn setup(kind: Kind, ticks: u64, health: f32) -> (Shared,PlayerState,usize) {
    let catalog=Catalog::new(Profile::Preview26_3);
    let shared=Shared::new(World::in_memory_with_catalog(catalog),World::in_memory_with_catalog(catalog),
        Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(Path::new("tmp/allay-health-ops.json")),crate::skins::Settings::default());
    let p=PlayerState::new([217;16],"AllayHealth".into(),None,None,0x7f,217,(0.5,64.0,0.5));
    let from={let mut items=shared.items_in(p.dimension).lock().unwrap();
        let from=items.watch_changes(91);
        let mut m=Mob::new(42,kind,(0.5,64.0,2.5));m.no_ai=true;m.ticks=ticks;m.vitals.health=health;
        items.add_mob(m);items.snapshot_mobs();from};
    (shared,p,from)
}
fn out(shared: &Shared) -> Outgoing<'static> {
    Outgoing::capture_content(JavaVersion::V26_3,shared.catalog().content_codec())
}
fn wire_id(id: i32) -> i32 {
    ids::translate(RegistryKind::PlayClientbound,JavaVersion::V26_1_2,JavaVersion::V26_3,id as u32).unwrap() as i32
}
async fn packets(out: &Outgoing<'_>) -> Vec<(i32,Vec<u8>)> {
    let mut bytes=out.queued();let mut result=Vec::new();
    while !bytes.is_empty() {let p=read_packet(&mut bytes).await.unwrap();result.push((p.id,p.payload().to_vec()));}
    result
}
async fn pending(out: &mut Outgoing<'_>,shared: &Shared,p: &mut PlayerState,from: &mut usize) {
    let listener=(p.x,p.y,p.z);
    send_pending_item_changes(out,shared,p.dimension,from,&mut SeenItems::new(),
        Known {carts:&mut p.seen_carts,frames:&mut p.seen_frames,mobs:&mut p.seen_mobs,listener},91).await.unwrap();
}
fn hp_payload(value: f32) -> Vec<u8> {
    let mut bytes=encode_varint(42);bytes.extend([9,3]);bytes.extend(value.to_be_bytes());bytes.push(255);bytes
}
async fn tail(out: &Outgoing<'_>,prefix: usize) -> Vec<(i32,Vec<u8>)> {
    packets(out).await.into_iter().skip(prefix).collect()
}
async fn health_packet_count(out: &Outgoing<'_>,health: f32,prefix: usize) -> usize {
    tail(out,prefix).await.iter().filter(|(id,p)|*id==wire_id(SET_ENTITY_DATA)&&*p==hp_payload(health)).count()
}
struct Ground(Catalog);
impl View for Ground {
    fn catalog(&self) -> Catalog { self.0 }
    fn loaded(&self,_x:i32,_z:i32)->bool {true}
    fn block(&self,_x:i32,y:i32,_z:i32)->i32 {if y<64 {blocks::state_by_name("stone").unwrap()} else {0}}
}
fn step(shared: &Shared) {
    let mut items=shared.items_in(Dimension::Overworld).lock().unwrap();items.snapshot_mobs();
    crate::mob::step_all(&mut items,&Ground(shared.catalog()),&[],
        crate::mob::Env {difficulty:2,sky:15,sun_up:false,clock:0,tick:10},
        &mut Rng::new(7),&mut ||panic!("тихоня без ИИ не создаёт сущностей"));
}

#[tokio::test]
async fn frozen_allay_command_damage_publishes_health_without_a_world_step() {
    let (shared,mut p,mut from)=setup(Kind::Allay,7,20.0);
    let mut wire=out(&shared);mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    let prefix=packets(&wire).await.len();
    crate::commands::run("tick freeze",&crate::commands::Source::Console,&shared);
    assert!(shared.world_cmd.ticking.lock().unwrap().frozen);
    let answer=crate::commands::run("damage @e[type=allay,limit=1] 1 minecraft:generic",&crate::commands::Source::Console,&shared);
    assert!(answer.reply.contains("Нанесён"));
    {let items=shared.items_in(p.dimension).lock().unwrap();assert_eq!(items.mobs()[0].ticks,7);assert_eq!(items.mob_view()[0].vitals.health,19.0);}
    pending(&mut wire,&shared,&mut p,&mut from).await;
    assert_eq!(health_packet_count(&wire,19.0,prefix).await,1);
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    assert_eq!(health_packet_count(&wire,19.0,prefix).await,1);
    assert_eq!(health_packet_count(&wire,20.0,prefix).await,0);
}

#[tokio::test]
async fn natural_allay_healing_publishes_one_health_packet_on_tenth_tick() {
    let (shared,mut p,mut from)=setup(Kind::Allay,9,19.0);
    let mut wire=out(&shared);mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    let prefix=packets(&wire).await.len();
    step(&shared);
    {let items=shared.items_in(p.dimension).lock().unwrap();assert_eq!(items.mobs()[0].ticks,10);assert_eq!(items.mob_view()[0].ticks,9);assert_eq!(items.mob_view()[0].vitals.health,20.0);}
    pending(&mut wire,&shared,&mut p,&mut from).await;
    assert_eq!(tail(&wire,prefix).await,[(wire_id(SET_ENTITY_DATA),hp_payload(20.0))]);
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    assert_eq!(health_packet_count(&wire,20.0,prefix).await,1);
    shared.items_in(p.dimension).lock().unwrap().snapshot_mobs();
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    assert_eq!(health_packet_count(&wire,20.0,prefix).await,1);
}

#[tokio::test]
async fn natural_allay_health_does_not_advance_motion_tracking() {
    let (shared,mut p,mut from)=setup(Kind::Allay,19,19.0);
    let mut wire=out(&shared);mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    let prefix=packets(&wire).await.len();
    {let mut items=shared.items_in(p.dimension).lock().unwrap();items.mobs_mut()[0].x+=1.0;items.mobs_mut()[0].vx=0.5;}
    step(&shared); // Снимок19 не кратен3; фактическое лечение на20.
    pending(&mut wire,&shared,&mut p,&mut from).await;
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    assert_eq!(tail(&wire,prefix).await,[(wire_id(SET_ENTITY_DATA),hp_payload(20.0))]);
}

#[tokio::test]
async fn cow_command_damage_keeps_old_snapshot_health_until_next_snapshot() {
    let (shared,mut p,mut from)=setup(Kind::Cow,7,10.0);
    let mut wire=out(&shared);mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    let prefix=packets(&wire).await.len();
    crate::commands::run("damage @e[type=cow,limit=1] 1 minecraft:generic",&crate::commands::Source::Console,&shared);
    {let items=shared.items_in(p.dimension).lock().unwrap();assert_eq!(items.mobs()[0].vitals.health,9.0);assert_eq!(items.mob_view()[0].vitals.health,10.0);}
    pending(&mut wire,&shared,&mut p,&mut from).await;
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    assert_eq!(health_packet_count(&wire,9.0,prefix).await,0);
    shared.items_in(p.dimension).lock().unwrap().snapshot_mobs();
    mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    assert_eq!(health_packet_count(&wire,9.0,prefix).await,1);
}

#[tokio::test]
async fn allay_health_helper_ignores_unknown_or_nonliving_seen_entities_and_invalid_hp() {
    let (shared,mut p,_)=setup(Kind::Allay,7,20.0);
    let mut wire=out(&shared);
    mobs::send_allay_health(&mut wire,42,19.0,&mut p.seen_mobs).await.unwrap();
    assert!(wire.queued().is_empty());
    let orb=crate::xp::XpOrb::new(43,1,(0.5,64.0,2.5),(0.0,0.0,0.0));
    p.seen_mobs.insert(43,mobs::send_orb(&mut wire,&orb).await.unwrap());
    let prefix=packets(&wire).await.len();
    mobs::send_allay_health(&mut wire,43,19.0,&mut p.seen_mobs).await.unwrap();
    assert!(tail(&wire,prefix).await.is_empty());
    p.seen_mobs.clear();mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    let prefix=packets(&wire).await.len();
    for health in [f32::NAN,f32::INFINITY,0.0,-1.0,20.0] {
        mobs::send_allay_health(&mut wire,42,health,&mut p.seen_mobs).await.unwrap();
    }
    assert!(tail(&wire,prefix).await.is_empty());
    mobs::send_allay_health(&mut wire,42,19.0,&mut p.seen_mobs).await.unwrap();
    mobs::send_allay_health(&mut wire,42,19.0,&mut p.seen_mobs).await.unwrap();
    assert_eq!(tail(&wire,prefix).await,[(wire_id(SET_ENTITY_DATA),hp_payload(19.0))]);
}

#[tokio::test]
async fn allay_health_cache_is_not_advanced_after_failed_encoding() {
    let (shared,mut p,_)=setup(Kind::Allay,7,20.0);
    let mut wire=out(&shared);mobs::send_mob_ticks(&mut wire,&shared,&mut p).await.unwrap();
    let prefix=packets(&wire).await.len();
    // Outgoing без согласованного режима совместимости отклоняет этот
    // Preview26_3-пакет; кеш наблюдателя должен остаться на прежних20HP.
    let mut invalid=Outgoing::capture_content(JavaVersion::V1_21_11,shared.catalog().content_codec());
    assert!(mobs::send_allay_health(&mut invalid,42,19.0,&mut p.seen_mobs).await.is_err());
    mobs::send_allay_health(&mut wire,42,19.0,&mut p.seen_mobs).await.unwrap();
    assert_eq!(tail(&wire,prefix).await,[(wire_id(SET_ENTITY_DATA),hp_payload(19.0))]);
}

#[test]
fn allay_health_change_participates_in_dropped_collision_guard() {
    assert_eq!(other_item_change_id(&items::Change::AllayHealth {entity_id:42,health:19.0}),Some(42));
}
