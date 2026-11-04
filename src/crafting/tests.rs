use super::*;
use crate::{blocks, inventory::Inventory, stackdata::Data};
fn s(name: &str, count: i32) -> Stack { Stack::new(blocks::item_named(name).unwrap(), count) }
fn grid(names: &[&str]) -> Vec<Option<Stack>> { names.iter().map(|n| (!n.is_empty()).then(|| s(n,1))).collect() }
fn result(names: &[&str]) -> Stack { craft(&player::padded(&grid(names))).expect("рецепт") }

#[test]
fn all_log_forms_and_colored_inputs_match_without_broadening_specific_wood() {
    for wood in ["oak","spruce","birch","jungle","acacia","dark_oak","mangrove","cherry","pale_oak","crimson","warped"] {
        let (a,b) = if matches!(wood,"crimson"|"warped") { ("stem","hyphae") } else { ("log","wood") };
        for suffix in [a,b] { for prefix in ["","stripped_"] {
            assert_eq!(result(&[&format!("{prefix}{wood}_{suffix}"),"","",""]).item, s(&format!("{wood}_planks"),4).item);
        } }
    }
    for color in crate::sign::COLORS {
        assert_eq!(result(&[&format!("{color}_wool"), "red_dye","",""]).item, s("red_wool",1).item);
    }
    assert!(craft(&player::padded(&grid(&["oak_planks","spruce_planks","oak_planks","spruce_planks","oak_planks","spruce_planks","","",""]))).is_none());
}
#[test]
fn inventory_shape_padding_shift_order_and_full_inventory_are_atomic() {
    let mut slots = grid(&["oak_log","","",""]); slots[0] = Some(s("oak_log",17));
    let mut bag = Inventory::new(); let mut dropped = Vec::new();
    for _ in 0..17 { let r = craft(&player::padded(&slots)).unwrap(); assert!(player::take(&mut slots,&mut bag,r,1,0,&mut dropped)); }
    assert_eq!(bag.slot(44),Some(s("oak_planks",64))); assert_eq!(bag.slot(43),Some(s("oak_planks",4))); assert!(dropped.is_empty());
    for slot in 9..=44 { bag.set(slot,Some(s("stone",64))); }
    slots[0] = Some(s("oak_log",1)); let before = bag.clone();
    assert!(!player::take(&mut slots,&mut bag,s("oak_planks",4),1,0,&mut dropped));
    assert_eq!(bag,before); assert_eq!(slots[0],Some(s("oak_log",1)));
    assert_eq!(craft(&player::padded(&grid(&["oak_planks","oak_planks","oak_planks","oak_planks"]))).unwrap(),s("crafting_table",1));
}
#[test]
fn right_click_takes_whole_result_and_cursor_cannot_insert_into_result() {
    let mut grid = grid(&["oak_log","","",""]); let mut bag=Inventory::new();let mut dropped=Vec::new();
    assert!(player::take(&mut grid,&mut bag,s("oak_planks",4),0,1,&mut dropped));
    assert_eq!(bag.cursor(),Some(s("oak_planks",4))); assert!(grid.iter().all(Option::is_none));
    grid[0]=Some(s("oak_log",1)); bag.set_cursor(Some(s("stone",1)));
    assert!(!player::take(&mut grid,&mut bag,s("oak_planks",4),0,0,&mut dropped));assert!(grid[0].is_some());
}
#[test]
fn cake_returns_three_buckets_and_close_does_not_return_preview() {
    let mut grid=grid(&["milk_bucket","milk_bucket","milk_bucket","sugar","egg","sugar","wheat","wheat","wheat"]);
    let mut bag=Inventory::new();let mut dropped=Vec::new();let r=craft(&grid).unwrap();
    assert!(player::take(&mut grid,&mut bag,r,0,0,&mut dropped));
    assert_eq!(&grid[..3],&[Some(s("bucket",1));3]);assert!(grid[3..].iter().all(Option::is_none));
    bag.set_cursor(None);bag.set(1,Some(s("oak_log",1)));bag.set(0,Some(s("oak_planks",4)));
    bag.close_window();assert!(bag.slot(0).is_none());assert_eq!(bag.slot(36),Some(s("oak_log",1)));
}
#[test]
fn dynamic_components_survive_storage() {
    let mut armor=s("leather_chestplate",1).with_extras(Data{damage:12,name:Some("keep".into()),..Default::default()});
    let dyed=craft(&[Some(armor),Some(s("red_dye",1))]).unwrap();
    assert_eq!(dyed.extras().dyed_color,Some(special::DYE_COLORS[14]));assert_eq!(dyed.extras().damage,12);assert_eq!(dyed.extras().name.as_deref(),Some("keep"));
    armor=dyed; let mixed=craft(&[Some(armor),Some(s("blue_dye",1))]).unwrap(); assert_ne!(mixed.extras().dyed_color,dyed.extras().dyed_color);
    assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&mixed.extras())),Some(mixed.extras()));
    let pot=result(&["","angler_pottery_sherd","","brick","","archer_pottery_sherd","","arms_up_pottery_sherd",""]);
    assert_eq!(pot.extras().pot_decorations,Some([s("angler_pottery_sherd",1).item,s("brick",1).item,s("archer_pottery_sherd",1).item,s("arms_up_pottery_sherd",1).item]));
    assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&pot.extras())),Some(pot.extras()));
}
#[test]
fn firework_shape_fade_flight_and_rejections() {
    let star=result(&["gunpowder","red_dye","blue_dye","feather","diamond","glowstone_dust","","",""]);
    let e=star.extras().explosion.unwrap();assert_eq!(e.shape,4);assert_eq!(e.colors.len(),2);assert!(e.trail && e.twinkle);
    let faded=craft(&[Some(star),Some(s("white_dye",1))]).unwrap();assert_eq!(faded.extras().explosion.as_ref().unwrap().fade_colors.len(),1);
    let rocket=craft(&[Some(s("paper",1)),Some(s("gunpowder",1)),Some(s("gunpowder",1)),Some(faded)]).unwrap();assert_eq!(rocket.count,3);assert_eq!(rocket.extras().rocket().flight,2);assert_eq!(rocket.extras().rocket().explosions.len(),1);
    assert!(craft(&grid(&["gunpowder","red_dye","feather","gold_nugget"])).is_none());
    assert!(craft(&grid(&["paper","gunpowder","gunpowder","gunpowder","gunpowder"])).is_none());
}
#[test]
fn map_book_banner_and_container_copies_keep_data_and_originals() {
    let map=s("filled_map",1).with_extras(Data{map_id:Some(42),name:Some("map".into()),..Default::default()});
    assert_eq!(craft(&[Some(map),Some(s("map",1)),Some(s("map",1))]),Some(map.with_count(3)));
    let original=s("written_book",1).with_extras(Data{book:Some(crate::book::Book{pages:vec!["words".into()],title:Some("title".into()),author:"author".into(),generation:0,resolved:false}),..Default::default()});
    let mut grid=vec![Some(original),Some(s("writable_book",1)),None,None];let copy=craft(&player::padded(&grid)).unwrap();
    assert_eq!(copy.extras().book.as_ref().unwrap().generation,1);
    let mut bag=Inventory::new();assert!(player::take(&mut grid,&mut bag,copy,0,0,&mut Vec::new()));assert_eq!(grid[0],Some(original));assert!(grid[1].is_none());
    let second=craft(&[Some(copy),Some(s("writable_book",1))]).unwrap();assert_eq!(second.extras().book.as_ref().unwrap().generation,2);
    assert!(craft(&[Some(second),Some(s("writable_book",1))]).is_none());
    let banner=s("red_banner",1).with_extras(Data{banner_patterns:vec![(1,3)],..Default::default()});
    assert_eq!(craft(&[Some(banner),Some(s("red_banner",1))]),Some(banner));assert_eq!(remainders(&[Some(banner),Some(s("red_banner",1))]),vec![Some(banner),None]);
    let box_item=s("shulker_box",1).with_extras(Data{name:Some("kept".into()),contents:vec![crate::stackdata::Content{slot:0,item:s("diamond",1).item,count:3,data:None}],..Default::default()});
    let colored=craft(&[Some(box_item),Some(s("red_dye",1))]).unwrap();assert_eq!(colored.item,s("red_shulker_box",1).item);assert_eq!(colored.extras(),box_item.extras());
}
#[test]
fn repair_bonus_removes_enchants_but_keeps_curses_and_arrows_keep_potion() {
    let mut data=Data{damage:200,..Default::default()};data.set_enchant(crate::stackdata::enchantment_number("sharpness").unwrap(),3);data.set_enchant(crate::stackdata::enchantment_number("vanishing_curse").unwrap(),1);
    let tool=s("iron_sword",1).with_extras(data);let repaired=craft(&[Some(tool),Some(tool)]).unwrap();
    assert_eq!(repaired.extras().damage,138);assert_eq!(repaired.extras().level("sharpness"),0);assert_eq!(repaired.extras().level("vanishing_curse"),1);
    let mut grid=vec![Some(s("arrow",1));9];grid[4]=Some(s("lingering_potion",1).with_extras(Data{potion:Some(8),..Default::default()}));
    let arrows=craft(&grid).unwrap();assert_eq!(arrows.count,8);assert_eq!(arrows.extras().potion,Some(8));assert!(remainders(&grid)[4].is_none());
}
#[test]
fn recipe_book_fill_is_transactional_and_increments_stacks() {
    let recipe=recipes().iter().find(|r| r.result == s("crafting_table",1).item).unwrap();
    let mut bag=Inventory::new();bag.set(36,Some(s("spruce_planks",12)));let mut grid=vec![None;4];
    assert!(player::place(recipe,&mut grid,&mut bag,false));assert_eq!(grid,vec![Some(s("spruce_planks",1));4]);
    assert!(player::place(recipe,&mut grid,&mut bag,true));assert_eq!(grid,vec![Some(s("spruce_planks",3));4]);assert!(bag.slot(36).is_none());
    for slot in 9..=44 {bag.set(slot,Some(s("stone",64)));}let before=bag.clone();let grid_before=grid.clone();
    assert!(!player::place(recipe,&mut grid,&mut bag,true));assert_eq!(bag,before);assert_eq!(grid,grid_before);
}

#[test]
fn tag_batches_can_mix_ingredient_types_between_crafts() {
    let recipe=recipes().iter().find(|r| r.result==s("stick",1).item && r.cells.len()==2 && r.cells[0].contains(&s("oak_planks",1).item)).unwrap();
    assert!(recipe.covered_counts(&[(s("oak_planks",1).item,3),(s("spruce_planks",1).item,1)],2));
    assert!(!recipe.covered_counts(&[],1));
    assert!(!recipe.covered_counts(&[(s("oak_planks",1).item,4)],0));
    assert!(!recipe.covered_counts(&[(s("oak_planks",1).item,3),(s("stone",1).item,1)],2));
}

#[test]
fn previews_are_not_saved_and_bundle_coloring_retains_contents() {
    let mut bag=Inventory::new();bag.set(0,Some(s("diamond",64)));bag.set(1,Some(s("oak_log",1)));
    let saved=Inventory::from_bytes(&bag.to_bytes()).unwrap();assert!(saved.slot(0).is_none());assert_eq!(saved.slot(1),bag.slot(1));
    let bundle=s("bundle",1).with_extras(Data{bundle:vec![crate::stackdata::Content{slot:0,item:s("diamond",1).item,count:8,data:None}],..Default::default()});
    let dyed=craft(&[Some(bundle),Some(s("blue_dye",1))]).unwrap();assert_eq!(dyed.item,s("blue_bundle",1).item);assert_eq!(dyed.extras(),bundle.extras());
}

#[test]
fn banners_with_more_than_six_patterns_cannot_be_duplicated() {
    let ominous=s("white_banner",1).with_extras(Data{ominous_banner:true,..Default::default()});
    assert!(craft(&[Some(ominous),Some(s("white_banner",1))]).is_none());
    let layered=s("red_banner",1).with_extras(Data{banner_patterns:vec![(1,3);7],..Default::default()});
    assert!(craft(&[Some(layered),Some(s("red_banner",1))]).is_none());
}
