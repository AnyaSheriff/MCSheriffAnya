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
    PacketInfo { id, length: body.len() as i32, body }
}

#[test]
fn creative_fall_still_crits_but_does_not_cross_game_modes() {
    let shared = shared();
    let mut p = player();
    p.y = 68.0;
    vitals::moved(&mut p, &shared, (3.5,72.0,1.5));
    assert_eq!(p.vit.fall, 4.0);
    assert!(pvp::may_crit(&p));
    assert_eq!(p.vit.health, 20.0);
    vitals::change_game_mode(&mut p, GAME_MODE_SURVIVAL);
    p.on_ground = true;
    p.y = 67.5;
    vitals::moved(&mut p, &shared, (3.5,68.0,1.5));
    assert!(p.net.hits.is_empty());
    assert_eq!(p.vit.health, 20.0);
    // Падение, начатое уже в survival, по-прежнему ранит.
    p.on_ground = false;
    p.y = 63.5;
    vitals::moved(&mut p, &shared, (3.5,67.5,1.5));
    vitals::change_game_mode(&mut p, GAME_MODE_SURVIVAL);
    p.on_ground = true;
    vitals::moved(&mut p, &shared, (3.5,63.5,1.5));
    assert!(p.net.hits.iter().any(|hit| hit.source == crate::survival::Source::Fall));
}

#[test]
fn teleport_guard_survives_ground_and_rotation_only_packets() {
    let shared = shared();
    let mut p = player();
    p.vit.fall = 8.0;
    position_body(&mut p, Sync::Command);
    assert_eq!(p.vit.fall, 0.0);
    handle_client_packet(&packet(CONFIRM_TELEPORTATION, &encode_varint(p.teleport)), &mut p, &shared);
    handle_client_packet(&packet(SET_PLAYER_ON_GROUND, &[0]), &mut p, &shared);
    let mut turn = Vec::new();
    push_f32(&mut turn, 0.0); push_f32(&mut turn, 0.0); turn.push(0);
    handle_client_packet(&packet(SET_PLAYER_ROTATION, &turn), &mut p, &shared);
    assert!(p.net.teleported);
    vitals::change_game_mode(&mut p, GAME_MODE_SURVIVAL);
    let mut place = Vec::new();
    for v in [3.5,60.0,1.5] { push_f64(&mut place, v); }
    place.push(1);
    handle_client_packet(&packet(SET_PLAYER_POSITION, &place), &mut p, &shared);
    assert!(!p.net.teleported);
    assert!(p.net.hits.is_empty());
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
