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
    pub(crate) player_queries: crate::player_query::Broker,
    pub sessions: crate::sessions::Sessions,

    /// Лента сообщений в чат.
    pub chat: Mutex<Chat>,

    /// Предметы, лежащие в мирах, — у каждого измерения свои.
    items: [Mutex<Items>; 3],
    /// Только писатели sidecar: свежий снимок не обгоняет прежнюю запись.
    ground_saves: Mutex<()>,

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
    /// Настоящее состояние паузы без игроков; чтение API не считается активностью.
    pub idle_paused: std::sync::atomic::AtomicBool,
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

    /// Путь берётся у самого мира, включая отдельные каталоги измерений.
    fn ground_items_path(&self, dimension: Dimension) -> Option<PathBuf> {
        self.world_in(dimension).lock().expect("мир").storage_directory()
            .map(|directory| directory.join("dropped-items.mcri"))
    }

    /// Только при запуске, до такта и подключений. Ошибка файла не означает пустой мир.
    pub(crate) fn load_ground_items(&self) -> std::io::Result<()> {
        for dimension in Dimension::ALL {
            if let Some(path) = self.ground_items_path(dimension) {
                self.items_in(dimension).lock().expect("предметы")
                    .load_ground_items(&path, &mut || self.next_entity_id())
                    .map_err(|error| std::io::Error::new(error.kind(), format!(
                        "Не удалось загрузить выпавшие предметы {} из {}: {}",
                        dimension.name(), path.display(), error)))?;
            }
        }
        Ok(())
    }

    /// Полные снимки включают предметы в выгруженных чанках.
    /// Кодирование и диск не держат замки мира и предметов; писатели идут по очереди.
    pub(crate) fn save_ground_items(&self) -> std::io::Result<()> {
        let _writer = self.ground_saves.lock().expect("запись выпавших предметов");
        let mut first_error = None;
        for dimension in Dimension::ALL {
            if let Some(path) = self.ground_items_path(dimension) {
                let checkpoint = {
                    let items = self.items_in(dimension).lock().expect("предметы");
                    items.ground_items_checkpoint()
                };
                if let Err(error) = checkpoint.and_then(|checkpoint| checkpoint.save(&path)) {
                    crate::log_error!("Выпавшие предметы {} не записаны: {}", dimension.name(), error);
                    if first_error.is_none() { first_error = Some(error); }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Мир этого измерения.
    pub fn world_in(&self, dimension: Dimension) -> &Mutex<World> {
        &self.worlds[dimension.index()]
    }

    /// Подготовка с популяторами выполняется без замка мира и до публикации чанка.
    pub fn ensure_chunk(&self,dimension:Dimension,x:i32,z:i32)->bool {
        if !crate::plugins::active() {let mut world=self.world_in(dimension).lock().expect("мир");world.ensure(x,z);return world.has_chunk(x,z);}
        for _ in 0..2 {
            let source={let world=self.world_in(dimension).lock().expect("мир");if world.has_chunk(x,z) {return true;}world.source()};
            let ready=source.take(x,z);
            if self.world_in(dimension).lock().expect("мир").accept_prepared(x,z,ready) {return true;}
        }false
    }
    pub fn accept_chunk(&self,dimension:Dimension,x:i32,z:i32,ready:crate::world::ReadyChunk) {
        let accepted=self.world_in(dimension).lock().expect("мир").accept_prepared(x,z,ready);
        if !accepted {self.ensure_chunk(dimension,x,z);}
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
            ground_saves: Mutex::new(()),
            arrows: Mutex::new(crate::arrow::Arrows::new()),
            thrown: Mutex::new(crate::thrown::Thrown::new()),
            fishing: Mutex::new(Vec::new()),
            maps,
            riding: Mutex::new(crate::riding::Riding::new()),
            auth: None,
            premium: crate::auth::premium::Detector::default(),
            ops: Mutex::new(ops),
            ticks: tokio::sync::watch::channel(0).0,
            idle_paused: std::sync::atomic::AtomicBool::new(false),
            publication: Mutex::new(()),
            tick_starts: tokio::sync::watch::channel(0).0,
            waiting_for_tick: Arc::new([std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0)]),
            command_equipment: Mutex::new(Vec::new()),
            player_queries: Default::default(),
            // Номера начинаются с единицы: ноль клиент понимает как «никого».
            entity_ids: AtomicI32::new(1),
            readers: AtomicU64::new(1),
            playerdata,
            sculk_warnings: Mutex::new(std::collections::BTreeMap::new()),
            skins,
            properties,
            skin_settings,
            // Каталог переданного мира определяет начальные настройки и для старых
            // фикстур/миграции. Основной запуск отдельно проверяет конфиг и заменяет их.
            settings: crate::config::mcsheriffanya::Settings {
                java_version: Some(match catalog.profile() {
                    crate::gameplay_catalog::Profile::Java26_1_2 => mcsa_protocol::JavaVersion::V26_1_2,
                    crate::gameplay_catalog::Profile::Preview26_3 => mcsa_protocol::JavaVersion::V26_3,
                }),
                ..Default::default()
            },
            world_cmd: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ground_items_checkpoint_uses_actual_dimension_folders_and_survives_reopen() {
        use crate::config::server_properties::WorldKind;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp")
            .join(format!("shared-ground-reopen-{}", std::process::id()));
        assert!(!root.exists(), "каталог принадлежит только этой проверке");
        let folders = [root.join("custom-overworld"), root.join("custom-nether"), root.join("custom-end")];
        let open = || Shared::with_end(
            World::open(&folders[0], Some(1), WorldKind::Flat).unwrap(),
            World::open(&folders[1], Some(1), WorldKind::Nether).unwrap(),
            World::open(&folders[2], Some(1), WorldKind::End).unwrap(),
            root.join("players"), root.join("skins"), Default::default(),
            Ops::open(&root.join("ops.json")), Default::default(),
        );
        let shared = open();
        shared.load_ground_items().unwrap();
        let stack = crate::inventory::Stack::new(crate::blocks::item_named("diamond_sword").unwrap(), 1)
            .with_extras(crate::stackdata::Data { name: Some("Сохранённый меч".into()), damage: 23, ..Default::default() });
        let mut uuids = Vec::new();
        for dimension in Dimension::ALL {
            let uuid = shared.items_in(dimension).lock().unwrap().drop_from_player(
                shared.next_entity_id(), stack, (4096.5, 70.0, -4096.5), (0.1, 0.2, -0.1), [7; 16]);
            uuids.push(uuid);
            assert_eq!(shared.ground_items_path(dimension), Some(folders[dimension.index()].join("dropped-items.mcri")));
        }
        shared.save_ground_items().unwrap();
        // Повторная запись заменяет полный снимок, не добавляет записи.
        shared.save_ground_items().unwrap();
        drop(shared);
        let restored = open();
        restored.load_ground_items().unwrap();
        for dimension in Dimension::ALL {
            let items = restored.items_in(dimension).lock().unwrap();
            let drops = items.items();
            assert_eq!(drops.len(), 1);
            assert_eq!(drops[0].uuid, uuids[dimension.index()]);
            assert_eq!(drops[0].stack, stack);
            assert_eq!((drops[0].x, drops[0].y, drops[0].z), (4096.5, 70.0, -4096.5));
            assert_eq!(drops[0].thrower, Some([7; 16]));
        }
        assert!(restored.load_ground_items().is_err(), "повторная загрузка не дублирует предметы");
        restored.items_in(Dimension::Overworld).lock().unwrap().remove_items(|_| true);
        restored.save_ground_items().unwrap();
        drop(restored);
        let picked_up = open();
        picked_up.load_ground_items().unwrap();
        assert!(picked_up.items_in(Dimension::Overworld).lock().unwrap().items().is_empty(),
            "пустой снимок не воскрешает уже подобранные предметы");
        assert_eq!(picked_up.items_in(Dimension::Nether).lock().unwrap().items().len(), 1);
        drop(picked_up);
        let corrupt = folders[0].join("dropped-items.mcri");
        std::fs::write(&corrupt, b"broken").unwrap();
        let invalid = open();
        let error = invalid.load_ground_items().unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains(&corrupt.display().to_string()));
        assert!(error.to_string().contains(Dimension::Overworld.name()));
        assert_eq!(std::fs::read(&corrupt).unwrap(), b"broken");
        drop(invalid);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn in_memory_worlds_have_no_ground_items_checkpoint_path() {
        let shared = Shared::new(World::in_memory(), World::in_memory(), PathBuf::new(), PathBuf::new(),
            Default::default(), Ops::open(std::path::Path::new("tmp/shared-ground-memory/ops.json")), Default::default());
        for dimension in Dimension::ALL { assert!(shared.ground_items_path(dimension).is_none()); }
        shared.load_ground_items().unwrap();
        shared.save_ground_items().unwrap();
    }

    use super::*;

    #[test]
    fn constructor_settings_match_all_world_catalogs() {
        use crate::gameplay_catalog::{Catalog, Profile};
        for (profile, version) in [
            (Profile::Java26_1_2, mcsa_protocol::JavaVersion::V26_1_2),
            (Profile::Preview26_3, mcsa_protocol::JavaVersion::V26_3),
        ] {
            let catalog = Catalog::new(profile);
            let shared = Shared::new(
                World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
                PathBuf::new(), PathBuf::new(), Default::default(),
                Ops::open(std::path::Path::new("tmp/shared-catalog-fixture/ops.json")),
                Default::default(),
            );
            assert_eq!(shared.settings.java_version, Some(version));
            assert_eq!(Catalog::for_version(shared.settings.java_version.unwrap()), shared.catalog());
            for dimension in [Dimension::Overworld, Dimension::Nether, Dimension::End] {
                assert_eq!(shared.world_in(dimension).lock().unwrap().catalog(), catalog);
            }
        }
        assert_eq!(crate::config::mcsheriffanya::Settings::default().java_version,
            Some(mcsa_protocol::JavaVersion::V26_3));
    }
}
