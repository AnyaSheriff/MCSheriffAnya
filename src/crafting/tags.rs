// Дополнение потерянных тегов minecraft-data; minecraft.wiki, Planks:
// «any log or wood», Wool: «any color». Конкретные доски у дверей, лодок
// и лестниц не расширяются. [проверить] Полноту тегов — на оригинале.
use super::Recipe;
use crate::blocks;
use std::sync::OnceLock;

pub fn recipes() -> &'static [Recipe] {
    static RECIPES: OnceLock<Vec<Recipe>> = OnceLock::new();
    RECIPES.get_or_init(|| super::table::RECIPES.iter().map(|recipe| {
        let result = blocks::item_name(recipe.result).unwrap_or("");
        let cells: Vec<&'static [i32]> = recipe.cells.iter().map(|cell| {
            let mut ids = cell.to_vec();
            if let Some(wood) = result.strip_suffix("_planks") {
                for suffix in ["log", "wood", "stem", "hyphae"] {
                    for prefix in ["", "stripped_"] {
                        if let Some(item) = blocks::item_named(&format!("{prefix}{wood}_{suffix}")) { ids.push(item); }
                    }
                }
                if wood == "bamboo" { for n in ["bamboo_block", "stripped_bamboo_block"] { if let Some(item) = blocks::item_named(n) { ids.push(item); } } }
            } else if !recipe.shaped && recipe.cells.len() == 2 {
                let first = cell.first().and_then(|i| blocks::item_name(*i)).unwrap_or("");
                for suffix in ["_wool", "_carpet", "_bed"] {
                    if result.ends_with(suffix) && first.ends_with(suffix) && crate::sign::COLORS.iter().any(|c| result == format!("{c}{suffix}")) {
                        ids.extend(crate::sign::COLORS.iter().filter_map(|c| blocks::item_named(&format!("{c}{suffix}"))));
                    }
                }
            }
            ids.sort_unstable(); ids.dedup();
            if ids == *cell { *cell } else { Box::leak(ids.into_boxed_slice()) }
        }).collect();
        Recipe { cells: Box::leak(cells.into_boxed_slice()), ..recipe.clone() }
    }).collect())
}
