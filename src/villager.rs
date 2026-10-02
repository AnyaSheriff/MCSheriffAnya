// Жители: данные жителя (тип, профессия, уровень), сделки, торговля.
//
// Кода игры и чужих ядер здесь нет. Правила — со страниц minecraft.wiki
// (Villager, Trading), таблицы сделок — со страницы Trading (раздел «Trade
// offers», Java Edition, без опыта «Villager Trade Rebalance»), номера типов и
// профессий и всё, что видно по сети, сняты с официального сервера чёрным
// ящиком: сценарии villager-trade, villager-levelup, villager-shake,
// villager-claim, villager-levelup-ai.
//
// Окно торговли (ячейки оплаты, выход, выбор сделки) — здесь же, без сети:
// play.rs только передаёт щелчки и рассылает итог.
//
// Что не сверено, помечено «[проверить]».

use crate::blocks;
use crate::inventory::Stack;
use crate::mob::Rng;

/// Типы жителей по порядку номеров (чёрный ящик, villager-shake: plains — 2,
/// snow — 4, taiga — 6; порядок — по алфавиту).
pub const TYPES: [&str; 7] = ["desert", "jungle", "plains", "savanna", "snow", "swamp", "taiga"];

/// Профессии по порядку номеров (чёрный ящик, villager-trade и villager-shake:
/// farmer — 5, nitwit — 11; без профессии — 0).
pub const PROFESSIONS: [&str; 15] = [
    "none",
    "armorer",
    "butcher",
    "cartographer",
    "cleric",
    "farmer",
    "fisherman",
    "fletcher",
    "leatherworker",
    "librarian",
    "mason",
    "nitwit",
    "shepherd",
    "toolsmith",
    "weaponsmith",
];

pub const PLAINS: u8 = 2;
pub const NONE: u8 = 0;
pub const NITWIT: u8 = 11;

/// Сколько всего опыта нужно жителю на уровень 1–5 (вики, Trading, «Level»).
pub const LEVEL_XP: [i32; 5] = [0, 10, 70, 150, 250];

/// Сколько тактов житель мотает головой (чёрный ящик, villager-shake:
/// свойство 18 = 40 и вниз по одному за такт).
pub const UNHAPPY_TICKS: i32 = 40;

/// Через сколько тактов без торговли житель поднимает уровень (чёрный ящик,
/// villager-levelup-ai: окно закрыто на 80-м, уровень 2 — на 120-м).
pub const LEVEL_UP_DELAY: i32 = 40;

/// Регенерация после повышения: 10 секунд (вики, Trading: «10 seconds of
/// Regeneration I»).
pub const LEVEL_UP_REGEN: i32 = 200;

/// Звуковой порог «сделка обновилась»: не чаще, чем через 20 тактов после
/// прошлого голоса (чёрный ящик, villager-trade и villager-levelup: звук
/// «да» молчит через 10–20 тактов после голоса, звучит через 24–26).
pub const TRADE_SOUND_GAP: i32 = 20;

/// Номер типа по имени (`minecraft:taiga` или `taiga`).
pub fn type_named(name: &str) -> Option<u8> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);

    TYPES.iter().position(|known| *known == name).map(|index| index as u8)
}

/// Номер профессии по имени.
pub fn profession_named(name: &str) -> Option<u8> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);

    PROFESSIONS.iter().position(|known| *known == name).map(|index| index as u8)
}

/// Рабочий блок профессии (вики, Villager, «Professions»).
pub fn job_site(profession: u8) -> Option<&'static str> {
    Some(match *PROFESSIONS.get(usize::from(profession))? {
        "armorer" => "blast_furnace",
        "butcher" => "smoker",
        "cartographer" => "cartography_table",
        "cleric" => "brewing_stand",
        "farmer" => "composter",
        "fisherman" => "barrel",
        "fletcher" => "fletching_table",
        "leatherworker" => "cauldron",
        "librarian" => "lectern",
        "mason" => "stonecutter",
        "shepherd" => "loom",
        "toolsmith" => "smithing_table",
        "weaponsmith" => "grindstone",
        _ => return None,
    })
}

/// Профессия, которую даёт рабочий блок (имя блока без свойств). Котёл —
/// любой: с водой, лавой, снегом [проверить].
pub fn profession_of_block(block: &str) -> Option<u8> {
    let block = match block {
        "water_cauldron" | "lava_cauldron" | "powder_snow_cauldron" => "cauldron",
        other => other,
    };

    (1..PROFESSIONS.len() as u8).find(|profession| job_site(*profession) == Some(block))
}

/// Торгует ли житель этой профессии: у безработного и бестолкового сделок нет.
pub fn trades(profession: u8) -> bool {
    profession != NONE && profession != NITWIT && usize::from(profession) < PROFESSIONS.len()
}

/// Пора ли подниматься: опыт дорос до следующего уровня (вики, Trading,
/// «Level»: 10, 70, 150, 250).
pub fn should_level_up(level: u8, xp: i32) -> bool {
    (1..5).contains(&level) && xp >= LEVEL_XP[usize::from(level)]
}

/// Сделка.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    /// Что житель хочет (первое — с ценой по спросу) и второе, если есть.
    pub buy: Stack,
    pub buy_b: Option<Stack>,

    /// Что отдаёт.
    pub sell: Stack,

    pub uses: i32,
    pub max_uses: i32,

    /// Опыт жителю за сделку.
    pub xp: i32,

    /// Даёт ли сделка опыт игроку (`rewardExp`).
    pub reward_exp: bool,

    pub multiplier: f32,
    pub special: i32,
    pub demand: i32,
}

impl Offer {
    /// Цена первого товара (вики, Trading, «Sale prices», для Java):
    /// `⌊p·(m·max(0, d) + 1)⌋ + special`, не меньше 1 и не больше стопки.
    /// Репутации и «Героя деревни» у нас нет [проверить].
    pub fn cost_a(&self) -> i32 {
        let base = self.buy.count;
        let demand = (base as f32 * self.multiplier * self.demand.max(0) as f32).floor() as i32;

        (base + demand + self.special).clamp(1, blocks::stack_size(self.buy.item))
    }

    /// Распродана ли (вики, Trading: «the trade becomes disabled»).
    pub fn out_of_stock(&self) -> bool {
        self.uses >= self.max_uses
    }

    /// Хватает ли оплаты в ячейках `a` и `b` (в этом порядке).
    fn paid_by(&self, a: Option<Stack>, b: Option<Stack>) -> bool {
        let fits = |want: Option<(i32, i32)>, have: Option<Stack>| match (want, have) {
            (None, _) => true,
            (Some((item, count)), Some(have)) => have.item == item && have.count >= count,
            (Some(_), None) => false,
        };

        fits(Some((self.buy.item, self.cost_a())), a) && fits(self.buy_b.map(|stack| (stack.item, stack.count)), b)
    }

    /// Пополнение у рабочего места: спрос `d + uses − (maxUses − uses)`
    /// (вики, Trading: «subtract the number of possible purchases … and add
    /// twice the number of actually made purchases») [проверить], запас полон.
    pub fn restock(&mut self) {
        self.demand = self.demand + self.uses - (self.max_uses - self.uses);
        self.uses = 0;
    }
}

/// Житель: всё, чего нет у прочих мобов.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Тип (по биому), профессия, уровень 1–5.
    pub kind: u8,
    pub profession: u8,
    pub level: u8,

    /// Опыт жителя (`Xp`).
    pub xp: i32,

    pub offers: Vec<Offer>,

    /// Свойство 20 «villager_finalized» (чёрный ящик, villager-shake: у
    /// призванного с данными без VillagerData — нет, с ними или без данных — да).
    pub finalized: bool,

    /// Сколько тактов ещё мотает головой (свойство 18).
    pub unhappy: i32,

    /// С кем торгует (номер сущности игрока).
    pub trading: Option<i32>,

    /// Такты до повышения уровня и само повышение (вики, Trading).
    pub level_timer: i32,
    pub level_pending: bool,

    /// Занятый рабочий блок.
    pub job_site: Option<(i32, i32, i32)>,

    /// Пополнения: сколько за сегодня, день и такт последнего.
    pub restocks_today: u8,
    pub restock_day: i64,
    pub last_restock: i64,
}

impl State {
    pub fn new() -> Self {
        Self { kind: PLAINS, level: 1, ..Self::default() }
    }

    /// Ключ перевода заголовка окна (чёрный ящик, villager-trade:
    /// `entity.minecraft.villager.farmer`).
    pub fn title(&self) -> String {
        format!("entity.minecraft.villager.{}", PROFESSIONS.get(usize::from(self.profession)).copied().unwrap_or("none"))
    }

    /// Новые сделки уровня `level` из таблицы профессии: две случайные, если в
    /// пуле их больше двух, иначе все (вики, Trading: «If there are more than
    /// two possible trades, two are randomly selected»).
    pub fn add_trades(&mut self, level: u8, rng: &mut Rng) {
        let pool = pool(self.profession, level);
        let mut picks: Vec<usize> = (0..pool.len()).collect();

        if picks.len() > 2 {
            let first = picks.remove(rng.below(picks.len() as u32) as usize);
            let second = picks.remove(rng.below(picks.len() as u32) as usize);

            picks = vec![first, second];
        }

        for index in picks {
            if let Some(offer) = pool[index].make(rng) {
                self.offers.push(offer);
            }
        }
    }

    /// Профессия ушла (рабочий блок сломан у новичка без торговли): сделки
    /// забываются (чёрный ящик, villager-claim: профессия 0).
    pub fn lose_profession(&mut self) {
        self.profession = NONE;
        self.offers.clear();
        self.job_site = None;
    }

    /// Держит ли профессию без рабочего блока: торговал или не новичок (вики,
    /// Trading: «Novice villagers who have not traded can lose their profession»).
    pub fn keeps_profession(&self) -> bool {
        self.xp > 0 || self.level > 1
    }

    /// Пополнить все сделки (вики, Trading: «up to twice per day»).
    pub fn restock(&mut self) {
        for offer in &mut self.offers {
            offer.restock();
        }
    }

    /// Нужно ли пополнение: хоть одна сделка тронута.
    pub fn needs_restock(&self) -> bool {
        self.offers.iter().any(|offer| offer.uses > 0)
    }
}

/// Шаблон сделки из таблицы вики: что хочет (предмет, от и до штук), второе,
/// что отдаёт (предмет, штук), запас, опыт жителю, множитель цены.
#[derive(Clone, Copy, Debug)]
pub struct Template {
    pub buy: (&'static str, i32, i32),
    pub buy_b: Option<(&'static str, i32)>,
    pub sell: (&'static str, i32),
    pub max_uses: i32,
    pub xp: i32,
    pub mult: f32,
}

impl Template {
    fn make(&self, rng: &mut Rng) -> Option<Offer> {
        let (name, low, high) = self.buy;
        let count = low + rng.below((high - low + 1).max(1) as u32) as i32;

        Some(Offer {
            buy: Stack::new(blocks::item_named(name)?, count),
            buy_b: match self.buy_b {
                Some((name, count)) => Some(Stack::new(blocks::item_named(name)?, count)),
                None => None,
            },
            sell: Stack::new(blocks::item_named(self.sell.0)?, self.sell.1),
            uses: 0,
            max_uses: self.max_uses,
            xp: self.xp,
            reward_exp: true,
            multiplier: self.mult,
            special: 0,
            demand: 0,
        })
    }
}

/// Купить у игрока: `n` штук → изумруд.
const fn buy(item: &'static str, n: i32, max_uses: i32, xp: i32) -> Template {
    Template { buy: (item, n, n), buy_b: None, sell: ("emerald", 1), max_uses, xp, mult: 0.05 }
}

/// Продать игроку: `n` изумрудов → `count` штук.
const fn sell(n: i32, item: &'static str, count: i32, max_uses: i32, xp: i32, mult: f32) -> Template {
    Template { buy: ("emerald", n, n), buy_b: None, sell: (item, count), max_uses, xp, mult }
}

/// Продать с ценой от и до (зачарованные вещи: «8 plus the experience cost»,
/// опыт 5–19) — зачарований у нас пока нет, вещь простая [проверить].
const fn sell_range(low: i32, high: i32, item: &'static str, max_uses: i32, xp: i32, mult: f32) -> Template {
    Template { buy: ("emerald", low, high), buy_b: None, sell: (item, 1), max_uses, xp, mult }
}

/// Обмен с доплатой: `n` штук и изумруды → `count` штук.
const fn swap(item: &'static str, n: i32, emeralds: i32, out: &'static str, count: i32, max_uses: i32, xp: i32) -> Template {
    Template { buy: (item, n, n), buy_b: Some(("emerald", emeralds)), sell: (out, count), max_uses, xp, mult: 0.05 }
}

/// Цвета по порядку красителей (вики, Dye).
const COLORS: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

/// «Any color …»: каждый цвет — своя сделка (вики, Trading: «Every color is a
/// unique offer»). Имена предметов собираются один раз.
fn colored(suffix: &str, n: i32, count: i32, max_uses: i32, xp: i32) -> Vec<Template> {
    COLORS
        .iter()
        .map(|color| {
            let name: &'static str = Box::leak(format!("{color}_{suffix}").into_boxed_str());

            sell(n, name, count, max_uses, xp, 0.05)
        })
        .collect()
}

/// Пул сделок профессии на уровне (вики, Trading, таблицы Java Edition).
/// Карты картографа не предлагаются: «the map is not offered if no target
/// could be found», а цели у нас не ищутся [проверить]. Зачарованные вещи,
/// крашеная кожа, отвар и стрелы с эффектом — без довесков [проверить].
pub fn pool(profession: u8, level: u8) -> Vec<Template> {
    use std::sync::OnceLock;

    static TABLE: OnceLock<Vec<Vec<Vec<Template>>>> = OnceLock::new();

    let table = TABLE.get_or_init(|| PROFESSIONS.iter().map(|name| (1..=5).map(|level| pool_of(name, level)).collect()).collect());

    table.get(usize::from(profession)).and_then(|levels| levels.get(usize::from(level).wrapping_sub(1))).cloned().unwrap_or_default()
}

fn pool_of(profession: &str, level: u8) -> Vec<Template> {
    match (profession, level) {
        ("armorer", 1) => vec![
            buy("coal", 15, 16, 2),
            sell(5, "iron_helmet", 1, 12, 1, 0.2),
            sell(9, "iron_chestplate", 1, 12, 1, 0.2),
            sell(7, "iron_leggings", 1, 12, 1, 0.2),
            sell(4, "iron_boots", 1, 12, 1, 0.2),
        ],
        ("armorer", 2) => vec![
            buy("iron_ingot", 4, 12, 10),
            sell(36, "bell", 1, 12, 5, 0.2),
            sell(3, "chainmail_leggings", 1, 12, 5, 0.2),
            sell(1, "chainmail_boots", 1, 12, 5, 0.2),
        ],
        ("armorer", 3) => vec![
            buy("lava_bucket", 1, 12, 20),
            buy("diamond", 1, 12, 20),
            sell(1, "chainmail_helmet", 1, 12, 10, 0.2),
            sell(4, "chainmail_chestplate", 1, 12, 10, 0.2),
            sell(5, "shield", 1, 12, 10, 0.2),
        ],
        ("armorer", 4) => vec![sell_range(19, 33, "diamond_leggings", 3, 15, 0.2), sell_range(13, 27, "diamond_boots", 3, 15, 0.2)],
        ("armorer", 5) => vec![sell_range(13, 27, "diamond_helmet", 3, 30, 0.2), sell_range(21, 35, "diamond_chestplate", 3, 30, 0.2)],
        ("butcher", 1) => vec![buy("chicken", 14, 16, 2), buy("rabbit", 4, 16, 2), buy("porkchop", 7, 16, 2), sell(1, "rabbit_stew", 1, 12, 1, 0.05)],
        ("butcher", 2) => vec![buy("coal", 15, 16, 2), sell(1, "cooked_chicken", 8, 16, 5, 0.05), sell(1, "cooked_porkchop", 5, 16, 5, 0.05)],
        ("butcher", 3) => vec![buy("beef", 10, 16, 20), buy("mutton", 7, 16, 20)],
        ("butcher", 4) => vec![buy("dried_kelp_block", 10, 12, 30)],
        ("butcher", 5) => vec![buy("sweet_berries", 10, 12, 30)],
        ("cartographer", 1) => vec![buy("paper", 24, 12, 2), sell(7, "map", 1, 12, 1, 0.05)],
        ("cartographer", 2) => vec![buy("glass_pane", 11, 12, 10)],
        ("cartographer", 3) => vec![buy("compass", 1, 12, 10)],
        ("cartographer", 4) => vec![sell(7, "item_frame", 1, 12, 15, 0.05), sell(3, "white_banner", 1, 12, 15, 0.05)],
        ("cartographer", 5) => vec![sell(8, "globe_banner_pattern", 1, 12, 30, 0.05)],
        ("cleric", 1) => vec![buy("rotten_flesh", 32, 16, 2), sell(1, "redstone", 2, 12, 1, 0.05)],
        ("cleric", 2) => vec![buy("gold_ingot", 3, 12, 10), sell(1, "lapis_lazuli", 1, 12, 5, 0.05)],
        ("cleric", 3) => vec![buy("rabbit_foot", 2, 12, 20), sell(4, "glowstone", 1, 12, 10, 0.05)],
        ("cleric", 4) => vec![buy("turtle_scute", 4, 12, 30), buy("glass_bottle", 9, 12, 30), sell(5, "ender_pearl", 1, 12, 15, 0.05)],
        ("cleric", 5) => vec![buy("nether_wart", 22, 12, 30), sell(3, "experience_bottle", 1, 12, 30, 0.05)],
        ("farmer", 1) => vec![buy("wheat", 20, 16, 2), buy("potato", 26, 16, 2), buy("carrot", 22, 16, 2), buy("beetroot", 15, 16, 2), sell(1, "bread", 6, 16, 1, 0.05)],
        ("farmer", 2) => vec![buy("pumpkin", 6, 12, 10), sell(1, "pumpkin_pie", 4, 12, 5, 0.05), sell(1, "apple", 4, 16, 5, 0.05)],
        ("farmer", 3) => vec![buy("melon", 4, 12, 20), sell(3, "cookie", 18, 12, 10, 0.05)],
        ("farmer", 4) => vec![sell(1, "suspicious_stew", 1, 12, 15, 0.05), sell(1, "cake", 1, 12, 15, 0.05)],
        ("farmer", 5) => vec![sell(3, "golden_carrot", 3, 12, 30, 0.05), sell(4, "glistering_melon_slice", 3, 12, 30, 0.05)],
        ("fisherman", 1) => vec![buy("string", 20, 16, 2), buy("coal", 10, 16, 2), sell(3, "cod_bucket", 1, 16, 1, 0.05), swap("cod", 6, 1, "cooked_cod", 6, 16, 1)],
        ("fisherman", 2) => vec![buy("cod", 15, 16, 10), sell(2, "campfire", 1, 12, 5, 0.05), swap("salmon", 6, 1, "cooked_salmon", 6, 16, 5)],
        ("fisherman", 3) => vec![buy("salmon", 13, 16, 20), sell_range(8, 22, "fishing_rod", 3, 10, 0.2)],
        ("fisherman", 4) => vec![buy("tropical_fish", 6, 12, 30)],
        // Лодка — порода по биому (вики: «The wanted boat's material varies by biome») [проверить].
        ("fisherman", 5) => vec![buy("pufferfish", 4, 12, 30), buy("oak_boat", 1, 12, 30)],
        ("fletcher", 1) => vec![buy("stick", 32, 16, 2), sell(1, "arrow", 16, 12, 1, 0.05), swap("gravel", 10, 1, "flint", 10, 12, 1)],
        ("fletcher", 2) => vec![buy("flint", 26, 12, 10), sell(2, "bow", 1, 12, 5, 0.05)],
        ("fletcher", 3) => vec![buy("string", 14, 16, 20), sell(3, "crossbow", 1, 12, 10, 0.05)],
        ("fletcher", 4) => vec![buy("feather", 24, 16, 30), sell_range(7, 21, "bow", 3, 15, 0.05)],
        ("fletcher", 5) => vec![
            buy("tripwire_hook", 8, 12, 30),
            sell_range(8, 22, "crossbow", 3, 15, 0.05),
            Template { buy: ("emerald", 2, 2), buy_b: Some(("arrow", 5)), sell: ("tipped_arrow", 5), max_uses: 12, xp: 30, mult: 0.05 },
        ],
        ("leatherworker", 1) => vec![buy("leather", 6, 16, 2), sell(3, "leather_leggings", 1, 12, 1, 0.2), sell(7, "leather_chestplate", 1, 12, 1, 0.2)],
        ("leatherworker", 2) => vec![buy("flint", 26, 12, 10), sell(5, "leather_helmet", 1, 12, 5, 0.2), sell(4, "leather_boots", 1, 12, 5, 0.2)],
        ("leatherworker", 3) => vec![buy("rabbit_hide", 9, 12, 20), sell(7, "leather_chestplate", 1, 12, 10, 0.2)],
        ("leatherworker", 4) => vec![buy("turtle_scute", 4, 12, 30), sell(6, "leather_horse_armor", 1, 12, 15, 0.2)],
        ("leatherworker", 5) => vec![sell(5, "leather_helmet", 1, 12, 30, 0.2), sell(6, "saddle", 1, 12, 30, 0.2)],
        ("librarian", 1) => vec![buy("paper", 24, 16, 2), sell(9, "bookshelf", 1, 12, 1, 0.05), book(1)],
        ("librarian", 2) => vec![buy("book", 4, 12, 10), sell(1, "lantern", 1, 12, 5, 0.05), book(5)],
        ("librarian", 3) => vec![buy("ink_sac", 5, 12, 20), sell(1, "glass", 4, 12, 10, 0.05), book(10)],
        ("librarian", 4) => vec![buy("writable_book", 2, 12, 30), sell(4, "compass", 1, 12, 15, 0.05), sell(5, "clock", 1, 12, 15, 0.05), book(15)],
        ("librarian", 5) => vec![sell(3, "red_candle", 1, 12, 30, 0.05), sell(3, "yellow_candle", 1, 12, 30, 0.05)],
        ("mason", 1) => vec![buy("clay_ball", 10, 16, 2), sell(1, "brick", 10, 16, 1, 0.05)],
        ("mason", 2) => vec![buy("stone", 20, 16, 10), sell(1, "chiseled_stone_bricks", 4, 16, 5, 0.05)],
        ("mason", 3) => vec![
            buy("granite", 16, 16, 20),
            buy("andesite", 16, 16, 20),
            buy("diorite", 16, 16, 20),
            sell(1, "dripstone_block", 4, 16, 10, 0.05),
            sell(1, "polished_andesite", 4, 16, 10, 0.05),
            sell(1, "polished_diorite", 4, 16, 10, 0.05),
            sell(1, "polished_granite", 4, 16, 10, 0.05),
        ],
        ("mason", 4) => {
            let mut pool = vec![buy("quartz", 12, 12, 30)];

            pool.extend(colored("terracotta", 1, 1, 12, 15));
            pool.extend(colored("glazed_terracotta", 1, 1, 12, 15));
            pool
        }
        ("mason", 5) => vec![sell(1, "quartz_pillar", 1, 12, 30, 0.05), sell(1, "quartz_block", 1, 12, 30, 0.05)],
        ("shepherd", 1) => vec![buy("white_wool", 18, 16, 2), buy("brown_wool", 18, 16, 2), buy("black_wool", 18, 16, 2), buy("gray_wool", 18, 16, 2), sell(2, "shears", 1, 12, 1, 0.05)],
        ("shepherd", 2) => {
            let mut pool: Vec<Template> = ["white_dye", "gray_dye", "black_dye", "light_blue_dye", "lime_dye"].iter().map(|dye| buy(dye, 12, 16, 10)).collect();

            pool.extend(colored("wool", 1, 1, 16, 5));
            pool.extend(colored("carpet", 1, 4, 16, 5));
            pool
        }
        ("shepherd", 3) => {
            let mut pool: Vec<Template> = ["yellow_dye", "light_gray_dye", "orange_dye", "red_dye", "pink_dye"].iter().map(|dye| buy(dye, 12, 16, 20)).collect();

            pool.extend(colored("bed", 3, 1, 12, 10));
            pool
        }
        ("shepherd", 4) => {
            let mut pool: Vec<Template> = ["brown_dye", "purple_dye", "blue_dye", "green_dye", "magenta_dye", "cyan_dye"].iter().map(|dye| buy(dye, 12, 16, 30)).collect();

            pool.extend(colored("banner", 3, 1, 12, 15));
            pool
        }
        ("shepherd", 5) => vec![sell(2, "painting", 3, 12, 30, 0.05)],
        ("toolsmith", 1) => vec![
            buy("coal", 15, 16, 2),
            sell(1, "stone_axe", 1, 12, 1, 0.2),
            sell(1, "stone_shovel", 1, 12, 1, 0.2),
            sell(1, "stone_pickaxe", 1, 12, 1, 0.2),
            sell(1, "stone_hoe", 1, 12, 1, 0.2),
        ],
        ("toolsmith", 2) => vec![buy("iron_ingot", 4, 12, 10), sell(36, "bell", 1, 12, 5, 0.2)],
        ("toolsmith", 3) => vec![
            buy("flint", 30, 12, 20),
            sell_range(6, 20, "iron_axe", 3, 10, 0.2),
            sell_range(7, 21, "iron_shovel", 3, 10, 0.2),
            sell_range(8, 22, "iron_pickaxe", 3, 10, 0.2),
            sell(4, "diamond_hoe", 1, 3, 10, 0.2),
        ],
        ("toolsmith", 4) => vec![buy("diamond", 1, 12, 30), sell_range(17, 31, "diamond_axe", 3, 15, 0.2), sell_range(10, 24, "diamond_shovel", 3, 15, 0.2)],
        ("toolsmith", 5) => vec![sell_range(18, 32, "diamond_pickaxe", 3, 30, 0.2)],
        ("weaponsmith", 1) => vec![buy("coal", 15, 16, 2), sell(3, "iron_axe", 1, 12, 1, 0.2), sell_range(7, 21, "iron_sword", 3, 1, 0.05)],
        ("weaponsmith", 2) => vec![buy("iron_ingot", 4, 12, 10), sell(36, "bell", 1, 12, 5, 0.2)],
        ("weaponsmith", 3) => vec![buy("flint", 24, 12, 20)],
        ("weaponsmith", 4) => vec![buy("diamond", 1, 12, 30), sell_range(17, 31, "diamond_axe", 3, 15, 0.2)],
        ("weaponsmith", 5) => vec![sell_range(13, 27, "diamond_sword", 3, 30, 0.2)],
        _ => Vec::new(),
    }
}

/// Книга с чарами у библиотекаря: 5–64 изумруда и книга (вики, Trading:
/// цена по уровню чар) — чар у нас нет, цена во всём размахе [проверить].
const fn book(xp: i32) -> Template {
    Template { buy: ("emerald", 5, 64), buy_b: Some(("book", 1)), sell: ("enchanted_book", 1), max_uses: 12, xp, mult: 0.2 }
}

/// Читает данные жителя из SNBT призыва: `VillagerData:{type,profession,level}`,
/// `Offers:{Recipes:[…]}`, `Xp` (вики, Villager, «Entity data»).
pub fn apply_data(state: &mut State, data: &crate::snbt::Value) {
    if let Some(villager) = data.get("VillagerData") {
        if let Some(kind) = villager.get("type").and_then(|value| value.as_str()).and_then(type_named) {
            state.kind = kind;
        }

        if let Some(profession) = villager.get("profession").and_then(|value| value.as_str()).and_then(profession_named) {
            state.profession = profession;
        }

        if let Some(level) = villager.get("level").and_then(|value| value.as_f64()) {
            state.level = level.clamp(1.0, 5.0) as u8;
        }
    }

    if let Some(xp) = data.get("Xp").and_then(|value| value.as_f64()) {
        state.xp = xp.max(0.0) as i32;
    }

    if let Some(crate::snbt::Value::List(recipes)) = data.get("Offers").and_then(|offers| offers.get("Recipes")) {
        state.offers = recipes.iter().filter_map(read_offer).collect();
    }
}

/// Предмет SNBT `{id:"minecraft:wheat",count:20}` (без `count` — одна штука).
fn read_stack(value: &crate::snbt::Value) -> Option<Stack> {
    let id = value.get("id")?.as_str()?;
    let item = blocks::item_named(id.strip_prefix("minecraft:").unwrap_or(id))?;
    let count = value.get("count").and_then(|count| count.as_f64()).unwrap_or(1.0) as i32;

    Some(Stack::new(item, count.max(1)))
}

fn read_offer(value: &crate::snbt::Value) -> Option<Offer> {
    let number = |key: &str, default: f64| value.get(key).and_then(|field| field.as_f64()).unwrap_or(default);

    Some(Offer {
        buy: read_stack(value.get("buy")?)?,
        buy_b: value.get("buyB").and_then(read_stack),
        sell: read_stack(value.get("sell")?)?,
        uses: number("uses", 0.0) as i32,
        // Вики, Villager, «Entity data»: maxUses по умолчанию 4 [проверить].
        max_uses: number("maxUses", 4.0) as i32,
        xp: number("xp", 1.0) as i32,
        reward_exp: number("rewardExp", 1.0) != 0.0,
        multiplier: number("priceMultiplier", 0.0) as f32,
        special: number("specialPrice", 0.0) as i32,
        demand: number("demand", 0.0) as i32,
    })
}

/// Окно торговли у игрока: ячейки оплаты 0 и 1, выход 2 и выбранная сделка.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Merchant {
    pub villager: i32,
    pub slots: [Option<Stack>; 3],
    pub selected: usize,
}

impl Merchant {
    pub fn new(villager: i32) -> Self {
        Self { villager, ..Self::default() }
    }

    /// Сделка, которую оплачивают ячейки: выбранная, если подходит, иначе
    /// первая подходящая; ячейки — в этом порядке или наоборот. Распроданная
    /// не выходит (вики, Trading).
    pub fn matching(&self, offers: &[Offer]) -> Option<(usize, bool)> {
        let (a, b) = (self.slots[0], self.slots[1]);
        let order = std::iter::once(self.selected).chain(0..offers.len());

        for index in order {
            let Some(offer) = offers.get(index) else {
                continue;
            };

            if offer.out_of_stock() {
                continue;
            }

            if offer.paid_by(a, b) {
                return Some((index, false));
            }

            if offer.paid_by(b, a) {
                return Some((index, true));
            }
        }

        None
    }

    /// Пересчёт ячейки выхода по оплате. Возвращает, сменился ли выход.
    pub fn update_result(&mut self, offers: &[Offer]) -> bool {
        let result = self.matching(offers).map(|(index, _)| offers[index].sell);
        let changed = self.slots[2] != result;

        self.slots[2] = result;
        changed
    }
}

/// Сделка совершилась: оплата снята, сделка тронута. Возвращает номер сделки.
pub fn pay(merchant: &mut Merchant, offers: &mut [Offer]) -> Option<usize> {
    let (index, swapped) = merchant.matching(offers)?;
    let offer = &mut offers[index];
    let (first, second) = if swapped { (1, 0) } else { (0, 1) };
    let take = |slot: &mut Option<Stack>, count: i32| {
        *slot = slot.and_then(|stack| crate::inventory::remainder(stack, stack.count - count));
    };

    take(&mut merchant.slots[first], offer.cost_a());

    if let Some(extra) = offer.buy_b {
        take(&mut merchant.slots[second], extra.count);
    }

    offer.uses += 1;
    Some(index)
}

/// Опыт игроку за сделку: 3–6 и ещё 5, если житель дорос до нового уровня
/// (вики, Trading: «3–6 experience plus an additional 5 experience if the
/// villager levels up»; чёрный ящик, villager-levelup: шарик 11 у новичка с
/// опытом 10).
pub fn reward(rng: &mut Rng, levels_up: bool) -> i32 {
    3 + rng.below(4) as i32 + if levels_up { 5 } else { 0 }
}

/// Как часто безработный ищет рабочий блок: раз в столько тактов в среднем
/// (чёрный ящик, villager-claim: занял через 12 и 20 тактов после
/// появления блока, villager-levelup-ai — через 7 и 16 после призыва) [проверить].
pub const CLAIM_CHANCE: u32 = 10;

/// Где ищется рабочий блок: по горизонтали и вверх-вниз (вики, Villager: «48
/// blocks» — у нас ближе, ради скорости) [проверить].
const CLAIM_RANGE: i32 = 16;
const CLAIM_HEIGHT: i32 = 4;

/// С какого расстояния (до середины блока) житель занимает рабочий блок
/// (вики, Villager: подходит на 2 блока) [проверить].
const CLAIM_REACH: f64 = 2.0;

/// Рабочие часы, когда житель пополняет сделки у рабочего блока (вики,
/// Villager, «Schedules»: работа 2000–9000) [проверить], и пауза между
/// пополнениями [проверить].
const WORK_HOURS: std::ops::Range<i64> = 2000..9000;
const RESTOCK_GAP: i64 = 2400;

/// Такт жителя с ИИ сверх общего (mob/ai.rs): рабочий блок — потерять,
/// найти, занять; уровень; пополнение. `clock` — часы мира от начала (день ×
/// 24000 + час), `claimed` — чужие рабочие блоки.
pub fn step(mob: &mut crate::mob::Mob, view: &dyn crate::mob::View, clock: i64, claimed: &[(i32, i32, i32)], rng: &mut Rng) {
    let state = &mut mob.villager;

    // Рабочий блок сломан или заменён — он больше не наш (чёрный ящик,
    // villager-claim: профессия ушла в такте после замены блока).
    if let Some((x, y, z)) = state.job_site
        && view.loaded(x.div_euclid(16), z.div_euclid(16))
        && blocks::block_at_state(view.block(x, y, z)).and_then(profession_of_block) != Some(state.profession)
    {
        state.job_site = None;
    }

    // Новичок без торговли без рабочего блока теряет профессию (чёрный ящик,
    // villager-levelup-ai: призванный крестьянин без опыта — на первом такте).
    if trades(state.profession) && state.job_site.is_none() && !state.keeps_profession() {
        state.lose_profession();
    }

    if state.job_site.is_none() && state.profession != NITWIT && mob.age >= 0 && rng.below(CLAIM_CHANCE) == 0 {
        claim(mob, view, claimed);
    }

    let state = &mut mob.villager;

    // Новый уровень — через 40 тактов без торговли после сделки (чёрный
    // ящик, villager-levelup-ai), с регенерацией на 10 секунд (вики, Trading).
    if state.trading.is_none() && state.level_timer > 0 {
        state.level_timer -= 1;

        if state.level_timer == 0 {
            // Регенерация — только с новым уровнем (чёрный ящик,
            // villager-trade-ai: после сделки без повышения частиц нет).
            if std::mem::take(&mut state.level_pending) && state.level < 5 {
                state.level += 1;

                let level = state.level;

                state.add_trades(level, rng);
                mob.vitals.give_effect_shown(crate::survival::Effect { kind: crate::survival::EffectKind::Regeneration, amplifier: 0, ticks_left: LEVEL_UP_REGEN, ambient: false, visible: true });
            }
        }
    }

    restock(mob, clock);
}

/// Поиск свободного рабочего блока (своей профессии, если она есть) и шаг
/// к нему; рядом — занять: частицы (событие 14) и профессия (чёрный ящик,
/// villager-claim: событие 14, свойство 19 следующим тактом).
fn claim(mob: &mut crate::mob::Mob, view: &dyn crate::mob::View, claimed: &[(i32, i32, i32)]) {
    let (cx, cy, cz) = mob.cell();
    let mut best: Option<((i32, i32, i32), u8, f64)> = None;

    for y in cy - CLAIM_HEIGHT..=cy + CLAIM_HEIGHT {
        for x in cx - CLAIM_RANGE..=cx + CLAIM_RANGE {
            for z in cz - CLAIM_RANGE..=cz + CLAIM_RANGE {
                let Some(profession) = blocks::block_at_state(view.block(x, y, z)).and_then(profession_of_block) else {
                    continue;
                };

                if (mob.villager.profession != NONE && profession != mob.villager.profession) || claimed.contains(&(x, y, z)) {
                    continue;
                }

                let dist = (f64::from(x) + 0.5 - mob.x).powi(2) + (f64::from(y) + 0.5 - mob.y).powi(2) + (f64::from(z) + 0.5 - mob.z).powi(2);

                if best.is_none_or(|(_, _, known)| dist < known) {
                    best = Some(((x, y, z), profession, dist));
                }
            }
        }
    }

    let Some((site, profession, dist)) = best else {
        return;
    };

    if dist.sqrt() > CLAIM_REACH + 0.5 {
        // Далеко — идти к нему (вики, Villager: «pathfind to it»).
        let route = crate::mob::path::find(view, (cx, cy, cz), site, crate::mob::path::MAX_NODES);

        mob.brain.go(route, 0.5);
        return;
    }

    let state = &mut mob.villager;

    state.job_site = Some(site);

    if state.profession == NONE {
        state.profession = profession;
    }

    if state.offers.is_empty() {
        let mut rng = Rng::new(u64::from(mob.entity_id as u32) ^ mob.ticks.wrapping_mul(0x51));

        for level in 1..=state.level {
            state.add_trades(level, &mut rng);
        }
    }

    mob.events.push(crate::mob::Event::Status(14));
}

/// Пополнение у рабочего блока до двух раз в день (вики, Trading: «When
/// villagers work at their job site blocks, they activate their offers again,
/// up to twice per day») [проверить: часы и звук].
fn restock(mob: &mut crate::mob::Mob, clock: i64) {
    let state = &mut mob.villager;
    let Some((x, _, z)) = state.job_site else {
        return;
    };

    let day = clock.div_euclid(24_000);

    if state.restock_day != day {
        state.restock_day = day;
        state.restocks_today = 0;
    }

    let near = (f64::from(x) + 0.5 - mob.x).abs() < 1.5 && (f64::from(z) + 0.5 - mob.z).abs() < 1.5;

    if !near || !WORK_HOURS.contains(&clock.rem_euclid(24_000)) || state.restocks_today >= 2 || !state.needs_restock() || clock - state.last_restock < RESTOCK_GAP {
        return;
    }

    state.restock();
    state.restocks_today += 1;
    state.last_restock = clock;

    if let Some(sound) = work_sound(state.profession) {
        mob.events.push(crate::mob::Event::Sound { name: sound, volume: 1.0, pitch: 1.0 });
    }
}

/// Звук работы профессии (вики, Villager, «Sounds»: `entity.villager.work_*`).
pub fn work_sound(profession: u8) -> Option<&'static str> {
    const SOUNDS: [&str; 15] = [
        "",
        "minecraft:entity.villager.work_armorer",
        "minecraft:entity.villager.work_butcher",
        "minecraft:entity.villager.work_cartographer",
        "minecraft:entity.villager.work_cleric",
        "minecraft:entity.villager.work_farmer",
        "minecraft:entity.villager.work_fisherman",
        "minecraft:entity.villager.work_fletcher",
        "minecraft:entity.villager.work_leatherworker",
        "minecraft:entity.villager.work_librarian",
        "minecraft:entity.villager.work_mason",
        "",
        "minecraft:entity.villager.work_shepherd",
        "minecraft:entity.villager.work_toolsmith",
        "minecraft:entity.villager.work_weaponsmith",
    ];

    SOUNDS.get(usize::from(profession)).copied().filter(|sound| !sound.is_empty())
}

/// Запись жителя для файла региона (хвост записи моба).
pub fn write(state: &State, out: &mut Vec<u8>) {
    out.extend_from_slice(&[state.kind, state.profession, state.level, u8::from(state.finalized)]);
    out.extend_from_slice(&state.xp.to_be_bytes());

    let site = state.job_site.unwrap_or((i32::MIN, 0, 0));

    for value in [site.0, site.1, site.2] {
        out.extend_from_slice(&value.to_be_bytes());
    }

    out.push(state.restocks_today);
    out.extend_from_slice(&state.restock_day.to_be_bytes());
    out.extend_from_slice(&state.last_restock.to_be_bytes());
    out.extend_from_slice(&(state.offers.len() as u16).to_be_bytes());

    for offer in &state.offers {
        let b = offer.buy_b.unwrap_or(Stack::new(0, 0));

        for value in [offer.buy.item, offer.buy.count, b.item, b.count, offer.sell.item, offer.sell.count, offer.uses, offer.max_uses, offer.xp, offer.special, offer.demand] {
            out.extend_from_slice(&value.to_be_bytes());
        }

        out.extend_from_slice(&offer.multiplier.to_be_bytes());
        out.push(u8::from(offer.reward_exp));
    }
}

/// Читает запись `write`; None — она испорчена.
pub fn read(bytes: &mut &[u8]) -> Option<State> {
    let mut take = |count: usize| -> Option<&[u8]> {
        let (head, rest) = bytes.split_at_checked(count)?;
        *bytes = rest;
        Some(head)
    };
    let head = take(4)?.to_vec();
    let mut int = || -> Option<i32> { Some(i32::from_be_bytes(take(4)?.try_into().ok()?)) };
    let xp = int()?;
    let site = (int()?, int()?, int()?);
    let restocks_today = take(1)?[0];
    let restock_day = i64::from_be_bytes(take(8)?.try_into().ok()?);
    let last_restock = i64::from_be_bytes(take(8)?.try_into().ok()?);
    let count = u16::from_be_bytes(take(2)?.try_into().ok()?);
    let mut offers = Vec::with_capacity(usize::from(count));

    for _ in 0..count {
        let mut v = [0i32; 11];

        for value in &mut v {
            *value = i32::from_be_bytes(take(4)?.try_into().ok()?);
        }

        let multiplier = f32::from_be_bytes(take(4)?.try_into().ok()?);
        let reward_exp = take(1)?[0] != 0;

        offers.push(Offer {
            buy: Stack::new(v[0], v[1]),
            buy_b: (v[3] > 0).then(|| Stack::new(v[2], v[3])),
            sell: Stack::new(v[4], v[5]),
            uses: v[6],
            max_uses: v[7],
            xp: v[8],
            special: v[9],
            demand: v[10],
            multiplier,
            reward_exp,
        });
    }

    Some(State {
        kind: head[0].min(TYPES.len() as u8 - 1),
        profession: head[1].min(PROFESSIONS.len() as u8 - 1),
        level: head[2].clamp(1, 5),
        finalized: head[3] != 0,
        xp,
        offers,
        job_site: (site.0 != i32::MIN).then_some(site),
        restocks_today,
        restock_day,
        last_restock,
        ..State::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(name: &str, count: i32) -> Stack {
        Stack::new(blocks::item_named(name).unwrap(), count)
    }

    fn offer(buy: Stack, sell: Stack) -> Offer {
        Offer { buy, buy_b: None, sell, uses: 0, max_uses: 16, xp: 2, reward_exp: true, multiplier: 0.05, special: 0, demand: 0 }
    }

    /// Номера типов и профессий — как у оригинала (чёрный ящик).
    #[test]
    fn registry_numbers_match_vanilla() {
        assert_eq!(type_named("minecraft:plains"), Some(2));
        assert_eq!(type_named("snow"), Some(4));
        assert_eq!(type_named("minecraft:taiga"), Some(6));
        assert_eq!(profession_named("minecraft:farmer"), Some(5));
        assert_eq!(profession_named("nitwit"), Some(11));
        assert_eq!(profession_of_block("composter"), Some(5));
        assert_eq!(profession_of_block("water_cauldron"), profession_named("leatherworker"));
        assert_eq!(profession_of_block("stone"), None);
        assert!(!trades(NONE) && !trades(NITWIT) && trades(5));
    }

    /// Все предметы таблиц есть в реестре, у каждой профессии на каждом уровне
    /// есть сделки (кроме безработного и бестолкового).
    #[test]
    fn every_pool_is_known_and_filled() {
        let mut rng = Rng::new(7);

        for profession in 0..PROFESSIONS.len() as u8 {
            for level in 1..=5 {
                let pool = pool(profession, level);

                assert_eq!(pool.is_empty(), !trades(profession), "{} {}", PROFESSIONS[usize::from(profession)], level);

                for template in pool {
                    assert!(template.make(&mut rng).is_some(), "{:?}", template);
                }
            }
        }
    }

    /// Пороги уровней: 10, 70, 150, 250; мастер не растёт.
    #[test]
    fn level_thresholds() {
        assert!(!should_level_up(1, 9));
        assert!(should_level_up(1, 10));
        assert!(!should_level_up(2, 69));
        assert!(should_level_up(4, 250));
        assert!(!should_level_up(5, 10_000));
    }

    /// Цена: спрос поднимает первую цену на `p·m·d`, вниз до 1, вверх до стопки.
    #[test]
    fn price_follows_demand() {
        let mut deal = offer(stack("wheat", 20), stack("emerald", 1));

        assert_eq!(deal.cost_a(), 20);
        deal.demand = 10;
        assert_eq!(deal.cost_a(), 30);
        deal.demand = -5;
        assert_eq!(deal.cost_a(), 20);
        deal.special = -30;
        assert_eq!(deal.cost_a(), 1);
        deal.special = 100;
        assert_eq!(deal.cost_a(), 64);
    }

    /// Пополнение: запас полон, спрос `d + uses − (max − uses)`.
    #[test]
    fn restock_resets_uses_and_moves_demand() {
        let mut deal = offer(stack("wheat", 20), stack("emerald", 1));

        deal.uses = 12;
        deal.restock();
        assert_eq!((deal.uses, deal.demand), (0, 8));
    }

    /// Окно: оплата в любой ячейке, выход — товар сделки; сделка снимает цену.
    #[test]
    fn merchant_pays_from_either_slot() {
        let mut offers = vec![offer(stack("wheat", 20), stack("emerald", 1)), offer(stack("emerald", 1), stack("bread", 6))];
        let mut merchant = Merchant::new(1);

        merchant.slots[1] = Some(stack("wheat", 64));
        assert!(merchant.update_result(&offers));
        assert_eq!(merchant.slots[2], Some(stack("emerald", 1)));
        assert_eq!(pay(&mut merchant, &mut offers), Some(0));
        assert_eq!(merchant.slots[1], Some(stack("wheat", 44)));
        assert_eq!(offers[0].uses, 1);

        merchant.slots[1] = Some(stack("wheat", 4));
        assert!(merchant.update_result(&offers));
        assert_eq!(merchant.slots[2], None);
        assert_eq!(pay(&mut merchant, &mut offers), None);
    }

    /// Распроданная сделка не выходит.
    #[test]
    fn sold_out_offer_gives_nothing() {
        let mut offers = vec![offer(stack("wheat", 20), stack("emerald", 1))];
        let mut merchant = Merchant::new(1);

        offers[0].uses = 16;
        merchant.slots[0] = Some(stack("wheat", 64));
        merchant.update_result(&offers);
        assert_eq!(merchant.slots[2], None);
    }

    /// Данные призыва: тип, профессия, уровень, опыт, сделки со всеми полями.
    #[test]
    fn summon_data_is_read() {
        let data = crate::snbt::parse(
            "{Xp:5,VillagerData:{type:\"minecraft:taiga\",profession:\"minecraft:farmer\",level:2},Offers:{Recipes:[{buy:{id:\"minecraft:wheat\",count:20},buyB:{id:\"minecraft:book\",count:1},sell:{id:\"minecraft:emerald\",count:1},maxUses:16,uses:3,xp:2,priceMultiplier:0.05f,rewardExp:0b,demand:4}]}}",
        )
        .unwrap();
        let mut state = State::new();

        apply_data(&mut state, &data);
        assert_eq!((state.kind, state.profession, state.level, state.xp), (6, 5, 2, 5));
        assert_eq!(state.offers.len(), 1);

        let deal = &state.offers[0];

        assert_eq!((deal.uses, deal.max_uses, deal.xp, deal.reward_exp, deal.demand), (3, 16, 2, false, 4));
        assert_eq!(deal.buy_b, Some(stack("book", 1)));
        assert!((deal.multiplier - 0.05).abs() < 1e-6);
    }

    /// Новые сделки уровня: две из пула больше двух, все из меньшего.
    #[test]
    fn two_trades_are_picked_per_level() {
        let mut rng = Rng::new(3);
        let mut state = State { profession: 5, ..State::new() };

        state.add_trades(1, &mut rng);
        assert_eq!(state.offers.len(), 2);
        assert_ne!(state.offers[0], state.offers[1]);
        state.add_trades(3, &mut rng);
        assert_eq!(state.offers.len(), 4);
    }

    /// Запись в файл и чтение назад сохраняют жителя.
    #[test]
    fn state_round_trips() {
        let mut rng = Rng::new(5);
        let mut state = State { profession: 5, xp: 12, level: 2, job_site: Some((1, -60, 3)), finalized: true, ..State::new() };

        state.add_trades(1, &mut rng);
        state.offers[0].uses = 3;

        let mut bytes = Vec::new();

        write(&state, &mut bytes);

        let back = read(&mut bytes.as_slice()).unwrap();

        assert_eq!((back.kind, back.profession, back.level, back.xp, back.job_site, back.finalized), (2, 5, 2, 12, Some((1, -60, 3)), true));
        assert_eq!(back.offers, state.offers);
    }

    /// Опыт игроку: 3–6, при повышении ещё 5.
    #[test]
    fn player_reward_range() {
        let mut rng = Rng::new(11);

        for _ in 0..100 {
            assert!((3..=6).contains(&reward(&mut rng, false)));
            assert!((8..=11).contains(&reward(&mut rng, true)));
        }
    }
}
