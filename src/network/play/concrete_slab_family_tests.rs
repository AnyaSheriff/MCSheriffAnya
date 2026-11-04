//! Все цвета проходят реальные обработчики; внутренние ID не заменяются белыми.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile, SlabFamily, SlabHalf, slabs};

fn setup(profile: Profile) -> (Shared, PlayerState) {
    let catalog = Catalog::new(profile);
    let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/concrete-family-ops.json")), crate::skins::Settings::default());
    let mut player = PlayerState::new_for_catalog([241; 16], "ConcreteFamily".into(), None, None, 0, 241,
        (3.5, 64.0, 5.5), catalog);
    player.game_mode = GAME_MODE_SURVIVAL;
    player.on_ground = true;
    {
        let mut world = shared.world_in(player.dimension).lock().unwrap();
        world.ensure(0, 0);
        world.set_block_silently(3, 63, 3, blocks::state_by_name("stone").unwrap());
    }
    (shared, player)
}

fn click(at: (i32, i32, i32), face: i32) -> Vec<u8> {
    let mut bytes = encode_varint(0);
    push_packed_position(&mut bytes, at.0, at.1, at.2);
    bytes.extend(encode_varint(face));
    for _ in 0..3 { bytes.extend(0.5f32.to_be_bytes()); }
    bytes.push(0); bytes.extend(encode_varint(71)); bytes
}
fn dig(status: i32) -> Vec<u8> {
    let mut bytes = encode_varint(status);
    push_packed_position(&mut bytes, 3, 64, 3);
    bytes.push(1); bytes.extend(encode_varint(72)); bytes
}
fn target(shared: &Shared, player: &PlayerState) -> i32 {
    shared.world_in(player.dimension).lock().unwrap().get_block(3, 64, 3)
}

#[test]
fn every_color_places_dry_or_waterlogged_bottom_then_merges_spending_exactly_two() {
    for row in slabs() {
        for water in [false, true] {
            let (shared, mut player) = setup(Profile::Preview26_3);
            if water {
                shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3,
                    crate::fluids::state_of(crate::fluids::Kind::Water, crate::fluids::SOURCE));
            }
            let slot = player.inventory.selected_slot();
            let stack = Stack::new(row.item, 2);
            player.inventory.set(slot, Some(stack));
            let first = place_block(&click((3, 63, 3), 1), &mut player, &shared).unwrap();
            assert!(first.slots.contains(&slot), "{} wet={water}", row.name);
            assert_eq!(player.inventory.slot(slot), Some(stack.with_count(1)));
            assert_eq!(target(&shared, &player), shared.catalog().slab_state_for(row, SlabHalf::Bottom, water).unwrap(), "{}", row.name);
            let merged = place_block(&click((3, 64, 3), 1), &mut player, &shared).unwrap();
            assert!(merged.slots.contains(&slot));
            assert!(player.inventory.slot(slot).is_none());
            assert_eq!(target(&shared, &player), shared.catalog().slab_state_for(row, SlabHalf::Double, false).unwrap(), "{}", row.name);
            assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
        }
    }
}

#[test]
fn every_color_places_top_and_never_merges_into_another_color_or_native_profile() {
    let colors: Vec<_> = slabs().collect();
    for (index, row) in colors.iter().enumerate() {
        let (shared, mut player) = setup(Profile::Preview26_3);
        let slot = player.inventory.selected_slot();
        player.inventory.set(slot, Some(Stack::new(row.item, 2)));
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 65, 3, blocks::state_by_name("stone").unwrap());
        let placed = place_block(&click((3, 65, 3), 0), &mut player, &shared).unwrap();
        assert!(placed.slots.contains(&slot));
        let top = shared.catalog().slab_state_for(row, SlabHalf::Top, false).unwrap();
        assert_eq!(target(&shared, &player), top, "{}", row.name);
        assert_eq!(player.inventory.slot(slot), Some(Stack::new(row.item, 1)));
        // Чужой цвет и одинаковый цвет чужого семейства не сливаются.
        // Соседняя клетка снизу занята камнем: отказ не должен расходовать предмет.
        for other in [colors[index / 16 * 16 + (index + 1) % 16], colors[(index + 16) % 32]] {
            let held = Stack::new(other.item, 1);
            player.inventory.set(slot, Some(held));
            let refused = place_block(&click((3, 64, 3), 0), &mut player, &shared).unwrap();
            assert_eq!(target(&shared, &player), top, "{} / {}", row.name, other.name);
            assert_eq!(player.inventory.slot(slot), Some(held));
            assert!(refused.slots.is_empty());
        }
        let (base, mut native) = setup(Profile::Java26_1_2);
        let slot = native.inventory.selected_slot();
        native.inventory.set(slot, Some(Stack::new(row.item, 2)));
        let refused = place_block(&click((3, 63, 3), 1), &mut native, &base).unwrap();
        assert!(refused.slots.is_empty());
        assert_eq!(target(&base, &native), world::AIR);
        assert_eq!(native.inventory.slot(slot), Some(Stack::new(row.item, 2)));
    }
}

#[test]
fn every_slab_family_mines_own_color_with_family_specific_hand_drop_and_exact_tool_wear() {
    for row in slabs() {
        for half in [SlabHalf::Top, SlabHalf::Bottom, SlabHalf::Double] {
            for tool_name in [None, Some("iron_pickaxe"), Some("shears")] {
                let (shared, mut player) = setup(Profile::Preview26_3);
                let state = shared.catalog().slab_state_for(row, half, false).unwrap();
                shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, state);
                let slot = player.inventory.selected_slot();
                let tool = tool_name.map(|name| Stack::new(blocks::item_named(name).unwrap(), 1).with_extras(crate::stackdata::Data {
                    damage: 7, name: Some("Цветная добыча".into()), ..Default::default()
                }));
                player.inventory.set(slot, tool);
                break_block(&dig(0), &mut player, &shared).unwrap();
                assert_eq!(target(&shared, &player), state);
                shared.ticks.send_modify(|tick| *tick += 1000);
                break_block(&dig(2), &mut player, &shared).unwrap();
                assert_eq!(target(&shared, &player), world::AIR, "{} {half:?}", row.name);
                if let Some(tool) = tool {
                    let mut data = tool.extras(); data.damage += 1;
                    assert_eq!(player.inventory.slot(slot), Some(tool.with_extras(data)));
                } else { assert!(player.inventory.slot(slot).is_none()); }
                let items = shared.items_in(player.dimension).lock().unwrap();
                let drops: Vec<_> = items.items().iter().map(|item| item.stack).collect();
                let harvests = row.family == SlabFamily::Wool || tool_name == Some("iron_pickaxe");
                assert_eq!(drops, if harvests { vec![Stack::new(row.item, if half == SlabHalf::Double { 2 } else { 1 })] } else { Vec::new() },
                    "{} {half:?} tool={tool_name:?}", row.name);
            }
        }
    }
}
