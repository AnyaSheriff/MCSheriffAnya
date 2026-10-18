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
use super::containers as screens;
use super::health;
use super::session::{self, Connection, Identity};
use super::inventory::{Bag, Event};
use super::stations;
use super::skin;
use crate::containers;
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
        // Энд у Bedrock — от 0 до 256: 16 подчанков (minecraft.wiki, The End).
        Dimension::End => (0, 16),
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
        Dimension::End => 2,
    }
}

/// Биом Незера у Bedrock — «hell» (nether_wastes у Java), номер 8 по
/// таблице minecraft.wiki (Biome ID, Bedrock Edition): им считается место
/// Незера, биом которого не нашёлся в таблице.
const NETHER_WASTES: u32 = 8;

/// Биом Энда у Bedrock — «the_end», номер 9 (minecraft.wiki, Biome ID,
/// Bedrock Edition).
const THE_END: u32 = 9;

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

/// Сколько тактов после открытия окна Container Close с чужим номером ещё
/// считается уборкой клиента, а не закрытием нашего окна (0,2 с).
const EARLY_CLOSE_TICKS: u64 = 4;

/// Через сколько тактов щелчок по блоку в мире доказывает, что экран, открытый
/// у нас, у клиента уже закрыт: с экраном в руках по миру не щёлкнуть.
const STALE_WINDOW_TICKS: u64 = 10;
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

/// Бит «бежит» во вводе (InputFlag «Sprinting») [проверить].
const SPRINTING: u128 = 1 << 20;

/// Таблица «состояние Java → номер Bedrock» с поправками к данным minecraft-data.
fn java_to_bedrock() -> &'static [u16] {
    static TABLE: OnceLock<Vec<u16>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: Vec<u16> = JAVA_TO_BEDROCK.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
        swap_hopper_power(&mut table);
        table
    })
}

/// Воронка: у Bedrock `toggle_bit=1` — воронка под сигналом (отключена), у Java `enabled=false`.
/// Таблица J2B даёт наоборот; сверка с BDS (tools/blackbox/v-blocks.js, воронка рядом с блоком
/// красного камня и без него): запитанная — toggle_bit=1, обычная — 0. Меняем номера парами.
fn swap_hopper_power(table: &mut [u16]) {
    let (Some((low, high)), Some(rule)) = (crate::blocks::states_of("hopper"), crate::blocks::orientation("hopper")) else { return };

    for on in low..=high {
        if rule.value(on, "enabled") != Some("true") {
            continue;
        }

        if let Some(off) = rule.with(on, "enabled", "false") {
            let (a, b) = (on as usize, off as usize);

            if a < table.len() && b < table.len() {
                table.swap(a, b);
            }
        }
    }
}

/// Сетевой номер Bedrock для состояния Java.
pub fn bedrock_state(java: i32) -> u32 {
    match java_to_bedrock().get(java.max(0) as usize) {
        Some(&rid) => u32::from(rid),
        None => air(),
    }
}

/// Состояние Java для сетевого номера Bedrock: первое состояние Java,
/// которое переводится в этот номер. Нужно, когда клиент ставит блок.
pub fn java_state(bedrock: u32) -> Option<i32> {
    static REVERSE: OnceLock<Vec<i32>> = OnceLock::new();
    let reverse = REVERSE.get_or_init(|| {
        let mut table = vec![-1; super::tables::BLOCK_STATES];

        for (java, &target) in java_to_bedrock().iter().enumerate().rev() {
            let target = usize::from(target);

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
        u32::from(java_to_bedrock()[world::AIR.max(0) as usize])
    })
}

/// Update Player Game Type (0x97): смена режима у BDS.
const UPDATE_PLAYER_GAME_TYPE: u32 = 0x97;

/// Режим игры для Bedrock. Выживание, творческий и приключение совпадают
/// с Java; наблюдатель у Bedrock — 6 (GameMode в описании протокола), а 3
/// там — «наблюдатель выживания».
fn bedrock_game_mode(java: i32) -> i32 {
    match java {
        crate::commands::GAME_MODE_SPECTATOR => 6,
        mode => mode,
    }
}

/// Правила мира для StartGame (GameRuleVarint[] в описании протокола):
/// число правил, затем у каждого имя, «можно менять», тип (1 — да/нет)
/// и значение.
fn game_rules(settings: &crate::config::mcsheriffanya::Settings) -> Vec<u8> {
    let mut out = Out::default();
    out.varint(1).string("showcoordinates").bool(false).varint(1).bool(settings.bedrock_show_coordinates);
    out.bytes
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
    progress: crate::progress::Progress,
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

    /// Чужие игроки, чью смерть клиент уже видел (death_animation).
    dead_shown: HashSet<i32>,

    /// Где клиент видит вагонетки: чтобы слать движение, только когда
    /// вагонетка сдвинулась.
    carts_at: HashMap<i32, (f32, f32, f32, f32)>,

    /// Где клиент видит подожжённый динамит (effects.rs).
    primed_at: HashMap<i32, (f32, f32, f32)>,

    /// Где клиент видит мобов: место, поворот тела и головы, наклон.
    mobs_at: HashMap<i32, [f32; 6]>,
    /// Что клиент знает о виде моба (детёныш, цвет овцы…): при перемене шлём
    /// Set Actor Data.
    mobs_look: HashMap<i32, super::mobdata::Look>,
    carts_data: HashMap<i32, Vec<(u32, super::mobdata::Value)>>,
    entity_links: super::entity_links::Links,
    /// Где клиент видит зелья, оки Края, ракеты, падающие блоки и шарики опыта.
    things_at: HashMap<i32, super::things::Thing>,

    /// Где клиент видит блоки рамок (номер сущности рамки у Java → клетка):
    /// чтобы убрать блок, когда рамка ушла.
    frames_at: HashMap<i32, (i32, i32, i32)>,
    /// Такт сервера, когда игрок последний раз ударил: сила удара растёт
    /// с перезарядкой руки, как у Java.
    last_attack: u64,
    state_view: super::player_state::View,
    /// Тело для общего кода боя и использования предметов (network/play/body.rs):
    /// в нём тянется натяжение лука, еда, поднятый щит.
    body: crate::network::play::Body,
    radius: i32,
    sent: HashSet<(i32, i32)>,
    /// Кого из других игроков этот клиент видит: где, UUID, версия скина.
    shown: HashMap<i32, Shown>,
    /// Журнал мира и чата: докуда разослано.
    reader: crate::journal::Reader,
    events_from: usize,
    /// Что клиент знает о погоде: идёт ли дождь, гроза (Level Event).
    actors_shown: HashMap<containers::Pos, Vec<u8>>,
    brush_since: Option<u64>,
    seen_maps: HashMap<i32, Vec<u8>>,
    weather_shown: (bool, bool),
    ender: Vec<Option<crate::inventory::Stack>>,
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
    /// Открытое окно хранилища, если есть.
    window: Option<OpenWindow>,
    /// Номер окна станка (верстак, стол зачарования, наковальня, точило),
    /// открытого у клиента; сами ячейки — `Bag::station`.
    station_window: Option<u8>,
    sign_editor: Option<((i32,i32,i32),bool)>,
    lectern: Option<(u8,(i32,i32,i32))>,
    /// Такт, когда открыто последнее окно (хранилища или станка): ранний
    /// Container Close с чужим номером — не закрытие этого окна.
    opened_at: u64,
    /// Номер последнего открытого окна — следующее получит на один больше.
    next_window: u8,
    /// Окну хранилища нужен ещё один полный Inventory Content в ближайшем
    /// такте: так делает BDS у всех видов, кроме печей и стойки.
    resync_window: bool,
    /// Крышки сундуков, как их последний раз показали клиенту: открыта ли.
    /// Оригинал повторяет крышку раз в 5 тактов — клиенту уходит только
    /// перемена.
    lids: HashMap<(i32, i32, i32), bool>,
    /// Ходы поршней в пути: номер такта, когда они кончатся, и сам ход —
    /// изменения блоков в их клетках клиенту не шлются (pistons.rs).
    piston_moves: Vec<(u64, std::sync::Arc<crate::world::PistonMove>)>,
    /// Пакеты хода поршня, ждущие своего такта: конец хода и настоящие блоки.
    piston_later: Vec<(u64, Vec<Vec<u8>>)>,
    /// Блок, по которому последний щелчок сработал как действие (рычаг,
    /// сундук): повторы щелчка по нему, пока кнопка зажата, пропускаются.
    last_use: Option<(i32, i32, i32)>,
    /// Здоровье, еда, падение, смерть (health.rs).
    health: health::Health,
}

/// Окно хранилища, открытое у клиента. Его ячейки, какими их знает клиент,
/// — в инвентаре (`Bag::window_slots`).
struct OpenWindow {
    /// Номер окна у клиента.
    id: u8,
    kind: containers::Kind,
    /// Места хранилища (половины сундука) в порядке ячеек окна.
    halves: Vec<containers::Pos>,
    /// Свойства окна (печь, стойка), какими их знает клиент.
    props: Vec<(i32, i32)>,
    /// Вагонетка-хранилище, чьё это окно: ячейки — её, мест в мире нет
    /// (`halves` пусто), смотрящих не считают (как у Java, `cart`).
    entity: Option<i32>,
}

impl BedrockWorld {
    /// Отправляет всё, что нужно для появления в мире.
    ///
    /// Вызывается после получения разрешения Sessions::acquire.
    pub fn enter(shared: Arc<Shared>, connection: &Connection, identity: Identity, control: crate::sessions::Control) -> Option<BedrockWorld> {
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

        let health = health::Health::new(saved.as_ref().map(|data| &data.vitals));
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
            shoved: ((0.0, 0.0, 0.0), 0),
            unsettled: 0,
        };

        // acquire в session.rs уже дождался сохранения прежнего владельца.
        let joined = {
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
            let joined = players.join(member, entity_id).is_some();
            if joined { players.connection(identity.uuid, control); }
            joined
        };
        if !joined {
            log_info!("Bedrock: вход без освобождения прежней сессии: {}", identity.name);
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
        crate::plugins::player_join(&shared, &identity.name);
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
            .raw(&game_rules(&shared.settings))
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

        connection.send(&[start.bytes, items.bytes, menu.bytes, stations::crafting_data_packet().to_vec(), biome_definitions()]);
        // Определения свойств сущностей (свинья, корова, курица), как у BDS после Start Game.
        connection.send(&super::mobprops::definitions());

        let reader = shared.next_reader();
        let events_from = shared.world_in(dimension).lock().expect("мир захвачен другим потоком").watch_events(reader);
        let chat_from = shared.chat.lock().expect("чат захвачен другим потоком").count();

        let (items_from, lying) = lying_items(&shared, dimension, reader);
        let (arrived_tx, arrived_rx) = mpsc::unbounded_channel();

        let mut body = crate::network::play::Body::new(identity.uuid, identity.name.clone(), entity_id);
        body.restore_respawn(saved.as_ref().and_then(|data| data.respawn));
        let mut player = BedrockWorld {
            progress: crate::progress::Progress::load(&identity.uuid),
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
            dead_shown: HashSet::new(),
            carts_at: HashMap::new(),
            primed_at: HashMap::new(),
            mobs_at: HashMap::new(),
            mobs_look: HashMap::new(),
            carts_data: HashMap::new(),
            entity_links: super::entity_links::Links::default(),
            things_at: HashMap::new(),
            frames_at: HashMap::new(),
            last_attack: 0,
            state_view: super::player_state::View::default(),
            body,
            radius: START_RADIUS,
            sent: HashSet::new(),
            shown: HashMap::new(),
            reader,
            events_from,
            actors_shown: HashMap::new(),
            brush_since: None,
            seen_maps: HashMap::new(),
            weather_shown: (false, false),
            ender: saved.as_ref().map_or_else(crate::playerdata::empty_ender, |data| data.ender.clone()),
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
            window: None,
            station_window: None,
            sign_editor: None,
            lectern: None,
            opened_at: 0,
            next_window: 0,
            resync_window: false,
            lids: HashMap::new(),
            piston_moves: Vec::new(),
            piston_later: Vec::new(),
            last_use: None,
            health,
        };

        player.radius = player.radius.min(player.max_radius());
        player.send_chunks(connection);

        let mut status = Out::packet(session::PLAY_STATUS);
        status.i32_be(3);
        // При входе, как у BDS: рюкзак, броня, интерфейс, вторая рука и Player Hotbar.
        let mut packets = player.bag.open_sync_packets();

        for (entity_id, packet) in lying {
            player.items_shown.insert(entity_id);
            packets.push(packet);
        }

        let may_tnt = crate::permissions::allowed(&player.shared, &player.identity.uuid, crate::custom_tnt::GIVE_NODE);
        packets.push(available_commands(may_tnt));
        packets.push(abilities(runtime_id, game_mode));
        packets.push(own_entity_data(runtime_id));
        packets.push(health::attributes(runtime_id, &player.health.vit));
        packets.push(status.bytes);
        connection.send(&packets);

        log_info!("{} (Bedrock) появился в мире", player.identity.name);
        Some(player)
    }

    /// Пакет от клиента после входа.
    pub fn handle(&mut self, connection: &Connection, id: u32, payload: &[u8]) {
        match id {
            session::BLOCK_ACTOR_DATA => {
                let mut r=In::new(payload);
                let (Some(x),Some(y),Some(z))=(r.zigzag32(),r.zigzag32(),r.zigzag32()) else{return;};
                let Some((pos,front))=self.sign_editor.take().filter(|(p,_)|*p==(x,y,z)) else{return;};
                if distance((self.position.0 as f64,self.position.1 as f64,self.position.2 as f64),(x as f64+0.5,y as f64+0.5,z as f64+0.5))>6.0{return;}
                let Some(lines)=super::signs::read(&mut r).and_then(|v|super::signs::lines(&v,front)) else{return;};
                let mut world=self.shared.world_in(self.dimension).lock().expect("мир");
                if !crate::redstone::is_sign(world.get_block(x,y,z)) || world.sign(x,y,z).is_some_and(|s|s.waxed) {return;}
                world.with_sign(x,y,z,|s|s.side_mut(front).lines=lines);
                if let Some(kind)=crate::redstone::block_entity_type(world.get_block(x,y,z)) && let Some(data)=world.sign(x,y,z) {
                    let nbt=data.nbt();world.note_block_entity(x,y,z,kind,nbt);
                }
                if let Some(packet)=super::signs::actor(&world,pos) {connection.send(&[packet]);}
            }
            125 => {
                let mut r=In::new(payload);
                let (Some(page),Some(_),Some(x),Some(y),Some(z))=(r.u8(),r.u8(),r.zigzag32(),r.zigzag32(),r.zigzag32()) else{return;};
                let Some((id,pos))=self.lectern.filter(|(_,p)|*p==(x,y,z)) else{return;};
                if distance((self.position.0 as f64,self.position.1 as f64,self.position.2 as f64),(x as f64+0.5,y as f64+0.5,z as f64+0.5))>6.0{return;}
                let mut world=self.shared.world_in(self.dimension).lock().expect("мир");
                let mut packets=Vec::new();
                if page==255 {
                    let book=crate::redstone::lectern::take_book(&mut world,pos);drop(world);
                    if let Some(book)=book {let (changed,left)=self.bag.inventory.put(book);self.bag.touched(&changed);packets.extend(self.bag.open_sync_packets());if let Some(left)=left{self.throw_out(left);}}
                    self.lectern=None;packets.push(screens::close_packet(id,25,true));
                } else {
                    crate::redstone::lectern::turn_page(&mut world,pos,100+i32::from(page));packets.extend(super::lectern::actor(&world,pos));
                }
                connection.send(&packets);
            }
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
                let input = read_input_actions(&mut reader);
                if let Some((vehicle, rotation)) = input.vehicle {
                    if yaw.is_finite() && pitch.is_finite() { self.rotation = (yaw, pitch); }
                    if !self.health.vit.dead && self.changing.is_none() && self.teleport.is_none() && self.game_mode != crate::commands::GAME_MODE_SPECTATOR {
                        self.body.drive(&self.shared, &self.stance(), vehicle, (f64::from(x), f64::from(y), f64::from(z)), rotation, input.flags);
                    }
                } else {
                    self.take_position(connection, (x, y, z), (yaw, pitch), input.flags);
                }

                // Присел или встал — остальные узнают из общего списка.
                let sneaking = input.flags & SNEAKING != 0;
                if sneaking != self.sneaking {
                    self.sneaking = sneaking;
                    if sneaking { self.body.dismount(&self.shared, self.dimension); }
                    self.shared.players.lock().expect("список игроков захвачен другим потоком").set_sneaking(&self.identity.uuid, sneaking);
                }

                // В выживании летать нельзя: клиент, решивший взлететь,
                // снова получает свои способности — и падает.
                if input.flags & START_FLYING != 0 && !may_fly(self.game_mode) {
                    connection.send(&[abilities(self.runtime_id, self.game_mode)]);
                }

                let (actions, request) = (input.actions, input.request);

                if let Some(request) = request {
                    let (handled, mut packets) = self.stack_requests(&request);
                    packets.push(handled.response);
                    packets.extend(self.station_events(handled.events));
                    connection.send(&packets);

                    for stack in handled.thrown {
                        self.throw_out(stack);
                    }
                }

                for action in actions {
                    self.use_item(connection, action);
                }
            }
            306 => {
                let Some((pos, slot, disabled)) = screens::read_crafter_toggle(payload) else { return; };
                if self.game_mode == crate::commands::GAME_MODE_SPECTATOR || self.health.vit.dead
                    || self.changing.is_some() || !self.within_reach((pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5))
                    || !self.window.as_ref().is_some_and(|w| w.kind == containers::Kind::Crafter && w.halves == [pos]) { return; }
                let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                crate::redstone::crafter::toggle_slot(&mut world, pos, usize::from(slot), !disabled);
                connection.send(&super::block_actors::live(&world, pos).into_iter().collect::<Vec<_>>());
            }
            session::ITEM_STACK_REQUEST => {
                let (handled, mut packets) = self.stack_requests(payload);
                packets.push(handled.response);
                packets.extend(self.station_events(handled.events));
                connection.send(&packets);

                for stack in handled.thrown {
                    self.throw_out(stack);
                }
            }
            session::COMMAND_REQUEST => {
                let Some(command) = In::new(payload).string() else {
                    return;
                };
                let command = clean_text(command.trim().trim_start_matches('/'));
                if crate::auth::sensitive_command(&command) {
                    connection.send(&[raw_text("Парольная авторизация доступна только игрокам Java")]);
                    return;
                }
                log_info!("{} ввёл команду: /{}", self.identity.name, command);

                // Вход Xbox Live не подтверждён (auth.rs): кто угодно мог
                // назваться владельцем сервера. Такому игроку — только то,
                // что можно всем, как бы ни было записано в ops.json.
                // Подтверждённому — права по ops.json, как игроку Java.
                if !self.identity.verified && !open_to_everyone(&command) {
                    connection.send(&[raw_text("На это у тебя нет прав")]);
                    return;
                }

                if let Some(view) = crate::plugins::active().then(|| crate::plugins::PlayerView::find(&self.shared, &self.identity.name)).flatten()
                    && crate::plugins::player_command(&view, &command) == crate::plugins::Verdict::Deny
                {
                    connection.send(&[raw_text("Команду отменил плагин")]);
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
            68 => {
                // MapInfoRequest: только карта, которую игрок уже вправе видеть.
                let mut r = In::new(payload);
                if let Some(raw) = r.varint64() {
                    let id = ((raw >> 1) as i64) ^ -((raw & 1) as i64);
                    if let Ok(id) = i32::try_from(id) { self.seen_maps.remove(&id); }
                }
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
                None if let Some((target, action)) = read_entity_use(payload) => self.use_entity(connection, target, action),
                None if is_release(payload) => self.release_item(connection),
                None => crate::log_debug!("Bedrock: не разобрал действие предметом: {:02x?}", &payload[..payload.len().min(64)]),
            },
            session::TEXT => {
                if let Some(message) = read_chat(payload).map(|message| clean_text(&message)).filter(|m| !m.is_empty()) {
                    // В консоль строку выводит общий чат — здесь не пишем.
                    // Плагины видят чат Bedrock так же, как чат Java (plugins/).
                    let message = match crate::plugins::active().then(|| crate::plugins::PlayerView::find(&self.shared, &self.identity.name)).flatten() {
                        Some(view) => match crate::plugins::chat(&view, &message) {
                            crate::plugins::ChatOutcome::Cancel => return,
                            crate::plugins::ChatOutcome::Replace(new) => new,
                            crate::plugins::ChatOutcome::Pass => message,
                        },
                        None => message,
                    };
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

                if reader.varint64().is_some() {
                    let action = reader.zigzag32();

                    // Журнал для живой проверки экранов: что клиент шлёт
                    // сразу после щелчка (поля PlayerActionPacket). Ломание
                    // блока (ContinueDestroy = 18) слишком частое.
                    if action != Some(18) && (self.window.is_some() || self.station_window.is_some() || self.ticks - self.opened_at < 100) {
                        let position = (reader.zigzag32(), reader.zigzag32(), reader.zigzag32());

                        log_debug!(
                            "Bedrock: {} — PlayerAction {:?} по {:?}, через {} тактов после открытия окна",
                            self.identity.name, action, position, self.ticks - self.opened_at
                        );
                    }

                    match action {
                        Some(CHANGE_DIMENSION_ACK) => self.finish_change(connection, "подтвердил переход"),
                        Some(health::ACTION_RESPAWN) => self.respawn(connection),
                        _ => {}
                    }
                }
            }
            // Respawn от клиента: «готов появиться» — после экрана смерти.
            health::RESPAWN => {
                let mut reader = In::new(payload);

                if reader.lf32().is_some() && reader.lf32().is_some() && reader.lf32().is_some() && reader.u8() == Some(health::STATE_CLIENT_READY) {
                    self.respawn(connection);
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
            // больше не откроется. Было это окно хранилища — хранилище
            // теряет смотрящего.
            session::CONTAINER_CLOSE => {
                let mut reader = In::new(payload);

                if let (Some(window), Some(kind)) = (reader.u8(), reader.u8()) {
                    let mut packets = Vec::new();
                    self.lectern=None;
                    let by_server = reader.bool();
                    let since = self.ticks - self.opened_at;
                    let ours = self.window.as_ref().map(|open| open.id).or(self.station_window);

                    log_debug!(
                        "Bedrock: {} закрыл окно {} (экран {} = {}, сервер {:?}), через {} тактов после открытия; открыты у нас: хранилище {:?}, станок {:?}",
                        self.identity.name, window as i8, kind as i8, screens::screen_name(kind as i8), by_server, since,
                        self.window.as_ref().map(|open| open.id), self.station_window
                    );

                    // Закрытие сразу после открытия и не нашим номером —
                    // клиент прибирает своё (инвентарь, сетку), а не наш
                    // экран: у Bedrock вид «none» и окно 0 — это и закрытие
                    // инвентаря. Без этой оговорки такое закрытие тут же
                    // схлопывало бы только что открытый станок. Закрытие
                    // нашим номером (клиент отверг экран) — настоящее.
                    if ours.is_some() && Some(window) != ours && since <= EARLY_CLOSE_TICKS {
                        log_debug!("Bedrock: {} — ранний Container Close с чужим номером, окно оставлено", self.identity.name);
                        return;
                    }

                    // Экран у клиента один: что бы он ни закрыл, открытое у
                    // нас окно закрыто. Живой клиент закрывает окно воронки
                    // номером 0 и видом -9 (none), а не номером окна, — без
                    // этого окно у нас висело открытым и ни одно хранилище
                    // и ни один станок больше не открывались.
                    if self.window.is_some() {
                        self.close_window();
                    }

                    // Закрыт станок или инвентарь: сетка и ячейки станка
                    // отдают вещи обратно.
                    let trading = self.bag.station.is_some_and(|station| station.kind == stations::Kind::Trading);

                    if self.station_window.is_some() || window == 0 {
                        packets.extend(self.close_station());
                    }

                    // Окно торговли BDS в ответ ничего не закрывает — только
                    // сбрасывает метку у жителя (tmp/br-trade).
                    if !trading {
                        packets.push(screens::close_packet(window, kind, false)); // закрыл не сервер
                    }
                    connection.send(&packets);
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
            // BDS (br-record, gamemodes): смена режима командой — один Update Player
            // Game Type (0x97: режим, номер игрока, такт), без Set Player Game Type и
            // без Update Abilities; права по режиму клиент выводит сам.
            let mut set = Out::packet(UPDATE_PLAYER_GAME_TYPE);
            set.zigzag32(bedrock_game_mode(mode)).zigzag64(self.runtime_id as i64).varint64(0);
            packets.push(set.bytes);
        }

        for order in orders {
            match order {
                crate::players::Order::Hurt { amount, source } => self.health.hurt(amount, source),
                // [Bedrock: потом] Атрибуция команды; само здоровье обновляется.
                crate::players::Order::CommandDamage { amount, source, .. } => self.health.hurt(amount, source),
                crate::players::Order::MagicHit { amount, from, .. } => self.health.hurt(amount, if from.is_some() { crate::survival::Source::IndirectMagic } else { crate::survival::Source::Magic }),
                crate::players::Order::Respawn => self.respawn(connection),
                // Сон у Bedrock пока не ведётся [проверить].
                crate::players::Order::WakeUp => {}
                // Личное сообщение (плагины: player::send_message; командные блоки). Простой
                // текст-компонент — строкой; со стилями Bedrock пока не получит [проверить].
                crate::players::Order::Message { component } => {
                    if let Some(text) = plain_component(&component) {
                        packets.push(raw_text(&text));
                    }
                }
                crate::players::Order::Experience { set, amount, levels } => self.health.vit.command_xp(set, amount, levels),
                crate::players::Order::XpTaken { points } => {
                    let (slots, remaining) = crate::combat::mend_all(&mut self.bag.inventory, points,
                        || self.shared.world_in(self.dimension).lock().expect("мир").random_unit());
                    self.health.vit.add_xp(remaining);
                    self.bag.touched(&slots);
                    for slot in slots { packets.extend(self.bag.slot_packet(slot)); }
                }
                // Статистика и достижения Bedrock пока не ведутся [Bedrock: потом].
                crate::players::Order::KilledPlayer | crate::players::Order::Progress(_) | crate::players::Order::Advancement { .. } => {}
                // Пакеты и правки команд мира — пока только у Java [проверить].
                crate::players::Order::Packet(_) | crate::players::Order::Spectate { .. } => {}
                crate::players::Order::Enchant { enchantment, level } => {
                    let slot = self.bag.inventory.selected_slot();
                    if let Some(stack) = self.bag.inventory.slot(slot) {
                        let mut data = stack.extras(); data.set_enchant(enchantment as u16, level);
                        self.bag.inventory.set(slot, Some(stack.with_extras(data)));
                        self.bag.touched(&[slot]); packets.extend(self.bag.slot_packet(slot));
                    }
                }
                crate::players::Order::Attribute(edit) => {
                    let done = self.run(crate::network::play::Act::Attribute(&edit)); self.settle(done, packets);
                }
                // Состояние эффектов уходит отдельной сверкой MobEffectPacket.
                crate::players::Order::Effect(effect) => self.health.vit.give_effect_shown(effect),
                crate::players::Order::Recipes { give, names } => {
                    self.progress.recipes.grant_initial();
                    if give {
                        if names.is_empty() { self.progress.recipes.give_all(); } else { self.progress.recipes.learn_named(&names); }
                    } else { self.progress.recipes.take(&names); }
                }
                crate::players::Order::ClearEffects => { self.health.vit.remove_effects(None); }
                crate::players::Order::ClearEffect(kind) => { self.health.vit.remove_effects(Some(kind)); }
                crate::players::Order::Kick { reason } => {
                    let mut disconnect = Out::packet(session::DISCONNECT);
                    disconnect.zigzag32(0).bool(false).string(&reason).string("");
                    packets.push(disconnect.bytes);
                    connection.send(&std::mem::take(packets));
                    connection.close();
                    return;
                }
                // Готовые пакеты Java (title, звуки, частицы) Bedrock не нужны.
                crate::players::Order::Spawn { x, y, z, .. } => {
                    let done = self.run(crate::network::play::Act::Spawn((x, y, z))); self.settle(done, packets);
                }
                // Чат — простым текстом; прочие пакеты Java (title, звуки, частицы) Bedrock не нужны.
                crate::players::Order::PacketText { plain, .. } => {
                    if !plain.is_empty() {
                        packets.push(raw_text(&plain));
                    }
                }
                crate::players::Order::KickText { plain, .. } => {
                    let mut disconnect = Out::packet(session::DISCONNECT);
                    disconnect.zigzag32(0).bool(false).string(&plain).string("");
                    packets.push(disconnect.bytes);
                    connection.send(&std::mem::take(packets));
                    connection.close();
                    return;
                }
                crate::players::Order::PearlTeleport { x, y, z, dimension, .. } => {
                    if dimension == self.dimension && !self.health.vit.dead {
                        self.body.dismount(&self.shared, self.dimension);
                        self.health.vit.fall = 0.0;
                        self.health.vit.landed_from = 0.0;
                        self.teleport_to(connection, packets, (x, y, z));
                        self.shared.world_in(dimension).lock().expect("мир").note_sound_player_at((x, y, z), "minecraft:entity.player.teleport", 1.0, 1.0);
                        if !self.creative() { self.health.hurt(5.0, crate::survival::Source::EnderPearl); }
                    }
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
                crate::players::Order::Give { stack } | crate::players::Order::Pick { stack } => {
                    let (slots, left) = self.bag.inventory.put(stack);

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
                crate::players::Order::SetSlot { slot, stack } => {
                    self.bag.inventory.set(slot, stack); self.bag.touched(&[slot]); packets.extend(self.bag.slot_packet(slot));
                }
                crate::players::Order::SetEnderSlot { .. } => {}
                crate::players::Order::Hold { stack } => {
                    let slot = crate::inventory::FIRST_HOTBAR + self.bag.inventory.selected();

                    self.bag.inventory.set(slot, stack);
                    self.bag.touched(&[slot]);
                    packets.extend(self.bag.slot_packet(slot));
                }
                crate::players::Order::Clear => {
                    self.bag.clear();
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
        self.body.reset_motion();
        self.health.vit.fall = 0.0;
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
        // Окно хранилища закрывается: хранилище осталось в старом мире.
        packets.extend(self.close_window_by_server());
        packets.extend(self.items_shown.drain().map(remove_entity));
        self.primed_at.clear();
        self.mobs_at.clear();
        self.mobs_look.clear();
        self.things_at.clear();
        self.carts_at.clear();
        self.carts_data.clear();
        self.entity_links = super::entity_links::Links::default();
        for (id, shown) in std::mem::take(&mut self.shown) {
            packets.extend(forget_player(id, &shown));
        }
        connection.send(&std::mem::take(packets));

        self.body.dismount(&self.shared, self.dimension);
        let (old, reader) = (self.dimension, self.reader);
        self.shared.world_in(old).lock().expect("мир захвачен другим потоком").forget_reader(reader);
        self.shared.items_in(old).lock().expect("предметы захвачены другим потоком").forget_reader(reader);

        self.dimension = dimension;
        self.position = target;
        self.teleport = None;
        self.sent.clear();
        self.actors_shown.clear();
        self.seen_maps.clear();
        self.brush_since = None;
        self.loading.clear();
        self.lids.clear();
        self.piston_moves.clear();
        self.piston_later.clear();
        self.frames_at.clear();
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
        let end_portal = crate::portal::standing_in_end_portal(
            &self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком"),
            feet,
        );
        // В портале Края ждать не надо.
        let needed = if end_portal { 1 } else { crate::portal::wait(&self.shared.settings, self.creative()) };

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

            let carts: Vec<CartShown> = items.carts().iter()
                .map(|cart| (cart.entity_id, cart_place(cart), super::vehicle_data::entries(cart))).collect();

            // Рамки, о которых журнал что-то говорит: клиенту уходит их
            // нынешнее состояние целиком, а не по кусочкам.
            let touched: Vec<i32> = changes
                .iter()
                .filter_map(|change| match change {
                    crate::items::Change::Frame(frame) => Some(frame.entity_id),
                    crate::items::Change::FrameItem { entity_id, .. } | crate::items::Change::FrameRotation { entity_id, .. } => Some(*entity_id),
                    _ => None,
                })
                .collect();
            let frames: Vec<crate::frame::Frame> = if touched.is_empty() {
                Vec::new()
            } else {
                items.frames().iter().filter(|frame| touched.contains(&frame.entity_id)).cloned().collect()
            };

            let range = f64::from(self.shared.properties.view_distance.clamp(2, 10)) * 16.0;
            let mobs: Vec<(i32, super::mobdata::Look, [f32; 6])> = items
                .mobs()
                .iter()
                .filter(|mob| (mob.x - f64::from(self.position.0)).abs() <= range && (mob.z - f64::from(self.position.2)).abs() <= range)
                .map(|mob| (mob.entity_id, super::mobdata::Look::of(mob), [mob.x as f32, mob.y as f32, mob.z as f32, mob.yaw, mob.pitch, mob.head_yaw]))
                .collect();

            // Замок стрел — после предметов.
            let arrows: Vec<super::things::Thing> = self.shared.arrows.lock().expect("стрелы захвачены другим потоком")
                .list.iter().filter(|a| a.dimension == self.dimension && (a.x - f64::from(self.position.0)).abs() <= range && (a.z - f64::from(self.position.2)).abs() <= range)
                .map(super::projectiles::arrow).collect();

            // Подожжённый динамит: номер, место, скорость, запал.
            let primed: Vec<PrimedShown> = items
                .items()
                .iter()
                .filter_map(|tnt| Some((tnt.entity_id, (tnt.x as f32, tnt.y as f32, tnt.z as f32), (tnt.vx as f32, tnt.vy as f32, tnt.vz as f32), tnt.fuse?)))
                .collect();

            // Падающие блоки и шарики опыта (things.rs).
            let mut things: Vec<super::things::Thing> = items
                .items()
                .iter()
                .filter(|item| item.fuse.is_none())
                .filter_map(|item| Some(super::things::falling_block(item.entity_id, bedrock_state(item.block?), (item.x, item.y, item.z), (item.vx, item.vy, item.vz))))
                .collect();

            things.extend(
                items
                    .orbs()
                    .iter()
                    .map(|orb| super::things::orb(orb.entity_id, orb.value, (orb.x, orb.y, orb.z), (orb.vx, orb.vy, orb.vz))),
            );

            (changes, carts, frames, (mobs, arrows, primed, things))
        };
        let (changes, carts, frames, (mobs, arrows, primed, mut things)) = changes;
        let mut firework_blasts: Vec<(i32, crate::stackdata::Fireworks)> = Vec::new();

        // Снаряды: захват после предметов (как у стрел).
        {
            let thrown = self.shared.thrown.lock().expect("снаряды захвачены другим потоком");
            let range = f64::from(self.shared.properties.view_distance.clamp(2, 10)) * 16.0;
            let near = |x: f64, z: f64| (x - f64::from(self.position.0)).abs() <= range && (z - f64::from(self.position.2)).abs() <= range;

            for flying in thrown.flying.iter().filter(|flying| flying.dimension == self.dimension && near(flying.x, flying.z)) {
                things.push(super::projectiles::thrown(flying));
            }

            for eye in thrown.eyes.iter().filter(|eye| eye.dimension == self.dimension && near(eye.x, eye.z)) {
                things.push(super::things::eye(eye.entity_id, (eye.x, eye.y, eye.z), (eye.vx, eye.vy, eye.vz)));
            }

            for rocket in thrown.rockets.iter().filter(|rocket| rocket.dimension == self.dimension && near(rocket.x, rocket.z)) {
                things.push(super::things::rocket(rocket.entity_id, (rocket.x, rocket.y, rocket.z), (rocket.vx, rocket.vy, rocket.vz), &rocket.fireworks));
            }

            for blast in thrown.blasts.iter().filter(|blast| blast.dimension == self.dimension) {
                firework_blasts.push((blast.rocket, blast.last.fireworks.clone()));
            }
        }
        let mut vanished = Vec::new();

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
                crate::items::Change::Lightning(bolt) => {
                    packets.push(add_lightning(&bolt));
                    self.items_shown.insert(bolt.entity_id);
                }
                crate::items::Change::Cart(cart) => {
                    packets.push(add_cart(&cart));
                    self.items_shown.insert(cart.entity_id);
                    self.carts_at.insert(cart.entity_id, cart_place(&cart));
                }
                // [проверить]: качание ударенной вагонетки у Bedrock не
                // показываем.
                // [Bedrock: потом] Метаданные стопки после слияния.
                crate::items::Change::Stack { .. } => {}
                crate::items::Change::Dropped(_) | crate::items::Change::Fuse { .. } => {}
                // Рамки у Bedrock — блоки с данными (bedrock/frames.rs):
                // клиент получает блок и блок-актор, когда рамка появилась
                // или сменилась (предмет, поворот). Звук рамки клиент играет
                // сам по блоку. [проверить]
                crate::items::Change::Frame(_) | crate::items::Change::FrameItem { .. } | crate::items::Change::FrameRotation { .. } => {}
                crate::items::Change::FrameSound { .. } => {}
                // Удар и смерть моба: ActorEvent HURT (2) и DEATH (3).
                // Удар и смерть показываем (ActorEvent); голоса и частицы — нет.
                crate::items::Change::MobEvent { entity_id, at, event } => match event {
                    // Голоса — только те, что есть в таблице звуков (торг жителя:
                    // HaggleYes и HaggleNo, tmp/br-trade).
                    crate::mob::Event::Sound { name, .. } if self.within_earshot(at.0, at.2) => {
                        if let Some(cue) = super::effects::cue_for(name, 1.0) {
                            packets.push(super::effects::level_sound(cue.id, (at.0 as f32, at.1 as f32, at.2 as f32)));
                        }
                    }
                    crate::mob::Event::Hurt { source, .. } | crate::mob::Event::CommandHurt { source, .. } if self.items_shown.contains(&entity_id) => packets.push(health::hurt_event(entity_id as u64, source)),
                    crate::mob::Event::Death if self.items_shown.contains(&entity_id) => packets.push(health::actor_event(entity_id as u64, 3)),
                    // Сердечки любви: Actor Event love_particles (21, список ActorEvent minecraft-data) [проверить].
                    crate::mob::Event::Hearts if self.items_shown.contains(&entity_id) => packets.push(health::actor_event(entity_id as u64, 21)),
                    event if self.items_shown.contains(&entity_id) => {
                        if let Some(kind) = self.mobs_look.get(&entity_id).map(|l| l.kind)
                            && let Some(event) = super::entity_events::event(kind, &event)
                        { packets.push(health::actor_event(entity_id as u64, event)); }
                    }
                    _ => {}
                },
                // Опыт показывается снимком things, журнал не дублирует AddActor.
                crate::items::Change::Orb(_) => {}
                crate::items::Change::Gone { entity_id } => {
                    self.carts_at.remove(&entity_id);
                    self.carts_data.remove(&entity_id);
                    self.primed_at.remove(&entity_id);
                    self.mobs_at.remove(&entity_id);
                    self.mobs_look.remove(&entity_id);
                    self.things_at.remove(&entity_id);

                    if let Some(place) = self.frames_at.remove(&entity_id) {
                        vanished.push(place);
                    }

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

        self.move_carts(carts, packets);
        self.show_primed(primed, packets);
        self.show_mobs(mobs, packets);
        things.extend(arrows);
        {
            let fishing = self.shared.fishing.lock().expect("поплавки захвачены другим потоком");
            let range = f64::from(self.shared.properties.view_distance.clamp(2, 10)) * 16.0;
            things.extend(fishing.iter().filter(|b| b.dimension == self.dimension && (b.at.0 - f64::from(self.position.0)).abs() <= range && (b.at.2 - f64::from(self.position.2)).abs() <= range).map(super::projectiles::bobber));
        }

        // Взрыв ракеты: событие — до того, как ракета пропадёт из списка.
        // Как у BDS: событие 25, следом данные ракеты (по ним клиент рисует
        // цвета, форму, след и мерцание), и только потом ракета уходит.
        for (rocket, fireworks) in firework_blasts {
            if self.items_shown.contains(&rocket) {
                packets.push(health::actor_event(rocket as u64, super::things::EVENT_FIREWORK_PARTICLES));
                packets.push(super::things::rocket_data_packet(rocket, &fireworks));
            }
        }

        self.show_things(things, packets);
        let seats = self.shared.riding.lock().expect("учёт седоков захвачен другим потоком")
            .vehicles().filter(|(id, seats)| self.items_shown.contains(id) && seats.iter().all(|r| *r == self.runtime_id as i32 || self.items_shown.contains(r) || self.shown.contains_key(r)))
            .map(|(id, seats)| (id, seats.to_vec())).collect::<Vec<_>>();
        packets.extend(self.entity_links.update(&seats));
        if let Some(at) = super::entity_links::seat(&self.shared, self.dimension, self.runtime_id as i32) {
            self.position = (at.0 as f32, at.1 as f32 + EYES, at.2 as f32);
            self.health.vit.fall = 0.0;
            self.publish_position();
        }

        // Рамки: блок и блок-актор — в тех чанках, что клиент уже получил.
        for frame in frames {
            if self.sent.contains(&(frame.pos.0 >> 4, frame.pos.2 >> 4)) && heights(self.dimension).contains(&frame.pos.1) {
                if frame.painting.is_some() { self.items_shown.insert(frame.entity_id); } else {self.frames_at.insert(frame.entity_id, frame.pos);}
                packets.extend(super::frames::show(&frame));
            }
        }

        // Ушедшая рамка — на её месте снова то, что лежит в мире.
        if !vanished.is_empty() {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

            packets.extend(self.restore(&world, &vanished));
        }
    }

    /// Мобы: вошедшие в дальность появляются (Add Actor), сдвинувшиеся —
    /// переносом, ушедшие — убираются. Данные, снаряжение и атрибуты
    /// сравниваются с предыдущим снимком общего мира.
    fn show_mobs(&mut self, mobs: Vec<(i32, super::mobdata::Look, [f32; 6])>, packets: &mut Vec<Vec<u8>>) {
        let present: HashSet<i32> = mobs.iter().map(|(id, ..)| *id).collect();
        let gone: Vec<i32> = self.mobs_at.keys().filter(|id| !present.contains(id)).copied().collect();

        for entity_id in gone {
            self.mobs_at.remove(&entity_id);
            self.mobs_look.remove(&entity_id);

            if self.items_shown.remove(&entity_id) {
                packets.push(remove_entity(entity_id));
            }
        }

        for (entity_id, look, place) in mobs {
            match self.mobs_at.insert(entity_id, place) {
                None => {
                    packets.push(add_mob(entity_id, &look, place));
                    packets.extend(look.gear.spawn_packets(entity_id));
                    packets.push(super::entity_gear::attributes(entity_id, &look));
                    if let Some(s)=&look.stand {packets.extend(super::mobdata::stand_equipment(entity_id,s));}
                    self.items_shown.insert(entity_id);
                    self.mobs_look.insert(entity_id, look);
                }
                Some(before) => {
                    if before != place {
                        packets.push(move_mob(entity_id, place));
                    }

                    // Вырос, остригли, влюбился: клиенту — новые данные.
                    if self.mobs_look.get(&entity_id) != Some(&look) {
                        let old = self.mobs_look.get(&entity_id);
                        if old.is_some_and(|old| old.kind != look.kind) {
                            // Превращение сохраняет номер: клиенту нужен новый AddActor.
                            self.entity_links.forget(entity_id);
                            packets.push(remove_entity(entity_id));
                            packets.push(add_mob(entity_id, &look, place));
                            packets.extend(look.gear.spawn_packets(entity_id));
                            packets.push(super::entity_gear::attributes(entity_id, &look));
                            if let Some(s) = &look.stand { packets.extend(super::mobdata::stand_equipment(entity_id, s)); }
                        } else {
                            packets.push(super::mobdata::set_data_packet(entity_id, &super::mobdata::entries(&look)));
                            if old.is_none_or(|old| old.gear != look.gear) { packets.extend(look.gear.packets(entity_id)); }
                            if let Some(s) = &look.stand { packets.extend(super::mobdata::stand_equipment(entity_id, s)); }
                            if old.is_none_or(|old| (old.health, old.max_health, old.speed) != (look.health, look.max_health, look.speed)) {
                                packets.push(super::entity_gear::attributes(entity_id, &look));
                            }
                        }
                        self.mobs_look.insert(entity_id, look);
                    }
                }
            }
        }
    }

    /// Зелья, оки Края, ракеты, падающие блоки и шарики опыта: вошедшие в
    /// дальность появляются (Add Actor со скоростью), летящие — переносом,
    /// пропавшие из списка — убираются.
    fn show_things(&mut self, things: Vec<super::things::Thing>, packets: &mut Vec<Vec<u8>>) {
        let present: HashSet<i32> = things.iter().map(|thing| thing.id).collect();
        let gone: Vec<i32> = self.things_at.keys().filter(|id| !present.contains(id)).copied().collect();

        for entity_id in gone {
            self.things_at.remove(&entity_id);

            if self.items_shown.remove(&entity_id) {
                packets.push(remove_entity(entity_id));
            }
        }

        for thing in things {
            match self.things_at.insert(thing.id, thing.clone()) {
                None => {
                    packets.push(super::things::add_packet(&thing));
                    self.items_shown.insert(thing.id);
                }
                Some(before) => {
                    if before.place != thing.place { packets.push(move_mob(thing.id, thing.place)); }
                    if before.motion != thing.motion { packets.push(motion(thing.id as u64, (thing.motion[0], thing.motion[1], thing.motion[2]))); }
                    if before.data != thing.data {
                        // Флаги целиком: профиль BDS дописывается и здесь, иначе обновление стёрло бы их.
                        let mut data = thing.data.clone();
                        super::bds_profile::extend_thing(thing.name, &mut data);
                        packets.push(super::mobdata::set_data_packet(thing.id, &data));
                    }
                }
            }
        }
    }

    /// Вагонетки, которые сдвинулись, — переносом (Move Entity). Шлём, когда
    /// показанная вагонетка сдвинулась.
    fn move_carts(&mut self, carts: Vec<CartShown>, packets: &mut Vec<Vec<u8>>) {
        for (entity_id, place, data) in carts {
            if !self.items_shown.contains(&entity_id) {
                continue;
            }

            if self.carts_data.get(&entity_id) != Some(&data) {
                packets.push(super::mobdata::set_data_packet(entity_id, &data));
                self.carts_data.insert(entity_id, data);
            }
            if self.carts_at.insert(entity_id, place) != Some(place) {
                packets.push(move_cart(entity_id, place));
            }
        }
    }

    /// Подожжённый динамит: появился — Add Actor с запалом, сдвинулся —
    /// Move Actor. Пропадает записью журнала `Gone` (взрыв), как и прочее.
    fn show_primed(&mut self, primed: Vec<PrimedShown>, packets: &mut Vec<Vec<u8>>) {
        for (entity_id, at, velocity, fuse) in primed {
            match self.primed_at.insert(entity_id, at) {
                None => {
                    packets.push(super::effects::add_primed(entity_id, at, velocity, fuse));
                    self.items_shown.insert(entity_id);
                }
                Some(before) if before != at => packets.push(super::effects::move_primed(entity_id, at)),
                Some(_) => {}
            }
        }
    }

    /// Рядом ли место с игроком настолько, чтобы слышать и видеть событие.
    fn within_earshot(&self, x: f64, z: f64) -> bool {
        (x - f64::from(self.position.0)).abs() <= 128.0 && (z - f64::from(self.position.2)).abs() <= 128.0
    }

    /// Звук мира Java как пакеты Bedrock: то, что шлёт BDS (effects.rs).
    /// Удар игрока по существу бьющий слышит от своего клиента, так что
    /// звук удара из его же места ему не шлётся; звуки блоков идут всем,
    /// и тому, кто щёлкнул (как у BDS).
    fn sound_packets(&self, sound: &crate::world::WorldSound) -> Vec<Vec<u8>> {
        let at = sound.exact.map_or((sound.x as f32 + 0.5, sound.y as f32 + 0.5, sound.z as f32 + 0.5), |(x, y, z)| (x as f32, y as f32, z as f32));
        let Some(cue) = super::effects::cue_for(sound.name, sound.pitch) else {
            return Vec::new();
        };

        if cue.extra == super::effects::Extra::Player
            && sound.name.contains("attack")
            && (f64::from(at.0) - f64::from(self.position.0)).abs() < 0.05
            && (f64::from(at.2) - f64::from(self.position.2)).abs() < 0.05
        {
            return Vec::new();
        }

        let block = (cue.extra == super::effects::Extra::Block).then(|| {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

            bedrock_state(world.get_block(sound.x, sound.y, sound.z)) as i32
        });

        super::effects::sound_packets(sound.name, sound.pitch, at, block)
    }

    /// Выживание и приключение: урон и голод работают.
    fn mortal(&self) -> bool {
        self.game_mode == crate::commands::GAME_MODE_SURVIVAL || self.game_mode == crate::commands::GAME_MODE_ADVENTURE
    }

    /// Движение клиента для здоровья: падение и изнурение. Среда — по блокам
    /// у ног (вода, паутина, лестница гасят падение; сено, кровать смягчают).
    fn track_health(&mut self, dy: f64, run: f64, was_on_ground: bool) {
        if !self.mortal() || self.health.vit.dead || self.changing.is_some() {
            return;
        }

        let (x, y, z) = (self.position.0, self.position.1 - EYES, self.position.2);
        let (soft, multiplier) = {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
            let feet = world.get_block(x.floor() as i32, y.floor() as i32, z.floor() as i32);
            let below = world.get_block(x.floor() as i32, (y - 0.2).floor() as i32, z.floor() as i32);
            let soft = crate::fluids::fluid_at(feet).is_some() || crate::blocks::block_at_state(feet).is_some_and(crate::survival::soft_block);

            (soft, crate::blocks::block_at_state(below).map_or(1.0, crate::survival::landing_multiplier))
        };

        self.health.moved(dy, self.on_ground, run, was_on_ground && !self.on_ground && dy > 0.0, soft, multiplier);
    }

    /// Такт здоровья: удары, голод, атрибуты; смерть.
    fn tick_health(&mut self, packets: &mut Vec<Vec<u8>>) {
        if self.changing.is_some() {
            return;
        }

        let difficulty = self.shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
        let mortal = self.mortal();
        let immune = self.game_mode == crate::commands::GAME_MODE_CREATIVE || self.game_mode == crate::commands::GAME_MODE_SPECTATOR;
        let feet_y = self.position.1 - EYES;
        let done = self.run(crate::network::play::Act::Environment);
        self.settle(done, packets);
        let outcome = self.health.step(self.runtime_id, health::Rules { mortal, immune, difficulty, feet_y,
            natural_regeneration: self.shared.natural_regeneration.load(std::sync::atomic::Ordering::Relaxed) },
            &mut self.bag.inventory, self.body.attributes());
        if !outcome.slots.is_empty() {
            self.shared.world_in(self.dimension).lock().expect("мир").note_totem(self.runtime_id as i32);
        }
        self.bag.touched(&outcome.slots);
        packets.extend(outcome.slots.iter().filter_map(|&slot| self.bag.slot_packet(slot)));

        packets.extend(outcome.packets);

        // Остальным игрокам — Damage Event (Java) и «ранен» (Bedrock): у них
        // игрок пострадал так же, как у Bedrock-соседа.
        if !outcome.hurts.is_empty() {
            let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

            for source in &outcome.hurts {
                world.note_player_damaged(self.runtime_id as i32, source.type_id(), 0, 0);
            }
        }

        if let Some(source) = outcome.died { self.die(packets, source); }
        let done = self.run(crate::network::play::Act::Reactions);
        self.settle(done, packets);
        let stance = self.stance();
        let gear = self.body.gear(&self.shared, &stance, &self.bag.inventory, &self.health.vit, self.last_attack);
        self.shared.players.lock().expect("игроки").set_gear(&self.identity.uuid, gear);
        let feet = (self.position.0, self.position.1 - EYES, self.position.2);
        self.state_view.sync(self.runtime_id, &self.health.vit, self.body.frozen(), self.ticks, (feet.0, feet.1 + 0.9, feet.2), packets);
        self.state_view.attributes(self.runtime_id, self.body.attributes(), &self.health.vit.effects, packets);
    }

    /// Смерть: сообщение, вещи на землю, экран смерти у клиента.
    fn die(&mut self, packets: &mut Vec<Vec<u8>>, source: crate::survival::Source) {
        let name = self.identity.name.clone();

        packets.push(health::death_event(self.runtime_id));
        packets.push(health::death_info(source, &name));

        let text = crate::survival::death_text(source, &name);

        log_info!("{}", text);
        self.shared.chat.lock().expect("чат захвачен другим потоком").push_hidden(text, Some(self.identity.uuid));

        let keep = self.shared.keep_inventory.load(std::sync::atomic::Ordering::Relaxed);
        for stack in crate::combat::death_drops(&mut self.bag.inventory, keep) { self.throw_out(stack); }
        if !keep { packets.extend(self.bag.content_packets()); }

        self.health.vit.dead = true;
    }

    /// Клиент нажал «возродиться»: к точке возрождения, здоровье заново.
    fn respawn(&mut self, connection: &Connection) {
        if !self.health.vit.dead || self.changing.is_some() {
            return;
        }

        self.health.respawned();
        self.body.respawned();

        let spawn = self.shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком").spawn_position();
        let (dimension, (x, y, z), yaw, missing) = self.body.respawn_target(&self.shared, spawn);
        if let Some(yaw) = yaw { self.rotation.0 = yaw; }
        let mut packets = Vec::new();
        if missing { packets.push(raw_text("Your respawn point is missing or obstructed.")); }

        if self.dimension == dimension {
            self.teleport_to(connection, &mut packets, (x, y, z));
        } else {
            self.change_dimension(connection, &mut packets, dimension, (x, y, z));
        }

        let eyes = (x as f32, y as f32 + EYES, z as f32);
        let mut answer = packets;
        answer.push(health::respawn_packet(eyes, health::STATE_READY, self.runtime_id));
        answer.push(health::attributes(self.runtime_id, &self.health.vit));

        answer.push(abilities(self.runtime_id, self.game_mode));
        connection.send(&answer);
        self.save_if_changed();
        log_info!("{} (Bedrock) возродился", self.identity.name);
    }

    /// Поля, которые могут меняться без блока (disabled_slots, содержимое).
    /// Снимок Bedrock локален подключению: событий Java не добавляем.
    fn tick_actors(&mut self, packets: &mut Vec<Vec<u8>>) {
        if !self.ticks.is_multiple_of(5) { return; }
        let world = self.shared.world_in(self.dimension).lock().expect("мир");
        let mut seen = HashSet::new();
        let positions: Vec<_> = self.sent.iter().flat_map(|&(cx,cz)| world.chunk_containers(cx,cz).into_iter()
            .map(move |(x,y,z,_)| (cx*16+x,y,cz*16+z))).collect();
        for pos in positions {
            if !heights(self.dimension).contains(&pos.1) { continue; }
            let kind = world.container(pos.0,pos.1,pos.2).unwrap().kind;
            if !matches!(kind, containers::Kind::Crafter | containers::Kind::Campfire | containers::Kind::ChiseledBookshelf
                | containers::Kind::Shelf | containers::Kind::Lectern | containers::Kind::Jukebox | containers::Kind::Beacon) { continue; }
            seen.insert(pos);
            if let Some(packet) = super::block_actors::live(&world,pos) {
                if self.actors_shown.get(&pos) != Some(&packet) { packets.push(packet.clone()); }
                self.actors_shown.insert(pos,packet);
            }
        }
        self.actors_shown.retain(|pos,_| seen.contains(pos));
    }

    fn tick_brush(&mut self, packets: &mut Vec<Vec<u8>>) {
        let Some(since) = self.brush_since else { return; };
        let held = self.bag.inventory.held().filter(|s| crate::blocks::item_name(s.item) == Some("brush"));
        if held.is_none() || self.health.vit.dead || self.changing.is_some()
            || !matches!(self.game_mode, crate::commands::GAME_MODE_SURVIVAL | crate::commands::GAME_MODE_CREATIVE) {
            self.brush_since = None; return;
        }
        let now = *self.shared.ticks.borrow();
        let mut world = self.shared.world_in(self.dimension).lock().expect("мир");
        let Some((pos,face)) = super::brushing::target(&world,self.position,self.rotation) else { self.brush_since = None; return; };
        if now.saturating_sub(since) % 10 != 5 { return; }
        let sound = if crate::blocks::block_at_state(world.get_block(pos.0,pos.1,pos.2)) == Some("suspicious_sand") {
            "minecraft:item.brush.brushing.sand"
        } else { "minecraft:item.brush.brushing.gravel" };
        world.note_sound(pos.0,pos.1,pos.2,sound,1.0,1.0);
        if let Some(loot) = crate::archaeology::stroke(&mut world,pos,now,face) {
            if !self.creative() {
                let stack = held.unwrap(); let worn = crate::combat::wear(&stack,1,false,||world.random_unit());
                let slot = self.bag.inventory.selected_slot(); self.bag.inventory.set(slot,crate::combat::add_damage(stack,worn));
                self.bag.touched(&[slot]); packets.extend(self.bag.slot_packet(slot));
            }
            self.brush_since = None; drop(world);
            let mut items = self.shared.items_in(self.dimension).lock().expect("предметы");
            for stack in loot { items.drop_item(self.shared.next_entity_id(),stack,(pos.0 as f64+0.5,pos.1 as f64+1.0,pos.2 as f64+0.5),(0.0,0.0,0.0),crate::items::MINED_DELAY); }
        }
    }

    /// Карты в руках используют те же пиксели и номера, что Java.
    fn tick_maps(&mut self, packets: &mut Vec<Vec<u8>>) {
        if !self.ticks.is_multiple_of(5) { return; }
        let carried: HashSet<_> = self.bag.inventory.held().into_iter()
            .chain(self.bag.inventory.slot(crate::inventory::OFFHAND)).filter_map(|s| s.extras().map_id).collect();
        let frames: Vec<_> = self.shared.items_in(self.dimension).lock().expect("предметы")
            .frames().iter().filter(|f| self.sent.contains(&(f.pos.0 >> 4, f.pos.2 >> 4)))
            .filter_map(|f| f.item.and_then(|s| s.extras().map_id).map(|id| (id, f.pos, f.facing))).collect();
        let mut ids = carried.clone(); ids.extend(frames.iter().map(|f| f.0));
        self.seen_maps.retain(|id, _| ids.contains(id));
        let world = self.shared.world_in(self.dimension).lock().expect("мир");
        let mut maps = self.shared.maps.lock().expect("карты");
        for id in ids {
            if carried.contains(&id) && maps.maps.get(&id).is_some_and(|m| m.dimension == self.dimension.index() as u8) {
                maps.update(id, &world, (self.position.0 as f64, self.position.2 as f64), usize::MAX, false);
            }
            let Some(map) = maps.maps.get(&id) else { continue; };
            let mut icons = Vec::new();
            if let Some((x,z,kind)) = map.destination {
                let (x,z,d) = crate::maps::marker(map, x as f64, z as f64, 180.0); icons.push((kind,x,z,d));
            }
            if carried.contains(&id) && map.dimension == self.dimension.index() as u8
                && let Some(icon) = crate::maps::player_marker(map, self.position.0 as f64, self.position.2 as f64, self.rotation.0) { icons.push(icon); }
            for (_,p,facing) in frames.iter().filter(|f| f.0 == id) {
                let yaw = match facing { 2 => 180.0, 3 => 0.0, 4 => 90.0, _ => 270.0 };
                let (x,z,d) = crate::maps::marker(map, p.0 as f64, p.2 as f64, yaw); icons.push((1,x,z,d));
            }
            let patch = crate::maps::patch(self.seen_maps.get(&id).map(Vec::as_slice), &map.colors);
            packets.push(super::maps::packet(map, patch.as_ref(), &icons, !self.seen_maps.contains_key(&id)));
            self.seen_maps.insert(id, map.colors.clone());
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

        // Рамка в этой клетке — свои действия (у Java это сущность, тут блок).
        if self.use_frame(connection, &action) {
            return;
        }

        // Правая кнопка в воздух: еда, лук, щит, бросок (повтор, пока кнопка
        // зажата, начинает заново — пропускаем).
        if action.kind == CLICK_AIR {
            if action.trigger != SIMULATION_TICK {
                self.use_held(connection);
            }

            return;
        }

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

        if kind == CLICK && action.trigger != SIMULATION_TICK
            && !self.health.vit.dead && self.changing.is_none()
            && matches!(self.game_mode, crate::commands::GAME_MODE_SURVIVAL | crate::commands::GAME_MODE_CREATIVE)
            && self.within_reach((x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5))
        {
            let who = super::tools::Who { game_mode: self.game_mode, yaw: self.rotation.0 };
            let used = {
                let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                super::tools::surface(&mut self.bag, &who, &mut world, action.position, action.face)
            };
            if let Some(used) = used {
                self.bag.touched(&used.slots);
                connection.send(&used.slots.into_iter().filter_map(|slot| self.bag.slot_packet(slot)).collect::<Vec<_>>());
                for stack in used.thrown { self.throw_out(stack); }
                return;
            }
        }

        if kind == CLICK && !self.health.vit.dead && self.changing.is_none()
            && matches!(self.game_mode, crate::commands::GAME_MODE_SURVIVAL | crate::commands::GAME_MODE_CREATIVE)
            && self.bag.inventory.held().is_some_and(|s| crate::blocks::item_name(s.item) == Some("brush"))
        {
            let world = self.shared.world_in(self.dimension).lock().expect("мир");
            if super::brushing::target(&world,self.position,self.rotation).is_some() {
                self.brush_since.get_or_insert(*self.shared.ticks.borrow());
                return;
            }
        }

        // Щелчок по хранилищу, рычагу, кнопке, двери, музыкальному блоку —
        // это действие с ними, а не установка, как у Java (place_with_main_hand
        // в network/play.rs): рука тут ни при чём, а присев, игрок всё-таки
        // ставит блок.
        if kind == CLICK && !self.sneaking {
            // Повтор щелчка, пока кнопка зажата (trigger «simulation tick»),
            // по тому же блоку ничего не делает: ни рычаг ещё раз не дёргает,
            // ни блок к сундуку не ставит.
            if action.trigger == SIMULATION_TICK {
                if self.last_use == Some(action.position) {
                    return;
                }
            } else {
                self.last_use = None;
            }

            if let Some(packets) = self.use_block(action.position, action.face, action.click) {
                self.last_use = Some(action.position);

                if !packets.is_empty() {
                    connection.send(&packets);
                }

                return;
            }
        }

        // Рамка в руке вешается на грань блока, по которой щёлкнули.
        if kind == CLICK && action.trigger != SIMULATION_TICK && self.held_stack(action.held).is_some_and(|s|crate::blocks::item_name(s.item)==Some("armor_stand")) {
            if self.game_mode==crate::commands::GAME_MODE_SPECTATOR || self.game_mode==crate::commands::GAME_MODE_ADVENTURE || !(1..=5).contains(&action.face)
                || !self.within_reach((x as f64+0.5,y as f64+0.5,z as f64+0.5)) {return;}
            let placed={let mut world=self.shared.world_in(self.dimension).lock().expect("мир");let mut items=self.shared.items_in(self.dimension).lock().expect("сущности");
                crate::mob::armor_stand::place(&mut world,&mut items,&self.shared,action.position,action.face,self.rotation.0)};
            if placed && !self.creative() && let Some(slot)=self.bag.inventory.spend_held() {self.bag.touched(&[slot]);connection.send(&self.bag.slot_packet(slot).into_iter().collect::<Vec<_>>());}
            return;
        }
        if kind == CLICK && action.trigger != SIMULATION_TICK && self.place_frame(connection, &action) {
            return;
        }

        // Вагонетка в руке ставится только на рельсу, мимо — ничего.
        if kind == CLICK && action.trigger != SIMULATION_TICK && self.place_cart(connection, action.held, action.position).is_some() {
            return;
        }

        // Ракета фейерверка в руке летит из точки щелчка (Java: тот же пуск).
        if kind == CLICK && action.trigger != SIMULATION_TICK && self.launch_rocket(connection, &action) {
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
                // Те же отменяемые события, что у Java. Плагин вызывается без замка мира.
                if crate::plugins::active() {
                    let block = format!("minecraft:{}", crate::blocks::block_at_state(world.get_block(x, y, z)).unwrap_or("air"));
                    drop(world);
                    let denied = crate::plugins::PlayerView::find(&self.shared, &self.identity.name)
                        .is_some_and(|view| crate::plugins::block_break(&view, (x, y, z), &block) == crate::plugins::Verdict::Deny);
                    world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                    if denied || self.refusal(&world, kind, action.position, target).is_some() {
                        connection.send(&self.restore(&world, &[action.position, target]));
                        return;
                    }
                }
                // Особый динамит помнит свой вид до слома: выпадет он же.
                let pot = world.pot((x,y,z));
                let custom = world.custom_tnt_at((x, y, z));
                let tool = self.bag.inventory.held();
                let original = world.get_block(x, y, z);
                let creative = self.creative();
                let tile_drops = self.shared.block_drops.load(std::sync::atomic::Ordering::Relaxed);
                let silk = tool.is_some_and(|stack| stack.extras().level("silk_touch") > 0);
                if crate::blocks::block_at_state(original) == Some("turtle_egg") {
                    // Как у Java: ломается одно яйцо кладки; Silk сохраняет
                    // ровно его, в creative исчезает вся кладка без добычи.
                    if creative { world.set_block(x, y, z, crate::world::AIR); }
                    else { crate::mob::water::eggs::break_one(&mut world, (x, y, z)); }
                    crate::redstone::settle(&mut world);
                    drop(world);
                    if !creative && silk && tile_drops && let Some(item) = crate::blocks::item_named("turtle_egg") {
                        self.shared.items_in(self.dimension).lock().expect("предметы").drop_item(
                            self.shared.next_entity_id(), crate::inventory::Stack::new(item, 1),
                            (x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5), (0.0, 0.2, 0.0), crate::items::MINED_DELAY);
                    }
                    return;
                }
                if crate::mob::arthropod::releases(original, !creative, silk, tile_drops) {
                    let mob = crate::mob::arthropod::released(self.shared.next_entity_id(), (x, y, z));
                    self.shared.items_in(self.dimension).lock().expect("предметы").add_mob_late(mob.clone());
                    world.note_mob_spawn(mob);
                }
                let hive = crate::hive::is_hive(original);
                let hive_drop = if hive && silk { crate::hive::silk_stack(&world, (x, y, z)) } else { None };
                if hive && !silk {
                    let mut items = self.shared.items_in(self.dimension).lock().expect("предметы");
                    crate::hive::disturb(&mut world, &mut items, (x, y, z), self.runtime_id as i32, &mut || self.shared.next_entity_id());
                }
                if tool.is_some_and(|stack| crate::blocks::item_name(stack.item) == Some("shears")) {
                    crate::redstone::disarm_tripwire(&mut world, (x, y, z));
                }
                let Some(broken) = crate::placing::break_block(&mut world, x, y, z) else {
                    // Ломать было нечего — но клиент мог показать поломку.
                    let packets = self.restore(&world, &[action.position]);
                    drop(world);

                    if !packets.is_empty() {
                        connection.send(&packets);
                    }

                    return;
                };

                // Добыча учитывает чары даже на последнем пункте прочности.
                // Для выпадений ниже сохранена исходная стопка `tool`.
                crate::frost::broken(&mut world, (x, y, z), broken, self.creative(), tool.is_some_and(|stack| stack.extras().level("silk_touch") > 0));
                if !self.creative() && let Some(stack) = tool {
                    let wear = crate::combat::wear(&stack, crate::mining::wear(broken, stack), false, || world.random_unit());
                    if wear > 0 {
                        let slot = self.bag.inventory.selected_slot();
                        self.bag.inventory.set(slot, crate::combat::add_damage(stack, wear));
                        self.bag.touched(&[slot]);
                        connection.send(&self.bag.slot_packet(slot).into_iter().collect::<Vec<_>>());
                    }
                }

                if let Some(sides) = pot {
                    if !self.creative() && self.shared.block_drops.load(std::sync::atomic::Ordering::Relaxed) {
                        for stack in crate::pottery::drops(sides,tool) {
                            self.shared.items_in(self.dimension).lock().expect("предметы").drop_item(self.shared.next_entity_id(),stack,(x as f64+0.5,y as f64+0.25,z as f64+0.5),(0.0,0.2,0.0),crate::items::MINED_DELAY);
                        }
                    }
                    return;
                }
                if !self.creative() && self.shared.block_drops.load(std::sync::atomic::Ordering::Relaxed) {
                    let xp = crate::mining::experience(broken, tool, world.random_unit());
                    let fortune = tool.map_or(0, |stack| stack.extras().level("fortune"));
                    let drops = if hive {
                        hive_drop.or_else(|| (crate::blocks::block_at_state(broken) == Some("beehive"))
                            .then(|| crate::blocks::item_named("beehive").map(|item| crate::inventory::Stack::new(item, 1))).flatten())
                            .into_iter().collect()
                    } else {
                        crate::farming::harvest(&mut world, broken, fortune)
                            .unwrap_or_else(|| crate::mining::block_drops(broken, tool, || world.random_unit()))
                    };
                    drop(world);
                    if xp > 0 {
                        let mut rng = crate::mob::Rng::new(u64::from(self.shared.next_entity_id() as u32) ^ (*self.shared.ticks.borrow() << 20));
                        self.shared.items_in(self.dimension).lock().expect("предметы")
                            .drop_experience(xp, (x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5), &mut || self.shared.next_entity_id(), &mut rng);
                    }
                    for stack in drops.into_iter().map(|stack| crate::custom_tnt::tag_drop(stack, custom)) {
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

                // Огненный заряд по динамиту — поджог, как у Java (play.rs):
                // заряд тратится, огня рядом он не ставит.
                let fire_charge = crate::blocks::item_named("fire_charge");

                if held_item.is_some() && held_item == fire_charge && !self.sneaking && crate::tnt::ignite(&mut world, action.position) {
                    crate::redstone::settle(&mut world);
                    drop(world);

                    if !self.creative()
                        && let Some(slot) = self.bag.inventory.spend_held()
                    {
                        self.bag.touched(&[slot]);
                        connection.send(&self.bag.slot_packet(slot).into_iter().collect::<Vec<_>>());
                    }

                    return;
                }

                if held_item.is_some() && held_item == crate::blocks::item_named("flint_and_steel") {
                    // По динамиту — поджог, присевший ставит огонь рядом
                    // (вики, TNT). Сущность динамита клиенту показывает
                    // tick_items (подожжённый динамит — Add Actor tnt).
                    if !self.sneaking && crate::tnt::ignite(&mut world, action.position) {
                        crate::redstone::settle(&mut world);
                        return;
                    }

                    if world.get_block(target.0, target.1, target.2) == crate::world::AIR {
                        if crate::portal::light(&mut world, target, self.shared.settings.portal_sizes()) {
                            log_info!("{} зажёг портал в {} {} {}", self.identity.name, target.0, target.1, target.2);
                        } else if let Some(fire) = crate::blocks::state_by_name("fire") {
                            world.set_block(target.0, target.1, target.2, fire);
                        }

                        crate::redstone::settle(&mut world);
                    }

                    return;
                }

                // Костная мука, мотыга, вёдра, яйца призыва: тот же код, что у
                // игрока Java (bedrock/tools.rs).
                if let Some(item) = held_item {
                    let who = super::tools::Who { game_mode: self.game_mode, yaw: self.rotation.0 };
                    let shared = &self.shared;

                    if let Some(used) = super::tools::use_on_block(&mut self.bag, &who, &mut world, item, action.position, action.face, || shared.next_entity_id()) {
                        let mut packets = if used.refused { self.restore(&world, &[action.position, target]) } else { Vec::new() };
                        drop(world);

                        {
                            let mut items = self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком");

                            for (stack, at, velocity) in used.drops {
                                items.drop_item(self.shared.next_entity_id(), stack, at, velocity, crate::items::MINED_DELAY);
                            }

                            if let Some(mob) = used.mob {
                                items.add_mob(mob);
                            }
                        }

                        for stack in used.thrown {
                            self.throw_out(stack);
                        }

                        let mut slots = used.slots;

                        slots.sort_unstable();
                        slots.dedup();
                        self.bag.touched(&slots);
                        packets.extend(slots.iter().filter_map(|&slot| self.bag.slot_packet(slot)));

                        if !packets.is_empty() {
                            connection.send(&packets);
                        }

                        return;
                    }
                }

                // Что в руке — знает сервер: инвентарь у него. Если там пусто,
                // а клиент уверен в обратном (творческий, инвентарь ещё не
                // пришёл), верим клиенту только в творческом режиме.
                let from_bag = self.bag.inventory.held().and_then(|stack| crate::blocks::block_for_item(stack.item));
                let claimed = if self.creative() { action.held.and_then(held_block) } else { None };

                let Some((name, default_state)) = from_bag.or(claimed) else {
                    // Не блок: еда, лук, щит, зелье — действие предметом.
                    drop(world);

                    if action.trigger != SIMULATION_TICK {
                        self.use_held(connection);
                    }

                    return;
                };
                let click = crate::placing::Click {
                    face: action.face,
                    cursor_y: action.click_y,
                    yaw: self.rotation.0,
                    pitch: self.rotation.1,
                };
                let feet = (self.position.0 as f64, (self.position.1 - EYES) as f64, self.position.2 as f64);

                if crate::plugins::active() {
                    drop(world);
                    let denied = crate::plugins::PlayerView::find(&self.shared, &self.identity.name)
                        .is_some_and(|view| crate::plugins::block_place(&view, target, &format!("minecraft:{name}")) == crate::plugins::Verdict::Deny);
                    world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                    if denied || self.refusal(&world, kind, action.position, target).is_some() {
                        connection.send(&self.restore(&world, &[action.position, target]));
                        return;
                    }
                }

                let custom = if from_bag.is_some() { self.bag.inventory.held().and_then(|stack| stack.tnt) } else { None };
                let pot_at = crate::pottery::placement_at(&world,action.position,action.face);
                let refused = match crate::placing::place_tagged(&mut world, name, default_state, action.position, &click, feet, custom) {
                    crate::placing::Placement::Refused(places) => places,
                    crate::placing::Placement::Nothing => vec![target],
                    crate::placing::Placement::Placed => {
                        if let Some(held) = self.bag.inventory.held() {
                            crate::pottery::place(&mut world,pot_at,held);
                        }
                        if crate::redstone::is_sign(world.get_block(target.0,target.1,target.2)) {
                            self.sign_editor=Some((target,true));
                            let mut packets=Vec::new();packets.extend(super::signs::actor(&world,target));packets.push(super::signs::editor(target,true));connection.send(&packets);
                        }
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

    /// Щелчок по блоку, у которого есть своё действие. Хранилище
    /// открывается через общий ход (containers.rs) — игроки Java увидят
    /// крышку, услышат звук, ловушка даст сигнал; рычаг, кнопка, дверь,
    /// музыкальный блок срабатывают, как от щелчка игрока Java. Порядок тот
    /// же, что в place_with_main_hand (network/play.rs).
    ///
    /// None — действия у блока нет (тогда щелчок — установка блока); иначе —
    /// что отправить клиенту.
    /// Действие по сущности: удар или щелчок. Удар идёт тем же кодом, что у
    /// игрока Java (`attack` в network/play.rs): игрок, моб, вагонетка — заряд
    /// руки, критический удар, рывок, зачарования, взмах меча, износ оружия.
    /// Щелчок (кормление, стрижка) идёт общим `use_entity`; окна вагонеток и
    /// торговли жителя открывают `open_cart` и `open_trade`.
    fn use_entity(&mut self, connection: &Connection, target: u64, action: u32) {
        /// Interact и Attack (InventoryTransaction, ItemUseOnEntity,
        /// описание протокола Mojang).
        const ATTACK: u32 = 1;

        let Ok(target) = i32::try_from(target) else {
            return;
        };

        if self.game_mode == crate::commands::GAME_MODE_SPECTATOR || self.health.vit.dead || self.changing.is_some() {
            return;
        }

        // Interact — правая кнопка: общий код Java кормит, стрижёт, поджигает
        // крипера, открывает окна вагонетки и торговли жителя.
        let act = if action == ATTACK { crate::network::play::Act::Attack(target) } else { crate::network::play::Act::Interact(target) };
        let interact = action != ATTACK;
        let mut done = self.run(act);
        let mut packets = Vec::new();

        // Окна сущностей: вагонетка-хранилище (ячейки — её, как у Java) и
        // торговля жителя (Update Trade). Открыть нельзя, пока открыто другое
        // окно, — клиент сам закрыл бы его.
        let (cart, merchant) = (done.open_cart.take(), done.open_merchant.take());

        log_debug!("Bedrock: {} — окна сущности: вагонетка {:?}, житель {:?}", self.identity.name, cart, merchant);

        if let Some(cart) = cart {
            packets.extend(self.open_cart(cart));
        }

        if let Some(villager) = merchant {
            let opened = self.open_trade(villager);

            if opened.is_empty() {
                // Не открылось (занято другое окно): житель свободен.
                crate::network::play::release_villager(&self.shared, self.dimension, villager);
            }

            packets.extend(opened);
        }

        self.settle(done, &mut packets);
        log_debug!("Bedrock: {} {} сущность {}", self.identity.name, if interact { "щёлкнул по" } else { "ударил" }, target);

        if !packets.is_empty() {
            connection.send(&packets);
        }
    }

    /// Место игрока для тела боя.
    fn stance(&self) -> crate::network::play::Stance {
        crate::network::play::Stance {
            game_mode: self.game_mode,
            feet: (self.position.0 as f64, (self.position.1 - EYES) as f64, self.position.2 as f64),
            yaw: self.rotation.0,
            pitch: self.rotation.1,
            on_ground: self.on_ground,
            sneaking: self.sneaking,
            dimension: self.dimension,
        }
    }

    /// Действие общим кодом Java: тело берёт инвентарь и здоровье, делает,
    /// возвращает изменённое.
    fn run(&mut self, act: crate::network::play::Act) -> crate::network::play::Done {
        let stance = self.stance();

        self.body.act(&self.shared, &stance, &mut self.bag.inventory, &mut self.health.vit, &mut self.last_attack, act)
    }

    /// Последствия действия для клиента: изменившиеся слоты, толчок, урон.
    fn settle(&mut self, done: crate::network::play::Done, packets: &mut Vec<Vec<u8>>) {
        if !done.slots.is_empty() {
            self.bag.touched(&done.slots);
            packets.extend(done.slots.iter().filter_map(|&slot| self.bag.slot_packet(slot)));
        }

        if let Some((vx, vy, vz)) = done.velocity {
            packets.push(motion(self.runtime_id, (vx as f32, vy as f32, vz as f32)));
        }

        for (amount, source, effects) in done.damage { self.health.hurt_with_effects(amount, source, effects); }
        for (amount, source) in done.raw_damage { self.health.hurt(amount, source); }
        if let Some((category, ticks)) = done.cooldown { packets.push(super::player_state::cooldown(category, ticks)); }
        if done.shield_blocked {
            let at = (f64::from(self.position.0), f64::from(self.position.1 - EYES), f64::from(self.position.2));
            self.shared.world_in(self.dimension).lock().expect("мир").note_sound_player_except(at, "minecraft:item.shield.block", 1.0, 1.0, 0);
        }

        self.state_view.flags(self.runtime_id, super::player_state::Flags {
            using: self.body.busy(), sneaking: self.sneaking, sprinting: self.health.vit.sprinting,
            invisible: self.health.vit.has_effect(crate::survival::EffectKind::Invisibility), burning: self.health.vit.fire > 0,
            fire_immune: self.health.vit.has_effect(crate::survival::EffectKind::FireResistance), own: true,
            blocking: self.body.blocking(*self.shared.ticks.borrow()), spinning: self.body.spinning(),
            // BDS (br-record, gamemodes): при смене режима игроку метаданных не шлёт; спектатора скрывает клиент сам
            // (соседям он скрыт отдельным путём — show_players ниже по файлу).
            spectator: false,
        }, packets);

        // Доел или допил: клиенту — Completed Using Item (предмет и способ:
        // 1 — еда, 3 — питьё), еда — ещё отрыжка у лица (BDS: Burp на 0,1
        // ниже глаз, и самому игроку, и соседям; `of_player` — раздел «игроки»).
        if let Some((item, food)) = done.consumed
            && let Some((id, _, _)) = super::inventory::bedrock_item(item)
        {
            packets.push(super::effects::completed_using_item(id as i16, if food { 1 } else { 3 }));

            // Отрыжка уходит через журнал мира: её слышат и Java-игроки (кроме
            // самого едока — ему Java шлёт её сам), и Bedrock — все, и едок.
            if food {
                let at = (f64::from(self.position.0), f64::from(self.position.1) - 0.1, f64::from(self.position.2));
                let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

                world.note_sound_player_except(at, "minecraft:entity.player.burp", 0.5, 1.0, self.runtime_id as i32);
            }
        }
    }

    /// Правая кнопка в воздухе (или предметом, который блок не ставит):
    /// натянуть лук, поднять щит, начать есть, бросить зелье, выстрелить
    /// заряженным арбалетом — как Use Item у Java.
    fn use_held(&mut self, connection: &Connection) {
        if self.game_mode == crate::commands::GAME_MODE_SPECTATOR || self.health.vit.dead || self.changing.is_some() {
            return;
        }

        if let Some(stack) = self.bag.inventory.held().filter(|s| crate::blocks::item_name(s.item) == Some("map")) {
            let mut maps = self.shared.maps.lock().expect("карты");
            let id = maps.create(self.dimension, self.position.0.floor() as i32, self.position.2.floor() as i32, 0);
            if let Err(error) = maps.save() { crate::log_warn!("Карта не записана: {}", error); return; }
            drop(maps);
            let filled = crate::inventory::Stack::new(crate::blocks::item_named("filled_map").unwrap(), 1)
                .with_extras(crate::stackdata::Data { map_id: Some(id), ..Default::default() });
            let slot = self.bag.inventory.selected_slot();
            let mut slots = vec![slot];
            if !self.creative() && stack.count == 1 { self.bag.inventory.set(slot, Some(filled)); }
            else {
                if !self.creative() { self.bag.inventory.spend_held(); }
                let (changed,left) = self.bag.inventory.put(filled); slots.extend(changed);
                if let Some(left) = left { self.throw_out(left); }
            }
            self.bag.touched(&slots);
            connection.send(&self.bag.content_packets()); return;
        }

        let done = self.run(crate::network::play::Act::Use(0));
        let mut packets = Vec::new();

        log_debug!("Bedrock: {} начал действие предметом {:?}, режим {}, занят: {}", self.identity.name, self.bag.inventory.held(), self.game_mode, self.body.busy());

        self.settle(done, &mut packets);

        if !packets.is_empty() {
            connection.send(&packets);
        }
    }

    /// Ракета фейерверка щёлкнута по блоку: пуск общим кодом. Точка щелчка —
    /// угол блока плюс доля грани, которую прислал клиент. true — пущена.
    fn launch_rocket(&mut self, connection: &Connection, action: &ItemUse) -> bool {
        if self.game_mode == crate::commands::GAME_MODE_SPECTATOR || self.health.vit.dead || self.changing.is_some() {
            return false;
        }

        if self.bag.inventory.held().and_then(|stack| crate::combat::item_name(stack.item)) != Some("firework_rocket") {
            return false;
        }

        let (x, y, z) = action.position;
        let at = (f64::from(x) + f64::from(action.click.0), f64::from(y) + f64::from(action.click.1), f64::from(z) + f64::from(action.click.2));
        let done = self.run(crate::network::play::Act::UseOnBlock { at, face: action.face });
        let launched = !done.slots.is_empty() || self.game_mode == crate::commands::GAME_MODE_CREATIVE;
        let mut packets = Vec::new();

        self.settle(done, &mut packets);

        if !packets.is_empty() {
            connection.send(&packets);
        }

        launched
    }

    /// Кнопка отпущена (Inventory Transaction, ItemRelease)    /// Кнопка отпущена (Inventory Transaction, ItemRelease): лук спущен,
    /// еда доедена или прервана.
    fn release_item(&mut self, connection: &Connection) {
        self.brush_since = None;
        if self.health.vit.dead || self.changing.is_some() {
            return;
        }

        let done = self.run(crate::network::play::Act::Release);
        let mut packets = Vec::new();

        log_debug!("Bedrock: {} отпустил предмет, в руке {:?}, изменились ячейки {:?}", self.identity.name, self.bag.inventory.held(), done.slots);

        self.settle(done, &mut packets);

        if !packets.is_empty() {
            connection.send(&packets);
        }
    }

    /// Такт тела: еда, зарядка арбалета; и вещи с бойцом — другим игрокам
    /// (щит, броня, неуязвимость нужны тем, кто бьёт этого игрока).
    fn tick_body(&mut self, packets: &mut Vec<Vec<u8>>) {
        if self.changing.is_some() || self.health.vit.dead {
            return;
        }

        // Тягун и отложенные попадания живут и после отпускания кнопки.
        let done = self.run(crate::network::play::Act::Tick);
        self.settle(done, packets);

        let stance = self.stance();
        let gear = self.body.gear(&self.shared, &stance, &self.bag.inventory, &self.health.vit, self.last_attack);

        self.shared.players.lock().expect("список игроков захвачен другим потоком").set_gear(&self.identity.uuid, gear);
    }

    /// Что в руке у игрока: то, что знает сервер, а в творческом режиме,
    /// пока инвентарь пуст, — то, что назвал клиент (как при установке блока).
    fn held_stack(&self, claimed: Option<Held>) -> Option<crate::inventory::Stack> {
        self.bag.inventory.held().or_else(|| {
            if self.creative() { claimed.and_then(java_item).map(|item| crate::inventory::Stack::new(item, 1)) } else { None }
        })
    }

    /// Дальность руки до точки: как у остальных действий.
    fn within_reach(&self, point: (f64, f64, f64)) -> bool {
        let reach = if self.creative() { REACH_CREATIVE } else { REACH_SURVIVAL };
        let eyes = (self.position.0 as f64, self.position.1 as f64, self.position.2 as f64);

        distance(eyes, point) <= reach
    }

    /// Действие по клетке, где висит рамка: щелчок кладёт предмет или
    /// поворачивает его, удар вынимает предмет (или ломает пустую рамку),
    /// слом блока убирает рамку целиком — те же действия, что у игрока Java
    /// (`frame::use_on`, `frame::strike`, `frame::break_off`), так что оба
    /// видят одно. Ответ клиенту — то, что теперь на самом деле в клетке.
    /// Возвращает, была ли там рамка (тогда остальное не делается).
    ///
    /// [проверить] живым клиентом: у Bedrock удар по рамке приходит как
    /// начало слома и слом блока; старый пакет Item Frame Drop Item в
    /// протоколе 944 уже не описан.
    fn use_frame(&mut self, connection: &Connection, action: &ItemUse) -> bool {
        if self.game_mode == crate::commands::GAME_MODE_SPECTATOR
            || !matches!(action.kind, START_BREAK | BREAK | CLICK)
            || self.frames_at.is_empty()
            || !block_in_world(self.dimension, action.position)
        {
            return false;
        }

        let Some(frame) = super::frames::frame_at(
            self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком").frames(),
            action.position,
        )
        .cloned() else {
            return false;
        };

        // Повтор щелчка, пока кнопка зажата, ничего не делает.
        if action.kind == CLICK && action.trigger == SIMULATION_TICK {
            return true;
        }

        let (x, y, z) = frame.place();

        if !self.within_reach((x, y, z)) {
            return true;
        }

        let creative = self.creative();
        let mut packets = Vec::new();

        {
            let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

            {
                let mut items = self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком");

                match action.kind {
                    CLICK if self.game_mode != crate::commands::GAME_MODE_ADVENTURE => {
                        let used = crate::frame::use_on(&mut world, &mut items, frame.entity_id, self.held_stack(action.held));

                        if used == crate::frame::Used::Added
                            && !creative
                            && let Some(slot) = self.bag.inventory.spend_held()
                        {
                            self.bag.touched(&[slot]);
                            packets.extend(self.bag.slot_packet(slot));
                        }
                    }
                    CLICK => {}
                    // В творческом один удар ломает рамку с предметом, как
                    // блок; в выживании первый вынимает предмет.
                    START_BREAK if creative => {
                        crate::frame::break_off(&mut world, &mut items, &self.shared, frame.entity_id, true);
                    }
                    START_BREAK => {
                        crate::frame::strike(&mut world, &mut items, &self.shared, frame.entity_id, false);
                    }
                    _ => {
                        crate::frame::break_off(&mut world, &mut items, &self.shared, frame.entity_id, creative);
                    }
                }
            }

            packets.extend(self.restore(&world, &[action.position]));
        }

        log_debug!("Bedrock: {} — действие {} по рамке в {:?}", self.identity.name, action.kind, action.position);
        connection.send(&packets);
        true
    }

    /// Вешает рамку предметом в руке на грань блока (у Java — щелчок по
    /// блоку с рамкой в руке, `network::play`): звук, сама рамка и повтор
    /// соседей. Возвращает, была ли в руке рамка (тогда остальное не
    /// делается).
    fn place_frame(&mut self, connection: &Connection, action: &ItemUse) -> bool {
        let Some(held) = self.held_stack(action.held) else {
            return false;
        };
        let glow = match crate::blocks::item_name(held.item) {
            Some("item_frame") => false,
            Some("glow_item_frame") => true,
            Some("painting") => false,
            _ => return false,
        };
        let clicked = action.position;

        if self.game_mode == crate::commands::GAME_MODE_SPECTATOR
            || self.game_mode == crate::commands::GAME_MODE_ADVENTURE
            || !(0..=5).contains(&action.face)
            || !self.within_reach((clicked.0 as f64 + 0.5, clicked.1 as f64 + 0.5, clicked.2 as f64 + 0.5))
        {
            return true;
        }

        let beyond = offset(clicked, action.face);
        let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
        let placed = {
            let mut items = self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком");

            if self.held_stack(action.held).is_some_and(|s|crate::blocks::item_name(s.item)==Some("painting")) {
                crate::painting::place(&mut world,&mut items,&self.shared,clicked,action.face as u8)
            } else {crate::frame::place(&mut world, &mut items, &self.shared, clicked, action.face as u8, glow)}
        };

        if !placed {
            // Клиент уже нарисовал свою рамку — вернём ему правду.
            let packets = self.restore(&world, &[beyond]);
            drop(world);

            if !packets.is_empty() {
                connection.send(&packets);
            }

            return true;
        }

        world.tell_directly(clicked.0, clicked.1, clicked.2);
        world.tell_directly(beyond.0, beyond.1, beyond.2);
        drop(world);

        if !self.creative()
            && let Some(slot) = self.bag.inventory.spend_held()
        {
            self.bag.touched(&[slot]);
            connection.send(&self.bag.slot_packet(slot).into_iter().collect::<Vec<_>>());
        }

        log_debug!("Bedrock: {} повесил рамку в {:?}", self.identity.name, beyond);
        true
    }

    /// Ставит вагонетку предметом в руке на рельсу, как у Java (вики,
    /// Minecart: «can be placed on rails»). None — в руке не вагонетка,
    /// Some(false) — щёлкнул не по рельсе.
    fn place_cart(&mut self, connection: &Connection, held: Option<Held>, clicked: containers::Pos) -> Option<bool> {
        // Что в руке — знает сервер; клиенту верим только в творческом, как
        // при установке блока.
        let from_bag = self.bag.inventory.held().map(|stack| stack.item);
        let claimed = if self.creative() { held.and_then(java_item) } else { None };
        let kind = from_bag
            .or(claimed)
            .and_then(crate::blocks::item_name)
            .and_then(crate::minecart::Kind::from_name)?;
        let state = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком").get_block(clicked.0, clicked.1, clicked.2);
        let Some(at) = crate::minecart::place_on_rail(state, clicked) else {
            return Some(false);
        };

        self.shared
            .items_in(self.dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .add_cart(crate::minecart::Minecart::new(self.shared.next_entity_id(), kind, at));

        if !self.creative()
            && let Some(slot) = self.bag.inventory.spend_held()
        {
            self.bag.touched(&[slot]);
            connection.send(&self.bag.slot_packet(slot).into_iter().collect::<Vec<_>>());
        }

        log_debug!("Bedrock: {} поставил вагонетку в {:?}", self.identity.name, clicked);
        Some(true)
    }

    fn use_block(&mut self, clicked: containers::Pos, face: i32, cursor: (f32, f32, f32)) -> Option<Vec<Vec<u8>>> {
        // Наблюдатель ничего не трогает; в приключении рычаги, двери и
        // сундуки — можно.
        if self.game_mode == crate::commands::GAME_MODE_SPECTATOR {
            return None;
        }

        // Щёлкнули по миру — экрана у клиента уже нет.
        if self.window.is_some() || self.station_window.is_some() {
            self.forget_stale_window();
        }

        let reach = if self.creative() { REACH_CREATIVE } else { REACH_SURVIVAL };
        let eyes = (self.position.0 as f64, self.position.1 as f64, self.position.2 as f64);
        let center = (clicked.0 as f64 + 0.5, clicked.1 as f64 + 0.5, clicked.2 as f64 + 0.5);

        if distance(eyes, center) > reach {
            return None;
        }

        let respawn_block = {
            let world = self.shared.world_in(self.dimension).lock().expect("мир");
            let state = world.get_block(clicked.0, clicked.1, clicked.2);
            crate::bed::is_anchor(state) || (crate::bed::is_bed(state) && crate::bed::beds_work(self.dimension))
        };
        if respawn_block {
            let done = self.run(crate::network::play::Act::RespawnBlock { at: clicked, face });
            let used = done.block_used; let mut packets = Vec::new(); self.settle(done, &mut packets);
            if used { return Some(packets); }
        }

        let (x, y, z) = clicked;
        let beyond = offset(clicked, face);
        let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

        if !world.has_chunk(x >> 4, z >> 4) {
            return None;
        }

        if crate::redstone::is_sign(world.get_block(x,y,z)) {
            let front=super::signs::front(&world,clicked,self.position);
            let data=world.sign(x,y,z).cloned().unwrap_or_default();
            let held=self.bag.inventory.held().and_then(|s|crate::blocks::item_name(s.item));
            let has_text=data.side(front).lines.iter().any(|l|!l.is_empty());
            let mut changed=false;
            if !data.waxed {
                let mut next=data.clone();
                if held==Some("honeycomb") {next.waxed=true;changed=true;}
                else if has_text {let side=next.side_mut(front);
                    if held==Some("glow_ink_sac") && !side.glowing {side.glowing=true;changed=true;}
                    else if held==Some("ink_sac") && side.glowing {side.glowing=false;changed=true;}
                    else if let Some(color)=held.and_then(crate::sign::dye_color) && side.color!=color {side.color=color;changed=true;}
                }
                if changed {
                    world.with_sign(x,y,z,|s|*s=next.clone());
                    if let Some(kind)=crate::redstone::block_entity_type(world.get_block(x,y,z)){world.note_block_entity(x,y,z,kind,next.nbt());}
                }
            }
            let mut packets=Vec::new();packets.extend(super::signs::actor(&world,clicked));drop(world);
            if changed {
                if !self.creative() && let Some(slot)=self.bag.inventory.spend_held() {self.bag.touched(&[slot]);packets.extend(self.bag.slot_packet(slot));}
            } else if !data.waxed {self.sign_editor=Some((clicked,front));packets.push(super::signs::editor(clicked,front));}
            return Some(packets);
        }
        let kind = containers::Kind::of(world.get_block(x, y, z));
        // Vault не имеет окна: ключ и выдача награды — общий серверный ход.
        let held = self.bag.inventory.held();
        let now = *self.shared.ticks.borrow();
        let seed = now ^ self.runtime_id;
        let consumed = crate::trial::use_vault(&mut world, clicked, self.identity.uuid, held.and_then(|s| crate::blocks::item_name(s.item)), now, seed);
        if let Some(consumed) = consumed {
            drop(world);
            let mut packets = Vec::new();
            if consumed && !self.creative() && let Some(slot) = self.bag.inventory.spend_held() {
                self.bag.touched(&[slot]); packets.extend(self.bag.slot_packet(slot));
            }
            return Some(packets);
        }
        if kind == Some(containers::Kind::ChiseledBookshelf)
            && let Some(used) = crate::redstone::bookshelf::use_bookshelf(&mut world, clicked, face, cursor, held)
        {
            drop(world);
            let mut packets = Vec::new();
            match used {
                crate::redstone::bookshelf::Used::Placed => {
                    if !self.creative() && let Some(slot) = self.bag.inventory.spend_held() {
                        self.bag.touched(&[slot]); packets.extend(self.bag.slot_packet(slot));
                    }
                }
                crate::redstone::bookshelf::Used::Taken(stack) => {
                    let (slots, left) = self.bag.inventory.put(stack); self.bag.touched(&slots);
                    packets.extend(slots.into_iter().filter_map(|slot| self.bag.slot_packet(slot)));
                    if let Some(left) = left { self.throw_out(left); }
                }
            }
            return Some(packets);
        }
        if kind == Some(containers::Kind::Shelf) {
            let hotbar: Vec<_> = (crate::inventory::FIRST_HOTBAR..=crate::inventory::LAST_HOTBAR).map(|slot| self.bag.inventory.slot(slot)).collect();
            let selected = (self.bag.inventory.selected_slot() - crate::inventory::FIRST_HOTBAR) as usize;
            if let Some(used) = crate::redstone::shelf::use_shelf(&mut world,clicked,face,cursor,&hotbar,selected,self.creative()) {
                drop(world);
                let changes = match used {
                    crate::redstone::shelf::Used::Hand(stack) => vec![(self.bag.inventory.selected_slot(),stack)],
                    crate::redstone::shelf::Used::Hotbar(slots) => slots.into_iter().map(|(slot,stack)| (crate::inventory::FIRST_HOTBAR+slot as i32,stack)).collect(),
                };
                let mut packets = Vec::new();
                for (slot,stack) in changes { self.bag.inventory.set(slot,stack); self.bag.touched(&[slot]); packets.extend(self.bag.slot_packet(slot)); }
                return Some(packets);
            }
        }

        // Проигрыватель и кафедра — общий с Java ход (redstone/jukebox.rs,
        // redstone/lectern.rs): пластинка и книга из руки. Окна кафедры у
        // Bedrock пока нет. [проверить] пакет Lectern Update.
        if matches!(kind, Some(containers::Kind::Jukebox | containers::Kind::Lectern)) {
            let held = self.bag.inventory.held();
            let took = if kind == Some(containers::Kind::Jukebox) {
                crate::redstone::use_jukebox(&mut world, clicked, held)?
            } else {
                crate::redstone::lectern::place_book(&mut world, clicked, held, 0)?
            };

            world.tell_directly(x, y, z);
            world.tell_directly(beyond.0, beyond.1, beyond.2);
            drop(world);

            let mut packets = Vec::new();
            if !took && kind==Some(containers::Kind::Lectern) {
                self.next_window=screens::next_window(self.next_window);let id=self.next_window;
                let world=self.shared.world_in(self.dimension).lock().expect("мир");packets.extend(super::lectern::actor(&world,clicked));
                let mut open=Out::packet(session::CONTAINER_OPEN);open.u8(id).u8(25).block_position(x,y,z).zigzag64(-1);packets.push(open.bytes);
                self.lectern=Some((id,clicked));
            }

            if took
                && !self.creative()
                && let Some(slot) = self.bag.inventory.spend_held()
            {
                self.bag.touched(&[slot]);
                packets.extend(self.bag.slot_packet(slot));
            }

            return Some(packets);
        }

        // Верстак, стол зачарования, наковальня, точило: окно без хранилища
        // в мире, ячейки — в интерфейсе игрока (stations.rs).
        if let Some(station) = crate::blocks::block_at_state(world.get_block(x, y, z)).and_then(stations::Kind::of) {
            if self.window.is_some() || self.station_window.is_some() {
                return Some(Vec::new());
            }

            let shelves = if station == stations::Kind::Enchanting {
                crate::enchanting::bookshelves(|(bx, by, bz)| world.get_block(bx, by, bz), clicked)
            } else {
                0
            };

            drop(world);

            log_debug!("Bedrock: {} открыл {:?} в {:?}", self.identity.name, station, clicked);
            return Some(self.open_station(station, clicked, shelves));
        }

        // У этих блоков нет обычного окна; щелчок может поставить соседний блок.
        let kind = kind.filter(|kind| !matches!(kind, containers::Kind::Shelf | containers::Kind::ChiseledBookshelf | containers::Kind::Campfire));

        if let Some(kind) = kind {
            let mut halves = containers::halves(&world, clicked);

            // Эндер-сундук не хранилище в мире (halves пуст), но окно у него есть: его
            // «половина» — сам блок (без этого open_window падала на halves[0]).
            if halves.is_empty() && kind == containers::Kind::EnderChest {
                halves.push(clicked);
            }

            // Окно уже открыто или сундук придавлен сверху — ничего не
            // открывается, но и блок не ставится.
            if self.window.is_some() || !containers::can_open(&world, &halves) {
                return Some(Vec::new());
            }

            // Как у Java: звук, общая рассылка (бочка «открыта») и крышка в
            // фазе блочных событий. Повтора самого блока и соседа клиенту
            // не шлём: BDS на щелчок по хранилищу их не шлёт (снято по сети,
            // tmp/br-bds).
            let first = containers::open_begin(&mut world, &halves);

            containers::open_finish(&mut world, &halves, first);
            crate::redstone::settle(&mut world);

            let slots = if kind == containers::Kind::EnderChest { self.ender.clone() } else { containers::window_slots(&world, &halves) };
            drop(world);

            log_debug!("Bedrock: {} открыл {:?} в {:?}", self.identity.name, kind, clicked);

            // Бочка: BDS шлёт её крышку (Block Event 1) до Container Open.
            let mut packets = Vec::new();

            if kind == containers::Kind::Barrel {
                self.lids.insert(clicked, true);
                packets.push(screens::lid_packet(clicked, true));
            }

            packets.extend(self.open_window(kind, halves, &slots, None));
            return Some(packets);
        }

        // Куст ягод: сбор любой рукой (farming.rs) — как у Java до рычагов.
        if let Some(stack) = crate::farming::pick_berries(&mut world, clicked) {
            let (at, velocity) = crate::tick::scattered(clicked, || f64::from(world.random_unit()));

            world.tell_directly(x, y, z);
            world.tell_directly(beyond.0, beyond.1, beyond.2);
            drop(world);
            self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком").drop_item(
                self.shared.next_entity_id(),
                stack,
                at,
                velocity,
                crate::items::MINED_DELAY,
            );

            return Some(Vec::new());
        }

        // Статуя медного голема: следующая поза (redstone/statue.rs).
        if crate::redstone::statue::use_statue(&mut world, clicked) {
            world.tell_directly(x, y, z);
            world.tell_directly(beyond.0, beyond.1, beyond.2);
            crate::redstone::settle(&mut world);

            return Some(Vec::new());
        }

        if !crate::redstone::use_block_on(&mut world, clicked, face) {
            return None;
        }

        log_debug!("Bedrock: {} щёлкнул по {:?}", self.identity.name, clicked);

        // Повтор самого блока и того, что за гранью, — сразу, а перемены
        // вокруг — общей пачкой; нота музыкального блока — последней.
        world.tell_directly(x, y, z);
        world.tell_directly(beyond.0, beyond.1, beyond.2);
        crate::redstone::settle(&mut world);
        crate::redstone::after_use(&mut world, clicked);

        Some(Vec::new())
    }

    /// Клиент щёлкнул по блоку в мире, а окно у нас числится открытым: экран
    /// у него уже закрыт (сообщение о закрытии потерялось или пришло с чужим
    /// видом). Окно давнее — закрываем его у себя молча и говорим, что можно
    /// открывать заново. Совсем свежее окно — это повтор щелчка, его не трогаем.
    fn forget_stale_window(&mut self) -> bool {
        if self.ticks - self.opened_at < STALE_WINDOW_TICKS {
            return false;
        }

        log_debug!("Bedrock: {} — щелчок по миру при «открытом» окне, окно сброшено", self.identity.name);

        if self.window.is_some() {
            self.close_window();
        }

        if self.station_window.is_some() {
            self.close_station();
        }

        true
    }

    /// Открывает станок: Container Open с видом экрана; у стола — сразу его
    /// предложения.
    fn open_station(&mut self, kind: stations::Kind, at: containers::Pos, shelves: i32) -> Vec<Vec<u8>> {
        self.next_window = screens::next_window(self.next_window);

        let id = self.next_window;
        let mut open = Out::packet(session::CONTAINER_OPEN);
        open.u8(id).u8(kind.screen()).block_position(at.0, at.1, at.2).zigzag64(-1);

        self.station_window = Some(id);
        self.opened_at = self.ticks;
        self.bag.open_station(super::inventory::Station { kind, pos: at, seed: self.health.vit.xp_seed, shelves });

        // У стола зачарования в Bedrock есть блок-актор — клиенту он, как у
        // хранилищ, нужен до экрана.
        let mut packets = Vec::new();

        if kind == stations::Kind::Enchanting {
            packets.push(screens::enchant_table_actor(at));
        }
        if kind == stations::Kind::Beacon {
            let world = self.shared.world_in(self.dimension).lock().expect("мир");
            packets.extend(super::block_actors::live(&world, at));
        }

        log_debug!("Bedrock: {} — Container Open: окно {}, экран {} ({}), место {:?}", self.identity.name, id, kind.screen(), screens::screen_name(kind.screen() as i8), at);
        packets.push(open.bytes);
        packets.extend(self.bag.enchant_options());
        packets
    }

    /// Что станок попросил сделать миру после удавшегося запроса: уровни,
    /// звуки, износ наковальни, опыт точила, новое семя стола. Возвращает
    /// пакеты для клиента.
    fn station_events(&mut self, events: Vec<Event>) -> Vec<Vec<u8>> {
        let crafted = events.iter().any(|e| matches!(e, Event::Crafted(..)));
        for event in events {
            match event {
                Event::Crafted(item, count) => {
                    self.progress.crafted(item, count);
                    if crate::blocks::item_name(item) == Some("crafting_table") { self.progress.trigger("story/root"); }
                }
                // Уже применено под тем же замком, что проверка оплаты.
                Event::BeaconSelected(..) => {}
                Event::Levels(levels) => {
                    self.health.vit.command_xp(false, -levels, true);
                }
                Event::Enchanted(pos) => {
                    self.health.vit.xp_seed = crate::enchanting::new_seed(self.runtime_id);

                    if let Some(mut station) = self.bag.station {
                        station.seed = self.health.vit.xp_seed;
                        self.bag.station = Some(station);
                    }

                    let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                    let mut rng = crate::mob::Rng::new(u64::from(self.shared.next_entity_id() as u32));
                    let pitch = rng.unit() as f32 * 0.1 + 0.9;

                    world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.enchantment_table.use", 1.0, pitch);
                }
                Event::Traded(index) => {
                    let Some(villager) = self.bag.station.filter(|station| station.kind == stations::Kind::Trading).map(|station| station.pos.0) else {
                        continue;
                    };

                    // Пачка по Shift — сделки одна за другой: кончился запас — стоп.
                    if !self.bag.trade_stocked(index) {
                        continue;
                    }

                    // Счёт, опыт, звук и шарик — общий код Java (merchant.rs).
                    crate::network::play::trade_paid(&self.shared, self.dimension, villager, index);

                    // Сделки (запас, цены) — по новому состоянию жителя.
                    if let Some(state) = crate::network::play::villager_state(&self.shared, self.dimension, villager) {
                        self.bag.set_trade(super::trade::prices(&state));
                    }
                }
                Event::AnvilUsed(pos) => self.anvil_used(pos),
                Event::Smithed(pos) => {
                    self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком").note_level_event(pos.0, pos.1, pos.2, 1044, 0);
                }
                Event::StationWorked(kind,pos) => {
                    let sound=match kind {stations::Kind::Stonecutter=>"minecraft:ui.stonecutter.take_result",stations::Kind::Loom=>"minecraft:ui.loom.take_result",_=>"minecraft:ui.cartography_table.take_result"};
                    self.shared.world_in(self.dimension).lock().expect("мир").note_sound(pos.0,pos.1,pos.2,sound,1.0,1.0);
                }
                Event::Ground(inputs, pos) => {
                    let mut rng = crate::mob::Rng::new(u64::from(self.shared.next_entity_id() as u32) ^ (*self.shared.ticks.borrow() << 20));
                    let points = crate::enchanting::grind_xp(&inputs, |half| rng.below(half as u32) as i32);
                    let at = (pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5);

                    if points > 0 {
                        self.shared
                            .items_in(self.dimension)
                            .lock()
                            .expect("предметы захвачены другим потоком")
                            .drop_experience(points, at, &mut || self.shared.next_entity_id(), &mut rng);
                    }

                    // Звук точила — событие мира 1042 (вики, World Event).
                    self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком").note_level_event(pos.0, pos.1, pos.2, 1042, 0);
                }
            }
        }

        let mut packets: Vec<_> = self.bag.enchant_options().into_iter().collect();
        if crafted { packets.extend(self.bag.crafting_sync_packets()); }
        packets
    }

    /// Наковальня отработала: 12 % — на ступень хуже (разрушенная исчезает,
    /// событие 1029), иначе звук работы (событие 1030); в творческом не
    /// портится (как у Java, network/play/station.rs, anvil_used).
    fn anvil_used(&mut self, (x, y, z): containers::Pos) {
        let creative = self.creative();
        let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
        let state = world.get_block(x, y, z);
        let name = crate::blocks::block_at_state(state).unwrap_or("anvil");
        let mut rng = crate::mob::Rng::new(u64::from(self.shared.next_entity_id() as u32) ^ (*self.shared.ticks.borrow() << 20));

        if creative || rng.unit() >= crate::enchanting::ANVIL_DAMAGE_CHANCE {
            world.note_level_event(x, y, z, 1030, 0);
            return;
        }

        match crate::enchanting::anvil_damaged(name) {
            Some(next) => {
                let facing = crate::blocks::orientation_of(state).and_then(|block| block.value(state, "facing")).unwrap_or("north");

                if let Some(worse) = crate::blocks::state_from_text(&format!("minecraft:{next}[facing={facing}]")) {
                    world.set_block(x, y, z, worse);
                }

                world.note_level_event(x, y, z, 1030, 0);
            }
            None => {
                world.set_block(x, y, z, world::AIR);
                world.note_level_event(x, y, z, 1029, 0);
            }
        }

        crate::redstone::settle(&mut world);
    }

    /// Открывает клиенту окно хранилища в порядке официального BDS 1.26.13
    /// (снято по сети, tmp/br-bds): у печей и стойки сперва Container Set
    /// Data, потом Container Open (номер окна от 2, вид экрана, место),
    /// Inventory Content с ячейками хранилища одним пакетом (у двойного
    /// сундука все 54, в порядке окна Java — `containers::halves`; у печи
    /// и стойки — в порядке Bedrock), у сундука крышка, затем рюкзак, броня,
    /// интерфейс, вторая рука, Player Hotbar и снова ячейки хранилища.
    /// Место — ведущая половина: её ячейки в окне первые.
    ///
    /// `cart` — окно вагонетки-хранилища (номер сущности и её клетка):
    /// Container Open с номером сущности (BDS: тип 0 у сундука, 8 у воронки,
    /// место — клетка вагонетки), два Inventory Content подряд и больше
    /// ничего — ни рюкзака, ни крышек (снято по сети, tmp/br-trade).
    fn open_window(
        &mut self,
        kind: containers::Kind,
        halves: Vec<containers::Pos>,
        slots: &[Option<crate::inventory::Stack>],
        cart: Option<(i32, containers::Pos)>,
    ) -> Vec<Vec<u8>> {
        self.next_window = screens::next_window(self.next_window);

        let id = self.next_window;
        let lead = cart.map_or_else(|| halves[0], |(_, place)| place);
        let lids: Vec<containers::Pos> = if kind.is_chest() { halves.clone() } else { Vec::new() };

        match stations::layout(kind) {
            Some(layout) => self.bag.open_window_for(slots, layout, Some(kind)),
            None => self.bag.open_window_for(slots, Vec::new(), Some(kind)),
        }

        self.opened_at = self.ticks;
        self.window = Some(OpenWindow { id, kind, halves, props: Vec::new(), entity: cart.map(|(entity, _)| entity) });

        let mut packets = self.window_properties();

        packets.push(match cart {
            Some((entity, place)) => screens::open_entity_packet(id, kind, place, entity),
            None => screens::open_packet(id, kind, lead),
        });

        let content = self.bag.window_full_packet(id);

        packets.extend(content.iter().cloned());

        // Вагонетка: рюкзак и повтор содержимого BDS не шлёт в этом же такте —
        // второе содержимое придёт со следующим тактом (`resync_window`).
        if cart.is_some() {
            self.resync_window = true;
            return packets;
        }

        for place in lids {
            self.lids.insert(place, true);
            packets.push(screens::lid_packet(place, true));
        }

        packets.extend(self.bag.open_sync_packets());
        packets.extend(content);
        self.resync_window = !matches!(kind, containers::Kind::Furnace | containers::Kind::BlastFurnace | containers::Kind::Smoker | containers::Kind::BrewingStand);
        log_debug!(
            "Bedrock: {} — Container Open: окно {}, экран {} ({}), пакетов {}",
            self.identity.name, id, screens::screen(kind), screens::screen_name(screens::screen(kind) as i8), packets.len()
        );
        packets
    }

    /// Окно вагонетки с сундуком или воронкой: те же ячейки, что у Java
    /// (`open_cart_window` в network/play.rs), окно — как у сундука и воронки
    /// (`open_window`). Пусто — окна нет (вагонетка без хранилища, другое
    /// окно уже открыто).
    fn open_cart(&mut self, entity: i32) -> Vec<Vec<u8>> {
        let Some((cart_kind, (x, y, z), slots)) = crate::network::play::cart_info(&self.shared, self.dimension, entity) else {
            return Vec::new();
        };
        let kind = match cart_kind {
            crate::minecart::Kind::Chest => containers::Kind::Chest,
            crate::minecart::Kind::Hopper => containers::Kind::Hopper,
            _ => return Vec::new(),
        };

        if self.window.is_some() || self.station_window.is_some() {
            return Vec::new();
        }

        log_debug!("Bedrock: {} открыл вагонетку {} ({:?})", self.identity.name, entity, cart_kind);
        self.open_window(kind, Vec::new(), &slots, Some((entity, (x.floor() as i32, y.floor() as i32, z.floor() as i32))))
    }

    /// Окно торговли с жителем в порядке BDS (tmp/br-trade): Update Trade,
    /// пустое содержимое окна из трёх ячеек, метка «торгует с» у жителя.
    /// Ячейки оплаты — в интерфейсе игрока (`Bag::station` с видом
    /// `Trading`), а сделки и цены — те же, что у Java (villager.rs).
    /// Пусто — окно не открылось.
    fn open_trade(&mut self, villager: i32) -> Vec<Vec<u8>> {
        if self.window.is_some() || self.station_window.is_some() {
            return Vec::new();
        }

        let Some(state) = crate::network::play::villager_state(&self.shared, self.dimension, villager) else {
            return Vec::new();
        };

        self.next_window = screens::next_window(self.next_window);

        let id = self.next_window;

        self.station_window = Some(id);
        self.opened_at = self.ticks;
        self.bag.open_station(super::inventory::Station { kind: stations::Kind::Trading, pos: (villager, 0, 0), seed: 0, shelves: 0 });
        self.bag.set_trade(super::trade::prices(&state));
        log_debug!("Bedrock: {} торгует с жителем {}: окно {}, сделок {}", self.identity.name, villager, id, state.offers.len());

        vec![
            super::trade::update_packet(id, villager, &state),
            super::trade::content_packet(id),
            super::trade::trading_player_packet(villager, self.runtime_id as i32),
        ]
    }

    /// Свойства окна печи и стойки (Container Set Data): что сменилось
    /// с прошлого раза.
    fn window_properties(&mut self) -> Vec<Vec<u8>> {
        let Some(window) = &self.window else {
            return Vec::new();
        };
        let java = {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
            let Some(&(x, y, z)) = window.halves.first() else {
                return Vec::new();
            };

            world.container(x, y, z).map(crate::furnace::properties).unwrap_or_default()
        };
        let now = stations::bedrock_properties(window.kind, &java);
        let id = window.id;
        let packets = now
            .iter()
            .enumerate()
            .filter(|(index, value)| window.props.get(*index) != Some(value))
            .map(|(_, &(property, value))| stations::set_data_packet(id, property, value))
            .collect();

        if let Some(window) = &mut self.window {
            window.props = now;
        }

        packets
    }

    /// Хранилища окна больше нет (сломали, сменилось другим блоком).
    fn window_gone(world: &world::World, window: &OpenWindow) -> bool {
        window.halves.iter().any(|&(x, y, z)| {
            world.container(x, y, z).is_none() || containers::Kind::of(world.get_block(x, y, z)).is_none()
        })
    }

    /// Закрывает окно хранилища: хранилище теряет смотрящего, как у Java
    /// (close_own_window в network/play.rs), ячейки окна забываются.
    /// Возвращает, какое окно было закрыто.
    fn close_window(&mut self) -> Option<OpenWindow> {
        let window = self.window.take()?;

        self.bag.close_window();

        // У вагонетки смотрящих не считают.
        if window.entity.is_some() {
            return Some(window);
        }

        let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

        // Смотрящим он был, только пока хранилище стоит на месте.
        if window.halves.iter().all(|&(x, y, z)| world.container(x, y, z).is_some()) {
            containers::close(&mut world, &window.halves);
            crate::redstone::settle(&mut world);
        } else {
            for &(x, y, z) in &window.halves {
                world.add_viewers(x, y, z, -1);
            }
        }

        Some(window)
    }

    /// Закрывает окно хранилища по воле сервера — и говорит об этом
    /// клиенту (Container Close с «закрыл сервер»).
    fn close_window_by_server(&mut self) -> Option<Vec<u8>> {
        let window = self.close_window()?;

        log_debug!("Bedrock: {} — окно хранилища закрыто сервером", self.identity.name);
        Some(screens::close_packet(window.id, screens::screen(window.kind), true))
    }

    /// Item Stack Request. Пока открыто окно хранилища, его ячейки берутся
    /// из мира прямо перед запросом и кладутся обратно после — под одним
    /// замком мира, как щелчок окна у Java (click_window в network/play.rs):
    /// воронка между ними ничего не потеряет и не размножит. Вместе с ответом
    /// — ячейки, которые сменились без клиента, чтобы он видел то же, что
    /// сервер.
    fn stack_requests(&mut self, payload: &[u8]) -> (super::inventory::Handled, Vec<Vec<u8>>) {
        let creative = self.creative();

        self.bag.level = crate::survival::level_of(self.health.vit.xp).0;

        if let Some(station) = self.bag.station.filter(|s| s.kind == stations::Kind::Beacon) {
            let p = station.pos;
            let allowed = !self.health.vit.dead && self.changing.is_none()
                && self.game_mode != crate::commands::GAME_MODE_SPECTATOR
                && self.within_reach((p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5));
            let mut world = self.shared.world_in(self.dimension).lock().expect("мир");
            let exists = world.container(p.0,p.1,p.2).is_some_and(|c| c.kind == containers::Kind::Beacon);
            self.bag.station.as_mut().unwrap().shelves = if allowed && exists { crate::beacon::level(&world,p) } else { 0 };
            let handled = self.bag.handle_requests(payload,creative);
            self.bag.station.as_mut().unwrap().shelves = 0;
            for event in &handled.events {
                if let Event::BeaconSelected(p,primary,secondary) = *event {
                    world.with_container(p.0,p.1,p.2,|c| { c.beacon[1]=primary; c.beacon[2]=secondary; });
                    let data = crate::beacon::nbt(world.container(p.0,p.1,p.2).unwrap());
                    world.note_block_entity(p.0,p.1,p.2,containers::Kind::Beacon.block_entity_type(),data);
                    world.note_sound(p.0,p.1,p.2,"minecraft:block.beacon.power_select",1.0,1.0);
                }
            }
            return (handled,super::block_actors::live(&world,p).into_iter().collect());
        }

        let Some(window) = &self.window else {
            if self.bag.station.is_some_and(|s|s.kind==stations::Kind::Cartography) {
                let mut maps=self.shared.maps.lock().expect("карты");self.bag.cartography_maps=Some(maps.clone());
                let (handled,packets)=self.bag.cartography_requests(payload,creative);
                if let Some(next)=self.bag.cartography_maps.take(){*maps=next;if let Err(e)=maps.save(){crate::log_warn!("Карты: {}",e);}}
                return (handled,packets);
            }
            return (self.bag.handle_requests(payload, creative), Vec::new());
        };

        let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
        let cart = window.entity;
        let cart_now = cart.map(|entity| crate::network::play::cart_contents(&self.shared, self.dimension, entity));
        let now = match &cart_now {
            Some(slots) => slots.clone().unwrap_or_default(),
            None if window.kind == containers::Kind::EnderChest => self.ender.clone(),
            None => containers::window_slots(&world, &window.halves),
        };
        let gone = match &cart_now {
            Some(slots) => slots.is_none(),
            None => Self::window_gone(&world, window),
        };

        // Хранилища уже нет (его содержимое высыпалось): окно закрывается,
        // запрос выполняется без него — иначе вещи бы размножились.
        if gone || Some(now.len()) != self.bag.window_slots().map(<[_]>::len) {
            drop(world);

            let packets = self.close_window_by_server().into_iter().collect();
            return (self.bag.handle_requests(payload, creative), packets);
        }

        let (id, name, halves, kind) = (window.id, screens::slot_name(window.kind), window.halves.clone(), window.kind);
        let mut world = world;
        let packets: Vec<Vec<u8>> =
            self.bag.update_window(&now).into_iter().filter_map(|slot| self.bag.window_slot_packet(id, name, slot)).collect();
        self.bag.disabled_slots = if kind == containers::Kind::Crafter {
            let p = halves[0]; world.container(p.0, p.1, p.2).map_or(0, |c| c.disabled)
        } else { 0 };
        let handled = self.bag.handle_requests(payload, creative);
        let after = self.bag.window_slots().map(<[_]>::to_vec).unwrap_or_default();
        let mut xp_points = 0;

        if after != now && let Some(entity) = cart {
            // Вагонетка: ячейки — у неё; снимок для окон — тоже, иначе
            // ближайшая рассылка вернула бы клиенту прежнее.
            crate::network::play::store_cart(&self.shared, self.dimension, entity, &after);

            if let Some(published) = world.published_carts.get_mut(&entity)
                && published.len() == after.len()
            {
                published.clone_from(&after);
            }
        } else if after != now && kind == containers::Kind::EnderChest {
            self.ender.clone_from(&after);
        } else if after != now {
            containers::store_window_slots(&mut world, &halves, &after);

            // Из выхода печи забрали выплавленное — игроку весь накопленный
            // опыт (как у Java, network/play.rs, click_window).
            if crate::furnace::is_oven(kind) {
                let count = |slots: &[Option<crate::inventory::Stack>]| slots.get(crate::furnace::OUTPUT).copied().flatten().map_or(0, |stack| stack.count);

                if count(&after) < count(&now) {
                    xp_points = crate::furnace::take_experience(&mut world, halves[0]);
                }
            }

            crate::redstone::settle(&mut world);

            // Так это уже видит и этот клиент: снимок для окон — тоже, иначе
            // ближайшая рассылка вернула бы ему прежнее.
            let mut from = 0;

            for &(x, y, z) in &halves {
                let size = world.container(x, y, z).map_or(0, |container| container.slots.len());

                if let (Some(published), Some(part)) = (world.published.get_mut(&(x, y, z)), after.get(from..from + size))
                    && published.len() == size
                {
                    published.copy_from_slice(part);
                }

                from += size;
            }
        }

        drop(world);

        if xp_points > 0 {
            let feet = (self.position.0 as f64, (self.position.1 - EYES) as f64, self.position.2 as f64);
            let mut rng = crate::mob::Rng::new(u64::from(self.shared.next_entity_id() as u32) ^ (*self.shared.ticks.borrow() << 20));

            self.shared
                .items_in(self.dimension)
                .lock()
                .expect("предметы захвачены другим потоком")
                .drop_experience(xp_points, feet, &mut || self.shared.next_entity_id(), &mut rng);
        }

        (handled, packets)
    }

    /// Досылает открытому окну перемены содержимого, опубликованные к этому
    /// мгновению такта (`World::published`), — как send_window_changes у
    /// Java. Хранилища больше нет — окно закрывается.
    fn window_changes(&mut self) -> Vec<Vec<u8>> {
        let mut packets = self.window_slot_changes();

        packets.extend(self.window_properties());
        packets.extend(self.station_check());
        packets
    }

    /// Станок игрока стоит на месте? Нет (сломали, ушёл далеко) — окно
    /// закрывается сервером, вещи из ячеек возвращаются.
    fn station_check(&mut self) -> Vec<Vec<u8>> {
        let (Some(station), Some(id)) = (self.bag.station, self.station_window) else {
            return Vec::new();
        };
        let gone = if station.kind == stations::Kind::Trading {
            // Житель пропал или умер — окно закрывается (вики, Trading).
            crate::network::play::villager_missing(&self.shared, self.dimension, station.pos.0)
        } else {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
            let (x, y, z) = station.pos;

            crate::blocks::block_at_state(world.get_block(x, y, z)).and_then(stations::Kind::of) != Some(station.kind) || station.kind == stations::Kind::Workbench && (f64::from(self.position.0) - f64::from(x) - 0.5).powi(2) + (f64::from(self.position.1 - EYES) - f64::from(y) - 0.5).powi(2) + (f64::from(self.position.2) - f64::from(z) - 0.5).powi(2) > 64.0
        };

        if !gone {
            return Vec::new();
        }

        let mut packets = vec![screens::close_packet(id, station.kind.screen(), true)];

        packets.extend(self.close_station());
        packets
    }

    /// Закрывает станок: вещи из ячеек — в инвентарь, лишнее — на землю.
    /// Возвращает пакеты слотов инвентаря.
    fn close_station(&mut self) -> Vec<Vec<u8>> {
        self.station_window = None;

        // Торговля закончилась: житель свободен и снова ни с кем не торгует.
        let trade = self.bag.station.filter(|station| station.kind == stations::Kind::Trading);
        let (overflow, changed) = self.bag.close_station();

        for stack in overflow {
            self.throw_out(stack);
        }

        let mut packets: Vec<Vec<u8>> = changed.into_iter().filter_map(|slot| self.bag.slot_packet(slot)).collect();

        if let Some(station) = trade {
            crate::network::play::release_villager(&self.shared, self.dimension, station.pos.0);
            packets.push(super::trade::trading_player_packet(station.pos.0, 0));
        }

        packets
    }

    fn window_slot_changes(&mut self) -> Vec<Vec<u8>> {
        let Some(window) = &self.window else {
            return Vec::new();
        };

        let (gone, now) = {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

            match window.entity {
                // Вагонетки нет в мире — окно закрывается (как у Java).
                Some(entity) => {
                    let alive = crate::network::play::cart_contents(&self.shared, self.dimension, entity).is_some();

                    (!alive, world.published_carts.get(&entity).cloned())
                }
                None if window.kind == containers::Kind::EnderChest => (Self::window_gone(&world, window), Some(self.ender.clone())),
                None => {
                    let now: Option<Vec<Option<crate::inventory::Stack>>> =
                        window.halves.iter().map(|place| world.published.get(place).cloned()).collect::<Option<Vec<_>>>().map(|parts| parts.concat());

                    (Self::window_gone(&world, window), now)
                }
            }
        };

        if gone {
            return self.close_window_by_server().into_iter().collect();
        }

        // Снимок ещё без этого хранилища (окно открылось после него) —
        // ждём следующего.
        let Some(now) = now.filter(|now| Some(now.len()) == self.bag.window_slots().map(<[_]>::len)) else {
            return Vec::new();
        };
        let (id, name) = (window.id, screens::slot_name(window.kind));

        self.bag.update_window(&now).into_iter().filter_map(|slot| self.bag.window_slot_packet(id, name, slot)).collect()
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

        // Клетка с рамкой у клиента — блок рамки, а не то, что лежит в мире
        // (там воздух).
        let frames: Vec<crate::frame::Frame> = if self.frames_at.is_empty() {
            Vec::new()
        } else {
            self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком").frames().to_vec()
        };

        places
            .into_iter()
            .filter(|&(x, y, z)| block_in_world(self.dimension, (x, y, z)) && self.sent.contains(&(x >> 4, z >> 4)))
            .flat_map(|(x, y, z)| match super::frames::frame_at(&frames, (x, y, z)) {
                Some(frame) => super::frames::show(frame).to_vec(),
                None => block_updates(x, y, z, world.get_block(x, y, z)).to_vec(),
            })
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

        if super::entity_links::seat(&self.shared, self.dimension, self.runtime_id as i32).is_some() {
            if rotation.0.is_finite() && rotation.1.is_finite() { self.rotation = rotation; }
            return;
        }

        let before = chunk_of(self.position);

        // Стоит на земле, если упёрся во что-то по вертикали и не поднимается.
        let was_on_ground = self.on_ground;
        let (dy, run) = ((eyes.1 - self.position.1) as f64, f64::from(eyes.0 - self.position.0).hypot(f64::from(eyes.2 - self.position.2)));

        self.body.moved(*self.shared.ticks.borrow(), (f64::from(eyes.0 - self.position.0), dy, f64::from(eyes.2 - self.position.2)));
        self.on_ground = flags & VERTICAL_COLLISION != 0 && eyes.1 <= self.position.1;
        self.position = eyes;
        self.health.vit.sprinting = flags & SPRINTING != 0;
        self.track_health(dy, run, was_on_ground);

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
            // [Bedrock: потом] Попугаи на плечах пока только сохраняются.
            shoulders: self.saved.as_ref().map(|s| s.shoulders.clone()).unwrap_or([None,None]),
            x: self.position.0 as f64,
            y: (self.position.1 - EYES) as f64,
            z: self.position.2 as f64,
            yaw: self.rotation.0,
            pitch: self.rotation.1,
            game_mode: self.game_mode,
            inventory: self.bag.inventory_with_cursor(),
            dimension: self.dimension,
            vitals: self.health.vit.saved(),
            // Точку возрождения Bedrock-игрок пока не ставит: сохраняем прежнюю.
            respawn: self.body.respawn_point(),
            ender: self.ender.clone(),
        }
    }

    /// Записывает файл игрока, если что-то изменилось с прошлого раза.
    fn save_if_changed(&mut self) {
        self.progress.save(&self.identity.uuid);
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
        let names: Vec<_> = self.bag.inventory.slots().iter().flatten().filter_map(|s| crate::blocks::item_name(s.item)).collect();
        self.progress.scan_items(&names);
        self.progress.recipes.grant_initial();
        self.progress.recipes.scan(&names, names.len(), false);
        self.bag.allowed_crafting = self.shared.limited_crafting.load(std::sync::atomic::Ordering::Relaxed).then(|| (0..crate::crafting::recipes().len()).filter(|i| self.progress.recipes.knows_crafting(*i)).collect());

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
        let (events, weather) = {
            let mut world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
            let (events, count) = world.events_since(self.events_from, self.reader);
            self.events_from = count;
            (events, (world.weather.is_raining(), world.weather.is_thundering()))
        };

        // Погода: у Bedrock не сила по тактам, а начало и конец дождя и грозы.
        if weather != self.weather_shown {
            packets.extend(weather_packets(self.weather_shown, weather));
            self.weather_shown = weather;
        }

        // Места, где сменился блок: хранилищу там и сундукам по соседству —
        // новый блок-актор (сундук стал половиной большого или перестал,
        // когда вторую половину сломали), после самого блока.
        let mut changed = Vec::new();
        let mut under_frames = Vec::new();
        let mut contents = false;
        let mut late_sounds = Vec::new();
        let mut held_piston_sound = None;

        for event in events {
            match event {
                WorldEvent::Change(change)
                    if self.sent.contains(&(change.x >> 4, change.z >> 4)) && heights(self.dimension).contains(&change.y) =>
                {
                    // В клетке рамки у клиента блок рамки — мир там не показываем.
                    if self.frames_at.values().any(|place| *place == (change.x, change.y, change.z)) {
                        under_frames.push((change.x, change.y, change.z));
                        continue;
                    }

                    // Ход поршня клиенту рисуют свои пакеты (pistons.rs): блоки
                    // хода, голову и покой основания отдельно не шлём.
                    if self.piston_moves.iter().any(|(_, mv)| super::pistons::hides(mv, (change.x, change.y, change.z), change.state)) {
                        continue;
                    }

                    // Крышка бочки: BDS шлёт её Block Event'ом перед самим
                    // блоком (снято по сети, tmp/br-bds); открытие своего окна
                    // уже сказало об этом при Container Open.
                    if let Some(open) = screens::barrel_open(change.state) {
                        let place = (change.x, change.y, change.z);

                        if self.lids.insert(place, open).unwrap_or(false) != open {
                            packets.push(screens::lid_packet(place, open));
                        }
                    }

                    packets.extend(block_updates(change.x, change.y, change.z, change.state));

                    // Основание поршня: рычаг рисует блок-актор (pistons.rs).
                    if let Some(base) = super::pistons::base_of(change.state) {
                        packets.push(super::pistons::actor_packet((change.x, change.y, change.z), base));
                    }

                    {
                        let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                        packets.extend(super::block_actors::live(&world, (change.x, change.y, change.z)));
                    }
                    changed.push((change.x, change.y, change.z));
                }
                // Крышка сундука (Block Action 1, параметр — число
                // смотрящих): у Bedrock — Block Event «сменить состояние».
                WorldEvent::Action(action)
                    if action.action == 1
                        && screens::is_chest_block(action.block)
                        && self.sent.contains(&(action.x >> 4, action.z >> 4))
                        && heights(self.dimension).contains(&action.y) =>
                {
                    let (place, open) = ((action.x, action.y, action.z), action.param > 0);

                    if self.lids.insert(place, open) != Some(open) {
                        packets.push(screens::lid_packet(place, open));
                    }
                }
                WorldEvent::BlockEntity { x, y, z, .. }
                    if self.sent.contains(&(x >> 4, z >> 4)) && heights(self.dimension).contains(&y) =>
                {
                    let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
                    packets.extend(super::block_actors::live(&world, (x, y, z)));
                }
                WorldEvent::Contents => contents = true,
                // Взрыв: звук и вспышка (effects.rs), своему игроку — толчок.
                WorldEvent::Explosion(blast) if self.within_earshot(blast.centre.0, blast.centre.2) => {
                    packets.extend(super::effects::explosion_packets(&blast));

                    if let Some((_, (vx, vy, vz))) = blast.pushed.iter().find(|(id, _)| *id == self.runtime_id as i32) {
                        packets.push(super::effects::motion(self.runtime_id, (*vx as f32, *vy as f32, *vz as f32)));
                    }
                }
                // Событие мира (частицы роста костной муки) и звук сервера.
                WorldEvent::Level { x, y, z, effect, data, .. } if self.within_earshot(f64::from(x), f64::from(z)) => {
                    packets.extend(super::effects::level_packets(x, y, z, effect, data));
                }
                // Звук поршня идёт у BDS не в начале хода, а в его конце, вместе с
                // последним состоянием актора: придерживаем до сообщения о ходе.
                WorldEvent::Sound(sound) if is_piston_sound(sound.name) && self.within_earshot(f64::from(sound.x), f64::from(sound.z)) => {
                    held_piston_sound = Some(sound);
                }
                // Ход поршня: актор в пути и блоки хода сейчас, конец и
                // настоящие блоки — на следующих тактах (pistons.rs).
                WorldEvent::Piston(mv)
                    if self.within_earshot(f64::from(mv.base.0), f64::from(mv.base.2)) && heights(self.dimension).contains(&mv.base.1) =>
                {
                    packets.extend(super::pistons::start_packets(&mv));

                    let mut end = Vec::new();

                    // Сперва звук (у BDS «PistonOut» и «PistonIn» на углу блока), потом актор.
                    if let Some(sound) = held_piston_sound.take() {
                        let corner = (f64::from(sound.x), f64::from(sound.y), f64::from(sound.z));

                        end.extend(self.sound_packets(&crate::world::WorldSound { exact: Some(corner), ..sound }));
                    }

                    end.extend(super::pistons::end_packets(&mv));
                    self.piston_later.push((self.ticks + super::pistons::END_AFTER, end));

                    let last = self.ticks + super::pistons::FINAL_AFTER;

                    if !mv.cargo.is_empty() {
                        self.piston_later.push((last, super::pistons::final_packets(&mv)));
                    }

                    self.piston_moves.push((last + 2, mv));
                }
                WorldEvent::Sound(sound) if self.within_earshot(f64::from(sound.x), f64::from(sound.z)) => {
                    // Звук крышки — после самого движения крышки: BDS шлёт сперва
                    // Block Event, потом звук (чёрный ящик, tmp/br-sounds).
                    if super::effects::is_lid_sound(sound.name) {
                        late_sounds.extend(self.sound_packets(&sound));
                    } else {
                        packets.extend(self.sound_packets(&sound));
                    }
                }
                // Чужой игрок ранен: остальные видят «ранен» — боль клиент
                // играет сам (BDS не шлёт звука боли). Причина урона — по
                // виду урона (ActorDamageCause).
                WorldEvent::PlayerDamaged { entity_id, type_id, .. } if entity_id != self.runtime_id as i32 && self.shown.contains_key(&entity_id) => {
                    packets.push(health::actor_event_data(entity_id as u64, 2, health::damage_cause_of_type(type_id)));
                }
                // Ракета фейерверка взорвалась: частицы (событие 25).
                WorldEvent::RocketBlast { entity_id } if self.items_shown.contains(&entity_id) => {
                    packets.push(health::actor_event(entity_id as u64, 25));
                }
                // Взрыв ранит и Bedrock-игрока; толчка ему пока нет [проверить].
                WorldEvent::Hurt { entity_id, amount, .. } if entity_id == self.runtime_id as i32 => {
                    self.health.hurt(amount, crate::survival::Source::Explosion);
                }
                // Удар моба или стрелы: урон — здоровью, толчок — клиенту
                // (как у Java: 0,218 вдоль удара и 0,275 вверх с земли).
                WorldEvent::MobHurt { entity_id, amount, source, attacker, killer, push, at, enchantments } if entity_id == self.runtime_id as i32 => {
                    let done = self.run(crate::network::play::Act::MobHit { amount, source, attacker, killer, push, at, enchantments });
                    self.settle(done, &mut packets);
                }
                // Удар другого игрока (или стрела игрока): щит, броня, отдача —
                // тем же кодом, что у Java (network/play/body.rs).
                WorldEvent::PlayerStrike(strike) if strike.victim == self.runtime_id as i32 => {
                    let done = self.run(crate::network::play::Act::Struck(&strike));

                    self.settle(done, &mut packets);
                }
                WorldEvent::Particle { kind, block, at, skip, .. } if skip != self.runtime_id as i32 && self.within_earshot(at.0, at.2) => {
                    if kind == 41 {
                        packets.push(super::effects::soul_particle(bedrock_dimension(self.dimension) as u8, at));
                    } else {
                        packets.extend(super::effects::combat_particles(kind, block, at));
                    }
                }
                WorldEvent::EntitySound { entity_id, name, volume: _, pitch } => {
                    let at = if entity_id == self.runtime_id as i32 { Some((self.position.0, self.position.1 - 1.62, self.position.2)) }
                        else if let Some(shown) = self.shown.get(&entity_id) { Some((shown.position.x as f32, shown.position.y as f32, shown.position.z as f32)) }
                        else { self.mobs_at.get(&entity_id).map(|p| (p[0], p[1], p[2])) };
                    if let Some(at) = at { packets.extend(super::effects::sound_packets(name, pitch, at, None)); }
                }
                WorldEvent::Totem { entity_id } if entity_id != self.runtime_id as i32 && self.shown.contains_key(&entity_id) => {
                    packets.push(health::actor_event(entity_id as u64, 65));
                }
                WorldEvent::Animation { entity_id, kind } if self.shown.contains_key(&entity_id) || self.items_shown.contains(&entity_id) || entity_id == self.runtime_id as i32 => {
                    packets.push(super::player_state::animation(entity_id as u64, kind));
                }
                WorldEvent::MobHitEffect { entity_id, effect } if entity_id == self.runtime_id as i32 && self.body.mob_hit_landed() => self.health.vit.give_effect_shown(effect),
                WorldEvent::MobEffect { entity_id, effect } if entity_id == self.runtime_id as i32 => self.health.vit.give_effect_shown(effect),
                WorldEvent::PotionEffect { entity_id, effect } if entity_id == self.runtime_id as i32 => self.health.vit.give_effect_shown(effect),
                WorldEvent::PotionHeal { entity_id, amount } if entity_id == self.runtime_id as i32 => self.health.vit.heal(amount),
                _ => {}
            }
        }

        // Звук поршня, о ходе которого сообщения не было, — как есть.
        if let Some(sound) = held_piston_sound {
            packets.extend(self.sound_packets(&sound));
        }

        // Пакеты хода поршня, чей такт пришёл.
        if !self.piston_later.is_empty() || !self.piston_moves.is_empty() {
            let now = self.ticks;

            for (_, due) in self.piston_later.extract_if(.., |(when, _)| *when <= now) {
                packets.extend(due);
            }

            self.piston_moves.retain(|(until, _)| *until >= now);
        }

        packets.extend(late_sounds);

        if !under_frames.is_empty() {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");

            packets.extend(self.restore(&world, &under_frames));
        }

        if !changed.is_empty() {
            let world = self.shared.world_in(self.dimension).lock().expect("мир захвачен другим потоком");
            let mut actors = Vec::new();

            for (x, y, z) in changed {
                let around = [(x, y, z), (x + 1, y, z), (x - 1, y, z), (x, y, z + 1), (x, y, z - 1)];

                for (index, place) in around.into_iter().enumerate() {
                    let kind = containers::Kind::of(world.get_block(place.0, place.1, place.2));
                    let wanted = match kind {
                        Some(containers::Kind::Chest | containers::Kind::TrappedChest | containers::Kind::CopperChest) => true,
                        Some(_) => index == 0,
                        None => false,
                    };

                    if wanted && self.sent.contains(&(place.0 >> 4, place.2 >> 4)) && !actors.contains(&place) {
                        actors.push(place);
                    }
                }
            }

            // Как у BDS: после смены блока — только то, что шлёт он (бочка,
            // сундук, крафтер); остальные блок-акторы клиент заводит сам.
            packets.extend(actors.into_iter().filter_map(|place| screens::live_actor_at(&world, place)));
        }

        // Ещё один полный Inventory Content окна — после блока и актора, как у BDS.
        if std::mem::take(&mut self.resync_window)
            && let Some(window) = &self.window
            && let Some(packet) = self.bag.window_full_packet(window.id)
        {
            packets.push(packet);
        }

        if contents {
            packets.extend(self.window_changes());
        }

        self.tick_body(&mut packets);
        self.tick_health(&mut packets);
        self.tick_maps(&mut packets);
        self.tick_brush(&mut packets);
        self.tick_actors(&mut packets);

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
                            combat_shown: Vec::new(),
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

        // Чужой игрок погиб: остальным — «умер» (BDS шлёт death_animation
        // всем, кто видит); поднялся — снова может умереть.
        {
            let players = self.shared.players.lock().expect("список игроков захвачен другим потоком");

            for (id, shown) in &mut self.shown {
                let gear = players.gear(&shown.uuid);
                let dead = gear.health <= 0.0;
                let now = *self.shared.ticks.borrow();
                let spectator = players.members().iter().any(|m| m.uuid == shown.uuid && m.game_mode == crate::commands::GAME_MODE_SPECTATOR);
                let flags = super::player_state::Flags {
                    using: gear.living & 1 != 0, sneaking: shown.sneaking, sprinting: gear.sprinting,
                    blocking: gear.view.blocking_since.is_some_and(|since| now >= since + u64::from(crate::combat::SHIELD_DELAY)) && now >= gear.view.shield_disabled_until,
                    spinning: gear.living & crate::players::SPIN_POSE_BIT != 0, spectator, ..Default::default()
                };
                let state = vec![super::player_state::flags(*id as u64, flags), super::player_state::armor(*id as u64, &gear.worn),
                    super::player_state::offhand(*id as u64, gear.worn[0]), super::player_state::environment(*id as u64, gear.air, 0)];
                if state != shown.combat_shown { packets.extend(state.iter().cloned()); shown.combat_shown = state; }

                if dead && self.dead_shown.insert(*id) {
                    packets.push(health::death_event(*id as u64));
                } else if !dead {
                    self.dead_shown.remove(id);
                }
            }
        }

        let gone: Vec<i32> = self.shown.keys().copied().filter(|id| !present.contains(id)).collect();

        for id in gone {
            self.dead_shown.remove(&id);

            if let Some(shown) = self.shown.remove(&id) {
                packets.extend(forget_player(id, &shown));
            }
        }
    }

    /// Уход из мира.
    pub fn leave(mut self) {
        self.body.dismount(&self.shared, self.dimension);
        log_info!("{} (Bedrock) вышел", self.identity.name);

        // Открытое хранилище теряет смотрящего.
        self.close_window();
        self.close_station();

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

    /// Блок-акторы хранилищ чанка — пакетами Block Actor Data вслед за
    /// Level Chunk: так клиент рисует сундуки и спаривает половины. Если
    /// вторая половина большого сундука — в соседнем, уже отправленном чанке,
    /// ей актор уходит заново: пара там видна только теперь.
    /// Блок-акторы хранилищ чанка: свои — NBT для самого чанка (BDS кладёт
    /// их в полезную нагрузку Level Chunk, отдельных Block Actor Data при
    /// входе не шлёт — снято по сети, tmp/br-bds), а половины больших
    /// сундуков в уже отправленных соседних чанках — пакетами.
    fn chunk_actors(&self, world: &world::World, chunk_x: i32, chunk_z: i32, sections: &[Option<Arc<[u16]>>]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let embedded = super::block_actors::chunk(world, chunk_x, chunk_z, sections, heights(self.dimension));
        let mut packets = Vec::new();

        for (x, y, z, _) in world.chunk_containers(chunk_x, chunk_z) {
            let place = (chunk_x * 16 + x, y, chunk_z * 16 + z);

            if !heights(self.dimension).contains(&y) {
                continue;
            }

            for (hx, hy, hz) in containers::halves(world, place) {
                let other_chunk = (hx >> 4, hz >> 4);

                if other_chunk != (chunk_x, chunk_z) && self.sent.contains(&other_chunk) {
                    packets.extend(screens::actor_at(world, (hx, hy, hz)));
                }
            }
        }
        (embedded, packets)
    }

    /// Рамки, что висят в этом чанке (в пределах высоты мира клиента): их
    /// блоки уходят следом за чанком (bedrock/frames.rs). Берётся замок
    /// предметов под замком мира — как везде.
    fn chunk_frames(&self, chunk_x: i32, chunk_z: i32) -> Vec<crate::frame::Frame> {
        let items = self.shared.items_in(self.dimension).lock().expect("предметы захвачены другим потоком");

        items
            .frames()
            .iter()
            .filter(|frame| frame.pos.0 >> 4 == chunk_x && frame.pos.2 >> 4 == chunk_z && heights(self.dimension).contains(&frame.pos.1))
            .cloned()
            .collect()
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

        // Крышки в забытых чанках клиент при новой отправке увидит
        // закрытыми — помнить их незачем.
        let sent = &self.sent;
        self.lids.retain(|(x, _, z), _| sent.contains(&(x >> 4, z >> 4)));
        self.frames_at.retain(|_, (x, _, z)| sent.contains(&(*x >> 4, *z >> 4)));

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
                    Some(parts) => {
                        let actors = self.chunk_actors(&world, x, z, &parts.0);
                        ready.push((x, z, parts, actors, self.chunk_frames(x, z)));
                    }
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

        for (x, z, (sections, biomes), (mut embedded, actors), frames) in ready {
            // Поршни тоже: актор PistonArm, пришедший пакетом после чанка,
            // клиент после перезахода не принимает — головы не видно.
            let pistons = super::pistons::chunk_actors(x, z, &sections, world::MIN_Y / 16, |y| heights(dimension).contains(&y));
            embedded.extend(pistons.iter().map(|packet| actor_nbt(packet)));

            batch.push(encode_chunk(dimension, x, z, &sections, &biomes, &embedded));
            batch.extend(actors);

            // Рамки: блоки поверх воздуха чанка, следом их данные.
            for frame in frames {
                if frame.painting.is_some() { self.items_shown.insert(frame.entity_id); } else {self.frames_at.insert(frame.entity_id, frame.pos);}
                batch.extend(super::frames::show(&frame));
            }

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

/// Подожжённый динамит для показа: номер, место, скорость, запал.
type PrimedShown = (i32, (f32, f32, f32), (f32, f32, f32), i32);

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

/// Звук хода поршня: у BDS он идёт в конце хода (pistons.rs).
fn is_piston_sound(name: &str) -> bool {
    matches!(name, "minecraft:block.piston.extend" | "minecraft:block.piston.contract")
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
fn block_updates(x: i32, y: i32, z: i32, state: i32) -> Vec<Vec<u8>> {
    let mut update = Out::packet(session::UPDATE_BLOCK);
    update
        .block_position(x, y, z)
        .varint(bedrock_state(state))
        .varint(UPDATE_NEIGHBORS | UPDATE_NETWORK) // как у BDS: соседям и по сети
        .varint(0); // основной слой

    // У хранилищ (кроме сундуков) слоя воды нет: BDS шлёт им один слой.
    if containers::Kind::of(state).is_some_and(|kind| !kind.is_chest())
        && crate::blocks::orientation_of(state).and_then(|o| o.value(state, "waterlogged")).is_none() {
        return vec![update.bytes];
    }

    // И слой воды: затоплен ли теперь этот блок.
    let mut liquid = Out::packet(session::UPDATE_BLOCK);
    liquid
        .block_position(x, y, z)
        .varint(if waterlogged(state) { still_water() } else { air() })
        .varint(UPDATE_NEIGHBORS | UPDATE_NETWORK)
        .varint(1);

    vec![update.bytes, liquid.bytes]
}

/// Флаги Update Block (UpdateBlockFlags): «соседям» и «по сети». У BDS на
/// каждое изменение стоят оба (снято по сети, tmp/br-bds).
const UPDATE_NEIGHBORS: u32 = 1;
const UPDATE_NETWORK: u32 = 2;

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
    matches!(command.split_whitespace().next().unwrap_or(""), "help" | "list" | "say" | "me" | "msg" | "tell" | "w")
}

/// Пакет Level Chunk со всеми подчанками и биомами измерения.
///
/// Поля по документации Mojang к r/26_u1 (dot/LevelChunkPacket.dot):
/// положение чанка, Dimension Id (число со знаком), число подчанков, кэш
/// и сами данные. `sections` и `biomes` — во всю нашу высоту −64..320;
/// клиенту уходят только подчанки его измерения (`sub_chunks`).
fn encode_chunk(dimension: Dimension, x: i32, z: i32, sections: &[Option<Arc<[u16]>>], biomes: &[u16], block_entities: &[Vec<u8>]) -> Vec<u8> {
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
                        Dimension::End => THE_END,
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

    // Блок-акторы чанка — сетевые NBT подряд, как у BDS.
    for nbt in block_entities {
        payload.raw(nbt);
    }

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

/// NBT из пакета Block Actor Data: без номера пакета и места блока.
fn actor_nbt(packet: &[u8]) -> Vec<u8> {
    let mut reader = In::new(packet);
    let start = (|| {
        reader.varint()?;
        reader.zigzag32()?;
        reader.zigzag32()?;
        reader.zigzag32()?;
        Some(packet.len() - reader.remaining())
    })();

    start.map_or_else(Vec::new, |start| packet[start..].to_vec())
}

/// Поля, которые BDS кладёт в блок-актор хранилища внутри чанка сверх
/// Block Actor Data (снято по сети, tmp/br-bds): пустой Items (тип
/// элементов — конец, длина 0), у сундука и бочки Findable, у воронки
/// TransferCooldown, у крафтера disabled_slots.
#[cfg(test)]
fn stored_fields(kind: containers::Kind, mut nbt: Vec<u8>) -> Vec<u8> {
    use containers::Kind as K;

    if matches!(kind, K::Jukebox | K::Lectern | K::Shelf | K::ChiseledBookshelf) || nbt.last() != Some(&0) {
        return nbt;
    }

    nbt.pop();

    let mut out = Out::default();
    out.u8(9).string("Items").u8(0).zigzag32(0);

    match kind {
        K::Chest | K::TrappedChest | K::CopperChest | K::Barrel => {
            out.u8(1).string("Findable").u8(0);
        }
        K::Hopper => {
            out.u8(3).string("TransferCooldown").zigzag32(0);
        }
        K::Crafter => {
            out.u8(2).string("disabled_slots").li16(0);
        }
        _ => {}
    }

    out.u8(0);
    nbt.extend(out.bytes);
    nbt
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
/// Строка из сетевого NBT-компонента Java, если это простой текст (тег 8, длина u16, байты).
fn plain_component(component: &[u8]) -> Option<String> {
    if component.first() != Some(&8) || component.len() < 3 {
        return None;
    }

    let length = usize::from(u16::from_be_bytes([component[1], component[2]]));

    std::str::from_utf8(component.get(3..3 + length)?).ok().map(str::to_owned)
}

fn raw_text(line: &str) -> Vec<u8> {
    let line = crate::colors::bedrock(line);
    let mut text = Out::packet(session::TEXT);
    text.bool(false).u8(0).u8(0).string(&line).string("").string("").bool(false);
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
    let mut lying: Vec<(i32, Vec<u8>)> = items
        .items()
        .iter()
        .filter(|item| item.block.is_none())
        .filter_map(|item| Some((item.entity_id, add_item_entity(item)?)))
        .collect();

    lying.extend(items.carts().iter().map(|cart| (cart.entity_id, add_cart(cart))));

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

/// Add Actor и Move Actor Absolute (описание протокола 1.26.10:
/// `add_entity` — 13, `move_entity` — 18).
const ADD_ACTOR: u32 = 0x0d;
const MOVE_ACTOR_ABSOLUTE: u32 = 0x12;

/// Место и поворот вагонетки для клиента Bedrock. [проверить]: Bedrock может
/// ждать вагонетку не на уровне низа ящика, а выше — живой клиент.
///
/// [проверить] Постановка вагонетки предметом на рельсу; удары уже проходят
/// общий код, hurt_time и hurt_dir передаются через vehicle_data.
type CartShown = (i32, (f32, f32, f32, f32), Vec<(u32, super::mobdata::Value)>);

fn cart_place(cart: &crate::minecart::Minecart) -> (f32, f32, f32, f32) {
    (cart.x as f32, cart.y as f32, cart.z as f32, cart.yaw)
}

/// Вагонетка появилась (Add Actor): вид сущности по имени Bedrock —
/// `minecraft:minecart`, `minecraft:chest_minecart` и т. д.
fn add_cart(cart: &crate::minecart::Minecart) -> Vec<u8> {
    let (x, y, z, yaw) = cart_place(cart);
    let name = super::vehicle_data::identifier(cart.kind);

    let mut out = Out::packet(ADD_ACTOR);
    out.zigzag64(cart.entity_id as i64)
        .varint64(cart.entity_id as u64)
        .string(name)
        .vec3f(x, y, z)
        .vec3f(cart.vx as f32, cart.vy as f32, cart.vz as f32)
        .lf32(cart.pitch)
        .lf32(yaw)
        .lf32(yaw)
        .lf32(yaw)
        .varint(0); // свойства-атрибуты
    super::mobdata::write(&mut out, &super::vehicle_data::entries(cart));
    out.varint(0).varint(0).varint(0); // свойства и связи
    out.bytes
}

/// Моб появился (Add Actor): имя вида Bedrock, место, поворот тела и головы.
fn add_mob(entity_id: i32, look: &super::mobdata::Look, [x, y, z, yaw, pitch, head]: [f32; 6]) -> Vec<u8> {
    let kind = look.kind;
    let mut out = Out::packet(ADD_ACTOR);
    out.zigzag64(i64::from(entity_id))
        .varint64(entity_id as u64)
        .string(super::entity_state::identifier(kind))
        .vec3f(x, y, z)
        .vec3f(0.0, 0.0, 0.0)
        .lf32(pitch)
        .lf32(yaw)
        .lf32(head)
        .lf32(yaw)
        .varint(0); // свойства-атрибуты
    super::mobdata::write(&mut out, &super::mobdata::entries(look)); // данные сущности
    super::mobprops::write_values(&mut out, kind); // свойства: целые, дробные
    out.varint(0); // связи
    out.bytes
}

/// Моб сдвинулся (Move Actor Absolute): флаги 0, углы — байтами.
fn move_mob(entity_id: i32, [x, y, z, yaw, pitch, head]: [f32; 6]) -> Vec<u8> {
    let angle = |degrees: f32| ((degrees / 360.0 * 256.0).round() as i32 & 0xFF) as u8;
    let mut out = Out::packet(MOVE_ACTOR_ABSOLUTE);

    out.varint64(entity_id as u64).u8(0).vec3f(x, y, z).u8(angle(pitch)).u8(angle(yaw)).u8(angle(head));
    out.bytes
}

/// Level Event (Mojang bedrock-protocol-docs, LevelEventPacket).
const LEVEL_EVENT: u32 = 0x19;

/// Погода в Level Event (minecraft-data, bedrock protocol.json:
/// 3001 start_rain, 3002 start_thunder, 3003 stop_rain, 3004 stop_thunder).
/// Данные — сила, 65 535 — полная [проверить].
fn weather_packets((rain, thunder): (bool, bool), (now_rain, now_thunder): (bool, bool)) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();

    for (was, now, start, stop) in [(rain, now_rain, 3001, 3003), (thunder, now_thunder, 3002, 3004)] {
        if was != now {
            let mut out = Out::packet(LEVEL_EVENT);
            out.zigzag32(if now { start } else { stop })
                .vec3f(0.0, 0.0, 0.0)
                .zigzag32(if now { 65_535 } else { 0 });
            packets.push(out.bytes);
        }
    }

    packets
}

/// Молния (Add Actor `minecraft:lightning_bolt`): без скорости, поворота
/// и свойств. [проверить]: играет ли клиент Bedrock гром сам, как Java.
fn add_lightning(bolt: &crate::lightning::Bolt) -> Vec<u8> {
    let mut out = Out::packet(ADD_ACTOR);
    out.zigzag64(bolt.entity_id as i64)
        .varint64(bolt.entity_id as u64)
        .string("minecraft:lightning_bolt")
        .vec3f(bolt.x as f32, bolt.y as f32, bolt.z as f32)
        .vec3f(0.0, 0.0, 0.0)
        .lf32(0.0)
        .lf32(0.0)
        .lf32(0.0)
        .lf32(0.0)
        .varint(0) // свойства-атрибуты
        .varint(0) // данные сущности
        .varint(0)
        .varint(0) // свойства: целые, дробные
        .varint(0); // связи
    out.bytes
}

/// Вагонетка сдвинулась (Move Actor Absolute): флаги 0 — просто сдвиг,
/// углы — байтами по 256 на оборот.
fn move_cart(entity_id: i32, (x, y, z, yaw): (f32, f32, f32, f32)) -> Vec<u8> {
    let angle = |degrees: f32| ((degrees / 360.0 * 256.0).round() as i32 & 0xFF) as u8;
    let mut out = Out::packet(MOVE_ACTOR_ABSOLUTE);

    out.varint64(entity_id as u64).u8(0).vec3f(x, y, z).u8(angle(yaw)).u8(0).u8(angle(yaw));
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
const COMMANDS: [(&str, &str); 61] = [
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
    ("worldborder", "Рамка мира"),
    ("tick", "Скорость такта"),
    ("clone", "Скопировать блоки"),
    ("attribute", "Атрибуты существ"),
    ("enchant", "Зачаровать предмет в руке"),
    ("locate", "Найти постройку или биом"),
    ("forceload", "Удерживать чанки"),
    ("spreadplayers", "Раскидать игроков"),
    ("random", "Случайное число"),
    ("ride", "Посадить на сущность"),
    ("rotate", "Повернуть сущность"),
    ("spectate", "Смотреть глазами сущности"),
    ("fillbiome", "Сменить биом"),
    ("place", "Поставить постройку"),
    ("loot", "Добыча"),
    ("scoreboard", "Табло: цели и счёт"),
    ("team", "Команды игроков"),
    ("tag", "Метки сущностей"),
    ("bossbar", "Полосы босса"),
    ("trigger", "Сработать триггер"),
    ("teammsg", "Сообщение своей команде"),
    ("function", "Выполнить функцию"),
    ("schedule", "Запланировать функцию"),
    ("datapack", "Наборы данных"),
    ("return", "Итог функции"),
    ("execute", "Выполнить с условиями"),
    ("me", "Сообщение о действии"),
    ("msg", "Личное сообщение"),
    ("tell", "Личное сообщение"),
    ("w", "Личное сообщение"),
    ("tellraw", "Сообщение с оформлением"),
    ("title", "Надпись на экране"),
    ("playsound", "Проиграть звук"),
    ("stopsound", "Остановить звук"),
    ("particle", "Показать частицы"),
    ("setworldspawn", "Точка появления мира"),
    ("spawnpoint", "Точка возрождения игрока"),
    ("defaultgamemode", "Режим игры по умолчанию"),
    ("seed", "Семя мира"),
    ("version", "Версия сервера"),
    ("reload", "Перезагрузить данные"),
    ("ban", "Заблокировать игрока"),
    ("banlist", "Список блокировок"),
    ("pardon", "Снять блокировку игрока"),
    ("whitelist", "Белый список"),
    ("setidletimeout", "Выгон за бездействие"),
    ("transfer", "Перевести на другой сервер"),
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
///
/// `tnt` (особый динамит) — только тем, кому он позволен (`may_tnt`), с
/// видами в описании. Перечисление видов отдельным параметром требует
/// символа разбора «перечисление», которого документация Mojang не
/// называет, — [проверить], пока текстом.
fn available_commands(may_tnt: bool) -> Vec<u8> {
    let tnt = format!(
        "Особый динамит: tnt give <игрок> <{}> [сколько] | tnt list",
        crate::custom_tnt::names().collect::<Vec<&str>>().join("|")
    );
    let mut commands: Vec<(&str, &str)> = COMMANDS.to_vec();

    if may_tnt {
        commands.push(("tnt", &tnt));
    }

    let mut out = Out::packet(session::AVAILABLE_COMMANDS);
    out.varint(0) // Enum Values
        .varint(0) // Chained Subcommand Values
        .varint(0) // Post Fixes
        .varint(0) // Enum Data
        .varint(0) // Chained Subcommand Data
        .varint(commands.len() as u32);

    for (name, description) in commands {
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
    combat_shown: Vec<Vec<u8>>,
}

/// Начал ломать (из Player Auth Input): ломает только в творческом режиме.
const START_BREAK: u32 = 100;

/// Виды действия предметом (TransactionUseItem): щелчок по блоку и поломка.
const CLICK: u32 = 0;
const CLICK_AIR: u32 = 1;
const BREAK: u32 = 2;

/// Отчего действие (trigger_type у TransactionUseItem): 2 — «simulation
/// tick», повтор, пока кнопка зажата.
const SIMULATION_TICK: u32 = 2;

/// Что игрок сделал предметом: сломал блок, щёлкнул по блоку.
struct ItemUse {
    kind: u32,
    /// Отчего: щелчок игрока или повтор (`SIMULATION_TICK`).
    trigger: u32,
    position: (i32, i32, i32),
    face: i32,
    /// Предмет в руке; None — рука пуста.
    held: Option<Held>,
    /// Высота точки щелчка внутри блока (0 — низ, 1 — верх).
    click_y: f32,
    /// Точка щелчка внутри блока целиком (доли грани).
    click: (f32, f32, f32),
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
pub(super) fn offset((x, y, z): (i32, i32, i32), face: i32) -> (i32, i32, i32) {
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
    vehicle: Option<(i32, (f32, f32))>,
    actions: Vec<ItemUse>,
    request: Option<Vec<u8>>,
}

/// Действия с блоками из остатка Player Auth Input (после позиции):
/// ломание приходит здесь, когда ломание проверяет сервер.
fn read_input_actions(reader: &mut In) -> Input {
    const ITEM_INTERACT: u128 = 1 << 34;
    const BLOCK_ACTION: u128 = 1 << 35;
    const ITEM_STACK_REQUEST: u128 = 1 << 36;
    const PREDICTED_VEHICLE: u128 = 1 << 45;

    let mut found = Vec::new();
    let mut request = None;
    let mut flags = 0u128;
    let mut vehicle = None;

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
            request = Some(Bag::read_embedded(reader)?);
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
                        26 => found.push(ItemUse { kind: BREAK, trigger: 0, position, face, held: None, click_y: 0.0, click: (0.0, 0.0, 0.0) }),
                        0 => found.push(ItemUse { kind: START_BREAK, trigger: 0, position, face, held: None, click_y: 0.0, click: (0.0, 0.0, 0.0) }),
                        _ => {}
                    }
                }
            }
        }

        // Транспорт — после действий с блоками (документация Mojang 944).
        if flags & PREDICTED_VEHICLE != 0 {
            let (pitch, yaw) = (reader.lf32()?, reader.lf32()?);
            let id = i32::try_from(reader.zigzag64()?).ok()?;
            vehicle = Some((id, (yaw, pitch)));
        }

        Some(())
    };

    read();
    Input { flags, vehicle, actions: found, request }
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

/// Из Inventory Transaction — действие по сущности (тип 3,
/// ItemUseOnEntity): номер сущности у клиента и что сделано (0 — щелчок,
/// 1 — удар). Дальше — ячейка, предмет, место игрока и щелчка: не нужны.
fn read_entity_use(payload: &[u8]) -> Option<(u64, u32)> {
    const USE_ON_ENTITY: u32 = 3;

    let mut reader = In::new(payload);
    skip_legacy(&mut reader)?;
    let transaction_type = reader.varint()?;
    skip_actions(&mut reader)?;

    if transaction_type != USE_ON_ENTITY {
        return None;
    }

    let target = reader.varint64()?;
    let action = reader.varint()?;

    Some((target, action))
}

/// Inventory Transaction «предмет отпущен» (тип 4, ItemRelease): лук спущен,
/// еда доедена.
fn is_release(payload: &[u8]) -> bool {
    const ITEM_RELEASE: u32 = 4;

    let mut reader = In::new(payload);

    skip_legacy(&mut reader).is_some() && reader.varint() == Some(ITEM_RELEASE)
}

/// Set Actor Motion: скорость сущности (блоков за такт).
fn motion(runtime_id: u64, (x, y, z): (f32, f32, f32)) -> Vec<u8> {
    let mut out = Out::packet(0x28);

    out.varint64(runtime_id).vec3f(x, y, z).varint64(0); // такт — 0: не по тактам клиента
    out.bytes
}

/// Тело «предмет применён»: что сделано, где, чем.
fn read_use(reader: &mut In) -> Option<ItemUse> {
    let kind = reader.varint()?;
    let trigger = reader.varint()?;
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

    Some(ItemUse { kind, trigger, position, face, held, click_y: click.1, click })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_weather_and_lightning_packets_decode() {
        assert!(weather_packets((true,false),(true,false)).is_empty());
        for (before,after,ids,strength) in [((false,false),(true,true),[3001,3002],65535),((true,true),(false,false),[3003,3004],0)] {
            for (packet,id) in weather_packets(before,after).iter().zip(ids) {
                let mut r=In::new(packet);
                assert_eq!(r.varint(),Some(LEVEL_EVENT)); assert_eq!(r.zigzag32(),Some(id));
                assert_eq!(r.take(12),Some([0;12].as_slice())); assert_eq!(r.zigzag32(),Some(strength)); assert_eq!(r.remaining(),0);
            }
        }
        let bytes=add_lightning(&crate::lightning::Bolt::new(123,(1.0,70.0,-2.0)));
        let mut r=In::new(&bytes);
        assert_eq!(r.varint(),Some(ADD_ACTOR)); assert_eq!(r.zigzag64(),Some(123)); assert_eq!(r.varint64(),Some(123));
        assert_eq!(r.string().as_deref(),Some("minecraft:lightning_bolt"));
        assert_eq!((r.lf32(),r.lf32(),r.lf32()),(Some(1.0),Some(70.0),Some(-2.0)));
        assert_eq!(r.take(28),Some([0;28].as_slice()));
        for _ in 0..5 { assert_eq!(r.varint(),Some(0)); }
        assert_eq!(r.remaining(),0);
    }

    /// Предмет в руке: камень с номером блока.
    fn stone(out: &mut Out) {
        out.zigzag32(1).lu16(1).varint(0).u8(0).zigzag32(2533);
        out.varint(10).lu16(0).li32(0).li32(0);
    }

    /// Set Actor Motion: номер, скорость и такт (протокол 944 добавил такт
    /// в конец — без него клиент падает на разборе).
    #[test]
    fn motion_has_tick_at_the_end() {
        let packet = motion(5, (1.0, 2.0, 3.0));
        let mut reader = In::new(&packet[1..]);

        assert_eq!(packet[0], 0x28);
        assert_eq!(reader.varint64(), Some(5));
        assert_eq!((reader.lf32(), reader.lf32(), reader.lf32()), (Some(1.0), Some(2.0), Some(3.0)));
        assert_eq!(reader.varint64(), Some(0));
    }

    /// Отпускание предмета — Inventory Transaction типа 4; щелчок по блоку и
    /// удар по сущности — нет.
    #[test]
    fn release_is_transaction_four() {
        let make = |kind: u32| {
            let mut out = Out::packet(session::INVENTORY_TRANSACTION);

            out.zigzag32(0).varint(kind).varint(0);
            out.bytes[1..].to_vec()
        };

        assert!(is_release(&make(4)));
        assert!(!is_release(&make(2)));
        assert!(!is_release(&make(3)));
    }

    /// Хвост «предмет применён» после предмета в руке.
    fn use_tail(out: &mut Out) {
        out.vec3f(0.0, 0.0, 0.0).vec3f(0.5, 1.0, 0.5).varint(0).varint(1).u8(0);
    }

    /// Моб для клиента Bedrock появляется под своим именем вида и
    /// сдвигается переносом; ушедший убирается.
    #[test]
    fn a_mob_is_shown_by_its_bedrock_name_and_moved() {
        let place = [1.5, 64.0, -2.5, 90.0, 10.0, 90.0];

        for kind in crate::mob::Kind::ALL {
            let packet = add_mob(7, &crate::bedrock::mobdata::Look::of(&crate::mob::Mob::new(7, kind, (0.0, 0.0, 0.0))), place);
            let text = String::from_utf8_lossy(&packet);

            assert!(text.contains(super::super::entity_state::identifier(kind)), "{}", kind.name());
            let mut r = In::new(&packet);
            assert_eq!((r.varint(), r.zigzag64(), r.varint64()), (Some(0x0d), Some(7), Some(7)));
            assert_eq!(r.string().as_deref(), Some(super::super::entity_state::identifier(kind)));
            for f in [1.5, 64.0, -2.5, 0.0, 0.0, 0.0, 10.0, 90.0, 90.0, 90.0] { assert_eq!(r.lf32(), Some(f)); }
            assert_eq!(r.varint(), Some(0));
            let look = super::super::mobdata::Look::of(&crate::mob::Mob::new(7, kind, (0.0, 0.0, 0.0)));
            assert_eq!(super::super::mobdata::read_test_entries(&mut r), super::super::mobdata::entries(&look));
            let count = r.varint().unwrap();
            for _ in 0..count { r.varint().unwrap(); r.zigzag32().unwrap(); }
            assert_eq!((r.varint(), r.varint(), r.remaining()), (Some(0), Some(0), 0));
        }

        let moved = move_mob(7, place);

        assert!(!moved.is_empty());
        assert_ne!(moved, move_mob(7, [1.5, 65.0, -2.5, 90.0, 10.0, 90.0]));
    }

    #[test]
    fn every_vehicle_spawn_round_trips_including_boat_variant() {
        for kind in crate::minecart::Kind::ALL.into_iter().chain(crate::minecart::boat_kinds()) {
            let cart = crate::minecart::Minecart::new(301, kind, (1.0, 64.0, 2.0));
            let p = add_cart(&cart);
            let mut r = In::new(&p);
            assert_eq!((r.varint(), r.zigzag64(), r.varint64()), (Some(13), Some(301), Some(301)));
            assert!(r.string().unwrap().starts_with("minecraft:"));
            for f in [1.0, 64.0, 2.0, cart.vx as f32, cart.vy as f32, cart.vz as f32, cart.pitch, cart.yaw, cart.yaw, cart.yaw] { assert_eq!(r.lf32(), Some(f)); }
            assert_eq!(r.varint(), Some(0));
            assert_eq!(super::super::mobdata::read_test_entries(&mut r), super::super::vehicle_data::entries(&cart));
            assert_eq!((r.varint(), r.varint(), r.varint(), r.remaining()), (Some(0), Some(0), Some(0), 0));
        }
    }

    #[test]
    fn vehicle_input_uses_signed_unique_id_after_block_actions() {
        let mut out = Out::default();
        out.vec3f(0.0, 0.0, 0.0).varint64((1 << 45) | (1 << 35));
        out.varint(1).varint(0).zigzag32(1).lf32(0.0).lf32(0.0).varint64(7).vec3f(0.0, 0.0, 0.0);
        out.zigzag32(1).zigzag32(26).zigzag32(3).zigzag32(64).zigzag32(-4).zigzag32(1);
        out.lf32(10.0).lf32(90.0).zigzag64(301);
        let input = read_input_actions(&mut In::new(&out.bytes));
        assert_eq!(input.vehicle, Some((301, (90.0, 10.0))));
        assert_eq!(input.actions.len(), 1);
        assert_eq!(input.actions[0].position, (3, 64, -4));
        out.bytes.pop();
        let input = read_input_actions(&mut In::new(&out.bytes));
        assert_eq!(input.vehicle, None);
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
    fn hopper_toggle_bit_means_powered_like_bds() {
        let toggle = |text: &str| {
            let rid = bedrock_state(crate::blocks::state_from_text(text).expect("состояние воронки"));
            let mut out = Out::default();
            out.u8(10).string("");
            crate::bedrock::pistons::write_block(&mut out, rid);
            let tag = crate::bedrock::nbt::decode(&out.bytes, crate::bedrock::nbt::Style::Network).expect("NBT блока");
            matches!(tag.get("states").and_then(|states| states.get("toggle_bit")), Some(crate::bedrock::nbt::Tag::Byte(1)))
        };

        assert!(toggle("hopper[facing=down,enabled=false]"), "запитанная воронка — toggle_bit=1");
        assert!(!toggle("hopper[facing=down,enabled=true]"), "обычная воронка — toggle_bit=0");
        assert!(!toggle("hopper[facing=north,enabled=true]"));
    }

    #[test]
    fn plain_component_reads_string_components() {
        assert_eq!(plain_component(&crate::network::nbt::text_component("Привет")).as_deref(), Some("Привет"));
        assert_eq!(plain_component(&[10, 0, 0]), None);
        assert_eq!(plain_component(&[8, 0, 9, b'x']), None);
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

        let actions = read_input_actions(&mut In::new(&out.bytes)).actions;
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

        let actions = read_input_actions(&mut In::new(&out.bytes)).actions;

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
        let Input { actions, request, .. } = read_input_actions(&mut In::new(&out.bytes));
        let request = request.unwrap();
        let handled = bag.handle_requests(&request, false);
        assert_eq!(handled.response, [148,1,1,0,9,0]);
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

    /// Щелчок с рамкой в руке по западной грани блока: действие читается, а
    /// предмет Bedrock (`frame` 553, `glow_frame` 665) — это рамки Java.
    #[test]
    fn a_click_with_a_frame_in_hand_reads_as_a_frame_placement() {
        for (network_id, name) in [(553, "item_frame"), (665, "glow_item_frame")] {
            let item = crate::blocks::item_named(name).expect("рамка");
            let mut out = Out::default();

            out.zigzag32(0).varint(2).varint(0);
            out.varint(0).varint(1).zigzag32(3).zigzag32(64).zigzag32(-2).zigzag32(4).zigzag32(0);
            super::super::inventory::write_item(&mut out, Some(crate::inventory::Stack::new(item, 1)), 0);
            use_tail(&mut out);

            let action = read_item_use(&out.bytes).expect("действие");
            let held = action.held.expect("рамка в руке");

            assert_eq!((action.kind, action.position, action.face), (CLICK, (3, 64, -2), 4));
            assert_eq!(held.network_id, network_id);
            assert_eq!(java_item(held), Some(item));
        }
    }

    /// Щелчок и удар по клетке с рамкой читаются обычными действиями
    /// (клетка та же, что у рамки Java), из Player Auth Input — начало слома.
    #[test]
    fn breaking_a_frame_block_reads_as_a_break_action() {
        let mut out = Out::default();

        out.zigzag32(0).varint(2).varint(0);
        out.varint(BREAK).varint(0).zigzag32(0).zigzag32(70).zigzag32(0).zigzag32(5).zigzag32(0);
        out.zigzag32(0);
        use_tail(&mut out);

        let action = read_item_use(&out.bytes).expect("действие");

        assert_eq!((action.kind, action.position), (BREAK, (0, 70, 0)));
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
        let packet = available_commands(false);

        // Номер пакета 0x4c, пять пустых списков, 61 команда.
        assert_eq!(packet[..7], [0x4c, 0, 0, 0, 0, 0, 61]);

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
        let packet = available_commands(false);
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

    /// Особый динамит в списке команд — только тем, кому он позволен.
    #[test]
    fn tnt_is_listed_only_when_allowed() {
        let has_tnt = |packet: &[u8]| packet.windows(4).any(|window| window == [3, b't', b'n', b't']);

        assert!(!has_tnt(&available_commands(false)));

        let packet = available_commands(true);
        assert!(has_tnt(&packet));
        assert_eq!(packet[6] as usize, COMMANDS.len() + 1);
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
        let (dimension, sub_chunks, biome_palettes) = read_chunk(&encode_chunk(Dimension::Nether, 3, -2, &sections, &biomes, &[]));

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
            read_chunk(&encode_chunk(Dimension::Overworld, 3, -2, &sections, &biomes, &[]));

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

    /// Update Block, как у BDS: флаги «соседям + по сети» (3), у хранилищ без
    /// воды — один основной слой, у обычного блока — ещё и слой воды.
    #[test]
    fn block_updates_follow_the_official_server() {
        let state = |text: &str| crate::blocks::state_from_text(text).expect(text);
        let flags_and_layer = |packet: &[u8]| {
            let mut reader = In::new(packet);

            assert_eq!(reader.varint(), Some(session::UPDATE_BLOCK));
            reader.zigzag32();
            reader.varint();
            reader.zigzag32();
            reader.varint();

            (reader.varint(), reader.varint())
        };

        let barrel = block_updates(2, -60, 0, state("barrel[facing=up,open=true]"));
        assert_eq!(barrel.len(), 1);
        assert_eq!(flags_and_layer(&barrel[0]), (Some(3), Some(0)));

        let stone = block_updates(2, -60, 0, state("stone"));
        assert_eq!(stone.len(), 2);
        assert_eq!(flags_and_layer(&stone[0]), (Some(3), Some(0)));
        assert_eq!(flags_and_layer(&stone[1]), (Some(3), Some(1)));

        let chest = block_updates(2, -60, 0, state("chest"));
        assert_eq!(chest.len(), 2);
    }

    #[test]
    fn stored_actor_carries_the_bds_fields() {
        let barrel = stored_fields(containers::Kind::Barrel, actor_nbt(&screens::actor_packet(containers::Kind::Barrel, (1, 64, 2), None)));
        let has = |nbt: &[u8], name: &[u8]| nbt.windows(name.len()).any(|w| w == name);

        assert!(has(&barrel, b"Items") && has(&barrel, b"Findable") && has(&barrel, b"Barrel"));
        assert_eq!(barrel.last(), Some(&0));

        let hopper = stored_fields(containers::Kind::Hopper, actor_nbt(&screens::actor_packet(containers::Kind::Hopper, (1, 64, 2), None)));
        assert!(has(&hopper, b"TransferCooldown") && !has(&hopper, b"Findable"));

        // Пакет без места и номера: NBT начинается с компаунда без имени.
        assert_eq!(&barrel[..2], &[10, 0]);
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
