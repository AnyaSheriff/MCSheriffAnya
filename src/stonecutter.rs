// Minecraft Wiki, Stonecutter: «one ingredient»; stairs 1, slabs 2.
// Таблица семейств из раздела Cutting, предметы из наших таблиц.
// [проверить] порядок кнопок и полный набор рецептов записью vanilla.
use std::sync::OnceLock;
use crate::{blocks, inventory::Stack};
#[derive(Clone, Copy, Debug)]
pub struct Recipe { pub input: i32, pub output: i32, pub count: i32 }
pub fn recipes() -> &'static [Recipe] {
    static ALL: OnceLock<Vec<Recipe>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all = Vec::new();
        let mut add = |input: &str, output: &str, count| {
            if let (Some(input),Some(output)) = (blocks::item_named(input), blocks::item_named(output)) {
                all.push(Recipe {input,output,count});
            }
        };
        for (input, families) in [
            ("stone", "stone stone_brick"), ("stone_bricks", "stone_brick"),
            ("cobblestone", "cobblestone"), ("mossy_cobblestone", "mossy_cobblestone"),
            ("mossy_stone_bricks", "mossy_stone_brick"),
            ("andesite", "andesite polished_andesite"), ("polished_andesite", "polished_andesite"),
            ("diorite", "diorite polished_diorite"), ("polished_diorite", "polished_diorite"),
            ("granite", "granite polished_granite"), ("polished_granite", "polished_granite"),
            ("sandstone", "sandstone cut_sandstone"), ("cut_sandstone", "cut_sandstone"), ("smooth_sandstone", "smooth_sandstone"),
            ("red_sandstone", "red_sandstone cut_red_sandstone"), ("cut_red_sandstone", "cut_red_sandstone"), ("smooth_red_sandstone", "smooth_red_sandstone"),
            ("bricks", "brick"), ("mud_bricks", "mud_brick"), ("resin_bricks", "resin_brick"),
            ("nether_bricks", "nether_brick"), ("red_nether_bricks", "red_nether_brick"),
            ("quartz_block", "quartz"), ("smooth_quartz", "smooth_quartz"),
            ("purpur_block", "purpur"), ("purpur_pillar", "purpur"), ("smooth_stone", "smooth_stone"),
            ("prismarine", "prismarine"), ("prismarine_bricks", "prismarine_brick"), ("dark_prismarine", "dark_prismarine"),
            ("end_stone", "end_stone_brick"), ("end_stone_bricks", "end_stone_brick"),
            ("blackstone", "blackstone polished_blackstone polished_blackstone_brick"),
            ("polished_blackstone", "polished_blackstone polished_blackstone_brick"), ("polished_blackstone_bricks", "polished_blackstone_brick"),
            ("cobbled_deepslate", "cobbled_deepslate polished_deepslate deepslate_brick deepslate_tile"),
            ("polished_deepslate", "polished_deepslate deepslate_brick deepslate_tile"),
            ("deepslate_bricks", "deepslate_brick deepslate_tile"), ("deepslate_tiles", "deepslate_tile"),
            ("tuff", "tuff polished_tuff tuff_brick"), ("polished_tuff", "polished_tuff tuff_brick"), ("tuff_bricks", "tuff_brick"),
        ] {
            for family in families.split_whitespace() {
                for (suffix,count) in [("stairs",1),("slab",2),("wall",1)] { add(input,&format!("{family}_{suffix}"),count); }
            }
        }
        for (input, outputs) in [
            ("stone","stone_bricks chiseled_stone_bricks"),("stone_bricks","chiseled_stone_bricks"),
            ("andesite","polished_andesite"),("diorite","polished_diorite"),("granite","polished_granite"),
            ("sandstone","cut_sandstone chiseled_sandstone"),("red_sandstone","cut_red_sandstone chiseled_red_sandstone"),
            ("quartz_block","quartz_pillar quartz_bricks chiseled_quartz_block"),("end_stone","end_stone_bricks"),
            ("nether_bricks","chiseled_nether_bricks"),("resin_bricks","chiseled_resin_bricks"),
            ("blackstone","polished_blackstone polished_blackstone_bricks chiseled_polished_blackstone"),
            ("polished_blackstone","polished_blackstone_bricks chiseled_polished_blackstone"),
            ("cobbled_deepslate","polished_deepslate deepslate_bricks deepslate_tiles chiseled_deepslate"),
            ("polished_deepslate","deepslate_bricks deepslate_tiles"),("deepslate_bricks","deepslate_tiles"),
            ("tuff","polished_tuff tuff_bricks chiseled_tuff chiseled_tuff_bricks"),
            ("polished_tuff","tuff_bricks chiseled_tuff_bricks"),("tuff_bricks","chiseled_tuff_bricks"),
        ] { for output in outputs.split_whitespace() { add(input,output,1); } }
        for wax in ["","waxed_"] { for age in ["","exposed_","weathered_","oxidized_"] {
            let block = format!("{wax}{age}{}",if age.is_empty() {"copper_block"} else {"copper"});
            let cut = format!("{wax}{age}cut_copper");
            add(&block,&cut,4);
            for (suffix,count) in [("stairs",4),("slab",8)] { add(&block,&format!("{cut}_{suffix}"),count); add(&cut,&format!("{cut}_{suffix}"),count/4); }
            add(&block,&format!("{wax}{age}chiseled_copper"),4);
            add(&cut,&format!("{wax}{age}chiseled_copper"),1);
            add(&block,&format!("{wax}{age}copper_grate"),4);
        } }
        all.sort_by_key(|r| (blocks::item_name(r.output).unwrap_or(""),r.input));
        all.dedup_by_key(|r| (r.input,r.output));
        all
    })
}
pub fn choices(input: i32) -> Vec<&'static Recipe> { recipes().iter().filter(|r| r.input == input).collect() }
pub fn result(input: Option<Stack>, selected: i32) -> Option<Stack> {
    let recipe = *choices(input?.item).get(usize::try_from(selected).ok()?)?;
    Some(Stack::new(recipe.output,recipe.count))
}
// Update Recipes, minecraft-data 26.1 protocol.json: IDSet, SlotDisplay.
pub fn recipe_packet() -> Vec<u8> {
    use crate::network::varint::encode_varint;
    let mut out=encode_varint(0x85); out.extend(encode_varint(0));
    out.extend(encode_varint(recipes().len() as i32));
    for r in recipes() {
        for v in [2,r.input,5,r.output,r.count,0,0] { out.extend(encode_varint(v)); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cutting_yields_and_invalid_inputs() {
        for (input, output, count) in [("stone","stone_slab",2),("stone","stone_stairs",1),("copper_block","cut_copper_slab",8)] {
            let input=blocks::item_named(input).unwrap(); let output=blocks::item_named(output).unwrap();
            assert!(choices(input).iter().any(|r|r.output==output&&r.count==count));
        }
        assert!(choices(blocks::item_named("oak_planks").unwrap()).is_empty());
        assert!(result(None,0).is_none());
        assert!(recipes().iter().all(|r|r.count>0));
    }
}

