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
        world.set_pot(at, held.extras().pot_decorations.unwrap_or([blocks::item_named("brick").unwrap();4]));
    }
}
pub fn drops(sides: [i32;4], tool: Option<Stack>) -> Vec<Stack> {
    let shatter = tool.is_some_and(|s| s.extras().level("silk_touch") == 0 && blocks::item_name(s.item).is_some_and(|n|
        matches!(n,"trident" | "mace") || ["_pickaxe","_axe","_shovel","_hoe","_sword"].iter().any(|suffix| n.ends_with(suffix))));
    if shatter { sides.into_iter().map(|id| Stack::new(id,1)).collect() }
    else { vec![Stack::new(blocks::item_named("decorated_pot").unwrap(),1).with_extras(crate::stackdata::Data { pot_decorations: Some(sides), ..Default::default() })] }
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
    fn replacing_grass_does_not_overwrite_adjacent_pot() {
        let mut world = World::in_memory();
        world.set_block(0,70,0,blocks::state_by_name("short_grass").unwrap());
        world.set_block(1,70,0,blocks::state_by_name("decorated_pot").unwrap());
        assert_eq!(placement_at(&world,(0,70,0),5),(0,70,0));
        world.set_block(0,70,0,blocks::state_by_name("stone").unwrap());
        assert_eq!(placement_at(&world,(0,70,0),1),(0,71,0));
    }

}
