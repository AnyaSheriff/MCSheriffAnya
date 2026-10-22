// Взаимодействия с простыми блоками без собственного окна.
// minecraft.wiki: Flower Pot, Candle, Cake, Composter, Block of Copper.
// [проверить]: Java 26.1.2, события и приоритет предмета/блока.
use super::*;

type Pos = (i32, i32, i32);

#[derive(Default, Debug)]
struct Used {
    spend: bool,
    swap: Option<i32>,
    wear: bool,
    give: Option<Stack>,
    eat: bool,
    copper: Option<crate::copper::Use>,
}

fn prop(state: i32, key: &str) -> Option<&'static str> { blocks::orientation_of(state)?.value(state, key) }
fn candle(n: &str) -> bool { n == "candle" || n.ends_with("_candle") }
fn cake_candle(n: &str) -> Option<&str> { n.strip_suffix("_cake").filter(|n| candle(n)) }
fn plant_item(n: &str) -> Option<&str> {
    Some(match n.strip_prefix("potted_")? {
        "azalea_bush" => "azalea", "flowering_azalea_bush" => "flowering_azalea", plant => plant,
    })
}
fn potted(item: &str) -> Option<i32> {
    let plant = match item { "azalea" => "azalea_bush", "flowering_azalea" => "flowering_azalea_bush", n => n };
    blocks::state_by_name(&format!("potted_{plant}"))
}
fn stack(n: &str) -> Option<Stack> { Some(Stack::new(blocks::item_named(n)?, 1)) }
fn set(world: &mut World, p: Pos, state: i32) { world.set_block(p.0, p.1, p.2, state); }
fn sound(world: &mut World, p: Pos, n: &'static str) { world.note_sound(p.0, p.1, p.2, n, 1.0, 1.0); }
/// Данные блока костра (тип блок-сущности 33): пустой список `Items`.
fn campfire_data(world: &mut World, p: Pos) {
    world.note_block_entity(p.0, p.1, p.2, 33, vec![0x0A, 0x09, 0x00, 0x05, b'I', b't', b'e', b'm', b's', 0x00, 0, 0, 0, 0, 0x00]);
}
fn hand_sound(world: &mut World, p: Pos, n: &'static str) { world.note_sound_by_hand(p.0, p.1, p.2, n, 1.0, 1.0); }
fn eject(world: &mut World, p: Pos, stack: Stack) {
    let (at, velocity) = crate::tick::scattered(p, || f64::from(world.random_unit()));
    world.eject(at, velocity, stack);
}

fn interact(world: &mut World, p: Pos, held: Option<&str>, hungry: bool, sneaking: bool, cursor_y: f32, face: i32) -> Option<Used> {
    let state = world.get_block(p.0, p.1, p.2);
    let n = blocks::block_at_state(state)?;
    if let Some((next, action)) = held.and_then(|item| crate::copper::interaction(state, item)) {
        crate::copper::replace(world, p, next);
        let (event, effect) = match action {
            crate::copper::Use::Wax => (3003, "minecraft:item.honeycomb.wax_on"),
            crate::copper::Use::Unwax => (3004, "minecraft:item.axe.wax_off"),
            crate::copper::Use::Scrape => (3005, "minecraft:item.axe.scrape"),
        };
        // Игрок слышит и видит это у себя сам (copper-wax-scrape): ему не шлётся.
        hand_sound(world, p, effect);
        world.note_level_event_by_hand(p.0, p.1, p.2, event, 0);
        return Some(Used { spend: action == crate::copper::Use::Wax, wear: action != crate::copper::Use::Wax, copper: Some(action), ..Used::default() });
    }
    if held == Some("shears") && matches!(n, "cave_vines" | "weeping_vines" | "twisting_vines" | "kelp")
        && prop(state, "age") != Some("25")
    {
        set(world, p, blocks::orientation_of(state)?.with(state, "age", "25")?);
        hand_sound(world, p, "minecraft:block.growing_plant.crop");
        return Some(Used { wear: true, ..Used::default() });
    }
    if held.is_some_and(|item| item.ends_with("_axe"))
        && let Some(next) = crate::copper::corresponding(state, &format!("stripped_{n}"))
    {
        set(world, p, next);
        // Игрок слышит звук у себя сам, ему он не шлётся (axe-strip-logs).
        hand_sound(world, p, "minecraft:item.axe.strip");
        return Some(Used { wear: true, ..Used::default() });
    }
    if held.is_some_and(|item| item.ends_with("_shovel")) {
        if matches!(n, "campfire" | "soul_campfire") && prop(state, "lit") == Some("true") {
            set(world, p, blocks::orientation_of(state)?.with(state, "lit", "false")?);
            // Костёр гасят событием 1009 и данными блока (чёрный ящик, shovel-path-campfire).
            world.note_level_event(p.0, p.1, p.2, 1009, 0);
            campfire_data(world, p);
            return Some(Used { wear: true, ..Used::default() });
        }
        if face != 0 && world.get_block(p.0, p.1 + 1, p.2) == world::AIR
            && matches!(n, "grass_block" | "dirt" | "coarse_dirt" | "rooted_dirt" | "podzol" | "mycelium")
        {
            set(world, p, blocks::state_by_name("dirt_path")?);
            hand_sound(world, p, "minecraft:item.shovel.flatten");
            return Some(Used { wear: true, ..Used::default() });
        }
    }
    let on_cake = cake_candle(n);
    if (candle(n) || on_cake.is_some() || matches!(n, "campfire" | "soul_campfire")) && matches!(held, Some("flint_and_steel" | "fire_charge"))
        && prop(state, "lit") == Some("false") && prop(state, "waterlogged") != Some("true")
    {
        set(world, p, blocks::orientation_of(state)?.with(state, "lit", "true")?);
        if held == Some("fire_charge") { sound(world, p, "minecraft:item.firecharge.use"); } else { hand_sound(world, p, "minecraft:item.flintandsteel.use"); }
        if matches!(n, "campfire" | "soul_campfire") { campfire_data(world, p); }
        return Some(Used { spend: held == Some("fire_charge"), wear: held == Some("flint_and_steel"), ..Used::default() });
    }
    if sneaking { return None; }
    if let Some(used) = crate::compost::use_item(world, p, held) {
        if used == crate::compost::Used::Emptied { eject(world, (p.0, p.1 + 1, p.2), stack("bone_meal")?); }
        return Some(Used { spend: used == crate::compost::Used::Added, ..Used::default() });
    }
    if n == "flower_pot" {
        set(world, p, potted(held?)?);
        return Some(Used { spend: true, ..Used::default() });
    }
    if let Some(plant) = plant_item(n) {
        // Подходящее растение в занятой ёмкости не меняет содержимое.
        if held.and_then(potted).is_some() { return Some(Used::default()); }
        let give = stack(plant)?;
        set(world, p, blocks::state_by_name("flower_pot")?);
        return Some(Used { give: Some(give), ..Used::default() });
    }
    if n == "cake" && prop(state, "bites") == Some("0") && held.is_some_and(candle) {
        set(world, p, blocks::state_by_name(&format!("{}_cake", held?))?);
        sound(world, p, "minecraft:block.cake.add_candle");
        return Some(Used { spend: true, ..Used::default() });
    }
    if (candle(n) || on_cake.is_some() && cursor_y > 0.5) && held.is_none() && prop(state, "lit") == Some("true") {
        set(world, p, blocks::orientation_of(state)?.with(state, "lit", "false")?);
        sound(world, p, "minecraft:block.candle.extinguish");
        return Some(Used::default());
    }
    if (n == "cake" || on_cake.is_some()) && hungry {
        if let Some(candle) = on_cake { eject(world, p, stack(candle)?); }
        let bites: u8 = prop(state, "bites").and_then(|v| v.parse().ok()).unwrap_or(0);
        let next = if bites == 6 { world::AIR } else { blocks::orientation("cake")?.state(&[("bites", &(bites + 1).to_string())])? };
        set(world, p, next);
        return Some(Used { eat: true, ..Used::default() });
    }
    None
}

// Вики, Pumpkin: «four pumpkin seeds»; горизонтальная грань задаёт лицо,
// на верхней/нижней — направление к игроку. [проверить]: скорость семян.
fn carve_pumpkin(world: &mut World, p: Pos, face: i32, yaw: f32) -> Option<Used> {
    if blocks::block_at_state(world.get_block(p.0, p.1, p.2)) != Some("pumpkin") { return None; }
    let facing = match face {
        2 => "north", 3 => "south", 4 => "west", 5 => "east",
        _ => match ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) {
            0 => "north", 1 => "east", 2 => "south", _ => "west",
        },
    };
    let next = blocks::orientation("carved_pumpkin")?.state(&[("facing", facing)])?;
    set(world, p, next);
    let (dx, dz) = match facing { "north" => (0.0, -1.0), "south" => (0.0, 1.0), "west" => (-1.0, 0.0), _ => (1.0, 0.0) };
    world.eject((f64::from(p.0) + 0.5 + dx * 0.65, f64::from(p.1) + 0.1, f64::from(p.2) + 0.5 + dz * 0.65),
        (dx * 0.05, 0.05, dz * 0.05), Stack::new(blocks::item_named("pumpkin_seeds")?, 4));
    sound(world, p, "minecraft:block.pumpkin.carve");
    Some(Used { wear: true, ..Used::default() })
}

// Вики, Mud: «dirt, coarse dirt, or rooted dirt»; только бутылка воды,
// а не любой предмет potion. Пустая бутылка возвращается в ту же руку.
fn pour_mud(world: &mut World, p: Pos, held: Stack) -> Option<Used> {
    if blocks::item_name(held.item) != Some("potion") || held.extras().potion != crate::survival::potion_number("water") { return None; }
    if !matches!(blocks::block_at_state(world.get_block(p.0, p.1, p.2)), Some("dirt" | "coarse_dirt" | "rooted_dirt")) { return None; }
    set(world, p, blocks::state_by_name("mud")?);
    // Плеск и пять брызг над блоком (чёрный ящик, block3-dirt) — до звука бутылки.
    sound(world, p, "minecraft:entity.generic.splash");
    for _ in 0..5 {
        let at = (f64::from(p.0) + f64::from(world.random_unit()), f64::from(p.1) + 1.0, f64::from(p.2) + f64::from(world.random_unit()));
        world.note_particle(70, None, at, (0.0, 0.0, 0.0), 1.0, 1);
    }
    sound(world, p, "minecraft:item.bottle.empty");
    Some(Used { swap: blocks::item_named("glass_bottle"), ..Used::default() })
}

pub(super) fn use_block(player: &mut PlayerState, shared: &Shared, p: Pos, face: i32, cursor_y: f32, sequence: i32) -> Option<Reaction> {
    let held = player.inventory.held();
    let held_name = held.and_then(|s| blocks::item_name(s.item));
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    world.actor = Some(player.entity_id);
    let used = held.and_then(|stack| pour_mud(&mut world, p, stack))
        .or_else(|| (held_name == Some("shears")).then(|| carve_pumpkin(&mut world, p, face, player.yaw)).flatten())
        .or_else(|| interact(&mut world, p, held_name, creative || player.vit.hungry(), player.sneaking, cursor_y, face));
    world.actor = None;
    let used = used?;
    let beyond = offset_towards(p.0, p.1, p.2, face);
    world.tell_directly(p.0, p.1, p.2);
    world.tell_directly(beyond.0, beyond.1, beyond.2);
    redstone::settle(&mut world);
    drop(world);
    if used.eat {
        player.vit.eat(2, 0.4);
        player.progress.custom("eat_cake_slice", 1);
    }
    let mut slots = if used.spend { spend_placed(player) } else { Vec::new() };
    if !creative && let Some(item) = used.swap { slots.extend(swap_held(player, item, shared)); }
    if used.wear && !creative && let Some(stack) = held {
        pvp::wear_held(shared, player, stack, 1, false);
        slots.push(player.inventory.selected_slot());
    }
    if let Some(stack) = used.give {
        let (changed, left) = player.inventory.put(stack);
        slots.extend(changed);
        if let Some(left) = left { throw_out(shared, player, left); }
    }
    if let Some(action) = used.copper {
        match action {
            crate::copper::Use::Wax => player.progress.trigger("husbandry/wax_on"),
            crate::copper::Use::Unwax => player.progress.trigger("husbandry/wax_off"),
            crate::copper::Use::Scrape => {},
        }
    }
    Some(Reaction { slots, ..Reaction::accepted(sequence) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(w: &mut World, n: &str) { w.set_block_silently(0, 10, 0, blocks::state_from_text(n).unwrap()); }
    fn use_it(w: &mut World, item: Option<&str>, hungry: bool, y: f32) -> Option<Used> { interact(w, (0, 10, 0), item, hungry, false, y, 1) }
    #[test]
    fn pumpkin_carving_has_four_seeds_faces_and_no_second_use() {
        let mut w = World::in_memory();
        for (face, yaw, facing) in [(2, 0.0, "north"), (3, 0.0, "south"), (4, 0.0, "west"), (5, 0.0, "east"), (1, 90.0, "east"), (0, 180.0, "south")] {
            put(&mut w, "pumpkin");
            assert!(carve_pumpkin(&mut w, (0, 10, 0), face, yaw).unwrap().wear);
            assert_eq!(prop(w.get_block(0, 10, 0), "facing"), Some(facing));
            let drops = w.take_ejected();
            assert_eq!(drops.len(), 1);
            assert_eq!(drops[0].2, Stack::new(blocks::item_named("pumpkin_seeds").unwrap(), 4));
            assert!(carve_pumpkin(&mut w, (0, 10, 0), face, yaw).is_none());
        }
    }

    fn muddy_soil(n: &str) {
        let mut w = World::in_memory();
        put(&mut w, n);
        let potion = Stack::new(blocks::item_named("potion").unwrap(), 1);
        let healing = potion.with_extras(crate::stackdata::Data { potion: crate::survival::potion_number("healing"), ..Default::default() });
        let water = potion.with_extras(crate::stackdata::Data { potion: crate::survival::potion_number("water"), ..Default::default() });
        assert!(pour_mud(&mut w, (0, 10, 0), healing).is_none());
        let used = pour_mud(&mut w, (0, 10, 0), water).unwrap();
        assert_eq!(used.swap, blocks::item_named("glass_bottle"));
        assert!(!used.spend && !used.wear);
        assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some("mud"));
        assert!(pour_mud(&mut w, (0, 10, 0), water).is_none());
    }
    #[test] fn dirt_water_bottle() { muddy_soil("dirt"); }
    #[test] fn coarse_dirt_water_bottle() { muddy_soil("coarse_dirt"); }
    #[test] fn rooted_dirt_water_bottle() { muddy_soil("rooted_dirt"); }

    #[test]
    fn mud_interaction_swaps_survival_bottle_and_preserves_creative_stack() {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(),
            Default::default(), crate::ops::Ops::open(std::path::Path::new("tmp/block3-ops.json")), crate::skins::Settings::default());
        let mut player = PlayerState::new([31; 16], "BlockUseTest".into(), None, None, 0x7F, 31, (0.5, 10.0, 2.5));
        let water = Stack::new(blocks::item_named("potion").unwrap(), 1).with_extras(crate::stackdata::Data {
            potion: crate::survival::potion_number("water"), ..Default::default()
        });
        for creative in [false, true] {
            player.game_mode = if creative { GAME_MODE_CREATIVE } else { GAME_MODE_SURVIVAL };
            player.inventory.set(inventory::FIRST_HOTBAR, Some(water));
            put(&mut shared.world_in(player.dimension).lock().unwrap(), "dirt");
            let reaction = use_block(&mut player, &shared, (0, 10, 0), 1, 0.5, 1).unwrap();
            assert_eq!(player.inventory.held(), Some(if creative { water } else { stack("glass_bottle").unwrap() }));
            assert_eq!(reaction.slots.is_empty(), creative);
        }
    }

    #[test]
    fn shears_stop_growth_without_erasing_berries() {
        let mut w = World::in_memory();
        put(&mut w, "cave_vines[age=4,berries=true]");
        assert!(use_it(&mut w, Some("shears"), false, 0.5).unwrap().wear);
        assert_eq!(prop(w.get_block(0, 10, 0), "age"), Some("25"));
        assert_eq!(prop(w.get_block(0, 10, 0), "berries"), Some("true"));
        assert!(use_it(&mut w, Some("shears"), false, 0.5).is_none());
    }

    #[test]
    fn axes_strip_all_logs_preserving_axis_and_shovels_need_clear_above() {
        let mut w = World::in_memory();
        for wood in ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak"] {
            for suffix in ["log", "wood"] {
                for axis in ["x", "y", "z"] {
                    let n = format!("{wood}_{suffix}");
                    put(&mut w, &format!("{n}[axis={axis}]"));
                    assert!(use_it(&mut w, Some("iron_axe"), false, 0.5).unwrap().wear);
                    assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some(format!("stripped_{n}").as_str()));
                    assert_eq!(prop(w.get_block(0, 10, 0), "axis"), Some(axis));
                    assert!(use_it(&mut w, Some("iron_axe"), false, 0.5).is_none());
                }
            }
        }
        put(&mut w, "grass_block");
        w.set_block_silently(0, 11, 0, blocks::state_by_name("stone").unwrap());
        assert!(use_it(&mut w, Some("iron_shovel"), false, 0.5).is_none());
        w.set_block_silently(0, 11, 0, world::AIR);
        assert!(use_it(&mut w, Some("iron_shovel"), false, 0.5).unwrap().wear);
        assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some("dirt_path"));
        put(&mut w, "soul_campfire[lit=true]");
        assert!(use_it(&mut w, Some("iron_shovel"), false, 0.5).unwrap().wear);
        assert_eq!(prop(w.get_block(0, 10, 0), "lit"), Some("false"));
        assert!(use_it(&mut w, Some("fire_charge"), false, 0.5).unwrap().spend);
    }

    #[test]
    fn every_potted_plant_can_be_inserted_and_retrieved() {
        // Обычный мир не знает тополя; профиль Preview проверяет и его горшок.
        for preview in [false, true] {
            let mut w = if preview { World::in_memory_with_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3)) } else { World::in_memory() };
            let mut poplar = 0;
            for state in 0..=u16::MAX as i32 {
                let Some(n) = blocks::block_at_state(state) else { continue; };
                if !preview && n.contains("poplar") { continue; }
                let Some(plant) = plant_item(n) else { continue; };
                poplar += usize::from(n.contains("poplar"));
                put(&mut w, "flower_pot");
                assert!(use_it(&mut w, Some(plant), false, 0.5).unwrap().spend, "{plant}");
                assert_eq!(w.get_block(0, 10, 0), state);
                assert_eq!(use_it(&mut w, None, false, 0.5).unwrap().give, stack(plant));
                assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some("flower_pot"));
            }
            assert_eq!(poplar, usize::from(preview));
        }
    }
    #[test]
    fn cake_hunger_seven_slices_and_candle_return() {
        let mut w = World::in_memory();
        put(&mut w, "cake");
        assert!(use_it(&mut w, None, false, 0.5).is_none());
        assert!(use_it(&mut w, Some("red_candle"), false, 0.5).unwrap().spend);
        assert_eq!(crate::containers::comparator_reading(&w, (0, 10, 0)), Some(14));
        assert!(use_it(&mut w, None, true, 0.2).unwrap().eat);
        assert_eq!(prop(w.get_block(0, 10, 0), "bites"), Some("1"));
        for _ in 0..6 { assert!(use_it(&mut w, None, true, 0.2).unwrap().eat); }
        assert_eq!(w.get_block(0, 10, 0), world::AIR);
        assert!(use_it(&mut w, None, true, 0.2).is_none());
    }
    #[test]
    fn candle_lighting_extinguishing_and_water() {
        let mut w = World::in_memory();
        for n in ["candle[candles=4]", "blue_candle_cake"] {
            put(&mut w, n);
            assert!(use_it(&mut w, Some("flint_and_steel"), false, 0.8).unwrap().wear);
            assert_eq!(prop(w.get_block(0, 10, 0), "lit"), Some("true"));
            assert!(use_it(&mut w, None, false, 0.8).is_some());
            assert_eq!(prop(w.get_block(0, 10, 0), "lit"), Some("false"));
        }
        put(&mut w, "candle[waterlogged=true]");
        assert!(use_it(&mut w, Some("fire_charge"), false, 0.5).is_none());
    }
    #[test]
    fn full_pot_does_not_consume_another_plant() {
        let mut w = World::in_memory();
        put(&mut w, "potted_poppy");
        assert!(!use_it(&mut w, Some("cactus"), false, 0.5).unwrap().spend);
        assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some("potted_poppy"));
    }
}
