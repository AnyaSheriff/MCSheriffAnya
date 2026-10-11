// Фантом, кошки на сундуке, крипер и кошка (вики: Phantom, Cat; чёрный ящик:
// phantom-swoop, phantom-spawn, cat-chest-block, creeper-flee-cat).

use super::*;
use crate::mob::phantom as ph;

fn survivor(id: i32, at: (f64, f64, f64)) -> ai::Watcher {
    ai::Watcher { id, x: at.0, y: at.1, z: at.2, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false }
}

#[test]
fn regional_difficulty_of_a_new_chunk_matches_the_wiki_table() {
    // Вики, Difficulty: новая местность в полночь — лёгкая 0,75, обычная 1,5, сложная 2,25.
    assert_eq!(ph::regional(0, 18_000, 0, 1.0), 0.0);
    assert!((ph::regional(1, 18_000, 0, 1.0) - 0.75).abs() < 1e-9);
    assert!((ph::regional(2, 18_000, 0, 1.0) - 1.5).abs() < 1e-9);
    assert!((ph::regional(3, 18_000, 0, 1.0) - 2.25).abs() < 1e-9);
    assert_eq!(ph::moon_brightness(0), 1.0);
    assert_eq!(ph::moon_brightness(4), 0.0);
}

#[test]
fn groups_are_one_to_difficulty_plus_one() {
    let mut rng = Rng::new(3);

    for difficulty in 1..=3u8 {
        let sizes: Vec<u32> = (0..200).map(|_| ph::group_size(difficulty, &mut rng)).collect();

        assert_eq!(*sizes.iter().min().unwrap(), 1);
        assert_eq!(*sizes.iter().max().unwrap(), u32::from(difficulty) + 1);
    }
}

/// Мир с открытым небом: камень не нужен, выше уровня моря всё воздух.
struct Open;

impl View for Open {
    fn block(&self, _: i32, _: i32, _: i32) -> i32 {
        0
    }

    fn loaded(&self, _: i32, _: i32) -> bool {
        true
    }

    fn open_sky(&self, _: i32, _: i32, _: i32) -> bool {
        true
    }
}

#[test]
fn spawn_needs_72000_ticks_without_sleep_and_height_above_the_sea() {
    let mut rng = Rng::new(5);
    let spot = ph::Spot { x: 0.5, y: 70.0, z: 0.5, rest: 0 };

    // Не спал меньше 72000 — не появляются никогда.
    for rest in [0, 1000, 71_999, 72_000] {
        assert!((0..300).all(|_| ph::try_spot(&Open, &ph::Spot { rest, ..spot }, 99.0, &mut rng).is_none()), "rest {rest}");
    }

    // Ниже уровня моря — тоже.
    assert!((0..300).all(|_| ph::try_spot(&Open, &ph::Spot { rest: 800_000, y: 62.0, ..spot }, 99.0, &mut rng).is_none()));

    // Долго не спал: шанс (x − 72000)/x, место — 20–34 над игроком, ±10 вбок.
    let found: Vec<_> = (0..400).filter_map(|_| ph::try_spot(&Open, &ph::Spot { rest: 800_000, ..spot }, 99.0, &mut rng)).collect();

    assert!(found.len() > 300, "почти всегда: {}", found.len());
    assert!(found.iter().all(|&(x, y, z)| (-10..=10).contains(&x) && (90..=104).contains(&y) && (-10..=10).contains(&z)));
}

#[test]
fn a_phantom_circles_swoops_bites_and_waits_72_ticks() {
    let view = Flat::new();
    let mut phantom = ph::spawn(5, (0, 28, 0));
    let players = [survivor(1, (0.5, 1.0, 0.5))];
    let ctx = ai::Ctx { players: &players, mates: &[], difficulty: 2, sky: 4, sun_up: false };
    let mut rng = Rng::new(9);
    let (mut swooped, mut bite) = (None, None);

    for tick in 0..600 {
        let plan = phantom.tick(&view, &ctx, &mut rng);

        if swooped.is_none() && phantom.events.iter().any(|event| matches!(event, Event::Sound { name, .. } if name.ends_with("phantom.swoop"))) {
            swooped = Some(tick);
        }

        if let Some(melee) = plan.melee {
            // Вики, Phantom: 2 урона на обычной.
            assert_eq!(melee.damage, 2.0);
            bite = Some(tick);
            break;
        }
    }

    // Чёрный ящик, phantom-swoop: звук пике на 27-м такте.
    assert_eq!(swooped, Some(27));
    assert!(bite.is_some(), "фантом укусил");
    assert!(phantom.phantom.wait >= ph::BITE_WAIT);
    assert_eq!(phantom.phantom.phase, ph::Phase::Circle);
}

#[test]
fn a_cat_within_sixteen_blocks_keeps_the_phantom_from_swooping() {
    let view = Flat::new();
    let mut phantom = ph::spawn(5, (0, 28, 0));
    let players = [survivor(1, (0.5, 1.0, 0.5))];
    let cat = [ai::Mate { id: 9, kind: Kind::Cat, x: 4.5, y: 27.0, z: 0.5, love: 0, baby: false, ready: false, tamed: false }];
    let ctx = ai::Ctx { players: &players, mates: &cat, difficulty: 2, sky: 4, sun_up: false };
    let mut rng = Rng::new(9);

    for _ in 0..200 {
        phantom.tick(&view, &ctx, &mut rng);
    }

    assert_eq!(phantom.phantom.phase, ph::Phase::Circle);
    assert!(!phantom.events.iter().any(|event| matches!(event, Event::Sound { name, .. } if name.ends_with("phantom.swoop"))));
}

#[test]
fn a_phantom_drops_a_membrane_only_for_a_player() {
    let mut rng = Rng::new(1);
    let mut phantom = ph::spawn(5, (0, 5, 0));

    assert!(loot(&phantom, &mut rng).is_empty());

    phantom.by_player = true;

    let membrane = blocks::item_named("phantom_membrane").expect("мембрана");
    let got = (0..200).filter(|_| loot(&phantom, &mut rng).iter().any(|stack| stack.item == membrane)).count();

    // Вики: 0–1, то есть половина.
    assert!((60..140).contains(&got), "{got}");
}

#[test]
fn a_creeper_walks_away_from_a_cat_instead_of_toward_the_player() {
    let view = Flat::new();
    let mut creeper = Mob::new(5, Kind::Creeper, (4.5, 1.0, 0.5));
    let players = [survivor(1, (0.5, 1.0, 0.5))];
    // Кошка между игроком и крипером, в 2 блоках от него.
    let cat = [ai::Mate { id: 9, kind: Kind::Cat, x: 2.5, y: 1.0, z: 0.5, love: 0, baby: false, ready: false, tamed: false }];
    let ctx = ai::Ctx { players: &players, mates: &cat, difficulty: 2, sky: 15, sun_up: false };
    let mut rng = Rng::new(4);

    creeper.on_ground = true;

    for _ in 0..40 {
        creeper.tick(&view, &ctx, &mut rng);
    }

    assert!(creeper.x > 4.5, "ушёл от кошки: {}", creeper.x);
}

#[test]
fn a_sitting_cat_blocks_the_chest_under_it() {
    let mut world = crate::world::World::in_memory();

    world.sitting_cats = vec![(2.5, 1.0, 0.5)];

    assert!(world.cat_sits_above(2, 0, 0));
    assert!(!world.cat_sits_above(5, 0, 0));
}

#[test]
fn damage_interrupts_swoop_and_cats_prevent_contact_damage() {
    let mut m=Mob::new(7,Kind::Phantom,(0.5,1.0,0.5));
    m.phantom.phase=ph::Phase::Swoop;m.phantom.target=Some(1);m.vy = -0.4;
    m.hurt(1.0,crate::survival::Source::Generic,None).unwrap();
    assert_eq!(m.phantom.phase,ph::Phase::Circle);assert!(m.vy>0.0);
    m.phantom.phase=ph::Phase::Swoop;
    let players=[survivor(1,(0.5,1.0,0.5))];
    let cats=[ai::Mate {id:9,kind:Kind::Cat,x:1.5,y:1.0,z:0.5,love:0,baby:false,ready:false,tamed:false}];
    let ctx=ai::Ctx {players:&players,mates:&cats,difficulty:2,sky:4,sun_up:false};
    assert!(ph::think(&mut m,&Flat::new(),&ctx,&mut Rng::new(3),ai::Plan::default()).melee.is_none());
    assert_eq!(m.phantom.phase,ph::Phase::Circle);
}
#[test]
fn creepers_flee_ocelots_without_a_player_target() {
    let mut m=Mob::new(5,Kind::Creeper,(4.5,1.0,0.5));m.on_ground=true;
    let cats=[ai::Mate {id:9,kind:Kind::Ocelot,x:2.5,y:1.0,z:0.5,love:0,baby:false,ready:false,tamed:false}];
    let ctx=ai::Ctx {players:&[],mates:&cats,difficulty:2,sky:4,sun_up:false};
    for _ in 0..40 {m.tick(&Flat::new(),&ctx,&mut Rng::new(4));}
    assert!(m.x>5.5);assert_eq!(m.h.fuse,0);
}

#[test]
fn creeper_finishes_escape_and_repaths_three_ticks_after_path_ends() {
    let mut mob=Mob::new(5,Kind::Creeper,(4.5,1.0,0.5));mob.on_ground=true;
    let mut rng=Rng::new(4);
    for _ in 0..80 {
        let cats=[ai::Mate {id:9,kind:Kind::Cat,x:mob.x-2.0,y:1.0,z:mob.z,love:0,baby:false,ready:false,tamed:false}];
        mob.tick(&Flat::new(),&ai::Ctx {players:&[],mates:&cats,difficulty:2,sky:4,sun_up:false},&mut rng);
    }
    assert!(mob.x>12.5,"кошке не удалось остановить бегство: {}",mob.x);
    mob.brain.stop();mob.h.cat_flee=true;
    let cats=[ai::Mate {id:9,kind:Kind::Cat,x:mob.x-2.0,y:1.0,z:mob.z,love:0,baby:false,ready:false,tamed:false}];
    let ctx=ai::Ctx {players:&[],mates:&cats,difficulty:2,sky:4,sun_up:false};
    for _ in 0..3 {mob.tick(&Flat::new(),&ctx,&mut rng);assert!(!mob.brain.walking());}
    mob.tick(&Flat::new(),&ctx,&mut rng);assert!(mob.brain.walking());
}
