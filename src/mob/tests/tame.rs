// Приручение, поводок, бирка (чёрный ящик: wolf-tame, cat-tame,
// wolf-sit-follow, wolf-defend, lead-attach, name-tag; зонды probe-follow,
// probe-cats).

use super::*;
use crate::mob::tame as tm;

const OWNER: ([u8; 16], i32) = ([7; 16], 1);

#[test]
fn a_bone_tames_a_wolf_one_time_in_three_and_gives_it_forty_health() {
    let mut rng = Rng::new(11);
    let (mut tamed, mut tries) = (0, 0);

    for _ in 0..300 {
        let mut wolf = Mob::new(5, Kind::Wolf, (0.5, 1.0, 0.5));

        loop {
            tries += 1;
            assert_eq!(tm::interact(&mut wolf, OWNER, Some("bone"), &mut rng), tm::Used::Spent);

            if wolf.tame.tamed() {
                break;
            }

            assert_eq!(wolf.events.pop(), Some(Event::Status(6)));
        }

        tamed += 1;
        assert_eq!(wolf.events.last(), Some(&Event::Status(7)));
        assert_eq!((wolf.vitals.health, wolf.vitals.max_health), (tm::TAME_HEALTH, tm::TAME_HEALTH));
        assert!(wolf.tame.ordered_sit);
        assert_eq!(wolf.tame.flags(), 4, "позу ставит ИИ, не щелчок");
    }

    // В среднем три кости на волка.
    let mean = f64::from(tries) / f64::from(tamed);

    assert!((2.6..3.4).contains(&mean), "{mean}");
}

#[test]
fn a_cat_takes_fish_and_an_owner_click_toggles_sitting() {
    let mut rng = Rng::new(3);
    let mut cat = Mob::new(5, Kind::Cat, (0.5, 1.0, 0.5));

    assert_eq!(tm::interact(&mut cat, OWNER, Some("bone"), &mut rng), tm::Used::Pass);

    while !cat.tame.tamed() {
        assert_eq!(tm::interact(&mut cat, OWNER, Some("salmon"), &mut rng), tm::Used::Spent);
    }

    assert_eq!(cat.vitals.max_health, 10.0, "у кошки здоровье прежнее");

    // Сытая здоровая — любовь (чёрный ящик, cat-tame: событие 18).
    cat.events.clear();
    assert_eq!(tm::interact(&mut cat, OWNER, Some("cod"), &mut rng), tm::Used::Spent);
    assert_eq!(cat.events, vec![Event::Hearts]);

    // Чужому — ничего, хозяину — сесть/встать.
    let sit = cat.tame.ordered_sit;

    assert_eq!(tm::interact(&mut cat, ([9; 16], 2), None, &mut rng), tm::Used::Pass);
    assert_eq!(tm::interact(&mut cat, OWNER, None, &mut rng), tm::Used::Kept);
    assert_eq!(cat.tame.ordered_sit, !sit);

    // Краситель — ошейник.
    assert_eq!(tm::interact(&mut cat, OWNER, Some("blue_dye"), &mut rng), tm::Used::Spent);
    assert_eq!(cat.tame.collar, 11);
}

#[test]
fn a_sitting_wolf_stays_and_a_standing_one_jumps_to_a_far_owner() {
    let view = Flat::new();
    let mut rng = Rng::new(1);
    let mut wolf = Mob::new(5, Kind::Wolf, (0.5, 1.0, 0.5));

    tm::tame(&mut wolf, OWNER);
    wolf.on_ground = true;

    let far = [player_at(1, 20.5, 0.5)];
    let ctx = ctx_for(&far, 2, 15, false);

    for _ in 0..5 {
        wolf.tick(&view, &ctx, &mut rng);
    }

    assert!(wolf.tame.sitting);
    assert!((wolf.x - 0.5).abs() < 1e-9, "сидящий не идёт");

    // Встал: хозяин в 20 блоках — на второй такт прыжок к нему.
    wolf.tame.ordered_sit = false;
    wolf.tick(&view, &ctx, &mut rng);
    assert!((wolf.x - 0.5).abs() < 0.5);
    // Десять попыток места за такт могут и не найти клетки: тогда — в следующий.
    for _ in 0..10 {
        wolf.tick(&view, &ctx, &mut rng);

        if wolf.teleports > 0 {
            break;
        }
    }

    let (dx, dz) = (wolf.x - 20.5, wolf.z - 0.5);

    assert_eq!(wolf.teleports, 1);
    assert!(dx.abs() <= 3.0 && dz.abs() <= 3.0 && (dx.abs() >= 2.0 || dz.abs() >= 2.0), "{dx} {dz}");
}

#[test]
fn an_owner_strike_sends_the_wolf_biting_for_four() {
    let view = Flat::new();
    let mut items = Items::new();
    let mut wolf = Mob::new(5, Kind::Wolf, (0.5, 1.0, 0.5));
    let mut sheep = Mob::new(6, Kind::Sheep, (1.8, 1.0, 0.5));

    tm::tame(&mut wolf, OWNER);
    wolf.tame.ordered_sit = false;
    sheep.no_ai = true;
    sheep.hurt(1.0, Source::PlayerAttack, Some(Push { dir: (1.0, 0.0), strength: 0.4, cause: OWNER.1 }));
    items.add_mob(wolf);
    items.add_mob(sheep);

    let players = [player_at(OWNER.1, 0.5, -2.5)];
    let mut rng = Rng::new(2);
    let env = Env { difficulty: 2, sky: 15, sun_up: false, clock: 1000 };
    let mut bitten = false;

    for _ in 0..60 {
        step_all(&mut items, &view, &players, env, &mut rng, &mut || 99);

        if items.mobs().iter().find(|mob| mob.kind == Kind::Sheep).is_none_or(|sheep| sheep.vitals.health <= 3.0) {
            bitten = true;
            break;
        }
    }

    assert!(bitten, "волк укусил овцу на 4");

    let wolf = items.mobs().iter().find(|mob| mob.kind == Kind::Wolf).expect("волк");

    assert!(wolf.tame.anger_until >= 1000 + tm::ANGER_MIN && wolf.tame.anger_until < 1000 + tm::ANGER_MIN + i64::from(tm::ANGER_SPREAD));
}

#[test]
fn a_lead_snaps_beyond_twelve_blocks_with_the_sound_at_the_holder() {
    let mut cow = Mob::new(5, Kind::Cow, (0.5, 1.0, 0.5));

    leash::attach(&mut cow, leash::Holder::Player { id: 1, uuid: [7; 16] });
    assert_eq!(cow.events, vec![Event::Leash(1)]);
    cow.events.clear();

    let mut mobs = vec![cow];

    assert!(leash::step_all(&mut mobs, &[(1, (8.5, 1.0, 0.5))]).is_empty());
    assert!(mobs[0].leash.is_some());

    let broke = leash::step_all(&mut mobs, &[(1, (13.0, 1.0, 0.5))]);

    assert_eq!(broke, vec![(13.0, 1.0, 0.5)]);
    assert!(mobs[0].leash.is_none());
    assert!(matches!(mobs[0].events[0], Event::SoundAt { name: leash::BROKEN, .. }));
    assert!(matches!(&mobs[0].events[1], Event::Loot(stacks) if stacks.len() == 1));
    assert_eq!(mobs[0].events[2], Event::Leash(0));
}

#[test]
fn a_knot_without_mobs_goes_away_and_a_knot_point_is_mid_fence() {
    assert_eq!(leash::knot_point((1, -59, 3)), (1.5, -58.625, 3.5));

    let mut knot = leash::new_knot(8, (1, 0, 3));
    let mut cow = Mob::new(5, Kind::Cow, (0.5, 1.0, 0.5));

    knot.ticks = 1;
    leash::attach(&mut cow, leash::Holder::Knot { id: 8, at: (1, 0, 3) });

    let mut mobs = vec![knot, cow];

    leash::step_all(&mut mobs, &[]);
    assert!(!mobs[0].h.despawn, "узел с коровой стоит");

    leash::drop_leash(&mut mobs[1], false);
    leash::step_all(&mut mobs, &[]);
    assert!(mobs[0].h.despawn, "пустой узел пропадает");
}

#[test]
fn names_leashes_and_tame_state_survive_saving() {
    let mut wolf = Mob::new(5, Kind::Wolf, (0.5, 1.0, 0.5));

    tm::tame(&mut wolf, OWNER);
    wolf.tame.collar = 3;
    wolf.tame.variant = tm::wolf_variant_for("forest");
    wolf.name = Some("Бобик".to_string());
    wolf.leash = Some(leash::Holder::Knot { id: 8, at: (1, 2, 3) });

    let mut bytes = Vec::new();

    wolf.write(&mut bytes);

    let back = Mob::read(&mut bytes.as_slice()).expect("запись");

    assert_eq!(back.tame.owner, Some([7; 16]));
    assert!(back.tame.ordered_sit);
    assert_eq!((back.tame.collar, back.tame.variant), (3, 8));
    assert_eq!((back.vitals.health, back.vitals.max_health), (40.0, 40.0));
    assert_eq!(back.name.as_deref(), Some("Бобик"));
    assert!(back.persistent_tag || back.name.is_some());
    assert_eq!(back.leash, Some(leash::Holder::Knot { id: 0, at: (1, 2, 3) }));

    // Широкий номер без общего хвоста не добавляет имени и поводка.
    let cow = Mob::new(6, Kind::Cow, (0.5, 1.0, 0.5));
    let mut saved = Vec::new();

    cow.write(&mut saved);
    assert_eq!(&saved[..3], &[0x7f, 0, 0]);
    assert!(Mob::read(&mut saved.as_slice()).is_some_and(|cow| cow.name.is_none() && cow.leash.is_none()));
}

#[test]
fn a_name_tag_renames_once_and_variants_follow_the_original_registry() {
    let mut cow = Mob::new(6, Kind::Cow, (0.5, 1.0, 0.5));

    assert!(leash::name_tag(&mut cow, Some("Бобик")));
    assert!(!leash::name_tag(&mut cow, Some("Бобик")), "то же имя — бирка цела");
    assert!(!leash::name_tag(&mut cow, None));
    assert!(cow.persistent_tag);

    assert_eq!(tm::variant_named(Kind::Wolf, "minecraft:ashen"), Some(0));
    assert_eq!(tm::variant_named(Kind::Cat, "tabby"), Some(9));
    assert_eq!(tm::variant_named(Kind::Cat, "white"), Some(10));
    assert_eq!(tm::default_variant(Kind::Cat), 1);
    assert_eq!(tm::wolf_variant_for("plains"), tm::WOLF_DEFAULT);
    assert_eq!(tm::wolf_variant_for("snowy_taiga"), 0);
    assert_eq!(tm::dye_colour("red_dye"), Some(tm::RED));
}
