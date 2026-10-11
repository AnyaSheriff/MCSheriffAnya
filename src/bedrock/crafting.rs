// Динамические рецепты с явными ингредиентами для CraftingData.
// minecraft-data Bedrock 1.26.10: shaped/shapeless, ItemStackRequest.
// [проверить] Родные multi-recipe UUID и все сочетания фейерверков ещё
// требуют записи клиента; неизвестные UUID не выдумываются.
use crate::{blocks, crafting::Recipe};
use std::sync::OnceLock;

pub fn recipes() -> &'static [Recipe] {
    static RECIPES: OnceLock<Vec<Recipe>> = OnceLock::new();
    RECIPES.get_or_init(|| {
        let mut out = Vec::new();
        let mut add = |names: Vec<&str>, result: &str, count: i32, shaped: bool| {
            let cells: Vec<&'static [i32]> = names.into_iter().map(|n| {
                let ids: Vec<i32> = blocks::item_named(n).into_iter().collect();
                &*Box::leak(ids.into_boxed_slice())
            }).collect();
            if let Some(result) = blocks::item_named(result) { out.push(Recipe { shaped, width: if shaped { 3 } else { cells.len() as u8 }, height: if shaped { 3 } else { 1 }, result, count, cells: Box::leak(cells.into_boxed_slice()) }); }
        };
        for name in blocks::item_names() {
            let stack = crate::inventory::Stack::new(blocks::item_named(name).unwrap(), 1);
            if crate::combat::max_damage(&stack) > 0 { add(vec![name, name], name, 1, false); }
            if name.ends_with("_banner") { add(vec![name, name], name, 1, false); }
            for color in crate::sign::COLORS {
                let dye = format!("{color}_dye");
                if crate::containers::is_shulker_item(stack.item) { add(vec![name, &dye], &format!("{color}_shulker_box"), 1, false); }
                if crate::bundle::is_bundle(stack.item) { add(vec![name, &dye], &format!("{color}_bundle"), 1, false); }
                if name == "firework_star" { add(vec![name, &dye], name, 1, false); }
            }
        }
        for color in crate::sign::COLORS {
            let dye = format!("{color}_dye");
            add(vec!["gunpowder", &dye], "firework_star", 1, false);
            for shape in ["fire_charge", "gold_nugget", "feather", "creeper_head", "skeleton_skull", "wither_skeleton_skull", "player_head", "zombie_head", "dragon_head", "piglin_head"] {
                for trail in [false, true] { for twinkle in [false, true] {
                    let mut cells = vec!["gunpowder", &dye, shape]; if trail { cells.push("diamond"); }
                    if twinkle { cells.push("glowstone_dust"); }
                    add(cells, "firework_star", 1, false);
                } }
            }
            for (trail, twinkle) in [(true,false),(false,true),(true,true)] {
                let mut cells=vec!["gunpowder", &dye]; if trail {cells.push("diamond");}
                    if twinkle {cells.push("glowstone_dust");} add(cells,"firework_star",1,false);
            }
            for flower in ["dandelion", "poppy", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley", "wither_rose", "torchflower", "open_eyeblossom", "closed_eyeblossom"] {
                if color == "white" { add(vec!["bowl", "red_mushroom", "brown_mushroom", flower], "suspicious_stew", 1, false); }
            }
            add(vec!["shield", &format!("{color}_banner")], "shield", 1, false);
        }
        for gunpowder in 1..=3 { for stars in 0..=8-gunpowder {
            let mut names = vec!["paper"]; names.extend(std::iter::repeat_n("gunpowder", gunpowder)); names.extend(std::iter::repeat_n("firework_star", stars));
            add(names, "firework_rocket", 3, false);
        } }
        for copies in 1..=8 {
            let mut names=vec!["filled_map"]; names.extend(std::iter::repeat_n("map", copies)); add(names,"filled_map",copies as i32+1,false);
            let mut names=vec!["written_book"]; names.extend(std::iter::repeat_n("writable_book", copies)); add(names,"written_book",copies as i32,false);
        }
        add(vec!["arrow","arrow","arrow","arrow","lingering_potion","arrow","arrow","arrow","arrow"], "tipped_arrow",8,true);
        out
    })
}
