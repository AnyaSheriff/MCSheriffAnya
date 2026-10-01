use super::*;
use crate::items::Items;

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
    let player = ai::Watcher { x: 8.5, y: 1.0, z: 0.5, eye: 1.62, holding: [Some(wheat), None], spectator: false, id: 1, targetable: true };
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
    let players = [ai::Watcher { x: 5.5, y: 1.0, z: 0.5, eye: 1.62, holding: [Some("carrot"), None], spectator: true, id: 1, targetable: false }];
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
        step_all(&mut items, &view, &[], Env { difficulty: 2, sky: 15, sun_up: false }, &mut rng, &mut || {
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
        let outcome = step_all(&mut items, &Meadow, &[], Env { difficulty: 2, sky: 15, sun_up: false }, &mut rng, &mut || {
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
    ai::Watcher { id, x, y: 1.0, z, eye: 1.62, holding: [None, None], spectator: false, targetable: true }
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
    assert!(Kind::ALL.iter().filter(|kind| kind.hostile()).count() == 4);
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
        let found = spawn::hostile_attempt(&view, &players, &|_, _| 40, &mut rng, &mut || {
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
            assert!(found.len() <= 4 && found.iter().all(|mob| mob.kind == found[0].kind));
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
