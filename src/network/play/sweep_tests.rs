// Без сокетов: тот же удар идёт из Java и через Body из Bedrock.
use super::*;
use crate::mob::{Event, Kind, Mob, Particle};

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/sweep-ops.json")), crate::skins::Settings::default())
}

fn sword(level: i32) -> Stack {
    let mut data = crate::stackdata::Data::default();
    for (name, value) in [("knockback", 3), ("sweeping_edge", level)] {
        let id = crate::stackdata::ENCHANTMENTS.iter().position(|(n, _)| *n == name).unwrap();
        data.set_enchant(id as u16, value);
    }
    Stack::new(blocks::item_named("wooden_sword").unwrap(), 1).with_extras(data)
}

fn player() -> PlayerState {
    let mut p = PlayerState::new([41;16], "Sweep".into(), None, None, 0x7f, 0, (3.5,64.0,3.5));
    p.entity_id = 41;
    p.game_mode = GAME_MODE_SURVIVAL;
    p.on_ground = true;
    p.awaiting_teleport = false;
    p.yaw = 180.0;
    p.inventory.set(p.inventory.selected_slot(), Some(sword(0)));
    p
}

#[test]
fn sweep_conditions_and_movement_expire_after_stopping() {
    let mut p = player();
    assert!(sweep::eligible(&p, true, false, 100));
    assert!(!sweep::eligible(&p, false, false, 100));
    assert!(!sweep::eligible(&p, true, true, 100));
    p.on_ground = false;
    assert!(!sweep::eligible(&p, true, false, 100));
    p.on_ground = true;
    p.vit.sprinting = true;
    assert!(!sweep::eligible(&p, true, false, 100));
    p.vit.sprinting = false;
    p.net.sweep_motion.moved(100, 0.1, 0.0);
    assert!(sweep::eligible(&p, true, false, 100));
    p.net.sweep_motion.moved(100, 0.1, 0.0);
    p.net.sweep_motion.moved(100, 0.0, 0.0);
    assert!(!sweep::eligible(&p, true, false, 100));
    assert!(!sweep::eligible(&p, true, false, 101));
    assert!(sweep::eligible(&p, true, false, 102));
    p.inventory.set(p.inventory.selected_slot(), Some(Stack::new(blocks::item_named("wooden_axe").unwrap(), 1)));
    assert!(!sweep::eligible(&p, true, false, 102));
}

#[test]
fn sweep_damage_and_box_boundaries() {
    for (level, expected) in [(0,1.0), (1,3.0), (2,1.0+8.0/3.0), (3,4.0)] {
        assert!((pvp::sweep_damage(4.0, &sword(level).extras()) - expected).abs() < 1e-6);
    }
    let main = pvp::MainTarget { entity_id: 2, feet: (0.0,64.0,1.0), size: (0.6,1.8) };
    let from = (0.0,64.0,0.0);
    assert!(pvp::in_sweep_box(&main,(1.59,64.0,1.0),(0.6,1.8),from));
    assert!(!pvp::in_sweep_box(&main,(1.6,64.0,1.0),(0.6,1.8),from));
    assert!(!pvp::in_sweep_box(&main,(0.0,66.05,1.0),(0.6,1.8),from));
    let far = pvp::MainTarget { feet: (0.0,64.0,2.9), ..main };
    assert!(!pvp::in_sweep_box(&far,(0.0,64.0,3.0),(0.6,1.8),from));
}

#[test]
fn enchanted_sword_sweeps_from_both_protocols_with_both_knockbacks() {
    for bedrock in [false,true] {
        let shared = shared();
        shared.ticks.send_replace(100);
        let mut p = player();
        // Робот прислал false, но стоит на каменном полу.
        p.on_ground = false;
        shared.world_in(p.dimension).lock().unwrap().set_block_silently(3,63,3,blocks::state_by_name("stone").unwrap());
        let reader = shared.next_reader();
        let from = {
            let mut items = shared.items_in(p.dimension).lock().unwrap();
            for (id,at) in [(100,(3.5,64.0,1.5)), (101,(4.4,64.0,1.5)), (102,(6.0,64.0,1.5))] {
                let mut m = Mob::new(id,Kind::Cow,at); m.no_ai = true; m.on_ground = true; items.add_mob(m);
            }
            items.watch_changes(reader)
        };
        if bedrock {
            let mut body = body::Body::new(p.uuid,p.name.clone(),p.entity_id);
            let stance = body::Stance { game_mode:p.game_mode,feet:(p.x,p.y,p.z),yaw:p.yaw,pitch:0.0,on_ground:false,sneaking:false,dimension:p.dimension };
            body.act(&shared,&stance,&mut p.inventory,&mut p.vit,&mut p.last_attack,body::Act::Attack(100));
        } else { attack(100,&mut p,&shared); }
        let mut items = shared.items_in(p.dimension).lock().unwrap();
        let (events,_) = items.changes_since(from,reader);
        assert!(events.iter().any(|e| matches!(e,crate::items::Change::MobEvent { event:Event::PlayerSound { name:"minecraft:entity.player.attack.sweep", .. }, .. })));
        assert!(!events.iter().any(|e| matches!(e,crate::items::Change::MobEvent { event:Event::PlayerSound { name:"minecraft:entity.player.attack.strong", .. }, .. })));
        let particle = events.iter().find_map(|e| match e {
            crate::items::Change::MobEvent { event:Event::Particle { kind:Particle::SweepAttack,at,offset,speed,count }, .. } => Some((at,offset,speed,count)), _ => None,
        }).expect("частица взмаха");
        assert!((particle.0.0-3.5).abs()<1e-6 && (particle.0.1-64.9).abs()<1e-6 && (particle.0.2-2.5).abs()<1e-6);
        assert!(particle.1.0.abs()<1e-6); assert_eq!((particle.1.1,particle.1.2,*particle.2,*particle.3),(0.0,-1.0,0.0,0));
        let secondary = &items.mobs()[1];
        assert_eq!(secondary.vitals.health,9.0);
        let distance = 0.9f64.hypot(2.0);
        assert!((secondary.vz - (-0.4 - 0.2 * 2.0 / distance)).abs()<1e-6);
        assert!((secondary.vx - 0.2 * 0.9 / distance).abs()<1e-6);
        assert_eq!(items.mobs()[2].vitals.health,10.0);
        assert!(crate::bedrock::effects::combat_particles(67,None,*particle.0).is_some());
    }
}

#[test]
fn combat_support_does_not_ground_a_jump_or_a_hovering_player() {
    let shared = shared();
    let mut p = player(); p.on_ground = false;
    let mut world = shared.world_in(p.dimension).lock().unwrap();
    world.set_block_silently(3,63,3,blocks::state_by_name("stone").unwrap());
    drop(world);
    p.net.last_delta.1 = 0.1;
    sweep::settle(&shared,&mut p); assert!(!p.on_ground);
    p.net.last_delta.1 = 0.0; p.y += 0.1;
    sweep::settle(&shared,&mut p); assert!(!p.on_ground);
    p.y = 64.0; p.awaiting_teleport = true;
    sweep::settle(&shared,&mut p); assert!(!p.on_ground);
    p.awaiting_teleport = false;
    sweep::settle(&shared,&mut p); assert!(p.on_ground);
}

#[test]
fn changing_to_a_sword_cannot_reuse_the_previous_weapons_charge() {
    let shared = shared();
    let mut p = player();
    p.inventory.set(p.inventory.selected_slot(),Some(Stack::new(blocks::item_named("wooden_axe").unwrap(),1)));
    vitals::update_equipment(&mut p,0);
    p.inventory.set(p.inventory.selected_slot(),Some(sword(0)));
    shared.ticks.send_replace(100);
    let reader = shared.next_reader();
    let from = {
        let mut items = shared.items_in(p.dimension).lock().unwrap();
        items.add_mob(Mob::new(100,Kind::Cow,(3.5,64.0,1.5)));
        items.add_mob(Mob::new(101,Kind::Cow,(4.4,64.0,1.5)));
        items.watch_changes(reader)
    };
    attack(100,&mut p,&shared);
    let mut items = shared.items_in(p.dimension).lock().unwrap();
    assert_eq!(items.mobs()[1].vitals.health,10.0);
    let (events,_) = items.changes_since(from,reader);
    assert!(!events.iter().any(|e|matches!(e,crate::items::Change::MobEvent { event:Event::Particle { kind:Particle::SweepAttack, .. }, .. })));
}
