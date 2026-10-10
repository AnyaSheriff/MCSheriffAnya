use super::*;
use crate::mob::{newblocks, newmobs};

#[test]
fn endermite_lifetime_and_name_are_independent_of_normal_despawn() {
    let mut m=Mob::new(1,Kind::Endermite,(0.5,1.0,0.5));
    m.newmobs.lifetime=2398;
    newmobs::lifecycle(&mut m,&Flat::new()); assert!(!m.h.despawn);
    newmobs::lifecycle(&mut m,&Flat::new()); assert!(m.h.despawn);
    let mut named=Mob::new(2,Kind::Endermite,(0.5,1.0,0.5));
    named.name=Some("Mite".into()); named.newmobs.lifetime=2399;
    newmobs::lifecycle(&mut named,&Flat::new()); assert!(!named.h.despawn);
}

#[test]
fn all_three_nether_mobs_convert_after_fifteen_seconds() {
    for (kind,to) in [(Kind::Hoglin,Kind::Zoglin),(Kind::Piglin,Kind::ZombifiedPiglin),(Kind::PiglinBrute,Kind::ZombifiedPiglin)] {
        let mut m=Mob::new(1,kind,(0.0,1.0,0.0));
        let extra=usize::from(kind==Kind::PiglinBrute);
        for _ in 0..300+extra {newmobs::lifecycle(&mut m,&Flat::new());}
        assert_eq!(m.h.converted,None);
        newmobs::lifecycle(&mut m,&Flat::new()); assert_eq!(m.h.converted,Some(to));
        m.h.converted=None;m.nether.immune=true;
        for _ in 0..310 {newmobs::lifecycle(&mut m,&Flat::new());}
        assert_eq!(m.h.converted,None); assert_eq!(m.newmobs.outside,0);
    }
}

#[test]
fn returning_to_nether_resets_conversion() {
    struct Nether;
    impl View for Nether {
        fn block(&self,_:i32,_:i32,_:i32)->i32 {0}
        fn loaded(&self,_:i32,_:i32)->bool {true}
        fn piglin_safe(&self)->bool {true}
    }
    let mut m=Mob::new(1,Kind::Hoglin,(0.0,1.0,0.0));m.newmobs.outside=299;
    newmobs::lifecycle(&mut m,&Nether);assert_eq!(m.newmobs.outside,0);assert_eq!(m.h.converted,None);
}

fn watcher() -> ai::Watcher {
    ai::Watcher {id:2,x:0.5,y:1.0,z:0.5,eye:1.62,holding:[None;2],spectator:false,targetable:true,look:(0.0,0.0),pumpkin:false,gold_armor:false,sprinting:false,riding:false}
}

#[test]
fn creaking_freezes_for_any_witness_but_not_through_a_wall() {
    let mut m=Mob::new(1,Kind::Creaking,(0.5,1.0,5.5));
    let p=watcher();assert!(newmobs::watched(&m,&Flat::new(),&p));
    let mut away=p;away.look=(180.0,0.0);assert!(!newmobs::watched(&m,&Flat::new(),&away));
    let players=[away,p];let ctx=ai::Ctx{players:&players,..ctx()};
    m.vx=1.0;assert!(newmobs::freeze(&mut m,&Flat::new(),&ctx));assert_eq!(m.vx,0.0);
    let mut wall=Flat::new();wall.stone.extend([(0,2,3),(0,3,3)]);assert!(!newmobs::watched(&m,&wall,&p));
    let mut pumpkin=p;pumpkin.pumpkin=true;assert!(!newmobs::watched(&m,&Flat::new(),&pumpkin));
}

#[test]
fn linked_creaking_is_protected_until_heart_breaks() {
    let mut m=Mob::new(1,Kind::Creaking,(0.0,1.0,0.0));m.newmobs.heart=Some((2,2,2));
    assert!(m.hurt(100.0,Source::PlayerAttack,None).is_none());assert!(!m.is_dead());assert!(m.newmobs.resin_hit);
    newmobs::lifecycle(&mut m,&Flat::new());assert!(m.is_dead());
    let mut summoned=Mob::new(3,Kind::Creaking,(0.0,1.0,0.0));assert!(summoned.hurt(1.0,Source::PlayerAttack,None).unwrap().died);
}

#[test]
fn snowballs_accelerate_only_baby_happy_ghasts() {
    let mut m=Mob::new(1,Kind::HappyGhast,(0.0,1.0,0.0));m.age=BABY_AGE;
    assert_eq!(m.size(),(0.95,0.95));assert!(newmobs::feed_snowball(&mut m));assert_eq!(m.age,-21600);
    m.age = -1;run(&mut m,&Flat::new(),1);assert!(!m.is_baby());assert_eq!(m.size(),(4.0,4.0));assert!(!newmobs::feed_snowball(&mut m));
    for colour in newmobs::COLOURS { assert!(blocks::item_named(&format!("{colour}_harness")).is_some());assert!(newmobs::harness_colour(&format!("{colour}_harness")).is_some()); }
}

#[test]
fn wax_prevents_scraping_until_removed() {
    let mut m=Mob::new(1,Kind::CopperGolem,(0.0,1.0,0.0));m.newmobs.oxidation=2;
    assert!(newmobs::copper_use(&mut m,"honeycomb"));assert!(!newmobs::copper_use(&mut m,"honeycomb"));
    assert!(newmobs::copper_use(&mut m,"iron_axe"));assert!(!m.newmobs.waxed);assert_eq!(m.newmobs.oxidation,2);
    assert!(newmobs::copper_use(&mut m,"iron_axe"));assert_eq!(m.newmobs.oxidation,1);
}

#[test]
fn new_mobs_persist_lifetime_heart_harness_and_cargo() {
    for kind in [Kind::Endermite,Kind::Zoglin,Kind::Creaking,Kind::HappyGhast,Kind::CopperGolem] {
        let mut m=Mob::new(1,kind,(1.0,2.0,3.0));m.newmobs.home=Some((4.0,5.0,6.0));m.newmobs.lifetime=1234;m.newmobs.heart=Some((1,4,3));m.newmobs.harness=Some(11);m.newmobs.waxed=true;m.newmobs.oxidation=2;
        m.newmobs.cargo=Some(Stack::new(blocks::item_named("stone").unwrap(),16));m.name=Some("test".into());
        let mut out=Vec::new();m.write(&mut out);let mut input=out.as_slice();let read=Mob::read(&mut input).unwrap();
        assert!(input.is_empty());assert_eq!(read.kind,kind);assert_eq!(read.name,m.name);assert_eq!(read.newmobs.lifetime,1234);assert_eq!(read.newmobs.heart,m.newmobs.heart);assert_eq!(read.newmobs.cargo,m.newmobs.cargo);assert_eq!(read.newmobs.harness,Some(11));assert!(read.newmobs.waxed);assert_eq!(read.newmobs.home,m.newmobs.home);
    }
}

#[test]
fn copper_build_accepts_all_stages_and_orientations() {
    let mut world=crate::world::World::in_memory();
    for body in ["copper_block","waxed_exposed_copper","weathered_copper","oxidized_copper"] {
        world.set_block(0,1,0,blocks::state_by_name(body).unwrap());world.set_block(0,2,0,blocks::state_by_name("carved_pumpkin").unwrap());
        let b=golem::find(&crate::tick::MobView(&world),(0,2,0)).unwrap();assert_eq!(b.kind,Kind::CopperGolem);assert_eq!(b.clear.len(),2);
    }
}

#[test]
fn copper_sorter_preserves_items_and_rejects_unrelated_chests() {
    let mut world=crate::world::World::in_memory();let mut items=Items::new();let mut rng=Rng::new(7);
    let stone=blocks::item_named("stone").unwrap();let dirt=blocks::item_named("dirt").unwrap();
    world.set_block(0,1,0,blocks::state_by_name("copper_chest").unwrap());world.set_block(2,1,0,blocks::state_by_name("chest").unwrap());
    world.with_container(0,1,0,|c|c.slots[0]=Some(Stack::new(stone,32)));
    world.with_container(2,1,0,|c|c.slots[0]=Some(Stack::new(dirt,1)));
    items.add_mob(Mob::new(1,Kind::CopperGolem,(1.5,1.0,0.5)));
    for _ in 0..60 {newblocks::copper_tick(&mut world,&mut items,&mut rng);}
    assert_eq!(items.mobs()[0].newmobs.cargo.unwrap().count,16);assert_eq!(world.container(0,1,0).unwrap().slots[0].unwrap().count,16);
    for _ in 0..60 {newblocks::copper_tick(&mut world,&mut items,&mut rng);}
    assert_eq!(items.mobs()[0].newmobs.cargo.unwrap().count,16);
    world.with_container(2,1,0,|c|c.slots[0]=Some(Stack::new(stone,1)));items.mobs_mut()[0].newmobs.visited.clear();
    for _ in 0..60 {newblocks::copper_tick(&mut world,&mut items,&mut rng);}
    assert!(items.mobs()[0].newmobs.cargo.is_none());assert_eq!(world.container(2,1,0).unwrap().slots[0].unwrap().count,17);
}

#[test]
fn heart_requires_aligned_pale_logs() {
    let mut w=crate::world::World::in_memory();w.set_block(0,2,0,blocks::state_by_name("creaking_heart").unwrap());
    assert!(!newblocks::heart_supported(&w,(0,2,0)));
    for y in [1,3] {w.set_block(0,y,0,blocks::state_from_text("pale_oak_log[axis=y]").unwrap());}
    assert!(newblocks::heart_supported(&w,(0,2,0)));
    w.set_block(0,3,0,blocks::state_from_text("pale_oak_log[axis=x]").unwrap());assert!(!newblocks::heart_supported(&w,(0,2,0)));
}

#[test]
fn happy_ghast_has_four_distinct_seats() {
    let mut riding=crate::riding::Riding::new();
    for id in 1..=4 {assert!(riding.mount(id,10,4));}
    assert!(!riding.mount(5,10,4));assert_eq!(riding.driver(10),Some(1));
    let seats:Vec<_>=(0..4).map(|i|newmobs::seat((0.0,0.0,0.0),0.0,i)).collect();
    for i in 0..4 {for j in 0..i {assert_ne!(seats[i],seats[j]);}}
}

#[test]
fn hydration_completes_four_stages_and_drying_reverses_one() {
    let mut world=crate::world::World::in_memory();
    world.set_block(0,1,0,blocks::state_from_text("dried_ghast[hydration=0,waterlogged=true]").unwrap());
    for level in 1..=3 {
        newblocks::hydration_tick(&mut world,(0,1,0));
        let state=world.get_block(0,1,0);
        assert_eq!(blocks::orientation_of(state).unwrap().value(state,"hydration"),Some(level.to_string().as_str()));
        assert!(world.take_loaded_mobs().is_empty());
    }
    newblocks::hydration_tick(&mut world,(0,1,0));
    assert_eq!(blocks::block_at_state(world.get_block(0,1,0)),Some("water"));
    let spawned=world.take_loaded_mobs();assert_eq!(spawned.len(),1);assert_eq!(spawned[0].kind,Kind::HappyGhast);assert_eq!(spawned[0].age,BABY_AGE);
    world.set_block(0,1,0,blocks::state_from_text("dried_ghast[hydration=2,waterlogged=false]").unwrap());
    newblocks::hydration_tick(&mut world,(0,1,0));
    let state=world.get_block(0,1,0);assert_eq!(blocks::orientation_of(state).unwrap().value(state,"hydration"),Some("1"));
}

#[test]
fn endermen_hunt_endermites_and_zoglins_spare_creepers() {
    let mite=ai::Mate{id:3,kind:Kind::Endermite,x:1.0,y:1.0,z:0.5,love:0,baby:false,ready:true,tamed:false};
    let mut enderman=Mob::new(1,Kind::Enderman,(0.5,1.0,0.5));enderman.on_ground=true;enderman.ticks=12;enderman.newmobs.active=true;
    let mates=[mite];let ctx=ai::Ctx{mates:&mates,..ctx()};
    let plan=newmobs::hunt_mob(&mut enderman,&Flat::new(),&ctx,&mut Rng::new(1)).unwrap();assert_eq!(plan.bite.unwrap().0,3);assert!(plan.melee.is_none());
    let mut zoglin=Mob::new(2,Kind::Zoglin,(0.5,1.0,0.5));
    let mates=[ai::Mate{kind:Kind::Creeper,..mite}];let ctx=ai::Ctx{mates:&mates,..super::ctx()};
    assert!(newmobs::hunt_mob(&mut zoglin,&Flat::new(),&ctx,&mut Rng::new(1)).is_none());
}

#[test]
fn converted_hoglin_keeps_name_leash_baby_and_persistence() {
    let mut old=Mob::new(1,Kind::Hoglin,(1.0,2.0,3.0));old.name=Some("test".into());old.persistent_tag=true;old.age = -10;
    old.leash=Some(leash::Holder::Player{id:7,uuid:[5;16]});
    let converted=convert(&old,Kind::Zoglin,2);
    assert_eq!(converted.name,old.name);assert_eq!(converted.leash,old.leash);assert!(converted.persistent_tag);assert!(converted.is_baby());assert_eq!(converted.vitals.health,40.0);
    old.newmobs.outside=123;old.nether.immune=true;
    let mut out=Vec::new();old.write(&mut out);let mut input=out.as_slice();let read=Mob::read(&mut input).unwrap();
    assert_eq!(read.newmobs.outside,123);assert!(read.nether.immune);assert!(input.is_empty());
}

#[test]
fn statues_unwax_scrape_then_reanimate() {
    let mut world=crate::world::World::in_memory();world.set_block(0,1,0,blocks::state_by_name("waxed_oxidized_copper_golem_statue").unwrap());
    for expected in ["oxidized_copper_golem_statue","weathered_copper_golem_statue","exposed_copper_golem_statue","copper_golem_statue","air"] {
        assert!(newblocks::statue_use(&mut world,(0,1,0),"iron_axe"));assert_eq!(blocks::block_at_state(world.get_block(0,1,0)),Some(expected));
    }
    assert_eq!(world.take_loaded_mobs()[0].kind,Kind::CopperGolem);
}

#[test]
fn new_spawn_eggs_and_loot_are_registered() {
    for kind in [Kind::Endermite,Kind::Zoglin,Kind::Creaking,Kind::HappyGhast,Kind::CopperGolem] {
        assert_eq!(Kind::from_name(kind.name()),Some(kind));assert!(blocks::item_named(&format!("{}_spawn_egg",kind.name())).is_some());
    }
    let mut mite=Mob::new(1,Kind::Endermite,(0.0,1.0,0.0));mite.credit_player();mite.die();assert_eq!(mite.pending_xp,3);assert!(loot(&mite,&mut Rng::new(1)).is_empty());
    let mut happy=Mob::new(2,Kind::HappyGhast,(0.0,1.0,0.0));happy.credit_player();happy.die();assert!((1..=3).contains(&happy.pending_xp));
    let mut zoglin=Mob::new(3,Kind::Zoglin,(0.0,1.0,0.0));assert!(zoglin.hurt(40.0,Source::Lava,None).is_none());assert_eq!(zoglin.vitals.health,40.0);
}

#[test]
fn copper_sorter_moves_past_empty_source_chest() {
    let mut world=crate::world::World::in_memory();let mut items=Items::new();let mut rng=Rng::new(17);
    let stone=blocks::item_named("stone").unwrap();
    for x in [0,2] {world.set_block(x,1,0,blocks::state_by_name("copper_chest").unwrap());}
    world.with_container(2,1,0,|c|c.slots[0]=Some(Stack::new(stone,7)));
    items.add_mob(Mob::new(1,Kind::CopperGolem,(1.5,1.0,0.5)));
    for _ in 0..120 {newblocks::copper_tick(&mut world,&mut items,&mut rng);}
    assert_eq!(items.mobs()[0].newmobs.cargo.unwrap().count,7);
    assert!(world.container(2,1,0).unwrap().slots[0].is_none());
}

#[test]
fn heart_activates_without_mobs_and_releases_link_at_dawn() {
    let mut world=crate::world::World::in_memory();let mut items=Items::new();let mut rng=Rng::new(19);
    world.set_block(0,2,0,blocks::state_from_text("creaking_heart[axis=y]").unwrap());
    for y in [1,3] {world.set_block(0,y,0,blocks::state_from_text("pale_oak_log[axis=y]").unwrap());}
    let status=|w:&crate::world::World| {let state=w.get_block(0,2,0);blocks::orientation_of(state).unwrap().value(state,"creaking_heart_state")};
    world.set_time_of_day(18000);
    newblocks::blocks_tick(&mut world,&mut items,&[],false,&mut rng,&mut ||2);
    assert_eq!(status(&world),Some("awake"));
    let mut m=Mob::new(1,Kind::Creaking,(2.5,1.0,0.5));m.newmobs.heart=Some((0,2,0));items.add_mob(m);
    world.set_block(0,3,0,0);
    newblocks::blocks_tick(&mut world,&mut items,&[],false,&mut rng,&mut ||2);
    assert_eq!(status(&world),Some("awake"));assert!(!items.mobs()[0].h.despawn);
    world.set_time_of_day(1000);
    newblocks::blocks_tick(&mut world,&mut items,&[],false,&mut rng,&mut ||2);
    assert!(items.mobs()[0].h.despawn);
    newblocks::blocks_tick(&mut world,&mut items,&[],false,&mut rng,&mut ||2);
    assert_eq!(status(&world),Some("uprooted"));
}

#[test]
fn copper_oxidation_obeys_world_age_and_wax() {
    let mut world=crate::world::World::in_memory();let mut items=Items::new();let mut rng=Rng::new(3);
    world.set_block(0,0,0,blocks::state_by_name("stone").unwrap());
    let mut m=Mob::new(1,Kind::CopperGolem,(0.5,1.0,0.5));m.no_ai=true;m.newmobs.oxidize_at = -1;m.newmobs.waxed=true;items.add_mob(m);
    newblocks::copper_tick(&mut world,&mut items,&mut rng);assert_eq!(items.mobs()[0].newmobs.oxidation,0);
    items.mobs_mut()[0].newmobs.waxed=false;
    newblocks::copper_tick(&mut world,&mut items,&mut rng);assert_eq!(items.mobs()[0].newmobs.oxidation,1);
    newblocks::copper_tick(&mut world,&mut items,&mut rng);assert!((504000..=552000).contains(&items.mobs()[0].newmobs.oxidize_at));
}

#[test]
fn ghastling_follows_adults_and_returns_toward_its_home() {
    let mut m=Mob::new(1,Kind::HappyGhast,(0.5,8.0,0.5));m.age = -1000;
    let mates=[ai::Mate{id:2,kind:Kind::Cow,x:8.0,y:6.0,z:0.5,love:0,baby:false,ready:true,tamed:false}];
    let context=ai::Ctx{mates:&mates,..ctx()};
    newmobs::happy_tick(&mut m,&Flat::new(),&context,&mut Rng::new(3));
    assert!(m.newmobs.tempted);assert!(m.vx>0.0);assert_eq!(m.newmobs.home,Some((0.5,8.0,0.5)));
    m.age=0;m.newmobs.tempted=false;m.newmobs.home=Some((0.0,8.0,0.0));m.nether.wander=None;m.x=100.0;m.vx=0.0;
    newmobs::happy_tick(&mut m,&Flat::new(),&ctx(),&mut Rng::new(9));
    assert_eq!(m.nether.wander,Some((0.0,8.0,0.0)));assert!(m.vx<0.0);
}

#[test]
fn heart_loot_respects_silk_touch_and_fortune() {
    let state=blocks::state_by_name("creaking_heart").unwrap();
    let silk=newblocks::heart_drop(state,true,3,0.99).unwrap();
    assert_eq!(blocks::item_name(silk.item),Some("creaking_heart"));assert_eq!(silk.count,1);
    for fortune in 0..=3 {
        let min=newblocks::heart_drop(state,false,fortune,0.0).unwrap();
        let max=newblocks::heart_drop(state,false,fortune,0.999).unwrap();
        assert_eq!(blocks::item_name(min.item),Some("resin_clump"));assert_eq!(min.count,1);assert_eq!(max.count,3+fortune);
    }
}
