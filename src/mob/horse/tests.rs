use super::*;

fn mob(kind: Kind) -> Mob { Mob::new(123, kind, (0.5, 0.0, 0.5)) }
fn item(name: &str) -> Stack { Stack::new(crate::blocks::item_named(name).unwrap(), 1) }

#[test]
fn equine_ids_dimensions_and_stats() {
    let mut rng = Rng::new(98);
    for kind in KINDS {
        assert_eq!(Kind::from_name(kind.name()), Some(kind));
        for _ in 0..1000 {
            let mut m = mob(kind);
            initialize(&mut m, &mut rng);
            assert!((15.0..=30.0).contains(&m.vitals.max_health));
            assert_eq!(m.vitals.health, m.vitals.max_health);
            assert!((0.1125..=0.3375).contains(&m.horse.speed));
            assert!((0.4..=1.0).contains(&m.horse.jump));
            if pack(kind) { assert_eq!((m.horse.speed, m.horse.jump), (0.175, 0.5)); }
            if undead(kind) { assert_eq!((m.vitals.max_health, m.horse.speed), (15.0, 0.2)); }
        }
    }
    assert_eq!(Kind::Horse.java_type(), 66);
    assert_eq!(Kind::Donkey.java_type(), 36);
    assert_eq!(Kind::Mule.java_type(), 87);
    assert_eq!(Kind::SkeletonHorse.java_type(), 116);
    assert_eq!(Kind::ZombieHorse.java_type(), 151);
}

#[test]
fn coat_and_markings_cover_all_35_combinations() {
    let mut rng = Rng::new(556);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..10000 { let v = wild_variant(&mut rng); assert_eq!(variant(v), v); seen.insert(v); }
    assert_eq!(seen.len(), 35);
    assert_eq!(variant(-1), 6 | (4 << 8));
}

#[test]
fn taming_threshold_bucks_then_records_owner() {
    let mut m = mob(Kind::Horse);
    m.horse.rider = Some(([9; 16], 42));
    assert!(!tame_attempt(&mut m, 0));
    assert_eq!(m.horse.temper, 5);
    assert!(m.horse.buck);
    assert!(m.events.contains(&Event::Status(6)));
    m.horse.buck = false;
    assert!(tame_attempt(&mut m, 4));
    assert!(m.horse.tamed);
    assert_eq!(m.tame.owner, Some([9; 16]));
    assert!(!m.tame.ordered_sit);
    assert!(m.events.contains(&Event::Status(7)));
    for kind in [Kind::SkeletonHorse, Kind::ZombieHorse] {
        let mut m = mob(kind); m.horse.temper = 100; m.horse.rider = Some(([9; 16], 42));
        assert!(!tame_attempt(&mut m, 0));
        assert!(!m.horse.tamed);
    }
}

#[test]
fn food_is_consumed_only_when_useful_and_growth_is_in_seconds() {
    let mut m = mob(Kind::Horse);
    assert!(feed(&mut m, "sugar"));
    assert_eq!(m.horse.temper, 3);
    m.horse.tamed = true;
    assert!(!feed(&mut m, "sugar"));
    m.age = -24000;
    assert!(feed(&mut m, "golden_apple"));
    assert_eq!(m.age, -19200);
    assert_eq!(m.love, 0);
    m.age = 0;
    assert!(feed(&mut m, "golden_carrot"));
    assert_eq!(m.love, super::super::LOVE_TICKS);
    assert!(!feed(&mut m, "golden_carrot"));
    let mut mule = mob(Kind::Mule); mule.horse.tamed = true;
    assert!(!feed(&mut mule, "golden_carrot"));
    let mut undead = mob(Kind::SkeletonHorse); undead.vitals.health = 1.0;
    assert!(!feed(&mut undead, "hay_block"));
}

#[test]
fn hybrid_breeding_has_no_taming_or_equipment_inheritance() {
    let mut a = mob(Kind::Horse); let mut b = mob(Kind::Donkey);
    a.horse.tamed = true; b.horse.tamed = true;
    a.horse.slots[0] = Some(item("saddle")); b.horse.chest = true;
    a.vitals.max_health = 30.0; b.vitals.max_health = 30.0;
    let mut rng = Rng::new(65);
    let mut high = false;
    for _ in 0..1000 {
        let baby = child(&a, &b, 1, &mut rng);
        assert_eq!(baby.kind, Kind::Mule);
        assert_eq!(baby.age, super::super::BABY_AGE);
        assert!(!baby.horse.tamed && !baby.horse.chest && !baby.horse.saddled());
        assert!((15.0..=30.0).contains(&baby.vitals.max_health));
        high |= baby.vitals.max_health > 29.0;
    }
    assert!(high, "высокие родители не усредняются со случайной третьей лошадью");
    assert!(!compatible(Kind::Mule, Kind::Horse));
    assert!(!compatible(Kind::SkeletonHorse, Kind::Horse));
}

#[test]
fn equine_storage_round_trip_preserves_stats_owner_and_components() {
    for kind in KINDS {
        let mut m = mob(kind);
        m.horse.tamed = true; m.horse.temper = 89; m.horse.variant = 3 | (4 << 8);
        m.tame.owner = Some([4; 16]); m.horse.chest = pack(kind); m.silent = true;
        m.horse.slots[0] = Some(item("saddle"));
        if kind == Kind::Horse { m.horse.slots[1] = Some(item("leather_horse_armor").with_extras(crate::stackdata::Data { name: Some("test".to_string()), ..Default::default() })); }
        if pack(kind) { m.horse.slots[16] = Some(item("diamond").with_count(23)); }
        m.vitals.health = 12.5;
        let mut bytes = Vec::new(); m.write(&mut bytes);
        let mut rest = bytes.as_slice(); let read = Mob::read(&mut rest).unwrap();
        assert!(rest.is_empty()); assert_eq!(read.kind, kind); assert_eq!(read.horse, m.horse);
        assert_eq!(read.tame.owner, m.tame.owner); assert_eq!(read.vitals.health, 12.5);
        assert_eq!(read.vitals.max_health, m.vitals.max_health); assert!(read.silent);
        for n in 0..bytes.len() { assert!(Mob::read(&mut &bytes[..n]).is_none()); }
    }
}

#[test]
fn nbt_tame_saddle_variant_and_chest_are_type_checked() {
    let mut m = mob(Kind::Horse);
    apply_data(&mut m, &crate::snbt::parse(r#"{Tame:1b,Temper:500,Variant:1026,SaddleItem:{id:"minecraft:saddle",Count:1b},ArmorItem:{id:"minecraft:diamond_horse_armor",count:1},ChestedHorse:1b}"#).unwrap());
    assert!(m.horse.tamed && m.horse.saddled()); assert_eq!(m.horse.temper, 100);
    assert_eq!(m.horse.variant, 1026); assert!(!m.horse.chest); assert_eq!(m.armor(), 11.0);
    apply_data(&mut m, &crate::snbt::parse(r#"{SaddleItem:{id:"minecraft:stone",count:1}}"#).unwrap());
    assert!(!m.horse.saddled());
}

#[test]
fn horse_window_rejects_wrong_equipment_and_shift_moves_saddle() {
    let mut inv = crate::inventory::Inventory::new();
    inv.set(crate::inventory::FIRST_HOTBAR, Some(item("saddle")));
    let mut slots = vec![None; 2];
    let mut w = crate::window::Window { container: &mut slots, inventory: &mut inv, kind: None, horse: Some(Kind::Horse) };
    w.click(29, 0, 1);
    assert_eq!(w.container[0], Some(item("saddle")));
    assert!(w.inventory.slot(crate::inventory::FIRST_HOTBAR).is_none());
    w.inventory.set_cursor(Some(item("stone")));
    w.click(1, 0, 0); assert!(w.container[1].is_none()); assert_eq!(w.inventory.cursor(), Some(item("stone")));
    w.inventory.set_cursor(Some(item("diamond_horse_armor")));
    w.click(1, 0, 0); assert_eq!(w.container[1], Some(item("diamond_horse_armor")));
}

#[test]
fn donkey_inventory_has_15_cargo_slots_and_drops_everything() {
    let mut m = mob(Kind::Donkey); m.horse.chest = true;
    m.horse.slots[0] = Some(item("saddle")); m.horse.slots[16] = Some(item("diamond").with_count(4));
    assert_eq!(m.horse.inventory().len(), 17);
    assert!(!may_place(Kind::Donkey, 1, item("diamond_horse_armor").item));
    let loot = super::super::loot(&m, &mut Rng::new(20));
    assert!(loot.contains(&item("chest"))); assert!(loot.contains(&item("saddle"))); assert!(loot.contains(&item("diamond").with_count(4)));
}

struct Floor { wall: bool }
impl super::super::View for Floor {
    fn block(&self, x: i32, y: i32, _: i32) -> i32 {
        if y < 0 || (self.wall && x == 2) { crate::blocks::state_by_name("stone").unwrap() } else { 0 }
    }
    fn loaded(&self, _: i32, _: i32) -> bool { true }
}

#[test]
fn riding_validates_driver_state_speed_and_wall_collision() {
    let view = Floor { wall: false };
    let mut m = mob(Kind::Horse);
    assert!(!drive(&mut m, &view, (0.6, 0.0, 0.5), (0.0, 0.0), true));
    m.horse.tamed = true; m.horse.slots[0] = Some(item("saddle")); m.horse.rider = Some(([1;16], 5));
    m.on_ground = true; m.ticks = 1; m.horse.move_budget = 1.0;
    assert!(drive(&mut m, &view, (0.6, 0.0, 0.5), (45.0, 0.0), true));
    assert!(m.on_ground);
    let before = (m.x, m.y, m.z);
    assert!(!drive(&mut m, &view, (100.0, 0.0, 0.5), (0.0, 0.0), true));
    assert!(!drive(&mut m, &view, (f64::NAN, 0.0, 0.5), (0.0, 0.0), true));
    assert_eq!((m.x, m.y, m.z), before);
    m.ticks += 5;
    assert!(!drive(&mut m, &Floor { wall: true }, (2.0, 0.0, 0.5), (0.0, 0.0), true));
    assert_eq!((m.x, m.y, m.z), before);
}

#[test]
fn jump_charge_and_safe_fall_boundaries() {
    assert_eq!(charge(0), 0.4); assert!((charge(45) - 0.6).abs() < 1e-9); assert_eq!(charge(90), 1.0);
    assert_eq!(fall_damage(6.0), 0.0); assert_eq!(fall_damage(7.0), 1.0); assert_eq!(fall_damage(8.0), 1.0); assert_eq!(fall_damage(9.0), 2.0);
}

#[test]
fn horse_jump_lands_without_damage_and_high_fall_hurts_rider_once() {
    let view = Floor { wall: false };
    let mut m = mob(Kind::Horse);
    m.horse.tamed = true; m.horse.slots[0] = Some(item("saddle")); m.horse.rider = Some(([1;16], 5));
    m.on_ground = true; m.horse.jump = 0.7; m.horse.jump_charge = charge(90);
    let health = m.vitals.health;
    let mut y: f64 = 0.0; let mut velocity: f64 = 0.7;
    for _ in 0..40 {
        m.ticks += 1;
        y = (y + velocity).max(0.0);
        assert!(drive(&mut m, &view, (0.5, y, 0.5), (0.0, 0.0), y == 0.0));
        velocity = (velocity - super::super::GRAVITY) * super::super::VERTICAL_DRAG;
        if y == 0.0 { break; }
    }
    assert!(m.on_ground); assert_eq!(m.y, 0.0); assert_eq!(m.vitals.health, health);
    m.y = 10.0; m.on_ground = false; m.vitals.fall = 0.0;
    m.ticks += 1;
    assert!(drive(&mut m, &view, (0.5, 0.0, 0.5), (0.0, 0.0), false));
    assert_eq!(m.vitals.health, health - 2.0); assert_eq!(m.horse.rider_fall, 2.0);
    let mut items = crate::items::Items::new(); items.add_mob(m);
    let mut riding = crate::riding::Riding::default(); riding.mount(5, 123, 1);
    assert_eq!(sync_riding(&mut items, &mut riding, &[([1;16], 5)], &view), vec![(5, 2.0)]);
    assert!(sync_riding(&mut items, &mut riding, &[([1;16], 5)], &view).is_empty());
    sync_riding(&mut items, &mut riding, &[], &view);
    assert_eq!(riding.vehicle_of(5), None);
}

#[test]
fn skeleton_trap_has_four_riders_and_expires_without_trigger() {
    let mut items = crate::items::Items::new();
    let mut m = mob(Kind::SkeletonHorse); m.horse.trap = true; items.add_mob(m);
    let mut riding = crate::riding::Riding::default(); let mut rng = Rng::new(41); let mut id = 200;
    let mut next = || { id += 1; id };
    let mut watcher = super::super::ai::Watcher { id: 5, x: 0.5, y: 0.0, z: 0.5, eye: 1.62, holding: [None;2], spectator: true, targetable: true, look: (0.0,0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false };
    traps(&mut items, &mut riding, &[watcher], &mut rng, &mut next);
    assert_eq!(items.mobs().len(), 1);
    watcher.spectator = false;
    traps(&mut items, &mut riding, &[watcher], &mut rng, &mut next);
    assert_eq!(items.mobs().iter().filter(|m| m.kind == Kind::SkeletonHorse && m.horse.tamed && !m.horse.trap).count(), 4);
    let skeletons: Vec<_> = items.mobs().iter().filter(|m| m.kind == Kind::Skeleton).collect();
    assert_eq!(skeletons.len(), 4);
    for m in skeletons { assert!(m.h.bow && m.h.helmet); assert!(riding.vehicle_of(m.entity_id).is_some()); }
    traps(&mut items, &mut riding, &[watcher], &mut rng, &mut next);
    assert_eq!(items.mobs().len(), 8);
    let mut expired = mob(Kind::SkeletonHorse); expired.entity_id = 300; expired.horse.trap = true; expired.horse.trap_age = 17999;
    items.add_mob(expired);
    traps(&mut items, &mut riding, &[], &mut rng, &mut next);
    assert!(items.mobs().iter().find(|m| m.entity_id == 300).unwrap().h.despawn);
}

#[test]
fn horse_and_donkey_breed_through_passive_mob_pipeline() {
    let mut items = crate::items::Items::new();
    for (id, kind, x) in [(1, Kind::Horse, 0.5), (2, Kind::Donkey, 2.0)] {
        let mut m = Mob::new(id, kind, (x, 0.0, 0.5)); m.horse.tamed = true; m.on_ground = true;
        assert!(feed(&mut m, "golden_carrot")); items.add_mob(m);
    }
    let mut rng = Rng::new(783); let mut id = 200;
    for _ in 0..100 {
        super::super::step_all(&mut items, &Floor { wall: false }, &[], super::super::Env { difficulty: 2, sky: 15, sun_up: false, clock: 0, tick: 0 }, &mut rng, &mut || { id += 1; id });
    }
    assert_eq!(items.mobs().iter().filter(|m| m.kind == Kind::Mule && m.is_baby()).count(), 1);
    assert!(items.mobs().iter().filter(|m| m.kind != Kind::Mule).all(|m| m.age > 0 && m.love == 0));
}

#[test]
fn summoned_equipment_keeps_supported_item_components() {
    let mut m = mob(Kind::Horse);
    apply_data(&mut m, &crate::snbt::parse(r#"{equipment:{saddle:{id:"minecraft:saddle",components:{"minecraft:custom_name":"A saddle"}},body:{id:"minecraft:diamond_horse_armor"}}}"#).unwrap());
    assert_eq!(m.horse.slots[0].unwrap().extras().name.as_deref(), Some("A saddle"));
    assert_eq!(toughness(&m), 2.0);
}

#[test]
fn natural_herds_use_biomes_group_sizes_and_shared_coats() {
    struct Grass;
    impl super::super::View for Grass {
        fn block(&self, _: i32, y: i32, _: i32) -> i32 { if y < 0 { crate::blocks::state_by_name("grass_block").unwrap() } else { 0 } }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }
    for biome in ["plains", "savanna", "meadow", "desert"] {
        let mut rng = Rng::new(127); let mut id = 1; let mut horses = 0; let mut donkeys = 0;
        for _ in 0..5000 {
            let herd = super::super::spawn::for_new_chunk_in(&Grass, 0, 0, biome, &mut rng, &mut || { id += 1; id });
            let Some(first) = herd.first() else { continue; };
            if !equine(first.kind) { continue; }
            assert!(herd.iter().all(|m| m.kind == first.kind));
            if first.kind == Kind::Horse {
                horses += 1;
                assert!((2..=6).contains(&herd.len()));
                assert!(herd.iter().all(|m| m.horse.variant & 255 == first.horse.variant & 255));
            } else {
                donkeys += 1;
                assert_eq!(first.kind, Kind::Donkey);
                let max = match biome { "plains" => 3, "meadow" => 2, _ => 1 };
                assert!((1..=max).contains(&herd.len()));
            }
        }
        assert_eq!(horses > 0, matches!(biome, "plains" | "savanna"));
        assert_eq!(donkeys > 0, biome != "desert");
    }
}

// Чёрный ящик, probe-horse: /summon с данными не бросает кости стартовых значений.
#[test]
fn summoned_equines_start_with_black_box_defaults() {
    for (kind, health, speed) in [(Kind::Horse, 53.0, 0.225), (Kind::Donkey, 53.0, 0.175), (Kind::Mule, 53.0, 0.175), (Kind::SkeletonHorse, 15.0, 0.2), (Kind::ZombieHorse, 25.0, 0.225)] {
        let m = mob(kind);
        assert_eq!((m.vitals.max_health, m.vitals.health, m.horse.speed, m.horse.jump), (health, health, speed, 0.7), "{kind:?}");
    }
}

#[test]
fn explicit_attributes_are_remembered_and_health_follows_the_maximum() {
    let mut m = mob(Kind::Horse);
    apply_data(&mut m, &crate::snbt::parse(r#"{attributes:[{id:"minecraft:max_health",base:30d},{id:"minecraft:jump_strength",base:0.7d}]}"#).unwrap());
    assert_eq!(m.horse.explicit, 1 | 4);
    assert_eq!((m.vitals.max_health, m.vitals.health), (30.0, 30.0));
    let mut plain = mob(Kind::Horse);
    apply_data(&mut plain, &crate::snbt::parse("{Tame:1b,Temper:0}").unwrap());
    assert_eq!(plain.horse.explicit, 0);
}

#[test]
fn seat_height_and_eating_flag_follow_the_black_box() {
    assert_eq!(seat(&mob(Kind::Horse)).1, 0.844);
    assert_eq!(seat(&mob(Kind::Donkey)).1, 0.512);
    assert_eq!(seat(&mob(Kind::Mule)).1, 0.612);
    assert_eq!(seat(&mob(Kind::SkeletonHorse)).1, 0.719);
    let mut state = State { tamed: true, eating: 30, ..State::default() };
    assert_eq!(state.flags(), 2 | 64);
    state.rearing = 20;
    assert_eq!(state.flags(), 2 | 32 | 64);
}

#[test]
fn gear_from_summon_data_drops_rarely_but_player_gear_always() {
    let mut rng = Rng::new(9);
    let mut by_data = mob(Kind::Horse);
    by_data.horse.slots[0] = Some(item("saddle"));
    by_data.horse.gear_by_data = true;
    let dropped = (0..2000).filter(|_| super::super::loot(&by_data, &mut rng).contains(&item("saddle"))).count();
    assert!((100..=250).contains(&dropped), "{dropped}");
    by_data.horse.gear_by_data = false;
    assert!((0..200).all(|_| super::super::loot(&by_data, &mut rng).contains(&item("saddle"))));
}
