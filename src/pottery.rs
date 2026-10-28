// Вики, Java Edition 1.20: «Drops itself when broken using an empty hand»;
// «Breaks apart into its crafting materials». Шёрды идут back, left, right, front.
use crate::{blocks, inventory::Stack, world::World};
pub fn placement_at(world: &World, clicked: (i32,i32,i32), face: i32) -> (i32,i32,i32) {
    let state = world.get_block(clicked.0,clicked.1,clicked.2);
    if state != crate::world::AIR && crate::fluids::fluid_at(state).is_none() && crate::placing::is_replaceable(state) { return clicked; }
    let (x,y,z) = clicked;
    match face { 0=>(x,y-1,z),1=>(x,y+1,z),2=>(x,y,z-1),3=>(x,y,z+1),4=>(x-1,y,z),5=>(x+1,y,z),_=>clicked }
}
pub fn place(world: &mut World, at: (i32,i32,i32), held: Stack) {
    if blocks::item_name(held.item) == Some("decorated_pot") {
        let data = held.extras();
        let blank = if world.catalog().changed_rules() { 0 } else { blocks::item_named("brick").unwrap() };
        let sides = data.pot_decorations.unwrap_or([blank;4]);
        if world.catalog().changed_rules() { world.set_pot_with_details(at, sides, data); }
        else { world.set_pot(at, sides); }
    }
}
pub fn drops(sides: [i32;4], tool: Option<Stack>) -> Vec<Stack> {
    let shatter = tool.is_some_and(|s| s.extras().level("silk_touch") == 0 && blocks::item_name(s.item).is_some_and(|n|
        matches!(n,"trident" | "mace") || ["_pickaxe","_axe","_shovel","_hoe","_sword"].iter().any(|suffix| n.ends_with(suffix))));
    if shatter { sides.into_iter().filter(|id| *id != 0).map(|id| Stack::new(id,1)).collect() }
    else { vec![Stack::new(blocks::item_named("decorated_pot").unwrap(),1).with_extras(crate::stackdata::Data { pot_decorations: Some(sides), ..Default::default() })] }
}
/// Полные стопки сторон хранятся отдельно от старого массива номеров.
pub fn drops_with_data(sides: [i32;4], data: crate::stackdata::Data, tool: Option<Stack>) -> Vec<Stack> {
    let mut result = drops(sides, tool);
    if result.first().is_some_and(|s| blocks::item_name(s.item) == Some("decorated_pot")) {
        let mut extra = result[0].extras(); extra.pot_sides = data.pot_sides;
        result[0] = result[0].with_extras(extra);
    } else {
        result = sides.into_iter().enumerate().filter_map(|(i,id)| {
            if id == 0 { return None; }
            let side = data.pot_sides.iter().find(|c| c.slot as usize == i);
            Some(side.map(|c| Stack::from_stored(c.item, 1).with_extras(c.data.map(crate::stackdata::get).unwrap_or_default())).unwrap_or_else(|| Stack::new(id,1)))
        }).collect();
    }
    result
}

pub fn apply_snbt(world: &mut World, at: (i32,i32,i32), text: &str) {
    if !world.catalog().changed_rules() { return; }
    let Some(root) = crate::snbt::parse(text) else { return; };
    // Чёрный ящик 26.3: sherds — именованные стороны, не прежний список.
    // Ошибка одной стороны не убирает остальные; неизвестные ключи игнорируются.
    // testdata/pottery-26.3-observations.json: "Not a map" для старого списка.
    let Some(crate::snbt::Value::Map(sides)) = root.get("sherds") else { return; };
    let mut ids = [0;4];
    let mut data = crate::stackdata::Data::default();
    for (i, name) in ["back", "left", "right", "front"].iter().enumerate() {
        let Some((_, side)) = sides.iter().find(|(key, _)| key.as_str() == *name) else { continue; };
        let stack = side.as_str().and_then(|n|blocks::item_named(n.trim_start_matches("minecraft:"))).map(|id|Stack::new(id,1)).or_else(||crate::stackdata::stack_from_value(side).ok());
        if let Some(stack) = stack {
            ids[i] = stack.item;
            data.pot_sides.push(crate::stackdata::Content { slot:i as u8,item:stack.stored_item(),count:1,data:stack.data });
        }
    }
    world.set_pot_with_details(at,ids,data);
}

/// Сетевые наблюдения 777: стороны — именованные шаблоны предметов.
/// Это представление блока: полные довески остаются в мире и в выпавшей вещи.
/// Здесь передаём имена сторон; остальные довески для отображения [проверить].
pub fn preview_nbt(sides: [i32;4], data: &crate::stackdata::Data) -> Vec<u8> {
    use crate::commands_admin::Tag;
    let mut fields = Vec::new();
    for (i, side) in ["back", "left", "right", "front"].iter().enumerate() {
        if sides[i] == 0 { continue; }
        let Some(name) = blocks::item_name(sides[i]) else { continue; };
        let mut item = vec![("id".into(), Tag::Str(format!("minecraft:{name}")))];
        if let Some(extra) = data.pot_sides.iter().find(|c| c.slot as usize == i
            && Stack::from_stored(c.item, 1).item == sides[i]).and_then(|c| c.data).map(crate::stackdata::get)
            && let Some(name) = extra.name
        {
            item.push(("components".into(), Tag::Compound(vec![("minecraft:custom_name".into(), Tag::Str(name))])));
        }
        fields.push(((*side).into(), Tag::Compound(item)));
    }
    Tag::Compound(if fields.is_empty() { Vec::new() } else { vec![("sherds".into(), Tag::Compound(fields))] }).encode()
}

pub fn nbt(sides: [i32;4]) -> Vec<u8> {
    let mut out = vec![10,9,0,6]; out.extend_from_slice(b"sherds"); out.push(8); out.extend_from_slice(&4i32.to_be_bytes());
    for item in sides {
        let name = format!("minecraft:{}",blocks::item_name(item).unwrap_or("brick"));
        out.extend_from_slice(&(name.len() as u16).to_be_bytes()); out.extend_from_slice(name.as_bytes());
    }
    out.push(0); out
}
#[cfg(test)]
mod tests {
    use super::*;
    fn observed_nbt(fixture: &str, case: &str, packet: &str) -> Vec<u8> {
        let data: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let hex = data["observations"].as_array().unwrap().iter()
            .find(|r| r["case"] == case && r["packet"] == packet).unwrap()["nbt_hex"].as_str().unwrap();
        hex.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
    }

    #[test]
    fn preview_world_emits_observed_side_maps_atomically_and_retains_details() {
        let fixture = include_str!("../tools/protocol-bridge/testdata/decorated-pot-wire-stone-777.json");
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        for (case, spec) in [
            ("named_front_brick", r#"{sherds:{front:{id:"minecraft:brick",components:{"minecraft:custom_name":"wire-side"}}}}"#),
            ("front_stone", r#"{sherds:{front:"minecraft:stone"}}"#),
        ] {
            let mut world = World::in_memory_with_catalog(catalog);
            let at = (-17, 65, -1);
            world.set_block(at.0, at.1, at.2, blocks::state_by_name("decorated_pot").unwrap());
            let (_, start) = world.events_since(0, 0);
            apply_snbt(&mut world, at, spec);
            let (events, _) = world.events_since(start, 0);
            let updates: Vec<_> = events.iter().filter_map(|e| match e {
                crate::world::WorldEvent::BlockEntity { x, y, z, kind, nbt } => Some(((*x, *y, *z), *kind, nbt.to_vec())),
                _ => None,
            }).collect();
            assert_eq!(updates, vec![(at, 42, observed_nbt(fixture, case, "tile_entity_data"))]);
            assert_eq!(world.chunk_pots(-2, -1), vec![((15, 65, 15), observed_nbt(fixture, case, "map_chunk"))]);
            let details = world.pot_details(at);
            let saved = crate::stackdata::to_bytes(&details);
            assert_eq!(crate::stackdata::from_bytes(&saved), Some(details));
        }
    }

    #[test]
    fn preview_empty_pot_has_the_observed_initial_and_incremental_empty_tags() {
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        let mut world = World::in_memory_with_catalog(catalog);
        let at = (1, 65, 1);
        world.set_block(at.0, at.1, at.2, blocks::state_by_name("decorated_pot").unwrap());
        apply_snbt(&mut world, at, "{sherds:{}}");
        assert_eq!(preview_nbt([0; 4], &world.pot_details(at)), [10, 0]);
        assert_eq!(world.chunk_pots(0, 0), vec![(at, vec![0])]);
        let fixture = include_str!("../tools/protocol-bridge/testdata/decorated-pot-wire-777.json");
        assert_eq!(observed_nbt(fixture, "blank", "map_chunk"), [0]);
        assert_eq!(observed_nbt(fixture, "blank", "tile_entity_data"), [10, 0]);
    }

    #[test]
    fn preview_import_and_drops_match_fifteen_official_observations() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/pottery-26.3-observations.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 15);
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        let state = blocks::state_by_name("decorated_pot").unwrap();
        let tool = Some(Stack::new(blocks::item_named("diamond_pickaxe").unwrap(), 1));
        for case in cases {
            let mut world = World::in_memory_with_catalog(catalog);
            let at = (1, 70, 1);
            world.set_block(at.0, at.1, at.2, state);
            apply_snbt(&mut world, at, case["input_snbt"].as_str().unwrap());
            let expected = case["expected_sides"].as_array().unwrap();
            let ids: [i32; 4] = std::array::from_fn(|i| expected[i]["id"].as_str()
                .map(|name| blocks::item_named(name.trim_start_matches("minecraft:")).unwrap()).unwrap_or(0));
            let sides = world.pot(at).unwrap();
            assert_eq!(sides, ids, "{}", case["id"]);
            let data = world.pot_details(at);
            for (i, side) in expected.iter().enumerate() {
                let actual = data.pot_sides.iter().find(|c| c.slot as usize == i);
                assert_eq!(actual.is_some(), !side.is_null(), "{} side {i}", case["id"]);
                if let Some(actual) = actual {
                    let item = Stack::from_stored(actual.item, 1);
                    assert_eq!(item.item, ids[i]);
                    let extra = actual.data.map(crate::stackdata::get).unwrap_or_default();
                    assert_eq!(extra.name.as_deref(), side["name"].as_str(), "{} side {i}", case["id"]);
                }
            }
            let restored = crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)).unwrap();
            assert_eq!(restored, data, "{} saved details", case["id"]);
            let pot = drops_with_data(sides, restored, None)[0];
            assert_eq!(pot.extras().pot_decorations, Some(ids));
            assert_eq!(pot.extras().pot_sides, data.pot_sides);
            let moved = (-17, 70, -1);
            world.set_block(moved.0, moved.1, moved.2, state);
            place(&mut world, moved, pot);
            assert_eq!(world.pot(moved), Some(ids));
            assert_eq!(world.pot_details(moved).pot_sides, data.pot_sides);
            let shattered = drops_with_data(ids, world.pot_details(moved), tool);
            assert_eq!(shattered.iter().map(|s| s.item).collect::<Vec<_>>(), ids.into_iter().filter(|id| *id != 0).collect::<Vec<_>>());
            let names: Vec<_> = shattered.iter().map(|s| s.extras().name).collect();
            assert_eq!(names, expected.iter().filter(|s| !s.is_null()).map(|s| s["name"].as_str().map(str::to_owned)).collect::<Vec<_>>());
        }
    }

    #[test]
    fn new_command_format_does_not_change_the_legacy_primary_profile() {
        let mut world = World::in_memory();
        let at = (0, 70, 0);
        world.set_block(at.0, at.1, at.2, blocks::state_by_name("decorated_pot").unwrap());
        let before = world.pot(at);
        apply_snbt(&mut world, at, r#"{sherds:{front:"minecraft:archer_pottery_sherd"}}"#);
        assert_eq!(world.pot(at), before);
        assert!(world.pot_details(at).pot_sides.is_empty());
    }

    #[test]
    fn placed_pot_preserves_materials_and_silk_touch() {
        let sides = ["archer_pottery_sherd","brick","arms_up_pottery_sherd","prize_pottery_sherd"].map(|n| blocks::item_named(n).unwrap());
        let stack = drops(sides,None)[0]; let mut world = World::in_memory();
        world.set_block(0,70,0,blocks::state_by_name("decorated_pot").unwrap()); place(&mut world,(0,70,0),stack);
        assert_eq!(world.pot((0,70,0)),Some(sides));
        let tool = Stack::new(blocks::item_named("diamond_pickaxe").unwrap(),1);
        assert_eq!(drops(sides,Some(tool)).iter().map(|s|s.item).collect::<Vec<_>>(),sides);
        let mut data = tool.extras(); data.set_enchant(crate::stackdata::enchantment_number("silk_touch").unwrap(),1);
        assert_eq!(drops(sides,Some(tool.with_extras(data))),vec![stack]);
        world.set_block(0,70,0,0); assert_eq!(world.pot((0,70,0)),None);
    }
    #[test]
    fn preview_sides_keep_data_through_item_and_block_and_empty_sides_do_not_shatter() {
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        let mut world = World::in_memory_with_catalog(catalog);
        let at = (1,70,1);
        world.set_block(at.0,at.1,at.2,blocks::state_by_name("decorated_pot").unwrap());
        apply_snbt(&mut world,at,r#"{sherds:{back:{id:"minecraft:brick",components:{"minecraft:custom_name":"side"}}}}"#);
        let sides = world.pot(at).unwrap();
        assert_eq!(sides[1..],[0;3]);
        let data = world.pot_details(at);
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data.clone()));
        let tool = Some(Stack::new(blocks::item_named("diamond_pickaxe").unwrap(),1));
        let parts = drops_with_data(sides,data.clone(),tool);
        assert_eq!(parts.len(),1); assert_eq!(parts[0].extras().name.as_deref(),Some("side"));
        let pot = drops_with_data(sides,data,None)[0];
        world.set_block(2,70,1,blocks::state_by_name("decorated_pot").unwrap());
        place(&mut world,(2,70,1),pot);
        assert_eq!(drops_with_data(world.pot((2,70,1)).unwrap(),world.pot_details((2,70,1)),tool),parts);
        assert!(drops([0;4],tool).is_empty());
    }

    #[test]
    fn replacing_grass_does_not_overwrite_adjacent_pot() {
        let mut world = World::in_memory();
        world.set_block(0,70,0,blocks::state_by_name("short_grass").unwrap());
        world.set_block(1,70,0,blocks::state_by_name("decorated_pot").unwrap());
        assert_eq!(placement_at(&world,(0,70,0),5),(0,70,0));
        world.set_block(0,70,0,blocks::state_by_name("stone").unwrap());
        assert_eq!(placement_at(&world,(0,70,0),1),(0,71,0));
    }

}
