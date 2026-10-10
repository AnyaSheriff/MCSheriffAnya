use super::*;
use crate::mob::skeleton as variant;
use crate::survival::EffectKind;

struct Snow(bool);

impl View for Snow {
    fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
        blocks::state_by_name(if y < 1 { "stone" } else if self.0 && y < 3 { "powder_snow" } else { "air" }).unwrap()
    }
    fn loaded(&self, _: i32, _: i32) -> bool { true }
}

#[test]
fn skeleton_variants_register_stats_eggs_and_sounds() {
    for (kind, name, hp, id) in [(Kind::Stray, "stray", 20.0, 128), (Kind::Bogged, "bogged", 16.0, 16)] {
        assert_eq!(Kind::from_name(&format!("minecraft:{name}")), Some(kind));
        assert!(blocks::item_named(&format!("{name}_spawn_egg")).is_some());
        assert_eq!(crate::spawner::egg_entity(&format!("{name}_spawn_egg")), Some(name));
        assert_eq!((kind.max_health(), kind.java_type(), kind.width(), kind.height(), kind.speed()), (hp, id, 0.6, 1.99, 0.25));
        let mob = Mob::new(9, kind, (0.5, 1.0, 0.5));
        assert!(kind.hostile() && mob.h.bow);
        assert_eq!(mob.eye(), 1.74);
        assert!(kind.sounds().0.contains(name));
        assert!(kind.step_sound().contains(name));
        assert!(hostile::killer_key(kind).ends_with(name));
    }
}

#[test]
fn variants_share_skeleton_aim_and_bogged_waits_thirty_ticks_longer() {
    let view = Flat::new();
    let players = [player_at(1, 8.5, 0.5)];
    for (kind, difficulty, gap) in [(Kind::Stray, 1, 60), (Kind::Stray, 2, 60), (Kind::Stray, 3, 40), (Kind::Bogged, 1, 90), (Kind::Bogged, 2, 90), (Kind::Bogged, 3, 70)] {
        let mut mob = Mob::new(9, kind, (0.5, 1.0, 0.5));
        let shots = hunt(&mut mob, &view, &ctx_for(&players, difficulty, 15, false), 400);
        assert!(shots.len() >= 4, "{kind:?}");
        assert!(shots.windows(2).all(|pair| pair[1].0 - pair[0].0 == gap), "{kind:?}: {shots:?}");
        assert_eq!(shots[0].1.shot.unwrap().speed, hostile::ARROW_SPEED);
    }
}

#[test]
fn skeleton_snow_requires_140_ticks_then_converts_after_300_and_leaving_cancels() {
    let mut mob = Mob::new(3, Kind::Skeleton, (0.5, 1.0, 0.5));
    for _ in 0..variant::FREEZE_TICKS - 1 { variant::snow_tick(&mut mob, &Snow(true)); }
    assert_eq!((mob.h.water_ticks, mob.h.converting), (variant::FREEZE_TICKS - 1, 0));
    variant::snow_tick(&mut mob, &Snow(false));
    assert_eq!(mob.h.water_ticks, 0);
    for _ in 0..variant::FREEZE_TICKS { variant::snow_tick(&mut mob, &Snow(true)); }
    assert_eq!(mob.h.converting, 300);
    // Чёрный ящик, skeleton-to-stray: вышел из снега в дрожи — `16=false`, превращения нет.
    for _ in 0..30 { variant::snow_tick(&mut mob, &Snow(true)); }
    variant::snow_tick(&mut mob, &Snow(false));
    assert_eq!(mob.h.converting, 270);
    variant::snow_tick(&mut mob, &Snow(false));
    assert_eq!((mob.h.water_ticks, mob.h.converting, mob.h.converted), (0, 0, None));
    for _ in 0..variant::FREEZE_TICKS { variant::snow_tick(&mut mob, &Snow(true)); }
    for _ in 0..299 { variant::snow_tick(&mut mob, &Snow(true)); }
    assert_eq!(mob.h.converted, None);
    variant::snow_tick(&mut mob, &Snow(true));
    assert_eq!(mob.h.converted, Some(Kind::Stray));
    for kind in [Kind::Stray, Kind::Bogged, Kind::WitherSkeleton] {
        let mut other = Mob::new(4, kind, (0.5, 1.0, 0.5));
        for _ in 0..500 { variant::snow_tick(&mut other, &Snow(true)); }
        assert_eq!(other.h.converted, None);
    }
    mob.h.converting = 200;
    mob.no_ai = true;
    variant::snow_tick(&mut mob, &Snow(true));
    assert_eq!(mob.h.converting, 200);
}

#[test]
fn snow_state_and_shearing_survive_save_and_do_not_consume_next_record() {
    let mut skeleton = Mob::new(3, Kind::Skeleton, (0.5, 1.0, 0.5));
    skeleton.h.water_ticks = variant::FREEZE_TICKS;
    skeleton.h.converting = 217;
    skeleton.name = Some("Cold bones".into());
    skeleton.persistent_tag = true;
    let mut bogged = Mob::new(4, Kind::Bogged, (0.5, 1.0, 0.5));
    bogged.sheared = true;
    let mut bytes = Vec::new();
    skeleton.write(&mut bytes);
    bogged.write(&mut bytes);
    Mob::new(5, Kind::Skeleton, (0.5, 1.0, 0.5)).write(&mut bytes);
    let mut reader = bytes.as_slice();
    let back = Mob::read(&mut reader).unwrap();
    assert_eq!((back.kind, back.h.water_ticks, back.h.converting, back.name), (Kind::Skeleton, variant::FREEZE_TICKS, 217, skeleton.name.clone()));
    let back = Mob::read(&mut reader).unwrap();
    assert!(back.sheared && back.h.bow && back.kind == Kind::Bogged);
    assert_eq!(Mob::read(&mut reader).unwrap().kind, Kind::Skeleton);
    assert!(reader.is_empty());
    let stray = convert(&skeleton, Kind::Stray, 20);
    assert!(stray.h.bow && stray.persistent_tag);
    assert_eq!(stray.name, skeleton.name);
    assert_eq!(stray.h.converting, 0);
}

#[test]
fn world_mob_step_replaces_converting_skeleton_and_emits_conversion_event() {
    let mut mob = Mob::new(3, Kind::Skeleton, (0.5, 1.0, 0.5));
    mob.h.converting = 2;
    mob.persistent_tag = true;
    mob.name = Some("Snow".into());
    let mut items = Items::new();
    items.add_mob(mob);
    let mut rng = Rng::new(3);
    let env = || Env { difficulty: 2, sky: 0, sun_up: false, clock: 0 };
    let first = step_all(&mut items, &Snow(true), &[], env(), &mut rng, &mut || 30);
    assert!(first.level_events.is_empty());
    assert_eq!(items.mobs()[0].kind, Kind::Skeleton);
    let second = step_all(&mut items, &Snow(true), &[], env(), &mut rng, &mut || 30);
    assert_eq!(second.level_events.len(), 1);
    assert_eq!(second.level_events[0].1, 1048);
    assert_eq!(items.mobs().len(), 1);
    assert_eq!((items.mobs()[0].entity_id, items.mobs()[0].kind), (30, Kind::Stray));
    assert_eq!(items.mobs()[0].name.as_deref(), Some("Snow"));
    assert!(items.items().is_empty(), "превращение не роняет добычу");
}

#[test]
fn bogged_shearing_yields_exactly_two_independent_mushrooms_only_once() {
    let mut combinations = std::collections::HashSet::new();
    let mut rng = Rng::new(7);
    for _ in 0..200 {
        let mut mob = Mob::new(3, Kind::Bogged, (0.0, 1.0, 0.0));
        let drops = variant::shear(&mut mob, &mut rng).unwrap();
        assert_eq!(drops.len(), 2);
        assert!(drops.iter().all(|stack| stack.count == 1 && matches!(blocks::item_name(stack.item), Some("red_mushroom" | "brown_mushroom"))));
        combinations.insert((drops[0].item, drops[1].item));
        assert!(variant::shear(&mut mob, &mut rng).is_none());
        assert!(matches!(mob.events.as_slice(), [Event::EntitySound { name: "minecraft:entity.bogged.shear", category: 7, .. }]));
    }
    assert_eq!(combinations.len(), 4);
}

#[test]
fn arrows_carry_exact_effects_and_undead_ignore_poison() {
    assert_eq!(variant::arrow_effect(Kind::Skeleton), None);
    let slow = variant::arrow_effect(Kind::Stray).unwrap();
    let poison = variant::arrow_effect(Kind::Bogged).unwrap();
    assert_eq!((slow.kind, slow.ticks_left, slow.amplifier), (EffectKind::Slowness, 600, 0));
    assert_eq!((poison.kind, poison.ticks_left, poison.amplifier), (EffectKind::Poison, 100, 0));
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    cow.no_ai = true;
    crate::arrow::apply_effects(&mut cow, &[poison]);
    run(&mut cow, &Flat::new(), 100);
    assert_eq!(cow.vitals.health, 6.0);
    assert!(cow.vitals.effects.is_empty());
    for kind in [Kind::Skeleton, Kind::Stray, Kind::Bogged, Kind::Spider] {
        let mut mob = Mob::new(2, kind, (0.5, 1.0, 0.5));
        crate::arrow::apply_effects(&mut mob, &[poison]);
        assert!(mob.vitals.effects.is_empty());
    }
}

#[test]
fn variant_loot_requires_credit_and_looting_changes_chance_and_counts() {
    let mut rng = Rng::new(27);
    for kind in [Kind::Stray, Kind::Bogged] {
        let mut mob = Mob::new(7, kind, (0.0, 1.0, 0.0));
        for _ in 0..100 {
            assert!(loot(&mob, &mut rng).iter().all(|s| matches!(blocks::item_name(s.item), Some("bone" | "arrow"))));
        }
        mob.by_player = true;
        for (level, chance) in [(0, 0.5), (1, 0.75), (2, 0.875), (3, 0.9375)] {
            mob.looting = level;
            let mut arrows = 0;
            let mut max_bones = 0;
            for _ in 0..4000 {
                for stack in loot(&mob, &mut rng) {
                    match blocks::item_name(stack.item).unwrap() {
                        "bone" | "arrow" => {
                            assert!((1..=2 + level).contains(&stack.count));
                            max_bones = max_bones.max(stack.count);
                        }
                        "tipped_arrow" => {
                            arrows += 1;
                            assert_eq!(stack.count, 1);
                            assert_eq!(stack.extras().potion, crate::survival::potion_number(if kind == Kind::Stray { "slowness" } else { "poison" }));
                        }
                        "bow" => assert!((326..384).contains(&stack.extras().damage)),
                        other => panic!("{other}"),
                    }
                }
            }
            assert_eq!(max_bones, 2 + level);
            assert!((f64::from(arrows) / 4000.0 - chance).abs() < 0.03);
        }
    }
}

#[test]
fn spawning_uses_java_biomes_and_weights() {
    for biome in ["snowy_plains", "ice_spikes", "frozen_ocean", "deep_frozen_ocean"] {
        let kinds = spawn::overworld_kinds(biome, false);
        assert!(kinds.contains(&(Kind::Stray, 80)) && kinds.contains(&(Kind::Skeleton, 20)));
    }
    for biome in ["plains", "frozen_river", "snowy_taiga", "snowy_beach"] {
        assert!(!spawn::overworld_kinds(biome, false).iter().any(|(kind, _)| *kind == Kind::Stray));
    }
    for biome in ["swamp", "mangrove_swamp"] {
        assert!(spawn::overworld_kinds(biome, false).contains(&(Kind::Bogged, 50)));
        assert!(!spawn::overworld_kinds(biome, true).iter().any(|(kind, _)| *kind == Kind::Bogged));
    }
}
