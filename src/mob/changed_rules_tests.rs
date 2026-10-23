// Контроль изменений 26.2/26.3 и сохранения прежнего профиля.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile};

struct Ground { catalog: Catalog, name: &'static str }
impl View for Ground {
    fn catalog(&self) -> Catalog { self.catalog }
    fn block(&self, _: i32, y: i32, _: i32) -> i32 {
        if y <= 0 { blocks::state_by_name(self.name).unwrap() } else { 0 }
    }
    fn loaded(&self, _: i32, _: i32) -> bool { true }
    fn open_sky(&self, _: i32, _: i32, _: i32) -> bool { true }
}

#[test]
fn teleport_rejects_bedrock_only_in_preview() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let view = Ground { catalog: Catalog::new(profile), name: "bedrock" };
        for kind in [Kind::Enderman, Kind::Shulker] {
            let mut m = Mob::new(1, kind, (0.5,1.,0.5));
            assert_eq!(hostile::teleport(&mut m, &view, &mut Rng::new(19), (0.5,4.,0.5), 8.), profile == Profile::Java26_1_2, "{profile:?} {kind:?}");
        }
    }
}

#[test]
fn riding_enderman_keeps_position_and_takes_environment_damage_without_teleport_request() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        let view = Ground { catalog, name: "stone" };
        let mut m = Mob::new(1, Kind::Enderman, (0.5,1.,0.5));
        m.catalog = catalog;
        m.riding = true;
        assert!(m.hurt(2., Source::Drown, None).is_some());
        assert_eq!(m.vitals.health, 38.);
        assert_eq!(m.h.want_teleport, profile == Profile::Java26_1_2);
        if profile == Profile::Preview26_3 {
            assert!(!hostile::teleport(&mut m, &view, &mut Rng::new(1), (0.5,4.,0.5), 8.));
            assert_eq!((m.x,m.y,m.z), (0.5,1.,0.5));
        }
    }
}

#[test]
fn bed_bounce_and_its_vibration_are_profile_gated() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        let view = Ground { catalog, name: "red_bed" };
        let mut m = Mob::new(1, Kind::Cow, (0.5,0.6,0.5));
        m.vy = -1.;
        m.step(&view, 0., -1., 0.);
        assert_eq!(m.vy, if catalog.changed_rules() { 0.75 } else { 0. });
        let mut items = crate::items::Items::with_catalog(catalog);
        items.add_mob(m);
        items.flush_mob_events(&mut || 10, &mut Rng::new(1));
        assert_eq!(items.vibrations.iter().any(|e| e.frequency == 2), catalog.changed_rules());
    }
}

#[test]
fn named_mob_stroll_deactivates_far_from_players_in_preview() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        let view = Ground { catalog, name: "stone" };
        let mut m = Mob::new(1, Kind::Cow, (0.5,1.,0.5));
        m.persistent_tag = true;
        let ctx = ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 15, sun_up: true };
        assert_eq!(ai::stroll_active(&m, &view, &ctx), profile == Profile::Java26_1_2);
    }
}

#[test]
fn baby_height_and_negative_knockback_resistance_are_profile_gated() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        let mut m = Mob::new(1, Kind::Zombie, (0.,1.,0.));
        m.catalog = catalog;
        m.age = -100;
        assert_eq!(m.size().1, if catalog.changed_rules() { 0.98 } else { Kind::Zombie.height()*0.5 });
        let mut a = crate::attributes::Attributes::player();
        a.set_catalog(catalog);
        a.set_base(crate::attributes::KNOCKBACK_RESISTANCE, -5.);
        assert_eq!(a.value(crate::attributes::KNOCKBACK_RESISTANCE), if catalog.changed_rules() { -2. } else { 0. });
        a.set_base(crate::attributes::EXPLOSION_KNOCKBACK_RESISTANCE, -5.);
        assert_eq!(a.value(crate::attributes::EXPLOSION_KNOCKBACK_RESISTANCE), 0.);
    }
}

#[test]
fn peaceful_spawning_and_despawning_keep_other_hostiles_unchanged() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        for kind in [Kind::Piglin, Kind::Ocelot, Kind::ZombieHorse, Kind::Zombie, Kind::PiglinBrute] {
            let exception = catalog.changed_rules() && matches!(kind, Kind::Piglin | Kind::Ocelot | Kind::ZombieHorse);
            assert_eq!(spawn::peaceful_exception(catalog, kind), exception);
            let mut m = Mob::new(1, kind, (0.,1.,0.));
            m.catalog = catalog;
            hostile::despawn_check(&mut m, &[], &mut Rng::new(1), true);
            assert_eq!(m.h.despawn, !exception);
        }
    }
}

#[test]
fn baby_nonhumanoid_dimensions_follow_wiki_and_sunwear_respects_unbreakable() {
    let catalog = Catalog::new(Profile::Preview26_3);
    for (kind, size) in [(Kind::Hoglin,(0.75,0.85)),(Kind::Zoglin,(0.75,0.85)),(Kind::Strider,(0.45,0.85)),(Kind::Chicken,(0.3,0.4))] {
        let mut m = Mob::new(1,kind,(0.,1.,0.)); m.catalog = catalog; m.age = -100;
        assert_eq!(m.size(),size);
    }
    let mut m = Mob::new(2,Kind::Zombie,(0.,1.,0.)); m.catalog = catalog;
    let head = crate::inventory::Stack::new(blocks::item_named("iron_helmet").unwrap(),1).with_extras(crate::stackdata::Data { unbreakable:true, ..Default::default() });
    equipment::set_slot(&mut m,5,Some(head));
    let view = Ground { catalog, name:"stone" };
    let ctx = ai::Ctx { players:&[], mates:&[], difficulty:2, sky:15, sun_up:true };
    let mut rng = Rng::new(34);
    for _ in 0..400 { hostile::burn(&mut m,&view,&ctx,&mut rng,true); }
    assert_eq!(equipment::slot(&m,5),Some(head)); assert_eq!(m.vitals.fire,0);
    let plain = crate::inventory::Stack::new(head.item,1);
    equipment::set_slot(&mut m,5,Some(plain));
    for _ in 0..1000 { hostile::burn(&mut m,&view,&ctx,&mut rng,true); }
    assert_eq!(equipment::slot(&m,5),None);
}
