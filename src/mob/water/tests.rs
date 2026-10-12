use super::*;
use crate::items::Items;

struct Pool { wet:bool, biome:&'static str, substrate:&'static str }
impl View for Pool {
    fn block(&self,_x:i32,y:i32,_z:i32)->i32 {
        blocks::state_by_name(if y<0 {self.substrate} else if self.wet {"water"} else {"air"}).unwrap()
    }
    fn loaded(&self,_x:i32,_z:i32)->bool {true}
    fn biome(&self,_x:i32,_y:i32,_z:i32)->&'static str {self.biome}
}
fn pool()->Pool {Pool{wet:true,biome:"lush_caves",substrate:"clay"}}
fn ctx()->ai::Ctx<'static> {ai::Ctx{players:&[],mates:&[],difficulty:2,sky:15,sun_up:false}}
fn player()->ai::Watcher {ai::Watcher{id:100,x:0.5,y:1.0,z:1.0,eye:1.62,holding:[None,None],spectator:false,targetable:true,look:(0.0,0.0),pumpkin: false, sprinting: false, riding: false, gold_armor: false}}

#[test]
fn all_tropical_patterns_are_unique_and_roundtrip() {
    let mut seen=std::collections::HashSet::new();
    for shape in 0..2 {for pattern in 0..6 {for base in 0..16 {for overlay in 0..16 {
        let value=pack_variant(shape,pattern,base,overlay).unwrap();
        assert!(seen.insert(value));
        assert_eq!(unpack_variant(value),Some((shape,pattern,base,overlay)));
    }}}}
    assert_eq!(seen.len(),3072);
    assert_eq!(pack_variant(0,6,0,0),None);
    assert_eq!(pack_variant(2,0,0,0),None);
    assert_eq!(unpack_variant(-1),None);
    assert_eq!(COMMON.iter().map(|(_,v)|*v).collect::<std::collections::HashSet<_>>().len(),22);
    assert!(COMMON.iter().all(|(_,v)|unpack_variant(*v).is_some()));
    assert_eq!(COMMON.iter().find(|(name,_)|*name=="Clownfish").unwrap().1,65536);
    assert_eq!(COMMON.iter().find(|(name,_)|*name=="Blue Dory").unwrap().1,185008129);
}

#[test]
fn bucket_roundtrip_preserves_variant_health_age_name_and_flags() {
    for kind in [Kind::Cod,Kind::Salmon,Kind::TropicalFish,Kind::Pufferfish,Kind::Axolotl,Kind::Tadpole] {
        let mut fish=Mob::new(1,kind,(0.5,1.0,0.5));
        fish.water.variant=if kind==Kind::TropicalFish {pack_variant(1,5,14,7).unwrap()} else {4};
        fish.water.salmon_size=2;
        fish.water.growth=1234;
        fish.water.hunt_cooldown=120;
        fish.vitals.health=2.5;
        fish.no_ai=true; fish.silent=true; fish.age = -123;
        fish.name=Some("Bucket fish".into());
        let data=bucket::capture(&fish);
        let bytes=crate::stackdata::to_bytes(&data);
        let saved=crate::stackdata::from_bytes(&bytes).unwrap();
        assert_eq!(data,saved);
        let mut other=Mob::new(2,kind,(2.5,1.0,0.5));
        bucket::release(&mut other,&saved);
        assert!(other.water.from_bucket && other.persistent_tag && other.no_ai && other.silent);
        assert_eq!(other.vitals.health,2.5);
        assert_eq!(other.name,fish.name);
        if matches!(kind,Kind::TropicalFish|Kind::Axolotl) {assert_eq!(other.water.variant,fish.water.variant);}
        if kind==Kind::Salmon {assert_eq!(other.water.salmon_size,2);}
        if kind==Kind::Tadpole {assert_eq!(other.water.growth,1234);} else {assert_eq!(other.age,-123);}
        let nbt=saved.bucket.as_ref().unwrap().nbt();
        assert_eq!(bucket::BucketData::from_nbt(&nbt),saved.bucket);
    }
}

#[test]
fn aquatic_save_record_preserves_home_and_bucket_origin() {
    for kind in Kind::ALL.into_iter().filter(|kind|creature(*kind)) {
        let mut mob=Mob::new(1,kind,(2.5,60.0,2.5));
        mob.water.home=(50,63,-8); mob.water.has_egg=true;
        mob.water.from_bucket=true; mob.water.variant=4;
        mob.water.salmon_size=2; mob.water.growth=22000;
        mob.persistent_tag=true; mob.name=Some("Aquatic".into());
        let mut bytes=Vec::new(); mob.write(&mut bytes);
        let mut input=bytes.as_slice(); let other=Mob::read(&mut input).unwrap();
        assert!(input.is_empty());
        assert_eq!(other.kind,kind); assert_eq!(other.water.home,(50,63,-8));
        assert!(other.water.has_egg && other.water.from_bucket && other.persistent_tag);
        assert_eq!(other.water.growth,22000);
        assert_eq!(other.water.variant,4);
        assert_eq!(other.name,mob.name);
    }
}

#[test]
fn fish_air_resets_and_dry_damage_uses_cooldown() {
    let mut cod=Mob::new(1,Kind::Cod,(0.5,1.0,0.5)); cod.no_ai=true;
    let dry=Pool{wet:false,..pool()}; let mut rng=Rng::new(1);
    for _ in 0..319 {cod.tick(&dry,&ctx(),&mut rng);}
    assert_eq!(cod.vitals.health,3.0);
    cod.tick(&dry,&ctx(),&mut rng); assert_eq!(cod.vitals.health,1.0);
    cod.tick(&pool(),&ctx(),&mut rng); assert_eq!(cod.water.air,300);
    for _ in 0..100 {cod.tick(&pool(),&ctx(),&mut rng);}
    assert!(!cod.is_dead());
}

#[test]
fn puffer_inflates_deflates_and_stings_without_nausea() {
    let mut fish=Mob::new(1,Kind::Pufferfish,(0.5,1.0,0.5));
    let players=[player()]; let near=ai::Ctx{players:&players,..ctx()}; let mut rng=Rng::new(2);
    think(&mut fish,&pool(),&near,&mut rng); assert_eq!(fish.water.puff,1);
    for _ in 0..40 {think(&mut fish,&pool(),&near,&mut rng);}
    assert_eq!(fish.water.puff,2);
    let mut mobs=vec![fish]; let mut out=super::super::Outcome::default();
    interactions(&mut mobs,&pool(),&players,&mut out,&mut rng);
    assert_eq!(out.stings.len(),1); assert_eq!(out.stings[0].3,2);
    assert!(out.water_effects.is_empty());
    for _ in 0..61 {think(&mut mobs[0],&pool(),&ctx(),&mut rng);}
    assert_eq!(mobs[0].water.puff,1);
    for _ in 0..40 {think(&mut mobs[0],&pool(),&ctx(),&mut rng);}
    assert_eq!(mobs[0].water.puff,0);
}

#[test]
fn squid_ink_and_glow_darkening_are_events() {
    for kind in [Kind::Squid,Kind::GlowSquid] {
        let mut mob=Mob::new(3,kind,(0.5,1.0,0.5));
        assert!(mob.hurt(1.0,Source::PlayerAttack,None).is_some());
        assert_eq!(mob.events.iter().filter(|e|matches!(e,Event::Particle{count:0,speed,..} if *speed==0.1)).count(),30);
        assert_eq!(mob.water.dark_ticks,if kind==Kind::GlowSquid {100} else {0});
    }
}

#[test]
fn axolotl_play_dead_only_in_water_and_recovers() {
    let mut played=None;
    for id in 1..100 {
        let mut mob=Mob::new(id,Kind::Axolotl,(0.5,1.0,0.5));
        // До удара 6 из 14 — меньше половины: притворяется в трети случаев (чёрный ящик axolotl-play-dead).
        mob.water.wet=true; mob.vitals.health=6.0;
        mob.hurt(1.0,Source::MobAttack,Some(Push{cause:100,dir:(0.0,0.0),strength:0.0}));
        if mob.water.dead_pending>0 {played=Some(mob);break;}
    }
    let mut mob=played.expect("ниже половины здоровья притворяется");
    let health=mob.vitals.health;
    let mut rng=Rng::new(1);
    // Метка — через такт после удара, Регенерация — ещё через такт.
    mob.tick(&pool(),&ctx(),&mut rng); assert_eq!(mob.water.playing_dead,0);
    mob.tick(&pool(),&ctx(),&mut rng); assert!(mob.water.playing_dead>0);
    mob.tick(&pool(),&ctx(),&mut rng);
    assert!(mob.vitals.has_effect(EffectKind::Regeneration));
    for _ in 0..197 {mob.tick(&pool(),&ctx(),&mut rng);}
    assert!(mob.water.playing_dead>0 && mob.vitals.health>health);
    for _ in 0..3 {mob.tick(&pool(),&ctx(),&mut rng);}
    assert_eq!(mob.water.playing_dead,0);
    // Выше половины и без случая — не притворяется.
    let mut healthy=Mob::new(2,Kind::Axolotl,(0.5,1.0,0.5)); healthy.water.wet=true; healthy.vitals.health=14.0;
    healthy.hurt(0.0,Source::MobAttack,None); assert_eq!(healthy.water.dead_pending,0);
    let mut dry=Mob::new(1,Kind::Axolotl,(0.5,1.0,0.5)); dry.vitals.health=5.0;
    dry.hurt(1.0,Source::MobAttack,None); assert_eq!(dry.water.playing_dead,0);
}

#[test]
fn frogs_eat_only_small_cubes_and_drop_correct_froglight() {
    for variant in 0..3 {
        let mut frog=Mob::new(1,Kind::Frog,(0.5,1.0,0.5)); frog.water.variant=variant;
        let mut cube=Mob::new(2,Kind::MagmaCube,(1.0,1.0,0.5)); cube.nether.size=1;
        let mut mobs=vec![frog,cube]; let mut out=super::super::Outcome::default(); let mut rng=Rng::new(1);
        for _ in 0..200 {interactions(&mut mobs,&pool(),&[],&mut out,&mut rng); if mobs[1].h.despawn {break;}}
        assert!(mobs[1].h.despawn);
        for _ in 0..4 {interactions(&mut mobs,&pool(),&[],&mut out,&mut rng);}
        assert!(mobs[1].events.iter().any(|e|matches!(e,Event::Loot(stacks) if stacks[0].item==blocks::item_named(froglight(variant)).unwrap())));
        mobs[1].h.despawn=false; mobs[1].nether.size=2; mobs[0].water.attack_cooldown=0;
        for _ in 0..20 {interactions(&mut mobs,&pool(),&[],&mut out,&mut rng);}
        assert!(!mobs[1].h.despawn);
    }
}

#[test]
fn tadpole_grows_into_biome_frog_and_keeps_name() {
    for (biome,variant) in [("plains",0),("desert",1),("snowy_plains",2)] {
        let mut tad=Mob::new(1,Kind::Tadpole,(0.5,1.0,0.5)); tad.water.growth=GROW_TADPOLE-1; tad.name=Some("Tad".into());
        let mut items=Items::new(); items.add_mob(tad); let mut id=10;
        let env=super::super::Env{difficulty:2,sky:15,sun_up:false,clock:0,tick:0};
        super::super::step_all(&mut items,&Pool{biome,..pool()},&[],env,&mut Rng::new(1),&mut || {id+=1;id});
        assert_eq!(items.mobs().len(),1); let frog=&items.mobs()[0];
        assert_eq!(frog.kind,Kind::Frog); assert_eq!(frog.water.variant,variant);
        assert_eq!(frog.name.as_deref(),Some("Tad"));
    }
}

#[test]
fn spawn_rules_require_dark_water_or_lush_clay() {
    let stone=Pool{substrate:"stone",..pool()};
    assert!(spawn::allowed(&stone,Kind::GlowSquid,(0,1,0),"plains",(0,0)));
    assert!(spawn::allowed(&pool(),Kind::GlowSquid,(0,0,0),"lush_caves",(0,0)));
    assert!(!spawn::allowed(&pool(),Kind::GlowSquid,(0,0,0),"deep_dark",(0,0)));
    assert!(!spawn::allowed(&stone,Kind::GlowSquid,(0,1,0),"plains",(0,1)));
    assert!(!spawn::allowed(&stone,Kind::GlowSquid,(0,30,0),"plains",(0,0)));
    assert!(spawn::allowed(&pool(),Kind::Axolotl,(0,1,0),"lush_caves",(0,0)));
    assert!(!spawn::allowed(&stone,Kind::Axolotl,(0,1,0),"lush_caves",(0,0)));
    assert!(!spawn::allowed(&pool(),Kind::Axolotl,(0,1,0),"ocean",(0,0)));
    assert!(!spawn::table("frozen_ocean").iter().any(|e|e.kind==Kind::Dolphin));
    assert!(spawn::table("river").iter().any(|e|e.kind==Kind::Salmon));
}

#[test]
fn turtle_eggs_hatch_only_on_sand_and_queue_babies() {
    let mut world=crate::world::World::in_memory();
    world.set_time_of_day(21500);
    let egg=blocks::state_from_text("turtle_egg[eggs=4,hatch=2]").unwrap();
    world.set_block_silently(0,64,0,egg);
    world.set_block_silently(0,63,0,blocks::state_by_name("stone").unwrap());
    eggs::turtle_tick(&mut world,(0,64,0),egg); assert!(world.take_loaded_mobs().is_empty());
    world.set_block_silently(0,63,0,blocks::state_by_name("sand").unwrap());
    eggs::turtle_tick(&mut world,(0,64,0),egg);
    let babies=world.take_loaded_mobs(); assert_eq!(babies.len(),4);
    assert!(babies.iter().all(|m|m.kind==Kind::Turtle && m.is_baby() && m.water.home==(0,64,0)));
    assert_eq!(world.get_block(0,64,0),crate::world::AIR);
}

#[test]
fn turtle_maturity_drops_one_scute_not_on_death() {
    let mut turtle=Mob::new(1,Kind::Turtle,(0.5,1.0,0.5)); turtle.age = -1; turtle.no_ai=true;
    turtle.tick(&pool(),&ctx(),&mut Rng::new(1));
    assert!(turtle.events.iter().any(|e|matches!(e,Event::Loot(s) if blocks::item_name(s[0].item)==Some("turtle_scute") && s[0].count==1)));
    turtle.events.clear(); turtle.tick(&pool(),&ctx(),&mut Rng::new(1));
    assert!(turtle.events.is_empty());
    assert!(loot_for(&turtle).iter().all(|s|blocks::item_name(s.item)!=Some("turtle_scute")));
}
fn loot_for(mob:&Mob)->Vec<Stack> {super::super::loot(mob,&mut Rng::new(1))}

#[test]
fn no_ai_tadpole_still_grows_and_retains_no_ai_after_conversion() {
    let mut tadpole=Mob::new(1,Kind::Tadpole,(0.5,1.0,0.5));
    tadpole.no_ai=true;
    tadpole.water.growth=GROW_TADPOLE-1;
    let mut items=Items::new(); items.add_mob(tadpole);
    let env=super::super::Env{difficulty:2,sky:15,sun_up:false,clock:0,tick:0};
    super::super::step_all(&mut items,&pool(),&[],env,&mut Rng::new(1),&mut ||2);
    assert_eq!(items.mobs().len(),1);
    assert_eq!(items.mobs()[0].kind,Kind::Frog);
    assert!(items.mobs()[0].no_ai);
}

#[test]
fn dry_axolotl_and_submerged_dolphin_have_separate_timers() {
    let dry=Pool{wet:false,..pool()};
    let mut axolotl=Mob::new(1,Kind::Axolotl,(0.5,1.0,0.5));
    axolotl.water.moisture=1;
    for _ in 0..20 {vitals(&mut axolotl,&dry);}
    assert_eq!(axolotl.vitals.health,14.0);
    vitals(&mut axolotl,&dry);
    assert_eq!(axolotl.vitals.health,12.0);
    sense(&mut axolotl,&pool()); vitals(&mut axolotl,&pool());
    assert_eq!(axolotl.water.moisture,DRY_AXOLOTL);
    let mut dolphin=Mob::new(2,Kind::Dolphin,(0.5,1.0,0.5));
    sense(&mut dolphin,&pool()); dolphin.water.air=1;
    for _ in 0..21 {vitals(&mut dolphin,&pool());}
    assert_eq!(dolphin.vitals.health,8.0);
    assert_eq!(dolphin.water.moisture,DOLPHIN_MOISTURE);
    sense(&mut dolphin,&dry); vitals(&mut dolphin,&dry);
    assert_eq!(dolphin.water.air,DOLPHIN_AIR);
}

#[test]
fn axolotl_assist_follows_a_player_kill_once_and_stacks_to_two_minutes() {
    let mut axolotl=Mob::new(1,Kind::Axolotl,(0.5,1.0,0.5));
    axolotl.water.target=Some(2);
    let mut enemy=Mob::new(2,Kind::Drowned,(1.5,1.0,0.5));
    enemy.hurt(100.0,Source::PlayerAttack,Some(Push{cause:100,dir:(0.0,0.0),strength:0.0}));
    let mut mobs=vec![axolotl,enemy]; let mut out=super::super::Outcome::default();
    interactions(&mut mobs,&pool(),&[player()],&mut out,&mut Rng::new(1));
    interactions(&mut mobs,&pool(),&[player()],&mut out,&mut Rng::new(1));
    assert_eq!(out.axolotl_assists,vec![100]);
    let mut vitals=Mob::new(3,Kind::Axolotl,(0.0,0.0,0.0)).vitals;
    assert_eq!(assist_effect(&vitals).ticks_left,100);
    vitals.give_effect(EffectKind::Regeneration,0,150);
    assert_eq!(assist_effect(&vitals).ticks_left,250);
    vitals.give_effect(EffectKind::Regeneration,0,2390);
    assert_eq!(assist_effect(&vitals).ticks_left,2400);
}

#[test]
fn dolphin_pod_retaliates_and_breathing_overrides_following_boats() {
    let mut hit=Mob::new(1,Kind::Dolphin,(0.5,1.0,0.5));
    hit.hurt(1.0,Source::PlayerAttack,Some(Push{cause:100,dir:(0.0,0.0),strength:0.0}));
    let neighbour=Mob::new(2,Kind::Dolphin,(2.5,1.0,0.5));
    let far=Mob::new(3,Kind::Dolphin,(100.5,1.0,0.5));
    let mut mobs=vec![hit,neighbour,far];
    interactions(&mut mobs,&pool(),&[player()],&mut super::super::Outcome::default(),&mut Rng::new(1));
    assert_eq!(mobs[1].water.target,Some(100));
    assert!(mobs[1].water.anger>0);
    assert_eq!(mobs[2].water.target,None);
    let dolphin=&mut mobs[2]; dolphin.water.boat=Some((99.0,1.0,4.0));
    think(dolphin,&pool(),&ctx(),&mut Rng::new(1));
    assert_eq!(dolphin.water.swim_to,dolphin.water.boat);
    dolphin.water.wet=true; dolphin.water.air=100;
    think(dolphin,&pool(),&ctx(),&mut Rng::new(1));
    assert_eq!(dolphin.water.swim_to,Some((100.5,3.0,0.5)));
}

#[test]
fn underwater_breeding_makes_eggs_or_inherited_axolotl_baby() {
    for kind in [Kind::Turtle,Kind::Frog,Kind::Axolotl] {
        let mut items=Items::new();
        for id in 1..=2 {
            let mut mob=Mob::new(id,kind,(id as f64,1.0,0.5));
            mob.love=super::super::LOVE_TICKS; mob.water.variant=id;
            items.add_mob(mob);
        }
        let env=super::super::Env{difficulty:2,sky:15,sun_up:false,clock:0,tick:0};
        let mut rng=Rng::new(13); let mut id=10;
        for _ in 0..61 {
            super::super::step_all(&mut items,&pool(),&[],env,&mut rng,&mut ||{id+=1;id});
        }
        let mobs=items.mobs();
        assert!(mobs[..2].iter().all(|m|m.love==0 && m.age>0),"{kind:?}");
        if kind==Kind::Axolotl {
            assert_eq!(mobs.len(),3);
            assert!(mobs[2].is_baby());
            assert!(matches!(mobs[2].water.variant,1|2|4));
        } else {
            assert_eq!(mobs.len(),2);
            assert_eq!(mobs.iter().filter(|m|m.water.has_egg).count(),1);
        }
    }
}

#[test]
fn frogspawn_hatches_two_to_five_tadpoles_and_needs_water() {
    let mut world=crate::world::World::in_memory();
    let spawn=blocks::state_by_name("frogspawn").unwrap();
    world.set_block_silently(0,64,0,spawn);
    eggs::frog_tick(&mut world,(0,64,0));
    assert!(world.take_loaded_mobs().is_empty());
    world.set_block_silently(0,63,0,blocks::state_by_name("water").unwrap());
    world.set_block_silently(0,64,0,spawn);
    eggs::frog_tick(&mut world,(0,64,0));
    let tadpoles=world.take_loaded_mobs();
    assert!((2..=5).contains(&tadpoles.len()));
    assert!(tadpoles.iter().all(|m|m.kind==Kind::Tadpole && m.water.growth==0));
    assert_eq!(world.get_block(0,64,0),crate::world::AIR);
    eggs::frog_tick(&mut world,(0,64,0));
    assert!(world.take_loaded_mobs().is_empty());
}

#[test]
fn turtle_egg_cracks_and_breaks_one_at_a_time() {
    let mut world=crate::world::World::in_memory();
    world.set_time_of_day(21500);
    world.set_block_silently(0,63,0,blocks::state_by_name("sand").unwrap());
    world.set_block_silently(0,64,0,blocks::state_from_text("turtle_egg[eggs=2,hatch=0]").unwrap());
    for crack in 1..=2 {
        let state=world.get_block(0,64,0);
        eggs::turtle_tick(&mut world,(0,64,0),state);
        assert_eq!(world.get_block(0,64,0),blocks::state_from_text(&format!("turtle_egg[eggs=2,hatch={crack}]")).unwrap());
        assert!(world.take_loaded_mobs().is_empty());
    }
    assert!(eggs::break_one(&mut world,(0,64,0)));
    assert_eq!(world.get_block(0,64,0),blocks::state_from_text("turtle_egg[eggs=1,hatch=2]").unwrap());
    assert!(eggs::break_one(&mut world,(0,64,0)));
    assert!(!eggs::break_one(&mut world,(0,64,0)));
    assert!(!eggs::hatch_window(21061));
    assert!(eggs::hatch_window(21062));
    assert!(eggs::hatch_window(21904));
    assert!(!eggs::hatch_window(21905));
}

#[test]
fn impaling_applies_to_aquatic_mobs_but_not_drowned_or_frogs() {
    for kind in [Kind::Cod,Kind::Axolotl,Kind::Turtle,Kind::Tadpole,Kind::Guardian,Kind::ElderGuardian] {
        assert_eq!(impaling(kind,3),7.5);
    }
    for kind in [Kind::Drowned,Kind::Frog,Kind::Zombie] { assert_eq!(impaling(kind,3),0.0); }
    assert_eq!(impaling(Kind::Cod,0),0.0);
}

#[test]
fn guardians_attack_aquatic_prey_but_forget_playing_dead_axolotls() {
    let mut guardian=Mob::new(1,Kind::Guardian,(0.5,1.0,0.5));
    guardian.guardian.target=Some(2);
    guardian.guardian.beam=super::super::guardian::BEAM_GUARDIAN-1;
    let mut axolotl=Mob::new(2,Kind::Axolotl,(5.0,1.0,0.5));
    axolotl.no_ai=true;
    let mut items=Items::new(); items.add_mob(guardian); items.add_mob(axolotl);
    let env=super::super::Env{difficulty:2,sky:15,sun_up:false,clock:0,tick:0};
    let outcome=super::super::step_all(&mut items,&pool(),&[],env,&mut Rng::new(1),&mut || 100);
    assert!(outcome.strikes.is_empty());
    assert_eq!(items.mobs().iter().find(|m|m.entity_id==2).unwrap().vitals.health,8.0);
    items.mobs_mut().iter_mut().find(|m|m.entity_id==2).unwrap().water.playing_dead=100;
    assert!(predator_targets(items.mobs(),&[],true).is_empty());
    assert!(predator_targets(items.mobs(),&[],false).is_empty());
    super::super::step_all(&mut items,&pool(),&[],env,&mut Rng::new(2),&mut || 101);
    let guardian=items.mobs().iter().find(|m|m.entity_id==1).unwrap();
    assert_eq!(guardian.guardian.target,None);
    assert_eq!(guardian.guardian.after,0);
}

#[test]
fn egg_blocks_do_not_drop_as_normal_loot() {
    for name in ["frogspawn","turtle_egg"] {
        let state=blocks::state_by_name(name).unwrap();
        assert!(blocks::drops_by_luck(state,None,0.0).is_none());
    }
}

#[test]
fn frogspawn_accepts_sources_and_waterlogged_blocks_but_not_flowing_water() {
    assert!(eggs::supports_frogspawn(blocks::state_by_name("water").unwrap()));
    assert!(eggs::supports_frogspawn(blocks::state_by_name("seagrass").unwrap()));
    assert!(eggs::supports_frogspawn(blocks::state_from_text("oak_slab[type=bottom,waterlogged=true]").unwrap()));
    assert!(!eggs::supports_frogspawn(blocks::state_from_text("water[level=1]").unwrap()));
    assert!(!eggs::supports_frogspawn(blocks::state_by_name("stone").unwrap()));
}

#[test]
fn pufferfish_recognizes_frog_as_non_aquatic_but_ignores_cod() {
    let mut fish=Mob::new(1,Kind::Pufferfish,(0.5,1.0,0.5));
    let mut frog=Mob::new(2,Kind::Frog,(0.7,1.0,0.5)); frog.no_ai=true;
    let cod=Mob::new(3,Kind::Cod,(0.7,1.0,0.5));
    let mates=[ai::Mate{id:2,kind:Kind::Frog,x:0.7,y:1.0,z:0.5,love:0,baby:false,ready:true,tamed:false}];
    think(&mut fish,&pool(),&ai::Ctx{mates:&mates,..ctx()},&mut Rng::new(1));
    assert_eq!(fish.water.puff,1);
    let mut mobs=vec![fish,frog,cod];
    interactions(&mut mobs,&pool(),&[],&mut super::super::Outcome::default(),&mut Rng::new(1));
    assert!(mobs[1].vitals.has_effect(EffectKind::Poison));
    assert_eq!(mobs[2].vitals.health,3.0);
}

#[test]
fn frogspawn_uses_delayed_block_queue_instead_of_immediate_neighbour_tick() {
    let mut world=crate::world::World::in_memory();
    world.set_block_silently(0,63,0,blocks::state_by_name("water").unwrap());
    world.set_block(0,64,0,blocks::state_by_name("frogspawn").unwrap());
    crate::farming::on_neighbour(&mut world,(0,64,0));
    let mut hatched=None;
    for tick in 1..=12000 {
        for (at,kind) in world.advance(100) {
            if kind==crate::world::TickKind::Block {crate::redstone::ticked(&mut world,at);}
            else {world.begin_tick(at,kind);}
        }
        let babies=world.take_loaded_mobs();
        if !babies.is_empty() {hatched=Some((tick,babies.len()));break;}
    }
    let (tick,count)=hatched.expect("икра должна получить отложенный такт");
    assert!((3600..=12000).contains(&tick));
    assert!((2..=5).contains(&count));
}
