use super::*;
use crate::{gameplay_catalog::{Catalog, Profile}, mob::{Kind, Mob, Rng}};

fn setup(kind: Kind, profile: Profile) -> Items {
    let mut items=Items::with_catalog(Catalog::new(profile));
    let mut mob=Mob::new(42,kind,(0.5,64.0,0.5)); mob.no_ai=true;
    items.add_mob(mob); items.snapshot_mobs(); items
}
fn flush(items: &mut Items) {
    items.flush_mob_events(&mut || panic!("публикация HP не выдаёт номера"),&mut Rng::new(5));
}
fn health_changes(items: &mut Items, from: usize) -> Vec<(i32,f32)> {
    items.changes_since(from,71).0.into_iter().filter_map(|change| match change {
        Change::AllayHealth { entity_id, health } => Some((entity_id,health)), _ => None,
    }).collect()
}

#[test]
fn allay_publication_changes_only_snapshot_health_and_deduplicates() {
    let mut items=setup(Kind::Allay,Profile::Preview26_3);
    let from=items.watch_changes(71);
    let mut expected=items.mob_view()[0].clone(); expected.vitals.health=19.0;
    let actual=&mut items.mobs_mut()[0]; actual.vitals.health=19.0;
    actual.x+=2.0; actual.vy=0.6; actual.ticks=100; actual.yaw=90.0;
    actual.vitals.give_effect(crate::survival::EffectKind::Speed,0,100);
    let live=format!("{:?}",items.mobs()[0]);
    flush(&mut items); flush(&mut items);
    assert_eq!(format!("{:?}",items.mob_view()[0]).as_bytes(),format!("{expected:?}").as_bytes());
    assert_eq!(format!("{:?}",items.mobs()[0]),live);
    assert_eq!(health_changes(&mut items,from),[(42,19.0)]);
}

#[test]
fn allay_publication_is_lazy_and_skips_non_targets() {
    for (kind,profile) in [(Kind::Cow,Profile::Preview26_3),(Kind::Allay,Profile::Java26_1_2)] {
        let mut items=setup(kind,profile); let from=items.watch_changes(71);
        let snapshot=format!("{:?}",items.mob_view());
        items.mobs_mut()[0].vitals.health=3.0;
        let mut lookup=None; items.publish_allay_health(0,&mut lookup);
        assert!(lookup.is_none()); assert_eq!(format!("{:?}",items.mob_view()),snapshot);
        flush(&mut items); assert!(health_changes(&mut items,from).is_empty());
    }
}

#[test]
fn allay_publication_requires_same_snapshot_identity_and_kind() {
    for mismatch in 0..4 {
        let mut items=setup(Kind::Allay,Profile::Preview26_3); let from=items.watch_changes(71);
        match mismatch {
            0 => items.mob_view[0].uuid[0]^=1,
            1 => items.mob_view[0].entity_id+=1,
            2 => items.mob_view[0].kind=Kind::Cow,
            _ => items.mob_view.clear(),
        }
        let snapshot=format!("{:?}",items.mob_view());
        items.mobs_mut()[0].vitals.health=19.0; flush(&mut items);
        assert_eq!(format!("{:?}",items.mob_view()),snapshot);
        assert!(health_changes(&mut items,from).is_empty());
    }
}

#[test]
fn allay_publication_rejects_dead_nonpositive_and_nonfinite_health() {
    for (health,dead,dead_for) in [(0.0,false,0),(-1.0,false,0),(f32::NAN,false,0),(f32::INFINITY,false,0),(19.0,true,0),(19.0,false,1)] {
        let mut items=setup(Kind::Allay,Profile::Preview26_3); let from=items.watch_changes(71);
        let snapshot=format!("{:?}",items.mob_view());
        let m=&mut items.mobs_mut()[0]; m.vitals.health=health; m.vitals.dead=dead; m.dead_for=dead_for;
        let mut lookup=None; items.publish_allay_health(0,&mut lookup);
        assert!(lookup.is_none()); assert_eq!(format!("{:?}",items.mob_view()),snapshot);
        assert!(health_changes(&mut items,from).is_empty());
    }
}

#[test]
fn allay_health_flush_neither_consumes_rng_nor_changes_mob_events() {
    let mut items=setup(Kind::Allay,Profile::Preview26_3);
    items.mobs_mut()[0].vitals.health=18.0;
    let mut rng=Rng::new(17); let mut untouched=rng.clone();
    items.flush_mob_events(&mut || panic!("нет выдачи ID"),&mut rng);
    assert_eq!(rng.next_u64(),untouched.next_u64());
    assert!(items.mobs()[0].events.is_empty());
}

#[test]
fn allay_health_updates_are_ordered_after_existing_hurt_events() {
    let mut items=setup(Kind::Allay,Profile::Preview26_3); let from=items.watch_changes(71);
    assert!(items.mobs_mut()[0].hurt(1.0,crate::survival::Source::Generic,None).is_some());
    flush(&mut items);
    let changes=items.changes_since(from,71).0;
    let hurt=changes.iter().position(|c|matches!(c,Change::MobEvent { event:crate::mob::Event::Hurt { .. },.. })).unwrap();
    let hp=changes.iter().position(|c|matches!(c,Change::AllayHealth { health,.. } if *health==19.0)).unwrap();
    assert!(hurt<hp);
}
