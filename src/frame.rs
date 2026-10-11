// Рамки для предметов и светящиеся рамки: сущность, что висит на грани блока.
//
// Кода игры и чужих ядер здесь нет: правила взяты со страниц minecraft.wiki
// (Item Frame, Glow Item Frame, Redstone Comparator), а числа, которых на вики
// нет, сняты с официального сервера чёрным ящиком — сценарии
// tools/blackbox/scenarios/item-frame-*.json и glow-frame.json: место
// сущности, углы, метаданные, звуки, порядок пакетов, срок проверки опоры.
//
// Что не сверено, помечено «[проверить]».

use crate::blocks;
use crate::inventory::Stack;
use crate::items::{Change, Items, MINED_DELAY};
use crate::redstone::{self, FrameMark};
use crate::shared::Shared;
use crate::world::World;

/// Стороны как в игре (вики, Item Frame, «Facing»): 0 — низ, 1 — верх,
/// 2 — север, 3 — юг, 4 — запад, 5 — восток. Это же номер стороны в данных
/// пакета появления.
pub const DOWN: u8 = 0;
pub const UP: u8 = 1;
pub const NORTH: u8 = 2;
pub const SOUTH: u8 = 3;
pub const WEST: u8 = 4;
pub const EAST: u8 = 5;

/// Сторона по умолчанию у метаданных — юг: у смотрящей на юг рамки пакет
/// свойств «сторона» не шлётся (чёрный ящик, item-frame-facing).
pub const DEFAULT_FACING: u8 = SOUTH;

/// Номера вида сущности для клиента Java (minecraft-data 26.1, entities.json).
pub const ITEM_FRAME_TYPE: i32 = 73;
pub const GLOW_FRAME_TYPE: i32 = 60;

/// Номера свойств: сторона, предмет, поворот (minecraft-data 26.1,
/// entities.json, metadataKeys рамки: девятое, десятое и одиннадцатое).
pub const DIRECTION_PROPERTY: u8 = 8;
pub const ITEM_PROPERTY: u8 = 9;
pub const ROTATION_PROPERTY: u8 = 10;

/// Вид значения «сторона» в списке видов свойств (protocol.json, 12).
pub const PROPERTY_DIRECTION: i32 = 12;

/// Сколько тактов от появления до проверки опоры и раз в сколько она
/// повторяется: у оригинала рамка без опоры отваливается ровно на сотом такте
/// жизни (чёрный ящик, item-frame-facing и item-frame-break: убрали камень —
/// рамка упала через 100 тактов после своего появления).
pub const SURVIVAL_CHECK: u64 = 100;

/// Толщина рамки — 1/16 блока: центр сдвинут к стене на полблока минус
/// половину толщины (чёрный ящик: звук и выпавший предмет у западной рамки
/// на 0,96875 от её западной границы блока).
const WALL_OFFSET: f64 = 0.5 - 1.0 / 32.0;

/// Предмет, выпавший из рамки, ложится на 0,15 блока от неё вперёд (чёрный
/// ящик: у рамки на западной стене x предмета 0,82 при рамке в 0,96875).
const DROP_AHEAD: f64 = 0.15;

/// Звуки (имена как в вики, Item Frame, «Sounds»).
pub fn sound(glow: bool, what: Sound) -> &'static str {
    match (glow, what) {
        (false, Sound::Place) => "minecraft:entity.item_frame.place",
        (false, Sound::AddItem) => "minecraft:entity.item_frame.add_item",
        (false, Sound::Rotate) => "minecraft:entity.item_frame.rotate_item",
        (false, Sound::RemoveItem) => "minecraft:entity.item_frame.remove_item",
        (false, Sound::Break) => "minecraft:entity.item_frame.break",
        (true, Sound::Place) => "minecraft:entity.glow_item_frame.place",
        (true, Sound::AddItem) => "minecraft:entity.glow_item_frame.add_item",
        (true, Sound::Rotate) => "minecraft:entity.glow_item_frame.rotate_item",
        (true, Sound::RemoveItem) => "minecraft:entity.glow_item_frame.remove_item",
        (true, Sound::Break) => "minecraft:entity.glow_item_frame.break",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Place,
    AddItem,
    Rotate,
    RemoveItem,
    Break,
}

/// Рамка в мире.
#[derive(Clone, PartialEq, Debug)]
pub struct Frame {
    pub entity_id: i32,
    pub uuid: [u8; 16],

    /// Клетка, в которой рамка висит (не блок, на котором она держится).
    pub pos: (i32, i32, i32),

    /// Куда смотрит лицо рамки, 0..=5.
    pub facing: u8,

    /// Светящаяся ли.
    pub glow: bool,
    pub painting: Option<u8>,

    /// Что в рамке.
    pub item: Option<Stack>,

    /// Поворот предмета: 0..=7, по 45° за щелчок.
    pub rotation: u8,

    /// Сколько тактов рамка живёт.
    pub age: u64,
}

impl Frame {
    pub fn new(entity_id: i32, pos: (i32, i32, i32), facing: u8, glow: bool) -> Self {
        Self { entity_id, uuid: uuid_for(entity_id), pos, facing: facing.min(EAST), glow, painting: None, item: None, rotation: 0, age: 0 }
    }

    /// Вид сущности для клиента.
    pub fn java_type(&self) -> i32 {
        if self.painting.is_some() {return 93;}
        if self.glow { GLOW_FRAME_TYPE } else { ITEM_FRAME_TYPE }
    }

    /// Имя предмета рамки (он же выпадает при поломке).
    pub fn item_name(&self) -> &'static str {
        if self.painting.is_some() {return "painting";}
        if self.glow { "glow_item_frame" } else { "item_frame" }
    }

    /// Сдвиг клетки за лицо рамки: куда она смотрит.
    pub fn step(&self) -> (i32, i32, i32) {
        step_of(self.facing)
    }

    /// Клетка блока, на котором рамка держится.
    pub fn support(&self) -> (i32, i32, i32) {
        let (dx, dy, dz) = self.step();

        (self.pos.0 - dx, self.pos.1 - dy, self.pos.2 - dz)
    }

    /// Настоящее место рамки: середина клетки, сдвинутая к стене. Звук
    /// рамки — здесь.
    pub fn place(&self) -> (f64, f64, f64) {
        let (dx, dy, dz) = self.step();
        let (across,up)=self.painting.and_then(|id|crate::painting::VARIANTS.get(id as usize)).map_or((0.0,0.0),|(_,w,h)|(if w%2==0 {0.5} else {0.0},if h%2==0 {0.5} else {0.0}));

        (
            f64::from(self.pos.0) + 0.5 - f64::from(dx) * WALL_OFFSET + f64::from(dz)*across,
            f64::from(self.pos.1) + 0.5 - f64::from(dy) * WALL_OFFSET + up,
            f64::from(self.pos.2) + 0.5 - f64::from(dz) * WALL_OFFSET - f64::from(dx)*across,
        )
    }

    /// Куда падает предмет из рамки: вперёд от лица на 0,15 — только по
    /// горизонтали, у верхней и нижней рамок высота остаётся рамкиной.
    pub fn drop_place(&self) -> (f64, f64, f64) {
        let (x, y, z) = self.place();
        let (dx, _, dz) = self.step();

        (x + f64::from(dx) * DROP_AHEAD, y, z + f64::from(dz) * DROP_AHEAD)
    }

    /// Сила, которую читает сравнитель: с предметом — поворот плюс один,
    /// пустая — ноль (вики, Redstone Comparator: «0 if the item frame is
    /// empty, or 1 to 8 for any item depending on its rotation»).
    pub fn comparator_power(&self) -> u8 {
        if self.item.is_some() { self.rotation + 1 } else { 0 }
    }

    /// Щелчок с предметом в руке по пустой рамке кладёт его; по полной —
    /// поворачивает предмет на 45°. Возвращает, что случилось.
    pub fn use_with(&mut self, held: Option<Stack>) -> Used {
        if self.painting.is_some() {return Used::Nothing;}
        if self.item.is_none() {
            let Some(held) = held else {
                return Used::Nothing;
            };

            self.item = Some(held.with_count(1));
            self.rotation = 0;
            return Used::Added;
        }

        self.rotation = (self.rotation + 1) % 8;
        Used::Rotated
    }

    /// Пакет появления: место — клетка целиком (не настоящее место!), углы —
    /// сторона, данные — номер стороны (чёрный ящик: `spawn` рамки в клетке
    /// 0,0,0 при настоящем месте 0,96875).
    pub fn spawn_place(&self) -> (f64, f64, f64) {
        (f64::from(self.pos.0), f64::from(self.pos.1), f64::from(self.pos.2))
    }

    /// Углы в пакете появления, байтами: (наклон, поворот). Верх и низ —
    /// наклон, стороны света — поворот (чёрный ящик: низ pitch=64, верх −64;
    /// север −128, запад 64, восток −64).
    pub fn angles(&self) -> (u8, u8) {
        match self.facing {
            DOWN => (64, 0),
            UP => (192, 0),
            NORTH => (0, 128),
            SOUTH => (0, 0),
            WEST => (0, 64),
            _ => (0, 192),
        }
    }

    /// Держится ли рамка: опорная клетка должна иметь ящик (не воздух, не
    /// жидкость, не трава). Повторитель и сравнитель вместо ящика тоже
    /// годятся у боковых рамок (вики, Item Frame: «they can also attach to
    /// non-solid blocks like repeaters and comparators»). [проверить]: точный
    /// список опор у оригинала — плиты, ступени, заборы приняты за опору.
    pub fn supported(&self, state_at: &dyn Fn(i32, i32, i32) -> i32) -> bool {
        if self.painting.is_some() {return crate::painting::supported(self,state_at);}
        let (x, y, z) = self.support();

        can_hang_on(state_at(x, y, z), self.facing)
    }
}

/// Годится ли блок в опору рамке, что смотрит в сторону `facing`.
pub fn can_hang_on(state: i32, facing: u8) -> bool {
    if state == 0 {
        return false;
    }

    if blocks::has_box(state) {
        return true;
    }

    // Повторитель и сравнитель тонкие, но рамку на боку держат.
    facing != UP
        && facing != DOWN
        && blocks::block_at_state(state).is_some_and(|name| name == "repeater" || name == "comparator")
}

/// Что вышло из щелчка по рамке.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Used {
    Nothing,
    Added,
    Rotated,
}

/// Сдвиг клетки по номеру стороны.
pub fn step_of(facing: u8) -> (i32, i32, i32) {
    match facing {
        DOWN => (0, -1, 0),
        UP => (0, 1, 0),
        NORTH => (0, 0, -1),
        SOUTH => (0, 0, 1),
        WEST => (-1, 0, 0),
        _ => (1, 0, 0),
    }
}

/// Уникальный номер сущности для клиента: как у прочих сущностей.
fn uuid_for(entity_id: i32) -> [u8; 16] {
    let mut uuid = [0u8; 16];

    uuid[0..4].copy_from_slice(b"FRAM");
    uuid[12..16].copy_from_slice(&entity_id.to_be_bytes());
    uuid
}

// ---------------------------------------------------------------------------
// Действия игрока и мира с рамками
// ---------------------------------------------------------------------------

/// Пересказывает редстоуну, что показывают рамки (сравнитель читает их из
/// памяти мира).
pub fn refresh_marks(world: &mut World, items: &Items) {
    let marks = items
        .frames()
        .iter()
        .filter(|frame|frame.painting.is_none())
        .map(|frame| FrameMark { pos: frame.pos, facing: frame.facing, power: frame.comparator_power() })
        .collect();

    redstone::note_frames(world, marks);
}

/// Ставит рамку в клетку за гранью `face` блока `clicked`. Возвращает,
/// поставилась ли: опорный блок должен держать рамку, клетка — быть
/// свободной от блоков с ящиком, и там не должно висеть такой же рамки на
/// той же стороне (вики, Item Frame: несколько рамок в одной клетке — только
/// на разных сторонах).
pub fn place(world: &mut World, items: &mut Items, shared: &Shared, clicked: (i32, i32, i32), face: u8, glow: bool) -> bool {
    let (dx, dy, dz) = step_of(face);
    let pos = (clicked.0 + dx, clicked.1 + dy, clicked.2 + dz);

    if !can_hang_on(world.get_block(clicked.0, clicked.1, clicked.2), face) {
        return false;
    }

    let inside = world.get_block(pos.0, pos.1, pos.2);

    if inside != 0 && blocks::has_box(inside) {
        return false;
    }

    if items.frames().iter().any(|there| there.pos == pos && there.facing == face) {
        return false;
    }

    let frame = Frame::new(shared.next_entity_id(), pos, face, glow);

    items.note_frame(Change::FrameSound { at: frame.place(), name: sound(glow, Sound::Place) });
    world.store_frame(&frame);
    items.add_frame(frame);
    refresh_marks(world, items);
    true
}

/// Щелчок игрока по рамке: с предметом в руке кладёт его, по полной рамке —
/// поворачивает предмет. Возвращает, что вышло (кроме `Used::Nothing` из руки
/// берётся один предмет — забота вызывающего). Звук и свойство уходят
/// клиентам в этом порядке (чёрный ящик, item-frame-rotate), а сравнители
/// вокруг узнают о смене сразу.
pub fn use_on(world: &mut World, items: &mut Items, entity_id: i32, held: Option<Stack>) -> Used {
    let Some(frame) = items.frames_mut().iter_mut().find(|frame| frame.entity_id == entity_id) else {
        return Used::Nothing;
    };

    let used = frame.use_with(held);
    let (glow, at, pos) = (frame.glow, frame.place(), frame.pos);
    let (item, rotation) = (frame.item, frame.rotation);
    let stored = frame.clone();

    match used {
        Used::Nothing => return used,
        Used::Added => {
            items.note_frame(Change::FrameSound { at, name: sound(glow, Sound::AddItem) });
            items.note_frame(Change::FrameItem { entity_id, item });
        }
        Used::Rotated => {
            items.note_frame(Change::FrameSound { at, name: sound(glow, Sound::Rotate) });
            items.note_frame(Change::FrameRotation { entity_id, rotation });
        }
    }

    world.store_frame(&stored);
    refresh_marks(world, items);
    redstone::comparator_update(world, pos);
    used
}

/// Удар игрока по рамке: предмет из неё выходит (выпадает, если игрок не в
/// творческом), а пустая рамка ломается (выпадает она сама, в творческом —
/// ничего). Возвращает, была ли такая рамка.
pub fn strike(world: &mut World, items: &mut Items, shared: &Shared, entity_id: i32, creative: bool) -> bool {
    let Some(frame) = items.frames_mut().iter_mut().find(|frame| frame.entity_id == entity_id) else {
        return false;
    };

    if let Some(stack) = frame.item.take() {
        frame.rotation = 0;

        let (glow, at, pos) = (frame.glow, frame.place(), frame.pos);
        let drop_at = frame.drop_place();
        let stored = frame.clone();

        // Сперва предмет появляется в мире, потом звук и свойство
        // (чёрный ящик, item-frame-rotate).
        if !creative {
            spill(world, items, shared, stack, drop_at);
        }

        items.note_frame(Change::FrameSound { at, name: sound(glow, Sound::RemoveItem) });
        items.note_frame(Change::FrameItem { entity_id, item: None });
        world.store_frame(&stored);
        refresh_marks(world, items);
        redstone::comparator_update(world, pos);
        return true;
    }

    break_off(world, items, shared, entity_id, creative);
    true
}

/// Рамка ломается целиком, вместе с предметом внутри (выпадают оба; в
/// творческом — ничего): так выходит у Bedrock, где рамка — блок и ломается
/// одним ударом. Возвращает, была ли такая рамка.
pub fn break_off(world: &mut World, items: &mut Items, shared: &Shared, entity_id: i32, creative: bool) -> bool {
    let removed = items.remove_frames(|frame| frame.entity_id == entity_id);

    for frame in &removed {
        break_frame(world, items, shared, frame, !creative);
        refresh_marks(world, items);
        redstone::comparator_update(world, frame.pos);
    }

    !removed.is_empty()
}

/// Рамка уже убрана из мира (клиентам ушёл `Gone`): звучит поломка, запись
/// стирается, а если `drops` — выпадают сама рамка и её предмет.
pub fn break_frame(world: &mut World, items: &mut Items, shared: &Shared, frame: &Frame, drops: bool) {
    items.note_frame(Change::FrameSound { at: frame.place(), name: if frame.painting.is_some() {"minecraft:entity.painting.break"} else {sound(frame.glow, Sound::Break)} });
    world.forget_frame(frame.pos, frame.facing);

    if !drops {
        return;
    }

    let at = frame.drop_place();

    if let Some(own) = blocks::item_named(frame.item_name()) {
        spill(world, items, shared, Stack::new(own, 1), at);
    }

    if let Some(stack) = frame.item {
        spill(world, items, shared, stack, at);
    }
}

/// Роняет предмет: вверх на 0,2, вбок случайно на ±0,1 (чёрный ящик: скорости
/// выпавших из рамки предметов).
fn spill(world: &mut World, items: &mut Items, shared: &Shared, stack: Stack, at: (f64, f64, f64)) {
    let velocity = (f64::from(world.random_unit()) * 0.2 - 0.1, 0.2, f64::from(world.random_unit()) * 0.2 - 0.1);

    items.drop_item(shared.next_entity_id(), stack, at, velocity, MINED_DELAY);
}

// ---------------------------------------------------------------------------
// Файл чанка
// ---------------------------------------------------------------------------

/// Запись рамки для файла региона: место внутри чанка (x, z — 4 бита, y —
/// два байта), сторона и признаки, поворот, потом предмет (номер и число),
/// если он есть. Возраст не пишется: проверка опоры после загрузки идёт
/// заново от нуля. [проверить]: у оригинала возраст хранится не там.
pub fn write(frame: &Frame, out: &mut Vec<u8>) {
    out.push(((frame.pos.0.rem_euclid(16) << 4) | frame.pos.2.rem_euclid(16)) as u8);
    out.extend_from_slice(&(frame.pos.1 as i16).to_be_bytes());
    out.push(frame.facing | (u8::from(frame.glow) << 3) | (u8::from(frame.item.is_some()) << 4) | (u8::from(frame.painting.is_some()) << 5));
    out.push(frame.rotation);
    if let Some(variant)=frame.painting {out.push(variant);}

    if let Some(stack) = frame.item {
        out.extend_from_slice(&stack.stored_item().to_be_bytes());
        out.push(stack.count.clamp(1, 127) as u8);
        stack.write_extras(out);
    }
}

/// Читает запись `write`; номер сущности выдаёт вызывающий. None — запись
/// испорчена.
pub fn read(bytes: &mut &[u8], chunk_x: i32, chunk_z: i32, entity_id: i32) -> Option<Frame> {
    let mut take = |count: usize| -> Option<&[u8]> {
        let (head, rest) = bytes.split_at_checked(count)?;
        *bytes = rest;
        Some(head)
    };

    let corner = take(1)?[0];
    let y = i16::from_be_bytes(take(2)?.try_into().ok()?) as i32;
    let flags = take(1)?[0];
    let rotation = take(1)?[0] & 7;
    let facing = flags & 7;

    if facing > EAST {
        return None;
    }

    let pos = (chunk_x * 16 + (corner >> 4) as i32, y, chunk_z * 16 + (corner & 15) as i32);
    let mut frame = Frame::new(entity_id, pos, facing, flags & 8 != 0);

    frame.rotation = rotation;
    if flags&32!=0 {let v=take(1)?[0]; if usize::from(v)>=crate::painting::VARIANTS.len() {return None;} frame.painting=Some(v);}

    if flags & 16 != 0 {
        let raw = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let mut stored = Stack::from_stored(raw, 0);
        let count = i32::from(take(1)?[0]);

        if let Some(extras) = Stack::read_extras(raw, &mut take)? {
            stored = stored.with_extras(extras);
        }

        if blocks::stack_size(stored.item) > 0 && count > 0 {
            frame.item = Some(stored.with_count(1));
        }
    }

    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone() -> Stack {
        Stack::new(blocks::item_named("stone").expect("камень"), 1)
    }

    #[test]
    fn place_and_drop_follow_the_wall() {
        // Западная рамка в клетке 0,0,0 на камне в 1,0,0.
        let frame = Frame::new(1, (0, 0, 0), WEST, false);

        assert_eq!(frame.support(), (1, 0, 0));
        assert_eq!(frame.place(), (0.96875, 0.5, 0.5));
        assert!((frame.drop_place().0 - 0.81875).abs() < 1e-9);
        assert_eq!(frame.drop_place().1, 0.5);

        // Нижняя рамка под камнем: место у верха её клетки, предмет прямо
        // под ней, без сдвига вперёд.
        let down = Frame::new(2, (1, 0, 1), DOWN, false);

        assert_eq!(down.place(), (1.5, 0.96875, 1.5));
        assert_eq!(down.drop_place(), (1.5, 0.96875, 1.5));

        // Верхняя — у низа клетки над камнем.
        let up = Frame::new(3, (1, 2, 1), UP, false);

        assert_eq!(up.place(), (1.5, 2.03125, 1.5));

        // Северная и южная стены.
        assert_eq!(Frame::new(4, (1, 1, 0), NORTH, false).place().2, 0.96875);
        assert_eq!(Frame::new(5, (1, 1, 2), SOUTH, false).place().2, 2.03125);
    }

    #[test]
    fn angles_match_the_original() {
        let angles = |facing| Frame::new(1, (0, 0, 0), facing, false).angles();

        assert_eq!(angles(DOWN), (64, 0));
        assert_eq!(angles(UP), (192, 0));
        assert_eq!(angles(NORTH), (0, 128));
        assert_eq!(angles(SOUTH), (0, 0));
        assert_eq!(angles(WEST), (0, 64));
        assert_eq!(angles(EAST), (0, 192));
    }

    #[test]
    fn item_goes_in_then_rotates_eight_times() {
        let mut frame = Frame::new(1, (0, 0, 0), WEST, false);

        // Пустая рамка и пустая рука — ничего.
        assert_eq!(frame.use_with(None), Used::Nothing);
        assert_eq!(frame.comparator_power(), 0);

        assert_eq!(frame.use_with(Some(Stack::new(stone().item, 5))), Used::Added);
        assert_eq!(frame.item, Some(stone()), "кладётся один предмет");
        assert_eq!(frame.comparator_power(), 1);

        // Восемь поворотов: сила 2, 3, … 8, потом снова 1.
        let powers: Vec<u8> = (0..8)
            .map(|_| {
                assert_eq!(frame.use_with(None), Used::Rotated);
                frame.comparator_power()
            })
            .collect();

        assert_eq!(powers, vec![2, 3, 4, 5, 6, 7, 8, 1]);
        assert_eq!(frame.rotation, 0);
    }

    #[test]
    fn support_needs_something_to_hang_on() {
        let stone_state = blocks::state_by_name("stone").expect("камень");
        let air = 0;
        let grass = blocks::state_by_name("short_grass").expect("трава");

        assert!(can_hang_on(stone_state, WEST));
        assert!(!can_hang_on(air, WEST));
        assert!(!can_hang_on(grass, WEST), "трава без ящика не держит");

        let frame = Frame::new(1, (0, 0, 0), WEST, false);

        assert!(frame.supported(&|x, _, _| if x == 1 { stone_state } else { air }));
        assert!(!frame.supported(&|_, _, _| air));
    }

    #[test]
    fn frame_survives_the_file() {
        let mut frame = Frame::new(1, (35, -7, -20), UP, true);

        frame.item = Some(stone());
        frame.rotation = 5;

        let mut bytes = Vec::new();

        write(&frame, &mut bytes);

        // Чанк (2, -2): x 35 — это 3 внутри чанка 2, z −20 — 12 внутри −2.
        let back = read(&mut bytes.as_slice(), 2, -2, 9).expect("прочиталась");

        assert_eq!(back.pos, (35, -7, -20));
        assert_eq!((back.facing, back.glow, back.rotation, back.item), (UP, true, 5, Some(stone())));

        // Пустая без предмета.
        let empty = Frame::new(1, (1, 2, 3), NORTH, false);
        let mut bytes = Vec::new();

        write(&empty, &mut bytes);
        assert_eq!(bytes.len(), 5);
        assert_eq!(read(&mut bytes.as_slice(), 0, 0, 1).expect("прочиталась").item, None);

        // Испорченная сторона.
        assert!(read(&mut [0u8, 0, 0, 7, 0].as_slice(), 0, 0, 1).is_none());
    }

    /// Сервер для проверок: два пустых мира, одна каменная стена в клетке
    /// 1,1,0 и читатель журнала предметов, записанный с самого начала.
    fn server() -> (Shared, u64) {
        let shared = Shared::new(
            World::in_memory(),
            World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsheriffanya-frame-ops-{}.json", std::process::id()))),
            crate::skins::Settings::default(),
        );
        let dimension = crate::dimension::Dimension::Overworld;

        shared.world_in(dimension).lock().unwrap().set_block_silently(1, 1, 0, blocks::state_by_name("stone").unwrap());
        shared.items_in(dimension).lock().unwrap().watch_changes(9);
        (shared, 9)
    }

    /// Что за это время записано в журнал, коротко: `S` — звук, `F` — рамка,
    /// `I` — предмет в рамке, `R` — поворот, `G` — ушла, `D` — выпал предмет.
    fn journal(items: &mut Items, from: &mut usize, reader: u64) -> String {
        let (changes, count) = items.changes_since(*from, reader);

        *from = count;
        changes
            .iter()
            .map(|change| match change {
                Change::FrameSound { .. } => 'S',
                Change::Frame(_) => 'F',
                Change::FrameItem { .. } => 'I',
                Change::FrameRotation { .. } => 'R',
                Change::Gone { .. } => 'G',
                Change::Dropped(_) => 'D',
                _ => '?',
            })
            .collect()
    }

    /// Рамка вешается на грань, у которой есть опора и свободная клетка; на
    /// воздух, на занятое место и второй раз на ту же сторону — нет. Звук идёт
    /// раньше самой рамки (сценарий item-frame-rotate).
    #[test]
    fn a_frame_hangs_only_where_it_can() {
        let (shared, reader) = server();
        let mut world = shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut items = shared.items_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut from = 0;

        assert!(place(&mut world, &mut items, &shared, (1, 1, 0), WEST, false));
        assert_eq!(items.frames()[0].pos, (0, 1, 0));
        assert_eq!(journal(&mut items, &mut from, reader), "SF");

        // Та же сторона занята, воздух не держит.
        assert!(!place(&mut world, &mut items, &shared, (1, 1, 0), WEST, false));
        assert!(!place(&mut world, &mut items, &shared, (5, 1, 0), WEST, false));

        // Другая сторона того же камня — можно, и клетка над камнем тоже.
        assert!(place(&mut world, &mut items, &shared, (1, 1, 0), UP, true));
        assert!(items.frames()[1].glow);
        assert_eq!(items.frames()[1].pos, (1, 2, 0));

        // Клетка, занятая блоком с ящиком, не годится.
        world.set_block_silently(2, 1, 0, blocks::state_by_name("stone").unwrap());
        assert!(!place(&mut world, &mut items, &shared, (1, 1, 0), EAST, false));
        assert_eq!(journal(&mut items, &mut from, reader), "SF");
        let _ = reader;
    }

    /// Предмет кладётся, рамка вращается, комнатный сравнитель узнаёт силу; удар
    /// вынимает предмет (в выживании он выпадает — раньше звука и свойства), а
    /// потом ломает рамку: она уходит, звучит поломка, выпадает рамка.
    #[test]
    fn using_and_striking_a_frame_follows_the_original() {
        let (shared, reader) = server();
        let mut world = shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut items = shared.items_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut from = 0;
        let stone_item = Stack::new(blocks::item_named("stone").unwrap(), 3);

        assert!(place(&mut world, &mut items, &shared, (1, 1, 0), WEST, false));
        let id = items.frames()[0].entity_id;

        journal(&mut items, &mut from, reader);

        assert_eq!(use_on(&mut world, &mut items, id, None), Used::Nothing);
        assert_eq!(journal(&mut items, &mut from, reader), "");

        assert_eq!(use_on(&mut world, &mut items, id, Some(stone_item)), Used::Added);
        assert_eq!(journal(&mut items, &mut from, reader), "SI");
        assert_eq!(redstone::frame_power(&world, (0, 1, 0), WEST), Some(1));

        assert_eq!(use_on(&mut world, &mut items, id, Some(stone_item)), Used::Rotated);
        assert_eq!(journal(&mut items, &mut from, reader), "SR");
        assert_eq!(redstone::frame_power(&world, (0, 1, 0), WEST), Some(2));

        // Удар: предмет выпал, потом звук и свойство; сила — ноль.
        assert!(strike(&mut world, &mut items, &shared, id, false));
        assert_eq!(journal(&mut items, &mut from, reader), "DSI");
        assert_eq!(redstone::frame_power(&world, (0, 1, 0), WEST), Some(0));
        assert_eq!(items.frames().len(), 1);

        // Второй удар: рамка ушла, потом поломка и выпавшая рамка.
        assert!(strike(&mut world, &mut items, &shared, id, false));
        assert_eq!(journal(&mut items, &mut from, reader), "GSD");
        assert!(items.frames().is_empty());
        assert_eq!(redstone::frame_power(&world, (0, 1, 0), WEST), None);
        assert!(!strike(&mut world, &mut items, &shared, id, false), "нет такой рамки");
    }

    /// В творческом режиме ничего не выпадает.
    #[test]
    fn creative_strikes_drop_nothing() {
        let (shared, reader) = server();
        let mut world = shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut items = shared.items_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut from = 0;

        assert!(place(&mut world, &mut items, &shared, (1, 1, 0), WEST, false));
        let id = items.frames()[0].entity_id;

        use_on(&mut world, &mut items, id, Some(Stack::new(1, 1)));
        journal(&mut items, &mut from, reader);

        assert!(strike(&mut world, &mut items, &shared, id, true));
        assert_eq!(journal(&mut items, &mut from, reader), "SI");
        assert!(strike(&mut world, &mut items, &shared, id, true));
        assert_eq!(journal(&mut items, &mut from, reader), "GS");
        assert!(items.items().is_empty());
    }

    /// Рамка без опоры падает вместе с предметом: две вещи на земле.
    #[test]
    fn a_frame_that_falls_drops_itself_and_its_item() {
        let (shared, reader) = server();
        let mut world = shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut items = shared.items_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let mut from = 0;

        assert!(place(&mut world, &mut items, &shared, (1, 1, 0), WEST, false));
        let id = items.frames()[0].entity_id;

        use_on(&mut world, &mut items, id, Some(Stack::new(1, 1)));
        journal(&mut items, &mut from, reader);

        let frame = items.remove_frames(|frame| frame.entity_id == id).remove(0);

        break_frame(&mut world, &mut items, &shared, &frame, true);
        assert_eq!(journal(&mut items, &mut from, reader), "GSDD");
        assert_eq!(items.items().len(), 2);
    }
}
