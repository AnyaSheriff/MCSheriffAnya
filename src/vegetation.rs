// Дополнительные правила растений — minecraft.wiki: Bone Meal, Pink Petals,
// Netherrack, Hanging Roots, Twisting Vines, Weeping Vines, Carpet, Snow.
// [проверить]: случайные последовательности, эффекты и задержки Java 26.1.2.
use crate::{blocks, fluids};
use crate::inventory::Stack;
use crate::world::{World, AIR, MIN_Y, WORLD_HEIGHT};

type Pos = (i32, i32, i32);
fn at(w: &World, p: Pos) -> i32 { w.get_block(p.0, p.1, p.2) }
fn set(w: &mut World, p: Pos, s: i32) { w.set_block(p.0, p.1, p.2, s); }
fn state(n: &str) -> i32 { blocks::state_by_name(n).expect("блок есть в таблице") }
fn name(s: i32) -> &'static str { blocks::block_at_state(s).unwrap_or("air") }
fn prop(s: i32, k: &str) -> Option<&'static str> { blocks::orientation_of(s)?.value(s, k) }
fn with(s: i32, k: &str, v: impl ToString) -> i32 {
    blocks::orientation_of(s).and_then(|b| b.with(s, k, &v.to_string())).unwrap_or(s)
}
fn shift(p: Pos, dy: i32) -> Pos { (p.0, p.1 + dy, p.2) }
fn in_world(p: Pos) -> bool { (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&p.1) }

/// Число растений/слоёв в одном месте. Вики: «up to four»; снег — восемь.
pub fn count_property(n: &str) -> Option<(&'static str, u8)> {
    Some(match n {
        "pink_petals" | "wildflowers" => ("flower_amount", 4),
        "leaf_litter" => ("segment_amount", 4),
        "snow" => ("layers", 8),
        _ => return None,
    })
}

pub fn stack_more(s: i32) -> Option<i32> {
    let (key, max) = count_property(name(s))?;
    let amount = prop(s, key)?.parse::<u8>().ok()?;
    (amount < max).then(|| with(s, key, amount + 1))
}

fn drop_one(w: &mut World, p: Pos, n: &str) {
    if let Some(item) = blocks::item_named(n) {
        let (at, velocity) = crate::tick::scattered(p, || f64::from(w.random_unit()));
        w.eject(at, velocity, Stack::new(item, 1));
    }
}

/// None — не наша механика, Some(false) — подходящий блок, но нет места.
/// Это отделяет неудачную попытку от расхода костной муки.
pub fn bone_meal(w: &mut World, p: Pos) -> Option<bool> {
    let s = at(w, p);
    let n = name(s);
    match n {
        "short_grass" | "fern" | "seagrass" => {
            let above = shift(p, 1);
            let target = match n { "short_grass" => "tall_grass", "fern" => "large_fern", _ => "tall_seagrass" };
            let free = if n == "seagrass" { at(w, above) == state("water") } else { at(w, above) == AIR };
            if !in_world(above) || !free || !crate::plants::survives(w, s, p.0, p.1, p.2) { return Some(false); }
            let pair = state(target);
            // Обе половины записаны до обработки очереди соседей.
            set(w, p, with(pair, "half", "lower"));
            set(w, above, with(pair, "half", "upper"));
        }
        "sunflower" | "lilac" | "rose_bush" | "peony" => {
            if !crate::plants::survives(w, s, p.0, p.1, p.2) { return Some(false); }
            drop_one(w, p, n);
        }
        "pink_petals" | "wildflowers" => {
            if let Some(next) = stack_more(s) { set(w, p, next); } else { drop_one(w, p, n); }
        }
        "rooted_dirt" => {
            let below = shift(p, -1);
            if !in_world(below) || at(w, below) != AIR { return Some(false); }
            set(w, below, with(state("hanging_roots"), "waterlogged", false));
        }
        "netherrack" => {
            if crate::world::light::filter(at(w, shift(p, 1))) == 15 { return Some(false); }
            let mut crimson = false;
            let mut warped = false;
            for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
                match name(at(w, (p.0 + dx, p.1 + dy, p.2 + dz))) {
                    "crimson_nylium" => crimson = true,
                    "warped_nylium" => warped = true,
                    _ => {}
                }
            } } }
            if !crimson && !warped { return Some(false); }
            let use_crimson = crimson && (!warped || w.random_bits().is_multiple_of(2));
            set(w, p, state(if use_crimson { "crimson_nylium" } else { "warped_nylium" }));
        }
        n if vine(n).is_some() => return Some(fertilize_vine(w, p)),
        _ => return None,
    }
    Some(true)
}

fn vine(n: &str) -> Option<(&'static str, &'static str, i32)> {
    match n {
        "twisting_vines" | "twisting_vines_plant" => Some(("twisting_vines", "twisting_vines_plant", 1)),
        "weeping_vines" | "weeping_vines_plant" => Some(("weeping_vines", "weeping_vines_plant", -1)),
        _ => None,
    }
}

/// Опора и восстановление верхушки общие для игрока и команд.
/// [проверить]: неполные грани опоры; используется существующая геометрия.
pub fn supported(w: &World, p: Pos, s: i32) -> Option<bool> {
    let n = name(s);
    if let Some((tip, body, dy)) = vine(n) {
        let support = at(w, shift(p, -dy));
        return Some(matches!(name(support), n if n == tip || n == body)
            || if dy == 1 { blocks::solid_top(support) } else { blocks::full_cube(support) });
    }
    if n == "hanging_roots" { return Some(blocks::full_cube(at(w, shift(p, 1)))); }
    // Вики, Carpet: «any block, including non-solid blocks», но не воздух.
    if n.ends_with("_carpet") && !matches!(n, "moss_carpet" | "pale_moss_carpet") {
        return Some(!matches!(name(at(w, shift(p, -1))), "air" | "cave_air" | "void_air"));
    }
    None
}

pub fn vine_changed(w: &mut World, p: Pos) -> bool {
    let s = at(w, p);
    let Some((tip, body, dy)) = vine(name(s)) else { return false; };
    if supported(w, p, s) == Some(false) {
        w.schedule_block_once(p.0, p.1, p.2, 1);
        return true;
    }
    let next = name(at(w, shift(p, dy)));
    let has_next = next == tip || next == body;
    if has_next && name(s) == tip { set(w, p, state(body)); }
    if !has_next && name(s) == body {
        // Вики: верхушка получает случайный возраст 0…24 [проверить].
        let age = w.random_bits() % 25;
        set(w, p, with(state(tip), "age", age));
    }
    true
}

pub fn ticked(w: &mut World, p: Pos) -> bool {
    let s = at(w, p);
    if vine(name(s)).is_none() { return false; }
    if supported(w, p, s) == Some(false) {
        // Лоза без опоры ломается «как положено», с событием 2001 (block3-weeping-vines-plant).
        w.note_level_event(p.0, p.1, p.2, 2001, s);
        w.note_destroyed(p.0, p.1, p.2);
        set(w, p, AIR);
    }
    true
}

fn fertilize_vine(w: &mut World, mut p: Pos) -> bool {
    let (tip, body, dy) = vine(name(at(w, p))).unwrap();
    while in_world(shift(p, dy)) && matches!(name(at(w, shift(p, dy))), n if n == tip || n == body) { p = shift(p, dy); }
    if !in_world(shift(p, dy)) || at(w, shift(p, dy)) != AIR { return false; }
    // Вики: мука действует и после age=25. Убывающая вероятность длины
    // 1, 0.826, 0.826²… [проверить] последовательность Java 26.1.2.
    let mut probability = 1.0;
    while in_world(shift(p, dy)) && at(w, shift(p, dy)) == AIR && w.random_unit() < probability {
        let age = prop(at(w, p), "age").and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
        let next = shift(p, dy);
        set(w, next, with(state(tip), "age", (age + 1).min(25)));
        set(w, p, state(body));
        p = next;
        probability *= 0.826;
    }
    true
}

/// Снежные слои тают только от блочного света, не от солнца.
pub fn snow_tick(w: &mut World, p: Pos) {
    if w.light_full(p.0, p.1, p.2).1 > 11 { set(w, p, AIR); }
}

/// Сломанное подводное растение оставляет источник, а не воздушный карман.
pub fn fluid_left(s: i32) -> i32 {
    if matches!(name(s), "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant") || prop(s, "waterlogged") == Some("true") {
        fluids::state_of(fluids::Kind::Water, fluids::SOURCE)
    } else { AIR }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone;
    use crate::placing::{self, Click, Placement};
    fn put(w: &mut World, p: Pos, text: &str) { w.set_block_silently(p.0, p.1, p.2, blocks::state_from_text(text).unwrap()); }
    fn grow(w: &mut World, p: Pos) -> bool { crate::farming::bone_meal(w, p).is_some() }
    fn settle(w: &mut World) { redstone::settle(w); }
    fn place(w: &mut World, n: &'static str, p: Pos) -> Placement {
        placing::place(w, n, state(n), p, &Click { face: 1, cursor_y: 1.0, yaw: 0.0, pitch: 0.0 }, (5.0, 10.0, 5.0))
    }
    fn ticks(w: &mut World, count: usize) {
        for _ in 0..count {
            for (p, kind) in w.advance(10000) {
                w.begin_tick(p, kind);
                if kind == crate::world::TickKind::Block { redstone::ticked(w, p); }
            }
            settle(w);
        }
    }

    fn flower_cycle(n: &'static str) {
        let mut w = World::in_memory();
        put(&mut w, (0, 9, 0), "dirt");
        put(&mut w, (0, 10, 0), &format!("{n}[half=lower]"));
        put(&mut w, (0, 11, 0), &format!("{n}[half=upper]"));
        for p in [(0, 10, 0), (0, 11, 0)] {
            assert!(grow(&mut w, p));
            settle(&mut w);
            let drops = w.take_ejected();
            assert_eq!(drops.len(), 1);
            assert_eq!(drops[0].2, Stack::new(blocks::item_named(n).unwrap(), 1));
            assert_eq!(name(at(&w, p)), n);
        }
        set(&mut w, (0, 9, 0), AIR);
        settle(&mut w);
        assert_eq!(at(&w, (0, 10, 0)), AIR);
        assert_eq!(at(&w, (0, 11, 0)), AIR);
    }
    macro_rules! flowers { ($($n:ident),+) => { $(#[test] fn $n() { flower_cycle(stringify!($n)); })+ }; }
    flowers!(sunflower, lilac, rose_bush, peony);

    fn grass_cycle(n: &'static str, tall: &'static str, break_upper: bool) {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        put(&mut w, shift(p, -1), "dirt");
        put(&mut w, p, n);
        put(&mut w, shift(p, 1), "stone");
        assert!(!grow(&mut w, p), "не затирает препятствие");
        let water = n == "seagrass";
        set(&mut w, shift(p, 1), if water { state("water") } else { AIR });
        assert!(grow(&mut w, p));
        settle(&mut w);
        assert_eq!(name(at(&w, p)), tall);
        assert_eq!(prop(at(&w, p), "half"), Some("lower"));
        assert_eq!(name(at(&w, shift(p, 1))), tall);
        assert_eq!(prop(at(&w, shift(p, 1)), "half"), Some("upper"));
        assert!(!grow(&mut w, p), "высокая трава не размножается в Java");
        let sheared = blocks::drops_by_luck(at(&w, p), blocks::item_named("shears"), 0.8).unwrap();
        assert_eq!(sheared.count, 2);
        assert_eq!(sheared.item, blocks::item_named(n).unwrap());
        let broken = if break_upper { shift(p, 1) } else { p };
        assert!(placing::break_block(&mut w, broken.0, broken.1, broken.2).is_some());
        settle(&mut w);
        assert_eq!(at(&w, p), if water { state("water") } else { AIR });
        assert_eq!(at(&w, shift(p, 1)), if water { state("water") } else { AIR });
    }
    #[test] fn short_grass() { grass_cycle("short_grass", "tall_grass", true); }
    #[test] fn tall_grass() { grass_cycle("short_grass", "tall_grass", false); }
    #[test] fn fern() { grass_cycle("fern", "large_fern", true); }
    #[test] fn large_fern() { grass_cycle("fern", "large_fern", false); }
    #[test] fn seagrass() { grass_cycle("seagrass", "tall_seagrass", true); }
    #[test] fn tall_seagrass() { grass_cycle("seagrass", "tall_seagrass", false); }

    fn flowerbed(n: &'static str) {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        put(&mut w, shift(p, -1), "dirt");
        put(&mut w, p, &format!("{n}[facing=west]"));
        let key = count_property(n).unwrap().0;
        for count in 2..=4 {
            assert_eq!(place(&mut w, n, p), Placement::Placed);
            assert_eq!(prop(at(&w, p), key), Some(count.to_string().as_str()));
            assert_eq!(prop(at(&w, p), "facing"), Some("west"));
        }
        assert_ne!(place(&mut w, n, p), Placement::Placed);
        let drops = crate::farming::harvest(&mut w, at_state(n, key, 4), 0).unwrap();
        assert_eq!(drops, vec![Stack::new(blocks::item_named(n).unwrap(), 4)]);
        if n != "leaf_litter" {
            assert!(grow(&mut w, p));
            assert_eq!(w.take_ejected()[0].2, Stack::new(blocks::item_named(n).unwrap(), 1));
            set(&mut w, p, at_state(n, key, 1));
            assert!(grow(&mut w, p));
            assert_eq!(prop(at(&w, p), key), Some("2"));
        } else { assert!(!grow(&mut w, p)); }
        set(&mut w, shift(p, -1), AIR);
        settle(&mut w);
        assert_eq!(at(&w, p), AIR);
    }
    fn at_state(n: &str, key: &str, value: u8) -> i32 { with(state(n), key, value) }
    #[test] fn pink_petals() { flowerbed("pink_petals"); }
    #[test] fn wildflowers() { flowerbed("wildflowers"); }
    #[test] fn leaf_litter() { flowerbed("leaf_litter"); }

    #[test]
    fn rooted_dirt_grows_roots_only_into_air_and_hoe_works_under_cover() {
        let mut w = World::in_memory();
        put(&mut w, (0, 10, 0), "rooted_dirt");
        assert!(grow(&mut w, (0, 10, 0)));
        settle(&mut w);
        assert_eq!(name(at(&w, (0, 9, 0))), "hanging_roots");
        assert!(!grow(&mut w, (0, 10, 0)));
        put(&mut w, (0, 11, 0), "stone");
        let dropped = crate::farming::till(&mut w, (0, 10, 0), 0).unwrap().unwrap();
        assert_eq!(dropped.item, blocks::item_named("hanging_roots").unwrap());
        assert_eq!(name(at(&w, (0, 10, 0))), "dirt");
    }

    #[test]
    fn hanging_roots() {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        put(&mut w, shift(p, 1), "stone");
        put(&mut w, p, "hanging_roots[waterlogged=true]");
        let roots = at(&w, p);
        assert_eq!(supported(&w, p, roots), Some(true));
        assert_eq!(blocks::drops_by_luck(roots, None, 0.0), None);
        assert_eq!(blocks::drops_by_luck(roots, blocks::item_named("shears"), 0.0).unwrap().count, 1);
        set(&mut w, shift(p, 1), AIR);
        settle(&mut w);
        assert_eq!(at(&w, p), state("water"));
    }

    #[test]
    fn netherrack() {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        put(&mut w, p, "netherrack");
        assert!(!grow(&mut w, p));
        put(&mut w, (2, 10, 0), "crimson_nylium");
        assert!(!grow(&mut w, p), "за радиусом 1");
        for n in ["crimson_nylium", "warped_nylium"] {
            put(&mut w, p, "netherrack");
            put(&mut w, (1, 11, 1), n);
            put(&mut w, shift(p, 1), "stone");
            assert!(!grow(&mut w, p));
            set(&mut w, shift(p, 1), state("glass"));
            assert!(grow(&mut w, p));
            assert_eq!(name(at(&w, p)), n);
        }
    }

    fn vine_cycle(n: &'static str) {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        let (tip, body, dy) = vine(n).unwrap();
        put(&mut w, shift(p, -dy), "stone");
        put(&mut w, p, body);
        put(&mut w, shift(p, dy), &format!("{tip}[age=25]"));
        let clicked = if n == body { p } else { shift(p, dy) };
        assert!(grow(&mut w, clicked));
        settle(&mut w);
        assert_eq!(name(at(&w, shift(p, dy))), body, "мука действует после age=25");
        assert!(vine(name(at(&w, shift(p, 2 * dy)))).is_some());
        set(&mut w, shift(p, dy), AIR);
        settle(&mut w);
        assert_eq!(name(at(&w, p)), tip, "восстановлена верхушка");
        let age: u8 = prop(at(&w, p), "age").unwrap().parse().unwrap();
        assert!(age < 25);
        set(&mut w, shift(p, -dy), AIR);
        settle(&mut w);
        assert_ne!(at(&w, p), AIR, "разрушение отложено");
        ticks(&mut w, 30);
        for step in -10..=10 { assert!(vine(name(at(&w, shift(p, step)))).is_none()); }
    }
    #[test] fn twisting_vines() { vine_cycle("twisting_vines"); }
    #[test] fn twisting_vines_plant() { vine_cycle("twisting_vines_plant"); }
    #[test] fn weeping_vines() { vine_cycle("weeping_vines"); }
    #[test] fn weeping_vines_plant() { vine_cycle("weeping_vines_plant"); }

    #[test]
    fn dirt_path() {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        put(&mut w, p, "dirt_path");
        set(&mut w, shift(p, 1), state("stone"));
        settle(&mut w);
        assert_eq!(name(at(&w, p)), "dirt_path");
        ticks(&mut w, 1);
        assert_eq!(name(at(&w, p)), "dirt");
        put(&mut w, p, "dirt_path");
        set(&mut w, shift(p, 1), state("oak_fence_gate"));
        settle(&mut w);
        ticks(&mut w, 2);
        assert_eq!(name(at(&w, p)), "dirt_path");
    }

    #[test]
    fn snow() {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        put(&mut w, shift(p, -1), "stone");
        put(&mut w, p, "snow[layers=1]");
        for layers in 2..=8 {
            assert_eq!(place(&mut w, "snow", p), Placement::Placed);
            assert_eq!(prop(at(&w, p), "layers"), Some(layers.to_string().as_str()));
        }
        assert_eq!(place(&mut w, "snow", p), Placement::Placed);
        assert_eq!(prop(at(&w, shift(p, 1)), "layers"), Some("1"));
        crate::farming::random_tick(&mut w, p, state("snow"));
        assert_eq!(prop(at(&w, p), "layers"), Some("8"), "солнце не топит снег");
        set(&mut w, shift(p, -1), AIR);
        settle(&mut w);
        assert_eq!(at(&w, p), AIR);
        assert_eq!(at(&w, shift(p, 1)), AIR);
        put(&mut w, shift(p, -1), "stone");
        put(&mut w, p, "snow[layers=5]");
        put(&mut w, (1, 10, 0), "glowstone");
        crate::farming::random_tick(&mut w, p, state("snow"));
        assert_eq!(at(&w, p), AIR, "блочный свет >11 топит весь слой без дропа");
    }

    fn carpet(n: &'static str) {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        assert!(!crate::block_behaviour::supported(&w, p, state(n)));
        assert_ne!(place(&mut w, n, shift(p, -1)), Placement::Placed);
        put(&mut w, shift(p, -1), "stone");
        assert_eq!(place(&mut w, n, shift(p, -1)), Placement::Placed);
        assert_eq!(place(&mut w, n, p), Placement::Placed);
        assert_eq!(name(at(&w, shift(p, 1))), n);
        assert_eq!(blocks::drops_by_luck(at(&w, p), None, 0.5).unwrap(), Stack::new(blocks::item_named(n).unwrap(), 1));
        set(&mut w, shift(p, -1), AIR);
        settle(&mut w);
        assert_eq!(at(&w, p), AIR);
        assert_eq!(at(&w, shift(p, 1)), AIR);
    }
    macro_rules! carpets { ($($n:ident),+) => { $(#[test] fn $n() { carpet(stringify!($n)); })+ }; }
    carpets!(white_carpet, orange_carpet, magenta_carpet, light_blue_carpet, yellow_carpet, lime_carpet,
        pink_carpet, gray_carpet, light_gray_carpet, cyan_carpet, purple_carpet, blue_carpet,
        brown_carpet, green_carpet, red_carpet, black_carpet);
}
