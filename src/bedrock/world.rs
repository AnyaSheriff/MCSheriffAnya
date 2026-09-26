// Игрок Bedrock в мире: Start Game, реестры, чанки, движение.
//
// Чанки кодируются из той же модели мира, что и у Java: состояние блока
// переводится таблицей java_to_bedrock.bin (tools/make_bedrock_tables.py),
// подчанки — в сетевом виде версии 9 (палитра сетевых номеров, индексы
// в порядке X, Z, Y), биомы — объёмными палитрами на каждый подчанк.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use super::codec::{In, Out};
use super::session::{self, Connection, Identity};
use super::inventory::Bag;
use super::skin;
use crate::players::{Member, Position};
use crate::shared::Shared;
use crate::world::{self, WorldEvent};
use crate::{log_debug, log_info, log_warn};

/// Таблица «состояние Java → сетевой номер Bedrock».
static JAVA_TO_BEDROCK: &[u8] = include_bytes!("java_to_bedrock.bin");

/// Готовое тело пакета Item Registry.
static ITEM_REGISTRY: &[u8] = include_bytes!("item_registry.bin");

/// Готовое тело пакета Creative Content: предметы Java, которые есть у Bedrock.
static CREATIVE_CONTENT: &[u8] = include_bytes!("creative_content.bin");

/// Сколько подчанков у верхнего мира: от -64 до 320.
const SUB_CHUNKS: usize = 24;

/// Номер самого нижнего подчанка.
const LOWEST_SUB_CHUNK: i32 = -4;

/// Дальность, с которой клиент начинает: потом он попросит свою.
const START_RADIUS: i32 = 6;

/// Больше этого клиенту чанков не шлём.
const MAX_RADIUS: i32 = 16;

/// Сетевой номер Bedrock для состояния Java.
pub fn bedrock_state(java: i32) -> u32 {
    let index = java.max(0) as usize * 2;

    match JAVA_TO_BEDROCK.get(index..index + 2) {
        Some(bytes) => u16::from_le_bytes([bytes[0], bytes[1]]) as u32,
        None => air(),
    }
}

/// Состояние Java для сетевого номера Bedrock: первое состояние Java,
/// которое переводится в этот номер. Нужно, когда клиент ставит блок.
pub fn java_state(bedrock: u32) -> Option<i32> {
    static REVERSE: OnceLock<Vec<i32>> = OnceLock::new();
    let reverse = REVERSE.get_or_init(|| {
        let mut table = vec![-1; super::tables::BLOCK_STATES];

        for java in (0..JAVA_TO_BEDROCK.len() / 2).rev() {
            let target = u16::from_le_bytes([JAVA_TO_BEDROCK[java * 2], JAVA_TO_BEDROCK[java * 2 + 1]]) as usize;

            if let Some(slot) = table.get_mut(target) {
                *slot = java as i32;
            }
        }

        table
    });

    reverse.get(bedrock as usize).copied().filter(|state| *state >= 0)
}

/// Сетевой номер воздуха.
fn air() -> u32 {
    static AIR: OnceLock<u32> = OnceLock::new();
    *AIR.get_or_init(|| {
        let index = world::AIR.max(0) as usize * 2;
        u16::from_le_bytes([JAVA_TO_BEDROCK[index], JAVA_TO_BEDROCK[index + 1]]) as u32
    })
}

/// Режим игры для Bedrock. Выживание, творческий и приключение совпадают
/// с Java; наблюдатель у Bedrock — 6 (GameMode в описании протокола), а 3
/// там — «наблюдатель выживания».
fn bedrock_game_mode(java: i32) -> i32 {
    match java {
        crate::commands::GAME_MODE_SPECTATOR => 6,
        mode => mode,
    }
}

/// Высота глаз игрока над ногами: Bedrock шлёт и ждёт положение глаз.
const EYES: f32 = 1.62;

/// Игрок Bedrock, уже вошедший в мир.
pub struct BedrockWorld {
    shared: Arc<Shared>,
    identity: Identity,
    runtime_id: u64,
    /// Где глаза игрока (как у Bedrock).
    position: (f32, f32, f32),
    /// Поворот и наклон, как у Java: 0 — на юг, наклон вниз положительный.
    rotation: (f32, f32),
    on_ground: bool,
    /// Режим игры (номера как у Java: 0 — выживание, 1 — творческий).
    game_mode: i32,
    /// Инвентарь.
    bag: Bag,
    /// Журнал лежащих предметов: докуда разослано и что клиент видит.
    items_from: usize,
    items_shown: HashSet<i32>,
    radius: i32,
    sent: HashSet<(i32, i32)>,
    /// Кого из других игроков этот клиент видит: где, UUID, версия скина.
    shown: HashMap<i32, Shown>,
    /// Журнал мира и чата: докуда разослано.
    reader: crate::journal::Reader,
    events_from: usize,
    chat_from: usize,
    ticks: u64,
}

impl BedrockWorld {
    /// Отправляет всё, что нужно для появления в мире.
    ///
    /// None — войти нельзя (ник уже занят): клиенту сказано почему.
    pub fn enter(shared: Arc<Shared>, connection: &Connection, identity: Identity) -> Option<BedrockWorld> {
        let world_spawn = shared.world.lock().expect("мир захвачен другим потоком").spawn_position();

        // Что осталось от прошлого захода — тот же файл игрока, что у Java:
        // место, поворот, режим игры, инвентарь. Впервые зашедший появляется
        // в точке появления в творческом режиме, как игрок Java.
        let saved = crate::playerdata::load(&shared.playerdata, &identity.uuid);
        let spawn = saved.as_ref().map_or(world_spawn, |data| (data.x, data.y, data.z));
        let rotation = saved.as_ref().map_or((0.0, 0.0), |data| (data.yaw, data.pitch));
        let game_mode = saved.as_ref().map_or(crate::commands::GAME_MODE_CREATIVE, |data| data.game_mode);
        let creative = game_mode == crate::commands::GAME_MODE_CREATIVE;
        let bag = Bag::new(saved.map(|data| data.inventory).unwrap_or_else(crate::inventory::Inventory::new));

        let entity_id = shared.next_entity_id();
        let runtime_id = entity_id as u64;
        let position = (spawn.0 as f32, spawn.1 as f32 + EYES, spawn.2 as f32);

        // Свой скин клиент прислал при входе — его увидят другие игроки Bedrock.
        if let Some(look) = identity.look.clone() {
            skin::remember(identity.uuid, look);
        }

        // В общий список — чтобы Java-игроки видели и этого игрока.
        let member = Member {
            uuid: identity.uuid,
            name: identity.name.clone(),
            game_mode,
            entity_id,
            position: Position {
                x: spawn.0,
                y: spawn.1,
                z: spawn.2,
                yaw: rotation.0,
                pitch: rotation.1,
                on_ground: true,
            },
            skin: None,
            skin_parts: 0x7f,
            main_hand: 1,
            held: None,
        };

        if shared.players.lock().expect("список игроков захвачен другим потоком").join(member, entity_id).is_none() {
            let mut disconnect = Out::packet(session::DISCONNECT);
            disconnect.zigzag32(0).bool(false).string("Игрок с таким ником уже на сервере").string("");
            connection.send_one(disconnect);
            return None;
        }

        shared.chat.lock().expect("чат захвачен другим потоком").push(format!("{} зашёл на сервер", identity.name));
        let seed = 0u64;

        let mut start = Out::packet(session::START_GAME);
        start
            .zigzag64(runtime_id as i64) // entity_id
            .varint64(runtime_id) // runtime_entity_id
            .zigzag32(bedrock_game_mode(game_mode)) // player_gamemode
            .vec3f(position.0, position.1, position.2)
            .lf32(rotation.1)
            .lf32(rotation.0) // rotation: наклон, поворот
            .lu64(seed)
            .li16(0) // biome_type
            .string("plains") // biome_name
            .zigzag32(0) // dimension
            .zigzag32(1) // generator: бесконечный
            .zigzag32(bedrock_game_mode(game_mode)) // world_gamemode
            .bool(false) // hardcore
            .zigzag32(2) // difficulty
            .block_position(world_spawn.0 as i32, world_spawn.1 as i32, world_spawn.2 as i32)
            .bool(true) // achievements_disabled
            .zigzag32(0) // editor_world_type
            .bool(false) // created_in_editor
            .bool(false) // exported_from_editor
            .zigzag32(-1) // day_cycle_stop_time
            .zigzag32(0) // edu_offer
            .bool(false) // edu_features_enabled
            .string("") // edu_product_uuid
            .lf32(0.0) // rain_level
            .lf32(0.0) // lightning_level
            .bool(false) // has_confirmed_platform_locked_content
            .bool(true) // is_multiplayer
            .bool(false) // broadcast_to_lan
            .varint(4) // xbox_live_broadcast_mode: публичный
            .varint(4) // platform_broadcast_mode
            .bool(true) // enable_commands
            .bool(false) // is_texturepacks_required
            .varint(0) // gamerules
            .li32(0) // experiments
            .bool(false) // experiments_previously_used
            .bool(false) // bonus_chest
            .bool(false) // map_enabled
            // permission_level: участник. Число со знаком переменной длины
            // (официальная документация Mojang к r/26_u1, LevelSettings,
            // «Player Permissions: varint»); у minecraft-data здесь u8 —
            // тот же один байт, но 1 превратился бы у клиента в −1.
            .zigzag32(1)
            .li32(4) // server_chunk_tick_range
            .bool(false) // has_locked_behavior_pack
            .bool(false) // has_locked_resource_pack
            .bool(false) // is_from_locked_world_template
            .bool(false) // msa_gamertags_only
            .bool(false) // is_from_world_template
            .bool(false) // is_world_template_option_locked
            .bool(false) // only_spawn_v1_villagers
            .bool(false) // persona_disabled
            .bool(false) // custom_skins_disabled
            .bool(false) // emote_chat_muted
            .string(super::GAME_VERSION)
            .li32(0) // limited_world_width
            .li32(0) // limited_world_length
            .bool(true) // is_new_nether
            .string("") // edu_resource_uri: button_name
            .string("") // edu_resource_uri: link_uri
            .bool(false) // experimental_gameplay_override
            .u8(0) // chat_restriction_level
            .bool(false) // disable_player_interactions
            .string("") // level_id
            .string("MCSheriffAnya") // world_name
            .string("") // premium_world_template_id
            .bool(false) // is_trial
            .zigzag32(0) // rewind_history_size
            .bool(false) // server_authoritative_block_breaking
            .li64(0) // current_tick
            .zigzag32(0) // enchantment_seed
            .varint(0) // block_properties
            .string("") // multiplayer_correlation_id
            .bool(true) // server_authoritative_inventory: инвентарём распоряжается сервер
            .string("*") // engine
            .empty_nbt() // property_data
            .lu64(0) // block_pallette_checksum
            .uuid([0; 16]) // world_template_id
            .bool(false) // client_side_generation
            .bool(false) // block_network_ids_are_hashes
            .bool(false) // server_controlled_sound
            .bool(false) // has_server_join_info
            .string("") // server_identifier
            .string("") // scenario_identifier
            .string("") // world_identifier
            .string(""); // owner_identifier

        let mut items = Out::packet(session::ITEM_REGISTRY);
        items.raw(ITEM_REGISTRY);

        let mut menu = Out::packet(session::CREATIVE_CONTENT);
        menu.raw(CREATIVE_CONTENT);

        let biomes = biome_definitions();

        connection.send(&[start.bytes, items.bytes, menu.bytes, biomes]);

        let reader = shared.next_reader();
        let events_from = shared.world.lock().expect("мир захвачен другим потоком").watch_events(reader);
        let chat_from = shared.chat.lock().expect("чат захвачен другим потоком").count();
        let items_from = shared.items.lock().expect("предметы захвачены другим потоком").watch_changes(reader);

        let mut player = BedrockWorld {
            shared,
            identity,
            runtime_id,
            position,
            rotation,
            on_ground: true,
            game_mode,
            bag,
            items_from,
            items_shown: HashSet::new(),
            radius: START_RADIUS,
            sent: HashSet::new(),
            shown: HashMap::new(),
            reader,
            events_from,
            chat_from,
            ticks: 0,
        };

        player.send_chunks(connection);

        let mut status = Out::packet(session::PLAY_STATUS);
        status.i32_be(3);
        let mut packets = player.bag.content_packets();
        packets.push(available_commands());
        packets.push(abilities(runtime_id, creative));
        packets.push(status.bytes);
        connection.send(&packets);

        log_info!("{} (Bedrock) появился в мире", player.identity.name);
        Some(player)
    }

    /// Пакет от клиента после входа.
    pub fn handle(&mut self, connection: &Connection, id: u32, payload: &[u8]) {
        match id {
            session::REQUEST_CHUNK_RADIUS => {
                let wanted = In::new(payload).zigzag32().unwrap_or(START_RADIUS);
                self.radius = wanted.clamp(2, MAX_RADIUS);
                let mut answer = Out::packet(session::CHUNK_RADIUS_UPDATE);
                answer.zigzag32(self.radius);
                connection.send_one(answer);
                self.send_chunks(connection);
            }
            session::PLAYER_AUTH_INPUT => {
                let mut reader = In::new(payload);
                let (Some(pitch), Some(yaw)) = (reader.lf32(), reader.lf32()) else {
                    return;
                };
                let (Some(x), Some(y), Some(z)) = (reader.lf32(), reader.lf32(), reader.lf32()) else {
                    return;
                };
                let before = chunk_of(self.position);
                self.position = (x, y, z);
                self.rotation = (yaw, pitch);

                self.shared.players.lock().expect("список игроков захвачен другим потоком").set_position(
                    &self.identity.uuid,
                    Position {
                        x: x as f64,
                        y: (y - EYES) as f64,
                        z: z as f64,
                        yaw,
                        pitch,
                        on_ground: self.on_ground,
                    },
                );

                if chunk_of(self.position) != before {
                    self.send_chunks(connection);
                }

                let creative = self.creative();
                let (actions, handled) = read_input_actions(&mut reader, &mut self.bag, creative);

                if let Some(handled) = handled {
                    connection.send(&[handled.response]);

                    for stack in handled.thrown {
                        self.throw_out(stack);
                    }
                }

                for action in actions {
                    self.use_item(connection, action);
                }
            }
            session::ITEM_STACK_REQUEST => {
                let handled = self.bag.handle_requests(payload, self.creative());
                connection.send(&[handled.response]);

                for stack in handled.thrown {
                    self.throw_out(stack);
                }
            }
            session::COMMAND_REQUEST => {
                let Some(command) = In::new(payload).string() else {
                    return;
                };
                let command = command.trim().trim_start_matches('/').to_string();
                log_info!("{} ввёл команду: /{}", self.identity.name, command);

                let source = crate::commands::Source::Player { uuid: self.identity.uuid, name: self.identity.name.clone() };
                let answer = crate::commands::run(&command, &source, &self.shared);

                if !answer.reply.is_empty() {
                    log_info!("[{}: {}]", self.identity.name, answer.reply);
                    connection.send(&[raw_text(&answer.reply)]);
                }

                // Смену своего режима подключение узнаёт из общего списка
                // игроков, как и чужую (tick_orders); остановить сервер
                // игроку нельзя.
            }
            session::MOB_EQUIPMENT => {
                // Сменил слот панели: номер сущности, предмет, слот, выбранный слот.
                let mut reader = In::new(payload);

                if reader.varint64().is_some()
                    && read_item(&mut reader).is_some()
                    && let (Some(_), Some(selected)) = (reader.u8(), reader.u8())
                {
                    self.bag.select(selected);
                }
            }
            session::INVENTORY_TRANSACTION => match read_item_use(payload) {
                Some(action) => self.use_item(connection, action),
                None => crate::log_debug!("Bedrock: не разобрал действие предметом: {:02x?}", &payload[..payload.len().min(64)]),
            },
            session::TEXT => {
                if let Some(message) = read_chat(payload) {
                    // В консоль строку выводит общий чат — здесь не пишем.
                    let line = format!("<{}> {}", self.identity.name, message);
                    self.shared.chat.lock().expect("чат захвачен другим потоком").push(line);
                }
            }
            session::SET_LOCAL_PLAYER_INITIALIZED => {
                log_debug!("Bedrock: {} готов", self.identity.name);
            }
            _ => {}
        }
    }

    /// Поручения от команд (выдать, очистить, перенести, выгнать) и смена
    /// режима игры, если её сделала команда.
    fn tick_orders(&mut self, connection: &Connection, packets: &mut Vec<Vec<u8>>) {
        let (orders, game_mode) = {
            let mut players = self.shared.players.lock().expect("список игроков захвачен другим потоком");
            let orders = players.take_orders(&self.identity.uuid);
            // Что в руке — чтобы игроки Java это видели.
            players.set_held(&self.identity.uuid, self.bag.inventory.held());
            let game_mode = players.members().iter().find(|m| m.uuid == self.identity.uuid).map(|m| m.game_mode);
            (orders, game_mode)
        };

        if let Some(mode) = game_mode
            && mode != self.game_mode
        {
            self.game_mode = mode;
            let mut set = Out::packet(session::SET_PLAYER_GAME_TYPE);
            set.zigzag32(bedrock_game_mode(mode));
            packets.push(set.bytes);
            packets.push(abilities(self.runtime_id, self.creative()));
        }

        for order in orders {
            match order {
                crate::players::Order::Kick { reason } => {
                    let mut disconnect = Out::packet(session::DISCONNECT);
                    disconnect.zigzag32(0).bool(false).string(&reason).string("");
                    packets.push(disconnect.bytes);
                    connection.send(&std::mem::take(packets));
                    connection.close();
                    return;
                }
                crate::players::Order::Teleport { x, y, z } => {
                    self.position = (x as f32, y as f32 + EYES, z as f32);
                    let mut teleport = Out::packet(session::MOVE_PLAYER);
                    teleport
                        .varint(self.runtime_id as u32)
                        .vec3f(self.position.0, self.position.1, self.position.2)
                        .lf32(self.rotation.1)
                        .lf32(self.rotation.0)
                        .lf32(self.rotation.0)
                        .u8(2) // перенос
                        .bool(false)
                        .varint(0)
                        .li32(3) // по команде
                        .li32(0)
                        .varint64(0);
                    packets.push(teleport.bytes);
                }
                crate::players::Order::Give { item, count } => {
                    let slots = self.bag.inventory.add(crate::inventory::Stack::new(item, count));
                    self.bag.touched(&slots);
                    packets.extend(slots.iter().filter_map(|&slot| self.bag.slot_packet(slot)));
                }
                crate::players::Order::Clear => {
                    self.bag = Bag::new(crate::inventory::Inventory::new());
                    packets.extend(self.bag.content_packets());
                }
            }
        }
    }

    /// Выбрасывает стопку туда, куда игрок смотрит, — как у Java.
    fn throw_out(&self, stack: crate::inventory::Stack) {
        let (yaw, pitch) = ((self.rotation.0 as f64).to_radians(), (self.rotation.1 as f64).to_radians());
        let look = (-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());
        let flat = look.0.hypot(look.2).max(f64::EPSILON);

        // От рук, чуть впереди; дальше предмет летит сам, по дуге.
        let from = (
            self.position.0 as f64 + look.0 / flat * 0.3,
            (self.position.1 - EYES) as f64 + 1.3,
            self.position.2 as f64 + look.2 / flat * 0.3,
        );
        let velocity = (
            look.0 * crate::items::THROW_SPEED,
            look.1 * crate::items::THROW_SPEED + crate::items::THROW_LIFT,
            look.2 * crate::items::THROW_SPEED,
        );

        self.shared.items.lock().expect("предметы захвачены другим потоком").drop_item(
            self.shared.next_entity_id(),
            stack,
            from,
            velocity,
            crate::items::THROWN_DELAY,
        );
    }

    /// Лежащие предметы: подбор тем, кто рядом, и показ клиенту.
    fn tick_items(&mut self, packets: &mut Vec<Vec<u8>>) {
        let feet = (self.position.0 as f64, (self.position.1 - EYES) as f64, self.position.2 as f64);

        let (taken, changes) = {
            let mut items = self.shared.items.lock().expect("предметы захвачены другим потоком");
            let taken = items.take_near(feet, self.runtime_id as i32);
            let (changes, count) = items.changes_since(self.items_from, self.reader);
            self.items_from = count;
            (taken, changes)
        };

        let mut slots = Vec::new();

        for item in taken {
            slots.extend(self.bag.inventory.add(item.stack));
        }

        slots.sort_unstable();
        slots.dedup();
        self.bag.touched(&slots);
        packets.extend(slots.iter().filter_map(|&slot| self.bag.slot_packet(slot)));

        for change in changes {
            match change {
                crate::items::Change::Dropped(item) if item.block.is_none() => {
                    if let Some(packet) = add_item_entity(&item) {
                        packets.push(packet);
                        self.items_shown.insert(item.entity_id);
                    }
                }
                crate::items::Change::Dropped(_) => {}
                crate::items::Change::Gone { entity_id } => {
                    if self.items_shown.remove(&entity_id) {
                        packets.push(remove_entity(entity_id));
                    }
                }
                crate::items::Change::Taken { entity_id, by, .. } => {
                    if self.items_shown.remove(&entity_id) {
                        let mut take = Out::packet(session::TAKE_ITEM_ENTITY);
                        take.varint64(entity_id as u64).varint(by as u32);
                        packets.push(take.bytes);
                        packets.push(remove_entity(entity_id));
                    }
                }
            }
        }
    }

    /// Творческий режим: полёт, мгновенное ломание, меню предметов.
    fn creative(&self) -> bool {
        self.game_mode == crate::commands::GAME_MODE_CREATIVE
    }

    /// Игрок сломал или поставил блок.
    fn use_item(&mut self, connection: &Connection, action: ItemUse) {
        let mut world = self.shared.world.lock().expect("мир захвачен другим потоком");
        let (x, y, z) = action.position;
        log_debug!(
            "Bedrock: {} — действие {} по {:?}, грань {}, в руке {:?}",
            self.identity.name, action.kind, action.position, action.face, action.held
        );

        let kind = match action.kind {
            START_BREAK if self.creative() => 2,
            START_BREAK => return,
            kind => kind,
        };

        match kind {
            // Сломал. В выживании из блока выпадает предмет — как у Java.
            2 => {
                let Some(broken) = crate::placing::break_block(&mut world, x, y, z) else {
                    return;
                };

                if !self.creative() {
                    let tool = self.bag.inventory.held().map(|stack| stack.item);
                    let luck = world.random_unit();
                    drop(world);

                    if let Some(item) = crate::blocks::drop_by_luck(broken, tool, luck) {
                        self.shared.items.lock().expect("предметы захвачены другим потоком").drop_item(
                            self.shared.next_entity_id(),
                            crate::inventory::Stack::new(item, 1),
                            (x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5),
                            (0.0, 0.0, 0.0),
                            crate::items::MINED_DELAY,
                        );
                    }
                }
            }
            // Щёлкнул по блоку предметом — если предмет ставит блок, ставим.
            0 => {
                // Что в руке — знает сервер: инвентарь у него. Если там пусто,
                // а клиент уверен в обратном (творческий, инвентарь ещё не
                // пришёл), верим клиенту только в творческом режиме.
                let from_bag = self.bag.inventory.held().and_then(|stack| crate::blocks::block_for_item(stack.item));
                let claimed = if self.creative() { action.held.and_then(held_block) } else { None };

                let Some((name, default_state)) = from_bag.or(claimed) else {
                    return;
                };
                let click = crate::placing::Click {
                    face: action.face,
                    cursor_y: action.click_y,
                    yaw: self.rotation.0,
                    pitch: self.rotation.1,
                };
                let feet = (self.position.0 as f64, (self.position.1 - EYES) as f64, self.position.2 as f64);

                let refused = match crate::placing::place(&mut world, name, default_state, action.position, &click, feet) {
                    crate::placing::Placement::Refused(places) => places,
                    crate::placing::Placement::Nothing => vec![offset(action.position, action.face)],
                    crate::placing::Placement::Placed => {
                        // В выживании поставленный блок уходит из руки.
                        if !self.creative()
                            && let Some(slot) = self.bag.inventory.spend_held()
                        {
                            self.bag.touched(&[slot]);
                            connection.send(&self.bag.slot_packet(slot).into_iter().collect::<Vec<_>>());
                        }
                        Vec::new()
                    }
                    crate::placing::Placement::Merged => Vec::new(),
                };

                // Клиент уже нарисовал свой блок — вернём ему правду.
                let packets: Vec<Vec<u8>> = refused
                    .into_iter()
                    .map(|(x, y, z)| {
                        let mut update = Out::packet(session::UPDATE_BLOCK);
                        update.block_position(x, y, z).varint(bedrock_state(world.get_block(x, y, z))).varint(2).varint(0);
                        update.bytes
                    })
                    .collect();

                if !packets.is_empty() {
                    connection.send(&packets);
                }
            }
            _ => {}
        }
    }

    /// Такт: другие игроки, чат, изменения блоков, время суток.
    pub fn tick(&mut self, connection: &Connection) {
        self.ticks += 1;
        let mut packets = Vec::new();

        self.show_players(&mut packets);
        self.tick_items(&mut packets);

        // Чат — строками без форматирования.
        let lines: Vec<String> = {
            let chat = self.shared.chat.lock().expect("чат захвачен другим потоком");
            let lines = chat.since(self.chat_from).to_vec();
            self.chat_from = chat.count();
            lines
        };

        packets.extend(lines.iter().map(|line| raw_text(line)));
        self.tick_orders(connection, &mut packets);

        // Изменения блоков в уже отправленных чанках.
        let events = {
            let mut world = self.shared.world.lock().expect("мир захвачен другим потоком");
            let (events, count) = world.events_since(self.events_from, self.reader);
            self.events_from = count;
            events
        };

        for event in events {
            if let WorldEvent::Change(change) = event
                && self.sent.contains(&(change.x >> 4, change.z >> 4))
            {
                let mut update = Out::packet(session::UPDATE_BLOCK);
                update
                    .block_position(change.x, change.y, change.z)
                    .varint(bedrock_state(change.state))
                    .varint(2) // соседям по сети
                    .varint(0); // основной слой
                packets.push(update.bytes);
            }
        }

        // Время суток — раз в секунду.
        if self.ticks % 20 == 1 {
            let time = self.shared.world.lock().expect("мир захвачен другим потоком").time_of_day();
            let mut set_time = Out::packet(session::SET_TIME);
            set_time.zigzag32((time % 24_000) as i32);
            packets.push(set_time.bytes);
        }

        if !packets.is_empty() {
            connection.send(&packets);
        }
    }

    /// Показывает других игроков: новых — в список и в мир, сдвинувшихся —
    /// двигает, ушедших — убирает.
    fn show_players(&mut self, packets: &mut Vec<Vec<u8>>) {
        let members: Vec<Member> = self.shared.players.lock().expect("список игроков захвачен другим потоком").members().to_vec();
        let own = self.runtime_id as i32;

        for member in &members {
            if member.entity_id == own {
                continue;
            }

            let (version, look) = skin::look_of(member);

            match self.shown.get_mut(&member.entity_id) {
                None => {
                    packets.push(player_list_add(member, &look));
                    packets.push(add_player(member));
                    if member.held.is_some() {
                        packets.push(equipment(member));
                    }
                    self.shown.insert(
                        member.entity_id,
                        Shown { position: member.position, uuid: member.uuid, skin: version, held: member.held },
                    );
                }
                Some(shown) => {
                    if shown.position != member.position {
                        packets.push(move_player(member));
                        shown.position = member.position;
                    }

                    if shown.held != member.held {
                        packets.push(equipment(member));
                        shown.held = member.held;
                    }

                    // Скин докачался или сменился.
                    if shown.skin != version {
                        packets.push(player_skin(member, &look));
                        shown.skin = version;
                    }
                }
            }
        }

        let gone: Vec<i32> = self
            .shown
            .keys()
            .copied()
            .filter(|id| !members.iter().any(|member| member.entity_id == *id))
            .collect();

        for id in gone {
            let Some(shown) = self.shown.remove(&id) else { continue };
            let mut remove = Out::packet(session::REMOVE_ENTITY);
            remove.zigzag64(id as i64);
            packets.push(remove.bytes);

            // И из списка игроков (Tab).
            let mut list = Out::packet(session::PLAYER_LIST);
            list.u8(1).varint(1).uuid(shown.uuid);
            packets.push(list.bytes);
        }
    }


    /// Уход из мира.
    pub fn leave(mut self) {
        log_info!("{} (Bedrock) вышел", self.identity.name);

        // В файл игрока — где он, куда смотрит, в каком режиме, что при нём.
        self.bag.put_cursor_back();
        let data = crate::playerdata::PlayerData {
            x: self.position.0 as f64,
            y: (self.position.1 - EYES) as f64,
            z: self.position.2 as f64,
            yaw: self.rotation.0,
            pitch: self.rotation.1,
            game_mode: self.game_mode,
            inventory: self.bag.inventory.clone(),
        };

        if let Err(error) = crate::playerdata::save(&self.shared.playerdata, &self.identity.uuid, &data) {
            log_warn!("Bedrock: файл игрока {} не записан: {}", self.identity.name, error);
        }

        self.shared.items.lock().expect("предметы захвачены другим потоком").forget_reader(self.reader);
        crate::network::play::player_left(&self.shared, &self.identity.name, &self.identity.uuid);
        skin::forget(&self.identity.uuid);
        self.shared.world.lock().expect("мир захвачен другим потоком").forget_reader(self.reader);
    }

    /// Шлёт недостающие чанки вокруг игрока, ближние первыми.
    fn send_chunks(&mut self, connection: &Connection) {
        let (cx, cz) = chunk_of(self.position);
        let radius = self.radius;

        let mut publisher = Out::packet(session::CHUNK_PUBLISHER_UPDATE);
        publisher
            .block_position(self.position.0 as i32, self.position.1 as i32, self.position.2 as i32)
            .varint((radius * 16) as u32)
            .lu32(0);
        connection.send_one(publisher);

        let mut wanted: Vec<(i32, i32)> = Vec::new();

        for x in cx - radius..=cx + radius {
            for z in cz - radius..=cz + radius {
                if (x - cx).pow(2) + (z - cz).pow(2) <= radius * radius && !self.sent.contains(&(x, z)) {
                    wanted.push((x, z));
                }
            }
        }

        wanted.sort_by_key(|(x, z)| (x - cx).pow(2) + (z - cz).pow(2));
        self.sent.retain(|(x, z)| (x - cx).pow(2) + (z - cz).pow(2) <= (radius + 2).pow(2));

        let mut batch = Vec::new();

        for (x, z) in wanted {
            let packet = {
                let mut world = self.shared.world.lock().expect("мир захвачен другим потоком");
                world.ensure(x, z);
                world.chunk_packet(x, z, |sections, biomes| encode_chunk(x, z, sections, biomes))
            };

            if let Some(packet) = packet {
                batch.push(packet);
                self.sent.insert((x, z));
            }

            if batch.len() >= 8 {
                connection.send(&batch);
                batch.clear();
            }
        }

        if !batch.is_empty() {
            connection.send(&batch);
        }
    }
}

fn chunk_of(position: (f32, f32, f32)) -> (i32, i32) {
    ((position.0.floor() as i32) >> 4, (position.2.floor() as i32) >> 4)
}

/// Пакет Level Chunk со всеми подчанками и биомами.
fn encode_chunk(x: i32, z: i32, sections: &[Option<Arc<[u16]>>], biomes: &[u16]) -> Vec<u8> {
    let mut payload = Out::default();

    for (index, section) in sections.iter().enumerate().take(SUB_CHUNKS) {
        payload.u8(9).u8(1).u8((LOWEST_SUB_CHUNK + index as i32) as i8 as u8);

        match section {
            None => palette_storage(&mut payload, &[air()], &[0; 4096]),
            Some(blocks) => {
                let mut palette: Vec<u32> = Vec::new();
                let mut indexes = vec![0u16; 4096];

                // Bedrock хранит блоки в порядке X, Z, Y; у нас — Y, Z, X.
                for bx in 0..16 {
                    for bz in 0..16 {
                        for by in 0..16 {
                            let state = bedrock_state(blocks[(by * 256 + bz * 16 + bx) as usize] as i32);
                            let at = match palette.iter().position(|known| *known == state) {
                                Some(at) => at,
                                None => {
                                    palette.push(state);
                                    palette.len() - 1
                                }
                            };

                            indexes[(bx * 256 + bz * 16 + by) as usize] = at as u16;
                        }
                    }
                }

                palette_storage(&mut payload, &palette, &indexes);
            }
        }
    }

    // Биомы: на каждый подчанк своя объёмная палитра (у нас клетка 4×4×4 —
    // здесь она раскладывается на блоки).
    for index in 0..SUB_CHUNKS {
        let mut palette: Vec<u32> = Vec::new();
        let mut indexes = vec![0u16; 4096];

        for bx in 0..16 {
            for bz in 0..16 {
                for by in 0..16 {
                    let layer = index * 4 + by / 4;
                    let cell = layer * 16 + (bz / 4) * 4 + bx / 4;
                    let ours = biomes.get(cell).copied().unwrap_or(0) as usize;
                    let biome = super::tables::BIOMES.get(ours).copied().unwrap_or(1);
                    let at = match palette.iter().position(|known| *known == biome) {
                        Some(at) => at,
                        None => {
                            palette.push(biome);
                            palette.len() - 1
                        }
                    };

                    indexes[bx * 256 + bz * 16 + by] = at as u16;
                }
            }
        }

        palette_storage(&mut payload, &palette, &indexes);
    }

    payload.u8(0); // блоков на краю мира нет

    let mut packet = Out::packet(session::LEVEL_CHUNK);
    packet
        .zigzag32(x)
        .zigzag32(z)
        .zigzag32(0) // измерение
        .varint(SUB_CHUNKS as u32)
        .bool(false) // без кэша блобов
        .varint(payload.bytes.len() as u32)
        .raw(&payload.bytes);
    packet.bytes
}

/// Палитровое хранилище: заголовок (биты << 1 | 1), слова little-endian,
/// размер палитры и сетевые номера — числами переменной длины со знаком.
fn palette_storage(out: &mut Out, palette: &[u32], indexes: &[u16]) {
    if palette.len() == 1 {
        out.u8(1); // ноль бит: одно значение на весь подчанк
        out.zigzag32(palette[0] as i32);
        return;
    }

    let bits = match palette.len() {
        0..=2 => 1,
        3..=4 => 2,
        5..=8 => 3,
        9..=16 => 4,
        17..=32 => 5,
        33..=64 => 6,
        65..=256 => 8,
        _ => 16,
    };
    let per_word = 32 / bits;
    let words = 4096_usize.div_ceil(per_word);

    out.u8(((bits as u8) << 1) | 1);

    for word in 0..words {
        let mut value: u32 = 0;

        for slot in 0..per_word {
            let index = word * per_word + slot;

            if index < 4096 {
                value |= (indexes[index] as u32) << (slot * bits);
            }
        }

        out.lu32(value);
    }

    out.zigzag32(palette.len() as i32);

    for entry in palette {
        out.zigzag32(*entry as i32);
    }
}

/// Текст из пакета Text: только обычный чат.
fn read_chat(payload: &[u8]) -> Option<String> {
    let mut reader = In::new(payload);
    let _translation = reader.bool()?;
    let _category = reader.u8()?;
    let kind = reader.u8()?;

    if kind != 1 {
        return None;
    }

    let _source = reader.string()?;
    let message = reader.string()?;
    let message = message.trim().to_string();

    (!message.is_empty()).then_some(message)
}

/// Строка в чат без форматирования.
fn raw_text(line: &str) -> Vec<u8> {
    let mut text = Out::packet(session::TEXT);
    text.bool(false).u8(0).u8(0).string(line).string("").string("").bool(false);
    text.bytes
}

/// Наши команды — чтобы клиент их знал и отправлял. У каждой один
/// необязательный аргумент «как есть»: разбирает команду общий обработчик,
/// тот же, что у Java и консоли.
fn available_commands() -> Vec<u8> {
    const COMMANDS: [(&str, &str); 14] = [
        ("help", "Список команд"),
        ("list", "Кто на сервере"),
        ("say", "Сказать всем"),
        ("tp", "Перенести игрока"),
        ("gamemode", "Сменить режим игры"),
        ("kick", "Выгнать игрока"),
        ("give", "Выдать предметы"),
        ("clear", "Очистить инвентарь"),
        ("time", "Время суток"),
        ("op", "Дать права"),
        ("deop", "Забрать права"),
        ("setblock", "Поставить блок"),
        ("fill", "Заполнить область"),
        ("stop", "Остановить сервер"),
    ];
    const RAW_TEXT: u16 = 58;
    const VALID: u16 = 16;

    let mut out = Out::packet(session::AVAILABLE_COMMANDS);
    out.varint(0) // значения перечислений
        .varint(0) // значения цепочек
        .varint(0) // суффиксы
        .varint(0) // перечисления
        .varint(0) // цепочки
        .varint(COMMANDS.len() as u32);

    for (name, description) in COMMANDS {
        out.string(name)
            .string(description)
            .lu16(0) // флаги
            .string("any") // уровень прав проверяет сервер
            .li32(-1) // без перечисления синонимов
            .varint(0) // смещения цепочек
            .varint(1) // один вариант
            .bool(false)
            .varint(1)
            .string("аргументы")
            .lu16(RAW_TEXT)
            .lu16(VALID)
            .bool(true) // необязательный
            .u8(0);
    }

    out.varint(0).varint(0); // динамические перечисления, ограничения
    out.bytes
}

fn player_list_add(member: &Member, look: &skin::Look) -> Vec<u8> {
    let mut out = Out::packet(session::PLAYER_LIST);
    out.u8(0).varint(1).uuid(member.uuid).zigzag64(member.entity_id as i64).string(&member.name).string("").string("").li32(0);
    skin::encode(&mut out, look);
    out.bool(false).bool(false).bool(false).li32(0);
    out.bool(true); // verified
    out.bytes
}

/// Определения наших биомов: имя, номер, климат, цвет воды. Имена — в
/// общем списке строк в конце пакета, определения ссылаются на них номером.
fn biome_definitions() -> Vec<u8> {
    use crate::world::terrain::{AUTUMN_FOREST, Biome};

    let mut out = Out::packet(session::BIOME_DEFINITION_LIST);
    let mut names: Vec<&str> = Vec::new();
    let mut entries = Out::default();

    for (index, (&bedrock_name, &id)) in super::tables::BIOME_NAMES.iter().zip(&super::tables::BIOMES).enumerate() {
        // Своё (осенний лес) у Bedrock показывается обычным лесом — он уже есть.
        if names.contains(&bedrock_name) {
            continue;
        }

        let (java_name, (temperature, downfall)) = match Biome::ALL.get(index) {
            Some(biome) => (biome.name(), biome.climate()),
            None => (AUTUMN_FOREST, Biome::Forest.climate()),
        };
        let water = crate::network::registries::water_color(java_name);

        entries
            .li16(names.len() as i16)
            .lu16(id as u16)
            .lf32(temperature)
            .lf32(downfall)
            .lf32(0.0) // snow_foliage
            .lf32(0.1) // depth
            .lf32(0.2) // scale
            .li32((0xff00_0000 | water) as i32)
            .bool(temperature < 2.0) // осадки
            .u8(0) // без тегов
            .u8(0); // без правил генерации
        names.push(bedrock_name);
    }

    out.varint(names.len() as u32).raw(&entries.bytes).varint(names.len() as u32);

    for name in names {
        out.string(name);
    }

    out.bytes
}

/// Лежащий предмет (Add Item Entity): где, с какой скоростью, что за предмет.
fn add_item_entity(item: &crate::items::Dropped) -> Option<Vec<u8>> {
    super::inventory::bedrock_item(item.stack.item)?;

    let mut out = Out::packet(session::ADD_ITEM_ENTITY);
    out.zigzag64(item.entity_id as i64).varint64(item.entity_id as u64);
    super::inventory::write_item(&mut out, Some(item.stack), 0);
    out.vec3f(item.x as f32, item.y as f32, item.z as f32)
        .vec3f(item.vx as f32, item.vy as f32, item.vz as f32)
        .varint(0) // metadata
        .bool(false); // from_fishing
    Some(out.bytes)
}

/// Что другой игрок держит в руке (Mob Equipment).
fn equipment(member: &Member) -> Vec<u8> {
    let mut out = Out::packet(session::MOB_EQUIPMENT);
    out.varint64(member.entity_id as u64);
    super::inventory::write_item(&mut out, member.held, 0);
    out.u8(0).u8(0).u8(0); // слот, выбранный слот, окно инвентаря
    out.bytes
}

fn remove_entity(entity_id: i32) -> Vec<u8> {
    let mut out = Out::packet(session::REMOVE_ENTITY);
    out.zigzag64(entity_id as i64);
    out.bytes
}

/// Новый скин уже показанного игрока.
fn player_skin(member: &Member, look: &skin::Look) -> Vec<u8> {
    let mut out = Out::packet(session::PLAYER_SKIN);
    out.uuid(member.uuid);
    skin::encode(&mut out, look);
    out.string("").string("").bool(true);
    out.bytes
}

fn add_player(member: &Member) -> Vec<u8> {
    let p = &member.position;
    let mut out = Out::packet(session::ADD_PLAYER);
    out.uuid(member.uuid)
        .string(&member.name)
        .varint64(member.entity_id as u64)
        .string("") // platform_chat_id
        .vec3f(p.x as f32, p.y as f32 + EYES, p.z as f32)
        .vec3f(0.0, 0.0, 0.0)
        .lf32(p.pitch)
        .lf32(p.yaw)
        .lf32(p.yaw)
        .zigzag32(0) // в руке пусто
        .zigzag32(bedrock_game_mode(member.game_mode))
        .varint(0) // metadata
        .varint(0)
        .varint(0) // properties
        .li64(member.entity_id as i64)
        .u8(1) // permission_level
        .u8(0) // command_permission
        .u8(0) // abilities
        .varint(0) // links
        .string("") // device_id
        .li32(0); // device_os
    out.bytes
}

fn move_player(member: &Member) -> Vec<u8> {
    let p = &member.position;
    let mut out = Out::packet(session::MOVE_PLAYER);
    out.varint(member.entity_id as u32)
        .vec3f(p.x as f32, p.y as f32 + EYES, p.z as f32)
        .lf32(p.pitch)
        .lf32(p.yaw)
        .lf32(p.yaw)
        .u8(0) // обычное движение
        .bool(p.on_ground)
        .varint(0)
        .varint64(0);
    out.bytes
}

/// Способности: строить и ломать — всегда; летать и ломать мгновенно —
/// в творческом режиме.
fn abilities(runtime_id: u64, creative: bool) -> Vec<u8> {
    // Порядок флагов — как в описании AbilitySet (proto.yml).
    const BUILD: u32 = 1 << 0;
    const MINE: u32 = 1 << 1;
    const DOORS: u32 = 1 << 2;
    const CONTAINERS: u32 = 1 << 3;
    const ATTACK_PLAYERS: u32 = 1 << 4;
    const ATTACK_MOBS: u32 = 1 << 5;
    const MAY_FLY: u32 = 1 << 10;
    const INSTANT_BUILD: u32 = 1 << 11;
    const FLY_SPEED: u32 = 1 << 13;
    const WALK_SPEED: u32 = 1 << 14;
    const VERTICAL_FLY_SPEED: u32 = 1 << 19;

    let mut enabled = BUILD | MINE | DOORS | CONTAINERS | ATTACK_PLAYERS | ATTACK_MOBS;

    if creative {
        enabled |= MAY_FLY | INSTANT_BUILD;
    }

    let allowed = enabled | FLY_SPEED | WALK_SPEED | VERTICAL_FLY_SPEED | (1 << 9);

    let mut out = Out::packet(session::UPDATE_ABILITIES);
    out.li64(runtime_id as i64)
        .u8(1) // permission_level: участник
        .u8(0) // command_permission
        .u8(1) // один слой — основной
        .lu16(1)
        .lu32(allowed)
        .lu32(enabled | FLY_SPEED | WALK_SPEED | VERTICAL_FLY_SPEED)
        .lf32(0.05)
        .lf32(1.0)
        .lf32(0.1);
    out.bytes
}

/// Другой игрок, которого видит этот клиент.
struct Shown {
    position: Position,
    uuid: [u8; 16],
    skin: u64,
    held: Option<crate::inventory::Stack>,
}

/// Начал ломать (из Player Auth Input): ломает только в творческом режиме.
const START_BREAK: u32 = 100;

/// Что игрок сделал предметом: сломал блок, щёлкнул по блоку.
struct ItemUse {
    kind: u32,
    position: (i32, i32, i32),
    face: i32,
    /// Предмет в руке; None — рука пуста.
    held: Option<Held>,
    /// Высота точки щелчка внутри блока (0 — низ, 1 — верх).
    click_y: f32,
}

/// Предмет в руке, как его описал клиент.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    network_id: i32,
    metadata: u32,
    /// Сетевой номер блока, который он ставит (0 — не блок).
    block: i32,
}

/// Таблица «предмет Bedrock → предмет Java» (см. make_bedrock_tables.py).
static ITEMS_TO_JAVA: &[u8] = include_bytes!("items_to_java.bin");

/// Предмет Java для предмета Bedrock: сначала с теми же метаданными
/// (цвет кровати, флага), потом с нулевыми.
fn java_item(held: Held) -> Option<i32> {
    let entries = ITEMS_TO_JAVA.as_chunks::<6>().0;
    let find = |metadata: u32| {
        entries
            .binary_search_by_key(&(held.network_id, metadata), |e| {
                (i16::from_le_bytes([e[0], e[1]]) as i32, u16::from_le_bytes([e[2], e[3]]) as u32)
            })
            .ok()
            .map(|index| u16::from_le_bytes([entries[index][4], entries[index][5]]) as i32)
    };

    find(held.metadata).or_else(|| find(0))
}

/// Какой блок ставит предмет: через предмет Java (так ставятся и пыль,
/// и семена), а если его нет в таблице — по номеру блока из пакета.
fn held_block(held: Held) -> Option<(&'static str, i32)> {
    if let Some(block) = java_item(held).and_then(crate::blocks::block_for_item) {
        return Some(block);
    }

    let state = java_state(u32::try_from(held.block).ok().filter(|b| *b != 0)?)?;
    let name = crate::blocks::block_at_state(state)?;
    Some((name, crate::blocks::state_by_name(name).unwrap_or(state)))
}

/// Соседний блок с той стороны, по которой щёлкнули.
fn offset((x, y, z): (i32, i32, i32), face: i32) -> (i32, i32, i32) {
    match face {
        0 => (x, y - 1, z),
        1 => (x, y + 1, z),
        2 => (x, y, z - 1),
        3 => (x, y, z + 1),
        4 => (x - 1, y, z),
        5 => (x + 1, y, z),
        _ => (x, y, z),
    }
}

/// Читает предмет из пакета; None внутри — пустая рука.
fn read_item(reader: &mut In) -> Option<Option<Held>> {
    let network_id = reader.zigzag32()?;

    if network_id == 0 {
        return Some(None);
    }

    let _count = reader.take(2)?;
    let metadata = reader.varint()?;

    if reader.u8()? != 0 {
        reader.zigzag32()?;
    }

    let block = reader.zigzag32()?;
    let extra = reader.varint()? as usize;
    reader.take(extra)?;

    Some(Some(Held { network_id, metadata, block }))
}

/// Из Inventory Transaction — действие предметом по блоку.
fn read_item_use(payload: &[u8]) -> Option<ItemUse> {
    let mut reader = In::new(payload);
    skip_legacy(&mut reader)?;
    let transaction_type = reader.varint()?;
    skip_actions(&mut reader)?;

    if transaction_type != 2 {
        return None;
    }

    read_use(&mut reader)
}

/// Действия с блоками из остатка Player Auth Input (после позиции):
/// ломание приходит здесь, когда ломание проверяет сервер.
fn read_input_actions(reader: &mut In, bag: &mut Bag, creative: bool) -> (Vec<ItemUse>, Option<super::inventory::Handled>) {
    const ITEM_INTERACT: u128 = 1 << 34;
    const BLOCK_ACTION: u128 = 1 << 35;
    const ITEM_STACK_REQUEST: u128 = 1 << 36;
    const PREDICTED_VEHICLE: u128 = 1 << 45;

    let mut found = Vec::new();
    let mut handled = None;

    let mut read = || -> Option<()> {
        reader.take(8 + 4)?; // move_vector, head_yaw
        let mut flags = 0u128;

        for shift in (0..128).step_by(7) {
            let byte = reader.u8()?;
            flags |= ((byte & 0x7f) as u128) << shift;

            if byte & 0x80 == 0 {
                break;
            }
        }

        reader.varint()?; // input_mode
        reader.varint()?; // play_mode
        reader.zigzag32()?; // interaction_model
        reader.take(8)?; // interact_rotation
        reader.varint64()?; // tick
        reader.take(12)?; // delta

        if flags & ITEM_INTERACT != 0 {
            skip_legacy(reader)?;
            skip_actions(reader)?;
            found.push(read_use(reader)?);
        }

        // Запрос к инвентарю (например, износ инструмента при добыче):
        // выполняем, а действия с блоками идут после него.
        if flags & ITEM_STACK_REQUEST != 0 {
            let (answer, whole) = bag.handle_embedded(reader, creative);
            handled = Some(answer);

            if !whole {
                return None;
            }
        }

        if flags & PREDICTED_VEHICLE != 0 {
            reader.take(8)?;
            reader.varint64()?;
        }

        if flags & BLOCK_ACTION != 0 {
            for _ in 0..reader.zigzag32()? {
                let action = reader.zigzag32()?;

                if matches!(action, 0 | 1 | 18 | 26 | 27) {
                    let position = (reader.zigzag32()?, reader.zigzag32()?, reader.zigzag32()?);
                    let face = reader.zigzag32()?;

                    // «Предсказал поломку» — клиент докопал. Творческий режим
                    // ломает с первого удара: такие отмечены отдельно.
                    match action {
                        26 => found.push(ItemUse { kind: 2, position, face, held: None, click_y: 0.0 }),
                        0 => found.push(ItemUse { kind: START_BREAK, position, face, held: None, click_y: 0.0 }),
                        _ => {}
                    }
                }
            }
        }

        Some(())
    };

    read();
    (found, handled)
}

fn skip_legacy(reader: &mut In) -> Option<()> {
    if reader.zigzag32()? != 0 {
        for _ in 0..reader.varint()? {
            reader.u8()?;

            for _ in 0..reader.varint()? {
                reader.u8()?;
            }
        }
    }

    Some(())
}

fn skip_actions(reader: &mut In) -> Option<()> {
    for _ in 0..reader.varint()? {
        match reader.varint()? {
            0 => {
                reader.zigzag32()?;
            }
            2 | 100 | 99999 => {
                reader.varint()?;
            }
            _ => {}
        }

        reader.varint()?;
        read_item(reader)?;
        read_item(reader)?;
    }

    Some(())
}

/// Тело «предмет применён»: что сделано, где, чем.
fn read_use(reader: &mut In) -> Option<ItemUse> {
    let kind = reader.varint()?;
    let _trigger = reader.varint()?;
    let position = (reader.zigzag32()?, reader.zigzag32()?, reader.zigzag32()?);
    let face = reader.zigzag32()?;
    let _hotbar = reader.zigzag32()?;
    let held = read_item(reader)?;
    let _player = reader.take(12)?;
    let click = (reader.lf32()?, reader.lf32()?, reader.lf32()?);
    // Хвост TransactionUseItem: номер блока, предсказание клиента, перезарядка.
    // В Player Auth Input за ним идут другие поля — его надо пройти целиком.
    let _block = reader.varint()?;
    let _prediction = reader.varint()?;
    let _cooldown = reader.u8()?;

    Some(ItemUse { kind, position, face, held, click_y: click.1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Предмет в руке: камень с номером блока.
    fn stone(out: &mut Out) {
        out.zigzag32(1).lu16(1).varint(0).u8(0).zigzag32(2533);
        out.varint(10).lu16(0).li32(0).li32(0);
    }

    /// Хвост «предмет применён» после предмета в руке.
    fn use_tail(out: &mut Out) {
        out.vec3f(0.0, 0.0, 0.0).vec3f(0.5, 1.0, 0.5).varint(0).varint(1).u8(0);
    }

    #[test]
    fn inventory_transaction_place_reads() {
        let mut out = Out::default();
        out.zigzag32(0).varint(2).varint(0); // без старого запроса, «применён», без действий
        out.varint(0).varint(1).zigzag32(-5).zigzag32(70).zigzag32(12).zigzag32(1).zigzag32(0);
        stone(&mut out);
        use_tail(&mut out);

        let action = read_item_use(&out.bytes).expect("действие");
        assert_eq!((action.kind, action.position, action.face), (0, (-5, 70, 12), 1));
        assert_eq!(action.held, Some(Held { network_id: 1, metadata: 0, block: 2533 }));
        assert_eq!(action.click_y, 1.0);
        assert_eq!(held_block(action.held.expect("камень")).map(|(name, _)| name), Some("stone"));
    }

    #[test]
    fn auth_input_block_actions_read() {
        let mut out = Out::default();
        out.lf32(0.0).lf32(0.0); // move_vector
        out.lf32(0.0); // head_yaw
        out.varint64(1 << 35); // только block_action
        out.varint(1).varint(0).zigzag32(1).lf32(0.0).lf32(0.0).varint64(7).vec3f(0.0, 0.0, 0.0);
        out.zigzag32(2);
        out.zigzag32(26).zigzag32(3).zigzag32(-60).zigzag32(-4).zigzag32(1); // предсказал поломку
        out.zigzag32(1).zigzag32(3).zigzag32(-60).zigzag32(-4).zigzag32(1); // бросил ломать

        let mut bag = Bag::new(crate::inventory::Inventory::new());
        let (actions, _) = read_input_actions(&mut In::new(&out.bytes), &mut bag, true);
        assert_eq!(actions.len(), 1);
        assert_eq!((actions[0].kind, actions[0].position), (2, (3, -60, -4)));
    }

    /// Действие предметом в Player Auth Input читается до конца
    /// (TransactionUseItem с номером блока, предсказанием и перезарядкой),
    /// и действия с блоками после него не теряются.
    #[test]
    fn block_actions_after_an_item_interaction_are_read() {
        let mut out = Out::default();
        out.lf32(0.0).lf32(0.0).lf32(0.0);
        out.varint64((1 << 34) | (1 << 35)); // item_interact и block_action
        out.varint(1).varint(0).zigzag32(1).lf32(0.0).lf32(0.0).varint64(7).vec3f(0.0, 0.0, 0.0);
        out.zigzag32(0).varint(0); // без старого запроса, без действий
        out.varint(0).varint(1).zigzag32(-5).zigzag32(70).zigzag32(12).zigzag32(1).zigzag32(0);
        stone(&mut out);
        use_tail(&mut out);
        out.zigzag32(1);
        out.zigzag32(26).zigzag32(3).zigzag32(-60).zigzag32(-4).zigzag32(1);

        let mut bag = Bag::new(crate::inventory::Inventory::new());
        let (actions, _) = read_input_actions(&mut In::new(&out.bytes), &mut bag, true);

        assert_eq!(actions.len(), 2);
        assert_eq!((actions[0].kind, actions[0].position), (0, (-5, 70, 12)));
        assert_eq!((actions[1].kind, actions[1].position), (2, (3, -60, -4)));
    }

    /// В выживании запрос к инвентарю (износ инструмента) идёт в том же
    /// Player Auth Input перед «докопал» — поломка не должна теряться.
    #[test]
    fn a_break_after_an_embedded_request_is_read() {
        let mut out = Out::default();
        out.lf32(0.0).lf32(0.0).lf32(0.0);
        out.varint64((1 << 35) | (1 << 36)); // block_action и item_stack_request
        out.varint(1).varint(0).zigzag32(1).lf32(0.0).lf32(0.0).varint64(7).vec3f(0.0, 0.0, 0.0);
        out.zigzag32(-5).varint(1).u8(11).zigzag32(0).zigzag32(0).zigzag32(0).varint(0).li32(0); // mine_block
        out.zigzag32(1);
        out.zigzag32(26).zigzag32(3).zigzag32(-60).zigzag32(-4).zigzag32(1);

        let mut bag = Bag::new(crate::inventory::Inventory::new());
        let (actions, handled) = read_input_actions(&mut In::new(&out.bytes), &mut bag, false);

        assert!(handled.is_some());
        assert_eq!(actions.len(), 1);
        assert_eq!((actions[0].kind, actions[0].position), (2, (3, -60, -4)));
    }

    /// Номера предметов Bedrock 1.26.10 (minecraft-data, items.json).
    #[test]
    fn bedrock_items_place_java_blocks() {
        let held = |network_id, metadata| Held { network_id, metadata, block: 0 };

        // Пыль ставит провод, хотя блоком сама не является.
        assert_eq!(held_block(held(405, 0)).map(|(name, _)| name), Some("redstone_wire"));
        // Цвет кровати — в метаданных: 14 — красная.
        assert_eq!(held_block(held(450, 14)).map(|(name, _)| name), Some("red_bed"));
        assert_eq!(held_block(held(450, 0)).map(|(name, _)| name), Some("white_bed"));
        assert_eq!(held_block(held(53, 0)).map(|(name, _)| name), Some("oak_stairs"));
    }

    #[test]
    fn spectator_is_bedrock_spectator() {
        assert_eq!(bedrock_game_mode(crate::commands::GAME_MODE_SURVIVAL), 0);
        assert_eq!(bedrock_game_mode(crate::commands::GAME_MODE_CREATIVE), 1);
        assert_eq!(bedrock_game_mode(crate::commands::GAME_MODE_ADVENTURE), 2);
        assert_eq!(bedrock_game_mode(crate::commands::GAME_MODE_SPECTATOR), 6);
    }

    #[test]
    fn bedrock_states_map_back() {
        assert_eq!(java_state(bedrock_state(1)), Some(1));
        assert_eq!(java_state(air()), Some(world::AIR));
    }
}
