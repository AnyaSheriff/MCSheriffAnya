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
pub const SHULKER: u8 = 30;
pub const BARREL: u8 = 58;

/// Ячейки крафтера — CrafterLevelEntityContainer. [проверить] живым клиентом.
pub const CRAFTER: u8 = 62;

/// Окна (WindowID) для Inventory Content.
const WINDOW_INVENTORY: u32 = 0;
const WINDOW_OFFHAND: u32 = 119;
const WINDOW_ARMOR: u32 = 120;
const WINDOW_UI: u32 = 124;

/// Сколько ячеек в окне интерфейса у BDS: курсор, сетка, предложения станков.
const UI_SLOTS: usize = 54;

/// Player Hotbar (PlayerHotbarPacket): номер ячейки, окно, «выбрать».
const PLAYER_HOTBAR: u32 = 0x30;

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

    let metadata = super::item_components::metadata(&stack, metadata);
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
        let blocking = if id == SHIELD { 8 } else { 0 };
        out.varint((2 + 1 + user_data.len() + 4 + 4 + blocking) as u32).li16(-1).u8(1).raw(&user_data).li32(0).li32(0);
        // Щит с NBT тоже содержит blocking_tick (ItemExtraDataWithBlockingTick).
        if id == SHIELD { out.li64(0); }
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
    pub disabled_slots: u16,
    ids: BTreeMap<Place, i32>,
    next_id: i32,
    /// Ячейки интерфейса: сетка крафта, станки.
    ui: BTreeMap<u8, Stack>,
    /// Открытый станок игрока.
    pub station: Option<Station>,
    /// Уровень игрока для запросов (зачарование, наковальня).
    pub level: i32,
    pub allowed_crafting: Option<std::collections::BTreeSet<usize>>,
    pub cartography_maps: Option<crate::maps::Maps>,
    /// Итог станка в этом запросе уже создан и его можно брать.
    craft_ready: bool,
    /// Что по запросу израсходовано: предмет, сколько, из ячейки интерфейса ли.
    consumed: Vec<(i32, i32, bool)>,
    /// Кузнечный стол: предметы, которые итог требует израсходовать (шаблон,
    /// основа, добавка).
    smithing: Option<[i32; 3]>,
    station_spend: Option<Vec<(Place,Stack)>>,
    station_consumed: Vec<(Place,Stack)>,
    planned_crafting: Vec<(Place,Stack)>,
    /// Рецепт верстака этого запроса и сколько раз.
    recipe: Option<(&'static crate::crafting::Recipe, i32)>,
    /// Что станок попросил сделать миру, когда запрос удался.
    events: Vec<Event>,
    /// Предложения стола, какими их знает клиент.
    offers: Option<(Option<Stack>, i32)>,
    /// Сделки открытого окна торговли (trade.rs) и та, что выбрана запросом.
    trade: Vec<super::trade::Price>,
    trade_pick: Option<usize>,
    /// Сколько сделок в запросе (пачка по Shift).
    trade_times: i32,
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
    Crafted(i32, i32),
    BeaconSelected((i32, i32, i32), i32, i32),
    /// Потратить уровни (стол, наковальня).
    Levels(i32),
    /// Состоялась сделка с жителем (номер сделки в его списке).
    Traded(usize),
    /// Стол зачаровал: новое семя, звук.
    Enchanted((i32, i32, i32)),
    /// Наковальня сработала.
    AnvilUsed((i32, i32, i32)),
    /// Точило сработало: что лежало (для опыта), где.
    Ground([Option<Stack>; 2], (i32, i32, i32)),
    /// Кузнечный стол сработал (звук — событие мира 1044).
    Smithed((i32, i32, i32)),
    StationWorked(stations::Kind,(i32,i32,i32)),
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
    pub fn new(mut inventory: Inventory) -> Bag {
        // Playerdata v10 хранит курсор отдельно от слотов. У Bag курсор свой:
        // переносим его ровно один раз, в общей Inventory копии не оставляем.
        let cursor = inventory.cursor();
        inventory.set_cursor(None);
        let mut bag = Bag {
            inventory,
            cursor,
            creative_output: None,
            window: None,
            layout: Vec::new(),
            restrict: None,
            disabled_slots: 0,
            ids: BTreeMap::new(),
            next_id: 1,
            ui: BTreeMap::new(),
            station: None,
            level: 0,
            cartography_maps: None,
            allowed_crafting: None,
            craft_ready: false,
            consumed: Vec::new(),
            smithing: None,
            station_spend: None,
            station_consumed: Vec::new(),
            planned_crafting: Vec::new(),
            recipe: None,
            events: Vec::new(),
            offers: None,
            trade: Vec::new(),
            trade_pick: None,
            trade_times: 1,
        };

        for slot in 0..crate::inventory::SLOTS as i32 {
            if bag.inventory.slot(slot).is_some() {
                bag.renumber(Place::Slot(slot));
            }
        }

        if bag.cursor.is_some() { bag.renumber(Place::Cursor); }
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

        // Не вталкиваем в слоты: они могут быть полностью заняты. v10
        // сохраняет курсор с количеством и компонентами отдельным хвостом.
        inventory.set_cursor(self.cursor);

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
    #[cfg(test)]
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
        self.disabled_slots = 0;
        self.ids.retain(|place, _| !matches!(place, Place::Container(_)));
    }

    /// Открылся станок: его ячейки интерфейса пусты, предложения стола ещё
    /// не посланы.
    pub fn open_station(&mut self, station: Station) {
        self.station = Some(station);
        self.offers = None;
    }

    /// Есть ли у сделки запас — перед тем, как посчитать очередную сделку пачки.
    pub fn trade_stocked(&self, index: usize) -> bool {
        self.trade.get(index).is_none_or(|price| price.stocked)
    }

    /// Сделки открытого окна торговли: что житель просит и что отдаёт.
    pub fn set_trade(&mut self, prices: Vec<super::trade::Price>) {
        self.trade = prices;
    }

    /// Станок закрылся (или закрыт инвентарь с сеткой 2×2): всё, что
    /// лежало в ячейках интерфейса, возвращается в инвентарь, лишнее
    /// отдаётся наружу — его бросают на землю. Слоты инвентаря, что
    /// поменялись, — тоже наружу (для пакетов).
    pub fn close_station(&mut self) -> (Vec<Stack>, Vec<i32>) {
        self.station = None;
        self.offers = None;
        self.trade.clear();

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

    /// Содержимое окна хранилища одним пакетом, ячейки — в порядке окна
    /// Bedrock (у печи и стойки он не тот, что у Java: `layout` называет
    /// место каждой ячейки), с нулевым именем контейнера, как у BDS.
    pub fn window_full_packet(&self, window: u8) -> Option<Vec<u8>> {
        let slots = self.window.as_ref()?;
        let mut order: Vec<Option<Stack>> = vec![None; slots.len()];
        let mut ids = vec![0; slots.len()];

        for (slot, stack) in slots.iter().enumerate() {
            let at = self.layout.get(slot).map_or(slot, |&(_, number)| usize::from(number));

            if at < order.len() {
                order[at] = *stack;
                ids[at] = self.id(Place::Container(slot));
            }
        }

        let mut out = Out::packet(super::session::INVENTORY_CONTENT);
        out.varint(u32::from(window)).varint(order.len() as u32);

        for (stack, id) in order.into_iter().zip(ids) {
            write_item(&mut out, stack, id);
        }

        out.u8(super::containers::WINDOW_CONTAINER).u8(0).zigzag32(0);
        Some(out.bytes)
    }

    /// Всё, что BDS шлёт игроку при входе и сразу после Container Open: рюкзак,
    /// броня, окно интерфейса (54 ячейки, включая курсор), вторая рука и Player
    /// Hotbar с выбранной ячейкой.
    pub fn open_sync_packets(&self) -> Vec<Vec<u8>> {
        let mut packets = self.content_packets();
        let mut ui = Out::packet(super::session::INVENTORY_CONTENT);

        ui.varint(WINDOW_UI).varint(UI_SLOTS as u32);

        for slot in 0..UI_SLOTS {
            if slot == 0 { write_item(&mut ui, self.cursor, self.id(Place::Cursor)); }
            else { ui.zigzag32(0); }
        }

        ui.u8(super::containers::WINDOW_CONTAINER).u8(0).zigzag32(0);
        // Порядок BDS: рюкзак, броня, интерфейс, вторая рука.
        packets.insert(2, ui.bytes);

        let mut hotbar = Out::packet(PLAYER_HOTBAR);
        hotbar.varint(self.inventory.selected() as u32).u8(WINDOW_INVENTORY as u8).bool(true);
        packets.push(hotbar.bytes);
        packets
    }

    pub fn cartography_requests(&mut self, payload: &[u8], creative: bool) -> (Handled, Vec<Vec<u8>>) {
        let handled = self.handle_requests(payload, creative);
        (handled, self.crafting_sync_packets())
    }

    /// Динамический итог несёт довески, которых нет в ItemStackResponse.
    /// Окно UI: курсор 0, сетки 28–40 (minecraft-data 1.26.10).
    pub fn crafting_sync_packets(&self) -> Vec<Vec<u8>> {
        let mut packets = self.content_packets();
        let mut ui = Out::packet(super::session::INVENTORY_CONTENT);
        ui.varint(WINDOW_UI).varint(UI_SLOTS as u32);
        for slot in 0..UI_SLOTS as u8 {
            let place = if slot == 0 { Place::Cursor } else { Place::Ui(slot) };
            write_item(&mut ui, self.get(place), self.id(place));
        }
        ui.u8(super::containers::WINDOW_CONTAINER).u8(0).zigzag32(0);
        packets.push(ui.bytes); packets
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
        // `padding` — пустые ячейки в конце: окно брони у BDS из пяти ячеек
        // (четыре предмета и ещё одна, пока пустая), у нас предметов четыре.
        let window = |window: u32, container: u8, slots: &[i32], padding: usize| {
            let mut out = Out::packet(super::session::INVENTORY_CONTENT);
            out.varint(window).varint((slots.len() + padding) as u32);

            for &slot in slots {
                write_item(&mut out, self.inventory.slot(slot), self.id(Place::Slot(slot)));
            }

            for _ in 0..padding {
                out.zigzag32(0);
            }

            out.u8(container).u8(0); // FullContainerName без динамического номера
            out.zigzag32(0); // storage_item — пусто
            out.bytes
        };

        let main: Vec<i32> = (FIRST_HOTBAR..=LAST_HOTBAR).chain(FIRST_MAIN..=LAST_MAIN).collect();
        let armor: Vec<i32> = (FIRST_ARMOR..FIRST_ARMOR + 4).collect();

        vec![
            window(WINDOW_INVENTORY, HOTBAR_AND_INVENTORY, &main, 0),
            window(WINDOW_ARMOR, ARMOR, &armor, 1),
            window(WINDOW_OFFHAND, OFFHAND_SLOT, &[OFFHAND], 0),
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

    /// Вложенный запрос извлекается без исполнения: world.rs применит его
    /// под тем же замком общего хранилища, что отдельный ItemStackRequest.
    pub fn read_embedded(reader: &mut In) -> Option<Vec<u8>> {
        let mut copy = reader.clone();
        let before = reader.remaining();
        reader.zigzag32()?;
        Self::parse_actions(reader)?;
        for _ in 0..reader.varint()? { reader.string()?; }
        reader.li32()?;
        let mut payload = vec![1];
        payload.extend_from_slice(copy.take(before - reader.remaining())?);
        Some(payload)
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
            // Ответ BDS на запрос кузнечного стола не называет выходную
            // ячейку: только вход, добавку, шаблон и курсор (снято по сети).
            if self.smithing.is_some() {
                touched.retain(|(_, place)| *place != Place::CreativeOutput);
            }

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
        self.smithing = None;
        self.station_spend = None;
        self.station_consumed.clear();
        self.planned_crafting.clear();
        self.recipe = None;
        self.trade_pick = None;
        self.trade_times = 1;

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

        if named.container == SHULKER && self.restrict != Some(crate::containers::Kind::ShulkerBox) { return None; }
        if matches!(named.container, LEVEL_ENTITY | BARREL | CRAFTER | SHULKER) {
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

            if kind == crate::containers::Kind::ShulkerBox && crate::containers::is_shulker_item(stack.item) { return false; }
            if kind == crate::containers::Kind::Crafter && self.disabled_slots & (1 << slot) != 0 { return false; }
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
                10 => Act::Beacon(reader.zigzag32()?, reader.zigzag32()?),
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
                    let times = i32::from(reader.u8()?);
                    Act::Recipe { id, times }
                }
                // Рецепт из книги рецептов: ещё список ингредиентов.
                13 => {
                    let id = reader.varint()?;
                    let _times = reader.u8()?;
                    let times = i32::from(reader.u8()?);

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
                    let pattern=reader.string()?;
                    reader.u8()?;
                    Act::Loom(pattern)
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
        self.planned_crafting = actions.iter().filter_map(|action| {
            if let Act::Consume { count, source, destroy: false } = action {
                let place = self.place_for(*source, creative)?;
                let stack = self.get(place)?;
                if *count > 0 && *count <= stack.count { Some((place, stack.with_count(*count))) } else { None }
            } else { None }
        }).collect();
        let mut beacon = None;
        let mut beacon_paid = 0;

        for act in actions {
            let ok = match act {
                Act::Move { count, source, destination } => self.move_stack(count, source, destination, creative, touched),
                Act::Swap(source, destination) => self.swap(source, destination, creative, touched),
                Act::Drop { count, source } => self.remove(count, source, creative, touched).map(|stack| dropped.push(stack)).is_some(),
                Act::Consume { count, source, destroy } => {
                    if destroy {
                        creative && self.remove(count, source, creative, touched).is_some()
                    } else {
                        if self.place_for(source, creative) == Some(Place::Ui(stations::UI_BEACON_PAYMENT)) {
                            if !self.get(Place::Ui(stations::UI_BEACON_PAYMENT)).is_some_and(|s| crate::beacon::payment(s.item)) { failed = true; }
                            beacon_paid += count;
                        }
                        self.consume(count, source, creative, touched, dropped)
                    }
                }
                Act::Beacon(primary, secondary) => {
                    if beacon.is_some() { false } else { beacon = Some((primary, secondary)); true }
                }
                Act::Skip => true,
                Act::CraftCreative(entry) => creative && self.craft_creative(entry),
                Act::Recipe { id, times } => if self.station.is_some_and(|s|s.kind==stations::Kind::Cartography) {
                    self.cartography(id,names.first().map(String::as_str))
                } else {self.craft_recipe(id, times, creative)},
                Act::Anvil => if self.station.is_some_and(|s|s.kind==stations::Kind::Cartography) {
                    self.cartography(stations::cartography_base()+3,names.first().map(String::as_str))
                } else {self.anvil(names.first().map(String::as_str), creative)},
                Act::Grind => self.grind(),
                Act::Loom(pattern) => self.loom(&pattern),
            };

            failed |= !ok;
        }

        if let Some((primary, secondary)) = beacon {
            // Эффекты Bedrock с единицы, Java с нуля. Уровень передаётся только
            // на время запроса под замком мира; платёж локален игроку.
            let station = self.station.filter(|s| s.kind == stations::Kind::Beacon);
            let powers = primary.checked_sub(1).zip(secondary.checked_sub(1));
            if let (Some(station), Some((primary, secondary))) = (station, powers) {
                failed |= !crate::beacon::valid_selection(station.shelves, primary, secondary);
                if beacon_paid == 0 {
                    let source = Named { container: stations::BEACON_PAYMENT, dynamic: None, slot: stations::UI_BEACON_PAYMENT };
                    failed |= !self.get(Place::Ui(stations::UI_BEACON_PAYMENT)).is_some_and(|s| crate::beacon::payment(s.item));
                    failed |= !self.consume(1, source, creative, touched, dropped);
                } else { failed |= beacon_paid != 1; }
                self.events.push(Event::BeaconSelected(station.pos, primary, secondary));
            } else { failed = true; }
        } else if beacon_paid != 0 { failed = true; }
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
    fn consume(&mut self, count: i32, source: Named, creative: bool, touched: &mut Vec<(Named, Place)>, dropped: &mut Vec<Stack>) -> bool {
        let Some(from) = self.place_for(source, creative) else {
            return false;
        };
        if self.bound(from, creative) { return false; }
        let Some(stack) = self.get(from) else {
            return false;
        };

        if count <= 0 || count > stack.count {
            return false;
        }

        let left = stack.count - count;
        let in_ui = matches!(from, Place::Ui(_));
        let mut rest = (left > 0).then(|| stack.with_count(left));

        if left == 0 && self.recipe.is_some() && let Some(item) = crate::crafting::remainder(stack.item) {
            rest = Some(Stack::new(item, count));
        }

        if self.station_spend.is_some() && self.recipe.is_some() {
            let n = blocks::item_name(stack.item).unwrap_or("");
            if n == "written_book" || n.ends_with("_banner") && (!stack.extras().banner_patterns.is_empty() || stack.extras().ominous_banner) && self.station_spend.as_ref().is_some_and(|v| v.iter().all(|(_,s)| blocks::item_name(s.item).is_some_and(|n| n.ends_with("_banner")))) { rest = Some(stack); }
        }
        self.set(from, rest);
        touched.push((source, from));
        if left > 0 && self.recipe.is_some() && let Some(item) = crate::crafting::remainder(stack.item) {
            let (changed, overflow) = self.inventory.put(Stack::new(item, count));
            self.touched(&changed); dropped.extend(overflow);
            for slot in changed {
                let wire = if slot >= FIRST_HOTBAR { slot - FIRST_HOTBAR } else { slot };
                touched.push((Named { container: HOTBAR_AND_INVENTORY, dynamic: None, slot: wire as u8 }, Place::Slot(slot)));
            }
        }
        self.consumed.push((stack.item, count, in_ui));
        self.station_consumed.push((from,stack.with_count(count)));
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

                if !creative && !stations::dynamic_recipe(id) && self.allowed_crafting.as_ref().is_some_and(|known| crate::crafting::recipes().iter().position(|r| std::ptr::eq(r, recipe)).is_none_or(|i| !known.contains(&i))) { return false; }
                if self.craft_ready || times <= 0 || times > 64 || recipe.count.checked_mul(times).is_none_or(|n| n > blocks::stack_size(recipe.result)) { return false; }
                let size = if self.station.is_some() { 3 } else { 2 };
                if recipe.shaped && (recipe.width > size || recipe.height > size) || recipe.cells.iter().filter(|c| !c.is_empty()).count() > usize::from(size * size) { return false; }
                let output_name = blocks::item_name(recipe.result).unwrap_or("");
                if stations::dynamic_recipe(id) || output_name.ends_with("shulker_box") || crate::bundle::is_bundle(recipe.result) {
                    let start = if size == 3 { stations::UI_GRID_3X3 } else { stations::UI_GRID_2X2 };
                    let mut grid: Vec<_> = (start..start + size * size).map(|i| self.get(Place::Ui(i))).collect();
                    let mut wanted: Vec<_> = grid.iter().enumerate().filter_map(|(i,s)| s.map(|s| (Place::Ui(start + i as u8), s.with_count(times)))).collect();
                    if grid.iter().all(Option::is_none) {
                        wanted = self.planned_crafting.clone();
                        if wanted.iter().any(|(_,s)| s.count % times != 0) { return false; }
                        let mut units: Vec<_> = wanted.iter().flat_map(|(_,s)| std::iter::repeat_n(s.with_count(1), (s.count / times) as usize)).collect();
                        if units.len() != recipe.cells.iter().filter(|c| !c.is_empty()).count() { return false; }
                        for (i,cell) in recipe.cells.iter().enumerate() {
                            if cell.is_empty() { continue; }
                            let Some(at) = units.iter().position(|s| cell.contains(&s.item)) else { return false; };
                            let target = if recipe.shaped { i / recipe.width as usize * usize::from(size) + i % recipe.width as usize } else { i };
                            grid[target] = Some(units.remove(at));
                        }
                    } else if grid.iter().flatten().any(|s| s.count < times) { return false; }
                    let Some(result) = crate::crafting::craft(&crate::crafting::player::padded(&grid)).filter(|s| s.item == recipe.result && s.count == recipe.count && s.extras().map_post.is_none()) else { return false; };
                    if !recipe.covered_by(&grid.iter().flatten().map(|s| s.item).collect::<Vec<_>>()) { return false; }
                    self.station_spend = Some(wanted);
                    self.craft_ready = true;
                    self.recipe = Some((recipe, times));
                    self.set(Place::CreativeOutput, Some(result.with_count(result.count * times)));
                    return true;
                }
                self.recipe = Some((recipe, times));
                self.craft_ready = true;
                self.set(Place::CreativeOutput, Some(Stack::new(recipe.result, (recipe.count * times).min(blocks::stack_size(recipe.result)))));
                true
            }
            Some(stations::Kind::Smithing) => self.smith(id),
            Some(stations::Kind::Stonecutter) => self.stonecut(id,times),
            Some(stations::Kind::Trading) => self.trade_recipe(id, times),
            Some(_) => false,
        }
    }

    /// Сделка с жителем: «рецепт» — номер сделки из Update Trade (`netId`).
    /// Итог рождается в выходе, оплата в ячейках должна покрывать цену —
    /// расходует её сам запрос (consume), сверка — в `finish_station`.
    /// Пачка по Shift — тот же запрос с `times` > 1 (BDS принимает, запись
    /// tmp/br-polish/bds-tr2.jsonl): итог и оплата умножаются, а каждая
    /// сделка потом считается отдельно (`Event::Traded` на каждую). Больше,
    /// чем хватает запаса или оплаты, клиент не просит; если просит — отказ.
    fn trade_recipe(&mut self, id: u32, times: i32) -> bool {
        let Some(index) = self.trade.iter().position(|price| price.net_id == id) else {
            return false;
        };
        let price = &self.trade[index];

        if times < 1 || !price.stocked {
            return false;
        }

        let (first, second) = (self.get(Place::Ui(stations::UI_TRADE_FIRST)), self.get(Place::Ui(stations::UI_TRADE_SECOND)));

        if times > price.left || !price.paid_times(first, second, times) {
            return false;
        }

        // Итог пачки — одна стопка: больше предел стопки не вместит.
        let sell = price.sell;
        let total = sell.count.saturating_mul(times);

        if total > blocks::stack_size(sell.item) {
            return false;
        }

        self.trade_pick = Some(index);
        self.trade_times = times;
        self.craft_ready = true;
        self.set(Place::CreativeOutput, Some(sell.with_count(total)));

        for _ in 0..times {
            self.events.push(Event::Traded(index));
        }

        true
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

    /// Итог кузнечного стола по его трём ячейкам (улучшение или отделка).
    /// Клиент называет рецепт, а считает сервер: подойти должен набор
    /// ячеек, а не номер (снято с BDS: craft_recipe, results_deprecated,
    /// три consume, take из выходной ячейки; звука от сервера нет).
    fn smith(&mut self, id: u32) -> bool {
        let Some(station) = self.station.filter(|station| station.kind == stations::Kind::Smithing) else {
            return false;
        };

        if !stations::is_smithing_recipe(id) {
            return false;
        }

        let inputs = [
            self.get(Place::Ui(stations::UI_SMITHING_TEMPLATE)),
            self.get(Place::Ui(stations::UI_SMITHING_INPUT)),
            self.get(Place::Ui(stations::UI_SMITHING_MATERIAL)),
        ];
        let Some(result) = crate::smithing::result(inputs[0], inputs[1], inputs[2]) else {
            return false;
        };

        self.craft_ready = true;
        self.smithing = Some(inputs.map(|stack| stack.map_or(0, |stack| stack.item)));
        self.set(Place::CreativeOutput, Some(result));
        self.events.push(Event::Smithed(station.pos));
        true
    }

    fn cartography(&mut self,id:u32,name:Option<&str>)->bool {
        if self.craft_ready || !(stations::cartography_base()..stations::cartography_base()+4).contains(&id) {return false;}
        let Some(input)=self.get(Place::Ui(12)) else {return false;};
        let material=self.get(Place::Ui(13));
        let Some(maps)=self.cartography_maps.as_mut() else {return false;};
        let result=if let Some(material)=material {maps.cartography(input,material,true)} else {
            (input.extras().map_id.is_some() && name.is_some()).then_some(input.with_count(1))
        };
        let Some(mut result)=result else {return false;};
        if let Some(name)=name.filter(|s|s.chars().count()<=50) {let mut data=result.extras();data.name=(!name.is_empty()).then(||name.to_owned());result=result.with_extras(data);}
        self.craft_ready=true; let mut wanted=vec![(Place::Ui(12),input.with_count(1))];if let Some(m)=material {wanted.push((Place::Ui(13),m.with_count(1)));}self.station_spend=Some(wanted);
        self.set(Place::CreativeOutput,Some(result));true
    }
    fn stonecut(&mut self, id: u32, times: i32) -> bool {
        let Some(r)=stations::stonecut_recipe(id) else { return false; };
        let Some(input)=self.get(Place::Ui(3)) else { return false; };
        if self.craft_ready || input.item!=r.input || times<=0 || times>input.count || times*r.count>blocks::stack_size(r.output) { return false; }
        self.craft_ready=true; self.station_spend=Some(vec![(Place::Ui(3),input.with_count(times))]);
        self.set(Place::CreativeOutput,Some(Stack::new(r.output,r.count*times))); true
    }
    fn loom(&mut self, name: &str) -> bool {
        if self.craft_ready || !self.station.is_some_and(|s|s.kind==stations::Kind::Loom) { return false; }
        let inputs=[self.get(Place::Ui(9)),self.get(Place::Ui(10)),self.get(Place::Ui(11))];
        let pattern=crate::loom::BEDROCK.iter().position(|p|*p==name).or_else(||crate::loom::PATTERNS.iter().position(|p|*p==name));
        let Some(pattern)=pattern else { return false; };
        let choices=crate::loom::choices(inputs[2]);
        let Some(selected)=choices.iter().position(|p|usize::from(*p)==pattern) else { return false; };
        let Some(result)=crate::loom::result(inputs[0],inputs[1],inputs[2],selected as i32) else { return false; };
        self.craft_ready=true;
        self.station_spend=Some(inputs[..2].iter().enumerate().filter_map(|(i,s)|s.map(|s|(Place::Ui(9+i as u8),s.with_count(1)))).collect());
        self.set(Place::CreativeOutput,Some(result)); true
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
            if times <= 0 || self.get(Place::CreativeOutput).is_some() { return false; }
            let consumed: Vec<_> = self.consumed.iter().map(|(item,count,_)| (*item,*count)).collect();
            if !recipe.covered_counts(&consumed, times) { return false; }
            self.events.push(Event::Crafted(recipe.result, recipe.count * times));

        }

        // Сделка: израсходовано ровно столько, сколько стоит, и итог взят.
        if let Some(index) = self.trade_pick {
            let Some(price) = self.trade.get(index) else {
                return false;
            };

            if self.get(Place::CreativeOutput).is_some() || !price.covers_times(&self.consumed, self.trade_times) {
                return false;
            }
        }

        if let Some(station) = self.station {
            let lapis = blocks::item_named("lapis_lazuli");

            match station.kind {
                stations::Kind::Beacon => {
                    if self.get(Place::Ui(stations::UI_BEACON_PAYMENT)).is_some_and(|s| !crate::beacon::payment(s.item)) { return false; }
                }
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
                stations::Kind::Smithing => {
                    let cells = [stations::UI_SMITHING_TEMPLATE, stations::UI_SMITHING_INPUT, stations::UI_SMITHING_MATERIAL];

                    if cells.iter().enumerate().any(|(at, cell)| self.get(Place::Ui(*cell)).is_some_and(|stack| !crate::smithing::may_place(at, &stack))) {
                        return false;
                    }
                }
                _ => {}
            }
        }

        if let Some(wanted)=&self.station_spend {
            let mut wanted= wanted.clone(); wanted.sort_unstable_by_key(|(p,_)|*p);
            let mut spent=self.station_consumed.clone(); spent.sort_unstable_by_key(|(p,_)|*p);
            if spent!=wanted || self.get(Place::CreativeOutput).is_some() { return false; }
            if let Some(station)=self.station.filter(|s| s.kind != stations::Kind::Workbench) {self.events.push(Event::StationWorked(station.kind,station.pos));}
        }
        // Стол расходует ровно по одному из трёх входов.
        if let Some(mut wanted) = self.smithing {
            let mut spent: Vec<i32> = self.consumed.iter().filter(|(_, _, in_ui)| *in_ui).flat_map(|&(item, count, _)| std::iter::repeat_n(item, count as usize)).collect();

            wanted.sort_unstable();
            spent.sort_unstable();

            if spent != wanted {
                return false;
            }
        }

        let _ = creative;
        true
    }

    /// Проклятие несъёмности удерживает надетую вещь, но не мешает
    /// носить её в рюкзаке и не ограничивает творческий режим.
    fn bound(&self, place: Place, creative: bool) -> bool {
        !creative && matches!(place, Place::Slot(5..=8))
            && self.get(place).is_some_and(|stack| stack.extras().level("binding_curse") > 0)
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

        if self.bound(from, creative) { return false; }

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

        if from != to && (self.bound(from, creative) || self.bound(to, creative)) { return false; }

        self.set(from, b);
        self.set(to, a);
        touched.push((source, from));
        touched.push((destination, to));
        true
    }

    fn remove(&mut self, count: i32, source: Named, creative: bool, touched: &mut Vec<(Named, Place)>) -> Option<Stack> {
        let from = self.place_for(source, creative)?;
        let stack = self.get(from)?;

        if self.bound(from, creative) { return None; }

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
            // Ответ BDS на сделку не называет выход (creative_output) —
            // клиент знает его сам (запись tmp/br-trade).
            if self.trade_pick.is_some() && *place == Place::CreativeOutput {
                continue;
            }

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
    Beacon(i32, i32),
    Move { count: i32, source: Named, destination: Named },
    Swap(Named, Named),
    Drop { count: i32, source: Named },
    Consume { count: i32, source: Named, destroy: bool },
    Skip,
    CraftCreative(u32),
    Recipe { id: u32, times: i32 },
    Anvil,
    Grind,
    Loom(String),
}

impl Act {
    /// Короткое имя действия для журнала.
    fn label(&self) -> &'static str {
        match self {
            Act::Beacon(..) => "beacon",
            Act::Move { .. } => "move",
            Act::Swap(..) => "swap",
            Act::Drop { .. } => "drop",
            Act::Consume { .. } => "consume",
            Act::Skip => "skip",
            Act::CraftCreative(_) => "craft_creative",
            Act::Recipe { .. } => "craft_recipe",
            Act::Anvil => "anvil",
            Act::Grind => "grind",
            Act::Loom(_) => "loom",
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

    #[test]
    fn component_items_decode_without_leaking_into_the_next_item() {
        use crate::bedrock::nbt::{decode,Style,Tag};
        for (name,extras,aux) in [
            ("shield",crate::stackdata::Data { damage:7,name:Some("guard".into()),..Default::default() },0),
            ("tipped_arrow",crate::stackdata::Data { potion:crate::survival::potion_number("poison"),..Default::default() },26),
            ("filled_map",crate::stackdata::Data { map_id:Some(31),..Default::default() },0),
            ("goat_horn",crate::stackdata::Data { instrument:Some(7),..Default::default() },7),
        ] {
            let stack = Stack::new(blocks::item_named(name).unwrap(),1).with_extras(extras);
            let mut out = Out::default(); write_item(&mut out,Some(stack),27); write_item(&mut out,None,0);
            let mut r = In::new(&out.bytes); let id = r.zigzag32().unwrap();
            assert_eq!(r.take(2),Some([1,0].as_slice())); assert_eq!(r.varint(),Some(aux));
            assert_eq!(r.bool(),Some(true)); assert_eq!(r.zigzag32(),Some(27)); r.zigzag32().unwrap();
            let len = r.varint().unwrap() as usize; let extra = r.take(len).unwrap();
            assert_eq!(r.zigzag32(),Some(0)); assert_eq!(r.remaining(),0);
            let suffix = if id == SHIELD { 16 } else { 8 };
            assert!(extra[extra.len()-suffix..].iter().all(|b| *b == 0));
            if extra[..2] == [255,255] {
                assert_eq!(extra[2],1);
                let tag = decode(&extra[3..extra.len()-suffix],Style::Little).unwrap();
                if name == "shield" { assert_eq!(tag.get("Damage"),Some(&Tag::Int(7))); }
                if name == "filled_map" { assert_eq!(tag.get("map_uuid"),Some(&Tag::Long(31))); }
            } else { assert_eq!(extra.len(),10); }
        }
    }

    #[test]
    fn shulker_nesting_and_disabled_crafter_slots_roll_back() {
        for (kind,name,disabled) in [(crate::containers::Kind::ShulkerBox,"red_shulker_box",0),
            (crate::containers::Kind::Crafter,"stone",1)] {
            let mut bag = Bag::new(Inventory::new());
            let stack = Stack::new(blocks::item_named(name).unwrap(),1); bag.inventory.set(FIRST_HOTBAR,Some(stack));
            bag.open_window_for(&vec![None;kind.size()],Vec::new(),Some(kind)); bag.disabled_slots = disabled;
            let container = super::super::containers::slot_name(kind);
            let payload = request(4,1,|out| { out.u8(1).u8(1); slot(out,HOTBAR_AND_INVENTORY,0); slot(out,container,0); });
            let handled = bag.handle_requests(&payload,false); assert_eq!(handled.response[3],1);
            assert_eq!(bag.inventory.held(),Some(stack)); assert!(bag.window_slots().unwrap().iter().all(Option::is_none));
            bag.disabled_slots=0; bag.inventory.set(FIRST_HOTBAR,Some(item("stone",1)));
            assert_eq!(bag.handle_requests(&payload,false).response[3],0);
            assert_eq!(bag.window_slots().unwrap()[0],Some(item("stone",1)));
            let packet=bag.window_slot_packet(2,container,0).unwrap(); let mut r=In::new(&packet);
            assert_eq!(r.varint(),Some(super::super::session::INVENTORY_SLOT));
            assert_eq!(r.varint(),Some(2)); assert_eq!(r.varint(),Some(0)); assert_eq!(r.u8(),Some(container));
        }
    }

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
    fn named_shield_keeps_blocking_tick_after_nbt() {
        let stack = Stack::new(crate::blocks::item_named("shield").unwrap(), 1)
            .with_extras(crate::stackdata::Data { name: Some("Guard".into()), ..Default::default() });
        let mut out = Out::default(); write_item(&mut out, Some(stack), 0);
        let mut r = In::new(&out.bytes);
        assert_eq!(r.zigzag32(), Some(SHIELD)); assert!(r.take(2).is_some()); assert!(r.varint().is_some());
        assert_eq!(r.bool(), Some(false)); assert!(r.zigzag32().is_some());
        let len = r.varint().unwrap() as usize; let extra = r.take(len).unwrap();
        assert_eq!(&extra[..3], &[255,255,1]);
        assert_eq!(&extra[extra.len()-16..], &[0;16]); // два списка + blocking_tick
        assert_eq!(r.remaining(), 0);
    }

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

    #[test]
    fn binding_blocks_bedrock_take_swap_and_drop_but_allows_creative() {
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("binding_curse").unwrap(), 1);
        let cursed = item("diamond_helmet", 1).with_extras(data);
        for action in [0, 2, 3] {
            for creative in [false, true] {
                let mut bag = Bag::new(Inventory::new());
                bag.inventory.set(5, Some(cursed));
                let payload = request(1, 1, |out| {
                    out.u8(action);
                    if action != 2 { out.u8(1); }
                    slot(out, ARMOR, 0);
                    if action == 3 { out.bool(false); } else { slot(out, HOTBAR_AND_INVENTORY, 0); }
                });
                let handled = bag.handle_requests(&payload, creative);
                assert_eq!(handled.response[3], if creative { 0 } else { 1 }, "action={action}, creative={creative}");
                assert_eq!(bag.inventory.slot(5), if creative { None } else { Some(cursed) });
            }
        }
        // Нельзя вытеснить проклятую броню обменом из рюкзака.
        let mut bag = Bag::new(Inventory::new());
        bag.inventory.set(5, Some(cursed));
        bag.inventory.set(FIRST_HOTBAR, Some(item("iron_helmet", 1)));
        let swap = request(1, 1, |out| { out.u8(2); slot(out, HOTBAR_AND_INVENTORY, 0); slot(out, ARMOR, 0); });
        assert_eq!(bag.handle_requests(&swap, false).response[3], 1);
        assert_eq!(bag.inventory.slot(5), Some(cursed));
        // Само проклятие не запрещает перемещать ненадетую вещь.
        bag.inventory.set(FIRST_HOTBAR, Some(cursed));
        let take = request(1, 1, |out| { out.u8(0).u8(1); slot(out, HOTBAR_AND_INVENTORY, 0); slot(out, CURSOR, 0); });
        assert_eq!(bag.handle_requests(&take, false).response[3], 0);
        assert_eq!(bag.get(Place::Cursor), Some(cursed));
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

    /// Окно стойки одним пакетом, ячейки — в порядке Bedrock (ингредиент 0,
    /// бутылки 1–3, порошок 4), а не Java; имя контейнера нулевое, как у BDS.
    #[test]
    fn brewing_window_is_one_packet_in_bedrock_order() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut bag = Bag::new(Inventory::new());
        let mut slots = vec![None; 5];

        // Java: 0–2 — бутылки, 3 — ингредиент, 4 — порошок.
        slots[3] = Some(Stack::new(stone, 1));
        bag.open_window_for(&slots, stations::layout(crate::containers::Kind::BrewingStand).expect("стойка"), None);

        let packet = bag.window_full_packet(9).expect("окно открыто");
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(super::super::session::INVENTORY_CONTENT));
        assert_eq!((reader.varint(), reader.varint()), (Some(9), Some(5)));
        // Камень — первым (ингредиент), остальные пусты.
        assert_eq!(reader.zigzag32(), Some(1));
        reader.take(2).expect("количество");
        reader.varint().expect("метаданные");
        reader.u8().expect("есть ли номер");
        reader.zigzag32().expect("номер стопки");
        reader.zigzag32().expect("блок");
        let extra = reader.varint().expect("длина") as usize;
        reader.take(extra).expect("данные");

        for _ in 1..5 {
            assert_eq!(reader.zigzag32(), Some(0));
        }

        assert_eq!((reader.u8(), reader.u8(), reader.zigzag32()), (Some(0), Some(0), Some(0)));
        assert_eq!(reader.u8(), None);
    }

    /// После Container Open BDS досылает рюкзак, броню, интерфейс (54 пустые
    /// ячейки), вторую руку и Player Hotbar.
    #[test]
    fn open_sync_follows_the_official_order() {
        let mut bag = Bag::new(Inventory::new());

        bag.select(4);

        let packets = bag.open_sync_packets();
        let windows: Vec<(u32, u32)> = packets[..4]
            .iter()
            .map(|packet| {
                let mut reader = In::new(packet);

                assert_eq!(reader.varint(), Some(super::super::session::INVENTORY_CONTENT));
                (reader.varint().expect("окно"), reader.varint().expect("ячеек"))
            })
            .collect();

        assert_eq!(windows, [(0, 36), (120, 5), (124, 54), (119, 1)]);
        assert_eq!(packets.len(), 5);
        // 30 04 00 01: ячейка 4, окно рюкзака, «выбрать».
        assert_eq!(packets[4], vec![0x30, 4, 0, 1]);
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

        let packet = bag.window_full_packet(4).expect("окно открыто");
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
        assert_eq!((reader.u8(), reader.u8(), reader.zigzag32()), (Some(super::super::containers::WINDOW_CONTAINER), Some(0), Some(0)));
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

    #[test]
    fn stonecutter_requires_payment_from_its_input_and_taking_the_result() {
        let make=|| {
            let mut bag=Bag::new(Inventory::new());
            bag.open_station(Station{kind:stations::Kind::Stonecutter,pos:(0,0,0),seed:0,shelves:0});
            bag.set(Place::Ui(3),Some(item("stone",2)));
            bag.inventory.set(FIRST_HOTBAR,Some(item("stone",2)));
            bag
        };
        let index=crate::stonecutter::recipes().iter().position(|r|r.input==item("stone",1).item&&r.output==item("stone_slab",1).item).unwrap();
        let id=stations::stonecut_base()+index as u32;
        for payment in [53,HOTBAR_AND_INVENTORY] {
            let mut bag=make();
            let payload=request(-19,3,|out| {
                out.u8(12).varint(id).u8(1);
                consume_action(out,1,payment,if payment==53 {3} else {0});
                take_output(out,2);
            });
            bag.handle_requests(&payload,false);
            if payment==53 {
                assert_eq!(bag.get(Place::Cursor),Some(item("stone_slab",2)));
                assert_eq!(bag.get(Place::Ui(3)),Some(item("stone",1)));
            } else {
                assert_eq!(bag.get(Place::Cursor),None);
                assert_eq!(bag.get(Place::Ui(3)),Some(item("stone",2)));
            }
            assert_eq!(bag.inventory.slot(FIRST_HOTBAR),Some(item("stone",2)));
        }
        let mut bag=make();
        bag.handle_requests(&request(-20,1,|out|{out.u8(12).varint(id).u8(1);}),false);
        assert_eq!(bag.get(Place::Ui(3)),Some(item("stone",2)));
        assert!(bag.get(Place::CreativeOutput).is_none());
    }

    #[test]
    fn failed_cartography_request_does_not_allocate_a_map() {
        let mut bag=Bag::new(Inventory::new());
        bag.open_station(Station{kind:stations::Kind::Cartography,pos:(0,0,0),seed:0,shelves:0});
        let mut maps=crate::maps::Maps::default();
        let id=maps.create(crate::dimension::Dimension::Overworld,0,0,0);
        bag.cartography_maps=Some(maps);
        let input=item("filled_map",1).with_extras(crate::stackdata::Data{map_id:Some(id),..Default::default()});
        bag.set(Place::Ui(12),Some(input));bag.set(Place::Ui(13),Some(item("paper",1)));
        bag.handle_requests(&request(-21,1,|out|{out.u8(12).varint(stations::cartography_base()).u8(1);}),false);
        assert_eq!(bag.cartography_maps.as_ref().unwrap().maps.len(),1);
        assert_eq!(bag.get(Place::Ui(12)),Some(input));
        let payload=request(-22,4,|out|{
            out.u8(12).varint(stations::cartography_base()).u8(1);
            consume_action(out,1,55,12);consume_action(out,1,56,13);take_output(out,1);
        });
        let (_,packets) = bag.cartography_requests(&payload,false);
        assert!(packets.iter().any(|p| p.windows(8).any(|s| s == b"map_uuid")));
        assert_eq!(bag.cartography_maps.as_ref().unwrap().maps.len(),2);
        assert_eq!(bag.get(Place::Cursor).unwrap().extras().map_id,Some(id+1));
        assert!(bag.get(Place::Ui(12)).is_none());assert!(bag.get(Place::Ui(13)).is_none());
    }

    #[test]
    fn beacon_payment_is_transactional_and_charged_once() {
        let mut bag = Bag::new(Inventory::new());
        bag.open_station(Station { kind: stations::Kind::Beacon, pos:(1,70,2), seed:0, shelves:4 });
        let pay = Place::Ui(stations::UI_BEACON_PAYMENT);
        let select = |primary,secondary,consume| request(-31,if consume {2} else {1},|out| {
            out.u8(10).zigzag32(primary).zigzag32(secondary);
            if consume { consume_action(out,1,stations::BEACON_PAYMENT,stations::UI_BEACON_PAYMENT); }
        });
        for consume in [false,true] {
            bag.set(pay,Some(item("emerald",2)));
            let handled = bag.handle_requests(&select(1,10,consume),false);
            assert_eq!(handled.events,[Event::BeaconSelected((1,70,2),0,9)]);
            assert_eq!(bag.get(pay),Some(item("emerald",1)));
            let mut r = In::new(&handled.response);
            assert_eq!(r.varint(),Some(super::super::session::ITEM_STACK_RESPONSE));
            assert_eq!(r.varint(),Some(1)); assert_eq!(r.u8(),Some(0));
        }
        for (primary,secondary,level,payment) in [(1,10,1,"emerald"),(99,0,4,"emerald"),(1,0,4,"dirt"),(i32::MIN,0,4,"emerald")] {
            bag.station.as_mut().unwrap().shelves=level; bag.set(pay,Some(item(payment,1)));
            let handled=bag.handle_requests(&select(primary,secondary,true),false);
            assert!(handled.events.is_empty()); assert_eq!(bag.get(pay),Some(item(payment,1)));
        }
        bag.station.as_mut().unwrap().shelves=4; bag.set(pay,None);
        assert!(bag.handle_requests(&select(1,0,false),true).events.is_empty());
        assert!(bag.get(pay).is_none());
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

    /// Кузнечный стол (BDS: craft_recipe, три consume, take из выходной
    /// ячейки): улучшение и отделка; расходуется по одному из трёх входов.
    #[test]
    fn the_smithing_table_upgrades_and_trims() {
        let id = stations::smithing_base();
        let fill = |bag: &mut Bag, template: &str, base: Stack, addition: &str| {
            bag.open_station(Station { kind: stations::Kind::Smithing, pos: (1, 2, 3), seed: 0, shelves: 0 });
            bag.set(Place::Ui(stations::UI_SMITHING_TEMPLATE), Some(item(template, 2)));
            bag.set(Place::Ui(stations::UI_SMITHING_INPUT), Some(base));
            bag.set(Place::Ui(stations::UI_SMITHING_MATERIAL), Some(item(addition, 1)));
        };
        let craft = |all: bool| {
            request(-21, 5, move |out| {
                out.u8(12).varint(id).u8(1);
                consume_action(out, 1, stations::SMITHING_TEMPLATE, stations::UI_SMITHING_TEMPLATE);
                consume_action(out, 1, stations::SMITHING_INPUT, stations::UI_SMITHING_INPUT);

                if all {
                    consume_action(out, 1, stations::SMITHING_MATERIAL, stations::UI_SMITHING_MATERIAL);
                } else {
                    consume_action(out, 1, stations::SMITHING_INPUT, stations::UI_SMITHING_INPUT);
                }

                take_output(out, 1);
            })
        };
        let mut bag = Bag::new(Inventory::new());

        fill(&mut bag, "netherite_upgrade_smithing_template", item("diamond_sword", 1), "netherite_ingot");

        let handled = bag.handle_requests(&craft(true), false);

        assert_eq!(bag.get(Place::Cursor).map(|stack| stack.item), crate::blocks::item_named("netherite_sword"));
        assert_eq!(bag.get(Place::Ui(stations::UI_SMITHING_TEMPLATE)).map(|stack| stack.count), Some(1));
        assert_eq!(bag.get(Place::Ui(stations::UI_SMITHING_INPUT)), None);
        assert_eq!(handled.events, vec![Event::Smithed((1, 2, 3))]);

        // Отделка: тот же запрос, шаблон sentry, золото.
        let mut trim = Bag::new(Inventory::new());

        fill(&mut trim, "sentry_armor_trim_smithing_template", item("iron_helmet", 1), "gold_ingot");
        trim.handle_requests(&craft(true), false);
        assert_eq!(trim.get(Place::Cursor).and_then(|stack| stack.extras().trim), Some((4, 8)));

        // Не всё израсходовано — отказ, ячейки на месте.
        let mut cheat = Bag::new(Inventory::new());

        fill(&mut cheat, "sentry_armor_trim_smithing_template", item("iron_helmet", 1), "gold_ingot");
        cheat.handle_requests(&craft(false), false);
        assert_eq!(cheat.get(Place::Cursor), None);
        assert_eq!(cheat.get(Place::Ui(stations::UI_SMITHING_INPUT)).map(|stack| stack.count), Some(1));

        // Чужой номер рецепта — тоже отказ.
        let mut wrong = Bag::new(Inventory::new());

        fill(&mut wrong, "sentry_armor_trim_smithing_template", item("iron_helmet", 1), "gold_ingot");
        wrong.handle_requests(&request(-23, 5, |out| {
            out.u8(12).varint(1).u8(1);
            consume_action(out, 1, stations::SMITHING_TEMPLATE, stations::UI_SMITHING_TEMPLATE);
            consume_action(out, 1, stations::SMITHING_INPUT, stations::UI_SMITHING_INPUT);
            consume_action(out, 1, stations::SMITHING_MATERIAL, stations::UI_SMITHING_MATERIAL);
            take_output(out, 1);
        }), false);
        assert_eq!(wrong.get(Place::Cursor), None);
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

    /// Сделка жителя: 24 бумаги за изумруд, как у библиотекаря новичка (BDS).
    fn trading_bag() -> (Bag, u32) {
        let mut bag = Bag::new(Inventory::new());
        let (paper, emerald) = (item("paper", 24), item("emerald", 1));

        bag.inventory.set(FIRST_HOTBAR, Some(item("paper", 40)));
        bag.open_station(Station { kind: stations::Kind::Trading, pos: (7, 0, 0), seed: 0, shelves: 0 });
        bag.set_trade(vec![crate::bedrock::trade::Price { net_id: 3000, a: (paper.item, 24), b: None, sell: emerald, stocked: true, left: 16 }]);
        (bag, 3000)
    }

    /// Запрос сделки как у клиента: рецепт, итог, расход, взять итог в панель.
    fn trade_request(bag: &mut Bag, net_id: u32, spend: u8) -> Handled {
        let payload = request(-3, 4, |out| {
            out.u8(12).varint(net_id).u8(1);
            out.u8(19).varint(0).u8(1);
            consume_action(out, spend, stations::TRADE2_INGREDIENT1, stations::UI_TRADE_FIRST);
            out.u8(0).u8(1);
            slot(out, CREATIVE_OUTPUT, 50);
            slot(out, HOTBAR_AND_INVENTORY, 8);
        });

        bag.handle_requests(&payload, false)
    }

    #[test]
    fn a_villager_trade_pays_and_hands_over_the_result() {
        let (mut bag, id) = trading_bag();
        let put = request(-1, 1, |out| {
            out.u8(0).u8(24);
            slot(out, HOTBAR_AND_INVENTORY, 0);
            slot(out, stations::TRADE2_INGREDIENT1, stations::UI_TRADE_FIRST);
        });

        bag.handle_requests(&put, false);
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), Some(item("paper", 24)));

        let handled = trade_request(&mut bag, id, 24);

        assert_eq!(handled.events, vec![Event::Traded(0)]);
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), None);
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR + 8), Some(item("emerald", 1)));
        // Ответ называет оплату и панель, но не выход (запись BDS).
        assert_eq!(handled.response[1], 1);
    }

    /// Пачка по Shift: два раза по 24 бумаги за изумруд одним запросом.
    fn batch_request(bag: &mut Bag, net_id: u32, times: u8, spend: u8, take: u8) -> Handled {
        let payload = request(-3, 4, |out| {
            out.u8(12).varint(net_id).u8(times);
            out.u8(19).varint(0).u8(times);
            consume_action(out, spend, stations::TRADE2_INGREDIENT1, stations::UI_TRADE_FIRST);
            out.u8(0).u8(take);
            slot(out, CREATIVE_OUTPUT, 50);
            slot(out, HOTBAR_AND_INVENTORY, 8);
        });

        bag.handle_requests(&payload, false)
    }

    #[test]
    fn a_shift_click_batch_pays_and_settles_each_trade() {
        let (mut bag, id) = trading_bag();

        bag.inventory.set(FIRST_HOTBAR, Some(item("paper", 64)));
        bag.set(Place::Ui(stations::UI_TRADE_FIRST), Some(item("paper", 48)));

        let handled = batch_request(&mut bag, id, 2, 48, 2);

        // Каждая сделка — своё событие, итог — одна стопка из двух.
        assert_eq!(handled.events, vec![Event::Traded(0), Event::Traded(0)]);
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), None);
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR + 8), Some(item("emerald", 2)));
    }

    #[test]
    fn a_batch_beyond_the_stock_or_the_payment_is_refused() {
        let (mut bag, id) = trading_bag();

        // Оплаты на полторы сделки: две не проходят, ничего не меняется.
        bag.set(Place::Ui(stations::UI_TRADE_FIRST), Some(item("paper", 36)));
        assert!(batch_request(&mut bag, id, 2, 36, 2).events.is_empty());
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), Some(item("paper", 36)));
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR + 8), None);

        // Запаса на одну: две не проходят, хотя оплаты хватает.
        bag.set(Place::Ui(stations::UI_TRADE_FIRST), Some(item("paper", 48)));
        bag.set_trade(vec![crate::bedrock::trade::Price { net_id: id, a: (item("paper", 1).item, 24), b: None, sell: item("emerald", 1), stocked: true, left: 1 }]);
        assert!(batch_request(&mut bag, id, 2, 48, 2).events.is_empty());
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), Some(item("paper", 48)));

        // Одна из одной проходит.
        assert_eq!(batch_request(&mut bag, id, 1, 24, 1).events, vec![Event::Traded(0)]);
    }

    #[test]
    fn an_underpaid_or_unknown_trade_changes_nothing() {
        let (mut bag, id) = trading_bag();

        bag.set(Place::Ui(stations::UI_TRADE_FIRST), Some(item("paper", 23)));

        let handled = trade_request(&mut bag, id, 23);

        assert!(handled.events.is_empty());
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), Some(item("paper", 23)));
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR + 8), None);

        bag.set(Place::Ui(stations::UI_TRADE_FIRST), Some(item("paper", 24)));
        trade_request(&mut bag, id + 5, 24);
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), Some(item("paper", 24)));

        // Взято меньше цены (потрачено 20 из 24) — отказ.
        trade_request(&mut bag, id, 20);
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), Some(item("paper", 24)));
    }

    #[test]
    fn a_sold_out_trade_is_refused_and_closing_returns_the_payment() {
        let (mut bag, id) = trading_bag();

        bag.set(Place::Ui(stations::UI_TRADE_FIRST), Some(item("paper", 24)));
        bag.set_trade(vec![crate::bedrock::trade::Price { net_id: id, a: (item("paper", 1).item, 24), b: None, sell: item("emerald", 1), stocked: false, left: 0 }]);
        assert!(trade_request(&mut bag, id, 24).events.is_empty());

        let (overflow, changed) = bag.close_station();

        assert!(overflow.is_empty() && !changed.is_empty());
        assert_eq!(bag.get(Place::Ui(stations::UI_TRADE_FIRST)), None);
        assert_eq!(bag.inventory.slot(FIRST_HOTBAR).map(|stack| stack.count), Some(64));
    }
    #[test]
    fn crafting_rejects_free_output_zero_times_and_oversized_batches() {
        let target=item("crafting_table",1).item;
        let id=(1..2000).find(|i|stations::recipe_by_network_id(*i).is_some_and(|r|r.result==target)).unwrap();
        for times in [0,1,65,255] {
            let mut bag=Bag::new(Inventory::new());
            let data=request(-80,2,|out|{out.u8(12).varint(id).u8(times);take_output(out,1);});
            let handled=bag.handle_requests(&data,false);assert!(handled.events.is_empty());assert!(bag.get(Place::Cursor).is_none());
        }
    }
    #[test]
    fn automatic_batches_validate_aggregate_ingredients() {
        let target=item("stick",1).item;
        let id=(1..stations::smithing_base()).find(|i|stations::recipe_by_network_id(*i).is_some_and(|r|r.result==target && r.cells.len()==2 && r.cells[0].contains(&item("oak_planks",1).item))).unwrap();
        let mut bag=Bag::new(Inventory::new());bag.set(Place::Ui(28),Some(item("oak_planks",2)));bag.set(Place::Ui(30),Some(item("spruce_planks",2)));
        let data=request(-81,4,|out|{out.u8(13).varint(id).u8(2).u8(2).varint(0);consume_action(out,2,stations::CRAFTING_INPUT,28);consume_action(out,2,stations::CRAFTING_INPUT,30);take_output(out,8);});
        let handled=bag.handle_requests(&data,false);assert_eq!(bag.get(Place::Cursor),Some(item("stick",8)));assert_eq!(handled.events,vec![Event::Crafted(target,8)]);
    }
    #[test]
    fn dynamic_fireworks_and_repair_preserve_server_components() {
        let rocket=item("firework_rocket",1).item;
        let id=(1..stations::smithing_base()).find(|i|stations::dynamic_recipe(*i) && stations::recipe_by_network_id(*i).is_some_and(|r|r.result==rocket && r.cells.len()==3 && r.cells[2].contains(&item("firework_star",1).item))).unwrap();
        let mut bag=Bag::new(Inventory::new());let star=item("firework_star",1).with_extras(crate::stackdata::Data{explosion:Some(crate::stackdata::Explosion{colors:vec![11743532],shape:4,..Default::default()}),..Default::default()});
        bag.set(Place::Ui(28),Some(item("paper",1)));bag.set(Place::Ui(29),Some(item("gunpowder",1)));bag.set(Place::Ui(30),Some(star));
        let data=request(-82,5,|out|{out.u8(12).varint(id).u8(1);for slot in 28..=30 {consume_action(out,1,stations::CRAFTING_INPUT,slot);}take_output(out,3);});
        bag.handle_requests(&data,false);let crafted=bag.get(Place::Cursor).unwrap();assert_eq!(crafted.item,rocket);assert_eq!(crafted.extras().rocket().explosions,vec![star.extras().explosion.unwrap()]);
    }

}
