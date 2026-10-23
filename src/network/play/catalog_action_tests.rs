//! Новая плита проходит настоящие обработчики установки и добычи.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile, SlabHalf, WHITE_SLAB_ITEM};

fn setup(profile: Profile) -> (Shared, PlayerState) {
    let catalog = Catalog::new(profile);
    let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/catalog-action-ops.json")), crate::skins::Settings::default());
    let mut player = PlayerState::new([219; 16], "CatalogAction".into(), None, None, 0x7f, 219, (3.5, 64.0, 5.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    player.on_ground = true;
    {
        let mut world = shared.world_in(player.dimension).lock().unwrap();
        world.ensure(0, 0);
        world.set_block_silently(3, 63, 3, blocks::state_by_name("stone").unwrap());
    }
    (shared, player)
}

fn click(at: (i32, i32, i32), sequence: i32) -> Vec<u8> {
    let mut body = encode_varint(0);
    push_packed_position(&mut body, at.0, at.1, at.2);
    body.extend(encode_varint(1));
    for _ in 0..3 { body.extend(0.5f32.to_be_bytes()); }
    body.push(0);
    body.extend(encode_varint(sequence));
    body
}

fn dig(status: i32, sequence: i32) -> Vec<u8> {
    let mut body = encode_varint(status);
    push_packed_position(&mut body, 3, 64, 3);
    body.push(1);
    body.extend(encode_varint(sequence));
    body
}

fn target(shared: &Shared, player: &PlayerState) -> i32 {
    shared.world_in(player.dimension).lock().unwrap().get_block(3, 64, 3)
}

#[test]
fn slab_native_handlers_spend_two_merge_mine_and_preserve_tool() {
    let (shared, mut player) = setup(Profile::Preview26_3);
    let slot = player.inventory.selected_slot();
    let slabs = Stack::new(WHITE_SLAB_ITEM, 2).with_extras(crate::stackdata::Data { name: Some("Плиты".into()), ..Default::default() });
    player.inventory.set(slot, Some(slabs));
    let first = place_block(&click((3, 63, 3), 11), &mut player, &shared).unwrap();
    assert_eq!(first.acknowledge, Some(11));
    assert!(first.slots.contains(&slot));
    assert_eq!(player.inventory.slot(slot), Some(slabs.with_count(1)));
    assert_eq!(shared.catalog().slab_properties(target(&shared, &player)), Ok(Some((SlabHalf::Bottom, false))));
    let second = place_block(&click((3, 64, 3), 12), &mut player, &shared).unwrap();
    assert_eq!(second.acknowledge, Some(12));
    assert!(second.slots.contains(&slot));
    assert!(player.inventory.slot(slot).is_none());
    let double = target(&shared, &player);
    assert_eq!(shared.catalog().slab_properties(double), Ok(Some((SlabHalf::Double, false))));
    let tool = Stack::new(blocks::item_named("iron_pickaxe").unwrap(), 1).with_extras(crate::stackdata::Data {
        damage: 7, name: Some("Кирка".into()), ..Default::default()
    });
    player.inventory.set(slot, Some(tool));
    break_block(&dig(0, 13), &mut player, &shared).unwrap();
    assert_eq!(target(&shared, &player), double);
    shared.ticks.send_modify(|tick| *tick += 100);
    let finish = break_block(&dig(2, 14), &mut player, &shared).unwrap();
    assert_eq!(finish.acknowledge, Some(14));
    assert_eq!(target(&shared, &player), world::AIR);
    let worn = player.inventory.slot(slot).unwrap();
    assert_eq!(worn.item, tool.item);
    assert_eq!(worn.count, 1);
    let mut expected = tool.extras();
    expected.damage += 1;
    assert_eq!(worn.extras(), expected);
    let items = shared.items_in(player.dimension).lock().unwrap();
    assert_eq!(items.items().len(), 1);
    assert_eq!(items.items()[0].stack, Stack::new(WHITE_SLAB_ITEM, 2));
}

#[test]
fn native_placement_rejects_extension_in_old_profile_without_spending() {
    let (shared, mut player) = setup(Profile::Java26_1_2);
    let slot = player.inventory.selected_slot();
    player.inventory.set(slot, Some(Stack::new(WHITE_SLAB_ITEM, 2)));
    let before = player.inventory.slots().to_vec();
    let reaction = place_block(&click((3, 63, 3), 15), &mut player, &shared).unwrap();
    assert_eq!(reaction.acknowledge, Some(15));
    assert!(reaction.slots.is_empty());
    assert_eq!(player.inventory.slots(), before);
    assert_eq!(target(&shared, &player), world::AIR);
    assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
}

#[test]
fn native_slab_and_egg_merge_spend_once_in_survival_and_never_in_creative() {
    for name in ["stone_slab", "turtle_egg"] {
        for mode in [GAME_MODE_SURVIVAL, GAME_MODE_CREATIVE] {
            let (shared, mut player) = setup(Profile::Java26_1_2);
            player.game_mode = mode;
            let slot = player.inventory.selected_slot();
            let stack = Stack::new(blocks::item_named(name).unwrap(), 2);
            player.inventory.set(slot, Some(stack));
            {
                let mut world = shared.world_in(player.dimension).lock().unwrap();
                world.set_block_silently(3, 64, 3, blocks::state_by_name(name).unwrap());
            }
            let reaction = place_block(&click((3, 64, 3), 20), &mut player, &shared).unwrap();
            assert_eq!(reaction.acknowledge, Some(20));
            assert_eq!(player.inventory.slot(slot), Some(stack.with_count(if mode == GAME_MODE_SURVIVAL { 1 } else { 2 })), "{name} mode{mode}");
            assert_eq!(reaction.slots.contains(&slot), mode == GAME_MODE_SURVIVAL);
            let state = target(&shared, &player);
            let orientation = blocks::orientation(name).unwrap();
            assert_eq!(orientation.value(state, if name == "stone_slab" { "type" } else { "eggs" }), Some(if name == "stone_slab" { "double" } else { "2" }));
            assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
        }
    }
}

#[test]
fn native_slab_refused_merge_preserves_stack_and_half() {
    let (shared, mut player) = setup(Profile::Java26_1_2);
    let slot = player.inventory.selected_slot();
    let slab = blocks::state_by_name("stone_slab").unwrap();
    let stack = Stack::new(blocks::item_named("stone_slab").unwrap(), 2);
    player.inventory.set(slot, Some(stack));
    player.set_position(3.5, 64.5, 3.5);
    shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, slab);
    let reaction = place_block(&click((3, 64, 3), 21), &mut player, &shared).unwrap();
    assert_eq!(player.inventory.slot(slot), Some(stack));
    assert!(reaction.slots.is_empty());
    assert_eq!(target(&shared, &player), slab);
    assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
}

#[test]
fn creative775_view_cannot_overwrite_or_drop_real_new_items() {
    let (shared,mut player)=setup(Profile::Preview26_3);
    player.game_mode=GAME_MODE_CREATIVE;
    let actual=Stack::new(WHITE_SLAB_ITEM,3);
    player.inventory.set_for_catalog(36,Some(actual),shared.catalog()).unwrap();
    let shown=Stack::new(blocks::item_named("quartz_slab").unwrap(),3);
    let slot=mcsa_protocol::items::Stack{item:shown.item as u32,count:3,added:vec![],removed:vec![]};
    for target in [36i16,37,-1] {
        let mut body=encode_varint(SET_CREATIVE_MODE_SLOT);body.extend(target.to_be_bytes());
        body.extend(mcsa_protocol::items::encode(mcsa_protocol::JavaVersion::V26_1_2,Some(&slot),true,Default::default()).unwrap());
        let packet=PacketInfo{id:SET_CREATIVE_MODE_SLOT,length:body.len() as i32,body,teleport:None,foreign_inventory:true,legacy_inventory:None};
        let reaction=handle_client_packet(&packet,&mut player,&shared);
        assert!(reaction.inventory);assert_eq!(player.inventory.slot(36),Some(actual));assert!(player.inventory.slot(37).is_none());
    }
}
