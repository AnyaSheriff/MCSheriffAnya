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
pub const COMPONENT_ATTRIBUTE_MODIFIERS: i32 = 16;
pub const COMPONENT_POTION_CONTENTS: i32 = 51;
/// Заряженные снаряды арбалета (minecraft-data, protocol.json: 49).
pub const COMPONENT_CHARGED_PROJECTILES: i32 = 49;
/// Снаряд, который нельзя подобрать (minecraft-data, protocol.json: 22).
pub const COMPONENT_INTANGIBLE_PROJECTILE: i32 = 22;

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

/// Довески одной стопки.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Data {
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
}

impl Data {
    pub fn is_default(&self) -> bool {
        *self == Data::default()
    }

    /// Уровень зачарования по имени (0 — нет).
    pub fn level(&self, name: &str) -> i32 {
        enchantment_number(name).and_then(|number| self.enchants.iter().find(|(known, _)| *known == number)).map_or(0, |(_, level)| *level)
    }

    /// Ставит зачарование (уровень 0 — снимает), держа список по номерам.
    pub fn set_enchant(&mut self, number: u16, level: i32) {
        self.enchants.retain(|(known, _)| *known != number);

        if level > 0 {
            self.enchants.push((number, level));
            self.enchants.sort_unstable();
        }
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

    if data.damage > 0 {
        out.extend_from_slice(&encode_varint(COMPONENT_DAMAGE));
        out.extend_from_slice(&encode_varint(data.damage));
        count += 1;
    }

    if data.unbreakable {
        out.extend_from_slice(&encode_varint(COMPONENT_UNBREAKABLE));
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

    (count, out)
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
        COMPONENT_DAMAGE => {
            if let Some(damage) = reader.varint() {
                data.damage = damage.max(0);
            }
        }
        COMPONENT_UNBREAKABLE => data.unbreakable = true,
        COMPONENT_ENCHANTMENTS => {
            let Some(count) = reader.varint() else { return };

            for _ in 0..count.clamp(0, 64) {
                let (Some(number), Some(level)) = (reader.varint(), reader.varint()) else { return };

                if let Ok(number) = u16::try_from(number)
                    && usize::from(number) < ENCHANTMENTS.len()
                    && (1..=255).contains(&level)
                {
                    data.set_enchant(number, level);
                }
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
            // Только обычные стрелы без довесков; иное — не заряжен.
            let Some(count) = reader.varint() else { return };
            let arrow = crate::blocks::item_named("arrow");
            let mut loaded = 0u8;

            for _ in 0..count.clamp(0, 3) {
                let (Some(item), Some(_), Some(added), Some(removed)) = (reader.varint(), reader.varint(), reader.varint(), reader.varint()) else { return };

                if Some(item) != arrow || added != 0 || removed != 0 {
                    return;
                }

                loaded += 1;
            }

            data.charged = loaded;
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
    out.push(u8::from(data.unbreakable) | (u8::from(data.attributes.is_some()) << 1) | (u8::from(data.potion.is_some()) << 2) | (u8::from(data.charged > 0) << 3));
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

    out
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
            "damage" => data.damage = value.as_f64().ok_or("damage — число")? as i32,
            "unbreakable" => data.unbreakable = true,
            "enchantments" => {
                let Value::Map(fields) = &value else { return Err("enchantments — {имя:уровень}".to_string()) };

                for (enchant, level) in fields {
                    let number = enchantment_number(enchant).ok_or_else(|| format!("нет зачарования {enchant}"))?;
                    let level = level.as_f64().ok_or("уровень — число")? as i32;

                    data.set_enchant(number, level.clamp(0, 255));
                }
            }
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
            // Прочие довески пока не ведутся.
            _ => {}
        }
    }

    Ok((name, data))
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
