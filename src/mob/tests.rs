use super::*;
use crate::items::Items;

mod drowned;
mod fire;
mod phantom;
mod skeleton;
mod tame;

/// Плоский мир: каменный пол по y = 0 (верх пола на y = 1), выше воздух;
/// заданные клетки — камень.
struct Flat {
    floor: i32,
    stone: Vec<(i32, i32, i32)>,
    water: Vec<(i32, i32, i32)>,
}

impl Flat {
    fn new() -> Self {
        Self { floor: 0, stone: Vec::new(), water: Vec::new() }
    }
}

impl View for Flat {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        if self.stone.contains(&(x, y, z)) || y <= self.floor {
            return blocks::state_by_name("stone").expect("камень");
        }

        if self.water.contains(&(x, y, z)) {
            return blocks::state_by_name("water").expect("вода");
        }

        blocks::state_by_name("air").expect("воздух")
    }

    fn loaded(&self, _: i32, _: i32) -> bool {
        true
    }
}

fn ctx<'a>() -> ai::Ctx<'a> {
    ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 15, sun_up: false }
}

fn run(mob: &mut Mob, view: &dyn View, ticks: usize) {
    let mut rng = Rng::new(7);

    for _ in 0..ticks {
        mob.tick(view, &ctx(), &mut rng);
    }
}

#[test]
fn kinds_have_the_wiki_numbers() {
    assert_eq!(Kind::Cow.max_health(), 10.0);
    assert_eq!(Kind::Pig.max_health(), 10.0);
    assert_eq!(Kind::Sheep.max_health(), 8.0);
    assert_eq!(Kind::Chicken.max_health(), 4.0);
    assert_eq!((Kind::Cow.width(), Kind::Cow.height()), (0.9, 1.4));
    assert_eq!((Kind::Chicken.width(), Kind::Chicken.height()), (0.4, 0.7));
    assert_eq!(Kind::from_name("minecraft:sheep"), Some(Kind::Sheep));
    assert_eq!(Kind::from_name("zombie"), Some(Kind::Zombie));
    assert_eq!(Kind::from_name("horse"), None);
}

#[test]
fn the_names_of_drops_exist_as_items() {
    for name in ["leather", "beef", "porkchop", "mutton", "feather", "chicken", "egg", "wheat", "shears"] {
        assert!(blocks::item_named(name).is_some(), "нет предмета {name}");
    }

    for name in WOOL {
        assert!(blocks::item_named(name).is_some(), "нет шерсти {name}");
    }
}

#[test]
fn a_mob_falls_with_gravity_and_drag_and_stops_on_the_floor() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 4.0, 0.5));
    let mut rng = Rng::new(1);

    // Первый такт: сдвиг на вертикальную скорость до тяжести, потом
    // v = (v − 0,08)·0,98.
    cow.tick(&view, &ctx(), &mut rng);
    assert!(cow.y <= 4.0);

    run(&mut cow, &view, 60);

    assert!(cow.on_ground, "после падения на земле");
    assert!((cow.y - 1.0).abs() < 1.0e-6, "стоит на полу: {}", cow.y);
    assert_eq!(cow.vitals.fall, 0.0);
}

#[test]
fn falling_more_than_three_blocks_hurts_by_the_excess() {
    let view = Flat::new();
    // Падение с высоты 5 над полом (пол — верх на y = 1): 5 блоков → урон 2.
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 6.0, 0.5));

    run(&mut cow, &view, 60);

    assert_eq!(cow.vitals.health, 8.0, "10 − ceil(5 − 3)");
    assert!(cow.events.iter().any(|event| matches!(event, Event::Hurt { source: Source::Fall, .. })));
}

#[test]
fn a_short_fall_does_not_hurt() {
    let view = Flat::new();
    let mut pig = Mob::new(1, Kind::Pig, (0.5, 3.5, 0.5));

    run(&mut pig, &view, 40);

    assert_eq!(pig.vitals.health, 10.0);
}

#[test]
fn a_chicken_falls_slowly_and_is_not_hurt() {
    let view = Flat::new();
    let mut chicken = Mob::new(1, Kind::Chicken, (0.5, 21.0, 0.5));
    let mut cow = Mob::new(2, Kind::Cow, (0.5, 21.0, 0.5));
    let mut rng = Rng::new(3);

    for _ in 0..20 {
        chicken.tick(&view, &ctx(), &mut rng);
        cow.tick(&view, &ctx(), &mut rng);
    }

    assert!(chicken.y > cow.y + 1.0, "курица опустилась меньше: {} против {}", chicken.y, cow.y);

    run(&mut chicken, &view, 200);

    assert!(chicken.on_ground);
    assert_eq!(chicken.vitals.health, 4.0, "курица не разбивается");
}

#[test]
fn no_ai_mob_stands_still() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(1);

    cow.no_ai = true;
    cow.vx = 0.3;
    cow.tick(&view, &ctx(), &mut rng);

    assert_eq!((cow.x, cow.z), (0.5, 0.5), "NoAI не двигается сам");
}

#[test]
fn a_hit_hurts_pushes_and_starts_invulnerability() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));

    run(&mut cow, &view, 3);
    assert!(cow.on_ground);

    let hit = cow.hurt(7.0, Source::Generic, Some(Push { dir: (0.0, 1.0), strength: BASE_KNOCKBACK, cause: 7 })).expect("удар прошёл");

    assert_eq!(hit.taken, 7.0);
    assert_eq!(cow.vitals.health, 3.0);
    assert!((cow.vz - 0.4).abs() < 1.0e-9, "толчок по направлению удара: {}", cow.vz);
    // Вверх: половина прежней скорости стоящего (−0,0784) плюс 0,4 — 0,361,
    // как у оригинала (чёрный ящик, cow-physics).
    assert!((cow.vy - 0.3608).abs() < 1.0e-9, "и вверх: {}", cow.vy);
    assert_eq!(cow.impulses, 1);
    assert!(cow.brain.is_panicking(), "раненый убегает");

    // Второй удар внутри неуязвимости слабее первого — не проходит.
    assert!(cow.hurt(5.0, Source::Generic, None).is_none());

    // Сильнее — проходит разницей.
    let more = cow.hurt(9.0, Source::Generic, None).expect("разница прошла");

    assert_eq!(more.taken, 2.0);
}

#[test]
fn a_killed_mob_dies_and_leaves_loot_then_vanishes() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));

    let hit = cow.hurt(99.0, Source::GenericKill, None).expect("удар прошёл");

    assert!(hit.died);
    assert!(cow.is_dead());
    assert!(cow.events.contains(&Event::Death));
    let drops: Vec<Stack> = cow.events.iter().filter_map(|event| if let Event::Loot(stacks) = event { Some(stacks.clone()) } else { None }).flatten().collect();

    assert!(drops.iter().any(|stack| blocks::item_name(stack.item) == Some("beef")), "корова роняет говядину: {drops:?}");
    assert!(matches!(cow.events.last(), Some(Event::Death)), "смерть — после добычи");

    // Мёртвого больше не бьют.
    assert!(cow.hurt(1.0, Source::Generic, None).is_none());

    run(&mut cow, &view, DEATH_TICKS as usize + 1);
    assert!(cow.gone());
}

#[test]
fn loot_follows_the_wiki_ranges() {
    let mut rng = Rng::new(99);

    for _ in 0..200 {
        let cow = Mob::new(1, Kind::Cow, (0.0, 0.0, 0.0));
        let drops = loot(&cow, &mut rng);
        let count = |name: &str| drops.iter().filter(|stack| blocks::item_name(stack.item) == Some(name)).map(|stack| stack.count).sum::<i32>();

        assert!((0..=2).contains(&count("leather")));
        assert!((1..=3).contains(&count("beef")));

        let sheep = Mob::new(2, Kind::Sheep, (0.0, 0.0, 0.0));
        let drops = loot(&sheep, &mut rng);

        assert!(drops.iter().any(|stack| blocks::item_name(stack.item) == Some("white_wool") && stack.count == 1));
        assert!(drops.iter().any(|stack| blocks::item_name(stack.item) == Some("mutton")));

        let mut sheared = Mob::new(3, Kind::Sheep, (0.0, 0.0, 0.0));

        sheared.sheared = true;
        assert!(loot(&sheared, &mut rng).iter().all(|stack| blocks::item_name(stack.item) != Some("white_wool")));

        let mut baby = Mob::new(4, Kind::Pig, (0.0, 0.0, 0.0));

        baby.age = BABY_AGE;
        assert!(loot(&baby, &mut rng).is_empty(), "детёныш ничего не роняет");
    }
}

#[test]
fn a_mob_walks_a_path_around_a_wall() {
    let mut view = Flat::new();

    // Стена в два блока высотой поперёк дороги от x = 0 к x = 6.
    for y in 1..=2 {
        for z in -2..=2 {
            view.stone.push((3, y, z));
        }
    }

    let from = (0, 1, 0);
    let to = (6, 1, 0);
    let route = path::find(&view, from, to, path::MAX_NODES);

    assert!(!route.is_empty());
    assert_eq!(*route.last().unwrap(), to);
    assert!(route.iter().all(|cell| !(cell.0 == 3 && (-2..=2).contains(&cell.2))), "путь обходит стену");
}

#[test]
fn the_path_steps_up_one_block_and_refuses_two() {
    let mut view = Flat::new();

    view.stone.push((2, 1, 0));

    let up = path::find(&view, (0, 1, 0), (3, 2, 0), path::MAX_NODES);

    assert!(up.contains(&(2, 2, 0)) || up.last() == Some(&(3, 2, 0)));

    // Две ступени подряд (стенка в два блока, без обхода): дойти нельзя.
    let mut wall = Flat::new();

    for y in 1..=2 {
        for z in -30..=30 {
            wall.stone.push((2, y, z));
        }
    }

    let route = path::find(&wall, (0, 1, 0), (5, 1, 0), path::MAX_NODES);

    assert!(route.last().is_none_or(|cell| cell.0 < 2), "через стену не идут: {route:?}");
}

#[test]
fn a_mob_avoids_lava_and_deep_drops() {
    let mut view = Flat::new();

    view.floor = 0;

    // Яма глубиной 5 поперёк дороги: спрыгивать глубже трёх блоков не станут.
    for x in 2..=3 {
        for z in -30..=30 {
            for y in -4..=0 {
                view.stone.retain(|cell| *cell != (x, y, z));
            }
        }
    }

    struct Pit;

    impl View for Pit {
        fn block(&self, x: i32, y: i32, _z: i32) -> i32 {
            let stone = blocks::state_by_name("stone").expect("камень");
            let air = blocks::state_by_name("air").expect("воздух");

            if (2..=3).contains(&x) {
                return if y <= -5 { stone } else { air };
            }

            if y <= 0 { stone } else { air }
        }

        fn loaded(&self, _: i32, _: i32) -> bool {
            true
        }
    }

    let route = path::find(&Pit, (0, 1, 0), (6, 1, 0), path::MAX_NODES);

    assert!(route.iter().all(|cell| !(2..=3).contains(&cell.0)), "яму обходят или не идут: {route:?}");
}

#[test]
fn a_stroll_moves_a_free_mob_over_time() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(42);
    let start = (cow.x, cow.z);

    for _ in 0..3000 {
        cow.tick(&view, &ctx(), &mut rng);
    }

    assert!(cow.on_ground);
    assert!((cow.x - start.0).abs() + (cow.z - start.1).abs() > 0.5, "за две с половиной минуты корова куда-то ушла");
}

#[test]
fn walking_reaches_the_end_of_a_given_path() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(5);

    run(&mut cow, &view, 3);

    let route = path::find(&view, cow.cell(), (4, 1, 0), path::MAX_NODES);

    cow.brain.go(route, 1.0);

    for _ in 0..200 {
        cow.tick(&view, &ctx(), &mut rng);

        if !cow.brain.walking() {
            break;
        }
    }

    assert!(!cow.brain.walking(), "путь пройден");
    assert!((cow.x - 4.5).abs() < 1.5, "корова у цели: {}", cow.x);
}

#[test]
fn a_mob_climbs_a_one_block_step_by_jumping() {
    let mut view = Flat::new();

    // Ступенька в блок высотой, дальше — возвышение.
    for x in 3..=8 {
        for z in -12..=12 {
            view.stone.push((x, 1, z));
        }
    }

    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(5);

    run(&mut cow, &view, 3);

    let route = path::find(&view, cow.cell(), (5, 2, 0), path::MAX_NODES);

    assert!(!route.is_empty());
    cow.brain.go(route, 1.0);

    // Идёт по пути, пока не окажется за ступенью.
    for _ in 0..300 {
        cow.tick(&view, &ctx(), &mut rng);

        if cow.x > 4.5 {
            break;
        }
    }

    assert!(cow.x > 4.5 && cow.y >= 2.0 - 1.0e-6, "взобралась на ступень: x={} y={}", cow.x, cow.y);
}

#[test]
fn a_tempting_player_draws_the_animal_in() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(9);
    let wheat = blocks::item_name(blocks::item_named("wheat").expect("пшеница")).expect("имя");
    let player = ai::Watcher { x: 8.5, y: 1.0, z: 0.5, eye: 1.62, holding: [Some(wheat), None], spectator: false, id: 1, targetable: true, look: (0.0, 0.0), pumpkin: false, gold_armor: false };
    let players = [player];
    let ctx = ai::Ctx { players: &players, mates: &[], difficulty: 2, sky: 15, sun_up: false };

    for _ in 0..200 {
        cow.tick(&view, &ctx, &mut rng);
    }

    assert!((cow.x - player.x).abs() < 3.5, "корова подошла к пшенице: {}", cow.x);
    assert_eq!(cow.brain.doing, ai::Doing::Tempt);

    // Без пшеницы в руке — не идёт.
    let mut other = Mob::new(2, Kind::Cow, (0.5, 1.0, 20.5));
    let empty = [ai::Watcher { holding: [None, None], ..player }];
    let ctx = ai::Ctx { players: &empty, mates: &[], difficulty: 2, sky: 15, sun_up: false };

    for _ in 0..40 {
        other.tick(&view, &ctx, &mut rng);
        assert_ne!(other.brain.doing, ai::Doing::Tempt);
    }
}

#[test]
fn a_spectator_does_not_attract() {
    let view = Flat::new();
    let mut pig = Mob::new(1, Kind::Pig, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(9);
    let players = [ai::Watcher { x: 5.5, y: 1.0, z: 0.5, eye: 1.62, holding: [Some("carrot"), None], spectator: true, id: 1, targetable: false, look: (0.0, 0.0), pumpkin: false, gold_armor: false }];
    let ctx = ai::Ctx { players: &players, mates: &[], difficulty: 2, sky: 15, sun_up: false };

    for _ in 0..20 {
        pig.tick(&view, &ctx, &mut rng);
        assert_ne!(pig.brain.doing, ai::Doing::Tempt);
    }
}

#[test]
fn two_animals_in_love_breed_a_baby() {
    let view = Flat::new();
    let mut items = Items::new();
    let mut next = 100;
    let mut rng = Rng::new(11);

    for (id, x) in [(1, 0.5), (2, 3.5)] {
        let mut cow = Mob::new(id, Kind::Cow, (x, 1.0, 0.5));

        cow.love = LOVE_TICKS;
        items.add_mob(cow);
    }

    let mut babies = 0;

    for _ in 0..600 {
        step_all(&mut items, &view, &[], Env { difficulty: 2, sky: 15, sun_up: false, clock: 0 }, &mut rng, &mut || {
            next += 1;
            next
        });
        babies = items.mobs().iter().filter(|mob| mob.is_baby()).count();

        if babies > 0 {
            break;
        }
    }

    assert_eq!(babies, 1, "родился один детёныш");

    let parents: Vec<&Mob> = items.mobs().iter().filter(|mob| !mob.is_baby()).collect();

    assert_eq!(parents.len(), 2);
    assert!(parents.iter().all(|mob| mob.love == 0 && mob.age > 0 && mob.age <= BREED_COOLDOWN), "родители в паузе");

    let baby = items.mobs().iter().find(|mob| mob.is_baby()).unwrap();

    assert_eq!(baby.age, BABY_AGE);
    assert_eq!(baby.size().0, 0.45, "детёныш вдвое меньше");
}

#[test]
fn a_baby_grows_up_after_twenty_minutes() {
    let view = Flat::new();
    let mut pig = Mob::new(1, Kind::Pig, (0.5, 1.0, 0.5));

    pig.age = -30;
    pig.no_ai = true;
    run(&mut pig, &view, 29);
    assert!(pig.is_baby());
    run(&mut pig, &view, 1);
    assert!(!pig.is_baby());
    assert_eq!(pig.age, 0);
}

#[test]
fn a_chicken_lays_eggs_in_six_thousand_to_twelve_thousand_ticks() {
    let view = Flat::new();
    let mut chicken = Mob::new(1, Kind::Chicken, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(1);
    let mut first = None;

    chicken.egg_timer = 6000 + 17;

    for tick in 1..=6100 {
        let plan = chicken.tick(&view, &ctx(), &mut rng);

        if plan.lay_egg && first.is_none() {
            first = Some(tick);
        }
    }

    assert_eq!(first, Some(6017));
    assert!((6000..=12000).contains(&chicken.egg_timer), "новый срок: {}", chicken.egg_timer);
}

#[test]
fn a_sheep_eats_grass_and_grows_wool_back() {
    struct Meadow;

    impl View for Meadow {
        fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
            if y < 0 {
                blocks::state_by_name("stone").expect("камень")
            } else if y == 0 {
                blocks::state_by_name("grass_block").expect("трава")
            } else {
                blocks::state_by_name("air").expect("воздух")
            }
        }

        fn loaded(&self, _: i32, _: i32) -> bool {
            true
        }
    }

    let mut items = Items::new();
    let mut sheep = Mob::new(1, Kind::Sheep, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(8);
    let mut next = 100;

    sheep.sheared = true;
    sheep.brain.eating = 5;
    items.add_mob(sheep);

    let mut eaten = None;

    for _ in 0..6 {
        let outcome = step_all(&mut items, &Meadow, &[], Env { difficulty: 2, sky: 15, sun_up: false, clock: 0 }, &mut rng, &mut || {
            next += 1;
            next
        });

        if let Some(eat) = outcome.eaten.first() {
            eaten = Some(*eat);
        }
    }

    assert_eq!(eaten, Some(ai::Eat { at: (0, 0, 0), block: true }), "дёрн под овцой становится землёй");
    assert!(!items.mobs()[0].sheared, "шерсть отросла");
}

#[test]
fn mobs_survive_the_chunk_file() {
    let mut cow = Mob::new(5, Kind::Cow, (12.25, 64.0, -3.5));

    cow.age = -1200;
    cow.love = 30;
    cow.no_ai = true;
    cow.vitals.health = 4.5;
    cow.yaw = 90.0;

    let mut sheep = Mob::new(6, Kind::Sheep, (1.0, 2.0, 3.0));

    sheep.sheared = true;
    sheep.color = 14;

    let mut bytes = Vec::new();

    cow.write(&mut bytes);
    sheep.write(&mut bytes);

    let mut rest = bytes.as_slice();
    let back_cow = Mob::read(&mut rest).expect("корова");
    let back_sheep = Mob::read(&mut rest).expect("овца");

    assert!(rest.is_empty());
    assert_eq!((back_cow.x, back_cow.y, back_cow.z), (12.25, 64.0, -3.5));
    assert_eq!(back_cow.age, -1200);
    assert_eq!(back_cow.love, 30);
    assert!(back_cow.no_ai);
    assert_eq!(back_cow.vitals.health, 4.5);
    assert_eq!(back_cow.yaw, 90.0);
    assert!(back_sheep.sheared);
    assert_eq!(back_sheep.color, 14);

    // Испорченная запись не читается.
    assert!(Mob::read(&mut &bytes[..10]).is_none());
    assert!(Mob::read(&mut &[9u8; 40][..]).is_none());
}

#[test]
fn weapons_and_charge_follow_the_wiki() {
    assert_eq!(weapon(None), (1.0, 5.0));
    assert_eq!(weapon(Some("diamond_sword")), (7.0, 12.5));
    assert_eq!(weapon(Some("stone_sword")).0, 5.0);
    assert!((charged(7.0, 100, 12.5) - 7.0).abs() < 1.0e-6, "полный заряд — полный урон");
    assert!(charged(7.0, 0, 12.5) < 2.0, "нулевой заряд — мало");
}

#[test]
fn knockback_direction_follows_the_look() {
    // Смотрит на юг (yaw 0) — толкает на +z; на запад (yaw 90) — на −x.
    let (x, z) = look_direction(0.0);

    assert!(x.abs() < 1.0e-9 && (z - 1.0).abs() < 1.0e-9);

    let (x, z) = look_direction(90.0);

    assert!((x + 1.0).abs() < 1.0e-9 && z.abs() < 1.0e-9);
}

#[test]
fn a_mob_in_water_floats_instead_of_sinking() {
    let mut view = Flat::new();

    for y in 1..=3 {
        for x in -12..=12 {
            for z in -12..=12 {
                view.water.push((x, y, z));
            }
        }
    }

    let mut pig = Mob::new(1, Kind::Pig, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(2);

    for _ in 0..60 {
        pig.tick(&view, &ctx(), &mut rng);
    }

    assert!(pig.y > 1.5, "свинья всплыла: {}", pig.y);
    assert_eq!(pig.vitals.health, 10.0);
}

#[test]
fn natural_spawns_are_rare_and_stand_on_grass() {
    struct Meadow;

    impl View for Meadow {
        fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
            if y < 60 {
                blocks::state_by_name("stone").expect("камень")
            } else if y == 60 {
                blocks::state_by_name("grass_block").expect("трава")
            } else {
                blocks::state_by_name("air").expect("воздух")
            }
        }

        fn loaded(&self, _: i32, _: i32) -> bool {
            true
        }
    }

    let mut rng = Rng::new(123);
    let mut groups = 0;
    let mut next = 0;

    for chunk in 0..2000 {
        let born = spawn::for_new_chunk(&Meadow, chunk, 3, &mut rng, &mut || {
            next += 1;
            next
        });

        if born.is_empty() {
            continue;
        }

        groups += 1;
        assert!(born.iter().all(|mob| mob.kind == born[0].kind), "группа одного вида");
        assert!(born.iter().all(|mob| (mob.y - 61.0).abs() < 1.0e-9 && mob.chunk() == (chunk, 3)), "на траве, в своём чанке");
        assert!((2..=4).contains(&born.len()), "в группе {}", born.len());
    }

    // Шанс 10 % на чанк: на 2000 чанков — около двухсот групп.
    assert!((120..=280).contains(&groups), "групп: {groups}");
}

#[test]
fn wild_sheep_colours_follow_the_wiki_shares() {
    let mut rng = Rng::new(5);
    let mut counts = [0usize; 16];

    for _ in 0..100_000 {
        counts[usize::from(spawn::wild_colour(&mut rng))] += 1;
    }

    let share = |colour: usize| counts[colour] as f64 / 1000.0;

    assert!((share(0) - 81.8).abs() < 1.5, "белая {}", share(0));
    assert!((share(15) - 5.0).abs() < 0.8, "чёрная {}", share(15));
    assert!((share(12) - 3.0).abs() < 0.8, "коричневая {}", share(12));
    assert!(share(6) < 0.6, "розовая {}", share(6));
}

#[test]
fn only_a_mob_killed_by_a_player_gives_experience() {
    let mut by_player = Mob::new(1, Kind::Cow, (0.5, 0.0, 0.5));

    by_player.credit_player();
    by_player.hurt(f32::MAX, Source::Generic, None);
    assert!((1..=3).contains(&by_player.pending_xp), "{}", by_player.pending_xp);

    let mut by_fall = Mob::new(2, Kind::Pig, (0.5, 0.0, 0.5));

    by_fall.hurt(f32::MAX, Source::Fall, None);
    assert_eq!(by_fall.pending_xp, 0);

    let mut baby = Mob::new(3, Kind::Sheep, (0.5, 0.0, 0.5));

    baby.age = -100;
    baby.credit_player();
    baby.hurt(f32::MAX, Source::Generic, None);
    assert_eq!(baby.pending_xp, 0, "детёныш опыта не даёт");
}

#[test]
fn the_credit_for_a_player_hit_runs_out_after_100_ticks() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));

    cow.no_ai = true;
    cow.credit_player();
    run(&mut cow, &view, 101);
    cow.hurt(f32::MAX, Source::Generic, None);
    assert_eq!(cow.pending_xp, 0);
}

#[test]
fn a_critical_hit_needs_a_full_hand_and_a_fall() {
    assert!(is_critical(1.0, 0.4, false, false, false));
    assert!(!is_critical(0.5, 0.4, false, false, false), "рука не заряжена");
    assert!(!is_critical(1.0, 0.0, false, false, false), "не падает");
    assert!(!is_critical(1.0, 0.4, true, false, false), "на земле");
    assert!(!is_critical(1.0, 0.4, false, true, false), "бежит");
    assert!(!is_critical(1.0, 0.4, false, false, true), "в воде");
}

// ---------------------------------------------------------------------------
// Враждебные: зомби, скелет, крипер, паук
// ---------------------------------------------------------------------------

/// Плоский мир под солнцем: небо открыто везде, свет — как у неба.
struct Sunny(Flat);

impl View for Sunny {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        self.0.block(x, y, z)
    }

    fn loaded(&self, cx: i32, cz: i32) -> bool {
        self.0.loaded(cx, cz)
    }

    fn open_sky(&self, _: i32, _: i32, _: i32) -> bool {
        true
    }

    fn light(&self, _: i32, _: i32, _: i32, sky: i32) -> (u8, u8) {
        (sky as u8, 0)
    }
}

fn player_at(id: i32, x: f64, z: f64) -> ai::Watcher {
    ai::Watcher { id, x, y: 1.0, z, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, gold_armor: false }
}

fn ctx_for<'a>(players: &'a [ai::Watcher], difficulty: u8, sky: i32, sun_up: bool) -> ai::Ctx<'a> {
    ai::Ctx { players, mates: &[], difficulty, sky, sun_up }
}

/// Прогоняет моба, собирая удары, выстрелы и взрывы.
fn hunt(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, ticks: usize) -> Vec<(usize, ai::Plan)> {
    let mut rng = Rng::new(3);
    let mut out = Vec::new();

    for tick in 0..ticks {
        let plan = mob.tick(view, ctx, &mut rng);

        if plan.melee.is_some() || plan.shot.is_some() || plan.blast.is_some() {
            out.push((tick, plan));
        }
    }

    out
}

#[test]
fn hostile_kinds_have_the_wiki_numbers() {
    assert_eq!((Kind::Zombie.max_health(), Kind::Skeleton.max_health(), Kind::Creeper.max_health(), Kind::Spider.max_health()), (20.0, 20.0, 20.0, 16.0));
    assert_eq!((Kind::Zombie.width(), Kind::Zombie.height()), (0.6, 1.95));
    assert_eq!((Kind::Skeleton.width(), Kind::Skeleton.height()), (0.6, 1.99));
    assert_eq!((Kind::Creeper.width(), Kind::Creeper.height()), (0.6, 1.7));
    assert_eq!((Kind::Spider.width(), Kind::Spider.height()), (1.4, 0.9));
    assert_eq!((Kind::Zombie.java_type(), Kind::Skeleton.java_type(), Kind::Creeper.java_type(), Kind::Spider.java_type()), (150, 115, 32, 124));
    // С разбойниками, врединой и разорителем (mob/illager.rs): 27 + 6; бриз: +1; хранитель: +1.
    assert!(Kind::ALL.iter().filter(|kind| kind.hostile()).count() == 35);
    assert_eq!(Kind::Warden.max_health(), 500.0);
    assert_eq!((Kind::Warden.width(), Kind::Warden.height()), (0.9, 2.9));
    assert!(!Kind::Cow.hostile());
}

#[test]
fn hostile_drops_follow_the_wiki() {
    for name in ["rotten_flesh", "bone", "arrow", "gunpowder", "string", "spider_eye", "bow"] {
        assert!(blocks::item_named(name).is_some(), "нет предмета {name}");
    }

    let names = |mob: &Mob| -> Vec<&'static str> {
        let mut rng = Rng::new(1);
        let mut seen = Vec::new();

        for _ in 0..200 {
            for stack in loot(mob, &mut rng) {
                let name = blocks::item_name(stack.item).expect("имя");

                assert!(stack.count >= 1);

                if !seen.contains(&name) {
                    seen.push(name);
                }
            }
        }

        seen.sort_unstable();
        seen
    };

    assert_eq!(names(&Mob::new(1, Kind::Zombie, (0.0, 0.0, 0.0))), ["rotten_flesh"]);
    assert_eq!(names(&Mob::new(1, Kind::Skeleton, (0.0, 0.0, 0.0))), ["arrow", "bone"]);
    assert_eq!(names(&Mob::new(1, Kind::Creeper, (0.0, 0.0, 0.0))), ["gunpowder"]);
    // Паучий глаз — только если убил игрок.
    assert_eq!(names(&Mob::new(1, Kind::Spider, (0.0, 0.0, 0.0))), ["string"]);

    let mut spider = Mob::new(1, Kind::Spider, (0.0, 0.0, 0.0));

    spider.by_player = true;
    assert_eq!(names(&spider), ["spider_eye", "string"]);
}

#[test]
fn a_zombie_hunts_the_nearest_survival_player_only() {
    let view = Flat::new();
    let players = [player_at(1, 20.5, 0.5), player_at(2, 8.5, 0.5)];
    let mut zombie = Mob::new(9, Kind::Zombie, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(3);

    for _ in 0..200 {
        zombie.tick(&view, &ctx_for(&players, 2, 15, false), &mut rng);
    }

    assert_eq!(zombie.h.target, Some(2), "ближайший");

    // Творческий и наблюдатель не цели.
    let creative = [ai::Watcher { targetable: false, ..player_at(3, 4.5, 0.5) }];
    let mut other = Mob::new(10, Kind::Zombie, (0.5, 1.0, 0.5));

    for _ in 0..30 {
        other.tick(&view, &ctx_for(&creative, 2, 15, false), &mut rng);
    }

    assert_eq!(other.h.target, None);

    // Дальше 35 блоков — не видит.
    let far = [player_at(4, 40.5, 0.5)];
    let mut third = Mob::new(11, Kind::Zombie, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        third.tick(&view, &ctx_for(&far, 2, 15, false), &mut rng);
    }

    assert_eq!(third.h.target, None);

    // Скелет — дальше 16.
    let mut skeleton = Mob::new(12, Kind::Skeleton, (0.5, 1.0, 0.5));
    let mid = [player_at(5, 20.5, 0.5)];

    for _ in 0..200 {
        skeleton.tick(&view, &ctx_for(&mid, 2, 15, false), &mut rng);
    }

    assert_eq!(skeleton.h.target, None);
}

#[test]
fn a_wall_hides_the_player_from_the_zombie() {
    let mut flat = Flat::new();

    for y in 1..4 {
        for z in -2..3 {
            flat.stone.push((4, y, z));
        }
    }

    let players = [player_at(1, 8.5, 0.5)];
    let mut zombie = Mob::new(9, Kind::Zombie, (0.5, 1.0, 0.5));

    let mut rng = Rng::new(3);

    for _ in 0..200 {
        zombie.tick(&flat, &ctx_for(&players, 2, 15, false), &mut rng);
    }

    assert_eq!(zombie.h.target, None, "за стеной не видно");
}

#[test]
fn a_zombie_walks_up_and_strikes_every_twenty_ticks() {
    let view = Flat::new();
    let players = [player_at(1, 6.5, 0.5)];
    let mut zombie = Mob::new(9, Kind::Zombie, (0.5, 1.0, 0.5));
    let hits = hunt(&mut zombie, &view, &ctx_for(&players, 2, 15, false), 200);

    assert!(hits.len() >= 5, "ударов: {}", hits.len());

    let melee = hits[0].1.melee.expect("удар");

    assert_eq!((melee.target, melee.damage), (1, 3.0));
    assert!(melee.push.0 > 0.99, "толкает от зомби к игроку: {:?}", melee.push);

    for pair in hits.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, 20, "раз в 20 тактов");
    }
}

#[test]
fn a_zombie_does_not_strike_from_afar() {
    let view = Flat::new();
    let players = [player_at(1, 30.5, 0.5)];
    let mut zombie = Mob::new(9, Kind::Zombie, (0.5, 1.0, 0.5));
    let hits = hunt(&mut zombie, &view, &ctx_for(&players, 2, 15, false), 20);

    assert!(hits.is_empty());
}

#[test]
fn a_creeper_swells_near_a_player_and_blows_at_thirty() {
    let view = Flat::new();
    let players = [player_at(1, 2.5, 0.5)];
    let mut creeper = Mob::new(9, Kind::Creeper, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(3);
    let mut blasts = Vec::new();

    for tick in 0..400 {
        let plan = creeper.tick(&view, &ctx_for(&players, 2, 15, false), &mut rng);

        if plan.blast.is_some() {
            blasts.push((tick, plan.blast));
            break;
        }
    }

    assert_eq!(blasts.len(), 1);
    assert_eq!(blasts[0].1, Some(3.0));
    assert_eq!(creeper.h.fuse, 30);
    assert_eq!(creeper.h.swell_dir, 1);

    // Зашипел ровно один раз.
    let hisses = creeper.events.iter().filter(|event| matches!(event, Event::Sound { name: "minecraft:entity.creeper.primed", .. })).count();

    assert_eq!(hisses, 1);
}

#[test]
fn a_creeper_defuses_when_the_player_leaves() {
    let view = Flat::new();
    let mut creeper = Mob::new(9, Kind::Creeper, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(3);
    let near = [player_at(1, 2.5, 0.5)];

    while creeper.h.fuse < 15 {
        creeper.tick(&view, &ctx_for(&near, 2, 15, false), &mut rng);
    }

    assert_eq!(creeper.h.fuse, 15);

    // Игрок ушёл за 7 блоков — запал убывает и гаснет.
    let far = [player_at(1, 12.5, 0.5)];

    for _ in 0..30 {
        let plan = creeper.tick(&view, &ctx_for(&far, 2, 15, false), &mut rng);

        assert!(plan.blast.is_none());
    }

    assert_eq!(creeper.h.fuse, 0);
    assert_eq!(creeper.h.swell_dir, -1);
}

#[test]
fn a_lit_creeper_blows_without_a_player() {
    let view = Flat::new();
    let mut creeper = Mob::new(9, Kind::Creeper, (0.5, 1.0, 0.5));

    creeper.h.lit = true;

    let blasts = hunt(&mut creeper, &view, &ctx(), 40);

    assert!(!blasts.is_empty());
    assert_eq!(blasts[0].0, 29, "на 30-м такте запала");
}

#[test]
fn a_skeleton_draws_twenty_ticks_and_shoots_at_the_interval() {
    let view = Flat::new();
    let players = [player_at(1, 8.5, 0.5)];
    let mut skeleton = Mob::new(9, Kind::Skeleton, (0.5, 1.0, 0.5));
    let shots = hunt(&mut skeleton, &view, &ctx_for(&players, 2, 15, false), 300);

    assert!(shots.len() >= 4, "выстрелов: {}", shots.len());

    let gaps: Vec<usize> = shots.windows(2).map(|pair| pair[1].0 - pair[0].0).collect();

    assert!(gaps.iter().all(|gap| *gap == 60), "раз в три секунды: {gaps:?}");

    let shot = shots[0].1.shot.expect("выстрел");

    assert!(shot.aim.0 > 7.0 && (shot.from.1 - (1.0 + skeleton.eye() - 0.1)).abs() < 1.0e-9);

    // На трудной — вдвое чаще.
    let mut hard = Mob::new(10, Kind::Skeleton, (0.5, 1.0, 0.5));
    let quick = hunt(&mut hard, &view, &ctx_for(&players, 3, 15, false), 300);
    let gaps: Vec<usize> = quick.windows(2).map(|pair| pair[1].0 - pair[0].0).collect();

    assert!(gaps.iter().all(|gap| *gap == 40), "{gaps:?}");
    assert_eq!(hostile::arrow_error(1), 10.0);
    assert_eq!(hostile::arrow_error(2), 6.0);
    assert_eq!(hostile::arrow_error(3), 2.0);
}

#[test]
fn a_spider_climbs_a_wall_toward_the_player() {
    let mut flat = Flat::new();

    for y in 1..6 {
        for z in -3..4 {
            flat.stone.push((4, y, z));
        }
    }

    // Игрок на башне за стеной, стена выше головы.
    let view = flat;
    let mut spider = Mob::new(9, Kind::Spider, (2.5, 1.0, 0.5));
    let mut rng = Rng::new(3);
    let mut top = 1.0_f64;
    let players = [ai::Watcher { y: 9.0, ..player_at(1, 4.5, 0.5) }];

    for _ in 0..120 {
        spider.tick(&view, &ctx_for(&players, 2, 4, false), &mut rng);
        top = top.max(spider.y);
    }

    assert!(top > 5.0, "паук поднялся по стене: {top}");
}

#[test]
fn a_spider_is_neutral_in_daylight_unless_hit() {
    let view = Sunny(Flat::new());
    let players = [player_at(1, 5.5, 0.5)];
    let mut spider = Mob::new(9, Kind::Spider, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(3);

    for _ in 0..40 {
        spider.tick(&view, &ctx_for(&players, 2, 15, true), &mut rng);
    }

    assert_eq!(spider.h.target, None, "днём мирный");

    // Ударили — нападает.
    spider.hurt(1.0, Source::PlayerAttack, Some(Push { dir: (1.0, 0.0), strength: 0.4, cause: 1 }));
    spider.tick(&view, &ctx_for(&players, 2, 15, true), &mut rng);
    assert_eq!(spider.h.target, Some(1));
    assert!(spider.h.provoked);

    // В темноте (свет 4 ночью) — нападает сам.
    let dark = Sunny(Flat::new());
    let mut night = Mob::new(10, Kind::Spider, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        night.tick(&dark, &ctx_for(&players, 2, 4, false), &mut rng);
    }

    assert_eq!(night.h.target, Some(1));
}

#[test]
fn undead_burn_in_the_sun_and_not_in_the_shade_or_with_a_helmet() {
    let sunny = Sunny(Flat::new());
    let shade = Flat::new();
    let day = ctx_for(&[], 2, 15, true);
    let mut rng = Rng::new(5);
    let mut zombie = Mob::new(1, Kind::Zombie, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        zombie.tick(&sunny, &day, &mut rng);
    }

    assert!(zombie.vitals.fire > 0 || zombie.vitals.health < 20.0, "загорелся");
    assert!(zombie.vitals.health < 20.0, "огонь бьёт: {}", zombie.vitals.health);

    let mut skeleton = Mob::new(2, Kind::Skeleton, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        skeleton.tick(&shade, &day, &mut rng);
    }

    assert_eq!((skeleton.vitals.fire, skeleton.vitals.health), (0, 20.0), "в тени не горит");

    let mut helmet = Mob::new(3, Kind::Zombie, (0.5, 1.0, 0.5));

    helmet.h.helmet = true;

    for _ in 0..200 {
        helmet.tick(&sunny, &day, &mut rng);
    }

    assert_eq!(helmet.vitals.health, 20.0, "со шлемом не горит");

    // Ночью не горит.
    let mut night = Mob::new(4, Kind::Zombie, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        night.tick(&sunny, &ctx_for(&[], 2, 4, false), &mut rng);
    }

    assert_eq!(night.vitals.health, 20.0);

    // Крипер и паук не горят.
    let mut creeper = Mob::new(5, Kind::Creeper, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        creeper.tick(&sunny, &day, &mut rng);
    }

    assert_eq!(creeper.vitals.health, 20.0);
}

#[test]
fn water_puts_out_a_burning_zombie() {
    let mut flat = Flat::new();

    flat.water.push((0, 1, 0));
    flat.water.push((0, 2, 0));

    let mut zombie = Mob::new(1, Kind::Zombie, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(5);

    zombie.no_ai = true;
    zombie.vitals.ignite(160);
    zombie.tick(&flat, &ctx(), &mut rng);
    assert_eq!(zombie.vitals.fire, 0);
}

#[test]
fn fire_damage_lands_every_twenty_ticks() {
    let view = Flat::new();
    let mut zombie = Mob::new(1, Kind::Zombie, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(5);

    zombie.no_ai = true;
    zombie.vitals.ignite(160);

    let mut hurts = 0;

    for _ in 0..160 {
        zombie.tick(&view, &ctx(), &mut rng);
        hurts += zombie.events.drain(..).filter(|event| matches!(event, Event::Hurt { source: Source::OnFire, .. })).count();
    }

    assert_eq!(hurts, 8);
    assert_eq!(zombie.vitals.health, 12.0);
}

#[test]
fn hostile_mobs_despawn_far_from_players() {
    let mut rng = Rng::new(1);
    let mut far = Mob::new(1, Kind::Zombie, (0.5, 1.0, 0.5));

    hostile::despawn_check(&mut far, &[player_at(1, 200.5, 0.5)], &mut rng, false);
    assert!(far.h.despawn, "дальше 128 — сразу");

    let mut mid = Mob::new(2, Kind::Zombie, (0.5, 1.0, 0.5));

    for _ in 0..600 {
        hostile::despawn_check(&mut mid, &[player_at(1, 50.5, 0.5)], &mut rng, false);
    }

    assert!(!mid.h.despawn, "первые 30 секунд не исчезает");

    for _ in 0..20_000 {
        hostile::despawn_check(&mut mid, &[player_at(1, 50.5, 0.5)], &mut rng, false);
    }

    assert!(mid.h.despawn, "потом исчезает случайно");

    // Без игроков не исчезает.
    let mut alone = Mob::new(4, Kind::Zombie, (0.5, 1.0, 0.5));

    hostile::despawn_check(&mut alone, &[], &mut rng, false);
    assert!(!alone.h.despawn);

    let mut near = Mob::new(3, Kind::Zombie, (0.5, 1.0, 0.5));

    for _ in 0..5000 {
        hostile::despawn_check(&mut near, &[player_at(1, 10.5, 0.5)], &mut rng, false);
    }

    assert!(!near.h.despawn, "рядом остаётся");

    // Мирная сложность убирает всех сразу.
    hostile::despawn_check(&mut near, &[player_at(1, 10.5, 0.5)], &mut rng, true);
    assert!(near.h.despawn);
}

#[test]
fn the_hostile_state_survives_the_region_file() {
    for kind in Kind::ALL {
        let mut mob = Mob::new(0, kind, (3.5, 70.0, 4.5));

        mob.h.helmet = true;
        mob.h.lit = kind == Kind::Creeper;

        let mut bytes = Vec::new();

        mob.write(&mut bytes);

        let back = Mob::read(&mut bytes.as_slice()).expect("читается");

        assert_eq!(back.kind, kind);
        assert_eq!((back.h.helmet, back.h.lit), (true, kind == Kind::Creeper));
    }
}

#[test]
fn a_hit_by_a_player_targets_the_attacker() {
    let mut zombie = Mob::new(1, Kind::Zombie, (0.5, 1.0, 0.5));

    zombie.hurt(1.0, Source::PlayerAttack, Some(Push { dir: (1.0, 0.0), strength: 0.4, cause: 7 }));
    assert_eq!((zombie.h.target, zombie.by_player), (Some(7), true));
    // Зомби не пугается, как корова.
    assert!(!zombie.brain.is_panicking());
}

#[test]
fn natural_spawning_needs_dark_air_over_a_solid_floor() {
    let view = Flat::new();
    let mut rng = Rng::new(1);

    assert!(spawn::spawnable(&view, 5, 1, 5, 2));
    assert!(!spawn::spawnable(&view, 5, 0, 5, 2), "внутри пола");
    assert!(!spawn::spawnable(&view, 5, 3, 5, 2), "в воздухе");

    // Блочный свет запрещает; небесный не выше случайного 0…7.
    assert!(!spawn::dark_enough(0, 1, &mut rng));
    assert!(!spawn::dark_enough(15, 0, &mut rng));

    let hits = (0..8000).filter(|_| spawn::dark_enough(4, 0, &mut rng)).count();

    assert!((3800..=4200).contains(&hits), "небо 4: шанс 4/8, было {hits}");
    assert_eq!((0..100).filter(|_| spawn::dark_enough(0, 0, &mut rng)).count(), 100);

    let players = [(0.0, 5.0, 0.0)];
    let mut id = 0;
    let mut groups = 0;

    for seed in 0..400 {
        let mut rng = Rng::new(seed);
        let found = spawn::hostile_attempt(&view, &players, &|_, _| 40, &spawn::HOSTILE, &mut rng, &mut || {
            id += 1;
            id
        });

        for mob in &found {
            assert!(mob.kind.hostile());
            assert!(((mob.x - players[0].0).powi(2) + (mob.y - players[0].1).powi(2) + (mob.z - players[0].2).powi(2)).sqrt() >= 24.0, "не ближе 24 блоков");
            assert!((mob.y - 1.0).abs() < 1.0e-9);
        }

        if !found.is_empty() {
            groups += 1;
            // Зомби-житель — 5 % обычной группы зомби, а не отдельная группа.
            let family = |kind| if kind == Kind::ZombieVillager { Kind::Zombie } else { kind };
            assert!(found.len() <= 4 && found.iter().all(|mob| family(mob.kind) == family(found[0].kind)));
        }
    }

    assert!(groups > 0);
}

#[test]
fn a_hostile_mob_gives_five_and_more_for_its_gear() {
    let mut zombie = Mob::new(1, Kind::Zombie, (0.5, 0.0, 0.5));

    zombie.credit_player();
    zombie.hurt(f32::MAX, Source::Generic, None);
    assert_eq!(zombie.pending_xp, 5);

    let mut skeleton = Mob::new(2, Kind::Skeleton, (0.5, 0.0, 0.5));

    skeleton.credit_player();
    skeleton.hurt(f32::MAX, Source::Generic, None);
    assert!((6..=8).contains(&skeleton.pending_xp), "лук: 5 + 1–3, а не {}", skeleton.pending_xp);
}

#[test]
fn dragon_parts_take_quarter_damage_except_head() {
    // Вики, Ender Dragon: «reduces all incoming damage … by ~75% unless the
    // attack is hitting its head»: 8 в голову — 8, в тело — 8/4 + 1 = 3.
    let mut dragon = Mob::new(10, Kind::EnderDragon, (0.0, 64.0, 0.0));

    assert_eq!(dragon.vitals.health, 200.0);
    let head = dragon.hurt_part(8.0, true, Source::PlayerAttack, None).expect("удар прошёл");
    assert_eq!(head.taken, 8.0);
    dragon.vitals.invulnerable = 0;
    let body = dragon.hurt_part(8.0, false, Source::PlayerAttack, None).expect("удар прошёл");
    assert_eq!(body.taken, 3.0);
}

#[test]
fn dragon_dies_for_200_ticks_and_gives_500_xp() {
    let mut dragon = Mob::new(10, Kind::EnderDragon, (0.0, 64.0, 0.0));

    // Обычный урон дракону нипочём (чёрный ящик, dragon-probe) — бьёт игрок.
    assert!(dragon.hurt(f32::MAX, Source::Generic, None).is_none());
    dragon.hurt(f32::MAX, Source::PlayerAttack, None);
    assert!(dragon.is_dead());
    assert_eq!(dragon.dragon.xp, DRAGON_XP);
    dragon.dead_for = DRAGON_DEATH_TICKS;
    assert!(!dragon.gone());
    dragon.dead_for += 1;
    assert!(dragon.gone());

    let mut first = Mob::new(11, Kind::EnderDragon, (0.0, 64.0, 0.0));
    first.first_dragon = true;
    first.hurt(f32::MAX, Source::PlayerAttack, None);
    assert_eq!(first.dragon.xp, DRAGON_XP_FIRST);
}

#[test]
fn crystal_explodes_from_any_hit_and_survives_the_file() {
    let mut crystal = Mob::new(12, Kind::EndCrystal, (3.5, 64.0, 0.5));

    assert!(crystal.hurt(0.0, Source::PlayerAttack, None).is_none());
    assert!(crystal.exploding, "кристалл взрывается от любого удара");

    let mut bytes = Vec::new();
    crystal.write(&mut bytes);
    let back = Mob::read(&mut bytes.as_slice()).expect("запись читается");
    assert_eq!(back.kind, Kind::EndCrystal);

    let mut bytes = Vec::new();
    Mob::new(13, Kind::EnderDragon, (0.0, 64.0, 0.0)).write(&mut bytes);
    assert_eq!(Mob::read(&mut bytes.as_slice()).expect("запись читается").kind, Kind::EnderDragon);
}

#[test]
fn end_mobs_have_the_wiki_numbers() {
    assert_eq!((Kind::Enderman.max_health(), Kind::Shulker.max_health(), Kind::Witch.max_health()), (40.0, 30.0, 26.0));
    assert_eq!((Kind::Enderman.width(), Kind::Enderman.height()), (0.6, 2.9));
    assert_eq!((Kind::Shulker.width(), Kind::Shulker.height()), (1.0, 1.0));
    assert_eq!((Kind::Witch.width(), Kind::Witch.height()), (0.6, 1.95));
    assert_eq!((Kind::Enderman.java_type(), Kind::Shulker.java_type(), Kind::ShulkerBullet.java_type(), Kind::Witch.java_type()), (41, 112, 113, 144));
    assert_eq!((Kind::Enderman.speed(), Kind::Witch.speed()), (0.3, 0.25));
    assert_eq!(Kind::from_name("minecraft:shulker_bullet"), Some(Kind::ShulkerBullet));
    assert!(Kind::ShulkerBullet.object() && !Kind::ShulkerBullet.hostile());
    assert_eq!(Mob::new(1, Kind::Shulker, (0.0, 0.0, 0.0)).h.shell_color, 16);
}

#[test]
fn end_mob_drops_follow_the_wiki() {
    let names = |mob: &Mob| -> Vec<&'static str> {
        let mut rng = Rng::new(1);
        let mut seen = Vec::new();

        for _ in 0..400 {
            for stack in loot(mob, &mut rng) {
                let name = blocks::item_name(stack.item).expect("имя");

                if !seen.contains(&name) {
                    seen.push(name);
                }
            }
        }

        seen.sort_unstable();
        seen
    };

    assert_eq!(names(&Mob::new(1, Kind::Enderman, (0.0, 0.0, 0.0))), ["ender_pearl"]);

    let mut carrying = Mob::new(1, Kind::Enderman, (0.0, 0.0, 0.0));

    carrying.h.carried = blocks::state_by_name("sand");
    assert_eq!(names(&carrying), ["ender_pearl", "sand"]);

    // Блок из рук падает всегда.
    let mut rng = Rng::new(9);
    assert!((0..50).all(|_| loot(&carrying, &mut rng).iter().any(|stack| blocks::item_name(stack.item) == Some("sand"))));

    assert_eq!(names(&Mob::new(1, Kind::Shulker, (0.0, 0.0, 0.0))), ["shulker_shell"]);
    assert_eq!(names(&Mob::new(1, Kind::Witch, (0.0, 0.0, 0.0))), ["glass_bottle", "glowstone_dust", "gunpowder", "redstone", "spider_eye", "stick", "sugar"]);
    assert!(names(&Mob::new(1, Kind::ShulkerBullet, (0.0, 0.0, 0.0))).is_empty());
}

#[test]
fn end_mobs_give_five_xp_to_a_player() {
    for kind in [Kind::Enderman, Kind::Shulker, Kind::Witch] {
        let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));

        mob.no_ai = true;
        mob.credit_player();
        mob.hurt(100.0, Source::PlayerAttack, None);
        assert_eq!(mob.pending_xp, 5, "{kind:?}");
    }
}

/// Игрок в `at`, глядящий на точку `to`.
fn gazing(id: i32, at: (f64, f64), to: (f64, f64, f64)) -> ai::Watcher {
    let eye = 1.0 + 1.62;
    let (dx, dy, dz) = (to.0 - at.0, to.1 - eye, to.2 - at.1);
    let yaw = (-dx.atan2(dz)).to_degrees() as f32;
    let pitch = (-dy.atan2((dx * dx + dz * dz).sqrt())).to_degrees() as f32;

    ai::Watcher { look: (yaw, pitch), ..player_at(id, at.0, at.1) }
}

#[test]
fn a_stare_at_the_head_provokes_the_enderman() {
    let view = Flat::new();
    let mut enderman = Mob::new(5, Kind::Enderman, (0.5, 1.0, 6.5));

    // Взгляд в ноги не злит, в голову — злит на пятом такте (вики: «5 game ticks»).
    let feet = [gazing(1, (0.5, 0.5), (0.5, 1.5, 6.5))];
    let head = [gazing(1, (0.5, 0.5), (0.5, 3.7, 6.5))];

    assert!(!hostile::looks_at_head(&enderman, &feet[0]));
    assert!(hostile::looks_at_head(&enderman, &head[0]));

    let mut rng = Rng::new(4);

    for _ in 0..10 {
        enderman.tick(&view, &ctx_for(&feet, 2, 0, false), &mut rng);
    }

    assert!(enderman.h.target.is_none());

    // Мысль (взгляд) — до хода: стоит на месте, пока не сдвинется.
    let mut ticks = 0;

    while enderman.h.target.is_none() && ticks < 10 {
        let (x, y, z) = (enderman.x, enderman.y, enderman.z);
        let looking = [gazing(1, (0.5, 0.5), (x, y + 2.7, z))];

        enderman.tick(&view, &ctx_for(&looking, 2, 0, false), &mut rng);
        ticks += 1;
    }

    assert_eq!((enderman.h.target, ticks), (Some(1), hostile::STARE_TICKS as usize));

    // Тыква на голове спасает.
    let mut calm = Mob::new(6, Kind::Enderman, (0.5, 1.0, 6.5));
    let masked = [ai::Watcher { pumpkin: true, ..head[0] }];

    for _ in 0..10 {
        calm.tick(&view, &ctx_for(&masked, 2, 0, false), &mut rng);
    }

    assert!(calm.h.target.is_none());
}

#[test]
fn a_hurt_enderman_teleports_away() {
    let view = Flat::new();
    let mut enderman = Mob::new(5, Kind::Enderman, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(2);

    enderman.tick(&view, &ctx(), &mut rng);
    enderman.hurt(2.0, Source::PlayerAttack, Some(Push { dir: (0.0, 1.0), strength: 0.4, cause: 1 }));
    enderman.tick(&view, &ctx(), &mut rng);

    assert_eq!(enderman.teleports, 1);
    assert!(enderman.events.iter().any(|event| matches!(event, Event::Sound { name: "minecraft:entity.enderman.teleport", .. })));
    assert!(enderman.events.contains(&Event::Status(46)));
    // На полу, в пределах 32 блоков по каждой оси.
    assert_eq!(enderman.y, 1.0);
    assert!((enderman.x - 0.5).abs() <= 32.0 && (enderman.z - 0.5).abs() <= 32.0);

    // NoAI не прыгает.
    let mut still = Mob::new(6, Kind::Enderman, (0.5, 1.0, 0.5));

    still.no_ai = true;
    still.hurt(2.0, Source::PlayerAttack, None);
    still.tick(&view, &ctx(), &mut rng);
    assert_eq!(still.teleports, 0);
}

#[test]
fn water_hurts_the_enderman_and_sends_it_away() {
    let mut view = Flat::new();

    view.water.push((0, 1, 0));
    view.water.push((0, 2, 0));

    let mut enderman = Mob::new(5, Kind::Enderman, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(8);

    enderman.tick(&view, &ctx(), &mut rng);

    assert_eq!(enderman.vitals.health, 39.0);
    assert!(enderman.events.iter().any(|event| matches!(event, Event::Hurt { source: Source::Drown, .. })));

    enderman.tick(&view, &ctx(), &mut rng);
    assert_eq!(enderman.teleports, 1);
    assert!(!is_water(view.block(enderman.x.floor() as i32, enderman.y as i32, enderman.z.floor() as i32)));
}

#[test]
fn the_enderman_picks_up_and_places_blocks() {
    struct Grass;

    // Пол каменный, рядом столбик дёрна в три блока: берёт эндермен из
    // области 4×3×4 «vertically encompassing it» (вики) — от ног и выше.
    impl View for Grass {
        fn block(&self, x: i32, y: i32, z: i32) -> i32 {
            let name = match (x, y, z) {
                (_, ..=0, _) => "stone",
                (1, 1..=3, 0) => "grass_block",
                _ => "air",
            };

            blocks::state_by_name(name).expect("блок")
        }

        fn loaded(&self, _: i32, _: i32) -> bool {
            true
        }
    }

    let grass = blocks::state_by_name("grass_block").expect("дёрн");

    assert!(hostile::holdable(grass) && !hostile::holdable(blocks::state_by_name("stone").expect("камень")));

    let mut enderman = Mob::new(5, Kind::Enderman, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(3);
    let mut taken = None;

    // Шанс 1/20 за такт и место в 4×3×4 вокруг: в столбик — изредка.
    for _ in 0..4000 {
        let mut plan = ai::Plan::default();

        hostile::grief(&mut enderman, &Grass, &mut rng, &mut plan);

        if plan.take.is_some() {
            taken = plan.take;
            break;
        }
    }

    let (x, y, z) = taken.expect("взял блок");

    assert_eq!(((x, z), (1..=3).contains(&y), enderman.h.carried), ((1, 0), true, Some(grass)));

    let mut placed = None;

    for _ in 0..40_000 {
        let mut plan = ai::Plan::default();

        hostile::grief(&mut enderman, &Grass, &mut rng, &mut plan);

        if plan.put.is_some() {
            placed = plan.put;
            break;
        }
    }

    let ((_, y, _), state) = placed.expect("поставил блок");

    assert_eq!((y, state, enderman.h.carried), (1, grass, None));
}

#[test]
fn a_closed_shulker_wears_twenty_armor() {
    let mut closed = Mob::new(5, Kind::Shulker, (0.5, 1.0, 0.5));

    closed.h.covered = true;

    let hit = closed.hurt(7.0, Source::PlayerAttack, None).expect("удар");

    // Броня 20: 7·(1 − max(4, 20 − 3,5)/25) = 2,38.
    assert!((hit.taken - 2.38).abs() < 1.0e-4, "{}", hit.taken);
    assert!(closed.events.iter().any(|event| matches!(event, Event::Sound { name: "minecraft:entity.shulker.hurt_closed", .. })));

    let mut open = Mob::new(6, Kind::Shulker, (0.5, 1.0, 0.5));

    open.h.peek = 30;
    assert_eq!(open.hurt(7.0, Source::PlayerAttack, None).expect("удар").taken, 7.0);

    // Призванный закрытым брони не носит (чёрный ящик, shulker-stand).
    let mut still = Mob::new(7, Kind::Shulker, (0.5, 1.0, 0.5));

    still.no_ai = true;
    assert_eq!(still.hurt(7.0, Source::PlayerAttack, None).expect("удар").taken, 7.0);
}

#[test]
fn a_shulker_opens_and_fires_at_a_player() {
    let view = Flat::new();
    let players = [player_at(1, 6.5, 0.5)];
    let ctx = ctx_for(&players, 2, 0, false);
    let mut shulker = Mob::new(5, Kind::Shulker, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(6);
    let mut fired = None;

    for tick in 0..300 {
        let plan = shulker.tick(&view, &ctx, &mut rng);

        if plan.bullet.is_some() {
            fired = Some((tick, plan.bullet));
            break;
        }
    }

    let (_, target) = fired.expect("выстрелил");

    assert_eq!((target, shulker.h.peek), (Some(1), 100));
    // Шалкер не падает и не ходит.
    assert_eq!((shulker.x, shulker.y, shulker.z), (0.5, 1.0, 0.5));
    assert!(shulker.events.iter().any(|event| matches!(event, Event::Sound { name: "minecraft:entity.shulker.open", .. })));
}

#[test]
fn a_shulker_without_a_floor_clings_to_a_wall() {
    let mut view = Flat::new();

    view.floor = -10;
    view.stone.push((1, 5, 0));

    let mut rng = Rng::new(1);

    assert_eq!(hostile::attach_face(&view, (0, 5, 0), &mut rng), Some(5));
    assert_eq!(hostile::attach_face(&view, (0, -9, 0), &mut rng), Some(0));
    assert_eq!(hostile::attach_face(&view, (5, 5, 5), &mut rng), None);
}

#[test]
fn a_shulker_bullet_homes_in_and_hits() {
    let view = Flat::new();
    let players = [player_at(1, 4.5, 4.5)];
    let ctx = ctx_for(&players, 2, 0, false);
    let mut bullet = Mob::new(9, Kind::ShulkerBullet, (0.5, 1.5, 0.5));
    let mut rng = Rng::new(1);

    bullet.h.target = Some(1);

    let mut hit = None;

    for tick in 0..300 {
        let plan = bullet.tick(&view, &ctx, &mut rng);

        if plan.bullet_hit.is_some() {
            hit = Some(tick);
            break;
        }
    }

    assert!(hit.is_some() && bullet.h.despawn);

    // Удар по снаряду гасит его.
    let mut struck = Mob::new(10, Kind::ShulkerBullet, (0.5, 1.5, 0.5));

    assert!(struck.hurt(1.0, Source::PlayerAttack, None).is_none());
    assert!(struck.h.despawn);
}

#[test]
fn a_shulker_bullet_flies_along_one_axis_with_the_black_box_ramp() {
    // Чёрный ящик, shulker-ai: снаряд летит по одной оси, скорость за три такта
    // 0,077, потом 0,123, 0,152 (сдвиг за три такта 672/4096 блока).
    let view = Flat::new();
    let players = [player_at(1, 3.5, 0.5)];
    let ctx = ctx_for(&players, 2, 0, false);
    let mut bullet = Mob::new(9, Kind::ShulkerBullet, (3.5, 1.0, 5.5));
    let mut rng = Rng::new(1);

    bullet.h.target = Some(1);

    let mut speeds = Vec::new();

    for _ in 0..10 {
        bullet.tick(&view, &ctx, &mut rng);
        speeds.push(bullet.vz);
    }

    assert!(bullet.vx == 0.0 && bullet.vy == 0.0);
    assert_eq!((-speeds[2] * 1000.0).round(), 77.0);
    assert_eq!((-speeds[5] * 1000.0).round(), 123.0);
    assert_eq!((-speeds[8] * 1000.0).round(), 152.0);
}

#[test]
fn a_hurt_witch_drinks_healing() {
    let view = Flat::new();
    let mut witch = Mob::new(5, Kind::Witch, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(5);

    witch.vitals.health = 10.0;

    let mut ticks = 0;

    while witch.h.drinking == 0 && ticks < 2000 {
        witch.tick(&view, &ctx(), &mut rng);
        ticks += 1;
    }

    assert_eq!((witch.h.drinking, crate::survival::POTIONS[usize::from(witch.h.potion)]), (hostile::DRINK_TICKS, "healing"));

    for _ in 0..hostile::DRINK_TICKS {
        witch.tick(&view, &ctx(), &mut rng);
    }

    // Лечение — тактом позже конца питья.
    assert_eq!(witch.h.drinking, 0);
    witch.tick(&view, &ctx(), &mut rng);
    assert_eq!(witch.vitals.health, 14.0);

    // Магия ей на 85 % слабее.
    let mut target = Mob::new(6, Kind::Witch, (0.5, 1.0, 0.5));

    target.no_ai = true;
    assert!((target.hurt(6.0, Source::Magic, None).expect("удар").taken - 0.9).abs() < 1.0e-5);
}

#[test]
fn end_mob_state_survives_the_region_file() {
    let mut enderman = Mob::new(0, Kind::Enderman, (3.5, 70.0, 4.5));

    enderman.h.carried = blocks::state_by_name("tnt");

    let mut shulker = Mob::new(0, Kind::Shulker, (3.5, 70.0, 4.5));

    shulker.h.face = 4;
    shulker.h.shell_color = 3;

    let mut bytes = Vec::new();

    enderman.write(&mut bytes);
    shulker.write(&mut bytes);
    Mob::new(0, Kind::Witch, (0.0, 0.0, 0.0)).write(&mut bytes);

    let mut rest = bytes.as_slice();
    let a = Mob::read(&mut rest).expect("эндермен");
    let b = Mob::read(&mut rest).expect("шалкер");
    let c = Mob::read(&mut rest).expect("ведьма");

    assert_eq!(a.h.carried, blocks::state_by_name("tnt"));
    assert_eq!((b.h.face, b.h.shell_color), (4, 3));
    assert_eq!(c.kind, Kind::Witch);
    assert!(rest.is_empty());
}

#[test]
fn endermen_and_witches_join_the_night_spawns() {
    let mut rng = Rng::new(11);
    let mut seen = std::collections::HashSet::new();

    for _ in 0..20_000 {
        seen.insert(spawn::pick_hostile_for_test(&spawn::HOSTILE, &mut rng));
    }

    assert!(seen.contains(&Kind::Enderman) && seen.contains(&Kind::Witch));
    assert!((0..100).all(|_| spawn::pick_hostile_for_test(&spawn::END_HOSTILE, &mut rng) == Kind::Enderman));
}

/// Пол и один блок поверх: рабочее место жителя.
struct Site {
    block: Option<((i32, i32, i32), &'static str)>,
}

impl View for Site {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        match self.block {
            Some((at, name)) if at == (x, y, z) => blocks::state_by_name(name).expect("рабочий блок"),
            _ if y <= 0 => blocks::state_by_name("stone").expect("камень"),
            _ => blocks::state_by_name("air").expect("воздух"),
        }
    }

    fn loaded(&self, _: i32, _: i32) -> bool {
        true
    }
}

/// Житель по вики и чёрному ящику: размеры, здоровье, скорость, номер.
#[test]
fn a_villager_has_the_vanilla_numbers() {
    assert_eq!((Kind::Villager.width(), Kind::Villager.height(), Kind::Villager.max_health()), (0.6, 1.95, 20.0));
    assert_eq!((Kind::Villager.speed(), Kind::Villager.java_type()), (0.5, 139));
    assert_eq!(Kind::from_name("minecraft:villager"), Some(Kind::Villager));
    assert!(!Kind::Villager.hostile());
}

/// Безработный житель занимает компостер рядом: частицы, профессия
/// крестьянина, две сделки новичка; компостер сломан — профессия уходит
/// (чёрный ящик, villager-claim).
#[test]
fn a_villager_claims_and_loses_a_job_site() {
    let mut view = Site { block: Some(((1, 1, 0), "composter")) };
    let mut mob = Mob::new(1, Kind::Villager, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(9);

    for _ in 0..200 {
        crate::villager::step(&mut mob, &view, 3000, &[], &mut rng);

        if mob.villager.job_site.is_some() {
            break;
        }
    }

    assert_eq!(mob.villager.job_site, Some((1, 1, 0)));
    assert_eq!(mob.villager.profession, crate::villager::profession_named("farmer").unwrap());
    assert_eq!(mob.villager.offers.len(), 2);
    assert!(mob.events.contains(&Event::Status(14)));

    // Чужой рабочий блок не занимается.
    let mut other = Mob::new(2, Kind::Villager, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        crate::villager::step(&mut other, &view, 3000, &[(1, 1, 0)], &mut rng);
    }

    assert_eq!(other.villager.job_site, None);

    view.block = None;
    crate::villager::step(&mut mob, &view, 3000, &[], &mut rng);
    assert_eq!((mob.villager.profession, mob.villager.offers.len()), (crate::villager::NONE, 0));
}

/// Торговавший житель профессию без рабочего места держит; новичок без
/// опыта — теряет сразу (чёрный ящик, villager-levelup-ai).
#[test]
fn only_a_villager_that_traded_keeps_its_profession() {
    let view = Site { block: None };
    let mut rng = Rng::new(3);
    let mut fresh = Mob::new(1, Kind::Villager, (0.5, 1.0, 0.5));
    let mut traded = Mob::new(2, Kind::Villager, (0.5, 1.0, 0.5));

    fresh.villager.profession = 5;
    traded.villager.profession = 5;
    traded.villager.xp = 5;
    crate::villager::step(&mut fresh, &view, 3000, &[], &mut rng);
    crate::villager::step(&mut traded, &view, 3000, &[], &mut rng);
    assert_eq!((fresh.villager.profession, traded.villager.profession), (0, 5));
}

/// Новый уровень — через 40 тактов без торговли: две новые сделки и
/// регенерация (чёрный ящик, villager-levelup-ai; вики, Trading).
#[test]
fn a_villager_levels_up_forty_ticks_after_the_trade() {
    let view = Site { block: None };
    let mut rng = Rng::new(4);
    let mut mob = Mob::new(1, Kind::Villager, (0.5, 1.0, 0.5));

    mob.villager.profession = 5;
    mob.villager.xp = 10;
    mob.villager.add_trades(1, &mut rng);
    mob.villager.level_timer = crate::villager::LEVEL_UP_DELAY;
    mob.villager.level_pending = true;
    mob.villager.trading = Some(7);

    for _ in 0..50 {
        crate::villager::step(&mut mob, &view, 3000, &[], &mut rng);
    }

    assert_eq!(mob.villager.level, 1, "пока торгуют — ждёт");
    mob.villager.trading = None;

    for _ in 0..39 {
        crate::villager::step(&mut mob, &view, 3000, &[], &mut rng);
    }

    assert_eq!(mob.villager.level, 1);
    crate::villager::step(&mut mob, &view, 3000, &[], &mut rng);
    assert_eq!((mob.villager.level, mob.villager.offers.len()), (2, 4));
    assert!(mob.vitals.has_effect(crate::survival::EffectKind::Regeneration));
}

/// Мотание головой: счёт 40 убывает каждый такт и у NoAI (чёрный ящик,
/// villager-shake).
#[test]
fn the_head_shake_counts_down_every_tick() {
    let mut mob = Mob::new(1, Kind::Villager, (0.5, 1.0, 0.5));

    mob.no_ai = true;
    mob.villager.unhappy = crate::villager::UNHAPPY_TICKS;
    run(&mut mob, &Flat::new(), 15);
    assert_eq!(mob.villager.unhappy, 25);
}

/// Житель переживает запись в чанк: профессия, уровень, сделки.
#[test]
fn a_villager_survives_the_chunk_file() {
    let mut mob = Mob::new(1, Kind::Villager, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(2);

    mob.villager.profession = 9;
    mob.villager.level = 3;
    mob.villager.add_trades(1, &mut rng);

    let mut bytes = Vec::new();

    mob.write(&mut bytes);

    let back = Mob::read(&mut bytes.as_slice()).expect("житель читается");

    assert_eq!(back.kind, Kind::Villager);
    assert_eq!((back.villager.profession, back.villager.level), (9, 3));
    assert_eq!(back.villager.offers, mob.villager.offers);
}

/// Вылеченный зомби-житель: житель с теми же профессией и сделками; вылечивший
/// получает репутацию, вид — равнинный (чёрный ящик, zombie-villager-cured).
#[test]
fn a_cured_zombie_villager_keeps_trades_and_remembers_its_healer() {
    let mut rng = Rng::new(3);
    let mut zombie = Mob::new(10, Kind::ZombieVillager, (1.5, 1.0, 1.5));

    zombie.villager.profession = 5;
    zombie.villager.kind = 6;
    zombie.villager.level = 2;
    zombie.villager.xp = 5;
    zombie.villager.add_trades(1, &mut rng);
    zombie.villager.converter = Some([4; 16]);
    zombie.no_ai = true;

    let offers = zombie.villager.offers.clone();
    let villager = cure(&mut zombie, 11);

    assert_eq!((villager.kind, villager.entity_id, villager.no_ai), (Kind::Villager, 11, true));
    assert_eq!((villager.villager.profession, villager.villager.level, villager.villager.xp, villager.villager.kind), (5, 2, 5, crate::villager::PLAINS));
    assert_eq!(villager.villager.offers, offers);
    assert_eq!(villager.villager.gossip, vec![([4; 16], CURE_REPUTATION)]);
    assert!(villager.villager.fresh_cure);
}

/// Зомби-житель записывается в файл и читается: профессия и счёт лечения целы.
#[test]
fn a_zombie_villager_survives_the_region_file() {
    let mut zombie = Mob::new(10, Kind::ZombieVillager, (1.5, 1.0, 1.5));

    zombie.villager.profession = 9;
    zombie.villager.conversion = 777;

    let mut bytes = Vec::new();

    zombie.write(&mut bytes);

    let back = Mob::read(&mut bytes.as_slice()).unwrap();

    assert_eq!((back.kind, back.villager.profession, back.villager.conversion), (Kind::ZombieVillager, 9, 777));
}

/// Зомби идёт на жителя, когда игрока нет (вики, Zombie: цели — житель).
#[test]
fn a_zombie_without_players_walks_to_a_villager() {
    let view = Flat::new();
    let mut rng = Rng::new(1);
    let mut zombie = Mob::new(1, Kind::Zombie, (0.5, 1.0, 0.5));
    let mates = [ai::Mate { id: 2, kind: Kind::Villager, x: 6.5, y: 1.0, z: 0.5, love: 0, baby: false, ready: true }];
    let ctx = ai::Ctx { players: &[], mates: &mates, difficulty: 2, sky: 0, sun_up: false };

    zombie.on_ground = true;

    for _ in 0..400 {
        zombie.tick(&view, &ctx, &mut rng);

        if zombie.h.target.is_some() {
            break;
        }
    }

    assert_eq!(zombie.h.target, Some(2));
}

/// Мобы Незера: числа с вики и чёрного ящика (magma-cube-stand: размер 2 даёт
/// здоровье 4, броню 6; ghast-stand и прочие *-stand).
#[test]
fn nether_kinds_have_the_wiki_numbers() {
    assert_eq!(Kind::Blaze.max_health(), 20.0);
    assert_eq!(Kind::Ghast.max_health(), 10.0);
    assert_eq!(Kind::PiglinBrute.max_health(), 50.0);
    assert_eq!(Kind::Hoglin.max_health(), 40.0);
    assert_eq!(Kind::Piglin.max_health(), 16.0);
    assert!(Kind::Blaze.nether() && Kind::Hoglin.nether() && !Kind::Strider.nether());
    assert_eq!(Kind::from_name("minecraft:zombified_piglin"), Some(Kind::ZombifiedPiglin));
    assert_eq!(Kind::Strider.java_type(), 129);
}

/// Зомби-пиглин злится после удара и зовёт стаю; без удара спокоен.
#[test]
fn a_hit_zombified_piglin_is_angry_and_alerts_the_pack() {
    let mut piglin = Mob::new(1, Kind::ZombifiedPiglin, (0.0, 0.0, 0.0));

    assert_eq!(piglin.nether.anger, 0);

    piglin.hurt(1.0, crate::survival::Source::PlayerAttack, Some(Push { dir: (0.0, 1.0), strength: 0.4, cause: 9 }));

    assert!((400..780).contains(&piglin.nether.anger) && piglin.nether.alert);
}

/// Магмовый куб рождается размера 2, его ящик растёт с размером.
#[test]
fn a_magma_cube_scales_with_its_size() {
    let mut cube = Mob::new(1, Kind::MagmaCube, (0.0, 0.0, 0.0));

    assert_eq!(cube.nether.size, 2);

    nether::set_size(&mut cube, 4);

    assert_eq!(cube.vitals.max_health, 16.0);
    assert!((cube.size().0 - 2.08).abs() < 1e-9);
}

/// Вики, Slime: квадрат здоровья, размер ящика и скорость по размеру.
#[test]
fn slime_size_stats_and_registry() {
    assert_eq!(Kind::from_name("minecraft:slime"), Some(Kind::Slime));
    assert_eq!(Kind::Slime.java_type(), 117);
    assert!(Kind::Slime.hostile() && !Kind::Slime.nether());
    for (size, hp, width, damage, speed) in [(1, 1.0, 0.52, 0.0, 0.3), (2, 4.0, 1.04, 2.0, 0.4), (4, 16.0, 2.08, 4.0, 0.6)] {
        let mut mob = Mob::new(7, Kind::Slime, (0.5, 1.0, 0.5));
        slime::set_size(&mut mob, size);
        assert_eq!((mob.vitals.health, mob.vitals.max_health), (hp, hp));
        assert_eq!(mob.size(), (width, width));
        assert_eq!(mob.armor(), 0.0);
        assert_eq!(slime::damage(size), damage);
        assert!((slime::speed(size) - speed).abs() < 1.0e-7);
    }
}

/// Контрольные точки отдельно вычислены по формуле вики с int32/long64.
#[test]
fn slime_chunks_use_seed_signed_coordinates_and_java_overflow() {
    for (seed, x, z, expected) in [
        (0, 0, 0, false), (0, -2, 0, true), (0, 2, 2, true),
        (0, -5, 5, true), (0, 1, -3, true), (1, -30, 14, true),
        (0, -30, 14, false), (-1, -1, -1, false),
        (123456789, 1875000, -1875000, false),
        (i64::MAX, i32::MAX, i32::MIN, false),
        (i64::MIN, 1874982, -1875000, true), (-42, 1874995, -1875000, true),
    ] {
        assert_eq!(slime::is_slime_chunk(seed, x, z), expected, "{seed}, {x}, {z}");
    }
    // Примерно десятая часть чанков; никакого цикла из 16 блоков.
    let count = (-100..100).flat_map(|x| (-100..100).map(move |z| (x, z))).filter(|&(x, z)| slime::is_slime_chunk(42, x, z)).count();
    assert!((3500..4500).contains(&count), "{count}");
}

#[test]
fn slime_split_is_two_to_four_half_size_children_only_after_death() {
    let mut rng = Rng::new(9123);
    let mut id = 100;
    let mut next_id = || { id += 1; id };
    let mut counts = std::collections::HashSet::new();
    for size in [1, 2, 4] {
        let mut parent = Mob::new(1, Kind::Slime, (0.5, 1.0, 0.5));
        slime::set_size(&mut parent, size);
        assert!(slime::split(&parent, &mut rng, &mut next_id).is_empty());
        parent.no_ai = true;
        parent.silent = true;
        parent.nether.persistence_required = true;
        parent.dead_for = DEATH_TICKS;
        assert!(slime::split(&parent, &mut rng, &mut next_id).is_empty());
        parent.dead_for += 1;
        for _ in 0..60 {
            let children = slime::split(&parent, &mut rng, &mut next_id);
            if size == 1 {
                assert!(children.is_empty());
                continue;
            }
            assert!((2..=4).contains(&children.len()));
            counts.insert(children.len());
            for child in children {
                assert_eq!(child.kind, Kind::Slime);
                assert_eq!(child.nether.size, size / 2);
                assert_eq!(child.vitals.health, ((size / 2).pow(2)) as f32);
                assert!(child.no_ai && child.silent && child.nether.persistence_required);
                assert_eq!(child.pending_xp, 0);
                assert_eq!(child.xp_until, 0);
            }
        }
        parent.h.despawn = true;
        assert!(slime::split(&parent, &mut rng, &mut next_id).is_empty());
    }
    assert_eq!(counts.len(), 3);
}

#[test]
fn slime_contact_has_no_melee_cooldown_and_small_is_harmless() {
    let view = Flat::new();
    let player = player_at(90, 0.6, 0.5);
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    assert!(slime::contact(&mob, &view, &player).is_none());
    slime::set_size(&mut mob, 2);
    mob.h.cooldown = 20;
    let hit = slime::contact(&mob, &view, &player).unwrap();
    assert_eq!((hit.target, hit.damage), (90, 2.0));
    assert!(slime::contact(&mob, &view, &player).is_some());
    assert!(slime::contact(&mob, &view, &ai::Watcher { targetable: false, ..player }).is_none());
    assert!(slime::contact(&mob, &view, &ai::Watcher { spectator: true, ..player }).is_none());
    assert!(slime::contact(&mob, &view, &ai::Watcher { x: 1.6, ..player }).is_none(), "нет касания");
    mob.die();
    assert!(slime::contact(&mob, &view, &player).is_none());
}

#[test]
fn slime_with_no_ai_does_not_touch_players() {
    let mut items = Items::new();
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    slime::set_size(&mut mob, 2);
    mob.no_ai = true;
    items.add_mob(mob);
    let players = [player_at(90, 0.6, 0.5), player_at(91, 0.4, 0.5)];
    let env = Env { difficulty: 2, sky: 15, sun_up: false, clock: 0 };
    let outcome = step_all(&mut items, &Flat::new(), &players, env, &mut Rng::new(8), &mut || 123);
    assert!(outcome.strikes.is_empty(), "чёрный ящик, slime-attack: NoAI не бьёт");
}

#[test]
fn slime_hops_at_target_and_waits_on_ground() {
    let view = Flat::new();
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    slime::set_size(&mut mob, 2);
    mob.nether.hop = 0;
    mob.on_ground = true;
    mob.h.target = Some(90);
    let players = [player_at(90, 8.5, 0.5)];
    let ctx = ctx_for(&players, 2, 15, false);
    let mut rng = Rng::new(8);
    let plan = mob.tick(&view, &ctx, &mut rng);
    assert!(plan.jump && mob.x > 0.5 && mob.y > 1.0);
    assert!((7..=19).contains(&mob.nether.hop) && mob.nether.hop % 2 == 1, "2·(10..29)/3 + 1");
    let delay = mob.nether.hop;
    mob.tick(&view, &ctx, &mut rng);
    assert_eq!(mob.nether.hop, delay, "в воздухе задержка не убывает");
    let mut landed = false;
    for _ in 0..30 {
        mob.tick(&view, &ctx, &mut rng);
        if mob.on_ground { landed = true; break; }
    }
    assert!(landed);
    assert!(!mob.tick(&view, &ctx, &mut rng).jump, "ждёт после приземления");
    assert!(mob.events.iter().any(|event| matches!(event, Event::SoundAt { name: "minecraft:entity.slime.jump", .. })));
    assert!(mob.events.iter().any(|event| matches!(event, Event::Sound { name: "minecraft:entity.slime.squish", .. })));
}

#[test]
fn slime_hop_lands_about_a_block_from_takeoff() {
    // Чёрный ящик, slime-hop: прыжок среднего слизня ≈0,95 блока (0,875–1,0).
    let view = Flat::new();
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    slime::set_size(&mut mob, 2);
    mob.nether.hop = 0;
    mob.on_ground = true;
    mob.yaw = 0.0;
    mob.h.target = Some(90);
    let players = [player_at(90, 0.5, 20.5)];
    let ctx = ctx_for(&players, 2, 15, false);
    let mut rng = Rng::new(8);
    mob.tick(&view, &ctx, &mut rng);
    for _ in 0..30 {
        if mob.on_ground { break; }
        mob.tick(&view, &ctx, &mut rng);
    }
    assert!(mob.on_ground);
    let hop = (mob.x - 0.5).hypot(mob.z - 0.5);
    assert!((0.85..1.05).contains(&hop), "{hop}");
}

#[test]
fn slime_size_and_flags_survive_region_round_trip() {
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    slime::set_size(&mut mob, 4);
    mob.vitals.health = 11.0;
    mob.silent = true;
    mob.no_ai = true;
    mob.nether.persistence_required = true;
    let mut bytes = Vec::new();
    mob.write(&mut bytes);
    let mut input = bytes.as_slice();
    let restored = Mob::read(&mut input).unwrap();
    assert!(input.is_empty());
    assert_eq!(restored.nether.size, 4);
    assert_eq!((restored.vitals.health, restored.vitals.max_health), (11.0, 16.0));
    assert!(restored.silent && restored.no_ai && restored.nether.persistence_required);
}

#[test]
fn slime_drops_only_small_and_xp_tracks_size() {
    let mut rng = Rng::new(12);
    for size in [1, 2, 4] {
        let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
        slime::set_size(&mut mob, size);
        mob.looting = 3;
        for _ in 0..100 {
            let drops = loot(&mob, &mut rng);
            if size > 1 { assert!(drops.is_empty()); }
            for drop in drops {
                assert_eq!(blocks::item_name(drop.item), Some("slime_ball"));
                assert!((1..=5).contains(&drop.count));
            }
        }
        mob.credit_player();
        mob.hurt(100.0, Source::PlayerAttack, None);
        assert_eq!(mob.pending_xp, size);
    }
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    mob.die();
    assert_eq!(mob.pending_xp, 0);
    let mean = (0..10000).map(|_| slime::slimeballs(3, &mut rng)).sum::<i32>() as f64 / 10000.0;
    assert!((mean - 2.5).abs() < 0.1);
}

#[test]
fn slime_spawning_obeys_chunk_moon_light_height_and_biome() {
    let mut rng = Rng::new(12);
    // Чанк (-2, 0) при семени 0 — слизневый, любой свет.
    assert!((0..200).any(|_| slime::can_spawn(0, (-32, 39, 0), "plains", 15, 0, &mut rng)));
    for _ in 0..100 {
        assert!(!slime::can_spawn(0, (-32, 40, 0), "plains", 0, 0, &mut rng));
        assert!(!slime::can_spawn(0, (0, 20, 0), "plains", 0, 0, &mut rng));
        assert!(!slime::can_spawn(0, (-32, 20, 0), "mushroom_fields", 0, 0, &mut rng));
        assert!(!slime::can_spawn(0, (-32, 20, 0), "deep_dark", 0, 0, &mut rng));
        assert!(!slime::can_spawn(0, (0, 60, 0), "swamp", 0, 4 * 24000, &mut rng));
        assert!(!slime::can_spawn(0, (0, 60, 0), "swamp", 8, 0, &mut rng));
        assert!(!slime::can_spawn(0, (0, 50, 0), "swamp", 0, 0, &mut rng));
        assert!(!slime::can_spawn(0, (0, 70, 0), "swamp", 0, 0, &mut rng));
    }
    for biome in ["swamp", "mangrove_swamp"] {
        assert!((0..200).any(|_| slime::can_spawn(0, (0, 60, 0), biome, 7, 18000, &mut rng)));
    }
    assert_eq!(slime::moon_brightness(8 * 24000), 1.0);
    assert_eq!(slime::moon_brightness(4 * 24000), 0.0);
}

#[test]
fn slime_despawns_peaceful_or_far_without_splitting() {
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    slime::set_size(&mut mob, 4);
    let mut rng = Rng::new(8);
    hostile::despawn_check(&mut mob, &[player_at(90, 130.0, 0.5)], &mut rng, false);
    assert!(mob.h.despawn);
    assert!(slime::split(&mob, &mut rng, &mut || 99).is_empty());
    mob.h.despawn = false;
    mob.nether.persistence_required = true;
    hostile::despawn_check(&mut mob, &[player_at(90, 130.0, 0.5)], &mut rng, false);
    assert!(!mob.h.despawn);
    hostile::despawn_check(&mut mob, &[], &mut rng, true);
    assert!(mob.h.despawn);
}

#[test]
fn slime_spawn_needs_space_for_the_whole_size() {
    let mut view = Flat::new();
    view.stone.push((1, 1, 0));
    let mut mob = Mob::new(3, Kind::Slime, (0.5, 1.0, 0.5));
    assert!(spawn::fits(&view, &mob));
    slime::set_size(&mut mob, 4);
    assert!(!spawn::fits(&view, &mob));
}

#[test]
fn small_arthropods_stats_eggs_and_persistence() {
    for (kind, name, id, health, size, speed) in [
        (Kind::CaveSpider, "cave_spider", 22, 12.0, (0.7, 0.5), 0.3),
        (Kind::Silverfish, "silverfish", 114, 8.0, (0.4, 0.3), 0.25),
    ] {
        assert_eq!(Kind::from_name(&format!("minecraft:{name}")), Some(kind));
        assert_eq!(kind.java_type(), id);
        assert_eq!(kind.max_health(), health);
        assert_eq!(kind.speed(), speed);
        assert!(kind.hostile());
        assert!(blocks::item_named(&format!("{name}_spawn_egg")).is_some());
        let mob = Mob::new(1, kind, (1.5, 64.0, -2.5));
        assert_eq!(mob.size(), size);
        let mut saved = Vec::new();
        mob.write(&mut saved);
        assert_eq!(Mob::read(&mut saved.as_slice()), Some(mob));
        assert_eq!(crate::spawner::Data::of(name).kind(), Some(kind));
    }
    assert_eq!(Kind::CaveSpider.sounds(), Kind::Spider.sounds());
    assert_eq!(Kind::Silverfish.step_sound(), "minecraft:entity.silverfish.step");
}

#[test]
fn cave_spider_climbs_but_silverfish_does_not() {
    let mut flat = Flat::new();
    flat.stone = (1..=6).map(|y| (1, y, 0)).collect();
    for kind in [Kind::CaveSpider, Kind::Silverfish] {
        let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
        mob.on_ground = true;
        mob.face(270.0);
        let mut climbed = false;
        let mut top = mob.y;
        for _ in 0..80 {
            mob.physics(&flat, 1.0, false);
            climbed |= mob.h.climbing;
            top = top.max(mob.y);
        }
        assert_eq!(climbed, kind == Kind::CaveSpider);
        assert_eq!(top > 2.0, kind == Kind::CaveSpider);
    }
}

#[test]
fn silverfish_surviving_damage_schedules_one_call() {
    let mut mob = Mob::new(1, Kind::Silverfish, (0.0, 1.0, 0.0));
    mob.hurt(1.0, Source::PlayerAttack, None).unwrap();
    assert_eq!(mob.h.silverfish_call, arthropod::CALL_DELAY);
    mob.h.silverfish_call = 7;
    mob.vitals.tick_invulnerable();
    mob.hurt(2.0, Source::PlayerAttack, None).unwrap();
    assert_eq!(mob.h.silverfish_call, 7);

    for source in [Source::PlayerAttack, Source::Magic] {
        let mut killed = Mob::new(2, Kind::Silverfish, (0.0, 1.0, 0.0));
        killed.hurt(8.0, source, None).unwrap();
        assert!(killed.is_dead());
        assert_eq!(killed.h.silverfish_call, 0);
    }
    let mut projectile = Mob::new(4, Kind::Silverfish, (0.0, 1.0, 0.0));
    projectile.hurt(1.0, Source::Fireball, Some(Push { cause: 42, dir: (1.0, 0.0), strength: 0.0 })).unwrap();
    assert_eq!(projectile.h.silverfish_call, arthropod::CALL_DELAY);

    for source in [Source::Fall, Source::OnFire, Source::InWall] {
        let mut quiet = Mob::new(3, Kind::Silverfish, (0.0, 1.0, 0.0));
        quiet.hurt(1.0, source, None).unwrap();
        assert_eq!(quiet.h.silverfish_call, 0);
    }
}

#[test]
fn poison_hurts_silverfish_and_cave_spider_is_immune() {
    for kind in [Kind::Silverfish, Kind::CaveSpider] {
        let mut mob = Mob::new(1, kind, (0.0, 1.0, 0.0));
        mob.no_ai = true;
        mob.vitals.give_effect(crate::survival::EffectKind::Poison, 0, 25);
        mob.tick(&Flat::new(), &ctx(), &mut Rng::new(4));
        assert_eq!(mob.vitals.health, kind.max_health() - if kind == Kind::Silverfish { 1.0 } else { 0.0 });
        assert_eq!(mob.h.silverfish_call, if kind == Kind::Silverfish { arthropod::CALL_DELAY } else { 0 });
    }
}

#[test]
fn arthropod_loot_and_xp_credit() {
    let mut rng = Rng::new(17);
    for kind in [Kind::CaveSpider, Kind::Silverfish] {
        let mut mob = Mob::new(1, kind, (0.0, 1.0, 0.0));
        mob.credit_player();
        mob.hurt(100.0, Source::PlayerAttack, None).unwrap();
        assert_eq!(mob.pending_xp, 5);
        if kind == Kind::Silverfish {
            assert!(loot(&mob, &mut rng).is_empty());
        }
    }
    let mut cave = Mob::new(2, Kind::CaveSpider, (0.0, 1.0, 0.0));
    let eye = blocks::item_named("spider_eye").unwrap();
    let string = blocks::item_named("string").unwrap();
    for _ in 0..200 {
        assert!(loot(&cave, &mut rng).iter().all(|stack| stack.item == string && stack.count <= 2));
    }
    cave.by_player = true;
    cave.looting = 3;
    let mut saw_eye = false;
    for _ in 0..200 {
        for stack in loot(&cave, &mut rng) {
            assert!(stack.count > 0);
            assert!(stack.item == string && stack.count <= 5 || stack.item == eye && stack.count <= 4);
            saw_eye |= stack.item == eye;
        }
    }
    assert!(saw_eye);
}

#[test]
fn small_arthropod_melee_damage() {
    let target = ai::Watcher { id: 20, x: 0.5, y: 1.0, z: 0.9, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, gold_armor: false };
    let players = [target];
    let context = ai::Ctx { players: &players, ..ctx() };
    for (kind, damage) in [(Kind::CaveSpider, 2.0), (Kind::Silverfish, 1.0)] {
        let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
        mob.on_ground = true;
        mob.h.target = Some(target.id);
        mob.h.provoked = true;
        let plan = hostile::think(&mut mob, &Flat::new(), &context, &mut Rng::new(18));
        assert_eq!(plan.melee.unwrap().damage, damage);
        assert_eq!(mob.h.cooldown, hostile::MELEE_COOLDOWN);
        assert_eq!(crate::survival::scaled(damage, 1), damage);
        assert_eq!(crate::survival::scaled(damage, 3), damage * 1.5);
    }
}

#[test]
fn breeding_and_curing_send_progress_only_to_the_responsible_player() {
    for kind in [Kind::Cow, Kind::Pig, Kind::Sheep, Kind::Chicken, Kind::Wolf, Kind::Cat] {
        let view = Flat::new();
        let mut items = Items::new();
        for (id, x) in [(1, 0.5), (2, 1.5)] {
            let mut mob = Mob::new(id, kind, (x, 1.0, 0.5));
            mob.love = LOVE_TICKS;
            mob.love_player = Some([id as u8; 16]);
            if matches!(kind, Kind::Wolf | Kind::Cat) { mob.tame.owner = Some([1; 16]); }
            items.add_mob(mob);
        }
        let mut rng = Rng::new(11);
        let mut next = 100;
        let mut events = Vec::new();
        for _ in 0..100 {
            let out = step_all(&mut items, &view, &[], Env { difficulty: 2, sky: 15, sun_up: false, clock: 0 }, &mut rng, &mut || { next += 1; next });
            events.extend(out.progress);
        }
        assert_eq!(events.len(), 1, "{kind:?}: {events:?}");
        assert!(events[0].0 == [1; 16] || events[0].0 == [2; 16]);
        assert!(matches!(events[0].1, crate::progress::Event::Bred(name) if name == kind.name()));
        assert!(items.mobs().iter().all(|m| m.love_player.is_none()));
    }
    let mut items = Items::new();
    let mut zombie = Mob::new(1, Kind::ZombieVillager, (0.5, 1.0, 0.5));
    zombie.no_ai = true;
    zombie.villager.conversion = 1;
    zombie.villager.converter = Some([3; 16]);
    items.add_mob(zombie);
    let out = step_all(&mut items, &Flat::new(), &[], Env { difficulty: 2, sky: 15, sun_up: false, clock: 0 }, &mut Rng::new(11), &mut || 100);
    assert_eq!(out.progress.len(), 1);
    assert_eq!(out.progress[0].0, [3; 16]);
    assert!(matches!(out.progress[0].1, crate::progress::Event::Trigger("story/cure_zombie_villager")));
}
