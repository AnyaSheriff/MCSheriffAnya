use super::*;
fn enchanted(name: &str, level: i32) -> Data {
    let mut data = Data::default();
    data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), level);
    data
}
fn near(got: f32, want: f32) { assert!((got-want).abs() < 0.0001, "{got} != {want}"); }
#[test]
fn sharpness_bonus_scales_linearly_and_is_not_critical() {
    for (level, bonus) in [(1, 1.0), (2, 1.5), (3, 2.0), (4, 2.5), (5, 3.0)] {
        let data = enchanted("sharpness", level);
        near(enchant_bonus(&data, false, false), bonus);
        near(strike(7.0, 0.0, bonus, 1.0, true).damage, 10.5 + bonus);
        near(strike(7.0, 0.0, bonus, 0.5, true).damage, 2.8 + bonus * 0.5);
    }
}
#[test]
fn smite_targets_include_all_java_undead_and_exclude_spiders_players() {
    for name in ["zombie", "husk", "drowned", "zombie_villager", "zombified_piglin", "zoglin", "zombie_horse", "camel_husk", "zombie_nautilus", "skeleton", "stray", "bogged", "parched", "wither_skeleton", "skeleton_horse", "wither", "phantom"] {
        assert!(mob_traits(name).0, "{name}");
        for level in 1..=5 { near(enchant_bonus(&enchanted("smite", level), true, false), level as f32 * 2.5); }
    }
    for name in ["spider", "player", "piglin", "nautilus", "cow"] { assert!(!mob_traits(name).0); }
}
#[test]
fn bane_damage_and_slowness_cover_all_arthropods() {
    for name in ["spider", "cave_spider", "silverfish", "endermite", "bee"] {
        for level in 1..=5 {
            let data = enchanted("bane_of_arthropods", level);
            near(enchant_bonus(&data, false, true), 2.5 * level as f32);
            assert_eq!(bane_effect(&data, name, 0.0).unwrap().ticks_left, 20);
            let slow = bane_effect(&data, name, 0.999).unwrap();
            assert_eq!((slow.amplifier, slow.ticks_left), (3, 19 + 10 * level));
        }
    }
    assert!(bane_effect(&enchanted("bane_of_arthropods", 5), "cow", 0.5).is_none());
}
#[test]
fn density_bonus_uses_all_fall_segments_and_critical_multiplier() {
    for (fall, base) in [(2.0, 8.0), (3.0, 12.0), (8.0, 22.0), (10.0, 24.0)] {
        for level in 0..=5 {
            let bonus = base + 0.5 * level as f32 * fall;
            near(smash_bonus(fall, level), bonus);
            near(strike(6.0, bonus, 0.0, 1.0, true).damage, (6.0 + bonus) * 1.5);
        }
    }
}
#[test]
fn power_critical_roll_has_integer_bound_for_odd_damage() {
    for (level, base) in [(0, 6.0), (1, 9.0), (2, 11.0), (3, 12.0), (4, 14.0), (5, 15.0)] {
        near(arrow_damage(3.0, level, false, 0.0), base);
        near(arrow_damage(3.0, level, true, 0.99999), base + (base / 2.0).floor() + 1.0);
    }
}
#[test]
fn quick_charge_all_levels_use_expected_completion_tick() {
    for (level, ticks) in [(0,25), (1,20), (2,15), (3,10)] { assert_eq!(crossbow_charge_ticks(level), ticks); }
}
#[test]
fn knockback_adds_half_a_block_per_level_and_sprint() {
    for level in 0..=2 {
        let data = enchanted("knockback", level);
        near(extra_knockback(0.0, &data, false) * 0.5, level as f32 * 0.5);
        near(extra_knockback(0.0, &data, true) * 0.5, (level + 1) as f32 * 0.5);
    }
}
#[test]
fn projectile_protection_includes_wither_skull_and_respects_epf_cap() {
    for source in [Source::Arrow, Source::Trident, Source::Thrown, Source::WindCharge, Source::Fireball, Source::WitherSkull] {
        assert!(source.is_projectile());
        assert_eq!(protection_points(&enchanted("projectile_protection", 4), source), 8);
    }
    near(after_protection(10.0, 32), 2.0);
}
#[test]
fn protection_covers_supported_bypass_armor_sources_but_not_void() {
    let data = enchanted("protection", 4);
    for source in [Source::Fall, Source::Magic, Source::Wither, Source::OnFire, Source::FlyIntoWall] {
        near(mitigated_damage(10.0, 20.0, 12.0, protection_points(&data, source), source, 1.0), 8.4);
    }
    // Шипы в теге bypasses_armor нет (вики, Damage type tag): броня их смягчает.
    assert!(mitigated_damage(10.0, 20.0, 12.0, protection_points(&data, Source::Thorns), Source::Thorns, 1.0) < 8.4);
    for source in [Source::Starve, Source::OutOfWorld, Source::GenericKill] { assert_eq!(protection_points(&data, source), 0); }
}
#[test]
fn fire_protection_all_levels_only_reduce_fire_damage() {
    for level in 1..=4 {
        let data = enchanted("fire_protection", level);
        for source in [Source::InFire, Source::OnFire, Source::Lava, Source::Fireball] { assert_eq!(protection_points(&data, source), level * 2); }
        assert_eq!(protection_points(&data, Source::PlayerAttack), 0);
    }
}
#[test]
fn blast_protection_all_levels_include_fireworks() {
    for level in 1..=4 {
        let data = enchanted("blast_protection", level);
        for source in [Source::Explosion, Source::PlayerExplosion, Source::Fireworks] { assert_eq!(protection_points(&data, source), level * 2); }
    }
}

#[test]
fn mending_repairs_one_equipped_item_and_grants_integer_remainder() {
    let mut inventory=Inventory::new();
    let mut data=enchanted("mending",1);data.damage=3;
    for slot in [5,6] { inventory.set(slot,Some(Stack::new(crate::blocks::item_named(if slot==5 {"diamond_helmet"}else{"diamond_chestplate"}).unwrap(),1).with_extras(data.clone()))); }
    let (slot,left)=mend(&mut inventory,7,0.0);assert_eq!((slot,left),(Some(5),6));
    assert_eq!(inventory.slot(5).unwrap().extras().damage,0);assert_eq!(inventory.slot(6).unwrap().extras().damage,3);
}
#[test]
fn unbreaking_armor_and_tools_apply_independently_per_point() {
    for level in 1..=3 {
        let stack=Stack::new(crate::blocks::item_named("diamond_chestplate").unwrap(),1).with_extras(enchanted("unbreaking",level));
        let threshold=0.6+0.4/(level as f32+1.0);
        assert_eq!(wear(&stack,2,true,||threshold-0.0001),2);
        assert_eq!(wear(&stack,2,true,||threshold+0.0001),0);
        let threshold=1.0/(level as f32+1.0);
        assert_eq!(wear(&stack,2,false,||threshold-0.0001),2);
        assert_eq!(wear(&stack,2,false,||threshold+0.0001),0);
    }
}
