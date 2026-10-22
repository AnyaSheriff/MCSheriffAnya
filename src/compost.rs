// Компостер: таблица minecraft.wiki (Composter / Komposter), семь слоёв
// и созревание, затем одна костная мука. [проверить]: такты и события Java.
use crate::{blocks, world::World};

/// Процент успеха. Бамбук, ядовитый картофель и мясо не компостируются.
pub fn chance(item: &str) -> Option<u32> {
    Some(match item {
        "cake" | "pumpkin_pie" => 100,
        "baked_potato" | "bread" | "cookie" | "hay_block" | "brown_mushroom_block"
        | "red_mushroom_block" | "nether_wart_block" | "warped_wart_block"
        | "flowering_azalea" | "torchflower" | "pitcher_plant" => 85,
        "apple" | "beetroot" | "carrot" | "potato" | "wheat" | "cocoa_beans"
        | "melon" | "pumpkin" | "carved_pumpkin" | "nether_wart" | "brown_mushroom"
        | "red_mushroom" | "shelf_mushroom" | "crimson_fungus" | "warped_fungus" | "crimson_roots"
        | "warped_roots" | "shroomlight" | "fern" | "large_fern" | "lily_pad"
        | "sea_pickle" | "spore_blossom" | "moss_block" | "azalea" | "big_dripleaf"
        | "dandelion" | "poppy" | "blue_orchid" | "allium" | "azure_bluet"
        | "red_tulip" | "orange_tulip" | "white_tulip" | "pink_tulip"
        | "oxeye_daisy" | "cornflower" | "lily_of_the_valley" | "wither_rose"
        | "sunflower" | "lilac" | "rose_bush" | "peony" => 65,
        "cactus" | "dried_kelp_block" | "melon_slice" | "sugar_cane" | "tall_grass"
        | "vine" | "nether_sprouts" | "weeping_vines" | "twisting_vines"
        | "flowering_azalea_leaves" => 50,
        "wheat_seeds" | "beetroot_seeds" | "melon_seeds" | "pumpkin_seeds"
        | "torchflower_seeds" | "pitcher_pod" | "sweet_berries" | "glow_berries"
        | "dried_kelp" | "kelp" | "seagrass" | "short_grass" | "hanging_roots"
        | "mangrove_roots" | "mangrove_propagule" | "moss_carpet" | "small_dripleaf"
        | "pink_petals" | "red_shrub" => 30,
        n if n.ends_with("_leaves") || n.ends_with("_sapling") && n != "bamboo_sapling" => 30,
        _ => return None,
    })
}

pub fn level(state: i32) -> Option<u8> {
    let rule = blocks::orientation_of(state)?;
    if rule.name != "composter" { return None; }
    rule.value(state, "level")?.parse().ok()
}

fn set_level(world: &mut World, p: (i32, i32, i32), level: u8) {
    let state = blocks::orientation("composter").unwrap().state(&[("level", &level.to_string())]).unwrap();
    world.set_block(p.0, p.1, p.2, state);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Used { Added, Waiting, Emptied }

pub fn use_item(world: &mut World, p: (i32, i32, i32), item: Option<&str>) -> Option<Used> {
    let current = level(world.get_block(p.0, p.1, p.2))?;
    if current == 8 {
        set_level(world, p, 0);
        world.note_sound(p.0, p.1, p.2, "minecraft:block.composter.empty", 1.0, 1.0);
        return Some(Used::Emptied);
    }
    let probability = chance(item?)?;
    if current == 7 { return Some(Used::Waiting); }
    // Первый подходящий предмет всегда создаёт слой (Composter).
    let success = current == 0 || world.random_unit() < probability as f32 / 100.0;
    if success {
        set_level(world, p, current + 1);
        if current == 6 { world.schedule_block_once(p.0, p.1, p.2, 20); }
    }
    // Событие 1500 даёт частицы и соответствующий звук у клиента.
    world.note_level_event(p.0, p.1, p.2, 1500, i32::from(success));
    Some(Used::Added)
}

/// Воронка извлекает без звука и без сущности предмета.
pub fn take(world: &mut World, p: (i32, i32, i32)) {
    if level(world.get_block(p.0, p.1, p.2)) == Some(8) { set_level(world, p, 0); }
}

pub fn changed(world: &mut World, p: (i32, i32, i32)) {
    if level(world.get_block(p.0, p.1, p.2)) == Some(7) {
        world.schedule_block_once(p.0, p.1, p.2, 20);
    }
}

pub fn ticked(world: &mut World, p: (i32, i32, i32)) {
    if level(world.get_block(p.0, p.1, p.2)) == Some(7) {
        set_level(world, p, 8);
        world.note_sound(p.0, p.1, p.2, "minecraft:block.composter.ready", 1.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone;
    #[test]
    fn seven_items_mature_on_tick_twenty_and_empty_only_once() {
        let mut w = World::in_memory();
        let p = (0, 10, 0);
        set_level(&mut w, p, 0);
        for expected in 1..=7 {
            assert_eq!(use_item(&mut w, p, Some("pumpkin_pie")), Some(Used::Added));
            redstone::settle(&mut w);
            assert_eq!(level(w.get_block(0, 10, 0)), Some(expected));
        }
        assert_eq!(use_item(&mut w, p, Some("pumpkin_pie")), Some(Used::Waiting));
        for _ in 0..19 {
            for (p, _) in w.advance(1000) { redstone::ticked(&mut w, p); }
            redstone::settle(&mut w);
        }
        assert_eq!(level(w.get_block(0, 10, 0)), Some(7));
        for (p, _) in w.advance(1000) { redstone::ticked(&mut w, p); }
        assert_eq!(level(w.get_block(0, 10, 0)), Some(8));
        assert_eq!(crate::containers::comparator_reading(&w, p), Some(8));
        assert_eq!(use_item(&mut w, p, None), Some(Used::Emptied));
        assert_eq!(use_item(&mut w, p, None), None);
    }
    #[test]
    fn first_seed_is_guaranteed_and_invalid_items_leave_no_layer() {
        let mut w = World::in_memory();
        for _ in 0..30 {
            set_level(&mut w, (0, 10, 0), 0);
            assert_eq!(use_item(&mut w, (0, 10, 0), Some("wheat_seeds")), Some(Used::Added));
            assert_eq!(level(w.get_block(0, 10, 0)), Some(1));
        }
        for item in ["bamboo", "bamboo_sapling", "poisonous_potato", "beef", "stone"] {
            assert_eq!(chance(item), None);
            assert_eq!(use_item(&mut w, (0, 10, 0), Some(item)), None);
        }
        assert_eq!(chance("flowering_azalea_leaves"), Some(50));
        assert_eq!(chance("flowering_azalea"), Some(85));
    }
}
