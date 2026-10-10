use super::*;
use crate::mob::{Kind, Mob};

fn shared() -> Shared {
    Shared::new(
        World::in_memory(), World::in_memory(), Default::default(), Default::default(),
        Default::default(), crate::ops::Ops::open(std::path::Path::new("target/skeleton-test-ops.json")),
        crate::skins::Settings::default(),
    )
}

fn player() -> PlayerState {
    let mut player = PlayerState::new([18; 16], "SkeletonTest".into(), None, None, 0x7F, 11, (3.5, 64.0, 5.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    player
}

#[test]
fn both_eggs_spawn_bow_users_and_consume_only_in_survival() {
    for kind in [Kind::Stray, Kind::Bogged] {
        for mode in [GAME_MODE_CREATIVE, GAME_MODE_SURVIVAL] {
            let shared = shared();
            let mut player = player();
            player.game_mode = mode;
            let egg = blocks::item_named(&format!("{}_spawn_egg", kind.name())).unwrap();
            player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(egg, 2)));
            use_spawn_egg(egg, (3, 63, 3), 1, 1, &mut player, &shared).unwrap();
            let items = shared.items_in(player.dimension).lock().unwrap();
            assert_eq!(items.mobs()[0].kind, kind);
            assert!(items.mobs()[0].h.bow);
            assert_eq!(player.inventory.held().unwrap().count, if mode == GAME_MODE_CREATIVE { 2 } else { 1 });
        }
    }
}

#[test]
fn shearing_handler_drops_once_and_wears_survival_shears() {
    for mode in [GAME_MODE_CREATIVE, GAME_MODE_SURVIVAL] {
        let shared = shared();
        let mut player = player();
        player.game_mode = mode;
        let id = shared.next_entity_id();
        shared.items_in(player.dimension).lock().unwrap().add_mob(Mob::new(id, Kind::Bogged, (3.5, 64.0, 3.5)));
        let shears = blocks::item_named("shears").unwrap();
        player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(shears, 1)));
        let mut reaction = Reaction::default();
        assert!(use_mob(&shared, &mut player, id, &mut reaction));
        assert!(use_mob(&shared, &mut player, id, &mut reaction));
        let items = shared.items_in(player.dimension).lock().unwrap();
        assert!(items.mobs()[0].sheared);
        assert_eq!(items.items().len(), 2);
        assert_eq!(player.inventory.held().unwrap().extras().damage, if mode == GAME_MODE_CREATIVE { 0 } else { 1 });
    }
}

#[test]
fn dropped_tipped_arrows_can_be_fired_and_infinity_does_not_save_them() {
    let shared = shared();
    let mut player = player();
    let mut data = crate::stackdata::Data::default();
    data.set_enchant(crate::stackdata::ENCHANTMENTS.iter().position(|(name, _)| *name == "infinity").unwrap() as u16, 1);
    player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("bow").unwrap(), 1).with_extras(data)));
    let potion = crate::survival::potion_number("slowness");
    let tipped = Stack::new(blocks::item_named("tipped_arrow").unwrap(), 2).with_extras(crate::stackdata::Data { potion, ..Default::default() });
    player.inventory.set(inventory::OFFHAND, Some(tipped));
    assert_eq!(bow::arrow_slot(&player), Some(inventory::OFFHAND));
    bow::release(&shared, &mut player, 0, 20);
    assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().count, 1);
    let arrows = shared.arrows.lock().unwrap();
    let arrows = arrows.in_dimension(player.dimension);
    let arrow = arrows.first().unwrap();
    assert_eq!(arrow.potion, potion);
    assert_eq!(arrow.effects[0].ticks_left, 225);
    assert_eq!(arrow.pickup, crate::arrow::Pickup::Yes);
    assert!(arrow.pickup_stack().stacks_with(&tipped));
}
