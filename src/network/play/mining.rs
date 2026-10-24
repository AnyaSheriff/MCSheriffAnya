use super::*;

#[derive(Clone, Copy)]
pub(super) struct Session {
    at: (i32, i32, i32),
    state: i32,
    dimension: crate::dimension::Dimension,
    started: u64,
    pending: bool,
    /// CANCEL гасит отложенное завершение, но цель и такт начала остаются:
    /// FINISH по тому же блоку всё равно проверяется по прошедшему времени
    /// (чёрный ящик mining-time-denied: оригинал ломает камень на такте 130
    /// после START 12 и CANCEL 13).
    cancelled: bool,
    face: u8,
    feedback_tick: u64,
}

fn progress(player: &PlayerState, shared: &Shared, state: i32) -> f32 {
    use crate::attributes::{BLOCK_BREAK_SPEED, MINING_EFFICIENCY, SUBMERGED_MINING_SPEED};
    use crate::survival::EffectKind;
    let tool = player.inventory.held();
    let underwater = vitals::eyes_in_water(&shared.world_in(player.dimension).lock().expect("мир"), player);
    crate::mining::progress_with_catalog(shared.catalog(), state, tool, crate::mining::Conditions {
        haste: player.vit.effect_level(EffectKind::Haste),
        fatigue: player.vit.effect_level(EffectKind::MiningFatigue),
        underwater,
        // Парящий робот чёрного ящика шлёт «в воздухе», стоя на полу; оригинал
        // его замедлением не наказывает (bed-place-break). Поэтому «на земле» —
        // ещё и «ноги на верхней грани твёрдого блока».
        on_ground: player.on_ground || {
            let world = shared.world_in(player.dimension).lock().expect("мир");
            let below = world.get_block(player.x.floor() as i32, (player.y - 0.001).floor() as i32, player.z.floor() as i32);
            (player.y - player.y.floor()).abs() < 0.001 && crate::blocks::solid_top(below)
        },
        // Зачарования уже включены refresh_equipment в атрибуты. Повторная
        // прибавка давала Efficiency V +52 вместо +26 и Aqua Affinity ×25
        // вместо ×5 (minecraft.wiki/w/Aqua_Affinity: «penalty to be ignored»).
        efficiency: player.attrs.value(MINING_EFFICIENCY) as f32,
        block_speed: player.attrs.value(BLOCK_BREAK_SPEED) as f32,
        submerged_speed: player.attrs.value(SUBMERGED_MINING_SPEED) as f32,
    })
}

/// Зажигание руды от START — тот же блок, а не подмена цели.
pub(super) fn punched(player: &mut PlayerState, state: i32) {
    if let Some(session) = &mut player.mining { session.state = state; }
}

pub(super) fn allows(player: &mut PlayerState, shared: &Shared, at: (i32, i32, i32), state: i32, status: i32, face: u8) -> bool {
    if player.game_mode == GAME_MODE_CREATIVE {
        player.mining = None;
        player.delayed = None;
        return status == 0;
    }
    let now = *shared.ticks.borrow();
    vitals::update_equipment(player, now);
    match status {
        0 => {
            let instant = progress(player, shared, state) >= 1.0;
            player.mining = (!instant).then_some(Session { at, state, dimension: player.dimension, started: now, pending: false, cancelled: false, face, feedback_tick: now });
            if !instant && state != 0 { feedback(shared, player, at, face, 0); }
            instant
        }
        1 => {
            if let Some(session) = &mut player.mining { session.cancelled = true; session.pending = false; }
            player.delayed = None;
            false
        }
        2 => {
            let Some(session) = player.mining else { return false; };
            if session.at != at || session.state != state || session.dimension != player.dimension {
                player.mining = None;
                return false;
            }
            if crate::mining::finished(progress(player, shared, state), now.saturating_sub(session.started)) {
                player.mining = None;
                player.delayed = None;
                true
            } else {
                // Ранний FINISH оставляет отложенное разрушение: оно живёт само
                // по себе и переживает новый START по тому же блоку (чёрный
                // ящик infested-mining-time: блок ломается на такте 24 при START 18).
                let mut delayed = session;
                delayed.pending = true;
                player.delayed = Some(delayed);
                player.mining.as_mut().expect("копает").pending = true;
                false
            }
        }
        _ => false,
    }
}

fn feedback(shared: &Shared, player: &PlayerState, at: (i32, i32, i32), face: u8, since: u64) {
    // Сетевой mining-audit777: камень, кровать, грани0/1/2; каждый четвёртый
    // такт2019 заменяется2020, data — грань из START. Прочие грани [проверить].
    // [проверить] В такте FINISH сетевой цикл пока может выдать лишний2019;
    // bed-place-break остаётся открытым, хотя прежние20пропусков устранены.
    if shared.catalog().profile() == crate::gameplay_catalog::Profile::Preview26_3 && face <= 5 {
        let effect = if since % 4 == 3 { 2020 } else { 2019 };
        shared.world_in(player.dimension).lock().expect("мир").note_level_event(at.0, at.1, at.2, effect, i32::from(face));
    }
}

/// [проверить] Ранний FINISH оставляет отложенное копание до полной прочности;
/// CANCEL, смена блока/измерения и выход из досягаемости отменяют его.
pub(super) fn tick(shared: &Shared, player: &mut PlayerState) {
    if shared.catalog().profile() == crate::gameplay_catalog::Profile::Preview26_3
        && let Some(session) = player.mining
        && !session.cancelled && !session.pending
        && session.dimension == player.dimension && player.game_mode == GAME_MODE_SURVIVAL && !player.vit.dead
        && session.state != 0 && within_reach(player, session.at)
    {
        let now = *shared.ticks.borrow();
        let state = shared.world_in(player.dimension).lock().expect("мир").get_block(session.at.0, session.at.1, session.at.2);
        if state == session.state && now > session.feedback_tick {
            feedback(shared, player, session.at, session.face, now.saturating_sub(session.started));
            player.mining.as_mut().expect("копает").feedback_tick = now;
        }
    }
    let Some(session) = player.delayed else { return; };
    let state = shared.world_in(player.dimension).lock().expect("мир").get_block(session.at.0, session.at.1, session.at.2);
    if player.dimension != session.dimension || state != session.state || !within_reach(player, session.at)
        || player.game_mode != GAME_MODE_SURVIVAL || player.vit.dead
    {
        player.delayed = None;
        return;
    }
    let since = (*shared.ticks.borrow()).saturating_sub(session.started);
    if progress(player, shared, state) * (since.saturating_add(1) as f32) >= 1.0 {
        player.delayed = None;
        let saved = player.mining.take();
        player.mining = Some(session);
        let mut payload = encode_varint(2);
        push_packed_position(&mut payload, session.at.0, session.at.1, session.at.2);
        payload.push(1);
        payload.extend_from_slice(&encode_varint(0));
        break_block(&payload, player, shared);
        player.mining = saved;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup(name: &str, tool: Option<&str>) -> (Shared, PlayerState) {
        setup_profile(name, tool, crate::gameplay_catalog::Profile::Java26_1_2)
    }
    fn setup_profile(name: &str, tool: Option<&str>, profile: crate::gameplay_catalog::Profile) -> (Shared, PlayerState) {
        let catalog = crate::gameplay_catalog::Catalog::new(profile);
        let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog), Default::default(), Default::default(), Default::default(), crate::ops::Ops::open(std::path::Path::new("target/mining-test-ops.json")), crate::skins::Settings::default());
        let mut player = PlayerState::new([18; 16], "MiningTest".into(), None, None, 0x7f, 1, (3.5, 64.0, 5.5));
        player.game_mode = GAME_MODE_SURVIVAL;
        player.on_ground = true;
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, blocks::state_by_name(name).unwrap());
        if let Some(tool) = tool { player.inventory.set(player.inventory.selected_slot(), Some(Stack::new(blocks::item_named(tool).unwrap(), 1))); }
        (shared, player)
    }
    fn dig(shared: &Shared, player: &mut PlayerState, status: i32) -> Reaction {
        let mut body = encode_varint(status);
        push_packed_position(&mut body, 3, 64, 3);
        body.push(1);
        body.extend_from_slice(&encode_varint(7));
        break_block(&body, player, shared).unwrap()
    }
    fn block(shared: &Shared, player: &PlayerState) -> i32 { shared.world_in(player.dimension).lock().unwrap().get_block(3, 64, 3) }

    #[test]
    fn mining_feedback_263_tracks_face_cadence_cancellation_and_changed_blocks() {
        use crate::gameplay_catalog::Profile;
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            for face in [0, 1, 2] {
                let (shared, mut player) = setup_profile("stone", None, profile);
                let reader = shared.next_reader();
                let start = shared.world_in(player.dimension).lock().unwrap().watch_events(reader);
                let stone = block(&shared, &player);
                allows(&mut player, &shared, (3, 64, 3), stone, 0, face);
                for now in 1..8 {
                    shared.ticks.send_replace(now);
                    tick(&shared, &mut player);
                    tick(&shared, &mut player);
                }
                allows(&mut player, &shared, (3, 64, 3), stone, 1, face);
                shared.ticks.send_replace(8);
                tick(&shared, &mut player);
                let (events, next) = shared.world_in(player.dimension).lock().unwrap().events_since(start, reader);
                let levels: Vec<_> = events.into_iter().filter_map(|e| match e {
                    WorldEvent::Level { effect: effect @ (2019 | 2020), data, .. } => Some((effect, data)),
                    _ => None,
                }).collect();
                let expected = if profile == Profile::Preview26_3 {
                    vec![(2019, i32::from(face)), (2019, i32::from(face)), (2019, i32::from(face)), (2020, i32::from(face)),
                         (2019, i32::from(face)), (2019, i32::from(face)), (2019, i32::from(face)), (2020, i32::from(face))]
                } else { vec![] };
                assert_eq!(levels, expected);
                // Исчезнувшая цель не создаёт частиц даже при оставшемся START.
                allows(&mut player, &shared, (3, 64, 3), stone, 0, face);
                let after_start = shared.world_in(player.dimension).lock().unwrap().events_since(next, reader).1;
                shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, 0);
                shared.ticks.send_replace(9);
                tick(&shared, &mut player);
                assert!(!shared.world_in(player.dimension).lock().unwrap().events_since(after_start, reader).0.iter()
                    .any(|e| matches!(e, WorldEvent::Level { effect: 2019 | 2020, .. })));
            }
        }
    }

    fn enchanted(player: &mut PlayerState, slot: i32, item: &str, name: &str, level: i32) {
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), level);
        player.inventory.set(slot, Some(Stack::new(blocks::item_named(item).unwrap(), 1).with_extras(data)));
    }

    #[test]
    fn efficiency_and_aqua_affinity_apply_once_and_refresh_before_digging() {
        let (shared, mut player) = setup("stone", Some("diamond_pickaxe"));
        let stone = block(&shared, &player);
        let hand = player.inventory.selected_slot();
        enchanted(&mut player, hand, "diamond_pickaxe", "efficiency", 5);
        // Первая попытка после замены вещи должна видеть новые атрибуты;
        // 34 / 45 < 1: Efficiency V не даёт мгновенного слома без Haste.
        assert!(!allows(&mut player, &shared, (3, 64, 3), stone, 0, 1));
        let dry = progress(&player, &shared, stone);
        assert!((dry - 34.0 / 45.0).abs() < 0.000_001);
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 65, 5, blocks::state_by_name("water").unwrap());
        assert!((progress(&player, &shared, stone) - dry / 5.0).abs() < 0.000_001);
        enchanted(&mut player, 5, "diamond_helmet", "aqua_affinity", 1);
        allows(&mut player, &shared, (3, 64, 3), stone, 0, 1);
        assert!((progress(&player, &shared, stone) - dry).abs() < 0.000_001);
        player.on_ground = false;
        player.y += 0.1;
        assert!((progress(&player, &shared, stone) - dry / 5.0).abs() < 0.000_001);
    }

    #[test]
    fn silk_dig_keeps_the_last_durability_drop_and_suppresses_ore_xp() {
        let (shared, mut player) = setup("diamond_ore", Some("diamond_pickaxe"));
        let hand = player.inventory.selected_slot();
        enchanted(&mut player, hand, "diamond_pickaxe", "silk_touch", 1);
        let held = player.inventory.held().unwrap();
        let mut data = held.extras();
        data.damage = crate::combat::max_damage(&held) - 1;
        player.inventory.set(hand, Some(held.with_extras(data)));
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 100);
        dig(&shared, &mut player, 2);
        assert_eq!(block(&shared, &player), 0);
        assert!(player.inventory.held().is_none());
        assert!(player.net.new_orbs.is_empty());
        let items = shared.items_in(player.dimension).lock().unwrap();
        assert_eq!(items.items().len(), 1);
        assert_eq!(blocks::item_name(items.items()[0].stack.item), Some("diamond_ore"));
    }

    #[test]
    fn wrong_tool_and_disabled_block_drops_never_grant_ore_experience() {
        for (tool, enabled) in [("wooden_pickaxe", true), ("diamond_pickaxe", false)] {
            let (shared, mut player) = setup("diamond_ore", Some(tool));
            shared.block_drops.store(enabled, std::sync::atomic::Ordering::Relaxed);
            dig(&shared, &mut player, 0);
            shared.ticks.send_modify(|tick| *tick += 1000);
            dig(&shared, &mut player, 2);
            assert_eq!(block(&shared, &player), 0);
            assert!(player.net.new_orbs.is_empty());
            assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
        }
    }

    #[test]
    fn finish_requires_start_and_seventy_percent_and_cancel_resets_it() {
        let (shared, mut player) = setup("stone", Some("iron_pickaxe"));
        let stone = block(&shared, &player);
        assert!(dig(&shared, &mut player, 2).confirm.is_empty());
        assert_eq!(block(&shared, &player), stone);
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 3);
        assert!(dig(&shared, &mut player, 2).confirm.is_empty());
        assert_eq!(block(&shared, &player), stone);
        // CANCEL цель не забывает: FINISH по тому же блоку позже ломает его
        // (чёрный ящик mining-time-denied, такт 130).
        dig(&shared, &mut player, 1);
        shared.ticks.send_modify(|tick| *tick += 100);
        dig(&shared, &mut player, 2);
        assert_eq!(block(&shared, &player), 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, stone);
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 5);
        dig(&shared, &mut player, 2);
        assert_eq!(block(&shared, &player), 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 2);
        let items = shared.items_in(player.dimension).lock().unwrap();
        let dropped = &items.items()[0];
        assert_eq!(dropped.vy, 0.2);
        assert!((-0.1..0.1).contains(&dropped.vx));
        assert!((3.25..3.75).contains(&dropped.x));
    }

    #[test]
    fn delayed_finish_completes_only_at_full_time_and_replacement_cancels() {
        let (shared, mut player) = setup("stone", Some("iron_pickaxe"));
        dig(&shared, &mut player, 0); dig(&shared, &mut player, 2);
        shared.ticks.send_modify(|tick| *tick += 5);
        tick(&shared, &mut player);
        assert_ne!(block(&shared, &player), 0);
        shared.ticks.send_modify(|tick| *tick += 2);
        tick(&shared, &mut player);
        assert_eq!(block(&shared, &player), 0);
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, blocks::state_by_name("stone").unwrap());
        dig(&shared, &mut player, 0);
        let dirt = blocks::state_by_name("dirt").unwrap();
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(3, 64, 3, dirt);
        shared.ticks.send_modify(|tick| *tick += 200);
        dig(&shared, &mut player, 2);
        assert_eq!(block(&shared, &player), dirt);
    }

    #[test]
    fn instant_creative_and_tool_break() {
        let (shared, mut player) = setup("torch", Some("iron_pickaxe"));
        dig(&shared, &mut player, 0);
        assert_eq!(block(&shared, &player), 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
        let (shared, mut player) = setup("stone", Some("iron_pickaxe"));
        let held = player.inventory.held().unwrap().with_extras(crate::stackdata::Data { damage: 249, ..Default::default() });
        player.inventory.set(player.inventory.selected_slot(), Some(held));
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 10);
        dig(&shared, &mut player, 2);
        assert!(player.inventory.held().is_none());
        let (shared, mut player) = setup("stone", Some("iron_pickaxe"));
        player.game_mode = GAME_MODE_CREATIVE;
        dig(&shared, &mut player, 0);
        assert_eq!(block(&shared, &player), 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
        assert!(shared.items_in(player.dimension).lock().unwrap().items().is_empty());
    }

    #[test]
    fn lighting_ore_does_not_wear_the_tool_and_swords_cost_two() {
        let (shared, mut player) = setup("redstone_ore", Some("iron_pickaxe"));
        dig(&shared, &mut player, 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
        shared.ticks.send_modify(|tick| *tick += 20);
        dig(&shared, &mut player, 2);
        assert_eq!(block(&shared, &player), 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 1);

        let (shared, mut player) = setup("cobweb", Some("iron_sword"));
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 10);
        dig(&shared, &mut player, 2);
        assert_eq!(block(&shared, &player), 0);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 2);
    }

    #[test]
    fn mining_break_publishes_equipment_status_and_unbreakable_is_untouched() {
        let (shared, mut player) = setup("stone", Some("iron_pickaxe"));
        let from = shared.world_in(player.dimension).lock().unwrap().watch_events(17);
        let tool = player.inventory.held().unwrap().with_extras(crate::stackdata::Data { damage: 249, ..Default::default() });
        player.inventory.set(player.inventory.selected_slot(), Some(tool));
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 20);
        dig(&shared, &mut player, 2);
        let (events, _) = shared.world_in(player.dimension).lock().unwrap().events_since(from, 17);
        assert!(events.iter().any(|event| matches!(event, WorldEvent::EquipmentBreak { status: 47, .. })));
        assert!(player.inventory.held().is_none());

        let (shared, mut player) = setup("stone", Some("iron_pickaxe"));
        let tool = player.inventory.held().unwrap().with_extras(crate::stackdata::Data { damage: 249, unbreakable: true, ..Default::default() });
        player.inventory.set(player.inventory.selected_slot(), Some(tool));
        dig(&shared, &mut player, 0);
        shared.ticks.send_modify(|tick| *tick += 20);
        dig(&shared, &mut player, 2);
        assert_eq!(player.inventory.held(), Some(tool));
    }

    #[test]
    fn observed_777_cancel_translation_clears_pending_native_mining() {
        use mcsa_protocol::{Direction,JavaVersion,State};
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/actual777-player-action-observations.json")).unwrap();
        let case=fixture["observations"].as_array().unwrap().iter().find(|v|v["case"]=="cancel").unwrap();
        let packets:Vec<Vec<u8>>=case["actions"].as_array().unwrap().iter().map(|event| {
            let mut body=encode_varint(PLAYER_ACTION);
            body.extend(event["payload_hex"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
                .map(|pair|u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()));
            body
        }).collect();
        assert_eq!(packets.len(),2);assert_eq!(packets[1][1],2);
        let apply=|body:Vec<u8>,player:&mut PlayerState,shared:&Shared| {
            handle_client_packet(&PacketInfo {id:PLAYER_ACTION,length:body.len() as i32,body,teleport:None,
                foreign_inventory:false,legacy_inventory:None},player,shared)
        };
        for untranslated in [false,true] {
            let (shared,mut player)=setup("obsidian",None);
            player.x=0.5;player.y=65.;player.z=0.5;
            let obsidian=blocks::state_by_name("obsidian").unwrap();
            shared.world_in(player.dimension).lock().unwrap().set_block_silently(2,65,0,obsidian);
            let mapped=mcsa_protocol::wire::translate(State::Play,Direction::Serverbound,
                JavaVersion::V26_3,JavaVersion::V26_1_2,&packets[0]).unwrap();
            apply(mapped.into_owned(),&mut player,&shared);
            assert!(!player.mining.unwrap().cancelled);
            // Создаём pending ранним родным FINISH, чтобы CANCEL имел
            // различимый эффект. Это проверка нашего ядра, не timer-parity.
            let mut early=packets[0].clone();early[1]=2;
            apply(early,&mut player,&shared);assert!(player.delayed.is_some());
            let cancel=if untranslated { packets[1].clone() } else {
                mcsa_protocol::wire::translate(State::Play,Direction::Serverbound,
                    JavaVersion::V26_3,JavaVersion::V26_1_2,&packets[1]).unwrap().into_owned()
            };
            apply(cancel,&mut player,&shared);
            assert_eq!(player.mining.unwrap().cancelled,!untranslated);
            assert_eq!(player.delayed.is_none(),!untranslated,
                "непереведённый777CANCEL2 — родной FINISH2, он не должен пройти контроль");
            assert_eq!(shared.world_in(player.dimension).lock().unwrap().get_block(2,65,0),obsidian);
        }
    }

}
