// Довески стопки: прочность, зачарования, свои модификаторы атрибутов.
//
// Стопка (`inventory::Stack`) — маленькая и копируемая, а довесков у неё
// может быть много. Поэтому довески лежат в общей таблице, а в стопке —
// только номер записи (`DataId`). Одинаковые довески — одна запись, так что
// сравнение стопок остаётся сравнением чисел, а стопки с разной прочностью
// или зачарованиями не сливаются. Запись в таблице не меняется: «поправить»
// довески — значит завести (или найти) другую запись.
//
// Клиенту довески уходят компонентами предмета (`damage`, `enchantments`,
// `attribute_modifiers`, `unbreakable`); номера компонентов и формат — 775,
// SlotComponent в protocol.json.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::attributes::{self, Modifier, Operation};
use crate::network::packet::decode_varint;
use crate::network::varint::{encode_string, encode_varint};
use crate::snbt::{self, Value};

/// Номера компонентов предмета в 775 (registries.json, data_component_type).
pub const COMPONENT_UNBREAKABLE: i32 = 4;
pub const COMPONENT_DAMAGE: i32 = 3;
pub const COMPONENT_ENCHANTMENTS: i32 = 13;
/// Своё имя (переименование на наковальне), цена прошлых работ на
/// наковальне и зачарования книги (protocol.json: 6, 19, 42).
pub const COMPONENT_CUSTOM_NAME: i32 = 6;
pub const COMPONENT_REPAIR_COST: i32 = 19;
pub const COMPONENT_STORED_ENCHANTMENTS: i32 = 42;
pub const COMPONENT_ATTRIBUTE_MODIFIERS: i32 = 16;
pub const COMPONENT_POTION_CONTENTS: i32 = 51;
pub const COMPONENT_STEW: i32 = 53;
pub const COMPONENT_INSTRUMENT: i32 = 61;
pub const HORNS: [&str; 8] = ["ponder_goat_horn","sing_goat_horn","seek_goat_horn","feel_goat_horn","admire_goat_horn","call_goat_horn","yearn_goat_horn","dream_goat_horn"];
/// Отделка брони (`trim`): материал и узор — записи реестров (26.1 protocol.json).
pub const COMPONENT_TRIM: i32 = 56;
/// Заряженные снаряды арбалета (minecraft-data, protocol.json: 49).
pub const COMPONENT_CHARGED_PROJECTILES: i32 = 49;
/// Снаряд, который нельзя подобрать (minecraft-data, protocol.json: 22).
pub const COMPONENT_INTANGIBLE_PROJECTILE: i32 = 22;
/// Взрыв звезды фейерверка и ракета: полёт и взрывы (protocol.json: 68, 69).
pub const COMPONENT_FIREWORK_EXPLOSION: i32 = 68;
pub const COMPONENT_FIREWORKS: i32 = 69;
/// Уровень зловещей бутылки и узор зловещего флага с его именем, подсказкой и
/// редкостью (protocol.json: ominous_bottle_amplifier 63, banner_patterns 72,
/// tooltip_display 18, item_name 9, rarity 12).
pub const COMPONENT_OMINOUS_AMPLIFIER: i32 = 63;
pub const COMPONENT_BANNER_PATTERNS: i32 = 72;
pub const COMPONENT_TOOLTIP_DISPLAY: i32 = 18;
pub const COMPONENT_ITEM_NAME: i32 = 9;
pub const COMPONENT_RARITY: i32 = 12;

/// Слои зловещего флага: узор (номер в реестре banner_pattern) и цвет
/// (вики, Ominous Banner: голубой ромб, светло-серое основание, серый столб,
/// светло-серая кайма, чёрный пояс, светло-серая верхняя половина,
/// светло-серый круг, чёрная кайма; чёрный ящик, evoker-fangs: 23/9 31/8
/// 32/7 1/8 36/15 17/8 3/8 1/15).
pub const OMINOUS_BANNER_LAYERS: [(i32, i32); 8] = [(23, 9), (31, 8), (32, 7), (1, 8), (36, 15), (17, 8), (3, 8), (1, 15)];
/// Содержимое шулкер-ящика (`container`, registries.json: 75) клиенту пока
/// не шлётся — см. `encode_components`.
#[allow(dead_code)]
pub const COMPONENT_CONTAINER: i32 = 75;

/// Одна занятая ячейка содержимого шулкер-ящика в предмете. Хранится
/// плоско (номер предмета, число, номер довесков), чтобы довески оставались
/// сравнимыми и хешируемыми; вложенные довески лежат в той же таблице.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Content {
    pub slot: u8,
    pub item: i32,
    pub count: i32,
    pub data: Option<DataId>,
}

/// Формы взрыва (вики, Data component format: firework_explosion), номер — место.
pub const EXPLOSION_SHAPES: [&str; 5] = ["small_ball", "large_ball", "star", "creeper", "burst"];

/// Один взрыв фейерверка (`firework_explosion`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Explosion {
    /// Форма: номер в `EXPLOSION_SHAPES`.
    pub shape: u8,
    pub colors: Vec<i32>,
    pub fade_colors: Vec<i32>,
    pub trail: bool,
    pub twinkle: bool,
}

/// Ракета (`fireworks`): длительность полёта и взрывы. У предмета ракеты по
/// умолчанию полёт 1 и взрывов нет — такие довески не хранятся.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Fireworks {
    pub flight: i8,
    pub explosions: Vec<Explosion>,
}

impl Default for Fireworks {
    fn default() -> Self {
        Self { flight: 1, explosions: Vec::new() }
    }
}

/// Номер записи в таблице довесков.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DataId(u32);

/// Свой модификатор предмета (`attribute_modifiers`): заменяет прежние,
/// «по умолчанию».
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ItemModifier {
    pub attribute: i32,
    pub id: String,
    /// Величина двойной точности — битами, чтобы запись была сравнимой.
    pub amount_bits: u64,
    pub operation: u8,
    /// Группа мест: 0 любое, 1 основная рука, 2 вторая, 3 обе руки, 4–7 обувь…шлем,
    /// 8 вся броня, 9 тело.
    pub slot: u8,
}

impl ItemModifier {
    pub fn amount(&self) -> f64 {
        f64::from_bits(self.amount_bits)
    }

    pub fn modifier(&self) -> Modifier {
        Modifier { id: self.id.clone(), amount: self.amount(), operation: Operation::from_number(i32::from(self.operation)).unwrap_or(Operation::AddValue) }
    }
}

/// Материалы отделки по номерам реестра `trim_material` (по алфавиту, как у
/// оригинала: чёрный ящик smithing-trim — золото 4, алмаз 2).
pub const TRIM_MATERIALS: [&str; 11] =
    ["amethyst", "copper", "diamond", "emerald", "gold", "iron", "lapis", "netherite", "quartz", "redstone", "resin"];

/// Узоры отделки по номерам реестра `trim_pattern` (по алфавиту; sentry — 8,
/// чёрный ящик smithing-trim).
pub const TRIM_PATTERNS: [&str; 18] = [
    "bolt", "coast", "dune", "eye", "flow", "host", "raiser", "rib", "sentry", "shaper", "silence", "snout", "spire", "tide", "vex", "ward",
    "wayfinder", "wild",
];

/// Довески одной стопки.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Data {
    /// Номер заполненной карты (minecraft:map_id, protocol.json: 46).
    pub map_id: Option<i32>,
    /// Износ (компонент `damage`).
    pub damage: i32,
    /// Не изнашивается.
    pub unbreakable: bool,
    /// Зачарования: номер по таблице `ENCHANTMENTS` и уровень, по номеру.
    pub enchants: Vec<(u16, i32)>,
    /// Свои модификаторы атрибутов вместо тех, что у предмета по умолчанию.
    pub attributes: Option<Vec<ItemModifier>>,
    /// Зелье (компонент `potion_contents`): номер по таблице
    /// `survival::POTIONS`. Свой цвет и свои эффекты пока не ведутся.
    pub potion: Option<u16>,
    /// Сколько стрел заряжено в арбалет (компонент `charged_projectiles`):
    /// 0 — не заряжен; с «залпом» заряжается одна, а в компоненте — три
    /// [проверить чёрным ящиком, crossbow-multishot].
    pub charged: u8,
    /// Заряжены ракеты, а не стрелы (их довески; полёт 1 без взрывов —
    /// обычная ракета).
    pub charged_rocket: Option<Fireworks>,
    /// Ракета: полёт и взрывы (компонент `fireworks`), если не по умолчанию.
    pub fireworks: Option<Fireworks>,
    /// Звезда: её взрыв (компонент `firework_explosion`).
    pub explosion: Option<Explosion>,
    /// Зачарования зачарованной книги (`stored_enchantments`), как `enchants`.
    pub stored: Vec<(u16, i32)>,
    /// Цена прошлых работ на наковальне (`repair_cost`, вики, Anvil
    /// mechanics: «prior work penalty»).
    pub repair_cost: i32,
    /// Своё имя предмета (`custom_name`) простым текстом.
    pub name: Option<String>,
    /// Отделка брони (`trim`): номер материала и узора по `TRIM_MATERIALS` и
    /// `TRIM_PATTERNS`.
    pub trim: Option<(u8, u8)>,
    /// Содержимое шулкер-ящика (`container`): занятые ячейки по порядку.
    pub contents: Vec<Content>,
    /// Зловещая бутылка: уровень «Дурного знамения» 0…4
    /// (`ominous_bottle_amplifier`, вики, Ominous Bottle).
    pub ominous: Option<u8>,
    /// Зловещий флаг (вики, Ominous Banner): узор, имя, редкость.
    pub ominous_banner: bool,
    /// Эффекты подозрительного рагу: номер эффекта и длительность.
    pub stew: Vec<(u8, i32)>,
    /// Козий рог: запись реестра instrument.
    pub instrument: Option<u8>,
    pub bucket: Option<crate::mob::water::bucket::BucketData>,
    pub mob_variants: Vec<(i32,i32)>,
}

impl Data {
    /// Довески ракеты (по умолчанию — полёт 1 без взрывов).
    pub fn rocket(&self) -> Fireworks {
        self.fireworks.clone().unwrap_or_default()
    }

    /// Ставит довески ракеты, не храня те, что по умолчанию.
    pub fn set_rocket(&mut self, fireworks: Fireworks) {
        self.fireworks = (fireworks != Fireworks::default()).then_some(fireworks);
    }

    pub fn is_default(&self) -> bool {
        *self == Data::default()
    }

    /// Уровень зачарования по имени (0 — нет).
    pub fn level(&self, name: &str) -> i32 {
        enchantment_number(name).and_then(|number| self.enchants.iter().find(|(known, _)| *known == number)).map_or(0, |(_, level)| *level)
    }

    /// Ставит зачарование (уровень 0 — снимает), держа список по номерам.
    pub fn set_enchant(&mut self, number: u16, level: i32) {
        put_enchant(&mut self.enchants, number, level);
    }

    /// То же для зачарований книги (`stored_enchantments`).
    pub fn set_stored(&mut self, number: u16, level: i32) {
        put_enchant(&mut self.stored, number, level);
    }
}

/// Ставит зачарование в список (уровень 0 — снимает). Порядок — порядок
/// добавления, как у оригинала: стол зачарования шлёт зачарования в порядке
/// выбора (чёрный ящик, enchant-sword: 35, 10, 18); прежнее остаётся на
/// своём месте.
fn put_enchant(list: &mut Vec<(u16, i32)>, number: u16, level: i32) {
    match list.iter().position(|(known, _)| *known == number) {
        Some(at) if level > 0 => list[at].1 = level,
        Some(at) => {
            list.remove(at);
        }
        None if level > 0 => list.push((number, level)),
        None => {}
    }
}

struct Table {
    entries: Vec<Data>,
    index: HashMap<Data, u32>,
}

fn table() -> &'static Mutex<Table> {
    static TABLE: OnceLock<Mutex<Table>> = OnceLock::new();

    TABLE.get_or_init(|| Mutex::new(Table { entries: Vec::new(), index: HashMap::new() }))
}

/// Номер записи для этих довесков; у пустых довесков номера нет.
pub fn intern(data: Data) -> Option<DataId> {
    if data.is_default() {
        return None;
    }

    let mut table = table().lock().expect("таблица довесков захвачена другим потоком");

    if let Some(number) = table.index.get(&data) {
        return Some(DataId(*number));
    }

    let number = table.entries.len() as u32;

    table.entries.push(data.clone());
    table.index.insert(data, number);

    Some(DataId(number))
}

/// Довески по номеру записи.
pub fn get(id: DataId) -> Data {
    table().lock().expect("таблица довесков захвачена другим потоком").entries.get(id.0 as usize).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Зачарования
// ---------------------------------------------------------------------------

/// Зачарования 26.1.2: имя и наибольший уровень (вики, страницы зачарований).
/// Номер — место в списке: так они идут клиенту в реестре
/// `minecraft:enchantment` (configuration.rs).
pub const ENCHANTMENTS: [(&str, i32); 43] = [
    ("aqua_affinity", 1),
    ("bane_of_arthropods", 5),
    ("binding_curse", 1),
    ("blast_protection", 4),
    ("breach", 4),
    ("channeling", 1),
    ("density", 5),
    ("depth_strider", 3),
    ("efficiency", 5),
    ("feather_falling", 4),
    ("fire_aspect", 2),
    ("fire_protection", 4),
    ("flame", 1),
    ("fortune", 3),
    ("frost_walker", 2),
    ("impaling", 5),
    ("infinity", 1),
    ("knockback", 2),
    ("looting", 3),
    ("loyalty", 3),
    ("luck_of_the_sea", 3),
    ("lunge", 3),
    ("lure", 3),
    ("mending", 1),
    ("multishot", 1),
    ("piercing", 4),
    ("power", 5),
    ("projectile_protection", 4),
    ("protection", 4),
    ("punch", 2),
    ("quick_charge", 3),
    ("respiration", 3),
    ("riptide", 3),
    ("sharpness", 5),
    ("silk_touch", 1),
    ("smite", 5),
    ("soul_speed", 3),
    ("sweeping_edge", 3),
    ("swift_sneak", 3),
    ("thorns", 3),
    ("unbreaking", 3),
    ("vanishing_curse", 1),
    ("wind_burst", 3),
];

pub fn enchantment_number(name: &str) -> Option<u16> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);

    ENCHANTMENTS.iter().position(|(known, _)| *known == name).map(|index| index as u16)
}


// ---------------------------------------------------------------------------
// Компоненты для клиента
// ---------------------------------------------------------------------------

/// Компоненты, которые надо добавить к предмету, чтобы клиент увидел довески:
/// число компонентов и их тела (без числа «убранных»).
pub fn encode_components(data: &Data) -> (i32, Vec<u8>) {
    let mut count = 0;
    let mut out = Vec::new();
    count += crate::mob::water::bucket::encode(data, &mut out);

    if !data.stew.is_empty() {
        out.extend_from_slice(&encode_varint(COMPONENT_STEW));
        out.extend_from_slice(&encode_varint(data.stew.len() as i32));
        for (id,ticks) in &data.stew { out.extend_from_slice(&encode_varint(i32::from(*id))); out.extend_from_slice(&encode_varint(*ticks)); }
        count += 1;
    }
    if let Some(id)=data.instrument {
        out.extend_from_slice(&encode_varint(COMPONENT_INSTRUMENT)); out.push(1);
        out.extend_from_slice(&encode_varint(i32::from(id)+1)); count += 1;
    }
    if let Some(id) = data.map_id { out.extend_from_slice(&encode_varint(46)); out.extend_from_slice(&encode_varint(id)); count += 1; }
    if data.damage > 0 {
        out.extend_from_slice(&encode_varint(COMPONENT_DAMAGE));
        out.extend_from_slice(&encode_varint(data.damage));
        count += 1;
    }

    if data.unbreakable {
        out.extend_from_slice(&encode_varint(COMPONENT_UNBREAKABLE));
        count += 1;
    }

    if let Some(name) = &data.name {
        out.extend_from_slice(&encode_varint(COMPONENT_CUSTOM_NAME));
        out.extend_from_slice(&crate::network::nbt::text_component(name));
        count += 1;
    }

    if data.repair_cost != 0 {
        out.extend_from_slice(&encode_varint(COMPONENT_REPAIR_COST));
        out.extend_from_slice(&encode_varint(data.repair_cost));
        count += 1;
    }

    if !data.stored.is_empty() {
        out.extend_from_slice(&encode_varint(COMPONENT_STORED_ENCHANTMENTS));
        out.extend_from_slice(&encode_varint(data.stored.len() as i32));

        for (number, level) in &data.stored {
            out.extend_from_slice(&encode_varint(i32::from(*number)));
            out.extend_from_slice(&encode_varint(*level));
        }

        count += 1;
    }

    if !data.enchants.is_empty() {
        out.extend_from_slice(&encode_varint(COMPONENT_ENCHANTMENTS));
        out.extend_from_slice(&encode_varint(data.enchants.len() as i32));

        for (number, level) in &data.enchants {
            out.extend_from_slice(&encode_varint(i32::from(*number)));
            out.extend_from_slice(&encode_varint(*level));
        }

        count += 1;
    }

    if data.charged > 0
        && let Some(rocket) = &data.charged_rocket
        && let Some(item) = crate::blocks::item_named("firework_rocket")
    {
        // Ракеты: у каждой свои довески `fireworks` (если не по умолчанию),
        // боковые «залпа» — ещё и `intangible_projectile` [проверить порядок].
        out.extend_from_slice(&encode_varint(COMPONENT_CHARGED_PROJECTILES));
        out.extend_from_slice(&encode_varint(i32::from(data.charged)));

        let own = *rocket != Fireworks::default();

        for index in 0..data.charged {
            out.extend_from_slice(&encode_varint(item));
            out.extend_from_slice(&encode_varint(1));
            out.extend_from_slice(&encode_varint(i32::from(own) + i32::from(index > 0)));
            out.extend_from_slice(&encode_varint(0));

            if own {
                out.extend_from_slice(&encode_varint(COMPONENT_FIREWORKS));
                encode_fireworks(&mut out, rocket);
            }

            if index > 0 {
                out.extend_from_slice(&encode_varint(COMPONENT_INTANGIBLE_PROJECTILE));
                out.extend_from_slice(&[0x0a, 0x00]);
            }
        }

        count += 1;
    } else if data.charged > 0
        && let Some(arrow) = crate::blocks::item_named("arrow")
    {
        // Список снарядов: предмет, число, без довесков (ItemStackTemplate).
        out.extend_from_slice(&encode_varint(COMPONENT_CHARGED_PROJECTILES));
        out.extend_from_slice(&encode_varint(i32::from(data.charged)));

        // Боковые стрелы «залпа» — с пометкой «не подбирается»
        // (`intangible_projectile`, пустой NBT) (чёрный ящик, crossbow-multishot).
        for index in 0..data.charged {
            out.extend_from_slice(&encode_varint(arrow));
            out.extend_from_slice(&encode_varint(1));
            out.extend_from_slice(&encode_varint(i32::from(index > 0)));
            out.extend_from_slice(&encode_varint(0));

            if index > 0 {
                out.extend_from_slice(&encode_varint(COMPONENT_INTANGIBLE_PROJECTILE));
                out.extend_from_slice(&[0x0a, 0x00]);
            }
        }

        count += 1;
    }

    if let Some((material, pattern)) = data.trim {
        // Оба — записи реестров по номеру + 1 (0 значил бы «записано тут же»).
        out.extend_from_slice(&encode_varint(COMPONENT_TRIM));
        out.extend_from_slice(&encode_varint(i32::from(material) + 1));
        out.extend_from_slice(&encode_varint(i32::from(pattern) + 1));
        count += 1;
    }

    if let Some(mods) = &data.attributes {
        out.extend_from_slice(&encode_varint(COMPONENT_ATTRIBUTE_MODIFIERS));
        out.extend_from_slice(&encode_varint(mods.len() as i32));

        for modifier in mods {
            out.extend_from_slice(&encode_varint(modifier.attribute));
            out.extend_from_slice(&encode_string(&modifier.id));
            out.extend_from_slice(&modifier.amount().to_be_bytes());
            out.extend_from_slice(&encode_varint(i32::from(modifier.operation)));
            out.extend_from_slice(&encode_varint(i32::from(modifier.slot)));
            out.extend_from_slice(&encode_varint(0)); // показ по умолчанию
        }

        count += 1;
    }

    if let Some(potion) = data.potion {
        // potion_contents: зелье есть, своего цвета нет, своих эффектов нет,
        // своего имени нет (protocol.json, PotionContents).
        out.extend_from_slice(&encode_varint(COMPONENT_POTION_CONTENTS));
        out.push(1);
        out.extend_from_slice(&encode_varint(i32::from(potion)));
        out.extend_from_slice(&[0, 0, 0]);
        count += 1;
    }

    if let Some(explosion) = &data.explosion {
        out.extend_from_slice(&encode_varint(COMPONENT_FIREWORK_EXPLOSION));
        encode_explosion(&mut out, explosion);
        count += 1;
    }

    if let Some(fireworks) = &data.fireworks {
        out.extend_from_slice(&encode_varint(COMPONENT_FIREWORKS));
        encode_fireworks(&mut out, fireworks);
        count += 1;
    }

    // Содержимое шулкер-ящика (`container`) клиенту пока не шлётся: по
    // protocol.json (26.1) это список `option<ItemStackTemplate>`, но
    // разбор записи оригинала у mineflayer на нём падает (чёрный ящик,
    // shulker-break), и неверный формат отключил бы клиента. Предмет
    // показывается без подсказки о содержимом; содержимое хранит сервер.
    // [проверить]
    if let Some(level) = data.ominous {
        out.extend_from_slice(&encode_varint(COMPONENT_OMINOUS_AMPLIFIER));
        out.extend_from_slice(&encode_varint(i32::from(level)));
        count += 1;
    }

    // Зловещий флаг: узор, подсказка без узора, имя и редкость «необычный»
    // (чёрный ящик, evoker-fangs: четыре компонента в таком порядке).
    if data.ominous_banner {
        out.extend_from_slice(&encode_varint(COMPONENT_BANNER_PATTERNS));
        out.extend_from_slice(&encode_varint(OMINOUS_BANNER_LAYERS.len() as i32));

        for (pattern, color) in OMINOUS_BANNER_LAYERS {
            out.extend_from_slice(&encode_varint(pattern + 1));
            out.extend_from_slice(&encode_varint(color));
        }

        out.extend_from_slice(&encode_varint(COMPONENT_TOOLTIP_DISPLAY));
        out.push(0);
        out.extend_from_slice(&encode_varint(1));
        out.extend_from_slice(&encode_varint(COMPONENT_BANNER_PATTERNS));
        out.extend_from_slice(&encode_varint(COMPONENT_ITEM_NAME));
        out.extend_from_slice(&crate::network::nbt::translated("block.minecraft.ominous_banner"));
        out.extend_from_slice(&encode_varint(COMPONENT_RARITY));
        out.extend_from_slice(&encode_varint(1));
        count += 4;
    }

    (count, out)
}

/// Взрыв для клиента: форма, цвета, цвета угасания, след, мерцание
/// (protocol.json, ItemFireworkExplosion).
fn encode_explosion(out: &mut Vec<u8>, explosion: &Explosion) {
    out.extend_from_slice(&encode_varint(i32::from(explosion.shape)));

    for colors in [&explosion.colors, &explosion.fade_colors] {
        out.extend_from_slice(&encode_varint(colors.len() as i32));

        for color in colors {
            out.extend_from_slice(&color.to_be_bytes());
        }
    }

    out.push(u8::from(explosion.trail));
    out.push(u8::from(explosion.twinkle));
}

/// Ракета для клиента: полёт числом и список взрывов.
fn encode_fireworks(out: &mut Vec<u8>, fireworks: &Fireworks) {
    out.extend_from_slice(&encode_varint(i32::from(fireworks.flight)));
    out.extend_from_slice(&encode_varint(fireworks.explosions.len() as i32));

    for explosion in &fireworks.explosions {
        encode_explosion(out, explosion);
    }
}

/// Взрыв от клиента; None — испорчен.
fn read_explosion(reader: &mut Reader) -> Option<Explosion> {
    let shape = u8::try_from(reader.varint()?).ok().filter(|shape| usize::from(*shape) < EXPLOSION_SHAPES.len())?;
    let mut lists = [Vec::new(), Vec::new()];

    for list in &mut lists {
        for _ in 0..reader.varint()?.clamp(0, 256) {
            list.push(i32::from_be_bytes(reader.take(4)?.try_into().ok()?));
        }
    }

    let [colors, fade_colors] = lists;
    let trail = reader.take(1)?[0] != 0;
    let twinkle = reader.take(1)?[0] != 0;

    Some(Explosion { shape, colors, fade_colors, trail, twinkle })
}

/// Ракета от клиента; None — испорчена.
fn read_fireworks(reader: &mut Reader) -> Option<Fireworks> {
    let flight = reader.varint()?.clamp(-128, 127) as i8;
    let mut explosions = Vec::new();

    for _ in 0..reader.varint()?.clamp(0, 256) {
        explosions.push(read_explosion(reader)?);
    }

    Some(Fireworks { flight, explosions })
}

/// Небольшой читатель байтов для довесков.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn varint(&mut self) -> Option<i32> {
        let (value, read) = decode_varint(self.bytes.get(self.at..)?).ok()?;

        self.at += read;

        Some(value)
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let part = self.bytes.get(self.at..self.at.checked_add(count)?)?;

        self.at += count;

        Some(part)
    }

    fn string(&mut self) -> Option<String> {
        let length = usize::try_from(self.varint()?).ok()?;

        String::from_utf8(self.take(length)?.to_vec()).ok()
    }
}

/// Читает один компонент, присланный клиентом (Untrusted Slot: тело
/// известной длины), и записывает его в довески. Незнакомый пропускается.
pub fn read_component(kind: i32, body: &[u8], data: &mut Data) {
    let mut reader = Reader { bytes: body, at: 0 };

    match kind {
        COMPONENT_STEW => {
            let Some(n)=reader.varint().filter(|n| (0..=32).contains(n)) else { return; };
            let mut effects=Vec::new();
            for _ in 0..n {
                let (Some(id),Some(ticks))=(reader.varint(),reader.varint()) else { return; };
                if (0..=32).contains(&id) && ticks>0 { effects.push((id as u8,ticks)); }
            }
            data.stew=effects;
        }
        COMPONENT_INSTRUMENT => {
            if reader.take(1)==Some(&[1]) && let Some(id)=reader.varint().filter(|n| (1..=8).contains(n)) { data.instrument=Some((id-1) as u8); }
        }
        59 => data.bucket = crate::mob::water::bucket::BucketData::from_nbt(body),
        86 | 88..=90 | 104 => {
            if let Some(value) = reader.varint().filter(|value| crate::mob::water::bucket::valid_variant(kind,*value)) {
                data.mob_variants.retain(|(key,_)| *key != kind);
                data.mob_variants.push((kind,value));
            }
        }
        46 => { data.map_id = reader.varint().filter(|id| *id >= 0); }
        COMPONENT_DAMAGE => {
            if let Some(damage) = reader.varint() {
                data.damage = damage.max(0);
            }
        }
        COMPONENT_UNBREAKABLE => data.unbreakable = true,
        COMPONENT_OMINOUS_AMPLIFIER => data.ominous = reader.varint().map(|level| level.clamp(0, 4) as u8),
        COMPONENT_TRIM => {
            if let (Some(material), Some(pattern)) = (reader.varint(), reader.varint())
                && (1..=TRIM_MATERIALS.len() as i32).contains(&material)
                && (1..=TRIM_PATTERNS.len() as i32).contains(&pattern)
            {
                data.trim = Some(((material - 1) as u8, (pattern - 1) as u8));
            }
        }
        COMPONENT_ENCHANTMENTS | COMPONENT_STORED_ENCHANTMENTS => {
            let Some(count) = reader.varint() else { return };

            for _ in 0..count.clamp(0, 64) {
                let (Some(number), Some(level)) = (reader.varint(), reader.varint()) else { return };

                if let Ok(number) = u16::try_from(number)
                    && usize::from(number) < ENCHANTMENTS.len()
                    && (1..=255).contains(&level)
                {
                    if kind == COMPONENT_ENCHANTMENTS {
                        data.set_enchant(number, level);
                    } else {
                        data.set_stored(number, level);
                    }
                }
            }
        }
        COMPONENT_REPAIR_COST => {
            if let Some(cost) = reader.varint() {
                data.repair_cost = cost.max(0);
            }
        }
        COMPONENT_CUSTOM_NAME => {
            // Простой текст — строковый тег NBT (8, длина в два байта).
            if reader.take(1) == Some(&[8])
                && let Some(length) = reader.take(2).map(|bytes| usize::from(u16::from_be_bytes([bytes[0], bytes[1]])))
                && let Some(text) = reader.take(length).and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
            {
                data.name = Some(text);
            }
        }
        COMPONENT_ATTRIBUTE_MODIFIERS => {
            let Some(count) = reader.varint() else { return };
            let mut mods = Vec::new();

            for _ in 0..count.clamp(0, 64) {
                let (Some(attribute), Some(id)) = (reader.varint(), reader.string()) else { return };
                let Some(amount) = reader.take(8).and_then(|bytes| bytes.try_into().ok()).map(f64::from_be_bytes) else { return };
                let (Some(operation), Some(slot), Some(display)) = (reader.varint(), reader.varint(), reader.varint()) else { return };

                if display == 2 || attributes::name_of(attribute).is_none() {
                    return;
                }

                mods.push(ItemModifier {
                    attribute,
                    id,
                    amount_bits: amount.to_bits(),
                    operation: operation.clamp(0, 2) as u8,
                    slot: slot.clamp(0, 10) as u8,
                });
            }

            data.attributes = Some(mods);
        }
        COMPONENT_POTION_CONTENTS => {
            if reader.take(1) == Some(&[1])
                && let Some(potion) = reader.varint().and_then(|number| u16::try_from(number).ok())
                && usize::from(potion) < crate::survival::POTIONS.len()
            {
                data.potion = Some(potion);
            }
        }
        COMPONENT_CHARGED_PROJECTILES => {
            // Обычные стрелы без довесков или ракеты (с `fireworks`); иное —
            // не заряжен.
            let Some(count) = reader.varint() else { return };
            let arrow = crate::blocks::item_named("arrow");
            let rocket_item = crate::blocks::item_named("firework_rocket");
            let mut loaded = 0u8;
            let mut rocket = None;

            for _ in 0..count.clamp(0, 3) {
                let (Some(item), Some(_), Some(added), Some(removed)) = (reader.varint(), reader.varint(), reader.varint(), reader.varint()) else { return };

                if removed != 0 || !(Some(item) == arrow || Some(item) == rocket_item) {
                    return;
                }

                let mut own = Fireworks::default();

                for _ in 0..added.clamp(0, 4) {
                    match reader.varint() {
                        Some(COMPONENT_FIREWORKS) if Some(item) == rocket_item => match read_fireworks(&mut reader) {
                            Some(read) => own = read,
                            None => return,
                        },
                        Some(COMPONENT_INTANGIBLE_PROJECTILE) => {
                            if reader.take(2).is_none() {
                                return;
                            }
                        }
                        _ => return,
                    }
                }

                if Some(item) == rocket_item {
                    rocket = Some(own);
                }

                loaded += 1;
            }

            data.charged = loaded;
            data.charged_rocket = rocket.filter(|_| loaded > 0);
        }
        COMPONENT_FIREWORK_EXPLOSION => data.explosion = read_explosion(&mut reader),
        COMPONENT_FIREWORKS => {
            if let Some(fireworks) = read_fireworks(&mut reader) {
                data.set_rocket(fireworks);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Файл
// ---------------------------------------------------------------------------

/// Довески в файл: износ, признак, зачарования, свои модификаторы.
pub fn to_bytes(data: &Data) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(&data.damage.to_be_bytes());
    out.push(
        u8::from(data.unbreakable)
            | (u8::from(data.attributes.is_some()) << 1)
            | (u8::from(data.potion.is_some()) << 2)
            | (u8::from(data.charged > 0) << 3)
            | (u8::from(data.fireworks.is_some()) << 4)
            | (u8::from(data.explosion.is_some()) << 5)
            | (u8::from(data.charged > 0 && data.charged_rocket.is_some()) << 6)
            | (u8::from(!data.stored.is_empty() || data.repair_cost != 0 || data.name.is_some() || data.trim.is_some() || !data.contents.is_empty() || data.ominous.is_some() || data.ominous_banner || !data.stew.is_empty() || data.instrument.is_some() || data.bucket.is_some() || !data.mob_variants.is_empty() || data.map_id.is_some()) << 7),
    );
    out.push(data.enchants.len().min(255) as u8);

    for (number, level) in data.enchants.iter().take(255) {
        out.extend_from_slice(&number.to_be_bytes());
        out.extend_from_slice(&level.to_be_bytes());
    }

    if let Some(mods) = &data.attributes {
        out.push(mods.len().min(255) as u8);

        for modifier in mods.iter().take(255) {
            out.extend_from_slice(&modifier.attribute.to_be_bytes());
            out.push(modifier.id.len().min(255) as u8);
            out.extend_from_slice(&modifier.id.as_bytes()[..modifier.id.len().min(255)]);
            out.extend_from_slice(&modifier.amount_bits.to_be_bytes());
            out.push(modifier.operation);
            out.push(modifier.slot);
        }
    }

    if let Some(potion) = data.potion {
        out.extend_from_slice(&potion.to_be_bytes());
    }

    if data.charged > 0 {
        out.push(data.charged);
    }

    for fireworks in [&data.fireworks, &data.charged_rocket.clone().filter(|_| data.charged > 0)].into_iter().flatten() {
        fireworks_to_bytes(&mut out, fireworks);
    }

    if let Some(explosion) = &data.explosion {
        explosion_to_bytes(&mut out, explosion);
    }

    // Хвост наковальни и книги: зачарования книги, цена работ, имя, отделка
    // (её байты в конце: в старых файлах их нет).
    if !data.stored.is_empty() || data.repair_cost != 0 || data.name.is_some() || data.trim.is_some() || !data.contents.is_empty() || data.ominous.is_some() || data.ominous_banner || !data.stew.is_empty() || data.instrument.is_some() || data.bucket.is_some() || !data.mob_variants.is_empty() || data.map_id.is_some() {
        out.push(data.stored.len().min(255) as u8);

        for (number, level) in data.stored.iter().take(255) {
            out.extend_from_slice(&number.to_be_bytes());
            out.extend_from_slice(&level.to_be_bytes());
        }

        out.extend_from_slice(&data.repair_cost.to_be_bytes());

        let name = data.name.as_deref().unwrap_or("");
        let length = name.len().min(u16::MAX as usize);

        out.push(u8::from(data.name.is_some()));
        out.extend_from_slice(&(length as u16).to_be_bytes());
        out.extend_from_slice(&name.as_bytes()[..length]);

        if let Some((material, pattern)) = data.trim {
            out.extend_from_slice(&[1, material, pattern]);
        }

        if !data.stew.is_empty() {
            out.extend_from_slice(&[5,data.stew.len().min(32) as u8]);
            for (id,ticks) in data.stew.iter().take(32) { out.push(*id); out.extend_from_slice(&ticks.to_be_bytes()); }
        }
        if let Some(id)=data.instrument { out.extend_from_slice(&[6,id]); }
        if let Some(bucket) = &data.bucket {
            let nbt = bucket.nbt();
            out.push(7);
            out.extend_from_slice(&(nbt.len() as u16).to_be_bytes());
            out.extend(nbt);
        }
        if !data.mob_variants.is_empty() {
            out.push(8);
            out.push(data.mob_variants.len().min(16) as u8);
            for (key,value) in data.mob_variants.iter().take(16) {
                out.extend_from_slice(&key.to_be_bytes());
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        if let Some(id) = data.map_id { out.push(9); out.extend_from_slice(&id.to_be_bytes()); }

        // Содержимое шулкер-ящика: метка 2, число ячеек, у каждой номер,
        // предмет, число и довески (длина и запись; 0 — без довесков).
        if !data.contents.is_empty() {
            out.push(2);
            out.push(data.contents.len().min(27) as u8);

            for content in data.contents.iter().take(27) {
                out.push(content.slot);
                out.extend_from_slice(&content.item.to_be_bytes());
                out.extend_from_slice(&content.count.to_be_bytes());

                let extras = content.data.map(|id| to_bytes(&get(id))).unwrap_or_default();

                out.extend_from_slice(&(extras.len() as u16).to_be_bytes());
                out.extend_from_slice(&extras);
            }
        }

        // Метка 3 — уровень зловещей бутылки, 4 — зловещий флаг (5 — рагу, 6 — рог выше).
        if let Some(level) = data.ominous {
            out.extend_from_slice(&[3, level]);
        }

        if data.ominous_banner {
            out.push(4);
        }
    }

    out
}

fn explosion_to_bytes(out: &mut Vec<u8>, explosion: &Explosion) {
    out.push(explosion.shape);

    for colors in [&explosion.colors, &explosion.fade_colors] {
        out.push(colors.len().min(255) as u8);

        for color in colors.iter().take(255) {
            out.extend_from_slice(&color.to_be_bytes());
        }
    }

    out.push(u8::from(explosion.trail) | (u8::from(explosion.twinkle) << 1));
}

fn fireworks_to_bytes(out: &mut Vec<u8>, fireworks: &Fireworks) {
    out.push(fireworks.flight as u8);
    out.push(fireworks.explosions.len().min(255) as u8);

    for explosion in fireworks.explosions.iter().take(255) {
        explosion_to_bytes(out, explosion);
    }
}

fn explosion_from_bytes(reader: &mut Reader) -> Option<Explosion> {
    let shape = reader.take(1)?[0];
    let mut lists = [Vec::new(), Vec::new()];

    for list in &mut lists {
        for _ in 0..reader.take(1)?[0] {
            list.push(i32::from_be_bytes(reader.take(4)?.try_into().ok()?));
        }
    }

    let [colors, fade_colors] = lists;
    let flags = reader.take(1)?[0];

    Some(Explosion { shape, colors, fade_colors, trail: flags & 1 != 0, twinkle: flags & 2 != 0 })
}

fn fireworks_from_bytes(reader: &mut Reader) -> Option<Fireworks> {
    let flight = reader.take(1)?[0] as i8;
    let mut explosions = Vec::new();

    for _ in 0..reader.take(1)?[0] {
        explosions.push(explosion_from_bytes(reader)?);
    }

    Some(Fireworks { flight, explosions })
}

/// Довески из файла; None — запись испорчена.
pub fn from_bytes(bytes: &[u8]) -> Option<Data> {
    let mut reader = Reader { bytes, at: 0 };
    let damage = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
    let flags = reader.take(1)?[0];
    let mut data = Data { damage, unbreakable: flags & 1 != 0, ..Data::default() };
    let count = reader.take(1)?[0];

    for _ in 0..count {
        let number = u16::from_be_bytes(reader.take(2)?.try_into().ok()?);
        let level = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);

        if usize::from(number) < ENCHANTMENTS.len() && level > 0 {
            data.set_enchant(number, level);
        }
    }

    if flags & 2 != 0 {
        let mut mods = Vec::new();

        for _ in 0..reader.take(1)?[0] {
            let attribute = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
            let length = usize::from(reader.take(1)?[0]);
            let id = String::from_utf8(reader.take(length)?.to_vec()).ok()?;
            let amount_bits = u64::from_be_bytes(reader.take(8)?.try_into().ok()?);
            let operation = reader.take(1)?[0];
            let slot = reader.take(1)?[0];

            mods.push(ItemModifier { attribute, id, amount_bits, operation, slot });
        }

        data.attributes = Some(mods);
    }

    if flags & 4 != 0 {
        data.potion = Some(u16::from_be_bytes(reader.take(2)?.try_into().ok()?));
    }

    if flags & 8 != 0 {
        data.charged = reader.take(1)?[0];
    }

    if flags & 16 != 0 {
        data.fireworks = Some(fireworks_from_bytes(&mut reader)?);
    }

    if flags & 64 != 0 {
        data.charged_rocket = Some(fireworks_from_bytes(&mut reader)?);
    }

    if flags & 32 != 0 {
        data.explosion = Some(explosion_from_bytes(&mut reader)?);
    }

    if flags & 128 != 0 {
        for _ in 0..reader.take(1)?[0] {
            let number = u16::from_be_bytes(reader.take(2)?.try_into().ok()?);
            let level = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);

            if usize::from(number) < ENCHANTMENTS.len() && level > 0 {
                data.set_stored(number, level);
            }
        }

        data.repair_cost = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);

        let named = reader.take(1)?[0] != 0;
        let length = usize::from(u16::from_be_bytes(reader.take(2)?.try_into().ok()?));
        let name = String::from_utf8(reader.take(length)?.to_vec()).ok()?;

        data.name = named.then_some(name);

        // Метки хвоста: 1 — отделка, 2 — содержимое шулкер-ящика.
        while let Some(mark) = reader.take(1).map(|mark| mark[0]) {
            match mark {
                9 => { data.map_id = Some(i32::from_be_bytes(reader.take(4)?.try_into().ok()?)); }
                1 => {
                    let pair = reader.take(2)?;

                    if usize::from(pair[0]) < TRIM_MATERIALS.len() && usize::from(pair[1]) < TRIM_PATTERNS.len() {
                        data.trim = Some((pair[0], pair[1]));
                    }
                }
                2 => {
                    for _ in 0..reader.take(1)?[0].min(27) {
                        let slot = reader.take(1)?[0];
                        let item = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
                        let count = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
                        let length = usize::from(u16::from_be_bytes(reader.take(2)?.try_into().ok()?));
                        let extras = reader.take(length)?;
                        let nested = if extras.is_empty() { None } else { intern(from_bytes(extras)?) };

                        if slot < 27 && count > 0 {
                            data.contents.push(Content { slot, item, count, data: nested });
                        }
                    }
                }
                3 => data.ominous = Some(reader.take(1)?[0].min(4)),
                4 => data.ominous_banner = true,
                5 => {
                    let n=reader.take(1)?[0]; if n>32 { return None; }
                    for _ in 0..n { let id=reader.take(1)?[0]; let ticks=i32::from_be_bytes(reader.take(4)?.try_into().ok()?); data.stew.push((id,ticks)); }
                }
                6 => { let id=reader.take(1)?[0]; if id>=8 { return None; } data.instrument=Some(id); }
                7 => {
                    let length = usize::from(u16::from_be_bytes(reader.take(2)?.try_into().ok()?));
                    data.bucket = Some(crate::mob::water::bucket::BucketData::from_nbt(reader.take(length)?)?);
                }
                8 => {
                    for _ in 0..reader.take(1)?[0] {
                        let key = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
                        let value = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
                        if crate::mob::water::bucket::valid_variant(key,value) { data.mob_variants.push((key,value)); }
                    }
                }
                _ => break,
            }
        }
    }

    Some(data)
}

// ---------------------------------------------------------------------------
// Запись предмета в командах: `diamond_sword[enchantments={sharpness:5},damage=10]`
// ---------------------------------------------------------------------------

/// Разбирает запись предмета: имя без пространства имён и довески.
pub fn parse_item_spec(spec: &str) -> Result<(String, Data), String> {
    let (name, components) = match spec.split_once('[') {
        Some((name, rest)) => (name, Some(rest.strip_suffix(']').ok_or_else(|| "нет закрывающей ]".to_string())?)),
        None => (spec, None),
    };
    let name = name.strip_prefix("minecraft:").unwrap_or(name).to_string();
    let mut data = Data::default();

    for part in components.map(split_top_level).unwrap_or_default() {
        let (key, value) = part.split_once('=').ok_or_else(|| format!("нет значения у {part}"))?;
        let key = key.trim().strip_prefix("minecraft:").unwrap_or(key.trim());
        let value = snbt::parse(value.trim()).ok_or_else(|| format!("не разобрать значение {value}"))?;

        match key {
            "bucket_entity_data" => data.bucket = Some(crate::mob::water::bucket::BucketData::parse(&value)),
            "salmon/size" | "axolotl/variant" | "tropical_fish/pattern" | "tropical_fish/base_color" | "tropical_fish/pattern_color" => {
                let (id,value)=crate::mob::water::bucket::parse_variant(key,&value)?;
                data.mob_variants.retain(|(key,_)|*key!=id);
                data.mob_variants.push((id,value));
            }
            "map_id" => data.map_id = Some(value.as_f64().ok_or("map_id — число")?.max(0.0) as i32),
            "damage" => data.damage = value.as_f64().ok_or("damage — число")? as i32,
            "unbreakable" => data.unbreakable = true,
            "ominous_bottle_amplifier" => data.ominous = Some(value.as_f64().ok_or("ominous_bottle_amplifier — число")?.clamp(0.0, 4.0) as u8),
            "enchantments" | "stored_enchantments" => {
                let Value::Map(fields) = &value else { return Err(format!("{key} — {{имя:уровень}}")) };

                for (enchant, level) in fields {
                    let number = enchantment_number(enchant).ok_or_else(|| format!("нет зачарования {enchant}"))?;
                    let level = level.as_f64().ok_or("уровень — число")? as i32;

                    if key == "enchantments" {
                        data.set_enchant(number, level.clamp(0, 255));
                    } else {
                        data.set_stored(number, level.clamp(0, 255));
                    }
                }
            }
            "repair_cost" => data.repair_cost = value.as_f64().ok_or("repair_cost — число")?.max(0.0) as i32,
            "attribute_modifiers" => {
                let Value::List(list) = &value else { return Err("attribute_modifiers — список".to_string()) };
                let mut mods = Vec::new();

                for entry in list {
                    let kind = entry.get("type").and_then(Value::as_str).and_then(attributes::by_name).ok_or("нет type или атрибут неизвестен")?;
                    let id = entry.get("id").and_then(Value::as_str).ok_or("нет id")?.to_string();
                    let amount = entry.get("amount").and_then(Value::as_f64).ok_or("нет amount")?;
                    let operation = entry.get("operation").and_then(Value::as_str).and_then(Operation::from_name).ok_or("нет operation")?;
                    let slot = entry.get("slot").and_then(Value::as_str).map_or(Some(0), slot_group).ok_or("неизвестный slot")?;

                    mods.push(ItemModifier { attribute: kind, id, amount_bits: amount.to_bits(), operation: operation as u8, slot });
                }

                data.attributes = Some(mods);
            }
            // Зелье: `potion_contents={potion:"minecraft:healing"}` или
            // короткая запись `potion_contents="minecraft:healing"` (вики,
            // Data component format: potion_contents).
            "potion_contents" => {
                let name = value.get("potion").and_then(Value::as_str).or_else(|| value.as_str()).ok_or("potion_contents — {potion:\"…\"}")?;
                let name = name.strip_prefix("minecraft:").unwrap_or(name);

                data.potion = Some(crate::survival::potion_number(name).ok_or_else(|| format!("нет зелья {name}"))?);
            }
            // Ракета: `fireworks={flight_duration:2,explosions:[{shape:"star",
            // colors:[I;255]}]}`, звезда: `firework_explosion={…}` (вики, Data
            // component format: fireworks, firework_explosion).
            "fireworks" => {
                let flight = value.get("flight_duration").map_or(Some(1.0), Value::as_f64).ok_or("flight_duration — число")?;
                let mut explosions = Vec::new();

                if let Some(list) = value.get("explosions") {
                    let Value::List(list) = list else { return Err("explosions — список".to_string()) };

                    for entry in list {
                        explosions.push(parse_explosion(entry)?);
                    }
                }

                data.set_rocket(Fireworks { flight: flight.clamp(-128.0, 127.0) as i8, explosions });
            }
            "firework_explosion" => data.explosion = Some(parse_explosion(&value)?),
            // Своё имя (`custom_name="Бобик"` или `{text:"Бобик"}`): бирка (вики, Name Tag).
            "custom_name" => {
                data.name = match &value {
                    Value::Str(text) => Some(text.clone()),
                    Value::Map(_) => value.get("text").and_then(|text| if let Value::Str(text) = text { Some(text.clone()) } else { None }),
                    _ => None,
                }
                .filter(|name| !name.is_empty());
            }
            // Отделка: `trim={material:"minecraft:gold",pattern:"minecraft:sentry"}`
            // (вики, Data component format: trim).
            "trim" => {
                let short = |key: &str| value.get(key).and_then(Value::as_str).map(|name| name.strip_prefix("minecraft:").unwrap_or(name).to_string());
                let material = short("material").and_then(|name| TRIM_MATERIALS.iter().position(|known| *known == name)).ok_or("trim: нет material")?;
                let pattern = short("pattern").and_then(|name| TRIM_PATTERNS.iter().position(|known| *known == name)).ok_or("trim: нет pattern")?;

                data.trim = Some((material as u8, pattern as u8));
            }
            // Прочие довески пока не ведутся.
            _ => {}
        }
    }

    // В записи команды — по номерам, как было прежде [проверить порядок
    // у оригинала].
    data.enchants.sort_unstable();
    data.stored.sort_unstable();

    Ok((name, data))
}

/// Взрыв из записи SNBT: форма обязательна, прочее — по умолчанию.
fn parse_explosion(value: &Value) -> Result<Explosion, String> {
    let shape = value.get("shape").and_then(Value::as_str).ok_or("у взрыва нет shape")?;
    let shape = shape.strip_prefix("minecraft:").unwrap_or(shape);
    let shape = EXPLOSION_SHAPES.iter().position(|known| *known == shape).ok_or_else(|| format!("нет формы {shape}"))? as u8;
    let colors = |key: &str| -> Result<Vec<i32>, String> {
        match value.get(key) {
            None => Ok(Vec::new()),
            Some(Value::List(list)) => list.iter().map(|color| color.as_f64().map(|c| c as i64 as i32).ok_or_else(|| format!("{key} — числа"))).collect(),
            Some(_) => Err(format!("{key} — список")),
        }
    };
    let flag = |key: &str| value.get(key).and_then(Value::as_f64).is_some_and(|v| v != 0.0);

    Ok(Explosion { shape, colors: colors("colors")?, fade_colors: colors("fade_colors")?, trail: flag("has_trail"), twinkle: flag("has_twinkle") })
}

/// Номер группы мест по имени (`mainhand`, `head`…).
pub fn slot_group(name: &str) -> Option<u8> {
    Some(match name {
        "any" => 0,
        "mainhand" | "main_hand" => 1,
        "offhand" | "off_hand" => 2,
        "hand" => 3,
        "feet" => 4,
        "legs" => 5,
        "chest" => 6,
        "head" => 7,
        "armor" => 8,
        "body" => 9,
        _ => return None,
    })
}

/// Делит по запятым вне скобок и кавычек.
fn split_top_level(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut start = 0;

    for (at, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[' | '{') => depth += 1,
            (None, ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }

    if !text[start..].trim().is_empty() {
        parts.push(&text[start..]);
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_data_share_one_entry_and_plain_stacks_have_none() {
        assert_eq!(intern(Data::default()), None);

        let mut data = Data { damage: 7, ..Data::default() };

        data.set_enchant(enchantment_number("sharpness").unwrap(), 5);

        let first = intern(data.clone()).unwrap();
        let second = intern(data.clone()).unwrap();

        assert_eq!(first, second);
        assert_eq!(get(first), data);
        assert_eq!(get(first).level("minecraft:sharpness"), 5);
        assert_eq!(get(first).level("knockback"), 0);
    }

    #[test]
    fn potion_contents_parse_encode_and_survive_the_file() {
        let (name, data) = parse_item_spec("minecraft:potion[potion_contents={potion:\"minecraft:strong_healing\"}]").unwrap();

        assert_eq!((name.as_str(), data.potion), ("potion", Some(25)));
        assert_eq!(parse_item_spec("potion[potion_contents=\"swiftness\"]").unwrap().1.potion, Some(13));
        assert!(parse_item_spec("potion[potion_contents={potion:\"nope\"}]").is_err());

        // Компонент 51: зелье 25, без цвета, без своих эффектов, без имени.
        assert_eq!(encode_components(&data), (1, vec![51, 1, 25, 0, 0, 0]));

        let mut back = Data::default();

        read_component(COMPONENT_POTION_CONTENTS, &[1, 25, 0, 0, 0], &mut back);
        assert_eq!(back, data);
        assert_eq!(from_bytes(&to_bytes(&data)), Some(data));
    }

    #[test]
    fn fireworks_parse_encode_and_survive_the_file() {
        let (name, data) = parse_item_spec(
            "minecraft:firework_rocket[fireworks={flight_duration:2,explosions:[{shape:\"large_ball\",colors:[I;16711680],fade_colors:[I;255],has_twinkle:true},{shape:\"burst\",has_trail:1b}]}]",
        )
        .unwrap();
        let rocket = data.rocket();

        assert_eq!((name.as_str(), rocket.flight, rocket.explosions.len()), ("firework_rocket", 2, 2));
        assert_eq!(rocket.explosions[0], Explosion { shape: 1, colors: vec![16711680], fade_colors: vec![255], trail: false, twinkle: true });
        assert!(rocket.explosions[1].trail && rocket.explosions[1].shape == 4);
        // Ракета по умолчанию (полёт 1, без взрывов) довесков не несёт.
        assert!(parse_item_spec("firework_rocket[fireworks={flight_duration:1}]").unwrap().1.is_default());
        assert!(parse_item_spec("firework_rocket[fireworks={explosions:[{shape:\"nope\"}]}]").is_err());

        // Компонент 69: полёт 2, два взрыва; обратно читается тем же.
        let (count, bytes) = encode_components(&data);

        assert_eq!((count, &bytes[..3]), (1, &[69u8, 2, 2][..]));

        let mut back = Data::default();

        read_component(COMPONENT_FIREWORKS, &bytes[1..], &mut back);
        assert_eq!(back, data);
        assert_eq!(from_bytes(&to_bytes(&data)), Some(data.clone()));

        // Звезда и заряженный ракетами арбалет переживают файл и клиента.
        let star = parse_item_spec("firework_star[firework_explosion={shape:\"creeper\",colors:[I;1,2]}]").unwrap().1;

        assert_eq!(star.explosion.as_ref().map(|e| (e.shape, e.colors.clone())), Some((3, vec![1, 2])));
        assert_eq!(from_bytes(&to_bytes(&star)), Some(star));

        let loaded = Data { charged: 3, charged_rocket: Some(rocket), ..Data::default() };
        let (_, bytes) = encode_components(&loaded);
        let mut back = Data::default();

        read_component(COMPONENT_CHARGED_PROJECTILES, &bytes[1..], &mut back);
        assert_eq!(back, loaded);
        assert_eq!(from_bytes(&to_bytes(&loaded)), Some(loaded));
    }

    #[test]
    fn data_survive_the_file_round_trip() {
        let mut data = Data { damage: 12, unbreakable: true, ..Data::default() };

        data.set_enchant(enchantment_number("knockback").unwrap(), 2);
        data.set_enchant(enchantment_number("fire_aspect").unwrap(), 1);
        data.attributes = Some(vec![ItemModifier { attribute: attributes::ATTACK_DAMAGE, id: "x:y".to_string(), amount_bits: 9.5f64.to_bits(), operation: 0, slot: 1 }]);

        assert_eq!(from_bytes(&to_bytes(&data)), Some(data));
        assert_eq!(from_bytes(&[1, 2]), None);

        // Заряженный арбалет: число стрел переживает файл, без него байта нет.
        let charged = Data { charged: 3, ..Data::default() };

        assert_eq!(from_bytes(&to_bytes(&charged)), Some(charged));
        assert_eq!(to_bytes(&Data::default()).len(), 6);
    }

    #[test]
    fn ominous_items_survive_the_file_and_encode() {
        let bottle = Data { ominous: Some(3), ..Data::default() };
        let banner = Data { ominous_banner: true, ..Data::default() };

        assert_eq!(from_bytes(&to_bytes(&bottle)), Some(bottle.clone()));
        assert_eq!(from_bytes(&to_bytes(&banner)), Some(banner.clone()));
        assert_eq!(encode_components(&bottle).0, 1);
        // Флаг: узор, подсказка, имя, редкость (чёрный ящик, evoker-fangs).
        assert_eq!(encode_components(&banner).0, 4);
    }

    #[test]
    fn client_components_are_read_back() {
        let mut data = Data { damage: 3, ..Data::default() };

        data.set_enchant(enchantment_number("sharpness").unwrap(), 4);

        let (count, bytes) = encode_components(&data);

        assert_eq!(count, 2);
        // damage: тип 3, значение 3; enchantments: тип 13, один, номер, уровень 4
        assert_eq!(&bytes[..2], &[3, 3]);

        let mut back = Data::default();

        read_component(COMPONENT_DAMAGE, &[3], &mut back);
        read_component(COMPONENT_ENCHANTMENTS, &bytes[3..], &mut back);

        assert_eq!(back, data);
    }

    #[test]
    fn a_hostile_client_cannot_make_up_enchantments() {
        let mut data = Data::default();

        // номер 200 и уровень 0 — обе записи отбрасываются
        read_component(COMPONENT_ENCHANTMENTS, &[2, 200, 1, 1, 0], &mut data);

        assert!(data.is_default());
    }

    #[test]
    fn item_specs_carry_enchantments_damage_and_modifiers() {
        let (name, data) = parse_item_spec(
            "minecraft:diamond_sword[damage=5,enchantments={\"minecraft:sharpness\":3,fire_aspect:2},attribute_modifiers=[{type:\"minecraft:attack_damage\",id:\"my:dmg\",amount:20,operation:\"add_value\",slot:\"mainhand\"}]]",
        )
        .expect("разобралось");

        assert_eq!(name, "diamond_sword");
        assert_eq!(data.damage, 5);
        assert_eq!(data.level("sharpness"), 3);
        assert_eq!(data.level("fire_aspect"), 2);

        let mods = data.attributes.expect("свои модификаторы");

        assert_eq!(mods[0].attribute, attributes::ATTACK_DAMAGE);
        assert_eq!(mods[0].amount(), 20.0);
        assert_eq!(mods[0].slot, 1);
        assert!(parse_item_spec("stick[enchantments={nope:1}]").is_err());
        assert_eq!(parse_item_spec("stick").unwrap().1, Data::default());
    }
}
