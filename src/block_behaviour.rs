// Обновления природных блоков: губка, кораллы, пузырьки, почки аметиста.
// Правила — minecraft.wiki (Sponge/Schwamm, Coral, Coral Fan,
// Bubble Column, Budding Amethyst). [проверить]: порядок пакетов и границы
// случайных задержек на Java 26.1.2; black box для этой работы не запускался.

use std::collections::{HashSet, VecDeque};
use crate::{blocks, fluids};
use crate::world::{World, AIR, MIN_Y, WORLD_HEIGHT};

type Pos = (i32, i32, i32);
const SIDES: [(Pos, &str); 6] = [
    ((0, -1, 0), "down"), ((0, 1, 0), "up"),
    ((0, 0, -1), "north"), ((0, 0, 1), "south"),
    ((-1, 0, 0), "west"), ((1, 0, 0), "east"),
];

fn at(world: &World, p: Pos) -> i32 { world.get_block(p.0, p.1, p.2) }
fn name(state: i32) -> &'static str { blocks::block_at_state(state).unwrap_or("air") }
fn state(name: &str) -> i32 { blocks::state_by_name(name).expect("блок есть в таблице") }
fn add(a: Pos, b: Pos) -> Pos { (a.0 + b.0, a.1 + b.1, a.2 + b.2) }
fn property(state: i32, key: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, key)
}

/// Жидкая вода, включая воду в растении или обводнённом блоке.
/// Не используем признак прозрачности: лёд коралл не поит.
fn contains_water(state: i32) -> bool {
    matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, _)))
        || property(state, "waterlogged") == Some("true")
        || matches!(name(state), "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass")
}

fn live_coral(name: &str) -> bool {
    ["tube_coral", "brain_coral", "bubble_coral", "fire_coral", "horn_coral"]
        .iter().any(|base| name.strip_prefix(base).is_some_and(|tail| matches!(tail, "" | "_block" | "_fan" | "_wall_fan")))
}

fn small_coral(name: &str) -> bool {
    let bare = name.strip_prefix("dead_").unwrap_or(name);
    live_coral(bare) && !bare.ends_with("_block")
}

/// Вики, Coral/Fan: вода берётся из клетки установки, а не из состояния
/// по умолчанию (в таблице кораллы обводнены). Веер смотрит от стены.
pub fn placed(world: &mut World, p: Pos, here: i32, face: i32) -> i32 {
    let here = if let Some(rule) = blocks::orientation_of(here) {
        let wet = contains_water(at(world, p)) && rule.value(here, "type") != Some("double");
        let next = rule.with(here, "waterlogged", if wet { "true" } else { "false" }).unwrap_or(here);
        if wet && matches!(rule.name, "campfire" | "soul_campfire") { rule.with(next, "lit", "false").unwrap_or(next) } else { next }
    } else { here };
    let n = name(here);
    if n == "pointed_dripstone" { return crate::dripstone::placed(world, p, here, face); }
    if n == "scaffolding" { return crate::scaffolding::placed(world, p, here); }
    // Верхушка лозы или ламинарии, поставленная игроком, получает случайный возраст 0…24
    // (чёрный ящик, cave-vines-cycle: age=6 при установке ягод).
    if matches!(n, "cave_vines" | "weeping_vines" | "twisting_vines" | "kelp")
        && let Some(next) = blocks::orientation_of(here).and_then(|b| b.with(here, "age", &(world.random_bits() % 25).to_string())) {
        return next;
    }
    let bud = n.ends_with("_amethyst_bud") || n == "amethyst_cluster";
    let candle = n == "candle" || n.ends_with("_candle");
    if !small_coral(n) && !bud && !candle { return here; }
    let Some(rule) = blocks::orientation_of(here) else { return here; };
    let water = if column_water(at(world, p)) { "true" } else { "false" };
    let mut target = rule.with(here, "waterlogged", water).unwrap_or(here);
    if (n.ends_with("_wall_fan") || bud) && let Some((_, facing)) = SIDES.get(face as usize) {
        target = rule.with(target, "facing", facing).unwrap_or(target);
    }
    target
}

/// Ведро меняет воду внутри блока, сохраняя его остальные свойства.
/// Вики, Waterlogging: двойная плита не обводняется; огонь тушится.
pub fn bucket_water(state: i32, filled: bool) -> Option<i32> {
    let rule = blocks::orientation_of(state)?;
    if rule.value(state, "waterlogged")? != if filled { "false" } else { "true" }
        || rule.value(state, "type") == Some("double") { return None; }
    let next = rule.with(state, "waterlogged", if filled { "true" } else { "false" })?;
    Some(if filled && matches!(rule.name, "campfire" | "soul_campfire" | "candle") || filled && rule.name.ends_with("_candle") {
        rule.with(next, "lit", "false").unwrap_or(next)
    } else { next })
}

/// Общая проверка опоры коралла при установке и обновлении формы.
/// [проверить]: неполные боковые грани; используем существующий solid_top.
pub fn supported(world: &World, p: Pos, here: i32) -> bool {
    let n = name(here);
    if let Some(supported) = crate::vegetation::supported(world, p, here) { return supported; }
    if matches!(n, "cave_vines" | "cave_vines_plant") { return crate::farming::cave_vine_supported(world, p); }
    if n == "pointed_dripstone" { return crate::dripstone::supported(world, p, here); }
    if n.ends_with("_amethyst_bud") || n == "amethyst_cluster" {
        let Some((d, _)) = SIDES.iter().find(|(_, facing)| Some(*facing) == property(here, "facing")) else { return false; };
        let support = at(world, add(p, (-d.0, -d.1, -d.2)));
        return if d.1 == 1 { blocks::solid_top(support) } else { blocks::full_cube(support) };
    }

    // Свеча без опоры не ломается: обновление формы её не проверяет (чёрный
    // ящик, candles-stacking: после setblock air под свечой она остаётся).
    if n == "cake" || n.ends_with("_candle_cake") || n == "candle_cake" {
        return blocks::solid_top(at(world, add(p, (0, -1, 0))));
    }
    if !small_coral(n) { return true; }
    if n.ends_with("_wall_fan") {
        let Some((d, _)) = SIDES.iter().find(|(_, facing)| Some(*facing) == property(here, "facing")) else { return false; };
        blocks::full_cube(at(world, add(p, (-d.0, -d.1, -d.2))))
    } else {
        blocks::solid_top(at(world, add(p, (0, -1, 0))))
    }
}

/// Обновление формы доходит и при «тихом» изменении соседа (например,
/// обводнённого люка). При потере опоры сохраняем воду в этой клетке.
pub fn shape_changed(world: &mut World, p: Pos, loud: bool) -> bool {
    let here = at(world, p);
    if crate::vegetation::vine_changed(world, p) { return true; }
    if name(here) == "pointed_dripstone" { crate::dripstone::changed(world, p); return true; }
    if matches!(name(here), "cave_vines" | "cave_vines_plant") { crate::farming::cave_vine_changed(world, p); return true; }
    if !supported(world, p, here) {
        world.note_destroyed(p.0, p.1, p.2);
        if loud { world.note_level_event(p.0, p.1, p.2, 2001, here); }
        let left = if property(here, "waterlogged") == Some("true") { state("water") } else { AIR };
        world.set_block(p.0, p.1, p.2, left);
        return true;
    }
    if live_coral(name(here)) || matches!(name(here), "bubble_column" | "scaffolding" | "dirt_path") { changed(world, p); }
    false
}

fn coral_wet(world: &World, p: Pos) -> bool {
    contains_water(at(world, p)) || SIDES.iter().any(|(d, _)| contains_water(at(world, add(p, *d))))
}

/// Вызвано установкой блока или изменением соседа. Используется общей
/// очередью обновлений, поэтому работает и для команд, и для игрока.
pub fn changed(world: &mut World, p: Pos) {
    if crate::vegetation::vine_changed(world, p) { return; }
    let here = at(world, p);
    match name(here) {
        n if (n == "candle" || n.ends_with("_candle"))
            && property(here, "waterlogged") == Some("true") && property(here, "lit") == Some("true") => {
            if let Some(next) = blocks::orientation_of(here).and_then(|b| b.with(here, "lit", "false")) {
                world.set_block(p.0, p.1, p.2, next);
                world.note_sound(p.0, p.1, p.2, "minecraft:block.candle.extinguish", 1.0, 1.0);
            }
        }
        "cave_vines" | "cave_vines_plant" => crate::farming::cave_vine_changed(world, p),
        "pointed_dripstone" => crate::dripstone::changed(world, p),
        "scaffolding" => crate::scaffolding::changed(world, p),
        "composter" => crate::compost::changed(world, p),
        "dirt_path" if blocks::full_cube(at(world, add(p, (0, 1, 0)))) => {
            world.schedule_block_once(p.0, p.1, p.2, 1);
        }
        "sponge" => { absorb(world, p); }
        "wet_sponge" if world.is_nether() => {
            world.set_block(p.0, p.1, p.2, state("sponge"));
            // Вики, Sponge: «dries out instantly» в Незере.
            // [проверить]: эффект испарения и высота звука.
            world.note_level_event(p.0, p.1, p.2, 2009, 0);
            world.note_sound(p.0, p.1, p.2, "minecraft:block.wet_sponge.dries", 1.0, 1.0);
        }
        "soul_sand" | "magma_block" => {
            if column_water(at(world, add(p, (0, 1, 0)))) {
                world.schedule_block_once(p.0, p.1, p.2, 20);
            }
        }
        "water" | "bubble_column" => {
            let below = at(world, add(p, (0, -1, 0)));
            if name(here) == "bubble_column" || name(below) == "bubble_column" {
                if column_water(here) && bubble_state(below) != Some(here) {
                    world.schedule_block_once(p.0, p.1, p.2, 5);
                }
            } else if column_water(here) && matches!(name(below), "soul_sand" | "magma_block") {
                world.schedule_block_once(p.0, p.1 - 1, p.2, 20);
            }
        }
        n if live_coral(n) && !coral_wet(world, p) => {
            // Вики, Coral: «3 to 4.95 seconds», даже при randomTickSpeed=0.
            let delay = 60 + u64::from(world.random_bits() % 40);
            world.schedule_block_once(p.0, p.1, p.2, delay);
        }
        _ => {}
    }
}

/// Губка идёт по связанной воде от ближних клеток к дальним; через воздух
/// и твёрдые блоки не перескакивает. Вики, Water: «taxicab distance of 7».
/// Лимит 65 по доступной странице Schwamm; [проверить] для 26.1.2.
/// Обводнённые твёрдые блоки оставляем как есть (Water: «do not absorb ...
/// waterlogged blocks»); подводные растения снимаем с обычным дропом.
fn absorb(world: &mut World, p: Pos) -> usize {
    let mut queue = VecDeque::from([(p, 0)]);
    let mut seen = HashSet::from([p]);
    let mut removed = 0;
    while let Some((from, distance)) = queue.pop_front() {
        if distance >= 7 { continue; }
        for (d, _) in SIDES {
            let next = add(from, d);
            if !seen.insert(next) { continue; }
            let here = at(world, next);
            let plant = matches!(name(here), "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass");
            if !plant && !matches!(fluids::fluid_at(here), Some((fluids::Kind::Water, _))) { continue; }
            if plant { world.note_destroyed(next.0, next.1, next.2); }
            if world.set_block(next.0, next.1, next.2, AIR) {
                removed += 1;
                queue.push_back((next, distance + 1));
                // У оригинала удаление воды у самой губки сразу оповещает её
                // соседа-губку, и та впитывает снова, вложенным вызовом, пока
                // ещё не намокла; каждый удавшийся вызов звучит (чёрный ящик,
                // sponge-connected-water: два звука при двух соседях-водах).
                if distance == 0 && name(at(world, p)) == "sponge" {
                    absorb(world, p);
                }
            }
            if removed == 65 { break; }
        }
        if removed == 65 { break; }
    }
    if removed > 0 {
        world.set_block(p.0, p.1, p.2, state("wet_sponge"));
        world.note_sound(p.0, p.1, p.2, "minecraft:block.sponge.absorb", 1.0, 1.0);
    }
    removed
}

fn column_water(state: i32) -> bool {
    matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, 0)))
}

fn bubble_state(below: i32) -> Option<i32> {
    let drag = match name(below) {
        "soul_sand" => "false",
        "magma_block" => "true",
        "bubble_column" => return Some(below),
        _ => return None,
    };
    blocks::orientation("bubble_column")?.state(&[("drag", drag)])
}

/// Вики, Bubble Column: «all ... simultaneously», источники только;
/// создание через 20 тактов основания, распад через 5 тактов колонны.
/// Вверх не грузим новые чанки и не проходим через течение/обводнение.
fn update_column(world: &mut World, mut p: Pos) {
    let mut below = at(world, add(p, (0, -1, 0)));
    while p.1 < MIN_Y + WORLD_HEIGHT && column_water(at(world, p)) {
        let wanted = bubble_state(below).unwrap_or_else(|| fluids::state_of(fluids::Kind::Water, 0));
        world.set_block(p.0, p.1, p.2, wanted);
        below = wanted;
        p.1 += 1;
    }
}

/// Возвращает true, если это такт одного из наших блоков.
pub fn ticked(world: &mut World, p: Pos) -> bool {
    if crate::vegetation::ticked(world, p) { return true; }
    let here = at(world, p);
    match name(here) {
        "pointed_dripstone" => crate::dripstone::ticked(world, p),
        "frosted_ice" => crate::frost::ticked(world, p),
        "cauldron" | "water_cauldron" => crate::dripstone::fill_cauldron(world, p),
        "scaffolding" => crate::scaffolding::ticked(world, p),
        "composter" => crate::compost::ticked(world, p),
        "dirt_path" => {
            if blocks::full_cube(at(world, add(p, (0, 1, 0)))) { crate::farming::to_dirt(world, p); }
        }
        "soul_sand" | "magma_block" => update_column(world, add(p, (0, 1, 0))),
        "water" | "bubble_column" => update_column(world, p),
        n if live_coral(n) => {
            // Вернулась вода — ожидающий такт не убивает коралл.
            if !coral_wet(world, p) {
                let dead = format!("dead_{n}");
                let target = if let Some(rule) = blocks::orientation(&dead) {
                    let mut values = vec![("waterlogged", "false")];
                    if let Some(facing) = property(here, "facing") { values.push(("facing", facing)); }
                    rule.state(&values)
                } else { blocks::state_by_name(&dead) };
                if let Some(target) = target { world.set_block(p.0, p.1, p.2, target); }
            }
        }
        _ => return false,
    }
    true
}

/// Вики, Budding Amethyst: «20% chance», одна случайная грань из шести.
/// Только сам budding-блок тикает, а почка должна смотреть от него.
pub fn amethyst_tick(world: &mut World, p: Pos) {
    if name(at(world, p)) != "budding_amethyst" || !world.random_bits().is_multiple_of(5) { return; }
    let side = world.random_bits() as usize % SIDES.len();
    grow_amethyst(world, p, side);
}

fn grow_amethyst(world: &mut World, p: Pos, side: usize) {
    let (delta, facing) = SIDES[side];
    let next = add(p, delta);
    let here = at(world, next);
    let next_name = match name(here) {
        "air" | "cave_air" | "void_air" => "small_amethyst_bud",
        "water" if column_water(here) => "small_amethyst_bud",
        "small_amethyst_bud" if property(here, "facing") == Some(facing) => "medium_amethyst_bud",
        "medium_amethyst_bud" if property(here, "facing") == Some(facing) => "large_amethyst_bud",
        "large_amethyst_bud" if property(here, "facing") == Some(facing) => "amethyst_cluster",
        _ => return,
    };
    let waterlogged = if contains_water(here) { "true" } else { "false" };
    if let Some(target) = blocks::orientation(next_name).and_then(|rule| rule.state(&[("facing", facing), ("waterlogged", waterlogged)])) {
        world.set_block(next.0, next.1, next.2, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{redstone, world::TickKind};

    fn parse(s: &str) -> i32 { blocks::state_from_text(s).unwrap() }
    fn put(w: &mut World, p: Pos, s: &str) { w.set_block(p.0, p.1, p.2, parse(s)); redstone::settle(w); }
    fn run(w: &mut World, ticks: usize) {
        for _ in 0..ticks {
            for (p, kind) in w.advance(10000) {
                if kind == TickKind::Block { redstone::ticked(w, p); }
                else { crate::tick::flow(w, p.0, p.1, p.2); }
                redstone::settle(w);
            }
        }
    }

    #[test]
    fn amethyst_support_uses_opposite_face_and_leaves_water() {
        for (delta, facing) in SIDES {
            let mut w = World::in_memory();
            let centre = (0, 10, 0);
            w.set_block_silently(0, 10, 0, state("stone"));
            let p = add(centre, delta);
            let bud = parse(&format!("small_amethyst_bud[facing={facing},waterlogged=true]"));
            w.set_block_silently(p.0, p.1, p.2, bud);
            assert!(supported(&w, p, bud));
            w.set_block(0, 10, 0, AIR);
            redstone::settle(&mut w);
            assert_eq!(at(&w, p), state("water"), "{facing}");
        }
    }

    #[test]
    fn buckets_preserve_properties_and_extinguish_only_when_filling() {
        let dry = parse("red_candle[candles=3,lit=true,waterlogged=false]");
        let wet = bucket_water(dry, true).unwrap();
        assert_eq!(property(wet, "candles"), Some("3"));
        assert_eq!(property(wet, "lit"), Some("false"));
        assert_eq!(bucket_water(wet, true), None);
        let drained = bucket_water(wet, false).unwrap();
        assert_eq!(property(drained, "waterlogged"), Some("false"));
        assert_eq!(property(drained, "lit"), Some("false"));
        assert_eq!(bucket_water(parse("oak_slab[type=double]"), true), None);
        let stairs = parse("oak_stairs[facing=west,half=top,shape=inner_left]");
        assert_eq!(bucket_water(bucket_water(stairs, true).unwrap(), false), Some(stairs));
    }

    #[test]
    fn sponge_follows_water_not_air_or_lava_and_only_once() {
        let mut w = World::in_memory();
        for x in 1..=8 { w.set_block_silently(x, 10, 0, state("water")); }
        w.set_block_silently(0, 10, 2, state("water"));
        w.set_block_silently(-1, 10, 0, state("lava"));
        put(&mut w, (0, 10, 0), "sponge");
        for x in 1..=7 { assert_eq!(at(&w, (x, 10, 0)), AIR); }
        assert_eq!(at(&w, (8, 10, 0)), state("water"));
        assert_eq!(at(&w, (0, 10, 2)), state("water"));
        assert_eq!(at(&w, (-1, 10, 0)), state("lava"));
        assert_eq!(at(&w, (0, 10, 0)), state("wet_sponge"));
        put(&mut w, (1, 10, 0), "water");
        assert_eq!(at(&w, (1, 10, 0)), state("water"));
    }

    #[test]
    fn dry_sponge_reacts_to_new_water_and_ignores_waterlogged_solids() {
        let mut w = World::in_memory();
        put(&mut w, (0, 10, 0), "sponge");
        put(&mut w, (1, 10, 0), "oak_slab[waterlogged=true]");
        assert_eq!(at(&w, (0, 10, 0)), state("sponge"));
        put(&mut w, (0, 11, 0), "water[level=3]");
        assert_eq!(at(&w, (0, 11, 0)), AIR);
        assert_eq!(at(&w, (0, 10, 0)), state("wet_sponge"));
        assert_eq!(property(at(&w, (1, 10, 0)), "waterlogged"), Some("true"));
    }

    #[test]
    fn sponge_search_is_bounded_in_a_large_pool() {
        let mut w = World::in_memory();
        for x in -7..=7 { for y in 3..=17 { for z in -7..=7 {
            w.set_block_silently(x, y, z, state("water"));
        } } }
        w.set_block_silently(0, 10, 0, state("sponge"));
        absorb(&mut w, (0, 10, 0));
        // Вложенные впитывания (как у оригинала) тоже ограничены 65 за вызов.
        let removed = (-7..=7).flat_map(|x| (3..=17).flat_map(move |y| (-7..=7).map(move |z| (x, y, z))))
            .filter(|&(x, y, z)| w.get_block(x, y, z) == AIR).count();
        assert!((65..=65 * 3).contains(&removed), "{removed}");
    }

    #[test]
    fn all_live_coral_variants_die_but_rehydration_cancels_death() {
        for base in ["tube", "brain", "bubble", "fire", "horn"] {
            for suffix in ["_coral", "_coral_block", "_coral_fan", "_coral_wall_fan"] {
                let mut w = World::in_memory();
                let n = format!("{base}{suffix}");
                let dry = if suffix == "_coral_block" { state(&n) } else {
                    blocks::orientation(&n).unwrap().state(&[("waterlogged", "false")]).unwrap()
                };
                w.set_block_silently(0, 10, 0, dry);
                changed(&mut w, (0, 10, 0));
                assert!(w.scheduled_count() > 0);
                w.set_block_silently(1, 10, 0, parse("oak_slab[waterlogged=true]"));
                ticked(&mut w, (0, 10, 0));
                assert_eq!(at(&w, (0, 10, 0)), dry);
                w.set_block_silently(1, 10, 0, AIR);
                ticked(&mut w, (0, 10, 0));
                let dead = at(&w, (0, 10, 0));
                assert_eq!(name(dead), format!("dead_{n}"));
                assert_eq!(property(dead, "facing"), property(dry, "facing"));
                w.set_block_silently(1, 10, 0, state("water"));
                assert!(!ticked(&mut w, (0, 10, 0)));
                assert_eq!(at(&w, (0, 10, 0)), dead);
            }
        }
    }

    #[test]
    fn coral_scheduled_death_works_without_random_ticks() {
        let mut w = World::in_memory();
        put(&mut w, (0, 10, 0), "tube_coral_block");
        run(&mut w, 59);
        assert_eq!(name(at(&w, (0, 10, 0))), "tube_coral_block");
        run(&mut w, 40);
        assert_eq!(name(at(&w, (0, 10, 0))), "dead_tube_coral_block");
    }

    #[test]
    fn player_placed_coral_uses_actual_water_and_wall_face() {
        use crate::placing::{self, Click, Placement};
        let mut w = World::in_memory();
        put(&mut w, (0, 10, 0), "stone");
        let click = Click { face: 5, cursor_y: 0.5, yaw: 0.0, pitch: 0.0 };
        assert_eq!(placing::place(&mut w, "brain_coral_fan", state("brain_coral_fan"), (0, 10, 0), &click, (4.0, 10.0, 4.0)), Placement::Placed);
        assert_eq!(name(at(&w, (1, 10, 0))), "brain_coral_wall_fan");
        assert_eq!(property(at(&w, (1, 10, 0)), "waterlogged"), Some("false"));
        assert_eq!(property(at(&w, (1, 10, 0)), "facing"), Some("east"));
        run(&mut w, 100);
        assert_eq!(name(at(&w, (1, 10, 0))), "dead_brain_coral_wall_fan");

        w.set_block_silently(0, 11, 0, state("water"));
        let click = Click { face: 1, ..click };
        assert_eq!(placing::place(&mut w, "tube_coral", state("tube_coral"), (0, 10, 0), &click, (4.0, 10.0, 4.0)), Placement::Placed);
        assert_eq!(property(at(&w, (0, 11, 0)), "waterlogged"), Some("true"));
        put(&mut w, (0, 10, 0), "air");
        assert_eq!(at(&w, (0, 11, 0)), state("water"));
        assert_eq!(at(&w, (1, 10, 0)), AIR);
        assert!(placing::block_for_face("tube_coral_fan", state("tube_coral_fan"), 0).is_none());
    }

    #[test]
    fn waterlogged_coral_survives_but_ice_does_not_water_dry_coral() {
        let mut w = World::in_memory();
        w.set_block_silently(0, 10, 0, state("tube_coral"));
        ticked(&mut w, (0, 10, 0));
        assert_eq!(name(at(&w, (0, 10, 0))), "tube_coral");
        w.set_block_silently(0, 10, 0, parse("tube_coral[waterlogged=false]"));
        w.set_block_silently(1, 10, 0, state("ice"));
        ticked(&mut w, (0, 10, 0));
        assert_eq!(name(at(&w, (0, 10, 0))), "dead_tube_coral");
    }

    #[test]
    fn amethyst_placement_preserves_face_and_only_existing_source_water() {
        let mut w = World::in_memory();
        for face in 0..6 {
            for water in [true, false] {
                w.set_block_silently(0, 10, 0, if water { state("water") } else { AIR });
                let bud = placed(&mut w, (0, 10, 0), state("small_amethyst_bud"), face);
                assert_eq!(property(bud, "facing"), Some(SIDES[face as usize].1));
                assert_eq!(property(bud, "waterlogged"), Some(if water { "true" } else { "false" }));
            }
        }
    }

    fn pipe() -> World {
        let mut w = World::in_memory();
        for y in 1..=4 {
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] { w.set_block_silently(dx, y, dz, state("glass")); }
            w.set_block_silently(0, y, 0, state("water"));
        }
        w
    }

    #[test]
    fn bubble_columns_form_together_change_direction_and_collapse() {
        let mut w = pipe();
        put(&mut w, (0, 0, 0), "soul_sand");
        run(&mut w, 19);
        assert_eq!(at(&w, (0, 1, 0)), state("water"));
        run(&mut w, 1);
        for y in 1..=4 { assert_eq!(property(at(&w, (0, y, 0)), "drag"), Some("false")); }
        run(&mut w, 10); // Жидкостные такты не превращают пузырьки обратно в воду.
        assert_eq!(name(at(&w, (0, 4, 0))), "bubble_column");
        put(&mut w, (0, 0, 0), "magma_block");
        run(&mut w, 5);
        for y in 1..=4 { assert_eq!(property(at(&w, (0, y, 0)), "drag"), Some("true")); }
        put(&mut w, (0, 0, 0), "stone");
        run(&mut w, 4);
        assert_eq!(name(at(&w, (0, 4, 0))), "bubble_column");
        run(&mut w, 1);
        for y in 1..=4 { assert_eq!(at(&w, (0, y, 0)), state("water")); }
    }

    #[test]
    fn bubbles_stop_at_flowing_water_and_waterlogged_blocks() {
        for barrier in ["water[level=1]", "oak_slab[waterlogged=true]", "stone"] {
            let mut w = pipe();
            w.set_block_silently(0, 0, 0, state("soul_sand"));
            w.set_block_silently(0, 2, 0, parse(barrier));
            update_column(&mut w, (0, 1, 0));
            assert_eq!(name(at(&w, (0, 1, 0))), "bubble_column");
            assert_eq!(at(&w, (0, 2, 0)), parse(barrier));
            assert_eq!(at(&w, (0, 3, 0)), state("water"));
        }
    }

    #[test]
    fn amethyst_grows_all_four_stages_on_each_face_and_keeps_water() {
        for (side, (delta, facing)) in SIDES.iter().enumerate() {
            let mut w = World::in_memory();
            let p = (0, 10, 0);
            let next = add(p, *delta);
            w.set_block_silently(p.0, p.1, p.2, state("budding_amethyst"));
            w.set_block_silently(next.0, next.1, next.2, state("water"));
            for n in ["small_amethyst_bud", "medium_amethyst_bud", "large_amethyst_bud", "amethyst_cluster", "amethyst_cluster"] {
                grow_amethyst(&mut w, p, side);
                let bud = at(&w, next);
                assert_eq!(name(bud), n);
                assert_eq!(property(bud, "waterlogged"), Some("true"));
                assert_eq!(property(bud, "facing"), Some(*facing));
            }
        }
    }

    #[test]
    fn amethyst_never_replaces_solid_flowing_water_or_a_differently_attached_bud() {
        for block in ["stone", "water[level=1]", "bubble_column", "small_amethyst_bud[facing=down]"] {
            let mut w = World::in_memory();
            w.set_block_silently(0, 10, 0, state("budding_amethyst"));
            w.set_block_silently(0, 11, 0, parse(block));
            grow_amethyst(&mut w, (0, 10, 0), 1);
            assert_eq!(at(&w, (0, 11, 0)), parse(block));
        }
    }
}
