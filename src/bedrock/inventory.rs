// Инвентарь игрока Bedrock.
//
// У Bedrock инвентарём распоряжается сервер: клиент не двигает предметы сам,
// а присылает запрос (Item Stack Request) — «взять столько-то отсюда,
// положить туда», «создать предмет из творческого меню», «выбросить», — и
// ждёт ответа: принято и какие теперь стопки в затронутых слотах, или отказ.
// Даже в творческом режиме взятый из меню предмет появляется в руке только
// после такого ответа.
//
// Каждая стопка несёт сетевой номер: клиент ссылается на стопки по нему, а
// сервер в ответе сообщает новые номера. Здесь номер меняется при каждом
// изменении слота — этого достаточно, клиент берёт номера из ответа.
//
// Хранится инвентарь в общем виде (crate::inventory, раскладка слотов как у
// Java): так его можно записать в файл игрока тем же кодом. Раскладка
// Bedrock: панель — слоты 0–8, рюкзак — 9–35 (окно 0), броня — 0–3 (окно
// 120), вторая рука — окно 119. Устройство запросов и ответов — по описанию
// протокола (minecraft-data, protocol.json 1.26.10: ItemStackRequest,
// packet_item_stack_response, FullContainerName).
//
// Пока открыто окно хранилища (сундук, бочка, воронка…), его ячейки — тоже
// места запросов: клиент называет их контейнером LevelEntity (7) или Barrel
// (58), слоты — от нуля по порядку окна. Сами ячейки живут в мире; здесь —
// их копия, какой её знает клиент, с номерами стопок (world.rs берёт её из
// мира перед запросом и кладёт обратно после).

use std::collections::BTreeMap;

use super::codec::{In, Out};
use super::stations;
use crate::blocks;
use crate::inventory::{FIRST_HOTBAR, FIRST_MAIN, Inventory, LAST_HOTBAR, LAST_MAIN, OFFHAND, Stack};

/// Предмет Java → (номер Bedrock, метаданные, номер блока); см. make_bedrock_tables.py.
static TO_BEDROCK: &[u8] = include_bytes!("items_to_bedrock.bin");

/// Предмет Java для каждой записи творческого инвентаря по порядку.
static CREATIVE_ITEMS: &[u8] = include_bytes!("creative_items.bin");

/// Виды контейнеров (ContainerSlotType), с которыми работает сервер.
const ARMOR: u8 = 6;
const HOTBAR_AND_INVENTORY: u8 = 12;
const HOTBAR: u8 = 28;
const INVENTORY: u8 = 29;
const OFFHAND_SLOT: u8 = 34;
const CURSOR: u8 = 59;
const CREATIVE_OUTPUT: u8 = 60;

/// Имена ячеек хранилища (ContainerEnumName документации Mojang к r/26_u1):
/// сундук, воронка, раздатчик, выбрасыватель — LevelEntityContainer, у бочки
/// своё имя.
pub const LEVEL_ENTITY: u8 = 7;
pub const BARREL: u8 = 58;

/// Ячейки крафтера — CrafterLevelEntityContainer. [проверить] живым клиентом.
pub const CRAFTER: u8 = 62;

/// Окна (WindowID) для Inventory Content.
const WINDOW_INVENTORY: u32 = 0;
const WINDOW_OFFHAND: u32 = 119;
const WINDOW_ARMOR: u32 = 120;

/// Первый слот брони в общей раскладке.
const FIRST_ARMOR: i32 = 5;

/// Предмет Bedrock для предмета Java: номер, метаданные, номер блока.
pub fn bedrock_item(java: i32) -> Option<(i32, u32, i32)> {
    let entry = TO_BEDROCK.get(usize::try_from(java).ok()? * 8..)?.get(..8)?;
    let id = i16::from_le_bytes([entry[0], entry[1]]) as i32;
    let metadata = u16::from_le_bytes([entry[2], entry[3]]) as u32;
    let block = i32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);

    (id != 0).then_some((id, metadata, block))
}

/// Предмет Java по номеру записи творческого инвентаря (с единицы).
pub fn creative_item(entry: u32) -> Option<i32> {
    let at = (entry as usize).checked_sub(1)? * 2;
    let bytes = CREATIVE_ITEMS.get(at..at + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]) as i32)
}

/// Предмет в пакете (тип Item): пустой — один ноль.
pub fn write_item(out: &mut Out, stack: Option<Stack>, stack_id: i32) {
    let Some((stack, (id, metadata, block))) = stack.and_then(|s| Some((s, bedrock_item(s.item)?))) else {
        out.zigzag32(0);
        return;
    };

    out.zigzag32(id).lu16(stack.count as u16).varint(metadata);

    if stack_id != 0 {
        out.u8(1).zigzag32(stack_id);
    } else {
        out.u8(0);
    }

    // Дополнительные данные: без NBT, пустые списки «можно ставить на» и
    // «можно ломать». У щита после них ещё blocking_tick (li64): описание
    // протокола, ItemExtraDataWithBlockingTick — без него клиент не разберёт
    // пакет с щитом.
    out.zigzag32(block);

    // Особый динамит (custom_tnt.rs) — с именем и описанием в NBT предмета;
    // зачарования, износ и имя (станки) — тоже в NBT (stations::item_nbt).
    if let Some(user_data) = stack.tnt.and_then(crate::custom_tnt::by_id).map(custom_tnt_user_data).or_else(|| stations::item_nbt(&stack)) {
        out.varint((2 + 1 + user_data.len() + 4 + 4) as u32).li16(-1).u8(1).raw(&user_data).li32(0).li32(0);
        return;
    }

    if id == SHIELD {
        out.varint(18).lu16(0).li32(0).li32(0).li64(0);
    } else {
        out.varint(10).lu16(0).li32(0).li32(0);
    }
}

/// NBT предмета особого динамита: `{display:{Name:"§r§cМега-ТНТ",
/// Lore:["§r§7…"]}}`. Данные предмета по документации Mojang
/// (ItemInstanceUserData): метка −1, версия 1, затем Compound Tag — здесь
/// в NBT Bedrock с числами little-endian, как в его файлах мира.
/// [проверить] на живом клиенте.
fn custom_tnt_user_data(kind: &crate::custom_tnt::Kind) -> Vec<u8> {
    const COMPOUND: u8 = 10;
    const LIST: u8 = 9;
    const STRING: u8 = 8;

    fn text(out: &mut Vec<u8>, value: &str) {
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    let mut out = vec![COMPOUND];
    text(&mut out, "");

    out.push(COMPOUND);
    text(&mut out, "display");

    out.push(STRING);
    text(&mut out, "Name");
    text(&mut out, &crate::custom_tnt::bedrock_title(kind));

    out.push(LIST);
    text(&mut out, "Lore");
    out.push(STRING);
    out.extend_from_slice(&1i32.to_le_bytes());
    text(&mut out, &format!("§r§7{}", kind.lore));

    out.push(0); // конец display
    out.push(0); // конец корня
    out
}

/// Сетевой номер щита у Bedrock 1.26.10 (minecraft-data, items.json): у него
/// особый вид дополнительных данных предмета.
pub const SHIELD: i32 = 387;

/// Где лежит стопка.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Place {
    /// Слот общей раскладки.
    Slot(i32),
    /// То, что игрок держит курсором в открытом инвентаре.
    Cursor,
    /// Выход творческого меню: сюда кладётся созданный предмет.
    CreativeOutput,
    /// Ячейка открытого окна хранилища.
    Container(usize),
    /// Ячейка контейнера интерфейса игрока: сетка крафта, наковальня,
    /// стол зачарования, точило (stations.rs).
    Ui(u8),
}

/// Слот общей раскладки по контейнеру и слоту Bedrock.
fn place(container: u8, slot: u8) -> Option<Place> {
    let slot = slot as i32;

    match container {
        HOTBAR_AND_INVENTORY | HOTBAR | INVENTORY => match slot {
            0..=8 => Some(Place::Slot(FIRST_HOTBAR + slot)),
            9..=35 => Some(Place::Slot(slot)),
            _ => None,
        },
        ARMOR if slot < 4 => Some(Place::Slot(FIRST_ARMOR + slot)),
        OFFHAND_SLOT if slot < 2 => Some(Place::Slot(OFFHAND)),
        CURSOR => Some(Place::Cursor),
        CREATIVE_OUTPUT => Some(Place::CreativeOutput),
        _ => None,
    }
}

/// Инвентарь с тем, что нужно Bedrock: курсор, выход меню, номера стопок.
#[derive(Clone)]
pub struct Bag {
    pub inventory: Inventory,
    cursor: Option<Stack>,
    creative_output: Option<Stack>,
    /// Ячейки открытого окна хранилища, какими их знает клиент; None — окна нет.
    window: Option<Vec<Option<Stack>>>,
    /// Как клиент называет ячейки окна (вид контейнера, номер); пусто —
    /// все ячейки обычного хранилища.
    layout: Vec<(u8, u8)>,
    /// Какая печь или стойка в окне: что в какую ячейку можно класть.
    restrict: Option<crate::containers::Kind>,
    ids: BTreeMap<Place, i32>,
    next_id: i32,
    /// Ячейки интерфейса: сетка крафта, станки.
    ui: BTreeMap<u8, Stack>,
    /// Открытый станок игрока.
    pub station: Option<Station>,
    /// Уровень игрока для запросов (зачарование, наковальня).
    pub level: i32,
    /// Итог станка в этом запросе уже создан и его можно брать.
    craft_ready: bool,
    /// Что по запросу израсходовано: предмет, сколько, из ячейки интерфейса ли.
    consumed: Vec<(i32, i32, bool)>,
    /// Рецепт верстака этого запроса и сколько раз.
    recipe: Option<(&'static crate::crafting::Recipe, i32)>,
    /// Что станок попросил сделать миру, когда запрос удался.
    events: Vec<Event>,
    /// Предложения стола, какими их знает клиент.
    offers: Option<(Option<Stack>, i32)>,
}

/// Станок, открытый у игрока.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub kind: stations::Kind,
    pub pos: (i32, i32, i32),
    /// Семя зачарования и число полок вокруг стола.
    pub seed: i32,
    pub shelves: i32,
}

/// Что станок просит сделать миру после удавшегося запроса.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Потратить уровни (стол, наковальня).
    Levels(i32),
    /// Стол зачаровал: новое семя, звук.
    Enchanted((i32, i32, i32)),
    /// Наковальня сработала.
    AnvilUsed((i32, i32, i32)),
    /// Точило сработало: что лежало (для опыта), где.
    Ground([Option<Stack>; 2], (i32, i32, i32)),
}

/// Насколько разобран запрос.
#[derive(Clone, Copy, PartialEq)]
enum Parsed {
    /// Даже номера нет.
    Nothing,
    /// Номер есть, дальше не разобрать: ответ — отказ.
    Broken,
    /// Разобран до конца (выполнен или отказан).
    Whole,
}

/// Что вышло из запросов: ответ клиенту и то, что выброшено на землю.
pub struct Handled {
    pub response: Vec<u8>,
    pub thrown: Vec<Stack>,
    pub events: Vec<Event>,
}

/// Слот в запросе: контейнер, как его назвал клиент, и номер.
#[derive(Clone, Copy)]
struct Named {
    container: u8,
    dynamic: Option<u32>,
    slot: u8,
}


impl Bag {
    pub fn new(inventory: Inventory) -> Bag {
        let mut bag = Bag {
            inventory,
            cursor: None,
            creative_output: None,
            window: None,
            layout: Vec::new(),
            restrict: None,
            ids: BTreeMap::new(),
            next_id: 1,
            ui: BTreeMap::new(),
            station: None,
            level: 0,
            craft_ready: false,
            consumed: Vec::new(),
            recipe: None,
            events: Vec::new(),
            offers: None,
        };

        for slot in 0..crate::inventory::SLOTS as i32 {
            if bag.inventory.slot(slot).is_some() {
                bag.renumber(Place::Slot(slot));
            }
        }

        bag
    }

    fn get(&self, place: Place) -> Option<Stack> {
        match place {
            Place::Slot(slot) => self.inventory.slot(slot),
            Place::Cursor => self.cursor,
            Place::CreativeOutput => self.creative_output,
            Place::Container(slot) => self.window.as_ref()?.get(slot).copied().flatten(),
            Place::Ui(slot) => self.ui.get(&slot).copied(),
        }
    }

    fn set(&mut self, place: Place, stack: Option<Stack>) {
        let stack = stack.filter(|s| s.count > 0);

        match place {
            Place::Slot(slot) => self.inventory.set(slot, stack),
            Place::Cursor => self.cursor = stack,
            Place::CreativeOutput => self.creative_output = stack,
            Place::Container(slot) => {
                if let Some(place) = self.window.as_mut().and_then(|window| window.get_mut(slot)) {
                    *place = stack;
                }
            }
            Place::Ui(slot) => match stack {
                Some(stack) => {
                    self.ui.insert(slot, stack);
                }
                None => {
                    self.ui.remove(&slot);
                }
            },
        }

        if stack.is_some() {
            self.renumber(place);
        } else {
            self.ids.remove(&place);
        }
    }

    fn renumber(&mut self, place: Place) {
        self.ids.insert(place, self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
    }

    fn id(&self, place: Place) -> i32 {
        self.ids.get(&place).copied().unwrap_or(0)
    }

    /// Слоты изменились не по запросу клиента (подбор, постановка блока):
    /// им нужны новые номера стопок.
    pub fn touched(&mut self, slots: &[i32]) {
        for &slot in slots {
            let stack = self.inventory.slot(slot);
            self.set(Place::Slot(slot), stack);
        }
    }

    /// Выбранный слот панели (0–8).
    pub fn select(&mut self, slot: u8) {
        if slot < 9 {
            self.inventory.select(slot as i32);
        }
    }

    /// Инвентарь для файла игрока: то, что держат курсором, — тоже в нём,
    /// иначе при выходе оно пропало бы.
    pub fn inventory_with_cursor(&self) -> Inventory {
        let mut inventory = self.inventory.clone();

        if let Some(stack) = self.cursor {
            // Не влезло — в файле этого не будет; при выходе лишнее
            // бросается на землю (`put_cursor_back`).
            inventory.put(stack);
        }

        inventory
    }

    /// Уходя, игрок не уносит курсор: что в нём, возвращается в инвентарь.
    /// Что не влезло, отдаётся наружу — это бросают на землю.
    pub fn put_cursor_back(&mut self) -> Option<crate::inventory::Stack> {
        self.inventory.put(self.cursor.take()?).1
    }

    /// Инвентарь очищен (команда clear): пусто всё, кроме окна хранилища —
    /// его ячейки лежат в мире.
    pub fn clear(&mut self) {
        self.inventory = Inventory::new();
        self.cursor = None;
        self.creative_output = None;
        self.ui.clear();
        self.offers = None;
        self.ids.retain(|place, _| matches!(place, Place::Container(_)));
    }

    /// Открылось окно хранилища: его ячейки становятся местами запросов,
    /// каждой стопке — свой номер.
    pub fn open_window(&mut self, slots: &[Option<Stack>]) {
        self.open_window_for(slots, Vec::new(), None);
    }

    /// Окно печи или стойки: ячейки названы по-своему и действуют правила
    /// `furnace::may_place` (в выход ничего, в топливо — только топливо…).
    pub fn open_window_for(&mut self, slots: &[Option<Stack>], layout: Vec<(u8, u8)>, restrict: Option<crate::containers::Kind>) {
        self.close_window();
        self.window = Some(vec![None; slots.len()]);
        self.layout = layout;
        self.restrict = restrict;

        for (slot, stack) in slots.iter().enumerate() {
            self.set(Place::Container(slot), *stack);
        }
    }

    /// Окно хранилища закрылось: его ячеек у клиента больше нет.
    pub fn close_window(&mut self) {
        self.window = None;
        self.layout.clear();
        self.restrict = None;
        self.ids.retain(|place, _| !matches!(place, Place::Container(_)));
    }

    /// Открылся станок: его ячейки интерфейса пусты, предложения стола ещё
    /// не посланы.
    pub fn open_station(&mut self, station: Station) {
        self.station = Some(station);
        self.offers = None;
    }

    /// Станок закрылся (или закрыт инвентарь с сеткой 2×2): всё, что
    /// лежало в ячейках интерфейса, возвращается в инвентарь, лишнее
    /// отдаётся наружу — его бросают на землю. Слоты инвентаря, что
    /// поменялись, — тоже наружу (для пакетов).
    pub fn close_station(&mut self) -> (Vec<Stack>, Vec<i32>) {
        self.station = None;
        self.offers = None;

        let places: Vec<u8> = self.ui.keys().copied().collect();
        let mut overflow = Vec::new();
        let mut changed = Vec::new();

        for slot in places {
            let Some(stack) = self.get(Place::Ui(slot)) else {
                continue;
            };

            self.set(Place::Ui(slot), None);

            let (slots, left) = self.inventory.put(stack);

            changed.extend(slots);
            overflow.extend(left);
        }

        for &slot in &changed {
            self.touched(&[slot]);
        }

        (overflow, changed)
    }

    /// Свежие предложения стола, если предмет или семя сменились с прошлого
    /// раза (Player Enchant Options); пусто — менять нечего.
    pub fn enchant_options(&mut self) -> Option<Vec<u8>> {
        let station = self.station.filter(|station| station.kind == stations::Kind::Enchanting)?;
        let item = self.get(Place::Ui(stations::UI_ENCHANTING_INPUT));

        if self.offers == Some((item, station.seed)) {
            return None;
        }

        self.offers = Some((item, station.seed));
        Some(stations::enchant_options_packet(station.seed, station.shelves, item))
    }

    /// Ячейки открытого окна, какими их знает клиент.
    pub fn window_slots(&self) -> Option<&[Option<Stack>]> {
        self.window.as_deref()
    }

    /// Хранилище изменилось не по запросу этого клиента (воронка, другой
    /// игрок): ячейки, ставшие другими, получают новые стопки и номера.
    /// Возвращает, какие ячейки сменились.
    pub fn update_window(&mut self, now: &[Option<Stack>]) -> Vec<usize> {
        let Some(window) = &self.window else {
            return Vec::new();
        };
        let changed: Vec<usize> = (0..window.len().min(now.len())).filter(|&slot| window[slot] != now[slot]).collect();

        for &slot in &changed {
            self.set(Place::Container(slot), now[slot]);
        }

        changed
    }

    /// Всё окно хранилища пакетом Inventory Content: номер окна из Container
    /// Open, только ячейки хранилища (инвентарь игрока в это окно у Bedrock
    /// не входит), имя контейнера, пустой storage item.
    pub fn window_content_packets(&self, window: u8, container: u8) -> Vec<Vec<u8>> {
        if self.layout.is_empty() {
            return self.window_content_packet(window, container).into_iter().collect();
        }

        (0..self.layout.len()).filter_map(|slot| self.window_slot_packet(window, container, slot)).collect()
    }

    pub fn window_content_packet(&self, window: u8, container: u8) -> Option<Vec<u8>> {
        let slots = self.window.as_ref()?;
        let mut out = Out::packet(super::session::INVENTORY_CONTENT);
        out.varint(window as u32).varint(slots.len() as u32);

        for (slot, stack) in slots.iter().enumerate() {
            write_item(&mut out, *stack, self.id(Place::Container(slot)));
        }

        out.u8(container).u8(0).zigzag32(0);
        Some(out.bytes)
    }

    /// Одна ячейка окна хранилища пакетом Inventory Slot.
    pub fn window_slot_packet(&self, window: u8, container: u8, slot: usize) -> Option<Vec<u8>> {
        let stack = *self.window.as_ref()?.get(slot)?;
        let (container, number) = self.layout.get(slot).copied().unwrap_or((container, slot as u8));
        let mut out = Out::packet(super::session::INVENTORY_SLOT);
        out.varint(window as u32).varint(u32::from(number)).u8(container).u8(0).zigzag32(0);
        write_item(&mut out, stack, self.id(Place::Container(slot)));
        Some(out.bytes)
    }

    /// Весь инвентарь: рюкзак с панелью, броня, вторая рука.
    pub fn content_packets(&self) -> Vec<Vec<u8>> {
        let window = |window: u32, container: u8, slots: &[i32]| {
            let mut out = Out::packet(super::session::INVENTORY_CONTENT);
            out.varint(window).varint(slots.len() as u32);

            for &slot in slots {
                write_item(&mut out, self.inventory.slot(slot), self.id(Place::Slot(slot)));
            }

            out.u8(container).u8(0); // FullContainerName без динамического номера
            out.zigzag32(0); // storage_item — пусто
            out.bytes
        };

        let main: Vec<i32> = (FIRST_HOTBAR..=LAST_HOTBAR).chain(FIRST_MAIN..=LAST_MAIN).collect();
        let armor: Vec<i32> = (FIRST_ARMOR..FIRST_ARMOR + 4).collect();

        vec![
            window(WINDOW_INVENTORY, HOTBAR_AND_INVENTORY, &main),
            window(WINDOW_ARMOR, ARMOR, &armor),
            window(WINDOW_OFFHAND, OFFHAND_SLOT, &[OFFHAND]),
        ]
    }

    /// Один слот общей раскладки — пакетом Inventory Slot.
    pub fn slot_packet(&self, slot: i32) -> Option<Vec<u8>> {
        let (window, container, index) = match slot {
            FIRST_HOTBAR..=LAST_HOTBAR => (WINDOW_INVENTORY, HOTBAR_AND_INVENTORY, slot - FIRST_HOTBAR),
            FIRST_MAIN..=LAST_MAIN => (WINDOW_INVENTORY, HOTBAR_AND_INVENTORY, slot),
            5..=8 => (WINDOW_ARMOR, ARMOR, slot - FIRST_ARMOR),
            OFFHAND => (WINDOW_OFFHAND, OFFHAND_SLOT, 0),
            _ => return None,
        };

        let mut out = Out::packet(super::session::INVENTORY_SLOT);
        out.varint(window).varint(index as u32).u8(container).u8(0).zigzag32(0);
        write_item(&mut out, self.inventory.slot(slot), self.id(Place::Slot(slot)));
        Some(out.bytes)
    }

    /// Разбирает Item Stack Request и выполняет его. Запрос, который не
    /// удался, не меняет ничего: инвентарь откатывается целиком.
    pub fn handle_requests(&mut self, payload: &[u8], creative: bool) -> Handled {
        let mut reader = In::new(payload);
        let mut answers = Out::default();
        let mut answered = 0u32;
        let mut thrown = Vec::new();

        self.events.clear();

        for _ in 0..reader.varint().unwrap_or(0) {
            let parsed = self.handle_one(&mut reader, creative, &mut answers, &mut thrown);
            answered += u32::from(parsed != Parsed::Nothing);

            // Запрос не разобран до конца — где начинается следующий,
            // неизвестно: остальные не трогаем.
            if parsed != Parsed::Whole {
                break;
            }
        }

        let mut response = Out::packet(super::session::ITEM_STACK_RESPONSE);
        response.varint(answered).raw(&answers.bytes);
        Handled { response: response.bytes, thrown, events: std::mem::take(&mut self.events) }
    }

    /// Один запрос из Player Auth Input: там он лежит без списка вокруг.
    /// Ответ — отдельным пакетом, как и на обычные запросы.
    pub fn handle_embedded(&mut self, reader: &mut In, creative: bool) -> (Handled, bool) {
        let mut answers = Out::default();
        let mut thrown = Vec::new();
        self.events.clear();

        let parsed = self.handle_one(reader, creative, &mut answers, &mut thrown);

        let mut response = Out::packet(super::session::ITEM_STACK_RESPONSE);
        response.varint(u32::from(parsed != Parsed::Nothing)).raw(&answers.bytes);
        (Handled { response: response.bytes, thrown, events: std::mem::take(&mut self.events) }, parsed == Parsed::Whole)
    }

    /// Разбирает и выполняет один запрос, дописывая ответ в `answers`.
    /// Отказ не меняет ничего: инвентарь откатывается целиком.
    fn handle_one(&mut self, reader: &mut In, creative: bool, answers: &mut Out, thrown: &mut Vec<Stack>) -> Parsed {
        let Some(request_id) = reader.zigzag32() else {
            return Parsed::Nothing;
        };
        let before = self.clone();
        let mut touched: Vec<(Named, Place)> = Vec::new();
        let mut dropped = Vec::new();

        // Сначала разбираем всё: имя для наковальни стоит в хвосте, после
        // действий, а нужно оно уже при первом из них.
        let actions = Self::parse_actions(reader);

        // Хвост запроса: имена (для наковальни) и причина.
        let mut names: Vec<String> = Vec::new();
        let tail = actions.is_some()
            && (|| {
                for _ in 0..reader.varint()? {
                    let length = reader.varint()? as usize;
                    names.push(String::from_utf8_lossy(reader.take(length)?).into_owned());
                }
                reader.li32()
            })()
            .is_some();

        // Журнал для живой проверки экранов: что именно просит клиент.
        crate::log_debug!(
            "Bedrock: запрос {} — действия {:?}, имена {:?}, разобран {}",
            request_id,
            actions.as_ref().map(|acts| acts.iter().map(Act::label).collect::<Vec<_>>()),
            names,
            tail
        );

        let result = match actions {
            Some(actions) if tail => Some(self.run_actions(actions, &names, creative, &mut touched, &mut dropped) && self.window_allowed(&before)),
            _ => None,
        };

        if result == Some(true) {
            answers.u8(0).zigzag32(request_id);
            self.write_containers(answers, &touched);
            thrown.extend(dropped);
        } else {
            *self = before;
            answers.u8(1).zigzag32(request_id);
        }

        // Выход творческого меню и станков живёт только внутри одного
        // запроса: то, что там осталось, не должно дожить до следующего.
        self.set(Place::CreativeOutput, None);
        self.craft_ready = false;
        self.consumed.clear();
        self.recipe = None;

        if tail { Parsed::Whole } else { Parsed::Broken }
    }

    /// Место по слоту из запроса. Выход творческого меню есть только
    /// в творческом режиме (и у станков, пока итог создан): в выживании
    /// оттуда ничего не берётся. Ячейки хранилища — только пока его окно
    /// открыто, и не дальше его размера; ячейки интерфейса — пока открыт
    /// станок, которому они принадлежат (сетка 2×2 — всегда).
    fn place_for(&self, named: Named, creative: bool) -> Option<Place> {
        if let Some(slot) = stations::ui_slot(named.container, named.slot) {
            let open = stations::is_grid_2x2(slot) || self.station.is_some_and(|station| station.kind.has_slot(slot));

            return open.then_some(Place::Ui(slot));
        }

        if !self.layout.is_empty() && !matches!(named.container, HOTBAR_AND_INVENTORY | HOTBAR | INVENTORY | CURSOR) {
            let at = self.layout.iter().position(|&(container, slot)| container == named.container && slot == named.slot)?;

            return Some(Place::Container(at));
        }

        if matches!(named.container, LEVEL_ENTITY | BARREL | CRAFTER) {
            let size = self.window.as_ref()?.len();
            let slot = named.slot as usize;

            return (slot < size).then_some(Place::Container(slot));
        }

        place(named.container, named.slot).filter(|place| creative || self.craft_ready || *place != Place::CreativeOutput)
    }

    /// Не нарушены ли правила ячеек печи и стойки: что положили (или чем
    /// заменили) — должно годиться в ячейку и помещаться в неё.
    fn window_allowed(&self, before: &Bag) -> bool {
        let (Some(kind), Some(now), Some(was)) = (self.restrict, self.window.as_ref(), before.window.as_ref()) else {
            return true;
        };

        now.iter().zip(was).enumerate().all(|(slot, (now, was))| {
            let Some(stack) = now else {
                return true;
            };

            // Тот же предмет, не больше прежнего — ничего не добавлено.
            if was.is_some_and(|was| was.stacks_with(stack) && stack.count <= was.count) {
                return true;
            }

            crate::furnace::may_place(kind, slot, stack.item) && stack.count <= crate::furnace::slot_limit(kind, slot, stack.item)
        })
    }

    /// Разбирает действия запроса. None — разобрать не вышло.
    fn parse_actions(reader: &mut In) -> Option<Vec<Act>> {
        let mut acts = Vec::new();

        for _ in 0..reader.varint()? {
            let kind = reader.u8()?;

            acts.push(match kind {
                // Взять / положить: count штук из source в destination.
                0 | 1 | 7 | 8 => {
                    let count = reader.u8()? as i32;
                    let source = read_slot(reader)?;
                    let destination = read_slot(reader)?;
                    Act::Move { count, source, destination }
                }
                2 => Act::Swap(read_slot(reader)?, read_slot(reader)?),
                // Выбросить.
                3 => {
                    let count = reader.u8()? as i32;
                    let source = read_slot(reader)?;
                    let _randomly = reader.u8()?;
                    Act::Drop { count, source }
                }
                // Уничтожить (творческий) или израсходовать.
                4 | 5 => {
                    let count = reader.u8()? as i32;
                    let source = read_slot(reader)?;
                    Act::Consume { count, source, destroy: kind == 4 }
                }
                // «Создать» (остатки рецепта) и устаревший список итогов —
                // только сопровождают другие действия.
                6 => {
                    reader.u8()?;
                    Act::Skip
                }
                19 => {
                    for _ in 0..reader.varint()? {
                        skip_item_legacy(reader)?;
                    }
                    reader.u8()?;
                    Act::Skip
                }
                // Добыча блока инструментом — прочности у нас пока нет.
                11 => {
                    reader.zigzag32()?;
                    reader.zigzag32()?;
                    reader.zigzag32()?;
                    Act::Skip
                }
                // Платёж маяку.
                10 => {
                    reader.zigzag32()?;
                    reader.zigzag32()?;
                    Act::Unsupported
                }
                // Предмет из творческого меню.
                14 => {
                    let entry = reader.varint()?;
                    let _times = reader.u8()?;
                    Act::CraftCreative(entry)
                }
                // Рецепт верстака (и зачарование: предложение стола —
                // тоже «рецепт»).
                12 => {
                    let id = reader.varint()?;
                    let times = i32::from(reader.u8()?).max(1);
                    Act::Recipe { id, times }
                }
                // Рецепт из книги рецептов: ещё список ингредиентов.
                13 => {
                    let id = reader.varint()?;
                    let _times = reader.u8()?;
                    let times = i32::from(reader.u8()?).max(1);

                    for _ in 0..reader.varint()? {
                        skip_ingredient(reader)?;
                    }

                    Act::Recipe { id, times }
                }
                // Наковальня.
                15 => {
                    reader.varint()?;
                    reader.li32()?;
                    Act::Anvil
                }
                18 => Act::Anvil,
                // Точило.
                16 => {
                    reader.varint()?;
                    reader.u8()?;
                    reader.varint()?;
                    Act::Grind
                }
                // Ткацкий станок: узор по имени.
                17 => {
                    let length = reader.varint()? as usize;
                    reader.take(length)?;
                    reader.u8()?;
                    Act::Unsupported
                }
                // Всё остальное разобрать нельзя — отказ.
                _ => return None,
            });
        }

        Some(acts)
    }

    /// Выполняет действия запроса: удались ли все.
    fn run_actions(
        &mut self,
        actions: Vec<Act>,
        names: &[String],
        creative: bool,
        touched: &mut Vec<(Named, Place)>,
        dropped: &mut Vec<Stack>,
    ) -> bool {
        let mut failed = false;

        for act in actions {
            let ok = match act {
                Act::Move { count, source, destination } => self.move_stack(count, source, destination, creative, touched),
                Act::Swap(source, destination) => self.swap(source, destination, creative, touched),
                Act::Drop { count, source } => self.remove(count, source, creative, touched).map(|stack| dropped.push(stack)).is_some(),
                Act::Consume { count, source, destroy } => {
                    if destroy {
                        creative && self.remove(count, source, creative, touched).is_some()
                    } else {
                        self.consume(count, source, creative, touched)
                    }
                }
                Act::Skip => true,
                Act::CraftCreative(entry) => creative && self.craft_creative(entry),
                Act::Recipe { id, times } => self.craft_recipe(id, times, creative),
                Act::Anvil => self.anvil(names.first().map(String::as_str), creative),
                Act::Grind => self.grind(),
                Act::Unsupported => false,
            };

            failed |= !ok;
        }

        !failed && self.finish_station(creative)
    }

    fn craft_creative(&mut self, entry: u32) -> bool {
        let Some(item) = creative_item(entry) else {
            return false;
        };

        self.set(Place::CreativeOutput, Some(Stack::new(item, blocks::stack_size(item))));
        true
    }

    /// Расходует `count` из ячейки (действие consume). У ячейки интерфейса
    /// остаток рецепта (пустое ведро от молока) занимает её место.
    fn consume(&mut self, count: i32, source: Named, creative: bool, touched: &mut Vec<(Named, Place)>) -> bool {
        let Some(from) = self.place_for(source, creative) else {
            return false;
        };
        let Some(stack) = self.get(from) else {
            return false;
        };

        if count <= 0 || count > stack.count {
            return false;
        }

        let left = stack.count - count;
        let in_ui = matches!(from, Place::Ui(_));
        let mut rest = (left > 0).then(|| stack.with_count(left));

        if left == 0 && in_ui && self.recipe.is_some() && let Some(item) = crate::crafting::remainder(stack.item) {
            rest = Some(Stack::new(item, 1));
        }

        self.set(from, rest);
        touched.push((source, from));
        self.consumed.push((stack.item, count, in_ui));
        true
    }

    /// Рецепт верстака или предложение стола: создаёт итог. Что израсходовано
    /// и положено в сетку, сверяется в конце запроса (`finish_station`).
    fn craft_recipe(&mut self, id: u32, times: i32, creative: bool) -> bool {
        match self.station.map(|station| station.kind) {
            Some(stations::Kind::Enchanting) => self.enchant(id, creative),
            Some(stations::Kind::Workbench) | None => {
                let Some(recipe) = stations::recipe_by_network_id(id) else {
                    return false;
                };

                self.recipe = Some((recipe, times));
                self.craft_ready = true;
                self.set(Place::CreativeOutput, Some(Stack::new(recipe.result, (recipe.count * times).min(blocks::stack_size(recipe.result)))));
                true
            }
            Some(_) => false,
        }
    }

    /// Зачарование предложением стола (вики, Enchanting mechanics): итог —
    /// предмет с зачарованиями; лазурит и уровни уходят (как у Java,
    /// network/play/station.rs, button).
    fn enchant(&mut self, id: u32, creative: bool) -> bool {
        let Some(station) = self.station else {
            return false;
        };
        let Some(slot) = stations::offer_slot(station.seed, id) else {
            return false;
        };
        let need = slot as i32 + 1;
        let cost = i32::from(crate::enchanting::properties(station.seed, station.shelves, self.get(Place::Ui(stations::UI_ENCHANTING_INPUT)))[slot]);
        let Some(item) = self.get(Place::Ui(stations::UI_ENCHANTING_INPUT)) else {
            return false;
        };
        let lapis = self.get(Place::Ui(stations::UI_ENCHANTING_LAPIS));

        if !creative && lapis.is_none_or(|lapis| lapis.count < need) {
            return false;
        }

        if cost <= 0 || (!creative && (self.level < need || self.level < cost)) {
            return false;
        }

        let Some(enchanted) = crate::enchanting::enchant(station.seed, station.shelves, item, slot) else {
            return false;
        };

        self.craft_ready = true;
        self.set(Place::CreativeOutput, Some(enchanted));

        if !creative {
            self.events.push(Event::Levels(need));
        }

        self.events.push(Event::Enchanted(station.pos));
        true
    }

    /// Итог наковальни по тому, что лежит в её ячейках, и имени из запроса.
    fn anvil(&mut self, name: Option<&str>, creative: bool) -> bool {
        let Some(station) = self.station.filter(|station| station.kind == stations::Kind::Anvil) else {
            return false;
        };
        let left = self.get(Place::Ui(stations::UI_ANVIL_INPUT));
        let right = self.get(Place::Ui(stations::UI_ANVIL_MATERIAL));
        let outcome = crate::enchanting::anvil(left, right, name, creative);

        let Some(result) = outcome.result else {
            return false;
        };

        if outcome.cost <= 0 || (!creative && self.level < outcome.cost) {
            return false;
        }

        self.craft_ready = true;
        self.set(Place::CreativeOutput, Some(result));

        if !creative {
            self.events.push(Event::Levels(outcome.cost));
        }

        self.events.push(Event::AnvilUsed(station.pos));
        true
    }

    /// Итог точила.
    fn grind(&mut self) -> bool {
        let Some(station) = self.station.filter(|station| station.kind == stations::Kind::Grindstone) else {
            return false;
        };
        let top = self.get(Place::Ui(stations::UI_GRINDSTONE_INPUT));
        let bottom = self.get(Place::Ui(stations::UI_GRINDSTONE_ADDITIONAL));

        let Some(result) = crate::enchanting::grind(top, bottom) else {
            return false;
        };

        self.craft_ready = true;
        self.set(Place::CreativeOutput, Some(result));
        self.events.push(Event::Ground([top, bottom], station.pos));
        true
    }

    /// Конец запроса: то, что он расходовал, соответствует итогу, а ячейки
    /// станков не нарушают правил (на стол — одна вещь, в ячейку лазурита —
    /// лазурит, на точило — изнашиваемое или зачарованное).
    fn finish_station(&mut self, creative: bool) -> bool {
        if let Some((recipe, times)) = self.recipe {
            let mut units: Vec<i32> = Vec::new();

            for &(item, count, _) in &self.consumed {
                units.extend(std::iter::repeat_n(item, count as usize));
            }

            let per = units.len() / times.max(1) as usize;

            // На каждый раз — по набору ингредиентов.
            if !units.len().is_multiple_of(times.max(1) as usize) || units.chunks(per.max(1)).any(|chunk| !recipe.covered_by(chunk)) {
                return false;
            }

            // Форма сетки не сверяется: клиент раскладывает сам, сервер
            // сверяет, что израсходован набор рецепта. [проверить]
        }

        if let Some(station) = self.station {
            let lapis = blocks::item_named("lapis_lazuli");

            match station.kind {
                stations::Kind::Enchanting => {
                    let item = self.get(Place::Ui(stations::UI_ENCHANTING_INPUT));
                    let dust = self.get(Place::Ui(stations::UI_ENCHANTING_LAPIS));

                    if item.is_some_and(|item| item.count > 1) || dust.is_some_and(|dust| Some(dust.item) != lapis) {
                        return false;
                    }
                }
                stations::Kind::Grindstone => {
                    let both = [Place::Ui(stations::UI_GRINDSTONE_INPUT), Place::Ui(stations::UI_GRINDSTONE_ADDITIONAL)];

                    if both.iter().filter_map(|place| self.get(*place)).any(|stack| !crate::enchanting::grindable(&stack)) {
                        return false;
                    }
                }
                _ => {}
            }
        }

        let _ = creative;
        true
    }

    fn move_stack(
        &mut self,
        count: i32,
        source: Named,
        destination: Named,
        creative: bool,
        touched: &mut Vec<(Named, Place)>,
    ) -> bool {
        let (Some(from), Some(to)) = (self.place_for(source, creative), self.place_for(destination, creative)) else {
            return false;
        };
        let Some(moving) = self.get(from) else {
            return false;
        };

        if count <= 0 || count > moving.count {
            return false;
        }

        // Откуда и куда — одно место (тот же слот, названный панелью и
        // рюкзаком, или курсор в курсор): перекладывать нечего. Без этой
        // проверки стопка прибавлялась бы к самой себе.
        if from == to {
            return true;
        }

        let there = self.get(to);

        if there.is_some_and(|t| !t.stacks_with(&moving)) {
            return false;
        }

        let total = there.map_or(0, |t| t.count) + count;

        if total > blocks::stack_size(moving.item) {
            return false;
        }

        self.set(from, Some(moving.with_count(moving.count - count)));
        self.set(to, Some(moving.with_count(total)));
        touched.push((source, from));
        touched.push((destination, to));
        true
    }

    fn swap(&mut self, source: Named, destination: Named, creative: bool, touched: &mut Vec<(Named, Place)>) -> bool {
        let (Some(from), Some(to)) = (self.place_for(source, creative), self.place_for(destination, creative)) else {
            return false;
        };
        let (a, b) = (self.get(from), self.get(to));

        self.set(from, b);
        self.set(to, a);
        touched.push((source, from));
        touched.push((destination, to));
        true
    }

    fn remove(&mut self, count: i32, source: Named, creative: bool, touched: &mut Vec<(Named, Place)>) -> Option<Stack> {
        let from = self.place_for(source, creative)?;
        let stack = self.get(from)?;

        if count <= 0 || count > stack.count {
            return None;
        }

        self.set(from, Some(stack.with_count(stack.count - count)));
        touched.push((source, from));
        Some(stack.with_count(count))
    }

    /// Затронутые слоты в ответе — по контейнерам, как их назвал клиент.
    fn write_containers(&self, out: &mut Out, touched: &[(Named, Place)]) {
        let mut groups: Vec<(ContainerKey, Vec<(u8, Place)>)> = Vec::new();

        for (named, place) in touched {
            let key = (named.container, named.dynamic);
            let at = match groups.iter().position(|(k, _)| *k == key) {
                Some(at) => at,
                None => {
                    groups.push((key, Vec::new()));
                    groups.len() - 1
                }
            };

            if !groups[at].1.iter().any(|(slot, _)| *slot == named.slot) {
                groups[at].1.push((named.slot, *place));
            }
        }

        out.varint(groups.len() as u32);

        for ((container, dynamic), slots) in groups {
            out.u8(container);

            match dynamic {
                Some(id) => out.u8(1).raw(&id.to_be_bytes()),
                None => out.u8(0),
            };

            out.varint(slots.len() as u32);

            for (slot, place) in slots {
                let stack = self.get(place);
                out.u8(slot)
                    .u8(slot)
                    .u8(stack.map_or(0, |s| s.count) as u8)
                    .zigzag32(self.id(place))
                    .string("")
                    .string("")
                    .zigzag32(0);
            }
        }
    }
}

/// Одно действие запроса, разобранное.
enum Act {
    Move { count: i32, source: Named, destination: Named },
    Swap(Named, Named),
    Drop { count: i32, source: Named },
    Consume { count: i32, source: Named, destroy: bool },
    Skip,
    CraftCreative(u32),
    Recipe { id: u32, times: i32 },
    Anvil,
    Grind,
    Unsupported,
}

impl Act {
    /// Короткое имя действия для журнала.
    fn label(&self) -> &'static str {
        match self {
            Act::Move { .. } => "move",
            Act::Swap(..) => "swap",
            Act::Drop { .. } => "drop",
            Act::Consume { .. } => "consume",
            Act::Skip => "skip",
            Act::CraftCreative(_) => "craft_creative",
            Act::Recipe { .. } => "craft_recipe",
            Act::Anvil => "anvil",
            Act::Grind => "grind",
            Act::Unsupported => "unsupported",
        }
    }
}

/// Пропускает ингредиент рецепта (RecipeIngredient): вид, описание, число.
fn skip_ingredient(reader: &mut In) -> Option<()> {
    match reader.u8()? {
        0 => {}
        1 => {
            if reader.take(2)? != [0, 0] {
                reader.take(2)?;
            }
        }
        2 => {
            reader.varint().and_then(|length| reader.take(length as usize))?;
            reader.u8()?;
        }
        3 | 5 => {
            let length = reader.varint()? as usize;
            reader.take(length)?;
        }
        4 => {
            let length = reader.varint()? as usize;
            reader.take(length)?;
            reader.take(2)?;
        }
        _ => return None,
    }

    reader.zigzag32()?;
    Some(())
}

/// Контейнер, как его назвал клиент: вид и динамический номер.
type ContainerKey = (u8, Option<u32>);

/// Слот в запросе (StackRequestSlotInfo): контейнер, слот, номер стопки.
fn read_slot(reader: &mut In) -> Option<Named> {
    let container = reader.u8()?;
    let dynamic = match reader.u8()? {
        0 => None,
        _ => Some(u32::from_be_bytes(reader.take(4)?.try_into().ok()?)),
    };
    let slot = reader.u8()?;
    let _stack_id = reader.zigzag32()?;

    Some(Named { container, dynamic, slot })
}

/// Пропускает предмет без номера стопки (ItemLegacy).
fn skip_item_legacy(reader: &mut In) -> Option<()> {
    if reader.zigzag32()? == 0 {
        return Some(());
    }

    reader.take(2)?;
    reader.varint()?;
    reader.zigzag32()?;
    let extra = reader.varint()? as usize;
    reader.take(extra)?;
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Выход творческого меню — слот 50, так его называет клиент.
    const CREATIVE_OUTPUT_SLOT: u8 = 50;

    fn slot(out: &mut Out, container: u8, slot: u8) {
        out.u8(container).u8(0).u8(slot).zigzag32(0);
    }

    /// Запрос с одним набором действий; `actions` дописывает сами действия.
    fn request(id: i32, actions: u32, write: impl Fn(&mut Out)) -> Vec<u8> {
        let mut out = Out::default();
        out.varint(1).zigzag32(id).varint(actions);
        write(&mut out);
        out.varint(0).li32(0);
        out.bytes
    }

    /// Как клиент берёт камень из творческого меню: создать, взять в курсор,
    /// положить в панель.
    #[test]
    fn creative_item_lands_in_the_hotbar() {
        let stone_entry = (1..=1496).find(|entry| creative_item(*entry) == crate::blocks::item_named("stone")).expect("камень в меню");
        let mut bag = Bag::new(Inventory::new());

        let payload = request(-3, 3, |out| {
            out.u8(14).varint(stone_entry).u8(1);
            out.u8(0).u8(64);
            slot(out, CREATIVE_OUTPUT, CREATIVE_OUTPUT_SLOT);
            slot(out, CURSOR, 0);
            out.u8(1).u8(64);
            slot(out, CURSOR, 0);
            slot(out, HOTBAR_AND_INVENTORY, 2);
        });

        let handled = bag.handle_requests(&payload, true);
        let stone = crate::blocks::item_named("stone").expect("камень");

        assert_eq!(bag.inventory.slot(FIRST_HOTBAR + 2), Some(Stack::new(stone, 64)));
        assert_eq!(bag.get(Place::Cursor), None);

        // Ответ после номера пакета (0x94 — два байта varint): один ответ,
        // принято (0), номер запроса −3 (зигзаг — 5).
        assert_eq!(&handled.response[2..5], &[1, 0, 5]);
    }

    #[test]
    fn creative_items_are_refused_in_survival() {
        let mut bag = Bag::new(Inventory::new());
        let payload = request(1, 1, |out| {
            out.u8(14).varint(1).u8(1);
        });

        let handled = bag.handle_requests(&payload, false);
        assert_eq!(handled.response[3], 1); // отказ
        assert_eq!(bag.get(Place::CreativeOutput), None);
    }

    /// Неудачное действие откатывает и те, что прошли до него.
    #[test]
    fn a_failed_request_changes_nothing() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut inventory = Inventory::new();
        inventory.set(FIRST_HOTBAR, Some(Stack::new(stone, 10)));
        let mut bag = Bag::new(inventory);

        let payload = request(7, 2, |out| {
            out.u8(0).u8(5);
            slot(out, HOTBAR_AND_INVENTORY, 0);
            slot(out, CURSOR, 0);
            out.u8(0).u8(50); // больше, чем есть
            slot(out, HOTBAR_AND_INVENTORY, 0);
            slot(out, CURSOR, 0);
        });

        bag.handle_requests(&payload, false);
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR), Some(Stack::new(stone, 10)));
        assert_eq!(bag.get(Place::Cursor), None);
    }

    /// Отказанный запрос не мешает следующему в том же пакете.
    #[test]
    fn a_refused_request_does_not_stop_the_next() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut inventory = Inventory::new();
        inventory.set(FIRST_HOTBAR, Some(Stack::new(stone, 10)));
        let mut bag = Bag::new(inventory);

        let mut out = Out::default();
        out.varint(2);
        out.zigzag32(1).varint(1).u8(0).u8(50); // больше, чем есть — отказ
        slot(&mut out, HOTBAR_AND_INVENTORY, 0);
        slot(&mut out, CURSOR, 0);
        out.varint(0).li32(0);
        out.zigzag32(2).varint(1).u8(0).u8(4);
        slot(&mut out, HOTBAR_AND_INVENTORY, 0);
        slot(&mut out, CURSOR, 0);
        out.varint(0).li32(0);

        let handled = bag.handle_requests(&out.bytes, false);
        assert_eq!(handled.response[2], 2); // два ответа
        assert_eq!(bag.get(Place::Cursor), Some(Stack::new(stone, 4)));
    }

    /// Перенос стопки в то же место — панель и «панель с рюкзаком» с тем же
    /// номером слота, курсор в курсор — не размножает её.
    #[test]
    fn moving_a_stack_onto_itself_changes_nothing() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut inventory = Inventory::new();
        inventory.set(FIRST_HOTBAR, Some(Stack::new(stone, 10)));
        let mut bag = Bag::new(inventory);

        let payload = request(1, 4, |out| {
            out.u8(0).u8(5);
            slot(out, HOTBAR, 0);
            slot(out, HOTBAR_AND_INVENTORY, 0);
            out.u8(0).u8(10);
            slot(out, HOTBAR_AND_INVENTORY, 0);
            slot(out, HOTBAR_AND_INVENTORY, 0);
            out.u8(0).u8(4);
            slot(out, HOTBAR, 0);
            slot(out, CURSOR, 0);
            out.u8(1).u8(4);
            slot(out, CURSOR, 0);
            slot(out, CURSOR, 0);
        });

        let handled = bag.handle_requests(&payload, false);
        assert_eq!(handled.response[3], 0); // принято
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR), Some(Stack::new(stone, 6)));
        assert_eq!(bag.get(Place::Cursor), Some(Stack::new(stone, 4)));
    }

    /// Из выхода творческого меню в выживании ничего не взять, а то, что
    /// там осталось, не переживает запрос.
    #[test]
    fn the_creative_output_is_creative_only_and_short_lived() {
        let stone_entry = (1..=1496).find(|entry| creative_item(*entry) == crate::blocks::item_named("stone")).expect("камень в меню");
        let mut bag = Bag::new(Inventory::new());

        // Создать и не забрать: выход пуст после запроса.
        let payload = request(1, 1, |out| {
            out.u8(14).varint(stone_entry).u8(1);
        });
        bag.handle_requests(&payload, true);
        assert_eq!(bag.get(Place::CreativeOutput), None);

        // Даже если что-то там лежит — в выживании его не взять.
        bag.creative_output = crate::blocks::item_named("stone").map(|stone| Stack::new(stone, 64));
        let payload = request(2, 1, |out| {
            out.u8(0).u8(64);
            slot(out, CREATIVE_OUTPUT, CREATIVE_OUTPUT_SLOT);
            slot(out, HOTBAR_AND_INVENTORY, 0);
        });
        let handled = bag.handle_requests(&payload, false);
        assert_eq!(handled.response[3], 1); // отказ
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR), None);
    }

    #[test]
    fn dropped_items_are_thrown() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut inventory = Inventory::new();
        inventory.set(FIRST_MAIN, Some(Stack::new(stone, 3)));
        let mut bag = Bag::new(inventory);

        let payload = request(2, 1, |out| {
            out.u8(3).u8(1);
            slot(out, INVENTORY, 9);
            out.u8(0);
        });

        let handled = bag.handle_requests(&payload, false);
        assert_eq!(handled.thrown, vec![Stack::new(stone, 1)]);
        assert_eq!(bag.inventory.slot(FIRST_MAIN), Some(Stack::new(stone, 2)));
    }

    /// Shift-щелчок из сундука: клиент шлёт «взять» из ячейки хранилища
    /// (LevelEntity, 7) прямо в панель — стопка переезжает, в ответе оба
    /// контейнера так, как их назвал клиент.
    #[test]
    fn a_stack_moves_out_of_a_chest() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut chest = vec![None; 54];
        chest[30] = Some(Stack::new(stone, 10));
        let mut bag = Bag::new(Inventory::new());
        bag.open_window(&chest);

        let payload = request(4, 1, |out| {
            out.u8(0).u8(10);
            slot(out, LEVEL_ENTITY, 30);
            slot(out, HOTBAR_AND_INVENTORY, 3);
        });

        let handled = bag.handle_requests(&payload, false);
        assert_eq!(handled.response[3], 0); // принято
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR + 3), Some(Stack::new(stone, 10)));
        assert_eq!(bag.window_slots().map(|slots| slots[30]), Some(None));

        // Ответ: два контейнера — 7 (слот 30, пусто) и 12 (слот 3, 10 штук).
        let mut reader = In::new(&handled.response[5..]);
        assert_eq!(reader.varint(), Some(2));
        assert_eq!((reader.u8(), reader.u8()), (Some(LEVEL_ENTITY), Some(0)));
        assert_eq!(reader.varint(), Some(1));
        assert_eq!((reader.u8(), reader.u8(), reader.u8()), (Some(30), Some(30), Some(0)));
    }

    /// Обратно — из курсора в бочку (имя 58), половину стопки.
    #[test]
    fn a_stack_goes_into_a_barrel() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut inventory = Inventory::new();
        inventory.set(FIRST_MAIN, Some(Stack::new(stone, 8)));
        let mut bag = Bag::new(inventory);
        bag.open_window(&[None; 27]);

        let payload = request(5, 2, |out| {
            out.u8(0).u8(8);
            slot(out, INVENTORY, 9);
            slot(out, CURSOR, 0);
            out.u8(1).u8(4);
            slot(out, CURSOR, 0);
            slot(out, BARREL, 26);
        });

        let handled = bag.handle_requests(&payload, false);
        assert_eq!(handled.response[3], 0);
        assert_eq!(bag.window_slots().map(|slots| slots[26]), Some(Some(Stack::new(stone, 4))));
        assert_eq!(bag.get(Place::Cursor), Some(Stack::new(stone, 4)));
    }

    /// Без открытого окна и за его краем ячеек хранилища нет: отказ, и
    /// ничего не меняется.
    #[test]
    fn container_slots_exist_only_while_the_window_is_open() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut inventory = Inventory::new();
        inventory.set(FIRST_HOTBAR, Some(Stack::new(stone, 5)));
        let mut bag = Bag::new(inventory);
        let payload = |to: u8| {
            request(6, 1, |out| {
                out.u8(1).u8(5);
                slot(out, HOTBAR, 0);
                slot(out, LEVEL_ENTITY, to);
            })
        };

        assert_eq!(bag.handle_requests(&payload(0), false).response[3], 1);

        bag.open_window(&[None; 5]); // воронка
        assert_eq!(bag.handle_requests(&payload(5), false).response[3], 1);
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR), Some(Stack::new(stone, 5)));

        assert_eq!(bag.handle_requests(&payload(4), false).response[3], 0);
        assert_eq!(bag.window_slots().map(|slots| slots[4]), Some(Some(Stack::new(stone, 5))));

        bag.close_window();
        assert_eq!(bag.window_slots(), None);
        assert!(!bag.ids.keys().any(|place| matches!(place, Place::Container(_))));
    }

    /// Что изменилось без клиента, получает новый номер стопки; окно
    /// пакетом Inventory Content — номер окна, все ячейки, имя контейнера.
    #[test]
    fn window_updates_and_content() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut bag = Bag::new(Inventory::new());
        bag.open_window(&[None; 9]);

        let mut now = vec![None; 9];
        now[2] = Some(Stack::new(stone, 1));
        assert_eq!(bag.update_window(&now), vec![2]);
        assert_ne!(bag.id(Place::Container(2)), 0);
        assert_eq!(bag.update_window(&now), Vec::<usize>::new());

        let packet = bag.window_content_packet(4, LEVEL_ENTITY).expect("окно открыто");
        let mut reader = In::new(&packet);
        assert_eq!(reader.varint(), Some(super::super::session::INVENTORY_CONTENT));
        assert_eq!(reader.varint(), Some(4));
        assert_eq!(reader.varint(), Some(9));
        assert_eq!(reader.zigzag32(), Some(0));
        assert_eq!(reader.zigzag32(), Some(0));
        assert_eq!(reader.zigzag32(), Some(1)); // камень у Bedrock — 1
        reader.take(2).expect("количество");
        reader.varint().expect("метаданные");
        assert_eq!(reader.u8(), Some(1)); // есть номер стопки
        assert_eq!(reader.zigzag32(), Some(bag.id(Place::Container(2))));
        reader.zigzag32().expect("блок");
        let extra = reader.varint().expect("длина") as usize;
        reader.take(extra).expect("данные");
        for _ in 3..9 {
            assert_eq!(reader.zigzag32(), Some(0));
        }
        assert_eq!((reader.u8(), reader.u8(), reader.zigzag32()), (Some(LEVEL_ENTITY), Some(0), Some(0)));
        assert_eq!(reader.u8(), None);

        // Очистка инвентаря окно не трогает.
        bag.clear();
        assert_eq!(bag.window_slots().map(|slots| slots[2]), Some(Some(Stack::new(stone, 1))));
    }

    /// Номер щита совпадает с реестром предметов, который получает клиент,
    /// и щит пишется с blocking_tick: дополнительные данные — 18 байт.
    #[test]
    fn a_shield_carries_its_blocking_tick() {
        let registry = include_bytes!("item_registry.bin");
        let name = b"minecraft:shield";
        let at = registry.windows(name.len() + 1).position(|w| w[0] as usize == name.len() && &w[1..] == name).expect("щит в реестре");
        let id = i16::from_le_bytes([registry[at + 1 + name.len()], registry[at + 2 + name.len()]]) as i32;
        assert_eq!(id, SHIELD);

        let shield = crate::blocks::item_named("shield").expect("щит");
        assert_eq!(bedrock_item(shield).map(|(id, _, _)| id), Some(SHIELD));

        let mut out = Out::default();
        write_item(&mut out, Some(Stack::new(shield, 1)), 0);
        let mut reader = In::new(&out.bytes);
        assert_eq!(reader.zigzag32(), Some(SHIELD));
        reader.take(2).expect("количество");
        reader.varint().expect("метаданные");
        assert_eq!(reader.u8(), Some(0));
        reader.zigzag32().expect("блок");
        assert_eq!(reader.varint(), Some(18));
        assert_eq!(reader.take(18).map(|rest| rest.len()), Some(18));
        assert_eq!(reader.u8(), None);

        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut out = Out::default();
        write_item(&mut out, Some(Stack::new(stone, 1)), 0);
        assert_eq!(out.bytes.len(), 1 + 2 + 1 + 1 + 2 + 1 + 10);
    }

    /// В творческом меню щит тоже с blocking_tick (make_bedrock_tables.py).
    #[test]
    fn the_creative_shield_carries_its_blocking_tick() {
        let mut reader = In::new(include_bytes!("creative_content.bin"));

        for _ in 0..reader.varint().expect("вкладки") {
            reader.li32().expect("категория");
            reader.string().expect("имя");
            assert_eq!(reader.zigzag32(), Some(0));
        }

        let mut shields = 0;

        for _ in 0..reader.varint().expect("предметы") {
            reader.varint().expect("номер записи");
            let id = reader.zigzag32().expect("предмет");
            reader.take(2).expect("количество");
            reader.varint().expect("метаданные");
            reader.zigzag32().expect("блок");
            let extra = reader.varint().expect("длина") as usize;
            assert_eq!(extra, if id == SHIELD { 18 } else { 10 });
            shields += usize::from(id == SHIELD);
            reader.take(extra).expect("дополнительные данные");
            reader.varint().expect("вкладка");
        }

        assert_eq!(shields, 1);
        assert_eq!(reader.u8(), None);
    }

    #[test]
    fn custom_tnt_carries_its_name() {
        let kind = crate::custom_tnt::by_name("mega").expect("mega");
        let stack = crate::custom_tnt::stack(kind, 3).expect("динамит");
        let mut out = Out::packet(0);

        write_item(&mut out, Some(stack), 0);

        let mut reader = In::new(&out.bytes);
        reader.varint().expect("номер пакета");
        reader.zigzag32().expect("предмет");
        reader.take(2).expect("число");
        reader.varint().expect("метаданные");
        assert_eq!(reader.u8(), Some(0));
        reader.zigzag32().expect("блок");

        let extra = reader.varint().expect("длина") as usize;
        let data = reader.take(extra).expect("данные");

        assert_eq!(reader.take(1), None, "лишние байты");
        assert_eq!(&data[..3], &[0xff, 0xff, 1]);
        assert_eq!(&data[data.len() - 8..], &[0; 8]);

        let title = crate::custom_tnt::bedrock_title(kind);
        let found = data.windows(title.len()).any(|window| window == title.as_bytes());
        assert!(found, "имени нет в NBT");
    }

    #[test]
    fn java_items_have_bedrock_forms() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        assert_eq!(bedrock_item(stone), Some((1, 0, 2533)));

        let red_bed = crate::blocks::item_named("red_bed").expect("кровать");
        assert_eq!(bedrock_item(red_bed).map(|(_, metadata, _)| metadata), Some(14));
    }

    /// Запрос со своим набором имён (наковальня).
    fn named_request(id: i32, name: &str, actions: u32, write: impl Fn(&mut Out)) -> Vec<u8> {
        let mut out = Out::default();
        out.varint(1).zigzag32(id).varint(actions);
        write(&mut out);
        out.varint(1).string(name).li32(0);
        out.bytes
    }

    fn consume_action(out: &mut Out, count: u8, container: u8, at: u8) {
        out.u8(5).u8(count);
        slot(out, container, at);
    }

    fn take_output(out: &mut Out, count: u8) {
        out.u8(0).u8(count);
        slot(out, CREATIVE_OUTPUT, 50);
        slot(out, CURSOR, 0);
    }

    fn item(name: &str, count: i32) -> Stack {
        Stack::new(crate::blocks::item_named(name).expect(name), count)
    }

    /// Верстак: четыре доски в сетке 2×2 → верстак в курсор; набор
    /// проверяется по израсходованному, подделка отклоняется.
    #[test]
    fn the_inventory_grid_crafts_by_recipe() {
        let table = crate::blocks::item_named("crafting_table").expect("верстак");
        let id = (1..2000).find(|id| stations::recipe_by_network_id(*id).is_some_and(|recipe| recipe.result == table)).expect("рецепт");
        let mut bag = Bag::new(Inventory::new());

        for at in 0..4 {
            bag.set(Place::Ui(stations::UI_GRID_2X2 + at), Some(item("oak_planks", 1)));
        }

        let craft = |bag: &mut Bag, used: u8| {
            let payload = request(-9, 2 + u32::from(used), |out| {
                out.u8(12).varint(id).u8(1);

                for at in 0..used {
                    consume_action(out, 1, stations::CRAFTING_INPUT, stations::UI_GRID_2X2 + at);
                }

                take_output(out, 1);
            });

            bag.handle_requests(&payload, false)
        };

        // Израсходовано три доски из четырёх — не рецепт.
        craft(&mut bag, 3);
        assert_eq!(bag.get(Place::Cursor), None);
        assert_eq!(bag.get(Place::Ui(stations::UI_GRID_2X2)), Some(item("oak_planks", 1)));

        craft(&mut bag, 4);
        assert_eq!(bag.get(Place::Cursor), Some(Stack::new(table, 1)));
        assert_eq!(bag.get(Place::Ui(stations::UI_GRID_2X2 + 3)), None);

        // Сетка 3×3 без верстака недоступна.
        let payload = request(-11, 1, |out| {
            out.u8(1).u8(1);
            slot(out, CURSOR, 0);
            slot(out, stations::CRAFTING_INPUT, stations::UI_GRID_3X3);
        });

        bag.handle_requests(&payload, false);
        assert_eq!(bag.get(Place::Cursor), Some(Stack::new(table, 1)));
    }

    /// Остаток рецепта: от молока остаётся ведро (торт).
    #[test]
    fn a_milk_bucket_leaves_a_bucket() {
        let cake = crate::blocks::item_named("cake").expect("торт");
        let id = (1..2000).find(|id| stations::recipe_by_network_id(*id).is_some_and(|recipe| recipe.result == cake)).expect("рецепт");
        let mut bag = Bag::new(Inventory::new());

        bag.open_station(Station { kind: stations::Kind::Workbench, pos: (0, 0, 0), seed: 0, shelves: 0 });

        let layout = ["milk_bucket", "milk_bucket", "milk_bucket", "sugar", "egg", "sugar", "wheat", "wheat", "wheat"];

        for (at, name) in layout.iter().enumerate() {
            bag.set(Place::Ui(stations::UI_GRID_3X3 + at as u8), Some(item(name, 1)));
        }

        let payload = request(-13, 11, |out| {
            out.u8(12).varint(id).u8(1);

            for at in 0..9 {
                consume_action(out, 1, stations::CRAFTING_INPUT, stations::UI_GRID_3X3 + at);
            }

            take_output(out, 1);
        });

        bag.handle_requests(&payload, false);
        assert_eq!(bag.get(Place::Cursor), Some(Stack::new(cake, 1)));
        assert_eq!(bag.get(Place::Ui(stations::UI_GRID_3X3)), Some(item("bucket", 1)));
    }

    /// Стол зачарования: предложение стоит уровней и лазурита (события),
    /// итог зачарован.
    #[test]
    fn the_table_enchants_by_offer() {
        let seed = -1_346_738_504;
        let mut bag = Bag::new(Inventory::new());

        bag.level = 30;
        bag.open_station(Station { kind: stations::Kind::Enchanting, pos: (1, 2, 3), seed, shelves: 15 });
        bag.set(Place::Ui(stations::UI_ENCHANTING_INPUT), Some(item("diamond_sword", 1)));
        bag.set(Place::Ui(stations::UI_ENCHANTING_LAPIS), Some(item("lapis_lazuli", 10)));

        let id = stations::offer_id(seed, 2);
        let payload = request(-15, 4, |out| {
            out.u8(12).varint(id).u8(1);
            consume_action(out, 1, stations::ENCHANTING_INPUT, stations::UI_ENCHANTING_INPUT);
            consume_action(out, 3, stations::ENCHANTING_LAPIS, stations::UI_ENCHANTING_LAPIS);
            take_output(out, 1);
        });

        let handled = bag.handle_requests(&payload, false);
        let sword = bag.get(Place::Cursor).expect("зачарованный меч");

        assert!(!sword.extras().enchants.is_empty());
        assert_eq!(handled.events, vec![Event::Levels(3), Event::Enchanted((1, 2, 3))]);
        assert_eq!(bag.get(Place::Ui(stations::UI_ENCHANTING_LAPIS)), Some(item("lapis_lazuli", 7)));

        // Не хватает уровней — отказ.
        let mut poor = Bag::new(Inventory::new());

        poor.level = 2;
        poor.open_station(Station { kind: stations::Kind::Enchanting, pos: (1, 2, 3), seed, shelves: 15 });
        poor.set(Place::Ui(stations::UI_ENCHANTING_INPUT), Some(item("diamond_sword", 1)));
        poor.set(Place::Ui(stations::UI_ENCHANTING_LAPIS), Some(item("lapis_lazuli", 10)));
        assert!(poor.handle_requests(&payload, false).events.is_empty());
        assert_eq!(poor.get(Place::Cursor), None);
    }

    /// Наковальня: переименование стоит уровень; точило снимает зачарования.
    #[test]
    fn the_anvil_renames_and_the_grindstone_strips() {
        let mut bag = Bag::new(Inventory::new());

        bag.level = 5;
        bag.open_station(Station { kind: stations::Kind::Anvil, pos: (4, 5, 6), seed: 0, shelves: 0 });
        bag.set(Place::Ui(stations::UI_ANVIL_INPUT), Some(item("diamond_sword", 1)));

        let payload = named_request(-17, "Excalibur", 3, |out| {
            out.u8(15).varint(1).li32(0);
            consume_action(out, 1, stations::ANVIL_INPUT, stations::UI_ANVIL_INPUT);
            take_output(out, 1);
        });

        let handled = bag.handle_requests(&payload, false);

        assert_eq!(bag.get(Place::Cursor).and_then(|stack| stack.extras().name), Some("Excalibur".to_string()));
        assert_eq!(handled.events, vec![Event::Levels(1), Event::AnvilUsed((4, 5, 6))]);

        // Без предмета — нечего считать.
        let handled = bag.handle_requests(&payload, false);
        assert!(handled.events.is_empty());

        let mut stone = Bag::new(Inventory::new());
        let mut data = item("diamond_pickaxe", 1).extras();

        data.set_enchant(crate::stackdata::enchantment_number("efficiency").expect("эффективность"), 3);
        stone.open_station(Station { kind: stations::Kind::Grindstone, pos: (7, 8, 9), seed: 0, shelves: 0 });
        stone.set(Place::Ui(stations::UI_GRINDSTONE_INPUT), Some(item("diamond_pickaxe", 1).with_extras(data)));

        let payload = request(-19, 3, |out| {
            out.u8(16).varint(1).u8(1).varint(0);
            consume_action(out, 1, stations::GRINDSTONE_INPUT, stations::UI_GRINDSTONE_INPUT);
            take_output(out, 1);
        });

        let handled = stone.handle_requests(&payload, false);

        assert!(stone.get(Place::Cursor).is_some_and(|stack| stack.extras().enchants.is_empty()));
        assert!(matches!(handled.events[..], [Event::Ground(_, (7, 8, 9))]));
    }

    /// Закрытие станка возвращает вещи в инвентарь.
    #[test]
    fn closing_a_station_returns_the_items() {
        let mut bag = Bag::new(Inventory::new());

        bag.open_station(Station { kind: stations::Kind::Anvil, pos: (0, 0, 0), seed: 0, shelves: 0 });
        bag.set(Place::Ui(stations::UI_ANVIL_INPUT), Some(item("diamond_sword", 1)));

        let (overflow, changed) = bag.close_station();

        assert!(overflow.is_empty());
        assert_eq!(changed.len(), 1);
        assert_eq!(bag.get(Place::Ui(stations::UI_ANVIL_INPUT)), None);
        assert_eq!(bag.inventory.slot(changed[0]), Some(item("diamond_sword", 1)));
    }

    /// Печь: в выход ничего не кладут, топливо — только топливо.
    #[test]
    fn furnace_cells_keep_their_rules() {
        let slots = [None, None, None];
        let mut inventory = Inventory::new();

        inventory.set(FIRST_HOTBAR, Some(item("stick", 1)));
        inventory.set(FIRST_HOTBAR + 1, Some(item("stone", 5)));

        let mut bag = Bag::new(inventory);

        bag.open_window_for(&slots, stations::layout(crate::containers::Kind::Furnace).expect("печь"), Some(crate::containers::Kind::Furnace));

        let put = |bag: &mut Bag, from: u8, container: u8, to: u8| {
            let payload = request(-21, 1, |out| {
                out.u8(1).u8(1);
                slot(out, HOTBAR_AND_INVENTORY, from);
                slot(out, container, to);
            });

            bag.handle_requests(&payload, false);
        };

        put(&mut bag, 0, stations::FURNACE_RESULT, 2);
        assert_eq!(bag.window_slots().expect("окно")[2], None);

        put(&mut bag, 1, stations::FURNACE_FUEL, 1);
        assert_eq!(bag.window_slots().expect("окно")[1], None);

        put(&mut bag, 0, stations::FURNACE_FUEL, 1);
        assert_eq!(bag.window_slots().expect("окно")[1], Some(item("stick", 1)));

        put(&mut bag, 1, stations::FURNACE_INGREDIENT, 0);
        assert_eq!(bag.window_slots().expect("окно")[0], Some(item("stone", 1)));
    }
}
