//! STEP policy проверяется через реальную подготовку/доставку и датчик.
//! Физику принятого движения эти unit не доказывают; mob parity не наблюдалась.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile, SlabHalf, WOOL_SLABS, CONCRETE_SLABS};
use crate::players::{Member, Position};
use crate::redstone::sculk_family::GameEvent;
use std::sync::atomic::{AtomicUsize, Ordering};

const SENSOR: (i32,i32,i32) = (3,65,4);
const UUID: [u8;16] = [113;16];

fn fixture(profile: Profile, state: i32, feet_y: f64) -> (Shared, World) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("mcsa-step-policy-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    let catalog = Catalog::new(profile);
    let shared = Shared::new(World::in_memory_with_catalog(catalog),World::in_memory_with_catalog(catalog),root.join("players"),root.join("skins"),Default::default(),crate::ops::Ops::open(&root.join("ops.json")),crate::skins::Settings::default());
    shared.players.lock().unwrap().join(Member {
        uuid: UUID, name: "StepPolicy".into(), game_mode: 0, entity_id: 113,
        skin: None, skin_parts: 0x7f, main_hand: 1, held: None,
        position: Position { x:0.5,y:feet_y,z:0.5,yaw:0.,pitch:0.,on_ground:true },
        dimension: Dimension::Overworld, sneaking:false,swings:0,shoved:((0.,0.,0.),0),unsettled:0,
    },113).unwrap();
    let mut world = World::in_memory_with_catalog(catalog);
    world.ensure(0,0);
    let stone = blocks::state_by_name("stone").unwrap();
    for x in 0..5 {
        world.set_block(x,63,0,stone);
        world.set_block(x,64,0,state);
        assert_eq!(world.get_block(x,64,0),state);
    }
    world.set_block(SENSOR.0,SENSOR.1-1,SENSOR.2,stone);
    let sensor = blocks::state_from_text("sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]").unwrap();
    world.set_block(SENSOR.0,SENSOR.1,SENSOR.2,sensor);
    assert_eq!(world.get_block(SENSOR.0,SENSOR.1,SENSOR.2),sensor);
    world.redstone.sculk.events.clear();
    (shared,world)
}
fn position(shared: &Shared, x: f64, y: f64, ground: bool) {
    shared.players.lock().unwrap().set_position(&UUID,Position {x,y,z:0.5,yaw:0.,pitch:0.,on_ground:ground});
}
fn sensor_active(world: &mut World) -> bool {
    for _ in 0..8 { redstone::sensors_tick(world); }
    let state = world.get_block(SENSOR.0,SENSOR.1,SENSOR.2);
    let orientation = blocks::orientation("sculk_sensor").unwrap();
    let active = orientation.value(state,"sculk_sensor_phase")==Some("active");
    assert_eq!(active,orientation.value(state,"power").unwrap().parse::<u8>().unwrap()>0);
    active
}
fn player_trial(profile: Profile,state:i32,y:f64,sneak:bool,moving:bool,landing:bool) -> (Vec<u8>,bool) {
    let (shared,mut world)=fixture(profile,state,y);
    shared.players.lock().unwrap().set_sneaking(&UUID,sneak);
    if landing { position(&shared,0.5,y+0.4,false); }
    sculk_prepare(&shared,Dimension::Overworld,&mut world);
    world.redstone.sculk.events.clear();
    position(&shared,if moving {2.5} else {0.5},y,true);
    sculk_prepare(&shared,Dimension::Overworld,&mut world);
    let events=world.redstone.sculk.events.iter().map(|e|e.frequency).collect();
    (events,sensor_active(&mut world))
}

#[test]
fn all_dry_wool_halves_suppress_actual_player_step_not_landing_or_wet() {
    let catalog=Catalog::new(Profile::Preview26_3);
    for row in &WOOL_SLABS {
        for half in [SlabHalf::Bottom,SlabHalf::Top,SlabHalf::Double] {
            let y=if half==SlabHalf::Bottom {64.5} else {65.};
            let dry=catalog.slab_state_for(row,half,false).unwrap();
            assert_eq!(player_trial(Profile::Preview26_3,dry,y,false,true,false),(vec![],false),"{} {half:?}",row.name);
            assert_eq!(player_trial(Profile::Preview26_3,dry,y,true,true,false),(vec![],false));
            assert_eq!(player_trial(Profile::Preview26_3,dry,y,false,false,false),(vec![],false));
            // Frequency2 сохраняет прежний путь; это unitконтроль, не ordinarylanding.
            assert_eq!(player_trial(Profile::Preview26_3,dry,y,false,true,true),(vec![2],true));
            let wet=catalog.slab_state_for(row,half,true).unwrap();
            assert_eq!(player_trial(Profile::Preview26_3,wet,y,false,true,false),(vec![1],true));
            assert_eq!(player_trial(Profile::Preview26_3,wet,y,false,true,true),(vec![2],true));
        }
    }
}

#[test]
fn native_carpet_contact_uses_top_sixteenth_but_preserves_moss_and_underlay_fallback() {
    for profile in [Profile::Java26_1_2,Profile::Preview26_3] {
        for name in ["white_carpet","red_carpet","black_carpet"] {
            let carpet=blocks::state_by_name(name).unwrap();
            assert_eq!(player_trial(profile,carpet,64.0625,false,true,false),(vec![],false));
            assert_eq!(player_trial(profile,carpet,64.0625,false,true,true),(vec![2],true));
            // Отступ от поверхности в той же клетке не включает новое правило.
            for wrong_y in [64.01,64.10,64.125] {
                assert_eq!(player_trial(profile,carpet,wrong_y,false,true,false),(vec![1],true));
            }
        }
        for name in ["moss_carpet","pale_moss_carpet"] {
            let carpet=blocks::state_by_name(name).unwrap();
            assert_eq!(player_trial(profile,carpet,64.0625,false,true,false),(vec![1],true));
            let (shared,mut world)=fixture(profile,carpet,64.0625);
            let wool=blocks::state_by_name("white_wool").unwrap();
            for x in 0..5 {world.set_block(x,63,0,wool);}
            sculk_prepare(&shared,Dimension::Overworld,&mut world);
            position(&shared,2.5,64.0625,true);
            sculk_prepare(&shared,Dimension::Overworld,&mut world);
            assert!(world.redstone.sculk.events.is_empty(),"старый woolunderlay не потерян");
        }
    }
}

#[test]
fn stone_concrete_sneak_noaction_native_and_peaceful_keep_previous_behavior() {
    for profile in [Profile::Java26_1_2,Profile::Preview26_3] {
        for name in ["stone","white_wool"] {
            let state=blocks::state_by_name(name).unwrap();let quiet=name=="white_wool";
            assert_eq!(player_trial(profile,state,65.,false,true,false),(if quiet {vec![]} else {vec![1]},!quiet));
            assert_eq!(player_trial(profile,state,65.,true,true,false),(vec![],false));
            assert_eq!(player_trial(profile,state,65.,false,false,false),(vec![],false));
        }
    }
    let catalog=Catalog::new(Profile::Preview26_3);
    for row in &CONCRETE_SLABS {
        for half in [SlabHalf::Bottom,SlabHalf::Top,SlabHalf::Double] {
            let y=if half==SlabHalf::Bottom {64.5} else {65.};
            assert_eq!(player_trial(Profile::Preview26_3,catalog.slab_state_for(row,half,false).unwrap(),y,false,true,false),(vec![1],true));
        }
    }
    let stone=blocks::state_by_name("stone").unwrap();let(shared,mut world)=fixture(Profile::Java26_1_2,stone,65.);
    assert!(!world.set_block(0,64,0,WOOL_SLABS[0].default_state));
    assert!(!world.set_block(0,64,0,i32::MAX));
    assert_eq!(world.get_block(0,64,0),stone,"native не принимает extension");
    shared.difficulty.store(crate::survival::PEACEFUL,Ordering::Relaxed);
    sculk_prepare(&shared,Dimension::Overworld,&mut world);position(&shared,2.5,65.,true);sculk_prepare(&shared,Dimension::Overworld,&mut world);
    assert!(!world.redstone.sculk.enabled,"peaceful выключает прежний warningchannel");
    assert_eq!(world.redstone.sculk.events.iter().map(|e|e.frequency).collect::<Vec<_>>(),vec![1]);
    assert!(sensor_active(&mut world),"peaceful sensorполитика не менялась");
}

#[test]
fn actual_mob_delivery_uses_step_policy_only_frequency_two_unchanged() {
    let catalog=Catalog::new(Profile::Preview26_3);
    for row in &WOOL_SLABS {
        for half in [SlabHalf::Bottom,SlabHalf::Top,SlabHalf::Double] {
            let y=if half==SlabHalf::Bottom {64.5} else {65.};
            for wet in [false,true] {
                for frequency in [1,2,3] {
                    let(shared,mut world)=fixture(Profile::Preview26_3,catalog.slab_state_for(row,half,wet).unwrap(),y);
                    shared.items_in(Dimension::Overworld).lock().unwrap().vibrations.push(GameEvent {at:(2.5,y,0.5),source:Some(991),frequency,projectile:false});
                    sculk_finish(&shared,Dimension::Overworld,&mut world,1);
                    let quiet=frequency==1&&!wet;
                    assert_eq!(world.redstone.sculk.events.len(),usize::from(!quiet));
                    assert_eq!(sensor_active(&mut world),!quiet);
                }
            }
        }
    }
    for (name,y,quiet) in [("white_carpet",64.0625,true),("moss_carpet",64.0625,false),("stone",65.,false)] {
        let(shared,mut world)=fixture(Profile::Java26_1_2,blocks::state_by_name(name).unwrap(),y);
        shared.items_in(Dimension::Overworld).lock().unwrap().vibrations.push(GameEvent {at:(2.5,y,0.5),source:None,frequency:1,projectile:false});
        sculk_finish(&shared,Dimension::Overworld,&mut world,1);assert_eq!(sensor_active(&mut world),!quiet);
    }
}
