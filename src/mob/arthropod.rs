// Пещерный паук и чешуйница: правила из minecraft.wiki/w/Cave_Spider,
// /Silverfish и /Infested_Block, без кода игры. Таймеры и случайный обход,
// которых нет в тексте вики, помечены [проверить].

use super::{Kind, Mob, Rng};
use crate::{blocks, inventory::Stack, items::Items, survival::{self, Effect, EffectKind, Source}, world::World};

type Cell = (i32, i32, i32);

/// Зов через 20 тактов после удара: у оригинала блоки ломаются на +19 или +20 такт
/// от события урона, смотря на фазу такта (чёрный ящик, silverfish-call, две записи);
/// повторный удар не отодвигает зов [проверить].
pub const CALL_DELAY: i32 = 20;

/// Вики, Cave Spider: «7 seconds … 15 seconds», на лёгкой яда нет.
pub fn poison(difficulty: u8) -> Option<Effect> {
    let ticks_left = match difficulty {
        survival::NORMAL => 140,
        survival::HARD => 300,
        _ => return None,
    };

    Some(Effect { kind: EffectKind::Poison, amplifier: 0, ticks_left, ambient: false, visible: true, show_icon: true })
}

/// Вики, Silverfish: урон существа и «Poison damage», но не падение/огонь.
/// Magic включает вредящее зелье и яд (общая модель survival.rs).
pub fn calls_for_help(source: Source) -> bool {
    matches!(source, Source::PlayerAttack | Source::MaceSmash | Source::MobAttack | Source::Sting | Source::Arrow | Source::Trident | Source::Magic | Source::Thorns | Source::PlayerExplosion)
}

const HOSTS: [(&str, &str); 7] = [
    ("stone", "infested_stone"),
    ("cobblestone", "infested_cobblestone"),
    ("stone_bricks", "infested_stone_bricks"),
    ("mossy_stone_bricks", "infested_mossy_stone_bricks"),
    ("cracked_stone_bricks", "infested_cracked_stone_bricks"),
    ("chiseled_stone_bricks", "infested_chiseled_stone_bricks"),
    ("deepslate", "infested_deepslate"),
];

/// При превращении глубинного сланца сохраняется axis (вики, Infested Block).
fn convert(state: i32, infest: bool) -> Option<i32> {
    let name = blocks::block_at_state(state)?;
    let &(host, infested) = HOSTS.iter().find(|&&(host, infested)| name == if infest { host } else { infested })?;
    let destination = if infest { infested } else { host };
    let mut out = blocks::state_by_name(destination)?;

    if let Some(axis) = blocks::orientation_of(state).and_then(|block| block.value(state, "axis")) {
        out = blocks::orientation(destination)?.with(out, "axis", axis)?;
    }

    Some(out)
}

pub fn infest(state: i32) -> Option<i32> {
    convert(state, true)
}

pub fn host(state: i32) -> Option<i32> {
    convert(state, false)
}

/// Вики, Infested Block: Silk Touch даёт обычный блок даже без кирки.
pub fn silk_drop(state: i32) -> Option<Stack> {
    let ordinary = host(state)?;

    Some(Stack::new(blocks::item_for_block(blocks::block_at_state(ordinary)?)?, 1))
}

/// doTileDrops запрещает выход; doMobSpawning не влияет. В мирной сложности чешуйница
/// выходит и тут же исчезает (чёрный ящик, infested-mining-time: появление и снос в соседних тактах).
pub fn releases(state: i32, survival: bool, silk: bool, drops: bool) -> bool {
    survival && !silk && drops && host(state).is_some()
}

/// Вышедшая чешуйница в центре бывшего блока, с частицами появления.
pub fn released(id: i32, at: Cell) -> Mob {
    let mut mob = Mob::new(id, Kind::Silverfish, (f64::from(at.0) + 0.5, f64::from(at.1), f64::from(at.2) + 0.5));

    mob.persistent = false;
    mob.spawn_poof = true;
    mob
}

/// [проверить] порядок: от центра попеременно в обе стороны каждой оси.
fn offsets(radius: i32) -> impl Iterator<Item = i32> {
    std::iter::once(0).chain((1..=radius).flat_map(|i| [i, -i]))
}

/// Вики, Silverfish: область зова «21×11×21» вокруг клетки моба.
fn call_cells(at: Cell) -> impl Iterator<Item = Cell> {
    offsets(5).flat_map(move |dy| offsets(10).flat_map(move |dx| offsets(10).map(move |dz| (at.0 + dx, at.1 + dy, at.2 + dz))))
}

#[derive(Clone, Copy)]
pub struct Rules {
    pub griefing: bool,
    pub drops: bool,
}

/// Изменение мира отделено от движения: замки мира/предметов уже у вызывающего.
pub fn step(items: &mut Items, world: &mut World, rules: Rules, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) {
    let mut calls = Vec::new();
    let mut hides = Vec::new();

    for mob in items.mobs_mut().iter_mut().filter(|mob| mob.kind == Kind::Silverfish && !mob.is_dead() && !mob.no_ai && !mob.h.despawn) {
        let at = mob.cell();

        if !world.has_chunk(at.0.div_euclid(16), at.2.div_euclid(16)) {
            continue;
        }

        if mob.h.silverfish_call > 0 {
            mob.h.silverfish_call -= 1;

            if mob.h.silverfish_call == 0 {
                calls.push((at, mob.h.target));
            }
        } else if rules.griefing && mob.h.target.is_none() && !mob.brain.walking() && rng.below(10) == 0 {
            // Вики: «When idle … enter a nearby … block»; шанс и выбор грани [проверить].
            let (dx, dy, dz) = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)][rng.below(6) as usize];
            let cell = (at.0 + dx, (mob.y + 0.5).floor() as i32 + dy, at.2 + dz);

            hides.push((mob.entity_id, cell));
        }
    }

    for (at, target) in calls {
        for (x, y, z) in call_cells(at) {
            if !world.has_chunk(x.div_euclid(16), z.div_euclid(16)) {
                continue;
            }

            let state = world.get_block(x, y, z);
            let Some(ordinary) = host(state) else { continue };

            // [проверить] mobGriefing=false освобождает блок без разрушения и моба.
            if rules.griefing {
                world.set_block(x, y, z, blocks::state_by_name("air").expect("воздух"));
                world.note_level_event(x, y, z, 2001, state);

                if releases(state, true, false, rules.drops) {
                    let mut friend = released(next_id(), (x, y, z));

                    friend.h.target = target;
                    friend.h.provoked = target.is_some();
                    items.add_mob_late(friend);
                }
            } else {
                world.set_block(x, y, z, ordinary);
            }

            // [проверить] после каждого найденного блока 1/2 завершить обход.
            if rng.below(2) == 0 {
                break;
            }
        }
    }

    for (id, (x, y, z)) in hides {
        if !world.has_chunk(x.div_euclid(16), z.div_euclid(16)) {
            continue;
        }

        if let Some(state) = infest(world.get_block(x, y, z)) {
            world.set_block(x, y, z, state);
            // Вики: моб уходит в блок, смерти, добычи и опыта нет.
            items.remove_mobs(|mob| mob.entity_id == id);
        }
    }
}

/// Спавнер чешуйницы: свет ≤11 либо камень-хозяин снизу; игроки не ближе 5.
/// Вики, Silverfish: «cannot spawn within a 5 block distance of any player».
pub fn silverfish_spawn(light: u8, below: i32, nearest_player_sq: f64) -> bool {
    nearest_player_sq > 25.0 && (light <= 11 || infest(below).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str) -> i32 {
        blocks::state_by_name(name).unwrap()
    }

    #[test]
    fn poison_depends_on_difficulty() {
        assert_eq!(poison(0), None);
        assert_eq!(poison(1), None);
        assert_eq!(poison(2).unwrap().ticks_left, 140);
        assert_eq!(poison(3).unwrap().ticks_left, 300);
        assert_eq!(poison(3).unwrap().amplifier, 0);
    }

    #[test]
    fn seven_hosts_roundtrip_including_every_deepslate_axis() {
        for (ordinary, infested) in HOSTS {
            assert_eq!(infest(state(ordinary)), Some(state(infested)));
            assert_eq!(host(state(infested)), Some(state(ordinary)));
            assert_eq!(silk_drop(state(infested)).unwrap().item, blocks::item_named(ordinary).unwrap());
        }
        for axis in ["x", "y", "z"] {
            let block = blocks::orientation("deepslate").unwrap();
            let ordinary = block.with(state("deepslate"), "axis", axis).unwrap();
            let infested = infest(ordinary).unwrap();

            assert_eq!(host(infested), Some(ordinary));
            assert_eq!(blocks::orientation_of(infested).unwrap().value(infested, "axis"), Some(axis));
        }
        for name in ["mossy_cobblestone", "smooth_stone", "andesite", "diorite", "granite", "stone_slab", "stone_brick_stairs"] {
            assert_eq!(infest(state(name)), None, "{name}");
        }
    }

    #[test]
    fn release_requires_survival_no_silk_and_drops() {
        let block = state("infested_stone");
        assert!(releases(block, true, false, true));
        assert!(!releases(block, false, false, true));
        assert!(!releases(block, true, true, true));
        assert!(!releases(block, true, false, false));
        assert!(!releases(state("stone"), true, false, true));
    }

    #[test]
    fn call_box_has_exact_extents_and_no_duplicates() {
        let cells: std::collections::HashSet<_> = call_cells((20, 60, -30)).collect();
        assert_eq!(cells.len(), 21 * 11 * 21);
        assert!(cells.contains(&(10, 55, -40)));
        assert!(cells.contains(&(30, 65, -20)));
        assert!(!cells.contains(&(31, 60, -30)));
        assert!(!cells.contains(&(20, 66, -30)));
    }

    #[test]
    fn spawner_light_host_and_player_exclusion() {
        assert!(silverfish_spawn(11, state("glass"), 26.0));
        assert!(!silverfish_spawn(12, state("glass"), 26.0));
        assert!(silverfish_spawn(15, state("stone"), 26.0));
        assert!(!silverfish_spawn(0, state("stone"), 25.0));
    }

    fn test_world() -> World {
        let mut world = World::in_memory();
        world.set_block_silently(0, 63, 0, state("glass"));
        world
    }

    #[test]
    fn call_waits_the_delay_and_releases_once() {
        let mut world = test_world();
        world.set_block_silently(1, 64, 0, state("infested_stone"));
        let mut mob = Mob::new(1, Kind::Silverfish, (0.5, 64.0, 0.5));
        mob.h.target = Some(42);
        mob.hurt(1.0, Source::PlayerAttack, None).unwrap();
        let mut items = Items::new();
        items.add_mob(mob);
        let rules = Rules { griefing: true, drops: true };
        let mut rng = Rng::new(7);
        for _ in 0..CALL_DELAY - 1 {
            step(&mut items, &mut world, rules, &mut rng, &mut || 2);
        }
        assert_eq!(world.get_block(1, 64, 0), state("infested_stone"));
        assert_eq!(items.mobs().len(), 1);
        step(&mut items, &mut world, rules, &mut rng, &mut || 2);
        assert_eq!(world.get_block(1, 64, 0), state("air"));
        assert_eq!(items.mobs().len(), 2);
        assert_eq!(items.mobs()[1].h.target, Some(42));
        assert_eq!(items.mobs()[1].size(), (0.4, 0.3));
        step(&mut items, &mut world, rules, &mut rng, &mut || 3);
        assert_eq!(items.mobs().len(), 2);
    }

    #[test]
    fn call_honors_griefing_drops_no_ai_and_death() {
        for (griefing, drops, no_ai, dead, expected) in [
            (false, true, false, false, "stone"),
            (true, false, false, false, "air"),
            (true, true, true, false, "infested_stone"),
            (true, true, false, true, "infested_stone"),
        ] {
            let mut world = test_world();
            world.set_block_silently(1, 64, 0, state("infested_stone"));
            let mut mob = Mob::new(1, Kind::Silverfish, (0.5, 64.0, 0.5));
            mob.h.silverfish_call = 1;
            mob.no_ai = no_ai;
            mob.vitals.dead = dead;
            let mut items = Items::new();
            items.add_mob(mob);
            step(&mut items, &mut world, Rules { griefing, drops }, &mut Rng::new(1), &mut || 2);
            assert_eq!(world.get_block(1, 64, 0), state(expected));
            assert_eq!(items.mobs().len(), 1);
        }
    }

    #[test]
    fn idle_fish_hides_without_loot_but_target_or_rule_prevents_it() {
        for (griefing, target, hides) in [(true, None, true), (false, None, false), (true, Some(42), false)] {
            let mut world = test_world();
            world.set_block_silently(0, 63, 0, state("deepslate"));
            let mut mob = Mob::new(1, Kind::Silverfish, (0.5, 64.0, 0.5));
            mob.h.target = target;
            let mut items = Items::new();
            items.add_mob(mob);
            let mut rng = Rng::new(93);
            for _ in 0..1000 {
                step(&mut items, &mut world, Rules { griefing, drops: true }, &mut rng, &mut || 2);
            }
            assert_eq!(items.mobs().is_empty(), hides);
            assert_eq!(world.get_block(0, 63, 0), state(if hides { "infested_deepslate" } else { "deepslate" }));
            assert!(items.orbs().is_empty());
        }
    }

}
