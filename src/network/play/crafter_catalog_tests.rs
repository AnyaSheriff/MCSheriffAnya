//! Каталог влияет на подсказку крафтера, но подсказка не выдаёт предметы.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile, WHITE_SLAB_ITEM};

const POS: (i32, i32, i32) = (0, 64, 0);

fn fixture(profile: Profile, complete: bool) -> (Shared, PlayerState) {
    let catalog = Catalog::new(profile);
    let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/crafter-catalog-ops.json")), crate::skins::Settings::default());
    let mut player = PlayerState::new_for_catalog([240; 16], "CrafterCatalog".into(), None, None, 0, 240,
        (0.5, 64.0, 2.5), catalog);
    player.game_mode = GAME_MODE_SURVIVAL;
    let mut slots = vec![None; 9];
    slots[..if complete { 3 } else { 2 }].fill(Some(concrete()));
    {
        let mut world = shared.world_in(player.dimension).lock().unwrap();
        world.ensure(0, 0);
        assert!(world.set_block(POS.0, POS.1, POS.2, blocks::state_by_name("crafter").unwrap()));
        containers::store_window_slots(&mut world, &[POS], &slots);
        assert_eq!(world.container(POS.0, POS.1, POS.2).unwrap().slots, slots);
    }
    player.window = Some(OpenWindow { id: 1, halves: vec![POS], cart: None, shown: slots,
        lectern: false, crafter: true, kind: Some(containers::Kind::Crafter), props: Vec::new(),
        merchant: None, station: None, ender: false });
    (shared, player)
}

fn concrete() -> Stack { Stack::new(blocks::item_named("white_concrete").unwrap(), 1) }
fn expected(profile: Profile) -> Option<Stack> {
    (profile == Profile::Preview26_3).then(|| Stack::new(WHITE_SLAB_ITEM, 6))
}
fn click_payload(slot: i16, mode: i32) -> Vec<u8> {
    let mut bytes = encode_varint(1);
    bytes.extend(encode_varint(0)); bytes.extend(slot.to_be_bytes()); bytes.push(0);
    bytes.extend(encode_varint(mode)); bytes.extend([0, 0]); bytes
}

async fn content(out: &Outgoing<'_>) -> (Vec<Option<Stack>>, Option<Stack>) {
    let mut bytes = out.queued();
    let packet = read_packet(&mut bytes).await.unwrap();
    assert!(bytes.is_empty());
    assert_eq!(packet.id, SET_CONTAINER_CONTENT);
    let mut cursor = Cursor::new(&packet.body);
    assert_eq!(cursor.varint(), Some(SET_CONTAINER_CONTENT));
    assert_eq!(cursor.varint(), Some(1));
    assert!(cursor.varint().unwrap() > 0);
    let count = cursor.varint().unwrap();
    assert_eq!(count, 46);
    let mut slots = Vec::new();
    for _ in 0..count {
        let (stack, readable) = read_slot(&mut cursor).unwrap();
        assert!(readable); slots.push(stack);
    }
    let (held, readable) = read_slot(&mut cursor).unwrap();
    assert!(readable);
    assert_eq!(cursor.offset, cursor.bytes.len());
    (slots, held)
}

#[tokio::test]
async fn initial_and_inventory_correction_packets_use_owned_catalog_for_display_slot_45() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, mut player) = fixture(profile, true);
        let before = player.inventory.clone();
        let inputs = player.window.as_ref().unwrap().shown.clone();
        let mut initial = Outgoing::capture();
        send_window_content(&mut initial, &mut player, shared.catalog()).await.unwrap();
        let (slots, held) = content(&initial).await;
        assert_eq!(&slots[..9], inputs.as_slice());
        assert_eq!(slots[45], expected(profile)); assert!(held.is_none());
        let mut correction = Outgoing::capture();
        send_container_content(&mut correction, &mut player, shared.catalog()).await.unwrap();
        assert_eq!(content(&correction).await, (slots, held));
        assert!(player.inventory_after_window);
        assert_eq!(player.inventory, before);
        assert_eq!(player.window.as_ref().unwrap().shown, inputs);
    }
}

#[test]
fn actual_input_click_updates_profile_preview_without_allowing_display_output_to_be_taken() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, mut player) = fixture(profile, false);
        player.inventory.set_cursor(Some(concrete()));
        let mut reaction = Reaction::default();
        click_window(&shared, &mut player, &click_payload(2, 0), &mut reaction);
        assert!(player.inventory.cursor().is_none());
        assert_eq!(player.window.as_ref().unwrap().shown[..3], [Some(concrete()); 3]);
        let world_slots = shared.world_in(player.dimension).lock().unwrap()
            .container(POS.0, POS.1, POS.2).unwrap().slots.clone();
        assert_eq!(world_slots, player.window.as_ref().unwrap().shown);
        assert_eq!(reaction.window_slots.contains(&(45, expected(profile))), profile == Profile::Preview26_3);
        let before = player.inventory.clone();
        for mode in [0, 1, 2, 4] {
            click_window(&shared, &mut player, &click_payload(45, mode), &mut Reaction::default());
            assert_eq!(player.inventory, before, "режим {mode}");
            assert_eq!(player.window.as_ref().unwrap().shown, world_slots);
            assert_eq!(shared.world_in(player.dimension).lock().unwrap()
                .container(POS.0, POS.1, POS.2).unwrap().slots, world_slots);
        }
        assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
    }
}

#[tokio::test]
async fn published_container_changes_send_profile_preview_and_clear_it_when_recipe_breaks() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, mut player) = fixture(profile, false);
        let mut complete = player.window.as_ref().unwrap().shown.clone();
        complete[2] = Some(concrete());
        // Публикация отделена от live-содержимого: именно её читает отправка окна.
        shared.world_in(player.dimension).lock().unwrap().published.insert(POS, complete.clone());
        let mut out = Outgoing::capture();
        send_window_changes(&mut out, &shared, &mut player).await.unwrap();
        let mut bytes = out.queued();
        let mut changes = Vec::new();
        while !bytes.is_empty() {
            let packet = read_packet(&mut bytes).await.unwrap();
            assert_eq!(packet.id, SET_CONTAINER_SLOT);
            let mut cursor = Cursor::new(&packet.body);
            assert_eq!(cursor.varint(), Some(SET_CONTAINER_SLOT));
            assert_eq!(cursor.varint(), Some(1)); cursor.varint().unwrap();
            let slot = cursor.u16().unwrap();
            let (stack, readable) = read_slot(&mut cursor).unwrap(); assert!(readable);
            changes.push((slot, stack));
        }
        let mut wanted = vec![(2, Some(concrete()))];
        if profile == Profile::Preview26_3 { wanted.push((45, expected(profile))); }
        assert_eq!(changes, wanted);
        assert_eq!(player.window.as_ref().unwrap().shown, complete);
        assert!(player.inventory.cursor().is_none());
        shared.world_in(player.dimension).lock().unwrap().published.get_mut(&POS).unwrap()[2] = None;
        let mut removed = Outgoing::capture();
        send_window_changes(&mut removed, &shared, &mut player).await.unwrap();
        let mut bytes = removed.queued();
        let mut slots = Vec::new();
        while !bytes.is_empty() {
            let packet = read_packet(&mut bytes).await.unwrap();
            assert_eq!(packet.id, SET_CONTAINER_SLOT);
            let mut cursor = Cursor::new(&packet.body);
            cursor.varint().unwrap(); cursor.varint().unwrap(); cursor.varint().unwrap();
            slots.push(cursor.u16().unwrap());
            assert_eq!(read_slot(&mut cursor), Some((None, true)));
        }
        assert_eq!(slots, if profile == Profile::Preview26_3 { vec![2, 45] } else { vec![2] });
    }
}
