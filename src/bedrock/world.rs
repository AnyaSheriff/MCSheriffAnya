// Игрок Bedrock в мире: Start Game, реестры, чанки, движение, переход
// между измерениями.
//
// Чанки кодируются из той же модели мира, что и у Java: состояние блока
// переводится таблицей java_to_bedrock.bin (tools/make_bedrock_tables.py),
// подчанки — в сетевом виде версии 9 (палитра сетевых номеров, индексы
// в порядке X, Z, Y), биомы — объёмными палитрами на каждый подчанк.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use tokio::sync::mpsc;

use super::codec::{In, Out};
use super::session::{self, Connection, Identity};
use super::inventory::Bag;
use super::skin;
use crate::dimension::Dimension;
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

/// Какие подчанки есть у клиента в этом измерении: номер нижнего и сколько их.
///
/// Номер подчанка — его высота в мире, делённая на 16 (документация Mojang
/// к r/26_u1, additional_docs/SubChunk Request System v1.18.10.md: «a
/// -64:319 block range would result in a -4:20 SubChunk Y range»). Обычный
/// мир — от −64 до 320: 24 подчанка, нижний −4. Незер у Bedrock — от 0 до
/// 128 (minecraft.wiki, The Nether: высота постройки в Незере у Bedrock —
/// 128): 8 подчанков, нижний 0. Наш Незер лежит в той же сетке −64..320,
/// что и обычный мир, а заполнен только от 0 до 127 — эта часть клиенту
/// и уходит.
fn sub_chunks(dimension: Dimension) -> (i32, usize) {
    match dimension {
        Dimension::Overworld => (-4, 24),
        Dimension::Nether => (0, 8),
    }
}

/// Высоты блоков, которые есть у клиента в этом измерении.
fn heights(dimension: Dimension) -> std::ops::Range<i32> {
    let (lowest, count) = sub_chunks(dimension);
    lowest * 16..(lowest + count as i32) * 16
}

/// Номер измерения у Bedrock — DimensionType документации Mojang к r/26_u1
/// (ChangeDimensionPacket: «0 -> Overworld, 1 -> Nether, 2 -> The End»).
fn bedrock_dimension(dimension: Dimension) -> i32 {
    match dimension {
        Dimension::Overworld => 0,
        Dimension::Nether => 1,
    }
}

/// Биом Незера у Bedrock — «hell» (nether_wastes у Java), номер 8 по
/// таблице minecraft.wiki (Biome ID, Bedrock Edition): им считается место
/// Незера, биом которого не нашёлся в таблице.
const NETHER_WASTES: u32 = 8;

/// Сколько ждать подтверждения перехода в другое измерение (в тактах): не
/// пришло — переход считается законченным и так.
const CHANGE_TIMEOUT: u64 = 600;

/// Сколько чанков вокруг места прибытия должно уйти клиенту, прежде чем
/// сказать ему «появился»: этот знак закрывает экран загрузки, и без чанков
/// игрок стоит в пустоте, окружённый заглушками. Но не дольше
/// SPAWN_WAIT тактов — вдруг мир не складывается.
const SPAWN_RADIUS: i32 = 2;
const SPAWN_WAIT: u64 = 100;

/// Дальность, с которой клиент начинает: потом он попросит свою.
const START_RADIUS: i32 = 6;

/// Больше этого клиенту чанков не шлём — и не больше, чем дальность
/// сервера с запасом в два чанка (см. `max_radius`).
const MAX_RADIUS: i32 = 16;

/// Сколько чанков одновременно читается с диска или складывается для
/// одного игрока, и сколько уходит клиенту за один такт.
const LOADING_AT_ONCE: usize = 8;
const SENT_PER_TICK: usize = 64;

/// Дальше этого от начала координат по X и Z игрок не бывает (граница мира
/// у оригинала — 29 999 984); по высоте — с запасом над миром и под ним.
const WORLD_LIMIT: f32 = 30_000_000.0;
const LOWEST_Y: f32 = (world::MIN_Y - 1024) as f32;
const HIGHEST_Y: f32 = (world::MIN_Y + world::WORLD_HEIGHT + 1024) as f32;

/// Больше этого за один пакет движения игрок не перемещается: скачок дальше —
/// откат на прежнее место.
const MAX_STEP: f64 = 100.0;

/// Как далеко от глаз можно ломать и ставить: у оригинала около 5 блоков
/// в выживании и около 6 в творческом, здесь — с запасом на задержку сети.
const REACH_SURVIVAL: f64 = 8.0;
const REACH_CREATIVE: f64 = 12.0;

/// Как часто файл игрока переписывается, если что-то изменилось (в тактах).
const SAVE_EVERY: u64 = 100;

/// Через сколько тактов без ответа на перенос он повторяется.
const TELEPORT_RETRY: u64 = 100;

/// Флаги входа Player Auth Input — номера битов из документации Mojang
/// к r/26_u1 (PlayerAuthInputPacket::InputData).
const HANDLED_TELEPORT: u128 = 1 << 37;
const START_FLYING: u128 = 1 << 42;

/// Бит «приседает» во вводе игрока (InputFlag, описание протокола).
const SNEAKING: u128 = 1 << 8;
const VERTICAL_COLLISION: u128 = 1 << 50;

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

/// Битовая карта «в этом состоянии Java есть вода» (make_bedrock_tables.py).
static JAVA_WATERLOGGED: &[u8] = include_bytes!("java_waterlogged.bin");

/// Есть ли вода в блоке — у Bedrock она лежит вторым слоем.
fn waterlogged(java: i32) -> bool {
    usize::try_from(java)
        .ok()
        .and_then(|state| JAVA_WATERLOGGED.get(state / 8).map(|byte| byte >> (state % 8) & 1 == 1))
        .unwrap_or(false)
}

/// Сетевой номер стоячей воды (источника) — для второго слоя.
fn still_water() -> u32 {
    static WATER: OnceLock<u32> = OnceLock::new();
    *WATER.get_or_init(|| bedrock_state(crate::blocks::state_by_name("water").unwrap_or(0)))
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

/// Чанк, прочитанный или сложенный в стороне: из какого он измерения, где
/// он и он сам.
type Arrived = (Dimension, i32, i32, world::ReadyChunk);

/// Перенос, который сервер отправил и ждёт, что клиент его примет.
struct Teleporting {
    /// Куда (глаза).
    target: (f32, f32, f32),
    /// В каком такте отправлен.
    since: u64,
}

/// Переход в другое измерение, который клиент ещё не закончил.
struct Changing {
    /// Номер экрана загрузки из Change Dimension.
    loading_screen: u32,
    /// В каком такте отправлен.
    since: u64,
    /// Ушёл ли Play Status «появился» (`spawn_when_ready`).
    spawned: bool,
}

/// Игрок Bedrock, уже вошедший в мир.
pub struct BedrockWorld {
    shared: Arc<Shared>,
    /// Измерение игрока.
    dimension: Dimension,
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
    /// Перенос, который клиент ещё не принял: до тех пор его прежнее место
    /// не считается движением.
    teleport: Option<Teleporting>,
    /// Что последний раз записано в файл игрока.
    saved: Option<crate::playerdata::PlayerData>,
    /// Чанки, которые читаются или складываются в стороне, и куда они приходят.
    loading: HashSet<(i32, i32)>,
    arrived_tx: mpsc::UnboundedSender<Arrived>,
    arrived_rx: mpsc::UnboundedReceiver<Arrived>,
    /// Остались ли вокруг игрока неотправленные чанки.
    chunks_missing: bool,
    /// Какая область объявлена клиенту (Chunk Publisher Update): центр и дальность.
    published: Option<((i32, i32), i32)>,
    /// Переход в другое измерение, который клиент ещё не подтвердил: пока он
    /// идёт, движение клиента не принимается, а игроки и предметы нового
    /// измерения не показываются.
    changing: Option<Changing>,
    /// Номер экрана загрузки для следующего перехода: у каждого свой.
    next_loading_screen: u32,
    /// Предметы, лежащие в новом измерении: покажутся, когда клиент закончит
    /// переход.
    waiting_items: Vec<(i32, Vec<u8>)>,
    /// Сколько тактов подряд игрок стоит в портале, и можно ли порталу
    /// сработать: после перехода и входа — только когда игрок из портала выйдет.
    in_portal: u32,
    /// Приседает ли — по вводу клиента (бит «sneaking»).
    sneaking: bool,
    portal_ready: bool,
}

impl BedrockWorld {
    /// Отправляет всё, что нужно для появления в мире.
    ///
    /// None — войти нельзя (ник уже занят): клиенту сказано почему.
    pub fn enter(shared: Arc<Shared>, connection: &Connection, identity: Identity) -> Option<BedrockWorld> {
        // Что осталось от прошлого захода — тот же файл игрока, что у Java:
        // измерение, место, поворот, режим игры, инвентарь. Впервые зашедший
        // появляется в точке появления обычного мира в творческом режиме,
        // как игрок Java.
        let saved = crate::playerdata::load(&shared.playerdata, &identity.uuid);
        let dimension = saved.as_ref().map(|data| data.dimension).unwrap_or_default();
        let world_spawn = shared.world_in(dimension).lock().expect("мир захвачен другим потоком").spawn_position();
        let spawn = saved.as_ref().map_or(world_spawn, |data| (data.x, data.y, data.z));
        let rotation = saved.as_ref().map_or((0.0, 0.0), |data| (data.yaw, data.pitch));
        let game_mode = saved.as_ref().map_or(crate::commands::GAME_MODE_CREATIVE, |data| data.game_mode);
        let bag = Bag::new(saved.as_ref().map(|data| data.inventory.clone()).unwrap_or_else(crate::inventory::Inventory::new));

        let entity_id = shared.next_entity_id();
        let runtime_id = entity_id as u64;
        let position = (spawn.0 as f32, spawn.1 as f32 + EYES, spawn.2 as f32);

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
            dimension,
            sneaking: false,
            swings: 0,
        };

        if shared.players.lock().expect("список игроков захвачен другим потоком").join(member, entity_id).is_none() {
            let mut disconnect = Out::packet(session::DISCONNECT);
            disconnect.zigzag32(0).bool(false).string("Игрок с таким ником уже на сервере").string("");
            connection.send_one(disconnect);
            return None;
        }

        // Свой скин клиент прислал при входе — его увидят другие игроки
        // Bedrock. Запоминается только после того, как вход удался: иначе
        // отказанный вход подменил бы скин того, кто уже играет.
        if let Some(look) = identity.look.clone() {
            skin::remember(identity.uuid, look);
        }

        // Игрокам Java скин нужен из Mojang: картинка загружается туда
        // в фоне (если владелец это разрешил), и тогда его покажут заново.
        if shared.skin_settings.bedrock_to_java
            && let Some((png, slim)) = identity.look.as_ref().and_then(skin::java_png)
        {
            let (shared, uuid, name) = (Arc::clone(&shared), identity.uuid, identity.name.clone());
            let cape = identity.look.as_ref().and_then(skin::cape).map(|(width, height, rgba)| (width, height, rgba.to_vec()));

            tokio::spawn(async move {
                let cape = match cape {
                    Some((width, height, rgba)) => crate::skins::java_cape(width, height, &rgba).await,
                    None => None,
                };

                match crate::skins::upload_bedrock(&shared.skins, &shared.skin_settings, png, slim, cape.as_deref()).await {
                    Some(java) => {
                        log_debug!("Bedrock: скин {} загружен для игроков Java", name);
                        shared.players.lock().expect("список игроков захвачен другим потоком").set_skin(&uuid, java);
                    }
                    None => log_info!("Скины: скин {} (Bedrock) игроки Java не увидят — MineSkin не ответил", name),
                }
            });
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
            .zigzag32(bedrock_dimension(dimension)) // dimension
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
            .zigzag32(4) // xbox_live_broadcast_mode: публичный (у Mojang — varint со знаком)
            .zigzag32(4) // platform_broadcast_mode
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

        connection.send(&[start.bytes, items.bytes, menu.bytes, biome_definitions()]);

        let reader = shared.next_reader();
        let events_from = shared.world_in(dimension).lock().expect("мир захвачен другим потоком").watch_events(reader);
        let chat_from = shared.chat.lock().expect("чат захвачен другим потоком").count();

        let (items_from, lying) = lying_items(&shared, dimension, reader);
        let (arrived_tx, arrived_rx) = mpsc::unbounded_channel();

        let mut player = BedrockWorld {
            shared,
            dimension,
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
            teleport: None,
            saved,
            loading: HashSet::new(),
            arrived_tx,
            arrived_rx,
            chunks_missing: false,
            published: None,
            changing: None,
            next_loading_screen: 1,
            waiting_items: Vec::new(),
            in_portal: 0,
            sneaking: false,
            // Зашёл, стоя в портале, — не переносить сразу (как у Java).
            portal_ready: false,
        };

        player.radius = player.radius.min(player.max_radius());
        player.send_chunks(connection);

        let mut status = Out::packet(session::PLAY_STATUS);
        status.i32_be(3);
        let mut packets = player.bag.content_packets();

        for (entity_id, packet) in lying {
            player.items_shown.insert(entity_id);
            packets.push(packet);
        }

        packets.push(available_commands());
        packets.push(abilities(runtime_id, game_mode));
        packets.push(own_entity_data(runtime_id));
        packets.push(own_attributes(runtime_id));
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
                self.radius = wanted.clamp(2, self.max_radius());
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

                // Сначала весь пакет: флаги нужны, чтобы принять положение,
                // а действиям с блоками нужно уже принятое положение.
                let creative = self.creative();
                let input = read_input_actions(&mut reader, &mut self.bag, creative);
                self.take_position(connection, (x, y, z), (yaw, pitch), input.flags);

                // Присел или встал — остальные узнают из общего списка.
                let sneaking = input.flags & SNEAKING != 0;
                if sneaking != self.sneaking {
                    self.sneaking = sneaking;
                    self.shared.players.lock().expect("список игроков захвачен другим потоком").set_sneaking(&self.identity.uuid, sneaking);
                }

                // В выживании летать нельзя: клиент, решивший взлететь,
                // снова получает свои способности — и падает.
                if input.flags & START_FLYING != 0 && !may_fly(self.game_mode) {
                    connection.send(&[abilities(self.runtime_id, self.game_mode)]);
                }

                let (actions, handled) = (input.actions, input.handled);

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
                let command = clean_text(command.trim().trim_start_matches('/'));
                log_info!("{} ввёл команду: /{}", self.identity.name, command);

                // Вход Xbox Live не подтверждён (auth.rs): кто угодно мог
                // назваться владельцем сервера. Такому игроку — только то,
                // что можно всем, как бы ни было записано в ops.json.
                // Подтверждённому — права по ops.json, как игроку Java.
                if !self.identity.verified && !open_to_everyone(&command) {
                    connection.send(&[raw_text("На это у тебя нет прав")]);
                    return;
                }

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
            // Взмах рукой (Animate, действие 1 — swing_arm; описание
            // протокола): остальные увидят его из общего списка.
            session::ANIMATE => {
                const SWING_ARM: u8 = 1;

                if In::new(payload).u8() == Some(SWING_ARM) {
                    self.shared.players.lock().expect("список игроков захвачен другим потоком").swing(&self.identity.uuid);
                }
            }
            session::INVENTORY_TRANSACTION => match read_item_use(payload) {
                Some(action) => self.use_item(connection, action),
                None => crate::log_debug!("Bedrock: не разобрал действие предметом: {:02x?}", &payload[..payload.len().min(64)]),
            },
            session::TEXT => {
                if let Some(message) = read_chat(payload).map(|message| clean_text(&message)).filter(|m| !m.is_empty()) {
                    // В консоль строку выводит общий чат — здесь не пишем.
                    let line = format!("<{}> {}", self.identity.name, message);
                    self.shared.chat.lock().expect("чат захвачен другим потоком").push(line);
                }
            }
            // Клиент закончил переход в другое измерение (Player Action
            // с ChangeDimensionAck = 14, PlayerActionType документации Mojang
            // к r/26_u1).
            session::PLAYER_ACTION => {
                const CHANGE_DIMENSION_ACK: i32 = 14;
                let mut reader = In::new(payload);

                if reader.varint64().is_some() && reader.zigzag32() == Some(CHANGE_DIMENSION_ACK) {
                    self.finish_change(connection, "подтвердил переход");
                }
            }
            // Экран загрузки (ServerboundLoadingScreenPacket, r/26_u1): тип —
            // 1 начало, 2 конец; номер — тот, что был в Change Dimension.
            // Закрылся экран нашего перехода — клиент уже в новом измерении.
            session::SERVERBOUND_LOADING_SCREEN => {
                const END_LOADING_SCREEN: i32 = 2;
                let mut reader = In::new(payload);
                let kind = reader.zigzag32();
                let id = match reader.bool() {
                    Some(true) => reader.li32().map(|id| id as u32),
                    _ => None,
                };
                log_debug!("Bedrock: {} — экран загрузки {:?}, номер {:?}", self.identity.name, kind, id);

                if kind == Some(END_LOADING_SCREEN)
                    && id.is_some()
                    && self.changing.as_ref().map(|changing| changing.loading_screen) == id
                {
                    self.finish_change(connection, "закрыл экран загрузки");
                }
            }
            session::SET_LOCAL_PLAYER_INITIALIZED => {
                log_debug!("Bedrock: {} готов", self.identity.name);

                // Инвентарь ещё раз: присланный до появления клиент мог и не
                // показать.
                connection.send(&self.bag.content_packets());
            }
            // Открыть свой инвентарь (клавиша E). Когда инвентарём
            // распоряжается сервер, клиент сам экран не открывает, а просит
            // (Interact с действием OpenInventory = 6) и ждёт Container Open:
            // документация Mojang к r/26_u1, additional_docs/ServerAuthInventory.
            session::INTERACT => {
                const OPEN_INVENTORY: u8 = 6;

                if In::new(payload).u8() == Some(OPEN_INVENTORY) {
                    connection.send(&[self.open_inventory()]);
                }
            }
            // Клиент закрыл экран — сервер отвечает тем же, иначе экран
            // больше не откроется.
            session::CONTAINER_CLOSE => {
                let mut reader = In::new(payload);

                if let (Some(window), Some(kind)) = (reader.u8(), reader.u8()) {
                    let mut close = Out::packet(session::CONTAINER_CLOSE);
                    close.u8(window).u8(kind).bool(false); // закрыл не сервер
                    connection.send_one(close);
                }
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
            packets.push(abilities(self.runtime_id, mode));
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
                crate::players::Order::Teleport { x, y, z } => self.teleport_to(connection, packets, (x, y, z)),
                crate::players::Order::Look { yaw, pitch } => {
                    self.rotation = (yaw, pitch);

                    // Во время перехода в другое измерение клиент ещё не
                    // в новом мире — поворот только запоминается; иначе
                    // уходит сразу, на том же месте.
                    if self.changing.is_none() {
                        packets.push(teleport_packet(self.runtime_id, self.position, self.rotation, TELEPORT_BY_COMMAND));
                        connection.send(&std::mem::take(packets));
                    }
                }
                crate::players::Order::Give { item, count } => {
                    let (slots, left) = self.bag.inventory.put(crate::inventory::Stack::new(item, count));

                    // Не влезло — падает рядом с игроком, а не пропадает.
                    if let Some(left) = left {
                        self.throw_out(left);
                    }

                    self.bag.touched(&slots);
                    packets.extend(slots.iter().filter_map(|&slot| self.bag.slot_packet(slot)));
                }
                // Режим уже подхвачен выше, из общего списка игроков.
                crate::players::Order::GameMode { .. } => {}
                // В то же измерение Change Dimension клиенту не шлют — это
                // обычный перенос.
                crate::players::Order::ChangeDimension { dimension, x, y, z } if dimension == self.dimension => {
                    self.teleport_to(connection, packets, (x, y, z));
                }
                crate::players::Order::ChangeDimension { dimension, x, y, z } => {
                    self.change_dimension(connection, packets, dimension, (x, y, z));
                }
                crate::players::Order::Clear => {
                    self.bag = Bag::new(crate::inventory::Inventory::new());
                    packets.extend(self.bag.content_packets());
                }
            }
        }
    }

    /// Перенос в этом же измерении (команда): клиенту — Move Player, за ним
    /// чанки на новом месте.
    fn teleport_to(&mut self, connection: &Connection, packets: &mut Vec<Vec<u8>>, (x, y, z): (f64, f64, f64)) {
        let target = (x as f32, y as f32 + EYES, z as f32);

        if !valid_position(target) {
            log_warn!("Bedrock: {} не перенесён — место за краем мира", self.identity.name);
            return;
        }

        self.position = target;
        self.teleport = Some(Teleporting { target, since: self.ticks });
        self.publish_position();
        packets.push(teleport_packet(self.runtime_id, target, self.rotation, TELEPORT_BY_COMMAND));

        // Перенос уходит первым, за ним — чанки на новом месте
        // и новая объявленная область.
        connection.send(&std::mem::take(packets));
        self.send_chunks(connection);
    }

    /// Переход в другое измерение (портал или команда).
    ///
    /// Клиенту — Change Dimension (документация Mojang к r/26_u1: «The
    /// server sends this packet from the level to kick off dimension
    /// changing process»), следом область и чанки нового мира и Play Status
    /// «появился», как при входе. Клиент заканчивает переход сам: шлёт
    /// Player Action с ChangeDimensionAck и закрывает экран загрузки. До тех
    /// пор его движение не принимается, а игроки и предметы нового измерения
    /// ждут (`finish_change`). Журналы старого мира забываются, нового —
    /// читаются с этого места, как у Java (network/play.rs).
    fn change_dimension(
        &mut self,
        connection: &Connection,
        packets: &mut Vec<Vec<u8>>,
        dimension: Dimension,
        (x, y, z): (f64, f64, f64),
    ) {
        let target = (x as f32, y as f32 + EYES, z as f32);

        if !valid_position(target) {
            log_warn!("Bedrock: {} не перешёл в {} — место за краем мира", self.identity.name, dimension.name());
            return;
        }

        // Всё, что накопилось для старого измерения, уходит до перехода —
        // вместе с удалением его сущностей: предметов и других игроков.
        packets.extend(self.items_shown.drain().map(remove_entity));
        for (id, shown) in std::mem::take(&mut self.shown) {
            packets.extend(forget_player(id, &shown));
        }
        connection.send(&std::mem::take(packets));

        let (old, reader) = (self.dimension, self.reader);
        self.shared.world_in(old).lock().expect("мир захвачен другим потоком").forget_reader(reader);
        self.shared.items_in(old).lock().expect("предметы захвачены другим потоком").forget_reader(reader);

        self.dimension = dimension;
        self.position = target;
        self.teleport = None;
        self.sent.clear();
        self.loading.clear();
        self.published = None;
        self.in_portal = 0;
        self.portal_ready = false;

        self.shared.players.lock().expect("список игроков захвачен другим потоком").set_dimension(&self.identity.uuid, dimension);
        self.publish_position();

        self.events_from = self.shared.world_in(dimension).lock().expect("мир захвачен другим потоком").watch_events(reader);
        (self.items_from, self.waiting_items) = lying_items(&self.shared, dimension, reader);

        let loading_screen = self.next_loading_screen;
        self.next_loading_screen = self.next_loading_screen.wrapping_add(1);
        self.changing = Some(Changing { loading_screen, since: self.ticks, spawned: false });

        connection.send(&[change_dimension_packet(dimension, target, Some(loading_screen))]);
        self.send_chunks(connection);
        self.spawn_when_ready(connection);

        self.save_if_changed();
        log_info!("{} (Bedrock) перешёл в {} ({:.1}, {:.1}, {:.1})", self.identity.name, dimension.name(), x, y, z);
    }

    /// Play Status «появился» после перехода — когда чанки вокруг места
    /// прибытия уже ушли: они идут по тому же упорядоченному каналу и
    /// дойдут раньше. Сразу вслед за Change Dimension этот знак закрывал
    /// экран загрузки до чанков — игрок видел себя в коробке из заглушек.
    fn spawn_when_ready(&mut self, connection: &Connection) {
        let (center_x, center_z) = chunk_of(self.position);
        let ready = (-SPAWN_RADIUS..=SPAWN_RADIUS)
            .all(|dx| (-SPAWN_RADIUS..=SPAWN_RADIUS).all(|dz| self.sent.contains(&(center_x + dx, center_z + dz))));
        let Some(changing) = self.changing.as_mut().filter(|changing| !changing.spawned) else {
            return;
        };

        if !ready && self.ticks - changing.since < SPAWN_WAIT {
            return;
        }

        changing.spawned = true;
        log_debug!("Bedrock: {} — чанки вокруг готовы через {} тактов", self.identity.name, self.ticks - changing.since);

        let mut status = Out::packet(session::PLAY_STATUS);
        status.i32_be(3); // PlayerSpawn
        connection.send_one(status);
    }

    /// Клиент закончил переход в другое измерение: движение снова
    /// принимается, предметы нового измерения показываются (игроков покажет
    /// сверка видимости в такте).
    fn finish_change(&mut self, connection: &Connection, why: &str) {
        let Some(changing) = self.changing.take() else {
            return;
        };

        log_debug!(
            "Bedrock: {} {} в {} за {} тактов",
            self.identity.name,
            why,
            self.dimension.name(),
            self.ticks - changing.since
        );

        let mut packets = Vec::new();

        for (entity_id, packet) in std::mem::take(&mut self.waiting_items) {
            self.items_shown.insert(entity_id);
            packets.push(packet);
        }

        if !packets.is_empty() {
            connection.send(&packets);
        }
    }

    /// Такт у портала — как у игрока Java (`step_portal` в network/play.rs):
    /// в творческом переход сразу, в выживании — через `portal-delay`
    /// тактов подряд в портале; после перехода портал сработает, только
    /// когда игрок из него выйдет. Переход — поручением себе же: его
    /// выполнит `tick_orders`.
    fn step_portal(&mut self) {
        if self.changing.is_some() {
            return;
        }

        let feet = (self.position.0 as f64, (self.position.1 - EYES) as f64, self.position.2 as f64);
        let inside = crate::portal::standing_in(
            &self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком"),
            feet,
        );

        if !inside {
            self.in_portal = 0;
            self.portal_ready = true;
            return;
        }

        if !self.portal_ready {
            return;
        }

        self.in_portal += 1;
        let needed = crate::portal::wait(&self.shared.settings, self.creative());

        if self.in_portal < needed {
            return;
        }

        self.in_portal = 0;
        self.portal_ready = false;

        let (dimension, (x, y, z)) = crate::portal::travel(&self.shared, self.dimension, feet);

        self.shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .order(&self.identity.uuid, crate::players::Order::ChangeDimension { dimension, x, y, z });
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

        self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком").drop_item(
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

        let mut slots = Vec::new();

        // Поднимается столько, сколько влезает: остальное остаётся лежать.
        let changes = {
            let mut items = self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком");
            let inventory = &mut self.bag.inventory;
            items.take_near(feet, self.runtime_id as i32, |stack| {
                let (changed, left) = inventory.put(stack);

                slots.extend(changed);
                stack.count - left.map_or(0, |left| left.count)
            });
            let (changes, count) = items.changes_since(self.items_from, self.reader);
            self.items_from = count;
            changes
        };

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
        let (x, y, z) = action.position;
        log_debug!(
            "Bedrock: {} — действие {} по {:?}, грань {}, в руке {:?}",
            self.identity.name, action.kind, action.position, action.face, action.held
        );

        let kind = match action.kind {
            START_BREAK if self.creative() => BREAK,
            START_BREAK => return,
            kind => kind,
        };

        if kind != BREAK && kind != CLICK {
            return;
        }

        // Место за краем мира — ни ломать, ни ставить, ни отвечать нечего:
        // таких блоков у клиента нет. Проверка до сложения координат —
        // соседний блок уже не переполнит число.
        if !block_in_world(self.dimension, action.position) {
            return;
        }

        let target = if kind == BREAK { action.position } else { offset(action.position, action.face) };
        let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

        if let Some(reason) = self.refusal(&world, kind, action.position, target) {
            log_debug!("Bedrock: {} — отказано ({}) в {:?}", self.identity.name, reason, action.position);

            // Клиент уже показал своё — вернём ему правду.
            let packets = self.restore(&world, &[action.position, target]);
            drop(world);

            if !packets.is_empty() {
                connection.send(&packets);
            }

            return;
        }

        match kind {
            // Сломал. В выживании из блока выпадает предмет — как у Java.
            BREAK => {
                let Some(broken) = crate::placing::break_block(&mut world, x, y, z) else {
                    // Ломать было нечего — но клиент мог показать поломку.
                    let packets = self.restore(&world, &[action.position]);
                    drop(world);

                    if !packets.is_empty() {
                        connection.send(&packets);
                    }

                    return;
                };

                if !self.creative() {
                    let tool = self.bag.inventory.held().map(|stack| stack.item);
                    let luck = world.random_unit();
                    drop(world);

                    if let Some(stack) = crate::blocks::drops_by_luck(broken, tool, luck) {
                        self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком").drop_item(
                            self.shared.next_entity_id(),
                            stack,
                            (x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5),
                            (0.0, 0.0, 0.0),
                            crate::items::MINED_DELAY,
                        );
                    }
                }
            }
            // Щёлкнул по блоку предметом — если предмет ставит блок, ставим.
            _ => {
                // Зажигалка — как у Java: в рамке из обсидиана портал, иначе
                // огонь на месте рядом с гранью, по которой щёлкнули.
                let held_item = self
                    .bag
                    .inventory
                    .held()
                    .map(|stack| stack.item)
                    .or_else(|| if self.creative() { action.held.and_then(java_item) } else { None });

                if held_item.is_some() && held_item == crate::blocks::item_named("flint_and_steel") {
                    if world.get_block(target.0, target.1, target.2) == crate::world::AIR {
                        if crate::portal::light(&mut world, target) {
                            log_info!("{} зажёг портал в {} {} {}", self.identity.name, target.0, target.1, target.2);
                        } else if let Some(fire) = crate::blocks::state_by_name("fire") {
                            world.set_block(target.0, target.1, target.2, fire);
                        }

                        crate::redstone::settle(&mut world);
                    }

                    return;
                }

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
                    crate::placing::Placement::Nothing => vec![target],
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
                let packets = self.restore(&world, &refused);
                drop(world);

                if !packets.is_empty() {
                    connection.send(&packets);
                }
            }
        }
    }

    /// Почему игроку нельзя сломать блок (`BREAK`) или поставить его
    /// щелчком (`CLICK`); None — можно.
    fn refusal(
        &self,
        world: &world::World,
        kind: u32,
        clicked: (i32, i32, i32),
        target: (i32, i32, i32),
    ) -> Option<&'static str> {
        use crate::commands::{GAME_MODE_ADVENTURE, GAME_MODE_SPECTATOR};

        // Наблюдатель не трогает мир, в приключении блоки не ломают и не ставят.
        if self.game_mode == GAME_MODE_SPECTATOR || self.game_mode == GAME_MODE_ADVENTURE {
            return Some("режим игры");
        }

        if !block_in_world(self.dimension, target) {
            return Some("за краем мира");
        }

        // До блока надо дотянуться.
        let reach = if self.creative() { REACH_CREATIVE } else { REACH_SURVIVAL };
        let eyes = (self.position.0 as f64, self.position.1 as f64, self.position.2 as f64);
        let center = (clicked.0 as f64 + 0.5, clicked.1 as f64 + 0.5, clicked.2 as f64 + 0.5);

        if distance(eyes, center) > reach {
            return Some("далеко");
        }

        // Чанк, которого нет в памяти, не трогаем: клиенту его и не слали.
        if !world.has_chunk(clicked.0 >> 4, clicked.2 >> 4) || !world.has_chunk(target.0 >> 4, target.2 >> 4) {
            return Some("чанк не загружен");
        }

        if kind == BREAK {
            // Как у Java: копьём не ломается ничего, мечом и подобным — только
            // не в творческом.
            if let Some(item) = self.bag.inventory.held().map(|stack| stack.item)
                && !(crate::blocks::can_break_at_all(item) && (!self.creative() || crate::blocks::can_break_in_creative(item)))
            {
                return Some("этим предметом не ломают");
            }

            if !self.creative() && unbreakable(world.get_block(clicked.0, clicked.1, clicked.2)) {
                return Some("блок не ломается");
            }
        }

        None
    }

    /// Пакеты с тем, что на самом деле стоит в этих местах (оба слоя), —
    /// для тех мест, чьи чанки клиент уже получил.
    fn restore(&self, world: &world::World, places: &[(i32, i32, i32)]) -> Vec<Vec<u8>> {
        let mut places = places.to_vec();
        places.sort_unstable();
        places.dedup();

        places
            .into_iter()
            .filter(|&(x, y, z)| block_in_world(self.dimension, (x, y, z)) && self.sent.contains(&(x >> 4, z >> 4)))
            .flat_map(|(x, y, z)| block_updates(x, y, z, world.get_block(x, y, z)))
            .collect()
    }

    /// Положение из Player Auth Input: принимается, если оно настоящее.
    /// За краем мира и с NaN — пропускается, скачок дальше `MAX_STEP` —
    /// откатывается. Пока клиент не принял перенос, его прежнее место не
    /// считается движением.
    fn take_position(&mut self, connection: &Connection, eyes: (f32, f32, f32), rotation: (f32, f32), flags: u128) {
        if !valid_position(eyes) {
            log_debug!("Bedrock: {} прислал негодное положение {:?}", self.identity.name, eyes);
            return;
        }

        // Идёт переход в другое измерение: клиент может слать ещё старое
        // место — его не принимаем.
        if self.changing.is_some() {
            return;
        }

        if let Some(teleport) = &mut self.teleport {
            if flags & HANDLED_TELEPORT == 0 && distance_f32(eyes, teleport.target) > MAX_STEP {
                // Ещё старое место. Ответа долго нет — перенос отправляется снова.
                if self.ticks.saturating_sub(teleport.since) > TELEPORT_RETRY {
                    teleport.since = self.ticks;
                    connection.send(&[teleport_packet(self.runtime_id, teleport.target, self.rotation, TELEPORT_UNKNOWN)]);
                }

                return;
            }

            self.teleport = None;
        }

        if distance_f32(eyes, self.position) > MAX_STEP {
            log_debug!("Bedrock: {} скакнул из {:?} в {:?} — откат", self.identity.name, self.position, eyes);
            self.teleport = Some(Teleporting { target: self.position, since: self.ticks });
            connection.send(&[teleport_packet(self.runtime_id, self.position, self.rotation, TELEPORT_UNKNOWN)]);
            return;
        }

        let before = chunk_of(self.position);

        // Стоит на земле, если упёрся во что-то по вертикали и не поднимается.
        self.on_ground = flags & VERTICAL_COLLISION != 0 && eyes.1 <= self.position.1;
        self.position = eyes;

        if rotation.0.is_finite() && rotation.1.is_finite() {
            self.rotation = rotation;
        }

        self.publish_position();

        if chunk_of(self.position) != before {
            self.send_chunks(connection);
        }
    }

    /// Кладёт положение игрока в общий список — оттуда его берут остальные.
    fn publish_position(&self) {
        self.shared.players.lock().expect("список игроков захвачен другим потоком").set_position(
            &self.identity.uuid,
            Position {
                x: self.position.0 as f64,
                y: (self.position.1 - EYES) as f64,
                z: self.position.2 as f64,
                yaw: self.rotation.0,
                pitch: self.rotation.1,
                on_ground: self.on_ground,
            },
        );
    }

    /// Container Open для своего инвентаря. Поля — документация Mojang
    /// к r/26_u1 (ContainerOpenPacket): окно 0 (инвентарь игрока), вид
    /// INVENTORY = −1 (ContainerType, одним байтом), место — блок под
    /// игроком, сущность — сам игрок.
    fn open_inventory(&self) -> Vec<u8> {
        const WINDOW_INVENTORY: u8 = 0;
        const CONTAINER_INVENTORY: i8 = -1;

        let (x, y, z) = self.position;
        let mut open = Out::packet(session::CONTAINER_OPEN);
        open.u8(WINDOW_INVENTORY)
            .u8(CONTAINER_INVENTORY as u8)
            .block_position(x.floor() as i32, (y - EYES).floor() as i32, z.floor() as i32)
            .zigzag64(self.runtime_id as i64);
        open.bytes
    }

    /// Дальность, больше которой клиенту чанков не шлём: не дальше, чем
    /// видят игроки Java, с запасом в два чанка.
    fn max_radius(&self) -> i32 {
        MAX_RADIUS.min(self.shared.properties.view_distance + 2).max(2)
    }

    /// Что записывается в файл игрока. Курсор — туда же, в инвентарь: иначе
    /// то, что держали курсором, пропало бы.
    fn player_data(&self) -> crate::playerdata::PlayerData {
        crate::playerdata::PlayerData {
            x: self.position.0 as f64,
            y: (self.position.1 - EYES) as f64,
            z: self.position.2 as f64,
            yaw: self.rotation.0,
            pitch: self.rotation.1,
            game_mode: self.game_mode,
            inventory: self.bag.inventory_with_cursor(),
            dimension: self.dimension,
        }
    }

    /// Записывает файл игрока, если что-то изменилось с прошлого раза.
    fn save_if_changed(&mut self) {
        let data = self.player_data();

        if self.saved.as_ref() == Some(&data) {
            return;
        }

        match crate::playerdata::save(&self.shared.playerdata, &self.identity.uuid, &data) {
            Ok(()) => self.saved = Some(data),
            Err(error) => log_warn!("Bedrock: файл игрока {} не записан: {}", self.identity.name, error),
        }
    }

    /// Такт: другие игроки, чат, изменения блоков, время суток.
    pub fn tick(&mut self, connection: &Connection) {
        self.ticks += 1;

        // Чанки, прочитанные или сложенные в стороне, — в мир, и клиенту то,
        // что ещё не ушло.
        let mut arrived = Vec::new();

        while let Ok(chunk) = self.arrived_rx.try_recv() {
            arrived.push(chunk);
        }

        // Чанк, заказанный ещё в старом измерении, ложится в свой мир —
        // прочитан, так пригодится, — но клиенту уже не нужен.
        for dimension in Dimension::ALL {
            if !arrived.iter().any(|chunk| chunk.0 == dimension) {
                continue;
            }

            let mut world = self.shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

            for (_, x, z, ready) in arrived.extract_if(.., |chunk| chunk.0 == dimension) {
                if dimension == self.dimension {
                    self.loading.remove(&(x, z));
                    self.chunks_missing = true;
                }
                world.accept(x, z, ready);
            }
        }

        if self.chunks_missing {
            self.send_chunks(connection);
        }
        self.spawn_when_ready(connection);

        // Файл игрока — не только при выходе: сервер могут и выключить.
        if self.ticks.is_multiple_of(SAVE_EVERY) {
            self.save_if_changed();
        }

        // Клиент так и не сказал, что закончил переход, — считаем, что
        // закончил: иначе игрок так и остался бы без движения.
        if self.changing.as_ref().is_some_and(|changing| self.ticks - changing.since > CHANGE_TIMEOUT) {
            log_warn!("Bedrock: {} не подтвердил переход в {}", self.identity.name, self.dimension.name());
            self.finish_change(connection, "не ответил");
        }

        let mut packets = Vec::new();

        // Пока идёт переход, новое измерение клиенту не показываем.
        if self.changing.is_none() {
            self.show_players(&mut packets);
            self.tick_items(&mut packets);
        }

        // Чат — строками без форматирования.
        let lines: Vec<String> = {
            let chat = self.shared.chat.lock().expect("чат захвачен другим потоком");
            let lines = chat.since(self.chat_from).to_vec();
            self.chat_from = chat.count();
            lines
        };

        packets.extend(lines.iter().map(|line| raw_text(line)));
        self.step_portal();
        self.tick_orders(connection, &mut packets);

        // Изменения блоков в уже отправленных чанках.
        let events = {
            let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
            let (events, count) = world.events_since(self.events_from, self.reader);
            self.events_from = count;
            events
        };

        for event in events {
            if let WorldEvent::Change(change) = event
                && self.sent.contains(&(change.x >> 4, change.z >> 4))
                && heights(self.dimension).contains(&change.y)
            {
                packets.extend(block_updates(change.x, change.y, change.z, change.state));
            }
        }

        // Время суток — раз в секунду.
        if self.ticks % 20 == 1 {
            let time = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком").time_of_day();
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
        let own = self.runtime_id as i32;
        let mut present = Vec::new();

        // Под замком только сравнение: копируются лишь те, у кого что-то
        // изменилось, — а не весь список каждый такт.
        let changed: Vec<Member> = {
            let players = self.shared.players.lock().expect("список игроков захвачен другим потоком");

            players
                .members()
                .iter()
                .filter(|member| member.entity_id != own)
                // Игроки из другого измерения — как отсутствующие.
                .filter(|member| member.dimension == self.dimension)
                .filter(|member| {
                    present.push(member.entity_id);
                    !self.shown.get(&member.entity_id).is_some_and(|shown| {
                        shown.position == member.position
                            && shown.held == member.held
                            && shown.sneaking == member.sneaking
                            && shown.swings == member.swings
                            && shown.skin == skin::version_of(member)
                    })
                })
                .cloned()
                .collect()
        };

        for member in &changed {
            match self.shown.get_mut(&member.entity_id) {
                None => {
                    let (version, look) = skin::look_of(member);
                    packets.push(player_list_add(member, &look));
                    packets.push(add_player(member));
                    if member.held.is_some() {
                        packets.push(equipment(member));
                    }
                    self.shown.insert(
                        member.entity_id,
                        Shown {
                            position: member.position,
                            uuid: member.uuid,
                            skin: version,
                            held: member.held,
                            sneaking: member.sneaking,
                            swings: member.swings,
                        },
                    );
                }
                Some(shown) => {
                    if shown.position != member.position {
                        packets.push(move_player(member));
                        shown.position = member.position;
                    }

                    if shown.sneaking != member.sneaking {
                        packets.push(other_entity_data(member));
                        shown.sneaking = member.sneaking;
                    }

                    // Взмахнул рукой — один взмах за всё, что набежало.
                    if shown.swings != member.swings {
                        packets.push(swing(member.entity_id));
                        shown.swings = member.swings;
                    }

                    if shown.held != member.held {
                        packets.push(equipment(member));
                        shown.held = member.held;
                    }

                    // Скин докачался или сменился.
                    if shown.skin != skin::version_of(member) {
                        let (version, look) = skin::look_of(member);
                        packets.push(player_skin(member, &look));
                        shown.skin = version;
                    }
                }
            }
        }

        let gone: Vec<i32> = self.shown.keys().copied().filter(|id| !present.contains(id)).collect();

        for id in gone {
            if let Some(shown) = self.shown.remove(&id) {
                packets.extend(forget_player(id, &shown));
            }
        }
    }

    /// Уход из мира.
    pub fn leave(mut self) {
        log_info!("{} (Bedrock) вышел", self.identity.name);

        // В файл игрока — где он, куда смотрит, в каком режиме, что при нём.
        if let Some(left) = self.bag.put_cursor_back() {
            self.throw_out(left);
        }

        // При выходе — в любом случае, даже если ничего не изменилось.
        self.saved = None;
        self.save_if_changed();

        self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком").forget_reader(self.reader);
        crate::network::play::player_left(&self.shared, &self.identity.name, &self.identity.uuid);
        skin::forget(&self.identity.uuid);
        self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком").forget_reader(self.reader);
    }

    /// Шлёт недостающие чанки вокруг игрока, ближние первыми.
    ///
    /// Чанков, которых нет в памяти, здесь не ждут: они читаются с диска или
    /// складываются в стороне, как у Java (`ChunkSource`), и уходят клиенту
    /// в одном из следующих тактов. Пакет чанка собирается вне замка мира —
    /// под замком только берутся ссылки на секции.
    fn send_chunks(&mut self, connection: &Connection) {
        let (cx, cz) = chunk_of(self.position);
        let radius = self.radius;

        // Область объявляется, только когда она сменилась.
        if self.published != Some(((cx, cz), radius)) {
            let mut publisher = Out::packet(session::CHUNK_PUBLISHER_UPDATE);
            publisher
                .block_position(self.position.0 as i32, self.position.1 as i32, self.position.2 as i32)
                .varint((radius * 16) as u32)
                .lu32(0);
            connection.send_one(publisher);
            self.published = Some(((cx, cz), radius));
        }

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

        let dimension = self.dimension;
        let (ready, missing, source) = {
            let world = self.shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
            let mut ready = Vec::new();
            let mut missing = Vec::new();

            for &(x, z) in &wanted {
                if ready.len() >= SENT_PER_TICK {
                    break;
                }

                match world.chunk_packet(x, z, |sections, biomes| (sections.to_vec(), biomes.to_vec())) {
                    Some(parts) => ready.push((x, z, parts)),
                    None if !self.loading.contains(&(x, z)) => missing.push((x, z)),
                    None => {}
                }
            }

            let source = (!missing.is_empty() && self.loading.len() < LOADING_AT_ONCE).then(|| Arc::new(world.source()));
            (ready, missing, source)
        };

        self.chunks_missing = wanted.len() > ready.len();

        if let Some(source) = source {
            for (x, z) in missing.into_iter().take(LOADING_AT_ONCE.saturating_sub(self.loading.len())) {
                let source = Arc::clone(&source);
                let arrived = self.arrived_tx.clone();

                self.loading.insert((x, z));
                tokio::task::spawn_blocking(move || {
                    // Игрок мог уйти — тогда чанк уже не нужен.
                    let _ = arrived.send((dimension, x, z, source.take(x, z)));
                });
            }
        }

        let mut batch = Vec::new();

        for (x, z, (sections, biomes)) in ready {
            batch.push(encode_chunk(dimension, x, z, &sections, &biomes));
            self.sent.insert((x, z));

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

/// Настоящее ли положение: числа, а не NaN и бесконечность, и в пределах мира.
fn valid_position((x, y, z): (f32, f32, f32)) -> bool {
    x.is_finite()
        && y.is_finite()
        && z.is_finite()
        && x.abs() <= WORLD_LIMIT
        && z.abs() <= WORLD_LIMIT
        && (LOWEST_Y..=HIGHEST_Y).contains(&y)
}

/// Лежит ли блок в мире, каким его видит клиент: по высоте — в подчанках
/// этого измерения, по X и Z — не за границей мира (тогда и соседний блок
/// не переполнит число).
fn block_in_world(dimension: Dimension, (x, y, z): (i32, i32, i32)) -> bool {
    let limit = WORLD_LIMIT as i32;

    heights(dimension).contains(&y) && x.abs() <= limit && z.abs() <= limit
}

fn distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
}

fn distance_f32(a: (f32, f32, f32), b: (f32, f32, f32)) -> f64 {
    distance((a.0 as f64, a.1 as f64, a.2 as f64), (b.0 as f64, b.1 as f64, b.2 as f64))
}

/// Блоки, которые в выживании не ломаются (прочность −1 на minecraft.wiki).
fn unbreakable(state: i32) -> bool {
    const NAMES: [&str; 15] = [
        "bedrock",
        "barrier",
        "command_block",
        "chain_command_block",
        "repeating_command_block",
        "structure_block",
        "jigsaw",
        "end_portal",
        "end_portal_frame",
        "end_gateway",
        "nether_portal",
        "moving_piston",
        "light",
        "test_block",
        "test_instance_block",
    ];

    crate::blocks::block_at_state(state).is_some_and(|name| NAMES.contains(&name))
}

/// Может ли игрок в этом режиме летать: в творческом и наблюдателем.
fn may_fly(game_mode: i32) -> bool {
    game_mode == crate::commands::GAME_MODE_CREATIVE || game_mode == crate::commands::GAME_MODE_SPECTATOR
}

/// Причины переноса в Move Player (li32 у переноса): 0 — неизвестна,
/// 3 — по команде.
const TELEPORT_UNKNOWN: i32 = 0;
const TELEPORT_BY_COMMAND: i32 = 3;

/// Перенос игрока (Move Player с видом «перенос»).
fn teleport_packet(runtime_id: u64, eyes: (f32, f32, f32), rotation: (f32, f32), cause: i32) -> Vec<u8> {
    let mut teleport = Out::packet(session::MOVE_PLAYER);
    teleport
        .varint(runtime_id as u32)
        .vec3f(eyes.0, eyes.1, eyes.2)
        .lf32(rotation.1)
        .lf32(rotation.0)
        .lf32(rotation.0)
        .u8(2) // перенос
        .bool(false)
        .varint(0)
        .li32(cause)
        .li32(0)
        .varint64(0);
    teleport.bytes
}

/// Блок в обоих слоях Bedrock: сам блок и вода, если он затоплен.
fn block_updates(x: i32, y: i32, z: i32, state: i32) -> [Vec<u8>; 2] {
    let mut update = Out::packet(session::UPDATE_BLOCK);
    update
        .block_position(x, y, z)
        .varint(bedrock_state(state))
        .varint(2) // соседям по сети
        .varint(0); // основной слой

    // И слой воды: затоплен ли теперь этот блок.
    let mut liquid = Out::packet(session::UPDATE_BLOCK);
    liquid
        .block_position(x, y, z)
        .varint(if waterlogged(state) { still_water() } else { air() })
        .varint(2)
        .varint(1);

    [update.bytes, liquid.bytes]
}

/// Самое длинное сообщение в чат, как у оригинала.
const MAX_CHAT: usize = 256;

/// Текст от клиента для чата и команд: без управляющих знаков и без «§»
/// (им клиенты красят текст — чужой цвет и подделанные строки ни к чему),
/// не длиннее `MAX_CHAT` знаков.
fn clean_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() && *c != '§').take(MAX_CHAT).collect::<String>().trim().to_string()
}

/// Команды, которые можно всем (у `commands::level_needed` — нулевой
/// уровень): неподтверждённому игроку Bedrock доступны только они.
fn open_to_everyone(command: &str) -> bool {
    matches!(command.split_whitespace().next().unwrap_or(""), "help" | "list" | "say")
}

/// Пакет Level Chunk со всеми подчанками и биомами измерения.
///
/// Поля по документации Mojang к r/26_u1 (dot/LevelChunkPacket.dot):
/// положение чанка, Dimension Id (число со знаком), число подчанков, кэш
/// и сами данные. `sections` и `biomes` — во всю нашу высоту −64..320;
/// клиенту уходят только подчанки его измерения (`sub_chunks`).
fn encode_chunk(dimension: Dimension, x: i32, z: i32, sections: &[Option<Arc<[u16]>>], biomes: &[u16]) -> Vec<u8> {
    let (lowest, count) = sub_chunks(dimension);
    // Сколько наших секций лежит ниже нижнего подчанка клиента.
    let skip = (lowest - world::MIN_Y / 16) as usize;
    let mut payload = Out::default();

    for index in 0..count {
        let y = (lowest + index as i32) as i8 as u8;

        match sections.get(skip + index).and_then(Option::as_ref) {
            None => {
                payload.u8(9).u8(1).u8(y);
                palette_storage(&mut payload, &[air()], &[0; 4096]);
            }
            Some(blocks) => {
                let mut palette: Vec<u32> = Vec::new();
                let mut known: HashMap<u32, u16> = HashMap::new();
                let mut indexes = vec![0u16; 4096];
                // Второй слой — вода в затопленных блоках (у Java это свойство
                // блока, у Bedrock — отдельный слой): 0 — воздух, 1 — вода.
                let mut water = vec![0u16; 4096];
                let mut wet = false;

                // Bedrock хранит блоки в порядке X, Z, Y; у нас — Y, Z, X.
                for bx in 0..16 {
                    for bz in 0..16 {
                        for by in 0..16 {
                            let java = blocks[(by * 256 + bz * 16 + bx) as usize] as i32;
                            let state = bedrock_state(java);

                            if waterlogged(java) {
                                water[(bx * 256 + bz * 16 + by) as usize] = 1;
                                wet = true;
                            }

                            let at = *known.entry(state).or_insert_with(|| {
                                palette.push(state);
                                (palette.len() - 1) as u16
                            });

                            indexes[(bx * 256 + bz * 16 + by) as usize] = at;
                        }
                    }
                }

                payload.u8(9).u8(if wet { 2 } else { 1 }).u8(y);
                palette_storage(&mut payload, &palette, &indexes);

                if wet {
                    palette_storage(&mut payload, &[air(), still_water()], &water);
                }
            }
        }
    }

    // Биомы: на каждый подчанк своя объёмная палитра (у нас клетка 4×4×4 —
    // здесь она раскладывается на блоки). Номера — по таблице
    // `tables::BIOMES`, в том числе биомы Незера.
    for index in skip..skip + count {
        let mut palette: Vec<u32> = Vec::new();
        let mut known: HashMap<u32, u16> = HashMap::new();
        let mut indexes = vec![0u16; 4096];

        for bx in 0..16 {
            for bz in 0..16 {
                for by in 0..16 {
                    let layer = index * 4 + by / 4;
                    let cell = layer * 16 + (bz / 4) * 4 + bx / 4;
                    let ours = biomes.get(cell).copied().unwrap_or(0) as usize;
                    let fallback = match dimension {
                        Dimension::Nether => NETHER_WASTES,
                        Dimension::Overworld => 1,
                    };
                    let biome = super::tables::BIOMES.get(ours).copied().unwrap_or(fallback);
                    let at = *known.entry(biome).or_insert_with(|| {
                        palette.push(biome);
                        (palette.len() - 1) as u16
                    });

                    indexes[bx * 256 + bz * 16 + by] = at;
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
        .zigzag32(bedrock_dimension(dimension))
        .varint(count as u32)
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

/// Change Dimension (номер 61). Поля — по документации Mojang к r/26_u1
/// (ChangeDimensionPacket.json): Dimension ID (число со знаком переменной
/// длины), Position (три float — у Bedrock это глаза), Respawn (переход
/// после смерти — у нас нет) и Loading Screen Id. Номер экрана загрузки —
/// необязательный uint32 (`brstd::optional<uint32_t>`, legacy_changelogs/
/// changelog_705: байт «есть ли» и само число); оригинал шлёт его, когда
/// игрок жив, и клиент потом называет этот номер в
/// ServerboundLoadingScreenPacket.
fn change_dimension_packet(dimension: Dimension, eyes: (f32, f32, f32), loading_screen: Option<u32>) -> Vec<u8> {
    let mut out = Out::packet(session::CHANGE_DIMENSION);
    out.zigzag32(bedrock_dimension(dimension)).vec3f(eyes.0, eyes.1, eyes.2).bool(false);

    match loading_screen {
        Some(id) => out.bool(true).lu32(id),
        None => out.bool(false),
    };

    out.bytes
}

/// Предметы, уже лежащие в мире измерения, и начало его журнала предметов:
/// из журнала о них не узнать — они попали туда до прихода игрока. Снимок
/// и начало журнала берутся под одним замком, чтобы ничего не выпало между
/// ними.
fn lying_items(shared: &Shared, dimension: Dimension, reader: crate::journal::Reader) -> (usize, Vec<(i32, Vec<u8>)>) {
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let lying: Vec<(i32, Vec<u8>)> = items
        .items()
        .iter()
        .filter(|item| item.block.is_none())
        .filter_map(|item| Some((item.entity_id, add_item_entity(item)?)))
        .collect();

    (items.watch_changes(reader), lying)
}

/// Убрать показанного игрока: из мира и из списка игроков (Tab).
fn forget_player(id: i32, shown: &Shown) -> [Vec<u8>; 2] {
    let mut list = Out::packet(session::PLAYER_LIST);
    list.u8(1).varint(1).uuid(shown.uuid);

    [remove_entity(id), list.bytes]
}

fn player_list_add(member: &Member, look: &skin::Look) -> Vec<u8> {
    let mut out = Out::packet(session::PLAYER_LIST);
    out.u8(0).varint(1).uuid(member.uuid).zigzag64(member.entity_id as i64).string(&member.name).string("").string("").li32(0);
    skin::encode(&mut out, look);
    out.bool(false).bool(false).bool(false).li32(0);
    out.bool(true); // verified
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

/// Флаги чужого игрока: приседает ли. Флаги уходят одним числом целиком,
/// поэтому вместе с приседанием — и те, без которых игрок выглядит не так:
/// ник над головой, дыхание, столкновения, тяжесть (MetadataFlags1).
fn other_entity_data(member: &Member) -> Vec<u8> {
    let mut out = Out::packet(session::SET_ENTITY_DATA);
    out.varint64(member.entity_id as u64).varint(1);
    out.varint(DATA_FLAGS).varint(DATA_LONG).zigzag64(other_flags(member));
    out.varint(0).varint(0); // свойства: целые, дробные
    out.varint64(0); // такт
    out.bytes
}

/// Animate: чужой игрок взмахнул рукой (действие 1 — swing_arm, без
/// источника взмаха; описание протокола, packet_animate).
fn swing(entity_id: i32) -> Vec<u8> {
    let mut out = Out::packet(session::ANIMATE);
    out.u8(1).varint64(entity_id as u64).lf32(0.0).bool(false);
    out.bytes
}

/// Ключи и виды значений в данных сущности (MetadataDictionary описания
/// протокола): флаги — long, ник над головой — строка.
const DATA_FLAGS: u32 = 0;
const DATA_NAMETAG: u32 = 4;
const DATA_STRING: u32 = 4;
const DATA_LONG: u32 = 7;

/// Флаги чужого игрока (MetadataFlags1): ник над головой показывать можно
/// и всегда (14, 15), лазит, дышит, сталкивается, падает (19, 35, 48, 49);
/// приседает (1) — если приседает.
fn other_flags(member: &Member) -> i64 {
    let base: i64 = (1 << 14) | (1 << 15) | (1 << 19) | (1 << 35) | (1 << 48) | (1 << 49);

    if member.sneaking { base | (1 << 1) } else { base }
}

/// Свойства сущности самого игрока: без флага тяжести он висит в воздухе
/// («ватный», полёт не выключается), без «может лазить» — не лезет по
/// лестницам. Номера флагов и ключей — описание протокола
/// (MetadataFlags1, MetadataDictionary).
fn own_entity_data(runtime_id: u64) -> Vec<u8> {
    const FLAGS: u32 = 0;
    const WIDTH: u32 = 53;
    const HEIGHT: u32 = 54;
    const LONG: u32 = 7;
    const FLOAT: u32 = 3;
    let flags: i64 = (1 << 19) | (1 << 35) | (1 << 48) | (1 << 49); // лазить, дышать, сталкиваться, тяжесть

    let mut out = Out::packet(session::SET_ENTITY_DATA);
    out.varint64(runtime_id).varint(3);
    out.varint(FLAGS).varint(LONG).zigzag64(flags);
    out.varint(WIDTH).varint(FLOAT).lf32(0.6);
    out.varint(HEIGHT).varint(FLOAT).lf32(1.8);
    out.varint(0).varint(0); // свойства: целые, дробные
    out.varint64(0); // такт
    out.bytes
}

/// Атрибуты игрока: скорость шага и здоровье. Без них клиент берёт что
/// попало, и движение кажется «ватным».
fn own_attributes(runtime_id: u64) -> Vec<u8> {
    let mut out = Out::packet(session::UPDATE_ATTRIBUTES);
    out.varint64(runtime_id).varint(2);

    for (name, max, value) in [("minecraft:movement", f32::MAX, 0.1), ("minecraft:health", 20.0, 20.0)] {
        out.lf32(0.0).lf32(max).lf32(value).lf32(0.0).lf32(max).lf32(value).string(name).varint(0);
    }

    out.varint64(0); // такт
    out.bytes
}

/// Определения биомов — пустые.
///
/// По документации Mojang к r/26_u1 (dot/BiomeDefinitionListPacket.dot)
/// пакет — это словарь «номер строки имени (short) → BiomeDefinitionData»
/// с длиной varuint32 и затем список строк-имён (varuint32 + строки).
/// Пустой словарь и пустой список — законный пакет: клиент оставляет свои
/// встроенные определения ванильных биомов. Все наши биомы — ванильные
/// (свой осенний лес показывается обычным лесом), так что прислать клиенту
/// нечего, а запись для ванильного биома заменила бы встроенную: без тегов
/// («frozen», «ocean», «overworld»…) и без данных генерации клиенту стало бы
/// хуже. Непустой список (54 записи по той же схеме) живой клиент 26.12
/// отвергал с bad_packet.
fn biome_definitions() -> Vec<u8> {
    let mut out = Out::packet(session::BIOME_DEFINITION_LIST);
    out.varint(0) // словарь определений
        .varint(0); // строки-имена
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
        .zigzag32(bedrock_game_mode(member.game_mode));
    // Данные сущности: без ника и флага «показывать ник» Bedrock имени над
    // головой не рисует.
    out.varint(2)
        .varint(DATA_FLAGS)
        .varint(DATA_LONG)
        .zigzag64(other_flags(member))
        .varint(DATA_NAMETAG)
        .varint(DATA_STRING)
        .string(&member.name);
    out.varint(0)
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

/// Наши команды с описаниями — клиент их подсказывает при наборе.
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

/// Символ разбора «сообщение» — текст до конца строки. Значение из
/// перечисления CommandRegistry::HardNonTerminal документации Mojang к
/// r/26_u1 (html/enums.html): Message = 0x100043, где 0x100000 — бит
/// нетерминала (прежний флаг «valid»). Номера типов из minecraft-data
/// (message 55, raw_text 58, string 44) отстали от этой версии на 12:
/// 0x100037 и 0x10003a — внутренние правила грамматики
/// (HasPropertySelector, CoordXInt), с ними клиент падал при открытии чата.
const PARSE_MESSAGE: u32 = 0x10_0043;

/// Список команд (AvailableCommandsPacket, Cereal). Порядок и типы полей —
/// по документации Mojang к r/26_u1: массивы с длиной varuint32, числа без
/// пометки Compression — фиксированной ширины (little-endian), уровень
/// прав — перечисление, которое передаётся строкой («Any»).
///
/// У каждой команды один необязательный аргумент «текст до конца строки»:
/// клиент пропускает любые аргументы, разбирает их сервер.
fn available_commands() -> Vec<u8> {
    let mut out = Out::packet(session::AVAILABLE_COMMANDS);
    out.varint(0) // Enum Values
        .varint(0) // Chained Subcommand Values
        .varint(0) // Post Fixes
        .varint(0) // Enum Data
        .varint(0) // Chained Subcommand Data
        .varint(COMMANDS.len() as u32);

    for (name, description) in COMMANDS {
        out.string(name)
            .string(description)
            .lu16(0) // Flags
            .string("Any") // Permission Level: права проверяет сервер
            .li32(-1) // Alias Enum: синонимов нет
            .varint(0) // Chained Subcommand Indexes
            .varint(1) // Overloads: один вариант
            .bool(false) // isChaining
            .varint(1) // Parameter Data: один параметр
            .string("args")
            .lu32(PARSE_MESSAGE)
            .bool(true) // Is Optional?
            .u8(0); // Options
    }

    out.varint(0) // Soft Enums
        .varint(0); // Constraints
    out.bytes
}

/// Способности по режиму игры: строить и ломать — в выживании и творческом;
/// летать и ломать мгновенно — в творческом; наблюдатель летает сквозь
/// блоки и ничего не трогает.
///
/// Номера — AbilitiesIndex из документации Mojang к r/26_u1 (html/enums.html),
/// всего их AbilityCount = 20. В `allowed` слоя отмечены все: способность,
/// не отмеченная там, клиент берёт по своему разумению — так в выживании
/// оставался доступен полёт. Значения в `enabled` поэтому заданы для всех.
fn abilities(runtime_id: u64, game_mode: i32) -> Vec<u8> {
    use crate::commands::{GAME_MODE_ADVENTURE, GAME_MODE_CREATIVE, GAME_MODE_SPECTATOR};

    const BUILD: u32 = 1 << 0;
    const MINE: u32 = 1 << 1;
    const DOORS: u32 = 1 << 2;
    const CONTAINERS: u32 = 1 << 3;
    const ATTACK_PLAYERS: u32 = 1 << 4;
    const ATTACK_MOBS: u32 = 1 << 5;
    const INVULNERABLE: u32 = 1 << 8;
    const FLYING: u32 = 1 << 9;
    const MAY_FLY: u32 = 1 << 10;
    const INSTANT_BUILD: u32 = 1 << 11;
    const FLY_SPEED: u32 = 1 << 13;
    const WALK_SPEED: u32 = 1 << 14;
    const NO_CLIP: u32 = 1 << 17;
    const VERTICAL_FLY_SPEED: u32 = 1 << 19;
    const ALL: u32 = (1 << 20) - 1;

    let play = DOORS | CONTAINERS | ATTACK_PLAYERS | ATTACK_MOBS;
    let enabled = match game_mode {
        GAME_MODE_CREATIVE => play | BUILD | MINE | INVULNERABLE | MAY_FLY | INSTANT_BUILD,
        GAME_MODE_ADVENTURE => play,
        GAME_MODE_SPECTATOR => INVULNERABLE | MAY_FLY | FLYING | NO_CLIP,
        _ => play | BUILD | MINE,
    };

    let mut out = Out::packet(session::UPDATE_ABILITIES);
    out.li64(runtime_id as i64)
        .u8(1) // permission_level: участник
        .u8(0) // command_permission
        .u8(1) // один слой — основной
        .lu16(1)
        .lu32(ALL)
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
    sneaking: bool,
    swings: u32,
}

/// Начал ломать (из Player Auth Input): ломает только в творческом режиме.
const START_BREAK: u32 = 100;

/// Виды действия предметом (TransactionUseItem): щелчок по блоку и поломка.
const CLICK: u32 = 0;
const BREAK: u32 = 2;

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

/// Что было в остатке Player Auth Input.
struct Input {
    /// Флаги входа (InputData).
    flags: u128,
    actions: Vec<ItemUse>,
    handled: Option<super::inventory::Handled>,
}

/// Действия с блоками из остатка Player Auth Input (после позиции):
/// ломание приходит здесь, когда ломание проверяет сервер.
fn read_input_actions(reader: &mut In, bag: &mut Bag, creative: bool) -> Input {
    const ITEM_INTERACT: u128 = 1 << 34;
    const BLOCK_ACTION: u128 = 1 << 35;
    const ITEM_STACK_REQUEST: u128 = 1 << 36;
    const PREDICTED_VEHICLE: u128 = 1 << 45;

    let mut found = Vec::new();
    let mut handled = None;
    let mut flags = 0u128;

    let mut read = || -> Option<()> {
        reader.take(8 + 4)?; // move_vector, head_yaw

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

        if flags & BLOCK_ACTION != 0 {
            for _ in 0..reader.zigzag32()? {
                let action = reader.zigzag32()?;

                if matches!(action, 0 | 1 | 18 | 26 | 27) {
                    let position = (reader.zigzag32()?, reader.zigzag32()?, reader.zigzag32()?);
                    let face = reader.zigzag32()?;

                    // «Предсказал поломку» — клиент докопал. Творческий режим
                    // ломает с первого удара: такие отмечены отдельно.
                    match action {
                        26 => found.push(ItemUse { kind: BREAK, position, face, held: None, click_y: 0.0 }),
                        0 => found.push(ItemUse { kind: START_BREAK, position, face, held: None, click_y: 0.0 }),
                        _ => {}
                    }
                }
            }
        }

        // Транспорт — после действий с блоками (документация Mojang 944).
        if flags & PREDICTED_VEHICLE != 0 {
            reader.take(8)?;
            reader.varint64()?;
        }

        Some(())
    };

    read();
    Input { flags, actions: found, handled }
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
        let actions = read_input_actions(&mut In::new(&out.bytes), &mut bag, true).actions;
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
        let actions = read_input_actions(&mut In::new(&out.bytes), &mut bag, true).actions;

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
        let Input { actions, handled, .. } = read_input_actions(&mut In::new(&out.bytes), &mut bag, false);

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
    fn kelp_and_waterlogged_blocks_hold_water() {
        let state = |text: &str| crate::blocks::state_from_text(text).expect(text);

        assert!(waterlogged(state("kelp")));
        assert!(waterlogged(state("seagrass")));
        assert!(waterlogged(state("oak_stairs[waterlogged=true]")));
        assert!(!waterlogged(state("oak_stairs[waterlogged=false]")));
        assert!(!waterlogged(state("stone")));
        assert_ne!(still_water(), air());
    }

    #[test]
    fn bedrock_states_map_back() {
        assert_eq!(java_state(bedrock_state(1)), Some(1));
        assert_eq!(java_state(air()), Some(world::AIR));
    }

    #[test]
    fn biome_definitions_are_an_empty_map_and_list() {
        assert_eq!(biome_definitions(), [0x7a, 0x00, 0x00]);
    }

    #[test]
    fn available_commands_bytes_of_one_command() {
        let packet = available_commands();

        // Номер пакета 0x4c, пять пустых списков, 14 команд.
        assert_eq!(packet[..7], [0x4c, 0, 0, 0, 0, 0, 14]);

        // Первая команда — байт в байт.
        let mut help = vec![4];
        help.extend_from_slice(b"help");
        help.push("Список команд".len() as u8);
        help.extend_from_slice("Список команд".as_bytes());
        help.extend_from_slice(&[0, 0]); // Flags: uint16
        help.extend_from_slice(&[3, b'A', b'n', b'y']); // Permission Level: строка
        help.extend_from_slice(&[0xff; 4]); // Alias Enum: int32 −1
        help.push(0); // Chained Subcommand Indexes
        help.extend_from_slice(&[1, 0, 1]); // один вариант, не цепочка, один параметр
        help.extend_from_slice(&[4, b'a', b'r', b'g', b's']);
        help.extend_from_slice(&[0x43, 0x00, 0x10, 0x00]); // Parse Symbol: uint32 Message
        help.extend_from_slice(&[1, 0]); // необязательный, без опций
        assert_eq!(packet[7..7 + help.len()], help[..]);

        // Хвост: без мягких перечислений и ограничений.
        assert_eq!(packet[packet.len() - 2..], [0, 0]);
    }

    #[test]
    fn available_commands_read_back() {
        let packet = available_commands();
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(session::AVAILABLE_COMMANDS));
        for _ in 0..5 {
            assert_eq!(reader.varint(), Some(0));
        }
        assert_eq!(reader.varint(), Some(COMMANDS.len() as u32));

        for (name, description) in COMMANDS {
            assert_eq!(reader.string().as_deref(), Some(name));
            assert_eq!(reader.string().as_deref(), Some(description));
            assert_eq!(reader.take(2), Some(&[0u8, 0][..]));
            assert_eq!(reader.string().as_deref(), Some("Any"));
            assert_eq!(reader.li32(), Some(-1));
            assert_eq!(reader.varint(), Some(0));
            assert_eq!(reader.varint(), Some(1));
            assert_eq!(reader.bool(), Some(false));
            assert_eq!(reader.varint(), Some(1));
            assert_eq!(reader.string().as_deref(), Some("args"));
            assert_eq!(reader.take(4), Some(&PARSE_MESSAGE.to_le_bytes()[..]));
            assert_eq!(reader.bool(), Some(true));
            assert_eq!(reader.u8(), Some(0));
        }

        assert_eq!(reader.varint(), Some(0));
        assert_eq!(reader.varint(), Some(0));
        assert_eq!(reader.take(1), None, "лишние байты в конце");
    }

    /// Пропускает палитровое хранилище и отдаёт его палитру.
    fn read_storage(reader: &mut In) -> Vec<u32> {
        let header = reader.u8().expect("заголовок хранилища");

        if header == 1 {
            return vec![reader.zigzag32().expect("единственное значение") as u32];
        }

        let bits = (header >> 1) as usize;
        let per_word = 32 / bits;
        reader.take(4096_usize.div_ceil(per_word) * 4).expect("слова хранилища");
        let size = reader.zigzag32().expect("размер палитры");
        (0..size).map(|_| reader.zigzag32().expect("запись палитры") as u32).collect()
    }

    /// Подчанки из Level Chunk: номер и палитра основного слоя.
    type SubChunks = Vec<(i8, Vec<u32>)>;

    /// Секции чанка во всю нашу высоту.
    type Sections = Vec<Option<Arc<[u16]>>>;

    /// Разбирает Level Chunk: измерение, номера подчанков с палитрами
    /// основного слоя и палитры биомов.
    fn read_chunk(packet: &[u8]) -> (i32, SubChunks, Vec<Vec<u32>>) {
        let mut reader = In::new(packet);
        assert_eq!(reader.varint(), Some(session::LEVEL_CHUNK));
        assert_eq!((reader.zigzag32(), reader.zigzag32()), (Some(3), Some(-2)));
        let dimension = reader.zigzag32().expect("измерение");
        let count = reader.varint().expect("число подчанков") as usize;
        assert_eq!(reader.bool(), Some(false), "без кэша");
        let length = reader.varint().expect("длина данных") as usize;
        let payload = reader.take(length).expect("данные");
        assert_eq!(reader.take(1), None, "лишние байты после данных");

        let mut reader = In::new(payload);
        let mut sub_chunks = Vec::new();

        for _ in 0..count {
            assert_eq!(reader.u8(), Some(9), "версия подчанка");
            let layers = reader.u8().expect("число слоёв");
            let y = reader.u8().expect("номер подчанка") as i8;
            let palette = read_storage(&mut reader);

            for _ in 1..layers {
                read_storage(&mut reader);
            }

            sub_chunks.push((y, palette));
        }

        let biomes = (0..count).map(|_| read_storage(&mut reader)).collect();
        assert_eq!(reader.u8(), Some(0), "край мира");
        assert_eq!(reader.take(1), None, "лишние байты в конце");

        (dimension, sub_chunks, biomes)
    }

    /// Чанк во всю нашу высоту: камень в секции y 0..15, земля на дне мира
    /// (−64) и незерак на высоте 128 — выше Незера Bedrock.
    fn tall_chunk() -> (Sections, Vec<u16>) {
        let state = |name: &str| crate::blocks::state_by_name(name).expect(name) as u16;
        let mut sections: Sections = vec![None; 24];
        let filled = |block: u16| -> Option<Arc<[u16]>> { Some(vec![block; 4096].into()) };

        sections[0] = filled(state("dirt"));
        sections[4] = filled(state("stone"));
        sections[12] = filled(state("netherrack"));

        (sections, vec![0; 24 * 4 * 16])
    }

    #[test]
    fn nether_chunk_has_eight_sub_chunks_from_zero() {
        let state = |name: &str| bedrock_state(crate::blocks::state_by_name(name).expect(name));
        let (sections, mut biomes) = tall_chunk();
        // Внизу — багровый лес, выше — пустоши (номера наших биомов).
        let (wastes, crimson) = (crate::world::terrain::Biome::NetherWastes, crate::world::terrain::Biome::CrimsonForest);
        biomes.fill(wastes.number() as u16);
        biomes[4 * 4 * 16..5 * 4 * 16].fill(crimson.number() as u16);
        let (dimension, sub_chunks, biome_palettes) = read_chunk(&encode_chunk(Dimension::Nether, 3, -2, &sections, &biomes));

        assert_eq!(dimension, 1);
        assert_eq!(sub_chunks.iter().map(|(y, _)| *y).collect::<Vec<_>>(), (0..8).collect::<Vec<i8>>());
        // y 0..15 — первый подчанк; дно мира и высота 128 не уходят.
        assert_eq!(sub_chunks[0].1, vec![state("stone")]);
        assert!(sub_chunks[1..].iter().all(|(_, palette)| *palette == vec![air()]));
        assert!(!sub_chunks.iter().any(|(_, palette)| palette.contains(&state("dirt")) || palette.contains(&state("netherrack"))));
        // Биомы — свои, по таблице Bedrock: багровый лес 179, пустоши «hell» 8.
        assert_eq!(biome_palettes[0], vec![179]);
        assert_eq!(biome_palettes[1..], vec![vec![NETHER_WASTES]; 7]);
    }

    #[test]
    fn overworld_chunk_keeps_its_full_height() {
        let state = |name: &str| bedrock_state(crate::blocks::state_by_name(name).expect(name));
        let (sections, biomes) = tall_chunk();
        let (dimension, sub_chunks, biome_palettes) =
            read_chunk(&encode_chunk(Dimension::Overworld, 3, -2, &sections, &biomes));

        assert_eq!(dimension, 0);
        assert_eq!(sub_chunks.iter().map(|(y, _)| *y).collect::<Vec<_>>(), (-4..20).collect::<Vec<i8>>());
        assert_eq!(sub_chunks[0].1, vec![state("dirt")]);
        assert_eq!(sub_chunks[4].1, vec![state("stone")]);
        assert_eq!(sub_chunks[12].1, vec![state("netherrack")]);
        assert_eq!(biome_palettes.len(), 24);
    }

    #[test]
    fn client_heights_by_dimension() {
        assert_eq!(heights(Dimension::Overworld), world::MIN_Y..world::MIN_Y + world::WORLD_HEIGHT);
        assert_eq!(heights(Dimension::Nether), 0..128);
        assert!(block_in_world(Dimension::Nether, (0, 127, 0)));
        assert!(!block_in_world(Dimension::Nether, (0, 128, 0)));
        assert!(!block_in_world(Dimension::Nether, (0, -1, 0)));
        assert!(block_in_world(Dimension::Overworld, (0, -64, 0)));
    }

    /// Change Dimension по полям документации Mojang: измерение, место,
    /// «после смерти», необязательный номер экрана загрузки.
    #[test]
    fn change_dimension_reads_back() {
        let packet = change_dimension_packet(Dimension::Nether, (12.5, 71.62, -3.5), Some(7));
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(61));
        assert_eq!(reader.zigzag32(), Some(1));
        assert_eq!((reader.lf32(), reader.lf32(), reader.lf32()), (Some(12.5), Some(71.62), Some(-3.5)));
        assert_eq!(reader.bool(), Some(false), "не после смерти");
        assert_eq!(reader.bool(), Some(true), "номер экрана есть");
        assert_eq!(reader.li32(), Some(7));
        assert_eq!(reader.take(1), None, "лишние байты в конце");

        // Без экрана загрузки — один нулевой байт вместо номера.
        let packet = change_dimension_packet(Dimension::Overworld, (0.0, 0.0, 0.0), None);
        assert_eq!(packet[0], 0x3d);
        assert_eq!(packet[1], 0); // обычный мир
        assert_eq!(packet[2..], [&[0u8; 12][..], &[0, 0][..]].concat()[..]);
    }

    #[test]
    fn listed_commands_are_unique_and_lowercase() {
        let names: HashSet<&str> = COMMANDS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names.len(), COMMANDS.len());

        // Все имена — строчной латиницей, как их ждёт клиент.
        assert!(names.iter().all(|name| name.bytes().all(|b| b.is_ascii_lowercase())));

        // Символ «сообщение» — нетерминал грамматики (бит 0x100000).
        assert_eq!(PARSE_MESSAGE & 0xfff0_0000, 0x10_0000);
    }
}
