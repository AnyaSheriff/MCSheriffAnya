use super::*;
use crate::mob::Kind;

fn shared() -> Shared {
    Shared::new(
        World::in_memory(), World::in_memory(), Default::default(), Default::default(),
        Default::default(), crate::ops::Ops::open(std::path::Path::new("target/arthropod-test-ops.json")),
        crate::skins::Settings::default(),
    )
}

fn player() -> PlayerState {
    let mut player = PlayerState::new([17; 16], "ArthropodTest".to_string(), None, None, 0x7F, 11, (3.5, 64.0, 5.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    player
}

fn dig(at: (i32, i32, i32), status: i32) -> Vec<u8> {
    let mut packet = encode_varint(status);
    push_packed_position(&mut packet, at.0, at.1, at.2);
    packet.push(1);
    packet.extend_from_slice(&encode_varint(1));
    packet
}

#[test]
fn all_infested_kinds_release_from_the_survival_break_handler() {
    for name in ["stone", "cobblestone", "stone_bricks", "mossy_stone_bricks", "cracked_stone_bricks", "chiseled_stone_bricks", "deepslate"] {
        let shared = shared();
        let mut player = player();
        let state = blocks::state_by_name(&format!("infested_{name}")).unwrap();
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, state);
        // Не мгновенны: «начал копать» блок не ломает, ломает «докопал».
        break_block(&dig((3, 64, 3), 0), &mut player, &shared).unwrap();
        assert_eq!(shared.world_in(player.dimension).lock().unwrap().get_block(3, 64, 3), state);
        assert!(break_block(&dig((3, 64, 3), 2), &mut player, &shared).is_some());
        assert_eq!(shared.world_in(player.dimension).lock().unwrap().get_block(3, 64, 3), blocks::state_by_name("air").unwrap());
        let items = shared.items_in(player.dimension).lock().unwrap();
        assert_eq!(items.mobs().len(), 1, "{name}");
        assert_eq!(items.mobs()[0].kind, Kind::Silverfish);
        assert!(items.items().is_empty());
    }
}

#[test]
fn silk_creative_and_disabled_drops_suppress_release() {
    for (creative, silk, drops, difficulty, expected_drop) in [
        (false, true, true, 2, true), (true, false, true, 2, false),
        (false, false, false, 2, false),
        (true, true, true, 2, false), (false, true, false, 2, false),
    ] {
        let shared = shared();
        let mut player = player();
        player.game_mode = if creative { GAME_MODE_CREATIVE } else { GAME_MODE_SURVIVAL };
        shared.difficulty.store(difficulty, std::sync::atomic::Ordering::Relaxed);
        shared.block_drops.store(drops, std::sync::atomic::Ordering::Relaxed);
        if silk {
            let mut extras = crate::stackdata::Data::default();
            let id = crate::stackdata::ENCHANTMENTS.iter().position(|(name, _)| *name == "silk_touch").unwrap();
            extras.set_enchant(id as u16, 1);
            // Любой инструмент: вики явно разрешает даже не кирку.
            player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("wooden_hoe").unwrap(), 1).with_extras(extras)));
        }
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, blocks::state_by_name("infested_stone").unwrap());
        break_block(&dig((3, 64, 3), if creative { 0 } else { 2 }), &mut player, &shared).unwrap();
        let items = shared.items_in(player.dimension).lock().unwrap();
        assert!(items.mobs().is_empty());
        assert_eq!(items.items().len(), usize::from(expected_drop));
        if expected_drop {
            assert_eq!(items.items()[0].stack.item, blocks::item_named("stone").unwrap());
        }
    }
}

#[test]
fn arthropod_spawn_eggs_spawn_and_consume_only_in_survival() {
    for kind in [Kind::CaveSpider, Kind::Silverfish] {
        for creative in [false, true] {
            let shared = shared();
            let mut player = player();
            player.game_mode = if creative { GAME_MODE_CREATIVE } else { GAME_MODE_SURVIVAL };
            let egg = blocks::item_named(&format!("{}_spawn_egg", kind.name())).unwrap();
            player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(egg, 2)));
            use_spawn_egg(egg, (3, 63, 3), 1, 1, &mut player, &shared).unwrap();
            let items = shared.items_in(player.dimension).lock().unwrap();
            assert_eq!(items.mobs().len(), 1);
            assert_eq!(items.mobs()[0].kind, kind);
            assert_eq!((items.mobs()[0].x, items.mobs()[0].y, items.mobs()[0].z), (3.5, 64.0, 3.5));
            assert_eq!(player.inventory.held().unwrap().count, if creative { 2 } else { 1 });
        }
    }
}

#[test]
fn peaceful_difficulty_still_releases_the_silverfish() {
    // Чёрный ящик, silverfish-infested: на мирной чешуйница выходит и исчезает
    // уже сама (despawn), а не блокируется при выходе.
    let shared = shared();
    let mut player = player();
    shared.difficulty.store(0, std::sync::atomic::Ordering::Relaxed);
    shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, blocks::state_by_name("infested_stone").unwrap());
    break_block(&dig((3, 64, 3), 2), &mut player, &shared).unwrap();
    let items = shared.items_in(player.dimension).lock().unwrap();
    assert_eq!(items.mobs().len(), 1);
    assert!(items.mobs()[0].spawn_poof);
}

#[test]
fn spawn_mobs_does_not_disable_infested_release() {
    let shared = shared();
    let mut player = player();
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, blocks::state_by_name("infested_stone").unwrap());
    break_block(&dig((3, 64, 3), 2), &mut player, &shared).unwrap();
    assert_eq!(shared.items_in(player.dimension).lock().unwrap().mobs().len(), 1);
}

#[test]
fn summon_commands_resolve_both_new_kinds() {
    for kind in [Kind::CaveSpider, Kind::Silverfish] {
        let shared = shared();
        crate::commands::run(&format!("summon {} 3.5 64 3.5 {{NoAI:1b}}", kind.name()), &crate::commands::Source::Console, &shared);
        let items = shared.items_in(Default::default()).lock().unwrap();
        assert_eq!(items.mobs().len(), 1);
        assert_eq!(items.mobs()[0].kind, kind);
        assert!(items.mobs()[0].no_ai);
    }
}

#[tokio::test]
async fn cave_spider_poison_requires_a_successful_unblocked_hit() {
    // Только локальная пара сокетов теста пакетов, без сервера Minecraft.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let _client = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let (mut server, _) = listener.accept().await.unwrap();
    let mut out = Outgoing::new(&mut server);
    for (difficulty, creative, invulnerable, shield, duration) in [
        (1, false, false, false, None), (2, false, false, false, Some(140)),
        (3, false, false, false, Some(300)), (2, true, false, false, None),
        (2, false, true, false, None), (2, false, false, true, None),
    ] {
        let shared = shared();
        shared.difficulty.store(difficulty, std::sync::atomic::Ordering::Relaxed);
        let mut player = player();
        player.game_mode = if creative { GAME_MODE_CREATIVE } else { GAME_MODE_SURVIVAL };
        if invulnerable {
            player.vit.hurt(2.0, crate::survival::Source::MobAttack, false);
        }
        if shield {
            player.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("shield").unwrap(), 1)));
            player.net.using = Some(vitals::Using { kind: vitals::UseKind::Shield, hand: 1, since: 0 });
            shared.ticks.send_replace(10);
        }
        let at = (player.x, player.y, player.z + 1.0);
        vitals::mob_hit(&mut out, &shared, &mut player, 2.0, crate::survival::Source::MobAttack, 42, "entity.minecraft.cave_spider", None, at).await.unwrap();
        assert_eq!(player.vit.effects.iter().find(|effect| effect.kind == crate::survival::EffectKind::Poison).map(|effect| effect.ticks_left), duration);
    }
}
