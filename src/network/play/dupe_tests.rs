//! Сохранение числа и компонентов вещей при закрытии окон и повторных пакетах.
use super::*;

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), Default::default(),
        Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/dupe-audit-ops.json")),
        crate::skins::Settings::default())
}

fn named(name: &str, count: i32, label: &str) -> Stack {
    Stack::new(blocks::item_named(name).unwrap(), count).with_extras(crate::stackdata::Data {
        name: Some(label.into()), ..Default::default()
    })
}

fn all_items(shared: &Shared, player: &PlayerState) -> Vec<Stack> {
    let mut all: Vec<_> = player.inventory.slots().iter().flatten().copied().collect();
    all.extend(player.inventory.cursor());
    all.extend(shared.items_in(player.dimension).lock().unwrap().items().iter().map(|item| item.stack));
    all
}

fn packet(id: i32, payload: &[u8]) -> PacketInfo {
    let mut body = encode_varint(id);
    body.extend_from_slice(payload);
    PacketInfo { id, length: body.len() as i32, body, teleport: None,
        foreign_inventory: false, legacy_inventory: None }
}

#[test]
fn merchant_cleanup_conserves_inputs_cursor_and_grid_once() {
    // Внезапный выход/смерть идут через close_own_window; обычное закрытие —
    // через пакет. При полном рюкзаке прежняя ветка выхода теряла всю оплату,
    // при частично заполненном — остаток. Выходная ячейка лишь предпросмотр.
    for free in [0, 4] {
        for abrupt in [false, true] {
            let shared = shared();
            let mut player = PlayerState::new([219; 16], "DupeAudit".into(), None,
                None, 0x7f, 219, (0.5, 64.0, 0.5));
            player.game_mode = GAME_MODE_SURVIVAL;
            let filler = Stack::new(blocks::item_named("stone").unwrap(), 64);
            for slot in 9..=44 { player.inventory.set(slot, Some(filler)); }
            let payment = named("emerald", 64, "Оплата с компонентами");
            if free > 0 { player.inventory.set(36, inventory::remainder(payment, 64 - free)); }
            let second = named("diamond", 5, "Вторая оплата");
            let cursor = named("lapis_lazuli", 3, "Курсор");
            let grid = named("gold_ingot", 7, "Сетка");
            let preview = named("bread", 64, "Предпросмотр, не настоящая вещь");
            player.inventory.set_cursor(Some(cursor));
            player.inventory.set(1, Some(grid));
            player.inventory.set(0, Some(preview));
            let mut expected = all_items(&shared, &player);
            expected.retain(|stack| *stack != preview);
            expected.extend([payment, second]);
            let mut merchant = crate::villager::Merchant::new(12345);
            merchant.slots = [Some(payment), Some(second), Some(preview)];
            player.window = Some(OpenWindow {
                id: 51, halves: Vec::new(), cart: None, shown: merchant.slots.to_vec(),
                lectern: false, crafter: false, kind: None, props: Vec::new(),
                merchant: Some(merchant), station: None, ender: false,
            });
            let close = packet(CLOSE_CONTAINER, &encode_varint(51));
            if abrupt {
                close_own_window(&shared, &mut player);
                for stack in player.inventory.close_window().thrown { throw_out(&shared, &player, stack); }
            } else {
                handle_client_packet(&close, &mut player, &shared);
            }
            assert!(player.window.is_none());
            assert!(player.inventory.cursor().is_none());
            assert!((0..=4).all(|slot| player.inventory.slot(slot).is_none()));
            let check = |player: &PlayerState| {
                let all = all_items(&shared, player);
                assert_eq!(all.iter().map(|s| s.count).sum::<i32>(),
                    expected.iter().map(|s| s.count).sum::<i32>(), "free={free}, abrupt={abrupt}");
                for kind in [filler, payment, second, cursor, grid] {
                    let count = |items: &[Stack]| items.iter().filter(|s| s.stacks_with(&kind)).map(|s| s.count).sum::<i32>();
                    assert_eq!(count(&all), count(&expected), "free={free}, abrupt={abrupt}, item={}", kind.item);
                }
                assert!(!all.iter().any(|s| s.item == preview.item));
            };
            check(&player);
            let on_ground: i32 = shared.items_in(player.dimension).lock().unwrap().items()
                .iter().filter(|item| item.stack.stacks_with(&payment)).map(|item| item.stack.count).sum();
            assert_eq!(on_ground, 64 - free);

            // Повтор Close и запоздавший Click старого окна не создают оплату
            // заново и не выдают виртуальный результат сделки.
            let mut click = encode_varint(51);
            click.extend(encode_varint(0)); click.extend(2i16.to_be_bytes());
            click.extend([0, 0, 0, 0]);
            handle_client_packet(&packet(CLICK_CONTAINER, &click), &mut player, &shared);
            handle_client_packet(&close, &mut player, &shared);
            close_own_window(&shared, &mut player);
            check(&player);
        }
    }
}

#[test]
fn bookshelf_pickup_with_full_inventory_preserves_book_and_components_once() {
    let shared = shared();
    let mut player = PlayerState::new([220; 16], "BookshelfAudit".into(), None,
        None, 0x7f, 220, (0.5, 64.0, 0.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    player.awaiting_teleport = false;
    let filler = Stack::new(blocks::item_named("diamond").unwrap(), 64);
    for slot in 9..=44 { player.inventory.set(slot, Some(filler)); }
    let book = named("book", 1, "Книга, которую нельзя потерять");
    {
        let mut world = shared.world_in(player.dimension).lock().unwrap();
        world.set_block(0, 64, 2, blocks::state_from_text("minecraft:chiseled_bookshelf[facing=north]").unwrap());
        world.with_container(0, 64, 2, |shelf| shelf.slots[0] = Some(book));
        redstone::bookshelf::contents_changed(&mut world, (0, 64, 2));
        assert_eq!(world.container(0, 64, 2).unwrap().slots[0], Some(book));
    }
    let mut body = encode_varint(0);
    push_packed_position(&mut body, 0, 64, 2);
    body.extend(encode_varint(2));
    for value in [0.8f32, 0.75, 0.0] { body.extend(value.to_be_bytes()); }
    body.push(0); body.extend(encode_varint(1));
    for _ in 0..2 {
        handle_client_packet(&packet(USE_ITEM_ON, &body), &mut player, &shared);
        assert!(shared.world_in(player.dimension).lock().unwrap().container(0, 64, 2).unwrap().slots[0].is_none());
        let items = all_items(&shared, &player);
        assert_eq!(items.iter().filter(|s| s.stacks_with(&book)).map(|s| s.count).sum::<i32>(), 1);
        assert_eq!(items.iter().filter(|s| s.stacks_with(&filler)).map(|s| s.count).sum::<i32>(), 36 * 64);
        assert_eq!(items.iter().map(|s| s.count).sum::<i32>(), 36 * 64 + 1);
        let drops = shared.items_in(player.dimension).lock().unwrap();
        assert_eq!(drops.items().len(), 1);
        assert_eq!(drops.items()[0].stack, book);
    }
}

fn market() -> (Shared, PlayerState) {
    let shared = shared();
    let mut player = PlayerState::new([221; 16], "TradeAudit".into(), None,
        None, 0x7f, 221, (0.5, 64.0, 0.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    let emerald = blocks::item_named("emerald").unwrap();
    let mut villager = crate::mob::Mob::new(12346, crate::mob::Kind::Villager, (0.5, 64.0, 2.5));
    villager.no_ai = true;
    villager.villager.offers = [(64, named("diamond", 64, "Дорогая сделка")),
        (1, named("bread", 1, "Дешёвая сделка"))].into_iter().map(|(cost, sell)| crate::villager::Offer {
            buy: Stack::new(emerald, cost), buy_b: None, sell,
            uses: 0, max_uses: 100, xp: 0, reward_exp: false,
            multiplier: 0.0, special: 0, demand: 0,
        }).collect();
    let mut merchant = crate::villager::Merchant::new(villager.entity_id);
    merchant.slots[0] = Some(Stack::new(emerald, 64));
    merchant.update_result(&villager.villager.offers);
    shared.items_in(player.dimension).lock().unwrap().add_mob(villager);
    player.window = Some(OpenWindow {
        id: 52, halves: Vec::new(), cart: None, shown: merchant.slots.to_vec(),
        lectern: false, crafter: false, kind: None, props: Vec::new(),
        merchant: Some(merchant), station: None, ender: false,
    });
    (shared, player)
}

fn take_trade(shared: &Shared, player: &mut PlayerState, mode: i32) {
    let mut body = encode_varint(52);
    body.extend(encode_varint(0)); body.extend(2i16.to_be_bytes()); body.push(0);
    body.extend(encode_varint(mode)); body.extend([0, 0]);
    handle_client_packet(&packet(CLICK_CONTAINER, &body), player, shared);
}

#[test]
fn selecting_trade_with_unchanged_payment_refreshes_the_actual_goods() {
    let (shared, mut player) = market();
    let expensive = named("diamond", 64, "Дорогая сделка");
    let cheap = named("bread", 1, "Дешёвая сделка");
    // Обе сделки автозаполняются теми же 64 изумрудами. Раньше выбранный
    // номер менялся, но выход оставался от дорогой сделки: 64 алмаза за 1.
    for selected in [1, 1, 0, 1] {
        handle_client_packet(&packet(merchant::SELECT_TRADE, &encode_varint(selected)), &mut player, &shared);
        let trade = player.window.as_ref().unwrap().merchant.as_ref().unwrap();
        assert_eq!(trade.slots[0].unwrap().count, 64);
        assert_eq!(trade.slots[2], Some(if selected == 0 { expensive } else { cheap }));
    }
    for count in 1..=2 {
        take_trade(&shared, &mut player, 0);
        assert_eq!(player.inventory.cursor(), Some(cheap.with_count(count)));
        assert_eq!(player.window.as_ref().unwrap().merchant.as_ref().unwrap().slots[0].unwrap().count, 64 - count);
        let items = shared.items_in(player.dimension).lock().unwrap();
        assert_eq!(items.mobs()[0].villager.offers[0].uses, 0);
        assert_eq!(items.mobs()[0].villager.offers[1].uses, count);
    }
    assert!(!all_items(&shared, &player).iter().any(|stack| stack.item == expensive.item));
}

#[test]
fn stale_trade_preview_cannot_authorize_wrong_goods_or_unpaid_output() {
    let expensive = named("diamond", 64, "Дорогая сделка");
    let cheap = named("bread", 1, "Дешёвая сделка");
    for mode in [0, 1] {
        let (shared, mut player) = market();
        // Имитируем устаревший предпросмотр независимо от SelectTrade:
        // даже корректный выбор не делает сохранённый output авторитетным.
        player.window.as_mut().unwrap().merchant.as_mut().unwrap().selected = 1;
        take_trade(&shared, &mut player, mode);
        let bought = if mode == 0 { 1 } else { 64 };
        let all = all_items(&shared, &player);
        assert_eq!(all.iter().filter(|s| s.stacks_with(&cheap)).map(|s| s.count).sum::<i32>(), bought);
        assert!(!all.iter().any(|s| s.item == expensive.item));
        assert_eq!(player.window.as_ref().unwrap().merchant.as_ref().unwrap().slots[0].map_or(0, |s| s.count), 64 - bought);
        let items = shared.items_in(player.dimension).lock().unwrap();
        assert_eq!(items.mobs()[0].villager.offers[0].uses, 0);
        assert_eq!(items.mobs()[0].villager.offers[1].uses, bought);
    }

    let (shared, mut player) = market();
    {
        let trade = player.window.as_mut().unwrap().merchant.as_mut().unwrap();
        trade.slots[0] = trade.slots[0].map(|s| s.with_count(63));
        // Для дорогой сделки оплаты мало, дешёвая уже распродана.
        shared.items_in(player.dimension).lock().unwrap().mobs_mut()[0].villager.offers[1].max_uses = 0;
    }
    for mode in [0, 1, 0] {
        take_trade(&shared, &mut player, mode);
        assert!(all_items(&shared, &player).is_empty());
        let trade = player.window.as_ref().unwrap().merchant.as_ref().unwrap();
        assert_eq!(trade.slots[0].unwrap().count, 63);
        assert!(trade.slots[2].is_none());
    }
    // Обратный случай: старый дешёвый предпросмотр не позволяет списать
    // дорогую оплату и выдать хлеб вместо положенных алмазов.
    let (shared, mut player) = market();
    player.window.as_mut().unwrap().merchant.as_mut().unwrap().slots[2] = Some(cheap);
    take_trade(&shared, &mut player, 0);
    assert_eq!(player.inventory.cursor(), Some(expensive));
    assert!(player.window.as_ref().unwrap().merchant.as_ref().unwrap().slots[0].is_none());
    take_trade(&shared, &mut player, 0);
    assert_eq!(player.inventory.cursor(), Some(expensive));
}
