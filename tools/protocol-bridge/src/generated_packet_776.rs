// Сгенерировано generate_packet_776.py из pinned JSON; packet-776-report.json.
// Только имена/ID/равенство графа схем; не разрешение payload/admission.
use crate::{JavaVersion, ids::RegistryKind};

pub const MISSING: u16 = u16::MAX;

pub fn names(kind: RegistryKind) -> Option<&'static [&'static str]> {
    match kind {
        RegistryKind::LoginClientbound => Some(&LOGINCLIENTBOUND_NAMES),
        RegistryKind::LoginServerbound => Some(&LOGINSERVERBOUND_NAMES),
        RegistryKind::ConfigurationClientbound => Some(&CONFIGURATIONCLIENTBOUND_NAMES),
        RegistryKind::ConfigurationServerbound => Some(&CONFIGURATIONSERVERBOUND_NAMES),
        RegistryKind::PlayClientbound => Some(&PLAYCLIENTBOUND_NAMES),
        RegistryKind::PlayServerbound => Some(&PLAYSERVERBOUND_NAMES),
        _ => None,
    }
}

pub fn route(kind: RegistryKind, from: JavaVersion, to: JavaVersion) -> Option<&'static [(u16, bool)]> {
    match (kind, from, to) {
        (RegistryKind::LoginClientbound, JavaVersion::V26_1_2, JavaVersion::V26_2) => Some(&LOGINCLIENTBOUND_775_776),
        (RegistryKind::LoginClientbound, JavaVersion::V26_2, JavaVersion::V26_1_2) => Some(&LOGINCLIENTBOUND_776_775),
        (RegistryKind::LoginClientbound, JavaVersion::V26_2, JavaVersion::V26_3) => Some(&LOGINCLIENTBOUND_776_777),
        (RegistryKind::LoginClientbound, JavaVersion::V26_3, JavaVersion::V26_2) => Some(&LOGINCLIENTBOUND_777_776),
        (RegistryKind::LoginServerbound, JavaVersion::V26_1_2, JavaVersion::V26_2) => Some(&LOGINSERVERBOUND_775_776),
        (RegistryKind::LoginServerbound, JavaVersion::V26_2, JavaVersion::V26_1_2) => Some(&LOGINSERVERBOUND_776_775),
        (RegistryKind::LoginServerbound, JavaVersion::V26_2, JavaVersion::V26_3) => Some(&LOGINSERVERBOUND_776_777),
        (RegistryKind::LoginServerbound, JavaVersion::V26_3, JavaVersion::V26_2) => Some(&LOGINSERVERBOUND_777_776),
        (RegistryKind::ConfigurationClientbound, JavaVersion::V26_1_2, JavaVersion::V26_2) => Some(&CONFIGURATIONCLIENTBOUND_775_776),
        (RegistryKind::ConfigurationClientbound, JavaVersion::V26_2, JavaVersion::V26_1_2) => Some(&CONFIGURATIONCLIENTBOUND_776_775),
        (RegistryKind::ConfigurationClientbound, JavaVersion::V26_2, JavaVersion::V26_3) => Some(&CONFIGURATIONCLIENTBOUND_776_777),
        (RegistryKind::ConfigurationClientbound, JavaVersion::V26_3, JavaVersion::V26_2) => Some(&CONFIGURATIONCLIENTBOUND_777_776),
        (RegistryKind::ConfigurationServerbound, JavaVersion::V26_1_2, JavaVersion::V26_2) => Some(&CONFIGURATIONSERVERBOUND_775_776),
        (RegistryKind::ConfigurationServerbound, JavaVersion::V26_2, JavaVersion::V26_1_2) => Some(&CONFIGURATIONSERVERBOUND_776_775),
        (RegistryKind::ConfigurationServerbound, JavaVersion::V26_2, JavaVersion::V26_3) => Some(&CONFIGURATIONSERVERBOUND_776_777),
        (RegistryKind::ConfigurationServerbound, JavaVersion::V26_3, JavaVersion::V26_2) => Some(&CONFIGURATIONSERVERBOUND_777_776),
        (RegistryKind::PlayClientbound, JavaVersion::V26_1_2, JavaVersion::V26_2) => Some(&PLAYCLIENTBOUND_775_776),
        (RegistryKind::PlayClientbound, JavaVersion::V26_2, JavaVersion::V26_1_2) => Some(&PLAYCLIENTBOUND_776_775),
        (RegistryKind::PlayClientbound, JavaVersion::V26_2, JavaVersion::V26_3) => Some(&PLAYCLIENTBOUND_776_777),
        (RegistryKind::PlayClientbound, JavaVersion::V26_3, JavaVersion::V26_2) => Some(&PLAYCLIENTBOUND_777_776),
        (RegistryKind::PlayServerbound, JavaVersion::V26_1_2, JavaVersion::V26_2) => Some(&PLAYSERVERBOUND_775_776),
        (RegistryKind::PlayServerbound, JavaVersion::V26_2, JavaVersion::V26_1_2) => Some(&PLAYSERVERBOUND_776_775),
        (RegistryKind::PlayServerbound, JavaVersion::V26_2, JavaVersion::V26_3) => Some(&PLAYSERVERBOUND_776_777),
        (RegistryKind::PlayServerbound, JavaVersion::V26_3, JavaVersion::V26_2) => Some(&PLAYSERVERBOUND_777_776),
        _ => None,
    }
}

static LOGINCLIENTBOUND_NAMES: [&str; 6] = [
    "disconnect",
    "encryption_begin",
    "success",
    "compress",
    "login_plugin_request",
    "cookie_request",
];

static LOGINCLIENTBOUND_775_776: [(u16, bool); 6] = [
    (0, true), // 0: disconnect
    (1, true), // 1: encryption_begin
    (2, false), // 2: success
    (3, true), // 3: compress
    (4, true), // 4: login_plugin_request
    (5, true), // 5: cookie_request
];

static LOGINCLIENTBOUND_776_775: [(u16, bool); 6] = [
    (0, true), // 0: disconnect
    (1, true), // 1: encryption_begin
    (2, false), // 2: success
    (3, true), // 3: compress
    (4, true), // 4: login_plugin_request
    (5, true), // 5: cookie_request
];

static LOGINCLIENTBOUND_776_777: [(u16, bool); 6] = [
    (0, true), // 0: disconnect
    (1, true), // 1: encryption_begin
    (2, true), // 2: success
    (3, true), // 3: compress
    (4, true), // 4: login_plugin_request
    (5, true), // 5: cookie_request
];

static LOGINCLIENTBOUND_777_776: [(u16, bool); 6] = [
    (0, true), // 0: disconnect
    (1, true), // 1: encryption_begin
    (2, true), // 2: success
    (3, true), // 3: compress
    (4, true), // 4: login_plugin_request
    (5, true), // 5: cookie_request
];

static LOGINSERVERBOUND_NAMES: [&str; 5] = [
    "login_start",
    "encryption_begin",
    "login_plugin_response",
    "login_acknowledged",
    "cookie_response",
];

static LOGINSERVERBOUND_775_776: [(u16, bool); 5] = [
    (0, true), // 0: login_start
    (1, true), // 1: encryption_begin
    (2, true), // 2: login_plugin_response
    (3, true), // 3: login_acknowledged
    (4, true), // 4: cookie_response
];

static LOGINSERVERBOUND_776_775: [(u16, bool); 5] = [
    (0, true), // 0: login_start
    (1, true), // 1: encryption_begin
    (2, true), // 2: login_plugin_response
    (3, true), // 3: login_acknowledged
    (4, true), // 4: cookie_response
];

static LOGINSERVERBOUND_776_777: [(u16, bool); 5] = [
    (0, true), // 0: login_start
    (1, true), // 1: encryption_begin
    (2, true), // 2: login_plugin_response
    (3, true), // 3: login_acknowledged
    (4, true), // 4: cookie_response
];

static LOGINSERVERBOUND_777_776: [(u16, bool); 5] = [
    (0, true), // 0: login_start
    (1, true), // 1: encryption_begin
    (2, true), // 2: login_plugin_response
    (3, true), // 3: login_acknowledged
    (4, true), // 4: cookie_response
];

static CONFIGURATIONCLIENTBOUND_NAMES: [&str; 20] = [
    "cookie_request",
    "custom_payload",
    "disconnect",
    "finish_configuration",
    "keep_alive",
    "ping",
    "reset_chat",
    "registry_data",
    "remove_resource_pack",
    "add_resource_pack",
    "store_cookie",
    "transfer",
    "feature_flags",
    "tags",
    "select_known_packs",
    "custom_report_details",
    "server_links",
    "clear_dialog",
    "show_dialog",
    "code_of_conduct",
];

static CONFIGURATIONCLIENTBOUND_775_776: [(u16, bool); 20] = [
    (0, true), // 0: cookie_request
    (1, true), // 1: custom_payload
    (2, true), // 2: disconnect
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: ping
    (6, true), // 6: reset_chat
    (7, true), // 7: registry_data
    (8, true), // 8: remove_resource_pack
    (9, true), // 9: add_resource_pack
    (10, true), // 10: store_cookie
    (11, true), // 11: transfer
    (12, true), // 12: feature_flags
    (13, true), // 13: tags
    (14, true), // 14: select_known_packs
    (15, true), // 15: custom_report_details
    (16, true), // 16: server_links
    (17, true), // 17: clear_dialog
    (18, true), // 18: show_dialog
    (19, true), // 19: code_of_conduct
];

static CONFIGURATIONCLIENTBOUND_776_775: [(u16, bool); 20] = [
    (0, true), // 0: cookie_request
    (1, true), // 1: custom_payload
    (2, true), // 2: disconnect
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: ping
    (6, true), // 6: reset_chat
    (7, true), // 7: registry_data
    (8, true), // 8: remove_resource_pack
    (9, true), // 9: add_resource_pack
    (10, true), // 10: store_cookie
    (11, true), // 11: transfer
    (12, true), // 12: feature_flags
    (13, true), // 13: tags
    (14, true), // 14: select_known_packs
    (15, true), // 15: custom_report_details
    (16, true), // 16: server_links
    (17, true), // 17: clear_dialog
    (18, true), // 18: show_dialog
    (19, true), // 19: code_of_conduct
];

static CONFIGURATIONCLIENTBOUND_776_777: [(u16, bool); 20] = [
    (0, true), // 0: cookie_request
    (1, true), // 1: custom_payload
    (2, true), // 2: disconnect
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: ping
    (6, true), // 6: reset_chat
    (7, true), // 7: registry_data
    (8, true), // 8: remove_resource_pack
    (9, true), // 9: add_resource_pack
    (11, true), // 10: store_cookie
    (12, true), // 11: transfer
    (13, true), // 12: feature_flags
    (14, true), // 13: tags
    (15, true), // 14: select_known_packs
    (16, true), // 15: custom_report_details
    (17, true), // 16: server_links
    (18, true), // 17: clear_dialog
    (19, true), // 18: show_dialog
    (20, true), // 19: code_of_conduct
];

static CONFIGURATIONCLIENTBOUND_777_776: [(u16, bool); 21] = [
    (0, true), // 0: cookie_request
    (1, true), // 1: custom_payload
    (2, true), // 2: disconnect
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: ping
    (6, true), // 6: reset_chat
    (7, true), // 7: registry_data
    (8, true), // 8: remove_resource_pack
    (9, true), // 9: add_resource_pack
    (65535, false), // 10: post_effects
    (10, true), // 11: store_cookie
    (11, true), // 12: transfer
    (12, true), // 13: feature_flags
    (13, true), // 14: tags
    (14, true), // 15: select_known_packs
    (15, true), // 16: custom_report_details
    (16, true), // 17: server_links
    (17, true), // 18: clear_dialog
    (18, true), // 19: show_dialog
    (19, true), // 20: code_of_conduct
];

static CONFIGURATIONSERVERBOUND_NAMES: [&str; 10] = [
    "settings",
    "cookie_response",
    "custom_payload",
    "finish_configuration",
    "keep_alive",
    "pong",
    "resource_pack_receive",
    "select_known_packs",
    "custom_click_action",
    "accept_code_of_conduct",
];

static CONFIGURATIONSERVERBOUND_775_776: [(u16, bool); 10] = [
    (0, true), // 0: settings
    (1, true), // 1: cookie_response
    (2, true), // 2: custom_payload
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: pong
    (6, true), // 6: resource_pack_receive
    (7, true), // 7: select_known_packs
    (8, true), // 8: custom_click_action
    (9, true), // 9: accept_code_of_conduct
];

static CONFIGURATIONSERVERBOUND_776_775: [(u16, bool); 10] = [
    (0, true), // 0: settings
    (1, true), // 1: cookie_response
    (2, true), // 2: custom_payload
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: pong
    (6, true), // 6: resource_pack_receive
    (7, true), // 7: select_known_packs
    (8, true), // 8: custom_click_action
    (9, true), // 9: accept_code_of_conduct
];

static CONFIGURATIONSERVERBOUND_776_777: [(u16, bool); 10] = [
    (0, true), // 0: settings
    (1, true), // 1: cookie_response
    (2, true), // 2: custom_payload
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: pong
    (6, true), // 6: resource_pack_receive
    (7, true), // 7: select_known_packs
    (8, true), // 8: custom_click_action
    (9, true), // 9: accept_code_of_conduct
];

static CONFIGURATIONSERVERBOUND_777_776: [(u16, bool); 10] = [
    (0, true), // 0: settings
    (1, true), // 1: cookie_response
    (2, true), // 2: custom_payload
    (3, true), // 3: finish_configuration
    (4, true), // 4: keep_alive
    (5, true), // 5: pong
    (6, true), // 6: resource_pack_receive
    (7, true), // 7: select_known_packs
    (8, true), // 8: custom_click_action
    (9, true), // 9: accept_code_of_conduct
];

static PLAYCLIENTBOUND_NAMES: [&str; 141] = [
    "bundle_delimiter",
    "spawn_entity",
    "animation",
    "statistics",
    "acknowledge_player_digging",
    "block_break_animation",
    "tile_entity_data",
    "block_action",
    "block_change",
    "boss_bar",
    "difficulty",
    "chunk_batch_finished",
    "chunk_batch_start",
    "chunk_biomes",
    "clear_titles",
    "tab_complete",
    "declare_commands",
    "close_window",
    "window_items",
    "craft_progress_bar",
    "set_slot",
    "cookie_request",
    "set_cooldown",
    "chat_suggestions",
    "custom_payload",
    "damage_event",
    "debug_block_value",
    "debug_chunk_value",
    "debug_entity_value",
    "debug_event",
    "debug_sample",
    "hide_message",
    "kick_disconnect",
    "profileless_chat",
    "entity_status",
    "sync_entity_position",
    "explosion",
    "unload_chunk",
    "game_state_change",
    "game_rule_values",
    "game_test_highlight_pos",
    "open_horse_window",
    "hurt_animation",
    "initialize_world_border",
    "keep_alive",
    "map_chunk",
    "world_event",
    "world_particles",
    "update_light",
    "login",
    "low_disk_space_warning",
    "map",
    "trade_list",
    "rel_entity_move",
    "entity_move_look",
    "move_minecart",
    "entity_look",
    "vehicle_move",
    "open_book",
    "open_window",
    "open_sign_entity",
    "ping",
    "ping_response",
    "craft_recipe_response",
    "abilities",
    "player_chat",
    "end_combat_event",
    "enter_combat_event",
    "death_combat_event",
    "player_remove",
    "player_info",
    "face_player",
    "position",
    "player_rotation",
    "recipe_book_add",
    "recipe_book_remove",
    "recipe_book_settings",
    "entity_destroy",
    "remove_entity_effect",
    "reset_score",
    "remove_resource_pack",
    "add_resource_pack",
    "respawn",
    "entity_head_rotation",
    "multi_block_change",
    "select_advancement_tab",
    "server_data",
    "action_bar",
    "world_border_center",
    "world_border_lerp_size",
    "world_border_size",
    "world_border_warning_delay",
    "world_border_warning_reach",
    "camera",
    "update_view_position",
    "update_view_distance",
    "set_cursor_item",
    "spawn_position",
    "scoreboard_display_objective",
    "entity_metadata",
    "attach_entity",
    "entity_velocity",
    "entity_equipment",
    "experience",
    "update_health",
    "held_item_slot",
    "scoreboard_objective",
    "set_passengers",
    "set_player_inventory",
    "teams",
    "scoreboard_score",
    "simulation_distance",
    "set_title_subtitle",
    "update_time",
    "set_title_text",
    "set_title_time",
    "entity_sound_effect",
    "sound_effect",
    "start_configuration",
    "stop_sound",
    "store_cookie",
    "system_chat",
    "playerlist_header",
    "nbt_query_response",
    "collect",
    "entity_teleport",
    "test_instance_block_status",
    "set_ticking_state",
    "step_tick",
    "transfer",
    "advancements",
    "entity_update_attributes",
    "entity_effect",
    "declare_recipes",
    "tags",
    "set_projectile_power",
    "custom_report_details",
    "server_links",
    "tracked_waypoint",
    "clear_dialog",
    "show_dialog",
];

static PLAYCLIENTBOUND_775_776: [(u16, bool); 141] = [
    (0, true), // 0: bundle_delimiter
    (1, true), // 1: spawn_entity
    (2, true), // 2: animation
    (3, true), // 3: statistics
    (4, true), // 4: acknowledge_player_digging
    (5, true), // 5: block_break_animation
    (6, true), // 6: tile_entity_data
    (7, true), // 7: block_action
    (8, true), // 8: block_change
    (9, true), // 9: boss_bar
    (10, true), // 10: difficulty
    (11, true), // 11: chunk_batch_finished
    (12, true), // 12: chunk_batch_start
    (13, true), // 13: chunk_biomes
    (14, true), // 14: clear_titles
    (15, true), // 15: tab_complete
    (16, true), // 16: declare_commands
    (17, true), // 17: close_window
    (18, false), // 18: window_items
    (19, true), // 19: craft_progress_bar
    (20, false), // 20: set_slot
    (21, true), // 21: cookie_request
    (22, true), // 22: set_cooldown
    (23, true), // 23: chat_suggestions
    (24, true), // 24: custom_payload
    (25, true), // 25: damage_event
    (26, true), // 26: debug_block_value
    (27, true), // 27: debug_chunk_value
    (28, true), // 28: debug_entity_value
    (29, true), // 29: debug_event
    (30, true), // 30: debug_sample
    (31, true), // 31: hide_message
    (32, true), // 32: kick_disconnect
    (33, true), // 33: profileless_chat
    (34, true), // 34: entity_status
    (35, true), // 35: sync_entity_position
    (36, false), // 36: explosion
    (37, true), // 37: unload_chunk
    (38, true), // 38: game_state_change
    (39, true), // 39: game_rule_values
    (40, true), // 40: game_test_highlight_pos
    (41, true), // 41: open_horse_window
    (42, true), // 42: hurt_animation
    (43, true), // 43: initialize_world_border
    (44, true), // 44: keep_alive
    (45, true), // 45: map_chunk
    (46, true), // 46: world_event
    (47, false), // 47: world_particles
    (48, true), // 48: update_light
    (49, false), // 49: login
    (50, true), // 50: low_disk_space_warning
    (51, true), // 51: map
    (52, false), // 52: trade_list
    (53, true), // 53: rel_entity_move
    (54, true), // 54: entity_move_look
    (55, true), // 55: move_minecart
    (56, true), // 56: entity_look
    (57, true), // 57: vehicle_move
    (58, true), // 58: open_book
    (59, true), // 59: open_window
    (60, true), // 60: open_sign_entity
    (61, true), // 61: ping
    (62, true), // 62: ping_response
    (63, false), // 63: craft_recipe_response
    (64, true), // 64: abilities
    (65, true), // 65: player_chat
    (66, true), // 66: end_combat_event
    (67, true), // 67: enter_combat_event
    (68, true), // 68: death_combat_event
    (69, true), // 69: player_remove
    (70, true), // 70: player_info
    (71, true), // 71: face_player
    (72, true), // 72: position
    (73, true), // 73: player_rotation
    (74, false), // 74: recipe_book_add
    (75, true), // 75: recipe_book_remove
    (76, true), // 76: recipe_book_settings
    (77, true), // 77: entity_destroy
    (78, true), // 78: remove_entity_effect
    (79, true), // 79: reset_score
    (80, true), // 80: remove_resource_pack
    (81, true), // 81: add_resource_pack
    (82, true), // 82: respawn
    (83, true), // 83: entity_head_rotation
    (84, true), // 84: multi_block_change
    (85, true), // 85: select_advancement_tab
    (86, true), // 86: server_data
    (87, true), // 87: action_bar
    (88, true), // 88: world_border_center
    (89, true), // 89: world_border_lerp_size
    (90, true), // 90: world_border_size
    (91, true), // 91: world_border_warning_delay
    (92, true), // 92: world_border_warning_reach
    (93, true), // 93: camera
    (94, true), // 94: update_view_position
    (95, true), // 95: update_view_distance
    (96, false), // 96: set_cursor_item
    (97, true), // 97: spawn_position
    (98, true), // 98: scoreboard_display_objective
    (99, false), // 99: entity_metadata
    (100, true), // 100: attach_entity
    (101, true), // 101: entity_velocity
    (102, false), // 102: entity_equipment
    (103, true), // 103: experience
    (104, true), // 104: update_health
    (105, true), // 105: held_item_slot
    (106, true), // 106: scoreboard_objective
    (107, true), // 107: set_passengers
    (108, false), // 108: set_player_inventory
    (109, false), // 109: teams
    (110, true), // 110: scoreboard_score
    (111, true), // 111: simulation_distance
    (112, true), // 112: set_title_subtitle
    (113, true), // 113: update_time
    (114, true), // 114: set_title_text
    (115, true), // 115: set_title_time
    (116, true), // 116: entity_sound_effect
    (117, true), // 117: sound_effect
    (118, true), // 118: start_configuration
    (119, true), // 119: stop_sound
    (120, true), // 120: store_cookie
    (121, true), // 121: system_chat
    (122, true), // 122: playerlist_header
    (123, true), // 123: nbt_query_response
    (124, true), // 124: collect
    (125, true), // 125: entity_teleport
    (126, true), // 126: test_instance_block_status
    (127, true), // 127: set_ticking_state
    (128, true), // 128: step_tick
    (129, true), // 129: transfer
    (130, false), // 130: advancements
    (131, false), // 131: entity_update_attributes
    (132, true), // 132: entity_effect
    (133, false), // 133: declare_recipes
    (134, true), // 134: tags
    (135, true), // 135: set_projectile_power
    (136, true), // 136: custom_report_details
    (137, true), // 137: server_links
    (138, true), // 138: tracked_waypoint
    (139, true), // 139: clear_dialog
    (140, true), // 140: show_dialog
];

static PLAYCLIENTBOUND_776_775: [(u16, bool); 141] = [
    (0, true), // 0: bundle_delimiter
    (1, true), // 1: spawn_entity
    (2, true), // 2: animation
    (3, true), // 3: statistics
    (4, true), // 4: acknowledge_player_digging
    (5, true), // 5: block_break_animation
    (6, true), // 6: tile_entity_data
    (7, true), // 7: block_action
    (8, true), // 8: block_change
    (9, true), // 9: boss_bar
    (10, true), // 10: difficulty
    (11, true), // 11: chunk_batch_finished
    (12, true), // 12: chunk_batch_start
    (13, true), // 13: chunk_biomes
    (14, true), // 14: clear_titles
    (15, true), // 15: tab_complete
    (16, true), // 16: declare_commands
    (17, true), // 17: close_window
    (18, false), // 18: window_items
    (19, true), // 19: craft_progress_bar
    (20, false), // 20: set_slot
    (21, true), // 21: cookie_request
    (22, true), // 22: set_cooldown
    (23, true), // 23: chat_suggestions
    (24, true), // 24: custom_payload
    (25, true), // 25: damage_event
    (26, true), // 26: debug_block_value
    (27, true), // 27: debug_chunk_value
    (28, true), // 28: debug_entity_value
    (29, true), // 29: debug_event
    (30, true), // 30: debug_sample
    (31, true), // 31: hide_message
    (32, true), // 32: kick_disconnect
    (33, true), // 33: profileless_chat
    (34, true), // 34: entity_status
    (35, true), // 35: sync_entity_position
    (36, false), // 36: explosion
    (37, true), // 37: unload_chunk
    (38, true), // 38: game_state_change
    (39, true), // 39: game_rule_values
    (40, true), // 40: game_test_highlight_pos
    (41, true), // 41: open_horse_window
    (42, true), // 42: hurt_animation
    (43, true), // 43: initialize_world_border
    (44, true), // 44: keep_alive
    (45, true), // 45: map_chunk
    (46, true), // 46: world_event
    (47, false), // 47: world_particles
    (48, true), // 48: update_light
    (49, false), // 49: login
    (50, true), // 50: low_disk_space_warning
    (51, true), // 51: map
    (52, false), // 52: trade_list
    (53, true), // 53: rel_entity_move
    (54, true), // 54: entity_move_look
    (55, true), // 55: move_minecart
    (56, true), // 56: entity_look
    (57, true), // 57: vehicle_move
    (58, true), // 58: open_book
    (59, true), // 59: open_window
    (60, true), // 60: open_sign_entity
    (61, true), // 61: ping
    (62, true), // 62: ping_response
    (63, false), // 63: craft_recipe_response
    (64, true), // 64: abilities
    (65, true), // 65: player_chat
    (66, true), // 66: end_combat_event
    (67, true), // 67: enter_combat_event
    (68, true), // 68: death_combat_event
    (69, true), // 69: player_remove
    (70, true), // 70: player_info
    (71, true), // 71: face_player
    (72, true), // 72: position
    (73, true), // 73: player_rotation
    (74, false), // 74: recipe_book_add
    (75, true), // 75: recipe_book_remove
    (76, true), // 76: recipe_book_settings
    (77, true), // 77: entity_destroy
    (78, true), // 78: remove_entity_effect
    (79, true), // 79: reset_score
    (80, true), // 80: remove_resource_pack
    (81, true), // 81: add_resource_pack
    (82, true), // 82: respawn
    (83, true), // 83: entity_head_rotation
    (84, true), // 84: multi_block_change
    (85, true), // 85: select_advancement_tab
    (86, true), // 86: server_data
    (87, true), // 87: action_bar
    (88, true), // 88: world_border_center
    (89, true), // 89: world_border_lerp_size
    (90, true), // 90: world_border_size
    (91, true), // 91: world_border_warning_delay
    (92, true), // 92: world_border_warning_reach
    (93, true), // 93: camera
    (94, true), // 94: update_view_position
    (95, true), // 95: update_view_distance
    (96, false), // 96: set_cursor_item
    (97, true), // 97: spawn_position
    (98, true), // 98: scoreboard_display_objective
    (99, false), // 99: entity_metadata
    (100, true), // 100: attach_entity
    (101, true), // 101: entity_velocity
    (102, false), // 102: entity_equipment
    (103, true), // 103: experience
    (104, true), // 104: update_health
    (105, true), // 105: held_item_slot
    (106, true), // 106: scoreboard_objective
    (107, true), // 107: set_passengers
    (108, false), // 108: set_player_inventory
    (109, false), // 109: teams
    (110, true), // 110: scoreboard_score
    (111, true), // 111: simulation_distance
    (112, true), // 112: set_title_subtitle
    (113, true), // 113: update_time
    (114, true), // 114: set_title_text
    (115, true), // 115: set_title_time
    (116, true), // 116: entity_sound_effect
    (117, true), // 117: sound_effect
    (118, true), // 118: start_configuration
    (119, true), // 119: stop_sound
    (120, true), // 120: store_cookie
    (121, true), // 121: system_chat
    (122, true), // 122: playerlist_header
    (123, true), // 123: nbt_query_response
    (124, true), // 124: collect
    (125, true), // 125: entity_teleport
    (126, true), // 126: test_instance_block_status
    (127, true), // 127: set_ticking_state
    (128, true), // 128: step_tick
    (129, true), // 129: transfer
    (130, false), // 130: advancements
    (131, false), // 131: entity_update_attributes
    (132, true), // 132: entity_effect
    (133, false), // 133: declare_recipes
    (134, true), // 134: tags
    (135, true), // 135: set_projectile_power
    (136, true), // 136: custom_report_details
    (137, true), // 137: server_links
    (138, true), // 138: tracked_waypoint
    (139, true), // 139: clear_dialog
    (140, true), // 140: show_dialog
];

static PLAYCLIENTBOUND_776_777: [(u16, bool); 141] = [
    (0, true), // 0: bundle_delimiter
    (1, true), // 1: spawn_entity
    (2, true), // 2: animation
    (3, true), // 3: statistics
    (4, true), // 4: acknowledge_player_digging
    (5, true), // 5: block_break_animation
    (6, true), // 6: tile_entity_data
    (7, true), // 7: block_action
    (8, true), // 8: block_change
    (9, true), // 9: boss_bar
    (10, true), // 10: difficulty
    (11, true), // 11: chunk_batch_finished
    (12, true), // 12: chunk_batch_start
    (13, true), // 13: chunk_biomes
    (14, true), // 14: clear_titles
    (15, true), // 15: tab_complete
    (16, true), // 16: declare_commands
    (17, true), // 17: close_window
    (18, false), // 18: window_items
    (19, true), // 19: craft_progress_bar
    (20, false), // 20: set_slot
    (21, true), // 21: cookie_request
    (22, true), // 22: set_cooldown
    (23, true), // 23: chat_suggestions
    (24, true), // 24: custom_payload
    (25, true), // 25: damage_event
    (26, true), // 26: debug_block_value
    (27, true), // 27: debug_chunk_value
    (28, true), // 28: debug_entity_value
    (29, true), // 29: debug_event
    (30, true), // 30: debug_sample
    (31, true), // 31: hide_message
    (32, true), // 32: kick_disconnect
    (33, true), // 33: profileless_chat
    (34, true), // 34: entity_status
    (35, false), // 35: sync_entity_position
    (36, false), // 36: explosion
    (38, true), // 37: unload_chunk
    (39, true), // 38: game_state_change
    (40, true), // 39: game_rule_values
    (41, true), // 40: game_test_highlight_pos
    (42, true), // 41: open_horse_window
    (43, true), // 42: hurt_animation
    (44, true), // 43: initialize_world_border
    (45, true), // 44: keep_alive
    (46, false), // 45: map_chunk
    (47, true), // 46: world_event
    (48, false), // 47: world_particles
    (49, false), // 48: update_light
    (50, true), // 49: login
    (51, true), // 50: low_disk_space_warning
    (52, true), // 51: map
    (53, false), // 52: trade_list
    (54, false), // 53: rel_entity_move
    (55, false), // 54: entity_move_look
    (56, true), // 55: move_minecart
    (57, true), // 56: entity_look
    (58, true), // 57: vehicle_move
    (59, true), // 58: open_book
    (60, true), // 59: open_window
    (61, true), // 60: open_sign_entity
    (62, true), // 61: ping
    (63, true), // 62: ping_response
    (64, false), // 63: craft_recipe_response
    (65, true), // 64: abilities
    (66, true), // 65: player_chat
    (67, true), // 66: end_combat_event
    (68, true), // 67: enter_combat_event
    (69, true), // 68: death_combat_event
    (70, true), // 69: player_remove
    (71, true), // 70: player_info
    (72, true), // 71: face_player
    (73, true), // 72: position
    (74, true), // 73: player_rotation
    (75, false), // 74: recipe_book_add
    (76, true), // 75: recipe_book_remove
    (77, true), // 76: recipe_book_settings
    (78, true), // 77: entity_destroy
    (79, true), // 78: remove_entity_effect
    (80, true), // 79: reset_score
    (81, true), // 80: remove_resource_pack
    (82, true), // 81: add_resource_pack
    (84, true), // 82: respawn
    (85, true), // 83: entity_head_rotation
    (86, true), // 84: multi_block_change
    (87, true), // 85: select_advancement_tab
    (88, true), // 86: server_data
    (89, true), // 87: action_bar
    (90, true), // 88: world_border_center
    (91, true), // 89: world_border_lerp_size
    (92, true), // 90: world_border_size
    (93, true), // 91: world_border_warning_delay
    (94, true), // 92: world_border_warning_reach
    (95, true), // 93: camera
    (96, true), // 94: update_view_position
    (97, true), // 95: update_view_distance
    (98, false), // 96: set_cursor_item
    (99, true), // 97: spawn_position
    (100, true), // 98: scoreboard_display_objective
    (101, false), // 99: entity_metadata
    (102, true), // 100: attach_entity
    (103, true), // 101: entity_velocity
    (104, false), // 102: entity_equipment
    (105, true), // 103: experience
    (106, true), // 104: update_health
    (107, true), // 105: held_item_slot
    (108, true), // 106: scoreboard_objective
    (109, true), // 107: set_passengers
    (110, false), // 108: set_player_inventory
    (111, false), // 109: teams
    (112, true), // 110: scoreboard_score
    (113, true), // 111: simulation_distance
    (114, true), // 112: set_title_subtitle
    (115, true), // 113: update_time
    (116, true), // 114: set_title_text
    (117, true), // 115: set_title_time
    (118, true), // 116: entity_sound_effect
    (119, true), // 117: sound_effect
    (120, true), // 118: start_configuration
    (121, true), // 119: stop_sound
    (122, true), // 120: store_cookie
    (124, true), // 121: system_chat
    (125, true), // 122: playerlist_header
    (126, true), // 123: nbt_query_response
    (127, true), // 124: collect
    (128, true), // 125: entity_teleport
    (129, true), // 126: test_instance_block_status
    (130, true), // 127: set_ticking_state
    (131, true), // 128: step_tick
    (132, true), // 129: transfer
    (133, false), // 130: advancements
    (134, true), // 131: entity_update_attributes
    (135, true), // 132: entity_effect
    (136, false), // 133: declare_recipes
    (137, true), // 134: tags
    (138, true), // 135: set_projectile_power
    (139, true), // 136: custom_report_details
    (140, true), // 137: server_links
    (141, true), // 138: tracked_waypoint
    (142, true), // 139: clear_dialog
    (143, true), // 140: show_dialog
];

static PLAYCLIENTBOUND_777_776: [(u16, bool); 144] = [
    (0, true), // 0: bundle_delimiter
    (1, true), // 1: spawn_entity
    (2, true), // 2: animation
    (3, true), // 3: statistics
    (4, true), // 4: acknowledge_player_digging
    (5, true), // 5: block_break_animation
    (6, true), // 6: tile_entity_data
    (7, true), // 7: block_action
    (8, true), // 8: block_change
    (9, true), // 9: boss_bar
    (10, true), // 10: difficulty
    (11, true), // 11: chunk_batch_finished
    (12, true), // 12: chunk_batch_start
    (13, true), // 13: chunk_biomes
    (14, true), // 14: clear_titles
    (15, true), // 15: tab_complete
    (16, true), // 16: declare_commands
    (17, true), // 17: close_window
    (18, false), // 18: window_items
    (19, true), // 19: craft_progress_bar
    (20, false), // 20: set_slot
    (21, true), // 21: cookie_request
    (22, true), // 22: set_cooldown
    (23, true), // 23: chat_suggestions
    (24, true), // 24: custom_payload
    (25, true), // 25: damage_event
    (26, true), // 26: debug_block_value
    (27, true), // 27: debug_chunk_value
    (28, true), // 28: debug_entity_value
    (29, true), // 29: debug_event
    (30, true), // 30: debug_sample
    (31, true), // 31: hide_message
    (32, true), // 32: kick_disconnect
    (33, true), // 33: profileless_chat
    (34, true), // 34: entity_status
    (35, false), // 35: sync_entity_position
    (36, false), // 36: explosion
    (65535, false), // 37: add_transient_block
    (37, true), // 38: unload_chunk
    (38, true), // 39: game_state_change
    (39, true), // 40: game_rule_values
    (40, true), // 41: game_test_highlight_pos
    (41, true), // 42: open_horse_window
    (42, true), // 43: hurt_animation
    (43, true), // 44: initialize_world_border
    (44, true), // 45: keep_alive
    (45, false), // 46: map_chunk
    (46, true), // 47: world_event
    (47, false), // 48: world_particles
    (48, false), // 49: update_light
    (49, true), // 50: login
    (50, true), // 51: low_disk_space_warning
    (51, true), // 52: map
    (52, false), // 53: trade_list
    (53, false), // 54: rel_entity_move
    (54, false), // 55: entity_move_look
    (55, true), // 56: move_minecart
    (56, true), // 57: entity_look
    (57, true), // 58: vehicle_move
    (58, true), // 59: open_book
    (59, true), // 60: open_window
    (60, true), // 61: open_sign_entity
    (61, true), // 62: ping
    (62, true), // 63: ping_response
    (63, false), // 64: craft_recipe_response
    (64, true), // 65: abilities
    (65, true), // 66: player_chat
    (66, true), // 67: end_combat_event
    (67, true), // 68: enter_combat_event
    (68, true), // 69: death_combat_event
    (69, true), // 70: player_remove
    (70, true), // 71: player_info
    (71, true), // 72: face_player
    (72, true), // 73: position
    (73, true), // 74: player_rotation
    (74, false), // 75: recipe_book_add
    (75, true), // 76: recipe_book_remove
    (76, true), // 77: recipe_book_settings
    (77, true), // 78: entity_destroy
    (78, true), // 79: remove_entity_effect
    (79, true), // 80: reset_score
    (80, true), // 81: remove_resource_pack
    (81, true), // 82: add_resource_pack
    (65535, false), // 83: post_effects
    (82, true), // 84: respawn
    (83, true), // 85: entity_head_rotation
    (84, true), // 86: multi_block_change
    (85, true), // 87: select_advancement_tab
    (86, true), // 88: server_data
    (87, true), // 89: action_bar
    (88, true), // 90: world_border_center
    (89, true), // 91: world_border_lerp_size
    (90, true), // 92: world_border_size
    (91, true), // 93: world_border_warning_delay
    (92, true), // 94: world_border_warning_reach
    (93, true), // 95: camera
    (94, true), // 96: update_view_position
    (95, true), // 97: update_view_distance
    (96, false), // 98: set_cursor_item
    (97, true), // 99: spawn_position
    (98, true), // 100: scoreboard_display_objective
    (99, false), // 101: entity_metadata
    (100, true), // 102: attach_entity
    (101, true), // 103: entity_velocity
    (102, false), // 104: entity_equipment
    (103, true), // 105: experience
    (104, true), // 106: update_health
    (105, true), // 107: held_item_slot
    (106, true), // 108: scoreboard_objective
    (107, true), // 109: set_passengers
    (108, false), // 110: set_player_inventory
    (109, false), // 111: teams
    (110, true), // 112: scoreboard_score
    (111, true), // 113: simulation_distance
    (112, true), // 114: set_title_subtitle
    (113, true), // 115: update_time
    (114, true), // 116: set_title_text
    (115, true), // 117: set_title_time
    (116, true), // 118: entity_sound_effect
    (117, true), // 119: sound_effect
    (118, true), // 120: start_configuration
    (119, true), // 121: stop_sound
    (120, true), // 122: store_cookie
    (65535, false), // 123: swing_animation
    (121, true), // 124: system_chat
    (122, true), // 125: playerlist_header
    (123, true), // 126: nbt_query_response
    (124, true), // 127: collect
    (125, true), // 128: entity_teleport
    (126, true), // 129: test_instance_block_status
    (127, true), // 130: set_ticking_state
    (128, true), // 131: step_tick
    (129, true), // 132: transfer
    (130, false), // 133: advancements
    (131, true), // 134: entity_update_attributes
    (132, true), // 135: entity_effect
    (133, false), // 136: declare_recipes
    (134, true), // 137: tags
    (135, true), // 138: set_projectile_power
    (136, true), // 139: custom_report_details
    (137, true), // 140: server_links
    (138, true), // 141: tracked_waypoint
    (139, true), // 142: clear_dialog
    (140, true), // 143: show_dialog
];

static PLAYSERVERBOUND_NAMES: [&str; 69] = [
    "teleport_confirm",
    "attack",
    "query_block_nbt",
    "select_bundle_item",
    "set_difficulty",
    "change_gamemode",
    "message_acknowledgement",
    "chat_command",
    "chat_command_signed",
    "chat_message",
    "chat_session_update",
    "chunk_batch_received",
    "client_command",
    "tick_end",
    "settings",
    "tab_complete",
    "configuration_acknowledged",
    "enchant_item",
    "window_click",
    "close_window",
    "set_slot_state",
    "cookie_response",
    "custom_payload",
    "debug_subscription_request",
    "edit_book",
    "query_entity_nbt",
    "use_entity",
    "generate_structure",
    "keep_alive",
    "lock_difficulty",
    "position",
    "position_look",
    "look",
    "flying",
    "vehicle_move",
    "steer_boat",
    "pick_item_from_block",
    "pick_item_from_entity",
    "ping_request",
    "craft_recipe_request",
    "abilities",
    "block_dig",
    "entity_action",
    "player_input",
    "player_loaded",
    "pong",
    "recipe_book",
    "displayed_recipe",
    "name_item",
    "resource_pack_receive",
    "advancement_tab",
    "select_trade",
    "set_beacon_effect",
    "held_item_slot",
    "update_command_block",
    "update_command_block_minecart",
    "set_creative_slot",
    "set_game_rule",
    "update_jigsaw_block",
    "update_structure_block",
    "set_test_block",
    "update_sign",
    "spectator_action",
    "arm_animation",
    "spectate",
    "test_instance_block_action",
    "block_place",
    "use_item",
    "custom_click_action",
];

static PLAYSERVERBOUND_775_776: [(u16, bool); 69] = [
    (0, true), // 0: teleport_confirm
    (1, true), // 1: attack
    (2, true), // 2: query_block_nbt
    (3, true), // 3: select_bundle_item
    (4, true), // 4: set_difficulty
    (5, true), // 5: change_gamemode
    (6, true), // 6: message_acknowledgement
    (7, true), // 7: chat_command
    (8, true), // 8: chat_command_signed
    (9, true), // 9: chat_message
    (10, true), // 10: chat_session_update
    (11, true), // 11: chunk_batch_received
    (12, true), // 12: client_command
    (13, true), // 13: tick_end
    (14, true), // 14: settings
    (15, true), // 15: tab_complete
    (16, true), // 16: configuration_acknowledged
    (17, true), // 17: enchant_item
    (18, false), // 18: window_click
    (19, true), // 19: close_window
    (20, true), // 20: set_slot_state
    (21, true), // 21: cookie_response
    (22, true), // 22: custom_payload
    (23, true), // 23: debug_subscription_request
    (24, true), // 24: edit_book
    (25, true), // 25: query_entity_nbt
    (26, true), // 26: use_entity
    (27, true), // 27: generate_structure
    (28, true), // 28: keep_alive
    (29, true), // 29: lock_difficulty
    (30, true), // 30: position
    (31, true), // 31: position_look
    (32, true), // 32: look
    (33, true), // 33: flying
    (34, true), // 34: vehicle_move
    (35, true), // 35: steer_boat
    (36, true), // 36: pick_item_from_block
    (37, true), // 37: pick_item_from_entity
    (38, true), // 38: ping_request
    (39, true), // 39: craft_recipe_request
    (40, true), // 40: abilities
    (41, true), // 41: block_dig
    (42, true), // 42: entity_action
    (43, true), // 43: player_input
    (44, true), // 44: player_loaded
    (45, true), // 45: pong
    (46, true), // 46: recipe_book
    (47, true), // 47: displayed_recipe
    (48, true), // 48: name_item
    (49, true), // 49: resource_pack_receive
    (50, true), // 50: advancement_tab
    (51, true), // 51: select_trade
    (52, true), // 52: set_beacon_effect
    (53, true), // 53: held_item_slot
    (54, true), // 54: update_command_block
    (55, true), // 55: update_command_block_minecart
    (56, false), // 56: set_creative_slot
    (57, true), // 57: set_game_rule
    (58, true), // 58: update_jigsaw_block
    (59, true), // 59: update_structure_block
    (60, true), // 60: set_test_block
    (61, true), // 61: update_sign
    (65535, false), // 62: spectate_entity
    (63, true), // 63: arm_animation
    (64, true), // 64: spectate
    (65, true), // 65: test_instance_block_action
    (66, true), // 66: block_place
    (67, true), // 67: use_item
    (68, true), // 68: custom_click_action
];

static PLAYSERVERBOUND_776_775: [(u16, bool); 69] = [
    (0, true), // 0: teleport_confirm
    (1, true), // 1: attack
    (2, true), // 2: query_block_nbt
    (3, true), // 3: select_bundle_item
    (4, true), // 4: set_difficulty
    (5, true), // 5: change_gamemode
    (6, true), // 6: message_acknowledgement
    (7, true), // 7: chat_command
    (8, true), // 8: chat_command_signed
    (9, true), // 9: chat_message
    (10, true), // 10: chat_session_update
    (11, true), // 11: chunk_batch_received
    (12, true), // 12: client_command
    (13, true), // 13: tick_end
    (14, true), // 14: settings
    (15, true), // 15: tab_complete
    (16, true), // 16: configuration_acknowledged
    (17, true), // 17: enchant_item
    (18, false), // 18: window_click
    (19, true), // 19: close_window
    (20, true), // 20: set_slot_state
    (21, true), // 21: cookie_response
    (22, true), // 22: custom_payload
    (23, true), // 23: debug_subscription_request
    (24, true), // 24: edit_book
    (25, true), // 25: query_entity_nbt
    (26, true), // 26: use_entity
    (27, true), // 27: generate_structure
    (28, true), // 28: keep_alive
    (29, true), // 29: lock_difficulty
    (30, true), // 30: position
    (31, true), // 31: position_look
    (32, true), // 32: look
    (33, true), // 33: flying
    (34, true), // 34: vehicle_move
    (35, true), // 35: steer_boat
    (36, true), // 36: pick_item_from_block
    (37, true), // 37: pick_item_from_entity
    (38, true), // 38: ping_request
    (39, true), // 39: craft_recipe_request
    (40, true), // 40: abilities
    (41, true), // 41: block_dig
    (42, true), // 42: entity_action
    (43, true), // 43: player_input
    (44, true), // 44: player_loaded
    (45, true), // 45: pong
    (46, true), // 46: recipe_book
    (47, true), // 47: displayed_recipe
    (48, true), // 48: name_item
    (49, true), // 49: resource_pack_receive
    (50, true), // 50: advancement_tab
    (51, true), // 51: select_trade
    (52, true), // 52: set_beacon_effect
    (53, true), // 53: held_item_slot
    (54, true), // 54: update_command_block
    (55, true), // 55: update_command_block_minecart
    (56, false), // 56: set_creative_slot
    (57, true), // 57: set_game_rule
    (58, true), // 58: update_jigsaw_block
    (59, true), // 59: update_structure_block
    (60, true), // 60: set_test_block
    (61, true), // 61: update_sign
    (65535, false), // 62: spectator_action
    (63, true), // 63: arm_animation
    (64, true), // 64: spectate
    (65, true), // 65: test_instance_block_action
    (66, true), // 66: block_place
    (67, true), // 67: use_item
    (68, true), // 68: custom_click_action
];

static PLAYSERVERBOUND_776_777: [(u16, bool); 69] = [
    (0, false), // 0: teleport_confirm
    (1, true), // 1: attack
    (2, true), // 2: query_block_nbt
    (3, true), // 3: select_bundle_item
    (4, true), // 4: set_difficulty
    (5, true), // 5: change_gamemode
    (6, true), // 6: message_acknowledgement
    (7, true), // 7: chat_command
    (8, true), // 8: chat_command_signed
    (9, true), // 9: chat_message
    (10, true), // 10: chat_session_update
    (11, true), // 11: chunk_batch_received
    (12, true), // 12: client_command
    (13, true), // 13: tick_end
    (14, true), // 14: settings
    (15, true), // 15: tab_complete
    (16, true), // 16: configuration_acknowledged
    (17, true), // 17: enchant_item
    (18, false), // 18: window_click
    (19, true), // 19: close_window
    (20, true), // 20: set_slot_state
    (21, true), // 21: cookie_response
    (22, true), // 22: custom_payload
    (23, true), // 23: debug_subscription_request
    (24, true), // 24: edit_book
    (25, true), // 25: query_entity_nbt
    (26, true), // 26: use_entity
    (27, true), // 27: generate_structure
    (28, true), // 28: keep_alive
    (29, true), // 29: lock_difficulty
    (30, true), // 30: position
    (31, true), // 31: position_look
    (32, true), // 32: look
    (33, true), // 33: flying
    (34, true), // 34: vehicle_move
    (35, true), // 35: steer_boat
    (36, true), // 36: pick_item_from_block
    (37, true), // 37: pick_item_from_entity
    (38, true), // 38: ping_request
    (39, true), // 39: craft_recipe_request
    (40, true), // 40: abilities
    (41, true), // 41: block_dig
    (42, true), // 42: entity_action
    (43, true), // 43: player_input
    (44, true), // 44: player_loaded
    (45, true), // 45: pong
    (47, true), // 46: recipe_book
    (48, true), // 47: displayed_recipe
    (49, true), // 48: name_item
    (50, true), // 49: resource_pack_receive
    (51, true), // 50: advancement_tab
    (52, true), // 51: select_trade
    (53, true), // 52: set_beacon_effect
    (54, true), // 53: held_item_slot
    (55, true), // 54: update_command_block
    (56, true), // 55: update_command_block_minecart
    (57, false), // 56: set_creative_slot
    (58, true), // 57: set_game_rule
    (59, true), // 58: update_jigsaw_block
    (60, true), // 59: update_structure_block
    (61, true), // 60: set_test_block
    (62, true), // 61: update_sign
    (65535, false), // 62: spectator_action
    (63, true), // 63: arm_animation
    (64, true), // 64: spectate
    (65, true), // 65: test_instance_block_action
    (66, true), // 66: block_place
    (67, true), // 67: use_item
    (68, true), // 68: custom_click_action
];

static PLAYSERVERBOUND_777_776: [(u16, bool); 69] = [
    (0, false), // 0: teleport_confirm
    (1, true), // 1: attack
    (2, true), // 2: query_block_nbt
    (3, true), // 3: select_bundle_item
    (4, true), // 4: set_difficulty
    (5, true), // 5: change_gamemode
    (6, true), // 6: message_acknowledgement
    (7, true), // 7: chat_command
    (8, true), // 8: chat_command_signed
    (9, true), // 9: chat_message
    (10, true), // 10: chat_session_update
    (11, true), // 11: chunk_batch_received
    (12, true), // 12: client_command
    (13, true), // 13: tick_end
    (14, true), // 14: settings
    (15, true), // 15: tab_complete
    (16, true), // 16: configuration_acknowledged
    (17, true), // 17: enchant_item
    (18, false), // 18: window_click
    (19, true), // 19: close_window
    (20, true), // 20: set_slot_state
    (21, true), // 21: cookie_response
    (22, true), // 22: custom_payload
    (23, true), // 23: debug_subscription_request
    (24, true), // 24: edit_book
    (25, true), // 25: query_entity_nbt
    (26, true), // 26: use_entity
    (27, true), // 27: generate_structure
    (28, true), // 28: keep_alive
    (29, true), // 29: lock_difficulty
    (30, true), // 30: position
    (31, true), // 31: position_look
    (32, true), // 32: look
    (33, true), // 33: flying
    (34, true), // 34: vehicle_move
    (35, true), // 35: steer_boat
    (36, true), // 36: pick_item_from_block
    (37, true), // 37: pick_item_from_entity
    (38, true), // 38: ping_request
    (39, true), // 39: craft_recipe_request
    (40, true), // 40: abilities
    (41, true), // 41: block_dig
    (42, true), // 42: entity_action
    (43, true), // 43: player_input
    (44, true), // 44: player_loaded
    (45, true), // 45: pong
    (65535, false), // 46: punch
    (46, true), // 47: recipe_book
    (47, true), // 48: displayed_recipe
    (48, true), // 49: name_item
    (49, true), // 50: resource_pack_receive
    (50, true), // 51: advancement_tab
    (51, true), // 52: select_trade
    (52, true), // 53: set_beacon_effect
    (53, true), // 54: held_item_slot
    (54, true), // 55: update_command_block
    (55, true), // 56: update_command_block_minecart
    (56, false), // 57: set_creative_slot
    (57, true), // 58: set_game_rule
    (58, true), // 59: update_jigsaw_block
    (59, true), // 60: update_structure_block
    (60, true), // 61: set_test_block
    (61, true), // 62: update_sign
    (63, true), // 63: arm_animation
    (64, true), // 64: spectate
    (65, true), // 65: test_instance_block_action
    (66, true), // 66: block_place
    (67, true), // 67: use_item
    (68, true), // 68: custom_click_action
];
