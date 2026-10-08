// Общее состояние сервера.
//
// Всё, что подключения делят между собой: мир, список игроков и лента чата.
// Подключения живут в отдельных задачах, поэтому состояние лежит под замком
// и передаётся им одним общим указателем, а не по частям.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use crate::chat::Chat;
use crate::dimension::Dimension;
use crate::config::server_properties::ServerProperties;
use crate::items::Items;
use crate::ops::Ops;
use crate::players::Players;
use crate::world::World;

/// То, что видят все подключения сразу.
///
/// **Порядок замков.** Если нужно держать несколько замков сразу, брать их
/// строго в таком порядке: мир (`world_in`), `players`, предметы
/// (`items_in`), `chat`. Два мира разом не держать. Такт держит
/// мир и берёт игроков и предметы; кто возьмёт их в обратном порядке, рано
/// или поздно застынет вместе с тактом — а за ними встанет весь сервер
/// (так и было: вход игрока брал игроков раньше мира).
pub struct Shared {
    /// Миры измерений по порядку `Dimension::ALL`: блоки, такты, события.
    worlds: [Mutex<World>; 3],

    /// Кто сейчас на сервере.
    pub players: Mutex<Players>,

    /// Лента сообщений в чат.
    pub chat: Mutex<Chat>,

    /// Предметы, лежащие в мирах, — у каждого измерения свои.
    items: [Mutex<Items>; 3],

    /// Стрелы (arrow.rs): в полёте и застрявшие, всех измерений.
    /// Замок — после предметов.
    pub arrows: Mutex<crate::arrow::Arrows>,

    /// Брошенные зелья, огненные шары и облака (thrown.rs), всех измерений.
    /// Замок — после предметов.
    pub thrown: Mutex<crate::thrown::Thrown>,

    /// Кто на ком едет (riding.rs): лодки, вагонетки, позже лошади.
    /// Замок — после предметов.
    pub riding: Mutex<crate::riding::Riding>,

    /// Кто и что может: список прав, он же файл ops.json.
    pub ops: Mutex<Ops>,

    /// Номер только что закончившегося такта: по нему подключения рассылают
    /// клиентам всё, что случилось за такт, — разом, как у оригинала.
    pub ticks: tokio::sync::watch::Sender<u64>,

    /// Номер такта, который вот-вот начнётся. Действия игроков применяются
    /// к миру по этому сигналу — в начале такта, до его работы: у оригинала
    /// пакеты разбираются раньше запланированных тактов и ходов поршней
    /// (сверено чёрным ящиком), и от этого зависит, в какой такт попадёт
    /// щелчок по рычагу относительно поршня.
    pub tick_starts: tokio::sync::watch::Sender<u64>,

    /// Сколько прочитанных действий игроков (щелчок, ломание, команда) ещё
    /// не применено. Такт не начинает работу, пока они не отработают: как
    /// у оригинала, всё, что пришло до начала такта, ложится в этот такт.
    /// Счётчик общий с читателями подключений, поэтому в `Arc`.
    ///
    /// Два счётчика по чётности такта, во время которого действие пришло:
    /// такт `T` ждёт только пришедшее до него (такт `T − 1`). Действие,
    /// прочитанное сразу после сигнала начала `T`, ляжет в `T + 1`, и ждать
    /// его такту незачем — раньше он терял на этом всё терпение.
    pub waiting_for_tick: Arc<[std::sync::atomic::AtomicUsize; 2]>,

    /// Номер, который достанется следующей сущности.
    ///
    /// Счётчик общий для игроков и лежащих предметов: клиент различает
    /// сущности только по номеру, и повтор означал бы, что он принял одну
    /// за другую.
    entity_ids: AtomicI32,

    /// Номер, который достанется следующему читателю журналов.
    ///
    /// Журналы держат записи, пока их не прочитали все, и отличают читателей
    /// по этому номеру.
    readers: AtomicU64,

    /// Директория, в которой лежат файлы игроков. Подключению она нужна,
    /// чтобы записывать положение игрока на диск и читать его при заходе.
    pub playerdata: PathBuf,

    /// Директория со скинами: там лежит своя таблица «кому чей скин» и то,
    /// что сервер запомнил из ответов Mojang.
    pub skins: PathBuf,

    /// Настройки сервера: дальность прорисовки и всё прочее, что задаётся
    /// в server.properties.
    pub properties: ServerProperties,

    /// Откуда брать скины.
    pub skin_settings: crate::skins::Settings,

    /// Свои настройки сервера (config/mcsa.properties).
    pub settings: crate::config::mcsheriffanya::Settings,

    /// Сложность сейчас (0…3): начинается со значения server.properties,
    /// команда `difficulty` меняет её на ходу.
    pub difficulty: std::sync::atomic::AtomicU8,

    /// Правило `keepInventory`: после смерти вещи остаются у игрока.
    pub keep_inventory: std::sync::atomic::AtomicBool,

    /// Правило `spawn_mobs` (26.1: прежнее doMobSpawning): появляются ли
    /// животные сами в новых чанках. Призванные командой — всегда.
    pub spawn_mobs: std::sync::atomic::AtomicBool,

    /// Правила чешуйниц: изменение блоков мобами и выход из заражённых блоков.
    pub mob_griefing: std::sync::atomic::AtomicBool,
    pub block_drops: std::sync::atomic::AtomicBool,

    /// Правило `announce_advancements` (прежнее announceAdvancements): писать ли
    /// в чат о полученных достижениях.
    pub announce_advancements: std::sync::atomic::AtomicBool,

    /// Правило `natural_health_regeneration`: лечит ли сытость.
    pub natural_regeneration: std::sync::atomic::AtomicBool,

    /// Правила командных блоков (command_block.rs): `command_block_output`,
    /// `command_blocks_work`, `max_command_sequence_length`.
    pub command_block_rules: crate::command_block::Rules,
    /// Рамки мира, скорость такта, удержанные чанки и прочее команд мира
    /// (commands_world.rs).
    pub world_cmd: crate::commands_world::State,
    /// Табло, команды, метки и полосы босса (scoreboard.rs). Замок — после
    /// всех прочих: под ним другие не берутся.
    pub scoreboard: Mutex<crate::scoreboard::Scoreboard>,

    /// Функции наборов данных и расписание (functions.rs). Под ним табло не
    /// берётся.
    pub functions: Mutex<crate::functions::Functions>,
    /// Правило `advance_weather` (прежнее doWeatherCycle): меняется ли
    /// погода сама.
    pub advance_weather: std::sync::atomic::AtomicBool,
    /// Правило `players_sleeping_percentage` (прежнее playersSleepingPercentage):
    /// какая доля игроков должна спать, чтобы пропустить ночь (bed.rs).
    pub sleeping_percentage: std::sync::atomic::AtomicI32,
}

impl Shared {
    /// Мир этого измерения.
    pub fn world_in(&self, dimension: Dimension) -> &Mutex<World> {
        &self.worlds[dimension.index()]
    }

    /// Выпавшие предметы этого измерения.
    pub fn items_in(&self, dimension: Dimension) -> &Mutex<Items> {
        &self.items[dimension.index()]
    }

    /// Выдаёт номер следующего читателя журналов.
    pub fn next_reader(&self) -> crate::journal::Reader {
        self.readers.fetch_add(1, Ordering::Relaxed)
    }

    /// Выдаёт номер следующей сущности.
    pub fn next_entity_id(&self) -> i32 {
        self.entity_ids.fetch_add(1, Ordering::Relaxed)
    }

    /// Для проверок: третий мир (Энд) — пустой, в памяти.
    #[cfg(test)]
    pub fn new(
        world: World,
        nether: World,
        playerdata: PathBuf,
        skins: PathBuf,
        properties: ServerProperties,
        ops: Ops,
        skin_settings: crate::skins::Settings,
    ) -> Self {
        Self::with_end(world, nether, World::in_memory(), playerdata, skins, properties, ops, skin_settings)
    }

    /// Заводит общее состояние сервера.
    #[allow(clippy::too_many_arguments)]
    pub fn with_end(
        world: World,
        nether: World,
        end: World,
        playerdata: PathBuf,
        skins: PathBuf,
        properties: ServerProperties,
        ops: Ops,
        skin_settings: crate::skins::Settings,
    ) -> Self {
        let difficulty = std::sync::atomic::AtomicU8::new(properties.difficulty);

        Self {
            difficulty,
            keep_inventory: std::sync::atomic::AtomicBool::new(false),
            spawn_mobs: std::sync::atomic::AtomicBool::new(true),
            mob_griefing: std::sync::atomic::AtomicBool::new(true),
            block_drops: std::sync::atomic::AtomicBool::new(true),
            announce_advancements: std::sync::atomic::AtomicBool::new(true),
            natural_regeneration: std::sync::atomic::AtomicBool::new(true),
            command_block_rules: crate::command_block::Rules::default(),
            scoreboard: Mutex::new(crate::scoreboard::Scoreboard::new()),
            functions: Mutex::new(crate::functions::Functions::new(std::path::Path::new("world"))),
            advance_weather: std::sync::atomic::AtomicBool::new(true),
            sleeping_percentage: std::sync::atomic::AtomicI32::new(100),
            worlds: [Mutex::new(world), Mutex::new(nether), Mutex::new(end)],
            players: Mutex::new(Players::new()),
            chat: Mutex::new(Chat::new()),
            items: [Mutex::new(Items::new()), Mutex::new(Items::new()), Mutex::new(Items::new())],
            arrows: Mutex::new(crate::arrow::Arrows::new()),
            thrown: Mutex::new(crate::thrown::Thrown::new()),
            riding: Mutex::new(crate::riding::Riding::new()),
            ops: Mutex::new(ops),
            ticks: tokio::sync::watch::channel(0).0,
            tick_starts: tokio::sync::watch::channel(0).0,
            waiting_for_tick: Arc::new([std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0)]),
            // Номера начинаются с единицы: ноль клиент понимает как «никого».
            entity_ids: AtomicI32::new(1),
            readers: AtomicU64::new(1),
            playerdata,
            skins,
            properties,
            skin_settings,
            settings: Default::default(),
            world_cmd: Default::default(),
        }
    }
}
