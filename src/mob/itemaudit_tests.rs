use super::*;
use crate::survival::EffectKind;

struct Floor;
impl View for Floor {
    fn block(&self, _x: i32, y: i32, _z: i32) -> i32 { if y < 0 { crate::blocks::state_by_name("stone").unwrap() } else { 0 } }
    fn loaded(&self, _x: i32, _z: i32) -> bool { true }
}
fn mob(kind: Kind) -> Mob { Mob::new(99, kind, (0.5, 0.0, 0.5)) }

#[test]
fn raider_effects_tick_once_and_villager_poison_is_not_discarded() {
    let ctx = ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 0, sun_up: false };
    for kind in [Kind::Pillager, Kind::Vindicator, Kind::Villager, Kind::Witch, Kind::Cow] {
        let mut mob = mob(kind);
        mob.no_ai = true;
        mob.vitals.give_effect(EffectKind::Glowing, 0, 60);
        mob.vitals.give_effect(EffectKind::Poison, 0, 25);
        let before = mob.vitals.health;
        mob.tick(&Floor, &ctx, &mut Rng::new(1));
        assert_eq!(mob.vitals.effects.iter().find(|e| e.kind == EffectKind::Glowing).unwrap().ticks_left, 59, "{kind:?}");
        assert!(mob.vitals.health < before, "{kind:?}: periodic damage lost");
    }
}

#[test]
fn regeneration_heals_common_mobs_once_per_due_tick() {
    let ctx = ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 0, sun_up: false };
    for kind in [Kind::Pillager, Kind::Villager, Kind::Cow] {
        let mut mob = mob(kind);
        mob.no_ai = true;
        mob.vitals.health = 3.0;
        mob.vitals.give_effect(EffectKind::Regeneration, 0, 50);
        mob.tick(&Floor, &ctx, &mut Rng::new(1));
        assert_eq!(mob.vitals.health, 4.0);
    }
}

#[test]
fn strength_and_weakness_change_mob_melee_without_negative_damage() {
    let mut mob = mob(Kind::Zombie);
    mob.vitals.give_effect(EffectKind::Strength, 1, 100);
    assert_eq!(mob.effect_melee_damage(3.0), 9.0);
    mob.vitals.give_effect(EffectKind::Weakness, 0, 100);
    assert_eq!(mob.effect_melee_damage(3.0), 5.0);
    mob.vitals.give_effect(EffectKind::Weakness, 3, 100);
    assert_eq!(mob.effect_melee_damage(3.0), 0.0);
}

#[test]
fn speed_and_slowness_both_change_mob_acceleration() {
    let step = |effect: Option<EffectKind>| {
        let mut mob = mob(Kind::Cow);
        mob.on_ground = true;
        if let Some(kind) = effect { mob.vitals.give_effect(kind, 0, 100); }
        mob.physics(&Floor, 1.0, false);
        mob.vx.hypot(mob.vz)
    };
    let normal = step(None);
    assert!(step(Some(EffectKind::Speed)) > normal);
    assert!(step(Some(EffectKind::Slowness)) < normal);
}

#[test]
fn jump_boost_increases_mob_jump_velocity() {
    let mut plain = mob(Kind::Cow);
    plain.on_ground = true;
    let mut boosted = plain.clone();
    boosted.vitals.give_effect(EffectKind::JumpBoost, 1, 100);
    plain.physics(&Floor, 0.0, true);
    boosted.physics(&Floor, 0.0, true);
    assert!((boosted.vy - plain.vy - 0.2 * 0.98).abs() < 1e-6);
}

#[test]
fn slow_falling_reduces_mob_gravity_and_prevents_fall_damage() {
    let mut mob = mob(Kind::Cow);
    mob.y = 10.0;
    mob.vitals.give_effect(EffectKind::SlowFalling, 0, 1000);
    mob.physics(&Floor, 0.0, false);
    assert!((mob.vy + 0.0098).abs() < 1e-6);
    for _ in 0..300 { mob.physics(&Floor, 0.0, false); }
    assert_eq!(mob.vitals.health, mob.kind.max_health());
    assert!(mob.on_ground);
}

#[test]
fn levitation_lifts_mobs_and_clears_fall_accumulation() {
    let mut mob = mob(Kind::Cow);
    mob.y = 10.0;
    mob.vitals.fall = 5.0;
    mob.vitals.give_effect(EffectKind::Levitation, 1, 100);
    mob.physics(&Floor, 0.0, false);
    assert!((mob.vy - 0.0196).abs() < 1e-6);
    assert_eq!(mob.vitals.fall, 0.0);
}

#[test]
fn potion_immunities_are_shared_by_arrows_splash_and_clouds() {
    for kind in [Kind::Zombie, Kind::Skeleton, Kind::Drowned] {
        assert!(!mob(kind).accepts_effect(EffectKind::Poison));
        assert!(!mob(kind).accepts_effect(EffectKind::Regeneration));
        assert!(mob(kind).accepts_effect(EffectKind::Weakness));
    }
    assert!(!mob(Kind::Spider).accepts_effect(EffectKind::Poison));
    assert!(mob(Kind::Cow).accepts_effect(EffectKind::Poison));
    assert!(mob(Kind::Wither).accepts_effect(EffectKind::InstantHealth));
    assert!(!mob(Kind::Wither).accepts_effect(EffectKind::Poison));
}

#[test]
fn fire_aspect_and_flame_cook_lethal_drops_and_failed_hits_do_not_ignite() {
    for (kind, cooked) in [(Kind::Cow, "cooked_beef"), (Kind::Pig, "cooked_porkchop"), (Kind::Chicken, "cooked_chicken"), (Kind::Sheep, "cooked_mutton")] {
        let mut mob = mob(kind);
        assert!(mob.hurt_burning(100.0, Source::PlayerAttack, None, 1.0, 160).unwrap().died);
        let items: Vec<_> = mob.events.iter().filter_map(|event| if let Event::Loot(stacks) = event { Some(stacks) } else { None }).flatten().collect();
        assert!(items.iter().any(|stack| blocks::item_name(stack.item) == Some(cooked)));
    }
    let mut mob = mob(Kind::Cow);
    mob.hurt(2.0, Source::PlayerAttack, None).unwrap();
    assert!(mob.hurt_burning(1.0, Source::Arrow, None, 1.0, 100).is_none());
    assert_eq!(mob.vitals.fire, 0);
}

#[test]
fn passive_mobs_burn_each_twenty_ticks_and_fire_resistance_keeps_the_fire_timer() {
    let ctx = ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 0, sun_up: false };
    for resistant in [false, true] {
        let mut mob = mob(Kind::Cow);
        mob.no_ai = true;
        mob.vitals.ignite(100);
        if resistant { mob.vitals.give_effect(EffectKind::FireResistance, 0, 100); }
        mob.tick(&Floor, &ctx, &mut Rng::new(1));
        assert_eq!(mob.vitals.health, if resistant { 10.0 } else { 9.0 });
        assert_eq!(mob.vitals.fire, 99);
    }
}

#[test]
fn deflected_ghast_fireball_still_kills_a_fire_immune_ghast() {
    let mut ghast = mob(Kind::Ghast);
    assert!(ghast.hurt(5.0, Source::Fireball, None).is_none());
    ghast.vitals.give_effect(EffectKind::FireResistance, 0, 100);
    assert!(ghast.hurt(1000.0, Source::LargeFireball, None).unwrap().died);
}
