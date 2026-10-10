// Медь: воск, топор и случайное окисление. Правила — minecraft.wiki,
// Oxidation / Block of Copper: «taxicab distance of 4», 64/1125 за такт.
// [проверить]: Java 26.1.2, порядок событий, парные двери и новые формы.
use crate::{blocks, world::World};

type Pos = (i32, i32, i32);
const STAGES: [&str; 4] = ["", "exposed_", "weathered_", "oxidized_"];

fn parts(name: &str) -> Option<(&str, usize, bool)> {
    let waxed = name.starts_with("waxed_");
    let bare = name.strip_prefix("waxed_").unwrap_or(name);
    let (base, age) = STAGES.iter().enumerate().skip(1)
        .find_map(|(age, prefix)| bare.strip_prefix(prefix).map(|base| (base, age)))
        .unwrap_or((bare, 0));
    let base = if base == "copper_block" { "copper" } else { base };
    // Проверка семейства по существующей таблице исключает руду, факелы
    // и сырой металл: у них нет окисленного варианта.
    blocks::state_by_name(&format!("oxidized_{base}"))?;
    Some((base, age, waxed))
}

fn name_at(base: &str, age: usize, waxed: bool) -> String {
    let base = if base == "copper" && age == 0 { "copper_block" } else { base };
    format!("{}{}{base}", if waxed { "waxed_" } else { "" }, STAGES[age])
}

/// Перенос всех общих свойств; свойства не сбрасываются к default.
pub fn corresponding(state: i32, target: &str) -> Option<i32> {
    let mut next = blocks::state_by_name(target)?;
    if let (Some(old), Some(new)) = (blocks::orientation_of(state), blocks::orientation(target)) {
        for prop in old.properties {
            if let Some(value) = old.value(state, prop.name) {
                next = new.with(next, prop.name, value).unwrap_or(next);
            }
        }
    }
    Some(next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use { Wax, Unwax, Scrape }

pub fn interaction(state: i32, item: &str) -> Option<(i32, Use)> {
    let (base, age, waxed) = parts(blocks::block_at_state(state)?)?;
    let (age, waxed, action) = if item == "honeycomb" && !waxed {
        (age, true, Use::Wax)
    } else if item.ends_with("_axe") && waxed {
        (age, false, Use::Unwax)
    } else if item.ends_with("_axe") && age > 0 {
        (age - 1, false, Use::Scrape)
    } else { return None; };
    Some((corresponding(state, &name_at(base, age, waxed))?, action))
}

pub fn replace(world: &mut World, p: Pos, next: i32) {
    let old = world.get_block(p.0, p.1, p.2);
    if let Some(rule) = blocks::orientation_of(old)
        && rule.name.ends_with("copper_door")
        && let Some(half) = rule.value(old, "half")
    {
        let dy = if half == "lower" { 1 } else { -1 };
        let other = world.get_block(p.0, p.1 + dy, p.2);
        if blocks::block_at_state(other) == Some(rule.name)
            && rule.value(other, "half") != Some(half)
            && let Some(target) = corresponding(other, blocks::block_at_state(next).unwrap_or("air"))
        {
            world.set_block(p.0, p.1 + dy, p.2, target);
        }
    }
    world.set_block(p.0, p.1, p.2, next);
}

pub fn ticks_randomly(state: i32) -> bool {
    blocks::block_at_state(state).and_then(parts).is_some_and(|(_, age, waxed)| age < 3 && !waxed)
}

fn oxidation_chance(world: &World, p: Pos, age: usize) -> f32 {
    let (mut same, mut older) = (0, 0);
    for dx in -4i32..=4 {
        for dy in -4i32..=4 {
            for dz in -4i32..=4 {
                let distance = dx.abs() + dy.abs() + dz.abs();
                if distance == 0 || distance > 4 { continue; }
                let n = blocks::block_at_state(world.get_block(p.0 + dx, p.1 + dy, p.2 + dz));
                let Some((_, other, false)) = n.and_then(parts) else { continue; };
                if other < age { return 0.0; }
                if other == age { same += 1; } else { older += 1; }
            }
        }
    }
    let ratio = (older + 1) as f32 / (older + same + 1) as f32;
    ratio * ratio * if age == 0 { 0.75 } else { 1.0 }
}

pub fn random_tick(world: &mut World, p: Pos, state: i32) {
    let Some((base, age, false)) = blocks::block_at_state(state).and_then(parts) else { return; };
    if age >= 3 || world.random_unit() >= 64.0 / 1125.0 { return; }
    let chance = oxidation_chance(world, p, age);
    if world.random_unit() < chance
        && let Some(next) = corresponding(state, &name_at(base, age + 1, false))
    {
        replace(world, p, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(s: &str) -> i32 { blocks::state_from_text(s).unwrap() }

    #[test]
    fn copper_chest_contents_survive_waxing_and_scraping() {
        let mut w = World::in_memory();
        w.set_block_silently(0, 10, 0, state("oxidized_copper_chest[facing=west]"));
        let stack = crate::inventory::Stack::new(blocks::item_named("diamond").unwrap(), 17);
        w.with_container(0, 10, 0, |c| c.slots[4] = Some(stack));
        for item in ["honeycomb", "iron_axe", "iron_axe", "iron_axe", "iron_axe"] {
            let next = interaction(w.get_block(0, 10, 0), item).unwrap().0;
            replace(&mut w, (0, 10, 0), next);
            assert_eq!(w.container(0, 10, 0).unwrap().slots[4], Some(stack));
        }
        assert_eq!(w.get_block(0, 10, 0), state("copper_chest[facing=west]"));
    }

    #[test]
    fn wax_and_axe_preserve_every_property_of_every_copper_state() {
        for old in 0..=u16::MAX as i32 {
            if let Some((wax, Use::Wax)) = interaction(old, "honeycomb") {
                assert!(!ticks_randomly(wax));
                assert_eq!(interaction(wax, "iron_axe"), Some((old, Use::Unwax)));
                if let Some(rule) = blocks::orientation_of(old) {
                    let next = blocks::orientation_of(wax).unwrap();
                    for prop in rule.properties {
                        assert_eq!(rule.value(old, prop.name), next.value(wax, prop.name));
                    }
                }
            }
        }
        for n in ["copper_ore", "raw_copper_block", "copper_torch", "stone"] {
            assert_eq!(interaction(state(n), "honeycomb"), None);
        }
    }

    #[test]
    fn axe_removes_wax_before_one_oxidation_stage() {
        let mut here = state("waxed_oxidized_copper_bulb[lit=true,powered=true]");
        for n in ["oxidized_copper_bulb", "weathered_copper_bulb", "exposed_copper_bulb", "copper_bulb"] {
            here = interaction(here, "diamond_axe").unwrap().0;
            assert_eq!(blocks::block_at_state(here), Some(n));
            assert_eq!(blocks::orientation_of(here).unwrap().value(here, "lit"), Some("true"));
        }
        assert_eq!(interaction(here, "diamond_axe"), None);
    }

    #[test]
    fn younger_neighbours_stop_oxidation_but_wax_and_distance_five_do_not() {
        let mut w = World::in_memory();
        assert_eq!(oxidation_chance(&w, (0, 10, 0), 0), 0.75);
        w.set_block_silently(4, 10, 0, state("copper_block"));
        assert_eq!(oxidation_chance(&w, (0, 10, 0), 1), 0.0);
        w.set_block_silently(4, 10, 0, state("waxed_copper_block"));
        w.set_block_silently(5, 10, 0, state("copper_block"));
        assert_eq!(oxidation_chance(&w, (0, 10, 0), 1), 1.0);
        w.set_block_silently(1, 10, 0, state("exposed_cut_copper"));
        assert_eq!(oxidation_chance(&w, (0, 10, 0), 1), 0.25);
        w.set_block_silently(2, 10, 0, state("oxidized_copper"));
        assert!((oxidation_chance(&w, (0, 10, 0), 1) - 4.0 / 9.0).abs() < 0.0001);
    }

    #[test]
    fn changing_a_door_changes_both_halves() {
        let mut w = World::in_memory();
        w.set_block_silently(0, 10, 0, state("weathered_copper_door[half=lower,open=true]"));
        w.set_block_silently(0, 11, 0, state("weathered_copper_door[half=upper,open=true]"));
        let next = interaction(w.get_block(0, 11, 0), "honeycomb").unwrap().0;
        replace(&mut w, (0, 11, 0), next);
        assert_eq!(w.get_block(0, 10, 0), state("waxed_weathered_copper_door[half=lower,open=true]"));
    }
}
