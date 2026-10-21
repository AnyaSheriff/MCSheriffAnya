//! Все цвета проходят реальные обработчики; внутренние ID не заменяются белыми.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile, SlabHalf, CONCRETE_SLABS};

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
    for row in &CONCRETE_SLABS {
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
    for (index, row) in CONCRETE_SLABS.iter().enumerate() {
        let (shared, mut player) = setup(Profile::Preview26_3);
        let slot = player.inventory.selected_slot();
        player.inventory.set(slot, Some(Stack::new(row.item, 2)));
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 65, 3, blocks::state_by_name("stone").unwrap());
        let placed = place_block(&click((3, 65, 3), 0), &mut player, &shared).unwrap();
        assert!(placed.slots.contains(&slot));
        let top = shared.catalog().slab_state_for(row, SlabHalf::Top, false).unwrap();
        assert_eq!(target(&shared, &player), top, "{}", row.name);
        assert_eq!(player.inventory.slot(slot), Some(Stack::new(row.item, 1)));
        let other = &CONCRETE_SLABS[(index + 1) % CONCRETE_SLABS.len()];
        player.inventory.set(slot, Some(Stack::new(other.item, 1)));
        // Снизу свободная половина, но предмет другого цвета не должен удвоить плиту.
        place_block(&click((3, 64, 3), 0), &mut player, &shared).unwrap();
        assert_eq!(target(&shared, &player), top, "{} / {}", row.name, other.name);
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
fn every_color_native_mining_drops_its_own_single_or_double_and_bare_hand_drops_nothing() {
    for row in &CONCRETE_SLABS {
        for half in [SlabHalf::Top, SlabHalf::Bottom, SlabHalf::Double] {
            for pickaxe in [false, true] {
                let (shared, mut player) = setup(Profile::Preview26_3);
                let state = shared.catalog().slab_state_for(row, half, false).unwrap();
                shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, state);
                let slot = player.inventory.selected_slot();
                let tool = Stack::new(blocks::item_named("iron_pickaxe").unwrap(), 1).with_extras(crate::stackdata::Data {
                    damage: 7, name: Some("Цветная добыча".into()), ..Default::default()
                });
                player.inventory.set(slot, pickaxe.then_some(tool));
                break_block(&dig(0), &mut player, &shared).unwrap();
                assert_eq!(target(&shared, &player), state);
                shared.ticks.send_modify(|tick| *tick += 1000);
                break_block(&dig(2), &mut player, &shared).unwrap();
                assert_eq!(target(&shared, &player), world::AIR, "{} {half:?}", row.name);
                if pickaxe {
                    let mut data = tool.extras(); data.damage += 1;
                    assert_eq!(player.inventory.slot(slot), Some(tool.with_extras(data)));
                } else { assert!(player.inventory.slot(slot).is_none()); }
                let items = shared.items_in(player.dimension).lock().unwrap();
                let drops: Vec<_> = items.items().iter().map(|item| item.stack).collect();
                assert_eq!(drops, if pickaxe { vec![Stack::new(row.item, if half == SlabHalf::Double { 2 } else { 1 })] } else { Vec::new() },
                    "{} {half:?} pickaxe={pickaxe}", row.name);
            }
        }
    }
}
