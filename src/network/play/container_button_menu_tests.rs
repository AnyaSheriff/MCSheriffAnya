//! Пакет клиента777 проходит production reader и обработчик настоящего окна.
//! Имена полей взяты из pinned JSON; семантика страниц — существующий lectern.rs.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile};

const POS: (i32, i32, i32) = (0, 64, 0);

fn scene(profile: Profile) -> (Shared, PlayerState, Stack) {
    let catalog = Catalog::new(profile);
    let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("unused/container-button-ops.json")), crate::skins::Settings::default());
    let mut player = PlayerState::new_for_catalog([0xe3; 16], "MenuButton".into(), None, None, 0x7f, 93,
        (0.5, 65.0, 0.5), catalog);
    let book = Stack::new(blocks::item_named("writable_book").unwrap(), 1);
    {
        let mut world = shared.world_in(player.dimension).lock().unwrap();
        assert!(world.set_block_silently(POS.0, POS.1, POS.2, blocks::state_by_name("lectern").unwrap()));
        assert_eq!(redstone::lectern::place_book(&mut world, POS, Some(book), 8), Some(true));
    }
    player.window = Some(OpenWindow { id: 7, halves: vec![POS], cart: None, shown: vec![Some(book)],
        lectern: true, crafter: false, kind: Some(containers::Kind::Lectern), props: vec![],
        merchant: None, station: None, ender: false, plugin_chest: None });
    (shared, player, book)
}

async fn read_button(catalog: Catalog, window: i32, button: i32, extra: &[u8]) -> io::Result<PacketInfo> {
    let table: serde_json::Value = serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/container-button-fields.json")).unwrap();
    let row = table.as_array().unwrap().iter().find(|r| r["version"] == 777).unwrap();
    let id = i32::from_str_radix(row["packet_id"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
    let mut body = encode_varint(id);
    body.extend(encode_varint(window)); body.extend(encode_varint(button)); body.extend(extra);
    let mut frame = Vec::new();
    super::super::packet::frame(&mut frame, &body, super::super::packet::compression_threshold());
    super::super::packet::read_play_packet_with_content(&mut frame.as_slice(), mcsa_protocol::JavaVersion::V26_3,
        None, false, catalog.content_codec()).await
}

#[tokio::test]
async fn native777_page_jump_above_enchantment_range_reaches_real_lectern() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, mut player, _) = scene(profile);
        let before = player.inventory.clone();
        let packet = read_button(shared.catalog(), 7, 103, &[]).await.unwrap();
        assert_eq!(packet.id, CLICK_CONTAINER_BUTTON);
        let reaction = handle_client_packet(&packet, &mut player, &shared);
        assert_eq!(redstone::lectern::page(&shared.world_in(player.dimension).lock().unwrap(), POS), Some((3, 8)));
        assert_eq!(reaction.window_property, Some((7, 0, 3)));
        assert_eq!(player.inventory, before);
    }
}

#[tokio::test]
async fn native777_take_book_above_enchantment_range_transfers_it_once() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, mut player, book) = scene(profile);
        let packet = read_button(shared.catalog(), 7, 3, &[]).await.unwrap();
        let reaction = handle_client_packet(&packet, &mut player, &shared);
        assert_eq!(reaction.close_window, Some(7));
        assert!(player.window.is_none());
        assert_eq!(player.inventory.slots().iter().copied().flatten().collect::<Vec<_>>(), vec![book]);
        let after = player.inventory.clone();
        handle_client_packet(&packet, &mut player, &shared);
        assert_eq!(player.inventory, after);
    }
}

#[tokio::test]
async fn native777_wrong_window_and_enchanting_button_keep_gameplay_bounds() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, mut player, _) = scene(profile);
        let packet = read_button(shared.catalog(), 8, 103, &[]).await.unwrap();
        let reaction = handle_client_packet(&packet, &mut player, &shared);
        assert!(reaction.window_property.is_none());
        assert_eq!(redstone::lectern::page(&shared.world_in(player.dimension).lock().unwrap(), POS), Some((0, 8)));
        let open = player.window.as_mut().unwrap();
        open.lectern = false; open.kind = None;
        open.shown = vec![Some(Stack::new(blocks::item_named("diamond_sword").unwrap(), 1)),
            Some(Stack::new(blocks::item_named("lapis_lazuli").unwrap(), 64))];
        open.station = Some(station::StationWindow { kind: crate::enchanting::Station::Enchanting, pos: POS,
            props: vec![1, 1, 1], name: None, cost: 0, repair_items: 0, selected: -1, input_item: None, sound_tick: None });
        let before = (open.shown.clone(), player.vit.xp, player.vit.xp_seed);
        let packet = read_button(shared.catalog(), 7, 103, &[]).await.unwrap();
        handle_client_packet(&packet, &mut player, &shared);
        assert_eq!((player.window.as_ref().unwrap().shown.clone(), player.vit.xp, player.vit.xp_seed), before);
    }
}

#[tokio::test]
async fn native777_malformed_container_buttons_stop_before_menu_mutation() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, player, _) = scene(profile);
        let before = player.inventory.clone();
        assert!(read_button(shared.catalog(), 7, 103, &[0]).await.is_err());
        assert!(read_button(shared.catalog(), 7, -1, &[]).await.is_err());
        assert!(read_button(shared.catalog(), -1, 103, &[]).await.is_err());
        assert_eq!(redstone::lectern::page(&shared.world_in(player.dimension).lock().unwrap(), POS), Some((0, 8)));
        assert_eq!(player.inventory, before);
    }
}
