// Регрессии записей tmp/bb для 26.1.2; пакеты в памяти, без сокетов.
use super::*;

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/mismatch-ops.json")), crate::skins::Settings::default())
}

fn player() -> PlayerState {
    let mut p = PlayerState::new([71;16], "Mismatch".into(), None, None, 0x7f, 0, (3.5,64.0,1.5));
    p.entity_id = 71;
    p.game_mode = GAME_MODE_CREATIVE;
    p.awaiting_teleport = false;
    p
}

fn packet(id: i32, payload: &[u8]) -> PacketInfo {
    let mut body = encode_varint(id);
    body.extend_from_slice(payload);
    PacketInfo { id, length: body.len() as i32, body, teleport: None, foreign_inventory: false, legacy_inventory: None }
}

#[test]
fn foreign_click_resyncs_and_forged_predictions_cannot_create_items() {
    let shared=shared();
    for foreign in [false,true] {
        let mut p=player();p.game_mode=GAME_MODE_SURVIVAL;
        let stone=blocks::item_named("stone").unwrap();
        p.inventory.set(36,Some(Stack::new(stone,5)));
        // Клиент обещает64алмаза вместо взятых5камней. Его хэш не авторитетен.
        let mut payload=vec![0,0,0,36,0,0,1,0,36,1];
        payload.extend(encode_varint(blocks::item_named("diamond").unwrap()));
        payload.extend([64,0,0,0]);
        let mut packet=packet(CLICK_CONTAINER,&payload);packet.foreign_inventory=foreign;
        let reaction=handle_client_packet(&packet,&mut p,&shared);
        assert_eq!(p.inventory.slot(36),None);
        assert_eq!(p.inventory.cursor(),Some(Stack::new(stone,5)));
        if foreign {assert!(reaction.inventory);}
    }
    // Даже когда непонятное действие ничего не изменило, иностранный клиент
    // получает полный снимок, а родной путь сохраняет прежнюю реакцию.
    for foreign in [false,true] {
        let mut p=player();let mut packet=packet(CLICK_CONTAINER,&[99,0,0,36,0,0,0,0]);packet.foreign_inventory=foreign;
        assert_eq!(handle_client_packet(&packet,&mut p,&shared).inventory,foreign);
    }
}

#[test]
fn creative_fall_crosses_game_modes_without_resetting() {
    // fall-mode-teleport-reset: 12 блоков, из них часть в creative, урон 9.
    let shared = shared();
    let mut p = player();
    p.y = 72.0;
    vitals::moved(&mut p, &shared, (3.5,76.0,1.5));
    assert_eq!(p.vit.fall, 4.0);
    assert!(pvp::may_crit(&p));
    vitals::change_game_mode(&mut p, GAME_MODE_SURVIVAL);
    p.y = 65.2;
    vitals::moved(&mut p, &shared, (3.5,72.0,1.5));
    p.on_ground = true;
    p.y = 64.0;
    vitals::moved(&mut p, &shared, (3.5,65.2,1.5));
    assert_eq!(p.net.hits.len(), 1);
    assert_eq!(p.net.hits[0].amount, 9.0);
    assert_eq!(p.net.landings[0].1, 100);
    assert_eq!(p.vit.fall, 0.0);
}

#[test]
fn teleport_preserves_fall_but_does_not_count_the_coordinate_jump() {
    // fall-mode-teleport-reset: после /tp вниз накопленные 4 блока дают урон 1.
    let shared = shared();
    let mut p = player();
    p.game_mode = GAME_MODE_SURVIVAL;
    p.vit.fall = 4.0;
    position_body(&mut p, Sync::Command);
    assert_eq!(p.vit.fall, 4.0);
    handle_client_packet(&packet(CONFIRM_TELEPORTATION, &encode_varint(p.teleport)), &mut p, &shared);
    handle_client_packet(&packet(SET_PLAYER_ON_GROUND, &[0]), &mut p, &shared);
    let mut turn = Vec::new();
    push_f32(&mut turn, 0.0); push_f32(&mut turn, 0.0); turn.push(0);
    handle_client_packet(&packet(SET_PLAYER_ROTATION, &turn), &mut p, &shared);
    assert!(p.net.teleported);
    let mut place = Vec::new();
    for v in [3.5,60.0,1.5] { push_f64(&mut place, v); }
    place.push(1);
    handle_client_packet(&packet(SET_PLAYER_POSITION, &place), &mut p, &shared);
    assert!(!p.net.teleported);
    assert_eq!(p.vit.fall, 4.0);
    assert!(p.net.hits.is_empty());
    handle_client_packet(&packet(SET_PLAYER_ON_GROUND, &[1]), &mut p, &shared);
    assert_eq!(p.net.hits.len(), 1);
    assert_eq!(p.net.hits[0].amount, 1.0);
    assert_eq!(p.net.landings[0].1, 40);
}

#[tokio::test]
async fn mode_change_packets_follow_the_recording() {
    let mut p = player();
    let mut out = Outgoing::capture();
    vitals::game_mode_order(&mut out, &mut p, GAME_MODE_SURVIVAL, 10).await.unwrap();
    let mut bytes = out.queued();
    let mut ids = Vec::new();
    while !bytes.is_empty() { ids.push(read_packet(&mut bytes).await.unwrap().id); }
    assert_eq!(ids, [PLAYER_ABILITIES, GAME_EVENT, PLAYER_ABILITIES]);
    assert_eq!(p.attrs_touched, [(crate::attributes::BLOCK_INTERACTION_RANGE,12), (crate::attributes::ENTITY_INTERACTION_RANGE,12)]);
}

#[tokio::test]
async fn anvil_levels_do_not_award_total_experience_or_score() {
    // anvil-rename-repair: level=29/26/21, total=0, без Metadata 18.
    let shared = shared();
    let mut p = player();
    p.vit.command_xp(true, 30, true);
    for (cost, level) in [(1,29), (3,26), (5,21)] {
        p.vit.command_xp(false, -cost, true);
        let mut out = Outgoing::capture();
        vitals::send_experience(&mut out, &p.vit).await.unwrap();
        let mut bytes = out.queued();
        let packet = read_packet(&mut bytes).await.unwrap();
        let mut cursor = Cursor::new(packet.payload());
        assert_eq!(cursor.bytes(4), Some(&0f32.to_be_bytes()[..]));
        assert_eq!(cursor.varint(), Some(level));
        assert_eq!(cursor.varint(), Some(0));
        let mut out = Outgoing::capture();
        vitals::tick(&mut out, &shared, &mut p).await.unwrap();
        let mut bytes = out.queued();
        while !bytes.is_empty() { assert_ne!(read_packet(&mut bytes).await.unwrap().id, SET_ENTITY_DATA); }
    }
}

#[tokio::test]
async fn ordinary_fall_announces_observer_packets_once_in_the_movement_tick() {
    // pvp-mace-landing-count: порядок звуков и Damage Event при уроне 1.
    let shared = shared();
    let mut p = player();
    p.game_mode = GAME_MODE_SURVIVAL;
    p.vit.fall = 4.0;
    p.on_ground = true;
    let stone = blocks::state_by_name("stone").unwrap();
    let reader = shared.next_reader();
    let start = {
        let mut world = shared.world_in(p.dimension).lock().unwrap();
        world.set_block_silently(3,63,1,stone);
        world.watch_events(reader)
    };
    vitals::moved(&mut p, &shared, (3.5,64.0,1.5));
    assert_eq!(p.net.announced_health, Some(19.0));
    let mut out = Outgoing::capture();
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    assert_eq!(p.vit.health,19.0);
    let mut world = shared.world_in(p.dimension).lock().unwrap();
    world.publish_events();
    let (events,_,_) = world.published_events_since(start,reader);
    let mut order = Vec::new();
    for event in events {
        match event {
            WorldEvent::Particle { count, .. } => { assert_eq!(count,40); order.push("particle"); }
            WorldEvent::Sound(sound) => { assert_eq!(sound.except,Some(p.entity_id)); order.push(sound.name); }
            WorldEvent::PlayerDamaged { type_id, .. } => { assert_eq!(type_id,10); order.push("damage"); }
            _ => {}
        }
    }
    assert_eq!(order,["particle","minecraft:entity.player.small_fall","minecraft:block.stone.fall","damage","minecraft:entity.player.hurt"]);
}

#[test]
fn swept_cow_matches_recorded_velocity_and_preserves_existing_motion() {
    for (vx, vz) in [(0.0,0.0), (0.8,-0.4)] {
        let mut p = player();
        p.yaw = 0.0;
        let mut items = crate::items::Items::new();
        let mut cow = crate::mob::Mob::new(90,crate::mob::Kind::Cow,(4.5,64.0,3.5));
        cow.no_ai = true;
        cow.vx = vx; cow.vz = vz;
        items.add_mob(cow);
        let main = pvp::MainTarget { entity_id: 91, feet: (3.5,64.0,3.5), size: (0.6,1.8) };
        pvp::sweep_mobs(&mut items,&mut p,&main,1.0);
        let cow = &items.mobs()[0];
        assert_eq!(cow.vitals.health,9.0);
        assert!((cow.vx - (vx / 4.0 + 0.2 / 5.0f64.sqrt())).abs() < 1e-8);
        assert!((cow.vz - (vz / 4.0 + 0.4 / 5.0f64.sqrt() + 0.4)).abs() < 1e-8);
        assert_eq!(cow.vy,0.0);
        let before = (cow.vx,cow.vy,cow.vz);
        pvp::sweep_mobs(&mut items,&mut p,&main,1.0);
        let cow = &items.mobs()[0];
        assert_eq!((cow.vx,cow.vy,cow.vz),before);
    }
}

#[test]
fn sweeping_edge_attribute_is_synced_and_removed_with_weapon() {
    let mut p = player();
    let mut data = crate::stackdata::Data::default();
    data.set_enchant(crate::stackdata::enchantment_number("sweeping_edge").unwrap(),3);
    p.inventory.set(p.inventory.selected_slot(),Some(Stack::new(blocks::item_named("wooden_sword").unwrap(),1).with_extras(data)));
    vitals::update_equipment(&mut p,0);
    let id = crate::attributes::SWEEPING_DAMAGE_RATIO;
    assert_eq!(p.attrs.value(id),0.75);
    assert!(crate::attributes::syncable(id));
    assert!(p.attrs_applied.contains(&(id,"minecraft:enchantment.sweeping_edge/mainhand".into())));
    p.inventory.set(p.inventory.selected_slot(),None);
    vitals::update_equipment(&mut p,1);
    assert_eq!(p.attrs.value(id),0.0);
}

#[tokio::test]
async fn sweeping_edge_resends_with_attack_speed_after_durability_change() {
    let shared = shared();
    let mut p = player();
    let mut data = crate::stackdata::Data::default();
    data.set_enchant(crate::stackdata::enchantment_number("sweeping_edge").unwrap(),3);
    let slot = p.inventory.selected_slot();
    let sword = Stack::new(blocks::item_named("wooden_sword").unwrap(),1).with_extras(data.clone());
    p.inventory.set(slot,Some(sword));
    for tick in 0..=2 {
        shared.ticks.send_replace(tick);
        vitals::tick(&mut Outgoing::capture(),&shared,&mut p).await.unwrap();
    }
    data.damage = 1;
    p.inventory.set(slot,Some(sword.with_extras(data)));
    for tick in 3..=5 {
        shared.ticks.send_replace(tick);
        let mut out = Outgoing::capture();
        vitals::tick(&mut out,&shared,&mut p).await.unwrap();
        let mut bytes = out.queued();
        let mut attributes = Vec::new();
        while !bytes.is_empty() {
            let packet = read_packet(&mut bytes).await.unwrap();
            if packet.id == 0x83 {
                let mut cursor = Cursor::new(packet.payload());
                assert_eq!(cursor.varint(),Some(p.entity_id));
                for _ in 0..cursor.varint().unwrap() {
                    attributes.push(cursor.varint().unwrap());
                    cursor.bytes(8).unwrap();
                    for _ in 0..cursor.varint().unwrap() {
                        let len = cursor.varint().unwrap() as usize;
                        cursor.bytes(len + 9).unwrap();
                    }
                }
            }
        }
        if tick == 5 { assert_eq!(attributes,vec![crate::attributes::ATTACK_SPEED,crate::attributes::SWEEPING_DAMAGE_RATIO]); }
        else { assert!(attributes.is_empty()); }
    }
}

#[test]
fn smash_landing_particles_are_published_in_the_movement_tick() {
    let shared = shared();
    let mut p = player();
    p.game_mode = GAME_MODE_SURVIVAL;
    p.vit.fall = 9.0;
    p.vit.smash_landing = true;
    p.vit.smash_forgives = true;
    p.on_ground = true;
    let mut world = shared.world_in(p.dimension).lock().unwrap();
    world.set_block_silently(3,63,1,blocks::state_by_name("stone").unwrap());
    let reader = shared.next_reader();
    let start = world.watch_events(reader);
    drop(world);
    vitals::moved(&mut p,&shared,(3.5,64.5,1.5));
    let mut world = shared.world_in(p.dimension).lock().unwrap();
    world.publish_events();
    let (events,_,_) = world.published_events_since(start,reader);
    let counts: Vec<_> = events.iter().filter_map(|e| match e {
        WorldEvent::Particle { count, skip, .. } if *skip == p.entity_id => Some(*count), _ => None,
    }).collect();
    assert_eq!(counts,vec![200,90]);
    assert_eq!(p.net.landings.len(),2);
    assert!(p.net.hits.is_empty());
}

#[tokio::test]
async fn teleported_minecart_tracker_matches_rail_detector_phases() {
    for (delay, expected_tick, expected_vy) in [(1,3,-0.056),(5,6,-0.019),(15,15,0.0),(25,27,-0.056),(35,36,-0.019)] {
        let shared = shared();
        let mut p = player();
        let cart = crate::minecart::Minecart::new(90,crate::minecart::Kind::Plain,(0.5,64.0,0.5));
        p.seen_carts.insert(90,ShownCart::of(&cart));
        shared.items_in(p.dimension).lock().unwrap().add_cart(cart);
        let stone = blocks::state_by_name("stone").unwrap();
        let rail = blocks::state_from_text("detector_rail[powered=false,shape=north_south,waterlogged=false]").unwrap();
        let view = |x,y,z| if y == 63 && z < 8 { stone } else if (x,y,z) == (0,64,0) { rail } else { world::AIR };
        let mut first = None;
        let mut on_pad = true;
        for tick in 0..=delay+3 {
            {
                let mut items = shared.items_in(p.dimension).lock().unwrap();
                if tick == delay {
                    let cart = &mut items.carts_mut()[0];
                    cart.z = 30.5; cart.y = 64.0; cart.vy = 0.0; cart.on_ground = true; cart.teleports += 1;
                }
                crate::minecart::step_all(items.carts_mut(),&view,&[], |_| true);
            }
            let mut out = Outgoing::capture();
            send_cart_moves(&mut out,&shared,&mut p).await.unwrap();
            let mut bytes = out.queued();
            while !bytes.is_empty() {
                let packet = read_packet(&mut bytes).await.unwrap();
                if tick >= delay && on_pad && packet.id == 0x65 {
                    first.get_or_insert((tick,p.seen_carts[&90].speed.1));
                }
                if packet.id == TELEPORT_ENTITY && p.seen_carts[&90].place.2 > 8.0 { on_pad = false; }
            }
        }
        if expected_vy == 0.0 { assert!(first.is_none()); }
        else {
            let (tick,vy) = first.expect("скорость перед переносом за площадку");
            assert_eq!(tick,expected_tick);
            assert!((vy-expected_vy).abs()<0.001,"{delay}: {vy}");
        }
    }
}


#[test]
fn confirmation_rejects_stale_duplicate_and_malformed_acknowledgements() {
    let shared = shared();
    let mut p = player();
    position_body(&mut p, Sync::Absolute);
    for payload in [encode_varint(p.teleport - 1), vec![], vec![1, 0], vec![255; 6]] {
        handle_client_packet(&packet(CONFIRM_TELEPORTATION, &payload), &mut p, &shared);
        assert!(p.awaiting_teleport);
    }
    handle_client_packet(&packet(CONFIRM_TELEPORTATION, &encode_varint(p.teleport)), &mut p, &shared);
    assert!(!p.awaiting_teleport);
    assert!(!accept_teleport(&packet(CONFIRM_TELEPORTATION, &encode_varint(p.teleport)), &mut p));
}

#[test]
fn modern_confirmation_applies_pose_without_counting_a_walk_or_fall() {
    use mcsa_protocol::teleport::{Confirmation, Pose};
    let shared = shared();
    let mut p = player();
    p.vit.fall = 4.0;
    position_body(&mut p, Sync::Command);
    let mut ack = packet(CONFIRM_TELEPORTATION, &encode_varint(p.teleport));
    ack.teleport = Some(Confirmation { id: p.teleport, pose: Some(Pose { x: 17.5, y: 81.0, z: 10.5, yaw: 47.0, pitch: 0.0 }) });
    handle_client_packet(&ack, &mut p, &shared);
    assert_eq!((p.x, p.y, p.z, p.yaw, p.pitch), (17.5, 81.0, 10.5, 47.0, 0.0));
    assert!(!p.awaiting_teleport && !p.moved_since_tp && p.net.teleported);
    assert_eq!(p.vit.fall, 4.0);
    assert!(p.net.hits.is_empty());
    p.x = 18.0;
    handle_client_packet(&ack, &mut p, &shared); // повтор уже принятого ответа
    assert_eq!(p.x, 18.0);
    position_body(&mut p, Sync::Command);
    for x in [f64::NAN, f64::INFINITY, 30_000_001.0] {
        ack.teleport = Some(Confirmation { id: p.teleport, pose: Some(Pose { x, y: 81.0, z: 10.5, yaw: 0.0, pitch: 0.0 }) });
        handle_client_packet(&ack, &mut p, &shared);
        assert!(p.awaiting_teleport);
        assert_eq!(p.x, 18.0);
    }
}

#[tokio::test]
async fn initial_confirmation_waits_for_the_right_id_and_keeps_client_settings() {
    use mcsa_protocol::{JavaVersion, Negotiated, wire, teleport::{Confirmation, Pose}};
    use super::super::connection::CryptoStream;
    use super::super::packet::{frame, compression_threshold, write_bytes};
    for version in [JavaVersion::V26_1_2, JavaVersion::V26_2, JavaVersion::V26_3] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let mut client = CryptoStream::new(client);
        let mut server = CryptoStream::new(listener.accept().await.unwrap().0);
        server.protocol = Some(Negotiated { client: version, native: wire::INTERNAL });
        server.probe_776 = version == JavaVersion::V26_2;
        client.enable_encryption(&[19; 16]); server.enable_encryption(&[19; 16]);
        let pose = (version == JavaVersion::V26_3).then_some(Pose { x: 17.5, y: 81.0, z: 10.5, yaw: 47.0, pitch: 0.0 });
        let mut frames = Vec::new();
        let mut wrong = vec![0]; wrong.extend(Confirmation { id: 16, pose }.encode(version).unwrap());
        frame(&mut frames, &wrong, compression_threshold());
        let mut settings = encode_varint(CLIENT_INFORMATION);
        settings.extend([5]); settings.extend(b"en_us"); settings.extend([2, 0, 1, 127, 0, 0, 1, 0]);
        frame(&mut frames, &settings, compression_threshold());
        let mut correct = vec![0]; correct.extend(Confirmation { id: 17, pose }.encode(version).unwrap());
        frame(&mut frames, &correct, compression_threshold());
        write_bytes(&mut client, &frames).await.unwrap();
        let mut p = player(); p.teleport = 17; p.awaiting_teleport = true;
        let mut out = Outgoing::new(&mut server);
        let look = read_confirm_teleportation(&mut out, &mut p).await.unwrap().unwrap();
        assert_eq!(look.main_hand, 0);
        assert_eq!(look.skin_parts, 127);
        assert!(!p.awaiting_teleport);
        if let Some(pose) = pose { assert_eq!((p.x, p.y, p.z, p.yaw), (pose.x, pose.y, pose.z, pose.yaw)); }
        else { assert_eq!(p.x, 3.5); }
    }
}

#[tokio::test]
async fn play_login_sends_the_real_online_flag_before_secure_chat() {
    use mcsa_protocol::{JavaVersion, Negotiated, wire};
    use super::super::connection::CryptoStream;
    for online in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let mut client = CryptoStream::new(client);
        let mut server = CryptoStream::new(listener.accept().await.unwrap().0);
        server.protocol = Some(Negotiated { client: JavaVersion::V26_3, native: wire::INTERNAL });
        client.enable_encryption(&[23; 16]); server.enable_encryption(&[23; 16]);
        let mut out = Outgoing::new(&mut server);
        send_login(&mut out, Dimension::Overworld, GAME_MODE_SURVIVAL, 7, 10, 10, online).await.unwrap();
        out.flush().await.unwrap();
        let received = read_packet(&mut client).await.unwrap();
        assert_eq!(received.id, 0x32);
        assert_eq!(&received.body[received.body.len() - 2..], &[u8::from(online), 0]);
    }
}
