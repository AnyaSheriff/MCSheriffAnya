// Вики, Player.dat_format: «abilities» сохраняет разрешение и состояние полёта.
use super::*;

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/abilities-ops.json")), crate::skins::Settings::default())
}

fn player(mode: i32) -> PlayerState {
    let mut p = PlayerState::new([39; 16], "Abilities".into(), None, None, 0x7f, 0, (0.5, 70.0, 0.5));
    vitals::change_game_mode(&mut p, mode);
    p
}

fn client_flags(p: &mut PlayerState, shared: &Shared, flags: &[u8]) {
    let mut body = encode_varint(SERVERBOUND_ABILITIES);
    body.extend_from_slice(flags);
    handle_client_packet(&PacketInfo { id: SERVERBOUND_ABILITIES, length: body.len() as i32, body }, p, shared);
}

fn reconnect(p: &mut PlayerState, label: &str) -> PlayerState {
    let directory = std::env::temp_dir().join(format!("mc_abilities_{label}"));
    std::fs::create_dir_all(&directory).unwrap();
    p.save(&directory);
    let saved = playerdata::load(&directory, &p.uuid).unwrap();
    let joined = PlayerState::new(p.uuid, p.name.clone(), Some(saved), None, 0x7f, 0, p.spawn_point);
    std::fs::remove_dir_all(directory).unwrap();
    joined
}

#[test]
fn client_can_only_change_flying_when_permitted() {
    let shared = shared();
    for mode in 0..=3 {
        let mut p = player(mode);
        let before = p.abilities;
        client_flags(&mut p, &shared, &[255]);
        assert_eq!(p.abilities, playerdata::Abilities { flying: matches!(mode, 1 | 3), ..before });
        client_flags(&mut p, &shared, &[0]);
        assert!(!p.abilities.flying);
        assert_eq!(p.abilities, playerdata::Abilities { flying: false, ..before });
    }
}

#[test]
fn empty_client_abilities_keeps_state_and_flight_clears_fall() {
    let shared = shared();
    let mut p = player(1);
    p.vit.fall = 12.0;
    client_flags(&mut p, &shared, &[]);
    assert_eq!(p.vit.fall, 12.0);
    assert!(!p.abilities.flying);
    client_flags(&mut p, &shared, &[2]);
    assert_eq!(p.vit.fall, 0.0);
}

#[test]
fn creative_flying_and_grounded_states_survive_disk_rejoin() {
    for flying in [false, true] {
        let mut p = player(1);
        p.abilities.flying = flying;
        let joined = reconnect(&mut p, if flying { "flying" } else { "grounded" });
        assert_eq!(joined.abilities, p.abilities);
        assert_eq!(joined.abilities.flags(), if flying { 15 } else { 13 });
    }
}

#[test]
fn spectator_rejoin_forces_flight_even_after_client_stopped_it() {
    let mut p = player(3);
    p.abilities.flying = false;
    let joined = reconnect(&mut p, "spectator");
    assert_eq!(joined.abilities.flags(), 7);
}

#[test]
fn plugin_permission_and_flying_share_persisted_state_in_survival() {
    let shared = shared();
    let mut p = player(0);
    assert!(p.apply_plugin_flight("fixture".into(), 0, 1));
    assert!(p.apply_plugin_flight("fixture".into(), 1, 1));
    let mut joined = reconnect(&mut p, "plugin");
    assert!(joined.flight_allowed() && joined.abilities.flying);
    assert_eq!(joined.abilities.flags(), 6);
    assert!(joined.flight_owners.iter().all(Option::is_none));
    client_flags(&mut joined, &shared, &[0]);
    assert!(joined.flight_allowed() && !joined.abilities.flying);
    assert!(joined.apply_plugin_flight("fixture".into(), 0, 0));
    client_flags(&mut joined, &shared, &[2]);
    assert!(!joined.abilities.flying);
}

#[test]
fn plugin_permission_overrides_survive_rejoin_in_creative_and_survival() {
    for mode in [0, 1] {
        for allowed in [false, true] {
            let mut p = player(mode);
            p.apply_plugin_flight("fixture".into(), 0, i8::from(allowed));
            let joined = reconnect(&mut p, &format!("permission_{mode}_{allowed}"));
            assert_eq!(joined.flight_allowed(), allowed);
            assert!(!joined.abilities.flying);
            assert_eq!(joined.abilities.instabuild, mode == 1);
            assert_eq!(joined.abilities.invulnerable, mode == 1);
        }
    }
}

#[test]
fn plugin_flying_rejects_missing_permission_and_unload_respects_owner() {
    let mut p = player(0);
    p.apply_plugin_flight("one".into(), 1, 1);
    assert!(!p.abilities.flying);
    p.apply_plugin_flight("one".into(), 0, 1);
    p.apply_plugin_flight("one".into(), 1, 1);
    assert!(!p.apply_plugin_flight("two".into(), 0, -2));
    assert!(p.abilities.flying);
    assert!(p.apply_plugin_flight("one".into(), 0, -2));
    assert!(!p.abilities.flying && !p.abilities.mayfly);
}

#[test]
fn mode_changes_clear_flight_in_survival_and_adventure_and_keep_spectator_flight_in_creative() {
    let mut p = player(1);
    p.abilities.flying = true;
    for mode in [0, 1, 3, 1, 2, 3, 2] {
        vitals::change_game_mode(&mut p, mode);
        let flags = match mode { 0 | 2 => 0, 3 => 7, 1 if p.prev_mode == 3 => 15, _ => 13 };
        assert_eq!(p.abilities.flags(), flags, "{mode}");
        assert_eq!(p.abilities.may_build, matches!(mode, 0 | 1));
    }
}

#[test]
fn mode_changes_replace_plugin_permission_but_preserve_speeds() {
    let mut p = player(0);
    p.apply_plugin_flight("fixture".into(), 0, 1);
    p.apply_plugin_flight("fixture".into(), 1, 1);
    p.abilities.fly_speed = 0.125;
    p.abilities.walk_speed = 0.25;
    vitals::change_game_mode(&mut p, 2);
    assert_eq!(p.abilities.flags(), 0);
    assert_eq!((p.abilities.fly_speed, p.abilities.walk_speed), (0.125, 0.25));
}

#[test]
fn unchanged_game_mode_preserves_flight_and_plugin_permission_without_orders() {
    for mode in [0, 1, 3] {
        let mut p = player(mode);
        p.apply_plugin_flight("fixture".into(), 0, 1);
        p.apply_plugin_flight("fixture".into(), 1, 1);
        let before = p.abilities;
        let shared = shared();
        shared.players.lock().unwrap().join(p.member(), 391).unwrap();
        commands::run(&format!("gamemode {mode} {}", p.name), &commands::Source::Console, &shared);
        assert!(shared.players.lock().unwrap().take_orders(&p.uuid).is_empty());
        vitals::change_game_mode(&mut p, mode);
        assert_eq!(p.abilities, before);
        assert_eq!(p.flight_owners[0].as_deref(), Some("fixture"));
    }
}

#[tokio::test]
async fn self_command_sends_abilities_after_state_was_applied_before_queued_order() {
    let shared = shared();
    let mut p = player(0);
    shared.players.lock().unwrap().join(p.member(), 391).unwrap();
    shared.ops.lock().unwrap().give(&p.uuid, &p.name, 4);
    let mut body = encode_varint(SERVERBOUND_CHAT_COMMAND);
    body.extend_from_slice(&encode_string("gamemode creative"));
    handle_client_packet(&PacketInfo { id: SERVERBOUND_CHAT_COMMAND, length: body.len() as i32, body }, &mut p, &shared);
    assert_eq!(p.game_mode, 1);
    let orders = shared.players.lock().unwrap().take_orders(&p.uuid);
    assert!(matches!(orders.as_slice(), [Order::GameMode { mode: 1 }]));
    let mut out = Outgoing::capture();
    vitals::game_mode_order(&mut out, &mut p, 1, 10).await.unwrap();
    let mut bytes = out.queued();
    let mut flags = Vec::new();
    while !bytes.is_empty() {
        let packet = read_packet(&mut bytes).await.unwrap();
        if packet.id == PLAYER_ABILITIES { flags.push(packet.payload()[0]); }
    }
    assert_eq!(flags, [13, 13]);
}

#[test]
fn respawn_restores_mode_defaults_and_resets_speeds_and_plugin_flight() {
    for mode in 0..=3 {
        let mut p = player(mode);
        p.abilities.flying = true;
        p.abilities.mayfly = true;
        p.abilities.fly_speed = 0.25;
        vitals::respawned(&mut p);
        assert_eq!(p.abilities, playerdata::Abilities::for_mode(mode));
    }
}

#[test]
fn flying_prevents_critical_and_smash_attacks_and_elytra_gliding() {
    let shared = shared();
    let mut p = player(1);
    p.vit.fall = 10.0;
    p.on_ground = false;
    assert!(pvp::may_crit(&p));
    assert!(pvp::may_smash(&p, true));
    p.inventory.set(6, Some(Stack::new(blocks::item_named("elytra").unwrap(), 1)));
    p.abilities.flying = true;
    assert!(!pvp::may_crit(&p));
    assert!(!pvp::may_smash(&p, true));
    let mut command = encode_varint(p.entity_id);
    command.extend_from_slice(&encode_varint(6));
    glide::command(&command, &mut p, &shared);
    assert!(!p.glide.active);
    p.abilities.flying = false;
    glide::command(&command, &mut p, &shared);
    assert!(p.glide.active);
    p.abilities.flying = true;
    glide::tick(&shared, &mut p);
    assert!(!p.glide.active);
}

#[tokio::test]
async fn abilities_packets_send_saved_flags_and_both_speeds_in_all_modes() {
    for mode in 0..=3 {
        let mut p = player(mode);
        p.abilities.flying = p.abilities.mayfly;
        p.abilities.fly_speed = 0.125;
        p.abilities.walk_speed = 0.25;
        let mut out = Outgoing::capture();
        send_player_abilities(&mut out, &p).await.unwrap();
        let mut bytes = out.queued();
        let packet = read_packet(&mut bytes).await.unwrap();
        assert_eq!(packet.id, PLAYER_ABILITIES);
        let mut expected = vec![p.abilities.flags()];
        push_f32(&mut expected, 0.125);
        push_f32(&mut expected, 0.25);
        assert_eq!(packet.payload(), expected);
    }
}

#[test]
fn saved_abilities_changes_trigger_save_and_publish_for_explosions() {
    let shared = shared();
    let mut p = player(1);
    let mut p = reconnect(&mut p, "changed");
    assert!(!p.needs_save());
    let (member, _) = shared.players.lock().unwrap().join(p.member(), 391).unwrap();
    p.entity_id = member.entity_id;
    client_flags(&mut p, &shared, &[2]);
    assert!(p.needs_save());
    publish_player(&shared, &p);
    let targets = crate::tick::blast_targets(&shared, p.dimension);
    assert_eq!(targets.len(), 1);
    assert!(targets[0].flying_creative);
}
