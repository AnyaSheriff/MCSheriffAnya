// Настоящий обработчик слежения и выходной буфер, включая перевод в Java777.
// Поля полных пакетов взяты из собственной opaque-server записи42167625247.
use super::*;
use crate::arrow::{Arrow, Arrows, Shooter};
use mcsa_protocol::{ids, wire, JavaVersion};

const ID: i32 = 701;
fn sample(trident: bool) -> serde_json::Value {
    let data: serde_json::Value = serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/arrow-landing-777-observations.json")).unwrap();
    data["cases"][usize::from(trident)].clone()
}
fn hex(text: &str) -> Vec<u8> {
    let (pairs, remainder)=text.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs.iter().map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()
}
fn server() -> Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/arrow-long-range/unused-ops.json")),crate::skins::Settings::default())
}
fn observer(id: u8) -> PlayerState {
    let mut p=PlayerState::new([id;16],format!("Landing{id}"),None,None,0x7f,u64::from(id),(4.5,65.0,-5.5));
    p.game_mode=GAME_MODE_SURVIVAL;p.awaiting_teleport=false;p
}
fn arrow(trident: bool) -> Arrow {
    let mut a=Arrow::from_mob(ID,Dimension::Overworld,(0.5,66.52,-5.5),(0.0,0.0,3.0),99,"");
    a.shooter=Shooter::Player;a.age=1;a.fresh=false;a.crit = !trident;
    if trident {a.trident=Some(Stack::new(blocks::item_named("trident").unwrap(),1));}
    a
}
fn land(a: &mut Arrow, trident: bool) {
    let value=sample(trident);let at=value["position"].as_array().unwrap();let rotation=value["rotation"].as_array().unwrap();
    (a.x,a.y,a.z)=(at[0].as_f64().unwrap(),at[1].as_f64().unwrap(),at[2].as_f64().unwrap());
    a.rot=Some((rotation[0].as_f64().unwrap() as f32,rotation[1].as_f64().unwrap() as f32));
    (a.vx,a.vy,a.vz)=(0.0,0.0,0.0);a.in_ground=true;a.ground_ticks=1;a.age=12;a.crit=false;
}
fn id(version: JavaVersion, internal: i32) -> i32 {
    if version==wire::INTERNAL {internal} else {
        ids::translate(ids::RegistryKind::PlayClientbound,wire::INTERNAL,version,internal as u32).unwrap() as i32
    }
}
async fn packets(out: &Outgoing<'_>) -> Vec<PacketInfo> {
    let mut bytes=out.queued();let mut result=Vec::new();
    while !bytes.is_empty() {result.push(read_packet(&mut bytes).await.unwrap());}
    result
}
async fn golden_landing(trident: bool) {
    for version in [wire::INTERNAL,JavaVersion::V26_3] {
        let shared=server();let mut state=Arrows {list:vec![arrow(trident)],retired:Vec::new()};
        let mut viewers=[observer(41),observer(42)];
        let mut outputs=[Outgoing::capture_content(version,shared.catalog().content_codec()),Outgoing::capture_content(version,shared.catalog().content_codec())];
        for (p,out) in viewers.iter_mut().zip(outputs.iter_mut()) {
            bow::send_arrow_ticks(out,&shared,p,Some(&state)).await.unwrap();
        }
        let before=packets(&outputs[0]).await.len();
        land(&mut state.list[0],trident);
        for (p,out) in viewers.iter_mut().zip(outputs.iter_mut()) {
            bow::send_arrow_ticks(out,&shared,p,Some(&state)).await.unwrap();
        }
        let a=packets(&outputs[0]).await;let b=packets(&outputs[1]).await;
        assert_eq!(a.iter().map(|p|&p.body).collect::<Vec<_>>(),b.iter().map(|p|&p.body).collect::<Vec<_>>());
        let landing=&a[before..];
        assert_eq!(landing.iter().map(|p|p.id).collect::<Vec<_>>(),[id(version,0x65),id(version,0x23),id(version,SET_ENTITY_DATA)]);
        assert_eq!(landing[0].payload(),[encode_varint(ID),vec![0]].concat());
        let sync=landing[1].payload();assert!(sync.starts_with(&encode_varint(ID)));
        let fields=&sync[encode_varint(ID).len()..];
        if version==JavaVersion::V26_3 {assert_eq!(fields,hex(sample(trident)["absolute_fields_hex"].as_str().unwrap()));}
        else {assert_eq!(fields.len(),57);assert_eq!(fields.last(),Some(&0));}
        let fixture=sample(trident);let mut expected=encode_varint(ID);
        let raw=hex(fixture["metadata_payloads"][0].as_str().unwrap());
        let captured_id=fixture["entity"].as_i64().unwrap() as i32;
        expected.extend_from_slice(&raw[encode_varint(captured_id).len()..]);
        assert_eq!(landing[2].payload(),expected);
    }
}

#[tokio::test]
async fn distant_player_arrow_landing_matches_recorded_777_fields_for_both_observers() {golden_landing(false).await;}
#[tokio::test]
async fn distant_trident_landing_keeps_packet_ground_separate_from_arrow_ground() {golden_landing(true).await;}

#[tokio::test]
async fn landing_waits_for_next_snapshot_before_sending_coordinates() {
    let shared=server();let mut p=observer(41);let mut state=Arrows {list:vec![arrow(false)],retired:Vec::new()};
    let mut out=Outgoing::capture();bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();
    let before=out.queued().len();land(&mut state.list[0],false);state.list[0].ground_ticks=0;
    bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();assert_eq!(out.queued().len(),before);
    state.list[0].ground_ticks=1;
    bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();assert!(out.queued().len()>before);
}

#[tokio::test]
async fn settled_long_player_arrow_uses_zero_relative_move_at_update_boundary() {
    let shared=server();let mut p=observer(41);let mut state=Arrows {list:vec![arrow(false)],retired:Vec::new()};
    let mut out=Outgoing::capture_content(JavaVersion::V26_3,shared.catalog().content_codec());
    bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();land(&mut state.list[0],false);
    bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();let before=packets(&out).await.len();
    state.list[0].age=21;state.list[0].ground_ticks=10;
    bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();let all=packets(&out).await;
    assert_eq!(all.len(),before+1);assert_eq!(all[before].id,id(JavaVersion::V26_3,0x36));
    let payload=all[before].payload();assert!(payload.starts_with(&encode_varint(ID)));
    assert_eq!(&payload[encode_varint(ID).len()..encode_varint(ID).len()+7],&[0;7]);
}

#[tokio::test]
async fn unobserved_non_player_long_landing_keeps_previous_branch() {
    for summoned in [false,true] {
        let shared=server();let mut p=observer(41);let mut a=arrow(false);a.crit=false;
        if summoned {a.summoned=true;} else {a.shooter=Shooter::Mob {killer:"skeleton"};}
        let mut state=Arrows {list:vec![a],retired:Vec::new()};let mut out=Outgoing::capture();
        bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();let before=packets(&out).await.len();
        land(&mut state.list[0],false);bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();
        let all=packets(&out).await;assert_eq!(all[before..].iter().map(|p|p.id).collect::<Vec<_>>(),[0x65]);
    }
}

#[tokio::test]
async fn close_player_landing_keeps_relative_motion_and_changed_metadata() {
    let shared=server();let mut p=observer(41);let mut state=Arrows {list:vec![arrow(false)],retired:Vec::new()};
    let mut out=Outgoing::capture();bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();let before=packets(&out).await.len();
    land(&mut state.list[0],false);state.list[0].z=1.0;
    bow::send_arrow_ticks(&mut out,&shared,&mut p,Some(&state)).await.unwrap();let all=packets(&out).await;
    assert_eq!(all[before..].iter().map(|p|p.id).collect::<Vec<_>>(),[0x65,0x36,SET_ENTITY_DATA]);
    assert_eq!(all.last().unwrap().payload(),[encode_varint(ID),vec![8,0,0,10,8,1,255]].concat());
}

#[test]
fn actual_block_collision_clears_critical_before_the_tracking_snapshot() {
    let shared=server();let mut a=arrow(false);a.x=0.5;a.y=64.5;a.z=0.5;
    shared.arrows.lock().unwrap().spawn(a);
    let mut world=shared.world_in(Dimension::Overworld).lock().unwrap();
    for z in 0..=3 {world.set_block_silently(0,64,z,crate::world::AIR);}
    world.set_block_silently(0,64,2,blocks::state_by_name("stone").unwrap());
    crate::arrow::step(&shared,Dimension::Overworld,&mut world);
    let arrows=shared.arrows.lock().unwrap();assert!(arrows.list[0].in_ground);assert!(!arrows.list[0].crit);
}
