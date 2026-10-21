//! Настоящий обработчик и очередь коррекции; это не доказательство сетевого паритета777.
use super::*;
use crate::gameplay_catalog::{Profile, WOOL_SLABS, CONCRETE_SLABS};

fn setup(state: i32, feet: f64) -> (Shared, PlayerState) {
    let catalog = Catalog::new(Profile::Preview26_3);
    let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/terrain-floor-ops.json")), Default::default());
    let mut player = PlayerState::new_for_catalog([213;16], "Floor".into(), None, None, 0x7f, 213,
        (0.5, feet, 0.5), catalog);
    player.entity_id = 213;
    player.game_mode = GAME_MODE_SURVIVAL;
    player.on_ground = true;
    player.moved_since_tp = false;
    shared.world_in(player.dimension).lock().unwrap().set_block_silently(0, 64, 0, state);
    shared.players.lock().unwrap().join(player.member(), player.entity_id).unwrap();
    (shared, player)
}

fn packet(id: i32, payload: &[u8]) -> PacketInfo {
    let mut body = encode_varint(id); body.extend_from_slice(payload);
    PacketInfo { id, length: body.len() as i32, body, teleport: None, foreign_inventory: false, legacy_inventory: None }
}

fn movement(to: Point, flags: u8, rotation: bool) -> PacketInfo {
    let mut payload = Vec::new();
    for value in [to.0, to.1, to.2] { push_f64(&mut payload, value); }
    if rotation { push_f32(&mut payload, 37.0); push_f32(&mut payload, 12.0); }
    payload.push(flags);
    packet(if rotation { SET_PLAYER_POSITION_AND_ROTATION } else { SET_PLAYER_POSITION }, &payload)
}

fn stone() -> i32 { blocks::state_by_name("stone").unwrap() }
fn position(p: &PlayerState) -> Point { (p.x, p.y, p.z) }
fn published(shared: &Shared) -> crate::players::Position { shared.players.lock().unwrap().find("Floor").unwrap().position }

#[test]
fn all_six_observed_support_shapes_reject_both_flags_before_vitals_and_publication() {
    let catalog = Catalog::new(Profile::Preview26_3);
    let shapes = [
        (stone(), 65.0), (blocks::state_by_name("white_wool").unwrap(), 65.0),
        (catalog.slab_state_for(&WOOL_SLABS[0], SlabHalf::Bottom, false).unwrap(), 64.5),
        (catalog.slab_state_for(&WOOL_SLABS[0], SlabHalf::Top, false).unwrap(), 65.0),
        (catalog.slab_state_for(&WOOL_SLABS[0], SlabHalf::Double, false).unwrap(), 65.0),
        (blocks::state_by_name("white_carpet").unwrap(), 64.0625),
    ];
    for (state, feet) in shapes {
        for flag in [0, 1] {
            for rotation in [false, true] {
                let (shared, mut p) = setup(state, feet);
                p.vit.fall = 4.25; p.net.last_delta = (0.1, -0.2, 0.3); p.glide.wall = true;
                let angles = (p.yaw, p.pitch);
                let before = (p.vit.health, p.vit.food, p.vit.saturation, p.vit.exhaustion, p.vit.fall);
                let reaction = handle_client_packet(&movement((0.5, feet - 0.125, 0.5), flag, rotation), &mut p, &shared);
                assert!(reaction.resync, "state={state} flag={flag} rotation={rotation}");
                assert_eq!(position(&p), (0.5, feet, 0.5)); assert!(p.on_ground); assert!(!p.moved_since_tp);
                assert_eq!((p.vit.health, p.vit.food, p.vit.saturation, p.vit.exhaustion, p.vit.fall), before);
                assert_eq!(p.net.last_delta, (0.1, -0.2, 0.3)); assert!(p.glide.wall);
                assert!(p.net.hits.is_empty() && p.net.landings.is_empty());
                // Углы исторически применяются раньше старых и нового guards.
                assert_eq!((p.yaw, p.pitch), if rotation { (37.0, 12.0) } else { angles });
                publish_player(&shared, &p);
                let member = published(&shared); assert_eq!((member.x, member.y, member.z), position(&p)); assert!(member.on_ground);
            }
        }
    }
}

#[test]
fn stone_intrusion_depths_and_airborne_preserve_whole_previous_pose() {
    for flag in [0, 1] {
        for depth in [0.125, 0.5, 1.0] {
            let (shared, mut p) = setup(stone(), 65.0);
            assert!(handle_client_packet(&movement((0.5, 65.0-depth, 0.5), flag, false), &mut p, &shared).resync);
            assert_eq!(p.y, 65.0); assert!(p.on_ground);
        }
        let (shared, mut p) = setup(stone(), 65.5); p.on_ground = false; p.vit.fall = 2.0;
        assert!(handle_client_packet(&movement((0.5, 64.875, 0.5), flag, false), &mut p, &shared).resync);
        assert_eq!(p.y, 65.5); assert!(!p.on_ground); assert_eq!(p.vit.fall, 2.0);
    }
}

#[tokio::test]
async fn rejection_uses_real_correction_then_normal_ack_unlocks_legal_input() {
    let (shared, mut p) = setup(stone(), 65.5); p.on_ground = false; p.vit.fall = 2.0;
    let bad = movement((0.5, 64.875, 0.5), 1, false);
    let reaction = handle_client_packet(&bad, &mut p, &shared);
    let mut out = Outgoing::capture();
    apply_reaction(&mut out, &shared, &mut p, &reaction).await.unwrap();
    assert!(p.awaiting_teleport); assert!(p.net.teleported); assert_eq!(p.vit.fall, 2.0);
    let mut bytes = out.queued();
    let correction = read_packet(&mut bytes).await.unwrap(); assert!(bytes.is_empty()); assert_eq!(correction.id, PLAYER_POSITION);
    let payload = correction.payload(); let (id, count) = decode_varint(payload).unwrap(); assert_eq!(id, p.teleport);
    assert_eq!(read_position(&payload[count..]), Some((0.5, 65.5, 0.5)));
    assert_eq!(&payload[payload.len()-4..], &0i32.to_be_bytes());
    assert!(!handle_client_packet(&bad, &mut p, &shared).resync); assert_eq!(p.y, 65.5);
    handle_client_packet(&packet(CONFIRM_TELEPORTATION, &encode_varint(id-1)), &mut p, &shared); assert!(p.awaiting_teleport);
    handle_client_packet(&packet(CONFIRM_TELEPORTATION, &encode_varint(id)), &mut p, &shared); assert!(!p.awaiting_teleport);
    let legal = movement((0.5, 65.0, 0.5), 1, false);
    assert!(!handle_client_packet(&legal, &mut p, &shared).resync); assert_eq!(p.y, 65.0);
    publish_player(&shared, &p); assert_eq!(published(&shared), p.position());
    assert!(p.net.hits.is_empty() && p.net.landings.is_empty());
    // 777 ACK с произвольной finite pose остаётся отдельным непроверенным путём;
    // здесь проверено обычное подтверждение коррекции, а не защита от такого ACK.
}

#[test]
fn lawful_fall_jump_descent_horizontal_and_edge_movement_are_accepted() {
    for (from, to, ground) in [
        ((0.5,66.0,0.5),(0.5,65.0,0.5),1),
        ((0.5,65.0,0.5),(0.5,65.5,0.5),0),
        ((0.5,65.5,0.5),(0.5,65.0,0.5),1),
        ((0.5,65.0,0.5),(1.3,65.0,0.5),1),
        ((0.5,65.0,0.5),(1.4,63.0,0.5),0),
        ((1.4,65.0,0.5),(1.4,63.0,0.5),0),
    ] {
        let (shared, mut p) = setup(stone(), from.1); p.set_position(from.0, from.1, from.2); p.on_ground = false;
        assert!(!handle_client_packet(&movement(to,ground,false), &mut p,&shared).resync, "{from:?} -> {to:?}");
        assert_eq!(position(&p),to);
    }
}

#[test]
fn footprint_edges_negative_coordinates_and_unloaded_columns_are_distinct() {
    let (shared, mut p) = setup(stone(),65.0);
    for (x, blocked) in [(1.3,false),(1.3-1e-5,true),(-0.3,false),(-0.3+1e-5,true)] {
        p.x=x;
        shared.world_in(p.dimension).lock().unwrap().set_block_silently(-1,64,0,world::AIR);
        let reaction=handle_client_packet(&movement((x,64.875,0.5),0,false),&mut p,&shared);
        assert_eq!(reaction.resync,blocked,"x={x}"); p.y=65.0;
    }
    // Центр уже за краем камня: пересечение даёт только край полного основания.
    let (shared, mut p)=setup(stone(),65.0); p.x=15.9;
    {
        let mut world=shared.world_in(p.dimension).lock().unwrap();
        world.set_block_silently(15,64,0,stone());
        assert_eq!(crossing(&world,(15.9,65.0,0.5),(15.9,64.875,0.5),1.8),Crossing::Uncovered);
        assert!(world.get_block_loaded(16,64,0).is_none());
    }
    assert!(!handle_client_packet(&movement((15.9,64.875,0.5),0,false),&mut p,&shared).resync);
    {
        let mut world=shared.world_in(p.dimension).lock().unwrap(); world.set_block_silently(16,64,0,world::AIR);
    }
    p.y=65.0;
    assert!(handle_client_packet(&movement((15.9,64.875,0.5),0,false),&mut p,&shared).resync);
    // Отрицательный камень использует floor, а не усечение к нулю.
    let (shared,mut p)=setup(stone(),65.0); p.x = -0.5;
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(-1,64,0,stone());
    assert!(handle_client_packet(&movement((-0.5,64.875,0.5),0,false),&mut p,&shared).resync);
}

#[test]
fn existing_overlap_exits_and_outside_scope_keep_previous_acceptance() {
    let (shared,mut p)=setup(stone(),64.875);
    for to in [(0.5,65.125,0.5),(0.5,64.75,0.5),(1.5,64.875,0.5)] {
        p.set_position(0.5,64.875,0.5);
        assert!(!handle_client_packet(&movement(to,0,false),&mut p,&shared).resync); assert_eq!(position(&p),to);
    }
    let catalog=Catalog::new(Profile::Preview26_3);
    for state in [blocks::state_by_name("bamboo").unwrap(),blocks::state_by_name("cactus").unwrap(),
        blocks::state_by_name("moss_carpet").unwrap(),catalog.slab_state_for(&WOOL_SLABS[0],SlabHalf::Bottom,true).unwrap()] {
        let (shared,mut p)=setup(state,65.0);
        assert!(!handle_client_packet(&movement((0.5,64.875,0.5),0,false),&mut p,&shared).resync);
    }
    let (shared,mut p)=setup(stone(),69.125);
    assert!(!handle_client_packet(&movement((0.5,64.875,0.5),0,false),&mut p,&shared).resync);
}

#[test]
fn catalog_dry_slab_geometry_and_crouching_share_the_bounded_guard() {
    let catalog=Catalog::new(Profile::Preview26_3);
    for row in WOOL_SLABS.iter().chain(CONCRETE_SLABS.iter()) {
        for (half,feet) in [(SlabHalf::Bottom,64.5),(SlabHalf::Top,65.0),(SlabHalf::Double,65.0)] {
            let state=catalog.slab_state_for(row,half,false).unwrap();
            let (shared,mut p)=setup(state,feet); p.sneaking=true; p.game_mode=GAME_MODE_ADVENTURE;
            assert!(handle_client_packet(&movement((0.5,feet-0.125,0.5),0,false),&mut p,&shared).resync);
        }
    }
    let native=Catalog::new(Profile::Java26_1_2);
    assert_eq!(shape(native,WOOL_SLABS[0].default_state),None);
}

#[test]
fn old_teleport_vehicle_sleep_piston_and_unsupported_pose_guards_keep_their_order() {
    for mode in [GAME_MODE_SPECTATOR,GAME_MODE_CREATIVE] {
        let (shared,mut p)=setup(stone(),65.0); p.game_mode=mode; p.creative_flying=mode==GAME_MODE_CREATIVE;
        assert!(!handle_client_packet(&movement((0.5,64.875,0.5),0,false),&mut p,&shared).resync); assert_eq!(p.y,64.875);
    }
    for pose in 0..5 {
        let (shared,mut p)=setup(stone(),65.0);
        match pose { 0=>p.glide.active=true,1=>p.net.spin_pose=true,2=>p.net.spin_since=Some(1),3=>p.net.in_water=true,_=>p.net.in_lava=true }
        assert!(!handle_client_packet(&movement((0.5,64.875,0.5),0,false),&mut p,&shared).resync); assert_eq!(p.y,64.875);
    }
    for guard in 0..3 {
        let (shared,mut p)=setup(stone(),65.0);
        match guard { 0=>p.awaiting_teleport=true,1=>p.ride.vehicle=Some(999),_=>p.sleeping=Some((0,64,0)) }
        assert!(!handle_client_packet(&movement((0.5,64.875,0.5),0,true),&mut p,&shared).resync); assert_eq!(p.y,65.0);
        assert_eq!(p.yaw,if guard==2 {37.0} else {0.0});
    }
    let (shared,mut p)=setup(stone(),65.0); p.shoved_to=Some((0.5,64.875,0.5));
    assert!(!handle_client_packet(&movement((0.5,64.875,0.5),0,false),&mut p,&shared).resync); assert_eq!(p.y,64.875); assert!(p.shoved_to.is_none());
    let (shared,mut p)=setup(stone(),65.0); p.shoved_to=Some((0.5,65.5,0.5));
    assert!(handle_client_packet(&movement((0.5,64.875,0.5),0,true),&mut p,&shared).resync); assert_eq!(p.y,65.0); assert_eq!(p.yaw,37.0); assert!(p.shoved_to.is_none());
}

#[test]
fn observed_near_contact_samples_keep_candidate_xyz_and_previous_ground_only_for_tiny_descent() {
    // Ordinary stone24 samples: это не измерение точной границы между1e-7 и1e-5.
    for (depth,rejected) in [(1e-7,false),(1e-5,true),(1e-4,true),(0.001,true),(0.01,true),(0.0625,true)] {
        for flag in [0,1] {
            for rotation in [false,true] {
                let (shared,mut p)=setup(stone(),65.0);
                let to=(0.5,65.0-depth,0.5);
                let reaction=handle_client_packet(&movement(to,flag,rotation),&mut p,&shared);
                assert_eq!(reaction.resync,rejected,"depth={depth} flag={flag} rotation={rotation}");
                assert_eq!(position(&p),if rejected {(0.5,65.0,0.5)} else {to});
                assert!(p.on_ground); assert_eq!(p.moved_since_tp,!rejected);
                assert_eq!(p.vit.fall,0.0); assert!(p.net.hits.is_empty() && p.net.landings.is_empty());
                publish_player(&shared,&p); assert_eq!(published(&shared),p.position());
            }
        }
    }
}

#[test]
fn near_contact_retention_does_not_infer_airborne_zero_motion_or_unsupported_ground() {
    for rotation in [false,true] {
        // Та же геометрия, но прежний groundfalse: клиентский0 сохраняется.
        let (shared,mut p)=setup(stone(),65.0); p.on_ground=false;
        assert!(!handle_client_packet(&movement((0.5,65.0-1e-7,0.5),0,rotation),&mut p,&shared).resync);
        assert!(!p.on_ground); assert_eq!(p.y,65.0-1e-7);
        // Неподвижный пакет не является направленным пересечением верха.
        let (shared,mut p)=setup(stone(),65.0);
        assert!(!handle_client_packet(&movement((0.5,65.0,0.5),0,rotation),&mut p,&shared).resync); assert!(!p.on_ground);
        // Неизвестная форма не доказывает даже малое касание.
        let (shared,mut p)=setup(blocks::state_by_name("cactus").unwrap(),65.0);
        assert!(!handle_client_packet(&movement((0.5,65.0-1e-7,0.5),0,rotation),&mut p,&shared).resync); assert!(!p.on_ground);
        // Начало в воздухе на.5 выше пола не превращаем в подтверждённое касание.
        let (shared,mut p)=setup(stone(),65.5); p.on_ground=false;
        assert!(!handle_client_packet(&movement((0.5,65.0-1e-7,0.5),0,rotation),&mut p,&shared).resync); assert!(!p.on_ground);
    }
}
