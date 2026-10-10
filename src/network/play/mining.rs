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
}

fn progress(player: &PlayerState, shared: &Shared, state: i32) -> f32 {
    use crate::attributes::{BLOCK_BREAK_SPEED, MINING_EFFICIENCY, SUBMERGED_MINING_SPEED};
    use crate::survival::EffectKind;
    let tool = player.inventory.held();
    let efficiency = tool.map_or(0, |stack| stack.extras().level("efficiency"));
    let aqua = player.inventory.slot(5).is_some_and(|stack| stack.extras().level("aqua_affinity") > 0);
    let underwater = vitals::eyes_in_water(&shared.world_in(player.dimension).lock().expect("мир"), player);
    crate::mining::progress(state, tool, crate::mining::Conditions {
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
        efficiency: player.attrs.value(MINING_EFFICIENCY) as f32 + if efficiency > 0 { (efficiency as f32).powi(2) + 1.0 } else { 0.0 },
        block_speed: player.attrs.value(BLOCK_BREAK_SPEED) as f32,
        submerged_speed: player.attrs.value(SUBMERGED_MINING_SPEED) as f32 * if aqua { 5.0 } else { 1.0 },
    })
}

/// Зажигание руды от START — тот же блок, а не подмена цели.
pub(super) fn punched(player: &mut PlayerState, state: i32) {
    if let Some(session) = &mut player.mining { session.state = state; }
}

pub(super) fn allows(player: &mut PlayerState, shared: &Shared, at: (i32, i32, i32), state: i32, status: i32) -> bool {
    if player.game_mode == GAME_MODE_CREATIVE {
        player.mining = None;
        player.delayed = None;
        return status == 0;
    }
    let now = *shared.ticks.borrow();
    match status {
        0 => {
            let instant = progress(player, shared, state) >= 1.0;
            player.mining = (!instant).then_some(Session { at, state, dimension: player.dimension, started: now, pending: false, cancelled: false });
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

/// [проверить] Ранний FINISH оставляет отложенное копание до полной прочности;
/// CANCEL, смена блока/измерения и выход из досягаемости отменяют его.
pub(super) fn tick(shared: &Shared, player: &mut PlayerState) {
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
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(), crate::ops::Ops::open(std::path::Path::new("target/mining-test-ops.json")), crate::skins::Settings::default());
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
}
