// Общее состояние сервера.
//
// Всё, что подключения делят между собой: мир, список игроков и лента чата.
// Подключения живут в отдельных задачах, поэтому состояние лежит под замком
// и передаётся им одним общим указателем, а не по частям.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use crate::chat::Chat;
use crate::config::server_properties::ServerProperties;
use crate::items::Items;
use crate::ops::Ops;
use crate::players::Players;
use crate::world::World;

/// То, что видят все подключения сразу.
///
/// **Порядок замков.** Если нужно держать несколько замков сразу, брать их
/// строго в таком порядке: `world`, `players`, `items`, `chat`. Такт держит
/// мир и берёт игроков и предметы; кто возьмёт их в обратном порядке, рано
/// или поздно застынет вместе с тактом — а за ними встанет весь сервер
/// (так и было: вход игрока брал игроков раньше мира).
pub struct Shared {
    /// Блоки мира.
    pub world: Mutex<World>,

    /// Кто сейчас на сервере.
    pub players: Mutex<Players>,

    /// Лента сообщений в чат.
    pub chat: Mutex<Chat>,

    /// Предметы, лежащие в мире.
    pub items: Mutex<Items>,

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

    /// Сколько подключений ждут начала такта, чтобы применить пакет. Такт
    /// не начинает работу, пока они не отработают.
    pub waiting_for_tick: std::sync::atomic::AtomicUsize,

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
}

impl Shared {
    /// Выдаёт номер следующего читателя журналов.
    pub fn next_reader(&self) -> crate::journal::Reader {
        self.readers.fetch_add(1, Ordering::Relaxed)
    }

    /// Выдаёт номер следующей сущности.
    pub fn next_entity_id(&self) -> i32 {
        self.entity_ids.fetch_add(1, Ordering::Relaxed)
    }

    /// Заводит общее состояние сервера.
    pub fn new(
        world: World,
        playerdata: PathBuf,
        skins: PathBuf,
        properties: ServerProperties,
        ops: Ops,
        skin_settings: crate::skins::Settings,
    ) -> Self {
        Self {
            world: Mutex::new(world),
            players: Mutex::new(Players::new()),
            chat: Mutex::new(Chat::new()),
            items: Mutex::new(Items::new()),
            ops: Mutex::new(ops),
            ticks: tokio::sync::watch::channel(0).0,
            tick_starts: tokio::sync::watch::channel(0).0,
            waiting_for_tick: std::sync::atomic::AtomicUsize::new(0),
            // Номера начинаются с единицы: ноль клиент понимает как «никого».
            entity_ids: AtomicI32::new(1),
            readers: AtomicU64::new(1),
            playerdata,
            skins,
            properties,
            skin_settings,
            settings: Default::default(),
        }
    }
}
