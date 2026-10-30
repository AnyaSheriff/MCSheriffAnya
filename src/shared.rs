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

/// Место в мире: x, y, z.
pub type Place = (f64, f64, f64);

/// Поручение /item удерживает границу только до публикации новой руки.
/// Arc допускает клонирование Order; finish/Drop освобождают счётчик один раз.
#[derive(Debug)]
pub struct CommandEquipmentCompletion {
    waiting: Arc<[std::sync::atomic::AtomicUsize; 2]>,
    index: usize,
    until: u64,
    active: std::sync::atomic::AtomicBool,
}

impl CommandEquipmentCompletion {
    pub(crate) fn finish(&self) {
        if self.active.swap(false, Ordering::AcqRel) {
            self.waiting[self.index].fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for CommandEquipmentCompletion {
    fn drop(&mut self) { self.finish(); }
}

/// То, что видят все подключения сразу.
///
/// **Порядок замков.** Если нужно держать несколько замков сразу, брать их
/// строго в таком порядке: мир (`world_in`), `players`, предметы
/// (`items_in`), `chat`. Два мира разом не держать. Такт держит
/// мир и берёт игроков и предметы; кто возьмёт их в обратном порядке, рано
/// или поздно застынет вместе с тактом — а за ними встанет весь сервер
/// (так и было: вход игрока брал игроков раньше мира).
pub struct Shared {
    catalog: crate::gameplay_catalog::Catalog,
    pub auth: Option<std::sync::Arc<crate::auth::Service>>,
    pub premium: crate::auth::premium::Detector,
    /// Миры измерений по порядку `Dimension::ALL`: блоки, такты, события.
    worlds: [Mutex<World>; 3],

    /// Кто сейчас на сервере.
    pub players: Mutex<Players>,
    pub sessions: crate::sessions::Sessions,

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
    pub fishing: Mutex<Vec<crate::fishing::Bobber>>,
    pub maps: Arc<Mutex<crate::maps::Maps>>,

    /// Кто на ком едет (riding.rs): лодки, вагонетки, позже лошади.
    /// Замок — после предметов.
    pub riding: Mutex<crate::riding::Riding>,

    /// Кто и что может: список прав, он же файл ops.json.
    pub ops: Mutex<Ops>,

    /// Номер только что закончившегося такта: по нему подключения рассылают
    /// клиентам всё, что случилось за такт, — разом, как у оригинала.
    pub ticks: tokio::sync::watch::Sender<u64>,
    /// Атомарная публикация связанных звуков и экипировки (арбалет).
    pub publication: Mutex<()>,

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
    command_equipment: Mutex<Vec<std::sync::Weak<CommandEquipmentCompletion>>>,

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
    pub sculk_warnings: Mutex<std::collections::BTreeMap<[u8; 16], crate::redstone::sculk_family::Warning>>,

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
    pub limited_crafting: std::sync::atomic::AtomicBool,
    pub keep_inventory: std::sync::atomic::AtomicBool,

    /// Правило `spawn_mobs` (26.1: прежнее doMobSpawning): появляются ли
    /// животные сами в новых чанках. Призванные командой — всегда.
    pub spawn_mobs: std::sync::atomic::AtomicBool,

    /// Правила 26.1 `spawn_monsters` (враждебные сами) и `spawn_phantoms`
    /// (фантомы ночью, вики, Phantom).
    pub spawn_monsters: std::sync::atomic::AtomicBool,
    pub spawn_phantoms: std::sync::atomic::AtomicBool,

    /// Тактов до следующей попытки появления фантомов (mob/phantom.rs).
    pub phantom_wait: std::sync::atomic::AtomicI32,
    pub trader_spawner: Mutex<crate::mob::trader::Spawner>,
    pub spawn_traders: std::sync::atomic::AtomicBool,

    /// Кто только что проснулся и где (хозяева кошек ждут подарков, mob/tame.rs).
    pub morning_gifts: std::sync::Mutex<Vec<([u8; 16], Place)>>,

    /// Правила чешуйниц: изменение блоков мобами и выход из заражённых блоков.
    pub mob_griefing: std::sync::atomic::AtomicBool,
    /// Правила 26.1 `raids` (прежнее disableRaids наоборот) и `spawn_patrols`
    /// (doPatrolSpawning): набеги и патрули (raid.rs).
    pub raids: std::sync::atomic::AtomicBool,
    pub spawn_patrols: std::sync::atomic::AtomicBool,
    pub block_drops: std::sync::atomic::AtomicBool,

    /// Правило `announce_advancements` (прежнее announceAdvancements): писать ли
    /// в чат о полученных достижениях.
    pub advancement_states: Mutex<std::collections::HashMap<[u8; 16], (crate::advancements::AdvState, u64)>>,
    pub announce_advancements: std::sync::atomic::AtomicBool,

    /// Правило `natural_health_regeneration`: лечит ли сытость.
    pub freeze_damage: std::sync::atomic::AtomicBool,
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
    /// Только /item weapon.mainhand в основном профиле 26.3. Профиль 26.1.2,
    /// Bedrock и другие ячейки сохраняют прежнюю очередь. Замков через await нет.
    pub(crate) fn order_mainhand(&self, uuid: &[u8; 16], stack: Option<crate::inventory::Stack>) {
        let java = self.players.lock().expect("игроки").has_java_waker(uuid);
        let order = if java && self.catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3 {
            let completed = *self.ticks.borrow();
            let started = *self.tick_starts.borrow();
            let until = started.max(completed.saturating_add(1));
            let index = (until.wrapping_sub(1) % 2) as usize;
            let completion = {
                let mut pending = self.command_equipment.lock().expect("поручения экипировки");
                pending.retain(|weak| weak.upgrade().is_some_and(|ticket| ticket.active.load(Ordering::Acquire)));
                // Не растим отдельный registry без границы. При перегрузке
                // поручение остаётся в FIFO и будит владельца, но такт не ждёт.
                if pending.len() >= 1024 { None } else {
                    self.waiting_for_tick[index].fetch_add(1, Ordering::AcqRel);
                    let ticket = Arc::new(CommandEquipmentCompletion {
                        waiting: self.waiting_for_tick.clone(), index, until,
                        active: std::sync::atomic::AtomicBool::new(true),
                    });
                    pending.push(Arc::downgrade(&ticket));
                    Some(ticket)
                }
            };
            crate::players::Order::CommandHold { stack, completion }
        } else { crate::players::Order::Hold { stack } };
        self.players.lock().expect("игроки").order(uuid, order);
    }

    /// После прежних 5 мс такт продолжает работу. Поздний владелец применит
    /// поручение, но уже не удержит следующий такт той же чётности.
    pub(crate) fn expire_command_equipment(&self, tick: u64) {
        self.command_equipment.lock().expect("поручения экипировки").retain(|weak| {
            let Some(ticket) = weak.upgrade() else { return false; };
            if ticket.until <= tick { ticket.finish(); }
            ticket.active.load(Ordering::Acquire)
        });
    }

    pub fn catalog(&self) -> crate::gameplay_catalog::Catalog { self.catalog }

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
        let world_catalog = world.catalog();
        Self::with_end(world, nether, World::in_memory_with_catalog(world_catalog), playerdata, skins, properties, ops, skin_settings)
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
        let catalog = world.catalog();
        assert_eq!(catalog, nether.catalog(), "измерения должны иметь один каталог");
        assert_eq!(catalog, end.catalog(), "измерения должны иметь один каталог");
        let difficulty = std::sync::atomic::AtomicU8::new(properties.difficulty);
        let maps = Arc::new(Mutex::new(crate::maps::Maps::default()));
        let (mut world, mut nether, mut end) = (world, nether, end);
        for world in [&mut world, &mut nether, &mut end] { world.maps = Arc::clone(&maps); }

        Self {
            catalog,
            difficulty,
            limited_crafting: std::sync::atomic::AtomicBool::new(false),
            keep_inventory: std::sync::atomic::AtomicBool::new(false),
            spawn_mobs: std::sync::atomic::AtomicBool::new(true),
            spawn_monsters: std::sync::atomic::AtomicBool::new(true),
            spawn_phantoms: std::sync::atomic::AtomicBool::new(true),
            phantom_wait: std::sync::atomic::AtomicI32::new(0),
            trader_spawner: Mutex::new(crate::mob::trader::Spawner::default()),
            spawn_traders: std::sync::atomic::AtomicBool::new(true),
            morning_gifts: std::sync::Mutex::new(Vec::new()),
            mob_griefing: std::sync::atomic::AtomicBool::new(true),
            raids: std::sync::atomic::AtomicBool::new(true),
            spawn_patrols: std::sync::atomic::AtomicBool::new(true),
            block_drops: std::sync::atomic::AtomicBool::new(true),
            advancement_states: Mutex::new(std::collections::HashMap::new()),
            announce_advancements: std::sync::atomic::AtomicBool::new(true),
            freeze_damage: std::sync::atomic::AtomicBool::new(true),
            natural_regeneration: std::sync::atomic::AtomicBool::new(true),
            command_block_rules: crate::command_block::Rules::default(),
            scoreboard: Mutex::new(crate::scoreboard::Scoreboard::new()),
            functions: Mutex::new(crate::functions::Functions::new(std::path::Path::new("world"))),
            advance_weather: std::sync::atomic::AtomicBool::new(true),
            sleeping_percentage: std::sync::atomic::AtomicI32::new(100),
            worlds: [Mutex::new(world), Mutex::new(nether), Mutex::new(end)],
            players: Mutex::new(Players::new()),
            sessions: crate::sessions::Sessions::default(),
            chat: Mutex::new(Chat::new()),
            items: std::array::from_fn(|_| Mutex::new(Items::with_catalog(catalog))),
            arrows: Mutex::new(crate::arrow::Arrows::new()),
            thrown: Mutex::new(crate::thrown::Thrown::new()),
            fishing: Mutex::new(Vec::new()),
            maps,
            riding: Mutex::new(crate::riding::Riding::new()),
            auth: None,
            premium: crate::auth::premium::Detector::default(),
            ops: Mutex::new(ops),
            ticks: tokio::sync::watch::channel(0).0,
            publication: Mutex::new(()),
            tick_starts: tokio::sync::watch::channel(0).0,
            waiting_for_tick: Arc::new([std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0)]),
            command_equipment: Mutex::new(Vec::new()),
            // Номера начинаются с единицы: ноль клиент понимает как «никого».
            entity_ids: AtomicI32::new(1),
            readers: AtomicU64::new(1),
            playerdata,
            sculk_warnings: Mutex::new(std::collections::BTreeMap::new()),
            skins,
            properties,
            skin_settings,
            settings: Default::default(),
            world_cmd: Default::default(),
        }
    }
}
