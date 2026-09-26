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

use std::collections::BTreeMap;

use super::codec::{In, Out};
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

    if id == SHIELD {
        out.varint(18).lu16(0).li32(0).li32(0).li64(0);
    } else {
        out.varint(10).lu16(0).li32(0).li32(0);
    }
}

/// Сетевой номер щита у Bedrock 1.26.10 (minecraft-data, items.json): у него
/// особый вид дополнительных данных предмета.
const SHIELD: i32 = 387;

/// Где лежит стопка.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Place {
    /// Слот общей раскладки.
    Slot(i32),
    /// То, что игрок держит курсором в открытом инвентаре.
    Cursor,
    /// Выход творческого меню: сюда кладётся созданный предмет.
    CreativeOutput,
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
    ids: BTreeMap<Place, i32>,
    next_id: i32,
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
        let mut bag = Bag { inventory, cursor: None, creative_output: None, ids: BTreeMap::new(), next_id: 1 };

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
        }
    }

    fn set(&mut self, place: Place, stack: Option<Stack>) {
        let stack = stack.filter(|s| s.count > 0);

        match place {
            Place::Slot(slot) => self.inventory.set(slot, stack),
            Place::Cursor => self.cursor = stack,
            Place::CreativeOutput => self.creative_output = stack,
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

    /// Уходя, игрок не уносит курсор: что в нём, возвращается в инвентарь.
    pub fn put_cursor_back(&mut self) {
        if let Some(stack) = self.cursor.take() {
            self.inventory.add(stack);
        }
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
        Handled { response: response.bytes, thrown }
    }

    /// Один запрос из Player Auth Input: там он лежит без списка вокруг.
    /// Ответ — отдельным пакетом, как и на обычные запросы.
    pub fn handle_embedded(&mut self, reader: &mut In, creative: bool) -> (Handled, bool) {
        let mut answers = Out::default();
        let mut thrown = Vec::new();
        let parsed = self.handle_one(reader, creative, &mut answers, &mut thrown);

        let mut response = Out::packet(super::session::ITEM_STACK_RESPONSE);
        response.varint(u32::from(parsed != Parsed::Nothing)).raw(&answers.bytes);
        (Handled { response: response.bytes, thrown }, parsed == Parsed::Whole)
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

        let result = self.run_actions(reader, creative, &mut touched, &mut dropped);

        // Хвост запроса: имена (для наковальни) и причина.
        let tail = result.is_some()
            && (|| {
                for _ in 0..reader.varint()? {
                    let length = reader.varint()? as usize;
                    reader.take(length)?;
                }
                reader.li32()
            })()
            .is_some();

        if result == Some(true) && tail {
            answers.u8(0).zigzag32(request_id);
            self.write_containers(answers, &touched);
            thrown.extend(dropped);
        } else {
            *self = before;
            answers.u8(1).zigzag32(request_id);
        }

        if tail { Parsed::Whole } else { Parsed::Broken }
    }

    /// Действия одного запроса: удались ли все. None — разобрать не вышло.
    fn run_actions(
        &mut self,
        reader: &mut In,
        creative: bool,
        touched: &mut Vec<(Named, Place)>,
        dropped: &mut Vec<Stack>,
    ) -> Option<bool> {
        let mut failed = false;

        for _ in 0..reader.varint()? {
            let kind = reader.u8()?;

            let ok = match kind {
                // Взять / положить: count штук из source в destination.
                0 | 1 | 7 | 8 => {
                    let count = reader.u8()? as i32;
                    let source = read_slot(reader)?;
                    let destination = read_slot(reader)?;
                    self.move_stack(count, source, destination, touched)
                }
                // Поменять местами.
                2 => {
                    let source = read_slot(reader)?;
                    let destination = read_slot(reader)?;
                    self.swap(source, destination, touched)
                }
                // Выбросить.
                3 => {
                    let count = reader.u8()? as i32;
                    let source = read_slot(reader)?;
                    let _randomly = reader.u8()?;
                    self.remove(count, source, touched).map(|stack| dropped.push(stack)).is_some()
                }
                // Уничтожить (творческий) или израсходовать.
                4 | 5 => {
                    let count = reader.u8()? as i32;
                    let source = read_slot(reader)?;
                    (kind == 5 || creative) && self.remove(count, source, touched).is_some()
                }
                // «Создать» и устаревший список итогов — только сопровождают
                // другие действия.
                6 => {
                    reader.u8()?;
                    true
                }
                19 => {
                    for _ in 0..reader.varint()? {
                        skip_item_legacy(reader)?;
                    }
                    reader.u8()?;
                    true
                }
                // Добыча блока инструментом — прочности у нас пока нет.
                11 => {
                    reader.zigzag32()?;
                    reader.zigzag32()?;
                    reader.zigzag32()?;
                    true
                }
                // Предмет из творческого меню.
                14 => {
                    let entry = reader.varint()?;
                    let _times = reader.u8()?;
                    creative && self.craft_creative(entry)
                }
                // Всё остальное (крафт, наковальня, ткацкий станок…) пока не
                // умеем. Разобрать длину тоже не всегда можно — отказ.
                _ => return None,
            };

            failed |= !ok;
        }

        Some(!failed)
    }

    fn craft_creative(&mut self, entry: u32) -> bool {
        let Some(item) = creative_item(entry) else {
            return false;
        };

        self.set(Place::CreativeOutput, Some(Stack::new(item, blocks::stack_size(item))));
        true
    }

    fn move_stack(&mut self, count: i32, source: Named, destination: Named, touched: &mut Vec<(Named, Place)>) -> bool {
        let (Some(from), Some(to)) = (place(source.container, source.slot), place(destination.container, destination.slot))
        else {
            return false;
        };
        let Some(moving) = self.get(from) else {
            return false;
        };

        if count <= 0 || count > moving.count {
            return false;
        }

        let there = self.get(to);

        if there.is_some_and(|t| t.item != moving.item) {
            return false;
        }

        let total = there.map_or(0, |t| t.count) + count;

        if total > blocks::stack_size(moving.item) {
            return false;
        }

        self.set(from, Some(Stack::new(moving.item, moving.count - count)));
        self.set(to, Some(Stack::new(moving.item, total)));
        touched.push((source, from));
        touched.push((destination, to));
        true
    }

    fn swap(&mut self, source: Named, destination: Named, touched: &mut Vec<(Named, Place)>) -> bool {
        let (Some(from), Some(to)) = (place(source.container, source.slot), place(destination.container, destination.slot))
        else {
            return false;
        };
        let (a, b) = (self.get(from), self.get(to));

        self.set(from, b);
        self.set(to, a);
        touched.push((source, from));
        touched.push((destination, to));
        true
    }

    fn remove(&mut self, count: i32, source: Named, touched: &mut Vec<(Named, Place)>) -> Option<Stack> {
        let from = place(source.container, source.slot)?;
        let stack = self.get(from)?;

        if count <= 0 || count > stack.count {
            return None;
        }

        self.set(from, Some(Stack::new(stack.item, stack.count - count)));
        touched.push((source, from));
        Some(Stack::new(stack.item, count))
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
    fn java_items_have_bedrock_forms() {
        let stone = crate::blocks::item_named("stone").expect("камень");
        assert_eq!(bedrock_item(stone), Some((1, 0, 2533)));

        let red_bed = crate::blocks::item_named("red_bed").expect("кровать");
        assert_eq!(bedrock_item(red_bed).map(|(_, metadata, _)| metadata), Some(14));
    }
}
