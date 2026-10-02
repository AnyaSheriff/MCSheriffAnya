// Атрибуты существ: основа и модификаторы.
//
// Вики, Attribute: у существа у каждого атрибута есть основное значение, на
// него действуют модификаторы — у каждого свой опознаватель, величина и
// операция. Порядок расчёта (Attribute modifiers):
//   1. к основе прибавляются все модификаторы «add_value» (0);
//   2. итог умножается на (1 + сумма модификаторов «add_multiplied_base» (1));
//   3. итог умножается на (1 + величина) каждого «add_multiplied_total» (2);
//   4. значение зажимается в пределы атрибута.
// Номера атрибутов — реестр minecraft:attribute 26.1.2 (отчёт штатного
// генератора: `registries.json`, поле protocol_id).

/// Номера атрибутов в реестре.
pub const ARMOR: i32 = 0;
pub const ARMOR_TOUGHNESS: i32 = 1;
pub const ATTACK_DAMAGE: i32 = 2;
pub const ATTACK_KNOCKBACK: i32 = 3;
pub const ATTACK_SPEED: i32 = 4;
pub const BLOCK_BREAK_SPEED: i32 = 5;
pub const BLOCK_INTERACTION_RANGE: i32 = 6;
pub const ENTITY_INTERACTION_RANGE: i32 = 10;
pub const GRAVITY: i32 = 14;
pub const JUMP_STRENGTH: i32 = 15;
pub const KNOCKBACK_RESISTANCE: i32 = 16;
pub const LUCK: i32 = 17;
pub const MAX_ABSORPTION: i32 = 18;
pub const MAX_HEALTH: i32 = 19;
pub const MOVEMENT_SPEED: i32 = 22;
pub const OXYGEN_BONUS: i32 = 23;
pub const SAFE_FALL_DISTANCE: i32 = 24;
pub const SNEAKING_SPEED: i32 = 26;
pub const SWEEPING_DAMAGE_RATIO: i32 = 30;

/// Операции модификатора.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Operation {
    /// `add_value`: прибавить к основе.
    AddValue = 0,
    /// `add_multiplied_base`: умножить основу (складывается с такими же).
    AddMultipliedBase = 1,
    /// `add_multiplied_total`: умножить итог (каждый отдельно).
    AddMultipliedTotal = 2,
}

impl Operation {
    pub fn from_number(number: i32) -> Option<Self> {
        match number {
            0 => Some(Operation::AddValue),
            1 => Some(Operation::AddMultipliedBase),
            2 => Some(Operation::AddMultipliedTotal),
            _ => None,
        }
    }

    /// Имя операции в командах и данных (`add_value`…).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.strip_prefix("minecraft:").unwrap_or(name) {
            "add_value" | "add" => Some(Operation::AddValue),
            "add_multiplied_base" | "multiply_base" => Some(Operation::AddMultipliedBase),
            "add_multiplied_total" | "multiply_total" => Some(Operation::AddMultipliedTotal),
            _ => None,
        }
    }
}

/// Модификатор атрибута.
#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    /// Опознаватель (`minecraft:base_attack_damage`): у одного атрибута
    /// два модификатора с одним опознавателем не уживаются.
    pub id: String,
    pub amount: f64,
    pub operation: Operation,
}

impl Modifier {
    pub fn new(id: &str, amount: f64, operation: Operation) -> Self {
        Self { id: id.to_string(), amount, operation }
    }
}

/// Уходит ли атрибут клиенту (вики, Attribute: «Syncable»): урон, отдача и
/// стойкость к ней клиенту не нужны. [проверить] по остальным атрибутам.
pub fn syncable(attribute: i32) -> bool {
    // Тяготение и сила прыжка клиенту не уходят (effect-flags: у плавного
    // падения и прыгучести пакета Attributes по ним нет).
    !matches!(attribute, ATTACK_DAMAGE | ATTACK_KNOCKBACK | KNOCKBACK_RESISTANCE | SWEEPING_DAMAGE_RATIO | GRAVITY | JUMP_STRENGTH)
}

/// Пределы значения атрибута (вики, Attribute: «Range»); неизвестные —
/// без ограничения.
pub fn limits(attribute: i32) -> (f64, f64) {
    match attribute {
        ARMOR => (0.0, 30.0),
        ARMOR_TOUGHNESS => (0.0, 20.0),
        ATTACK_DAMAGE => (0.0, 2048.0),
        ATTACK_KNOCKBACK => (0.0, 5.0),
        ATTACK_SPEED => (0.0, 1024.0),
        KNOCKBACK_RESISTANCE => (0.0, 1.0),
        MAX_ABSORPTION => (0.0, 2048.0),
        MAX_HEALTH => (1.0, 1024.0),
        MOVEMENT_SPEED => (0.0, 1024.0),
        SWEEPING_DAMAGE_RATIO => (0.0, 1.0),
        _ => (f64::MIN, f64::MAX),
    }
}

/// Один атрибут существа: основа и модификаторы по порядку добавления.
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    pub id: i32,
    pub base: f64,
    pub modifiers: Vec<Modifier>,
}

impl Attribute {
    /// Значение с учётом модификаторов.
    pub fn value(&self) -> f64 {
        let mut value = self.base;

        for modifier in self.modifiers.iter().filter(|m| m.operation == Operation::AddValue) {
            value += modifier.amount;
        }

        let base_sum: f64 = self.modifiers.iter().filter(|m| m.operation == Operation::AddMultipliedBase).map(|m| m.amount).sum();

        value *= 1.0 + base_sum;

        for modifier in self.modifiers.iter().filter(|m| m.operation == Operation::AddMultipliedTotal) {
            value *= 1.0 + modifier.amount;
        }

        let (low, high) = limits(self.id);

        value.clamp(low, high)
    }
}

/// Основные значения атрибутов игрока (вики, Attribute; Player). В списке
/// только те, что сервер ведёт.
pub fn player_base(attribute: i32) -> f64 {
    match attribute {
        ATTACK_DAMAGE => 1.0,
        ATTACK_SPEED => 4.0,
        BLOCK_BREAK_SPEED => 1.0,
        BLOCK_INTERACTION_RANGE => 4.5,
        ENTITY_INTERACTION_RANGE => 3.0,
        // Тяготение, сила прыжка и безопасная высота — по вики (Attribute,
        // «Default values») [проверить].
        GRAVITY => 0.08,
        JUMP_STRENGTH => f64::from(0.42f32),
        MAX_HEALTH => 20.0,
        SAFE_FALL_DISTANCE => 3.0,
        // Скорость шага одинарной точности — как её знает клиент.
        MOVEMENT_SPEED => f64::from(0.1f32),
        SNEAKING_SPEED => 0.3,
        _ => 0.0,
    }
}

/// Атрибуты, которые ведёт сервер у игрока.
const PLAYER_ATTRIBUTES: [i32; 12] = [
    ARMOR,
    ARMOR_TOUGHNESS,
    ATTACK_DAMAGE,
    ATTACK_KNOCKBACK,
    ATTACK_SPEED,
    BLOCK_INTERACTION_RANGE,
    ENTITY_INTERACTION_RANGE,
    KNOCKBACK_RESISTANCE,
    MAX_HEALTH,
    MOVEMENT_SPEED,
    OXYGEN_BONUS,
    SWEEPING_DAMAGE_RATIO,
];

/// Набор атрибутов существа.
#[derive(Clone, Debug, PartialEq)]
pub struct Attributes {
    list: Vec<Attribute>,
}

impl Attributes {
    /// Атрибуты игрока: основа и ни одного модификатора.
    pub fn player() -> Self {
        Self { list: PLAYER_ATTRIBUTES.iter().map(|id| Attribute { id: *id, base: player_base(*id), modifiers: Vec::new() }).collect() }
    }

    pub fn get(&self, attribute: i32) -> Option<&Attribute> {
        self.list.iter().find(|entry| entry.id == attribute)
    }

    fn entry(&mut self, attribute: i32) -> &mut Attribute {
        if let Some(index) = self.list.iter().position(|entry| entry.id == attribute) {
            return &mut self.list[index];
        }

        self.list.push(Attribute { id: attribute, base: player_base(attribute), modifiers: Vec::new() });
        self.list.last_mut().expect("только что добавлен")
    }

    /// Значение атрибута; у неведомого — основа игрока.
    pub fn value(&self, attribute: i32) -> f64 {
        self.get(attribute).map_or_else(|| player_base(attribute), Attribute::value)
    }

    /// Ставит модификатор; с тем же опознавателем — заменяет прежний.
    pub fn set_modifier(&mut self, attribute: i32, modifier: Modifier) {
        let entry = self.entry(attribute);

        match entry.modifiers.iter_mut().find(|m| m.id == modifier.id) {
            Some(existing) => *existing = modifier,
            None => entry.modifiers.push(modifier),
        }
    }

    /// Снимает модификатор по опознавателю.
    pub fn remove_modifier(&mut self, attribute: i32, id: &str) {
        if let Some(entry) = self.list.iter_mut().find(|entry| entry.id == attribute) {
            entry.modifiers.retain(|m| m.id != id);
        }
    }
}

/// Что клиенту уже сообщено об атрибутах: по этому слепку решается, что
/// досылать (у оригинала досылается изменившееся).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sent {
    list: Vec<Attribute>,
}

impl Sent {
    /// Слепок «клиент знает всё как есть».
    pub fn of(attributes: &Attributes) -> Self {
        Self { list: attributes.list.clone() }
    }

    /// Атрибуты, которые изменились с прошлой рассылки, — и запоминает их.
    #[cfg(test)]
    pub fn changed(&mut self, attributes: &Attributes) -> Vec<Attribute> {
        self.changed_with(attributes, &[], &[])
    }

    /// То же, но `touched` уходят в любом случае, даже если ничего не
    /// изменилось: у оригинала снять и поставить модификатор — уже
    /// «изменение» (чёрный ящик, pvp-armor: броня после износа шлётся снова).
    pub fn changed_with(&mut self, attributes: &Attributes, touched: &[i32], skip: &[i32]) -> Vec<Attribute> {
        let mut out = Vec::new();

        for entry in attributes.list.iter().filter(|entry| syncable(entry.id) && !skip.contains(&entry.id)) {
            if touched.contains(&entry.id) {
                match self.list.iter_mut().find(|known| known.id == entry.id) {
                    Some(known) => *known = entry.clone(),
                    None => self.list.push(entry.clone()),
                }

                out.push(entry.clone());
                continue;
            }

            match self.list.iter_mut().find(|known| known.id == entry.id) {
                Some(known) if known == entry => {}
                Some(known) => {
                    *known = entry.clone();
                    out.push(entry.clone());
                }
                None => {
                    self.list.push(entry.clone());
                    out.push(entry.clone());
                }
            }
        }

        out
    }
}

/// Тело пакета Update Attributes (без номера пакета): существо и записи —
/// номер атрибута, основа, модификаторы (опознаватель, величина, операция).
pub fn encode_update(entity_id: i32, entries: &[Attribute]) -> Vec<u8> {
    use crate::network::varint::encode_varint;

    let mut body = encode_varint(entity_id);

    body.extend_from_slice(&encode_varint(entries.len() as i32));

    for entry in entries {
        body.extend_from_slice(&encode_varint(entry.id));
        body.extend_from_slice(&entry.base.to_be_bytes());
        body.extend_from_slice(&encode_varint(entry.modifiers.len() as i32));

        for modifier in &entry.modifiers {
            body.extend_from_slice(&encode_varint(modifier.id.len() as i32));
            body.extend_from_slice(modifier.id.as_bytes());
            body.extend_from_slice(&modifier.amount.to_be_bytes());
            body.push(modifier.operation as u8);
        }
    }

    body
}

/// Имя атрибута по номеру — для команд и данных.
pub fn name_of(attribute: i32) -> Option<&'static str> {
    NAMES.iter().find(|(number, _)| *number == attribute).map(|(_, name)| *name)
}

/// Номер атрибута по имени (`minecraft:attack_damage` или `attack_damage`).
pub fn by_name(name: &str) -> Option<i32> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);

    NAMES.iter().find(|(_, known)| *known == name).map(|(number, _)| *number)
}

/// Реестр minecraft:attribute 26.1.2.
const NAMES: [(i32, &str); 35] = [
    (0, "armor"),
    (1, "armor_toughness"),
    (2, "attack_damage"),
    (3, "attack_knockback"),
    (4, "attack_speed"),
    (5, "block_break_speed"),
    (6, "block_interaction_range"),
    (7, "burning_time"),
    (8, "camera_distance"),
    (9, "explosion_knockback_resistance"),
    (10, "entity_interaction_range"),
    (11, "fall_damage_multiplier"),
    (12, "flying_speed"),
    (13, "follow_range"),
    (14, "gravity"),
    (15, "jump_strength"),
    (16, "knockback_resistance"),
    (17, "luck"),
    (18, "max_absorption"),
    (19, "max_health"),
    (20, "mining_efficiency"),
    (21, "movement_efficiency"),
    (22, "movement_speed"),
    (23, "oxygen_bonus"),
    (24, "safe_fall_distance"),
    (25, "scale"),
    (26, "sneaking_speed"),
    (27, "spawn_reinforcements"),
    (28, "step_height"),
    (29, "submerged_mining_speed"),
    (30, "sweeping_damage_ratio"),
    (31, "tempt_range"),
    (32, "water_movement_efficiency"),
    (33, "waypoint_transmit_range"),
    (34, "waypoint_receive_range"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_player_has_the_wiki_base_values() {
        let attributes = Attributes::player();

        assert_eq!(attributes.value(ATTACK_DAMAGE), 1.0);
        assert_eq!(attributes.value(ATTACK_SPEED), 4.0);
        assert_eq!(attributes.value(ARMOR), 0.0);
        assert_eq!(attributes.value(ARMOR_TOUGHNESS), 0.0);
        assert_eq!(attributes.value(KNOCKBACK_RESISTANCE), 0.0);
        assert_eq!(attributes.value(MAX_HEALTH), 20.0);
        assert_eq!(attributes.value(MOVEMENT_SPEED), f64::from(0.1f32));
    }

    #[test]
    fn operations_apply_in_the_wiki_order() {
        let mut attributes = Attributes::player();

        // (1 + 6) * (1 + 0.5 + 0.25) * 1.1 * 1.2
        attributes.set_modifier(ATTACK_DAMAGE, Modifier::new("a", 6.0, Operation::AddValue));
        attributes.set_modifier(ATTACK_DAMAGE, Modifier::new("b", 0.5, Operation::AddMultipliedBase));
        attributes.set_modifier(ATTACK_DAMAGE, Modifier::new("c", 0.25, Operation::AddMultipliedBase));
        attributes.set_modifier(ATTACK_DAMAGE, Modifier::new("d", 0.1, Operation::AddMultipliedTotal));
        attributes.set_modifier(ATTACK_DAMAGE, Modifier::new("e", 0.2, Operation::AddMultipliedTotal));

        let expected = 7.0 * 1.75 * 1.1 * 1.2;

        assert!((attributes.value(ATTACK_DAMAGE) - expected).abs() < 1e-9);
    }

    #[test]
    fn a_modifier_with_the_same_id_replaces_the_old_one() {
        let mut attributes = Attributes::player();

        attributes.set_modifier(ARMOR, Modifier::new("armor.chestplate", 8.0, Operation::AddValue));
        attributes.set_modifier(ARMOR, Modifier::new("armor.chestplate", 3.0, Operation::AddValue));

        assert_eq!(attributes.value(ARMOR), 3.0);

        attributes.remove_modifier(ARMOR, "armor.chestplate");

        assert_eq!(attributes.value(ARMOR), 0.0);
    }

    #[test]
    fn values_are_clamped_to_the_attribute_range() {
        let mut attributes = Attributes::player();

        attributes.set_modifier(ARMOR, Modifier::new("x", 99.0, Operation::AddValue));
        attributes.set_modifier(KNOCKBACK_RESISTANCE, Modifier::new("x", 5.0, Operation::AddValue));
        attributes.set_modifier(ATTACK_SPEED, Modifier::new("x", -9.0, Operation::AddValue));

        assert_eq!(attributes.value(ARMOR), 30.0);
        assert_eq!(attributes.value(KNOCKBACK_RESISTANCE), 1.0);
        assert_eq!(attributes.value(ATTACK_SPEED), 0.0);
    }

    #[test]
    fn only_changed_attributes_are_sent_again() {
        let mut attributes = Attributes::player();
        let mut sent = Sent::of(&attributes);

        assert!(sent.changed(&attributes).is_empty());

        attributes.set_modifier(ARMOR, Modifier::new("armor.helmet", 3.0, Operation::AddValue));

        let changed = sent.changed(&attributes);

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id, ARMOR);
        assert!(sent.changed(&attributes).is_empty());
    }

    #[test]
    fn the_update_packet_lists_modifiers_with_their_operation() {
        let entry = Attribute { id: ARMOR, base: 0.0, modifiers: vec![Modifier::new("a", 2.0, Operation::AddMultipliedTotal)] };
        let body = encode_update(7, &[entry]);

        // сущность 7, одна запись, атрибут 0, основа 0.0, один модификатор
        assert_eq!(&body[..3], &[7, 1, 0]);
        assert_eq!(&body[3..11], &0.0f64.to_be_bytes());
        assert_eq!(body[11], 1);
        assert_eq!(body[12], 1);
        assert_eq!(body[13], b'a');
        assert_eq!(*body.last().unwrap(), 2);
    }

    #[test]
    fn names_and_numbers_agree() {
        assert_eq!(by_name("minecraft:attack_damage"), Some(ATTACK_DAMAGE));
        assert_eq!(by_name("knockback_resistance"), Some(KNOCKBACK_RESISTANCE));
        assert_eq!(name_of(MOVEMENT_SPEED), Some("movement_speed"));
        assert_eq!(by_name("nope"), None);
    }
}
