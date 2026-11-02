//! Реальная цепочка Java snapshot → файл → Bedrock → файл → Java.
use super::*;

// Progress::save использует пути относительно cwd: отдельный процесс
// изолирует их, не меняя cwd других параллельных unit tests.
fn isolated(test: &str) -> bool {
    if std::env::var("MCSA_CURSOR_SAVE_WORKER").as_deref() == Ok(test) { return false; }
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp")
        .join(format!("cursor-save-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("network::play::save_conservation_tests::{test}"), "--nocapture"])
        .env("MCSA_CURSOR_SAVE_WORKER", test).current_dir(&directory).output().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    true
}

#[test]
fn java_autosaved_cursor_survives_disk_and_bedrock_with_full_inventory() {
    if isolated("java_autosaved_cursor_survives_disk_and_bedrock_with_full_inventory") { return; }
    let directory = std::path::Path::new("playerdata");
    std::fs::create_dir(directory).unwrap();
    let uuid = [237; 16];
    let mut player = PlayerState::new(uuid, "CursorSave".into(), None, None, 0x7f, 1, (0.5, 70.0, 0.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    let stone = Stack::new(blocks::item_named("stone").unwrap(), 64);
    let held = Stack::new(blocks::item_named("diamond").unwrap(), 37).with_extras(crate::stackdata::Data {
        name: Some("Не потерять после сбоя".into()), repair_cost: 17, ..Default::default()
    });
    for slot in 9..=44 { player.inventory.set(slot, Some(stone)); }
    player.inventory.set(36, Some(held));
    player.save_if_needed(directory, crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2));
    assert!(player.inventory.click(36, 0, 0).understood);
    assert_eq!(player.inventory.cursor(), Some(held));
    // Пока игрок держит курсор, освободившееся место заполняет другая стопка.
    assert!(player.inventory.put(stone).1.is_none());
    assert!(player.inventory.slots()[9..=44].iter().all(Option::is_some));
    assert!(player.needs_save());
    player.save_if_needed(directory, crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2));
    assert!(!player.needs_save());
    let original = player.inventory.clone();
    // Не вызываем finish_play/close_window: именно так остаётся файл после
    // аварийного завершения между autosave и обычным выходом игрока.
    drop(player);
    let saved = playerdata::load(directory, &uuid).unwrap();
    assert_eq!(saved.inventory, original, "autosave обязан сохранить и курсор");
    assert_eq!(saved.inventory.cursor().unwrap().extras(), held.extras());
    let bag = crate::bedrock::inventory::Bag::new(saved.inventory.clone());
    assert!(bag.inventory.cursor().is_none(), "единственный курсор хранится в Bag");
    assert_eq!(bag.inventory_with_cursor(), original);
    // Начальная отправка не должна очистить UI-курсор на клиенте. Используем
    // уже существующий формат синхронизации крафта с тем же курсором/stack ID.
    let initial_ui = bag.open_sync_packets()[2].clone();
    assert_eq!(initial_ui, *bag.crafting_sync_packets().last().unwrap());
    let empty_cursor_bag = crate::bedrock::inventory::Bag::new(bag.inventory.clone());
    assert_ne!(initial_ui, empty_cursor_bag.open_sync_packets()[2]);
    let mut overflow = bag.clone();
    assert_eq!(overflow.put_cursor_back(), Some(held), "полный рюкзак не поглощает курсор");
    assert!(overflow.inventory_with_cursor().cursor().is_none());
    let mut bedrock_saved = saved;
    bedrock_saved.inventory = bag.inventory_with_cursor();
    playerdata::save(directory, &uuid, &bedrock_saved).unwrap();
    let reloaded = playerdata::load(directory, &uuid).unwrap();
    let restored = PlayerState::new(uuid, "CursorSave".into(), Some(reloaded), None, 0x7f, 2, (0.5, 70.0, 0.5));
    assert_eq!(restored.inventory, original);
    assert_eq!(restored.inventory.cursor(), Some(held));
    assert_eq!(restored.inventory.cursor().unwrap().extras(), held.extras());
}

#[tokio::test]
async fn effects_survive_java_file_bedrock_and_initial_java_packets() {
    if isolated("effects_survive_java_file_bedrock_and_initial_java_packets") { return; }
    use crate::survival::EffectKind;
    let directory = std::path::Path::new("playerdata");
    std::fs::create_dir(directory).unwrap();
    let uuid = [236; 16];
    let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
    let mut player = PlayerState::new_for_catalog(uuid, "EffectsSave".into(), None, None, 0x7f, 1, (0.5, 70.0, 0.5), catalog);
    player.vit.give_effect(EffectKind::HealthBoost, 1, 900);
    player.vit.give_effect(EffectKind::Speed, 1, 900);
    for (level, ticks) in [(0, 900), (1, 700), (2, 500), (3, 100)] {
        player.vit.give_effect(EffectKind::Poison, level, ticks);
    }
    player.vit.give_effect(EffectKind::Absorption, 1, 900);
    vitals::apply_effect_modifiers(&mut player);
    player.vit.health = 27.0;
    player.vit.absorption = 3.0;
    let expected = player.vit.saved_effects();
    player.save_with_catalog(directory, catalog);
    assert!(!player.needs_save());
    player.vit.effects_tick();
    assert!(!player.needs_save(), "простое уменьшение длительности не пишет каждый такт");
    let mut saved = playerdata::load_checked_for_catalog(directory, &uuid, catalog).unwrap().unwrap();
    let mut bedrock = crate::bedrock::health::Health::new_with_effects(Some(&saved.vitals), &saved.effects);
    assert_eq!(bedrock.vit.health, 27.0);
    assert_eq!(bedrock.vit.max_health, 28.0);
    assert_eq!(bedrock.vit.saved_effects(), expected);
    let mut view = crate::bedrock::player_state::View::default();
    let mut packets = Vec::new();
    view.sync(1, &bedrock.vit, 0, 0, (0.0, 0.0, 0.0), &mut packets);
    for effect in &expected.active {
        assert!(packets.contains(&crate::bedrock::player_state::mob_effect(1, 1, *effect).unwrap()));
    }
    packets.clear();
    view.sync(1, &bedrock.vit, 0, 1, (0.0, 0.0, 0.0), &mut packets);
    assert!(packets.is_empty(), "первоначальные эффекты приходят один раз");
    bedrock.vit.effects_tick();
    saved.vitals = bedrock.vit.saved();
    saved.effects = bedrock.vit.saved_effects();
    playerdata::save_for_catalog(directory, &uuid, &saved, catalog).unwrap();
    let saved = playerdata::load_checked_for_catalog(directory, &uuid, catalog).unwrap().unwrap();
    let mut restored = PlayerState::new_for_catalog(uuid, "EffectsSave".into(), Some(saved.clone()), None, 0x7f, 2, (0.5, 70.0, 0.5), catalog);
    restored.entity_id = 42;
    assert_eq!(restored.vit.saved_effects(), saved.effects);
    assert_eq!(restored.vit.health, 27.0);
    assert_eq!(restored.attrs.value(crate::attributes::MAX_HEALTH), 28.0);
    assert!(restored.attrs.value(crate::attributes::MOVEMENT_SPEED) > 0.13);
    let mut outgoing = Outgoing::capture();
    vitals::send_initial_effects(&mut outgoing, &restored).await.unwrap();
    let bytes = outgoing.queued();
    let mut expected_bytes = Vec::new();
    for effect in &restored.vit.effects {
        let mut body = encode_varint(ENTITY_EFFECT);
        for value in [42, effect.kind.id(), effect.amplifier, effect.ticks_left] { body.extend_from_slice(&encode_varint(value)); }
        body.push(effect.flags());
        expected_bytes.extend_from_slice(&encode_varint(body.len() as i32));
        expected_bytes.extend(body);
    }
    assert_eq!(bytes, expected_bytes);
    assert_eq!(restored.vit.absorption, 3.0);
    restored.vit.remove_effects(None);
    assert!(restored.vit.saved_effects().hidden.is_empty());
    let mut dead = saved;
    dead.vitals.health = 0.0;
    let dead = PlayerState::new_for_catalog(uuid, "EffectsSave".into(), Some(dead), None, 0x7f, 2, (0.5, 70.0, 0.5), catalog);
    assert!(dead.vit.effects.is_empty());
    assert_eq!(dead.vit.absorption, 0.0);
}
