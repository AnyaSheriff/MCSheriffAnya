// Хранилища: сундуки, бочки, воронки, выбрасыватели, раздатчики.
//
// У такого блока есть содержимое, которое не помещается в его состояние, —
// в игре это «блочная сущность». Здесь она называется хранилищем и живёт в
// чанке рядом с блоками: записывается и читается вместе с ним.
//
// Как устроено у оригинала, взято с minecraft.wiki (страницы Chest, Hopper,
// Dropper, Dispenser, Redstone Comparator) и сверено чёрным ящиком
// (tools/blackbox, сценарии hopper-*, dropper-pulse, comparator-chest,
// chest-open, trapped-chest). Разбор — tools/research/containers.md.

use crate::blocks;
use crate::inventory::Stack;
use crate::world::World;

/// Вид хранилища: от него зависят число ячеек, окно и поведение.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Chest,
    TrappedChest,
    Barrel,
    Hopper,
    Dropper,
    Dispenser,

    /// Проигрыватель: одна ячейка под пластинку (redstone/jukebox.rs).
    Jukebox,

    /// Кафедра: одна ячейка под книгу и открытая страница
    /// (redstone/lectern.rs).
    Lectern,

    /// Крафтер: сетка 3×3, выключенные ячейки и такты «крафтит»
    /// (redstone/crafter.rs).
    Crafter,
}

impl Kind {
    /// Какое хранилище у блока в этом состоянии. None — блок без содержимого.
    pub fn of(state: i32) -> Option<Kind> {
        Some(match blocks::block_at_state(state)? {
            "chest" => Kind::Chest,
            "trapped_chest" => Kind::TrappedChest,
            "barrel" => Kind::Barrel,
            "hopper" => Kind::Hopper,
            "dropper" => Kind::Dropper,
            "dispenser" => Kind::Dispenser,
            "jukebox" => Kind::Jukebox,
            "lectern" => Kind::Lectern,
            "crafter" => Kind::Crafter,
            _ => return None,
        })
    }

    /// Сколько ячеек у хранилища.
    pub fn size(self) -> usize {
        match self {
            Kind::Chest | Kind::TrappedChest | Kind::Barrel => 27,
            Kind::Hopper => 5,
            Kind::Dropper | Kind::Dispenser | Kind::Crafter => 9,
            Kind::Jukebox | Kind::Lectern => 1,
        }
    }

    /// Номер вида блок-сущности (реестр `minecraft:block_entity_type`,
    /// снят штатным генератором отчётов официального сервера —
    /// tools/blackbox/data/registries.json).
    pub fn block_entity_type(self) -> i32 {
        match self {
            Kind::Chest => 1,
            Kind::TrappedChest => 2,
            Kind::Dispenser => 5,
            Kind::Dropper => 6,
            Kind::Hopper => 18,
            Kind::Barrel => 27,
            Kind::Jukebox => 4,
            Kind::Lectern => 30,
            Kind::Crafter => 43,
        }
    }

    /// Номер вида для записи на диск.
    fn number(self) -> u8 {
        match self {
            Kind::Chest => 0,
            Kind::TrappedChest => 1,
            Kind::Barrel => 2,
            Kind::Hopper => 3,
            Kind::Dropper => 4,
            Kind::Dispenser => 5,
            Kind::Jukebox => 6,
            Kind::Lectern => 7,
            Kind::Crafter => 8,
        }
    }

    fn by_number(number: u8) -> Option<Kind> {
        Some(match number {
            0 => Kind::Chest,
            1 => Kind::TrappedChest,
            2 => Kind::Barrel,
            3 => Kind::Hopper,
            4 => Kind::Dropper,
            5 => Kind::Dispenser,
            6 => Kind::Jukebox,
            7 => Kind::Lectern,
            8 => Kind::Crafter,
            _ => return None,
        })
    }
}

/// Содержимое одного хранилища.
#[derive(Clone, PartialEq, Debug)]
pub struct Container {
    pub kind: Kind,

    /// Ячейки по порядку; None — пустая.
    pub slots: Vec<Option<Stack>>,

    /// Сколько тактов воронке ждать до следующего переноса (у прочих — 0).
    pub cooldown: i32,

    /// В каком такте мира воронка последний раз отработала: от этого
    /// зависит, сколько ждать воронке, получившей предмет от другой.
    pub ticked: u64,

    /// Проигрыватель: сколько его тактов прошло с начала песни, плюс один;
    /// 0 — музыка не играет (пластинки нет или песня кончилась).
    pub started: u64,

    /// Кафедра: открытая страница, с нуля.
    pub page: i32,

    /// Кафедра: сколько страниц в книге.
    pub pages: i32,

    /// Крафтер: выключенные ячейки, по биту на ячейку (бит 0 — ячейка 0).
    pub disabled: u16,

    /// Крафтер: сколько тактов ещё держится `crafting=true`
    /// (`crafting_ticks_remaining`), 0 — не крафтит.
    pub crafting: i32,
}

impl Container {
    /// Пустое хранилище.
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            slots: vec![None; kind.size()],
            cooldown: 0,
            ticked: 0,
            started: 0,
            page: 0,
            pages: 0,
            disabled: 0,
            crafting: 0,
        }
    }

    /// Выключена ли ячейка крафтера.
    pub fn is_disabled(&self, slot: usize) -> bool {
        slot < 16 && self.disabled & (1 << slot) != 0
    }

    /// Пусто ли хранилище целиком.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// Сила сигнала, которую сравнитель читает с хранилища: 0 у пустого,
    /// иначе `1 + заполненность × 14` с округлением вниз, где заполненность —
    /// среднее по ячейкам «сколько лежит / сколько влезает в стопку»
    /// (вики, Redstone Comparator, «Measure block state»).
    pub fn signal(slots: &[Option<Stack>]) -> i32 {
        if slots.is_empty() {
            return 0;
        }

        let mut fullness = 0.0f32;
        let mut any = false;

        for stack in slots.iter().flatten() {
            fullness += stack.count as f32 / blocks::stack_size(stack.item).max(1) as f32;
            any = true;
        }

        if !any {
            return 0;
        }

        fullness /= slots.len() as f32;

        (fullness * 14.0).floor() as i32 + 1
    }

    /// Записывает хранилище для файла региона: вид, место внутри чанка,
    /// ожидание воронки и непустые ячейки.
    pub fn write(&self, (x, y, z): (i32, i32, i32), out: &mut Vec<u8>) {
        out.push(self.kind.number());
        out.push(((x.rem_euclid(16) << 4) | z.rem_euclid(16)) as u8);
        out.extend_from_slice(&(y as i16).to_be_bytes());
        out.push(self.cooldown.clamp(0, 127) as u8);

        let filled: Vec<(usize, Stack)> =
            self.slots.iter().enumerate().filter_map(|(slot, stack)| stack.map(|stack| (slot, stack))).collect();

        out.push(filled.len() as u8);

        for (slot, stack) in filled {
            out.push(slot as u8);
            // Пометка особого динамита — в старшем байте номера
            // (`Stack::stored_item`).
            out.extend_from_slice(&stack.stored_item().to_be_bytes());
            out.push(stack.count.clamp(0, 127) as u8);
        }

        // Своё у проигрывателя и кафедры — после ячеек.
        match self.kind {
            Kind::Jukebox => out.push(u8::from(self.started > 0)),
            Kind::Lectern => {
                out.extend_from_slice(&(self.page.clamp(0, i16::MAX as i32) as i16).to_be_bytes());
                out.extend_from_slice(&(self.pages.clamp(0, i16::MAX as i32) as i16).to_be_bytes());
            }
            Kind::Crafter => {
                out.extend_from_slice(&self.disabled.to_be_bytes());
                out.push(self.crafting.clamp(0, 127) as u8);
            }
            _ => {}
        }
    }

    /// Читает хранилище, записанное `write`. Возвращает место (мировые X и
    /// Z по номеру чанка) и само хранилище; None — запись испорчена.
    pub fn read(bytes: &mut &[u8], chunk_x: i32, chunk_z: i32) -> Option<((i32, i32, i32), Container)> {
        let mut take = |count: usize| -> Option<&[u8]> {
            let (head, rest) = bytes.split_at_checked(count)?;
            *bytes = rest;
            Some(head)
        };

        let kind = Kind::by_number(take(1)?[0])?;
        let corner = take(1)?[0];
        let y = i16::from_be_bytes(take(2)?.try_into().ok()?) as i32;
        let cooldown = take(1)?[0] as i32;
        let filled = take(1)?[0] as usize;
        let mut container = Container::new(kind);

        container.cooldown = cooldown;

        for _ in 0..filled {
            let slot = take(1)?[0] as usize;
            let stored = Stack::from_stored(i32::from_be_bytes(take(4)?.try_into().ok()?), 0);
            let count = take(1)?[0] as i32;
            let item = stored.item;

            if blocks::stack_size(item) <= 0 {
                continue;
            }

            *container.slots.get_mut(slot)? = (count > 0).then(|| stored.with_count(count.min(blocks::stack_size(item))));
        }

        match kind {
            // Играющая песня после запуска начинается сначала. [проверить]
            // у оригинала она продолжается с `ticks_since_song_started`.
            Kind::Jukebox => container.started = u64::from(take(1)?[0] != 0),
            Kind::Lectern => {
                container.page = i16::from_be_bytes(take(2)?.try_into().ok()?) as i32;
                container.pages = i16::from_be_bytes(take(2)?.try_into().ok()?) as i32;
            }
            Kind::Crafter => {
                container.disabled = u16::from_be_bytes(take(2)?.try_into().ok()?) & 0x1ff;
                container.crafting = take(1)?[0] as i32;
            }
            _ => {}
        }

        let place = (chunk_x * 16 + (corner >> 4) as i32, y, chunk_z * 16 + (corner & 15) as i32);

        Some((place, container))
    }
}

/// Ячейки из данных блока в команде: `{Items:[{Slot:0b,id:"minecraft:stone",
/// count:5}, …]}`. Понимаются поля Slot, id и count (и прежнее Count);
/// остальные пропускаются. None — запись не разобрать.
pub fn items_from_text(text: &str) -> Option<Vec<(usize, Stack)>> {
    let text = text.trim();

    if !text.starts_with('{') || !text.ends_with('}') {
        return None;
    }

    let Some(start) = text.find("Items:[") else {
        // Кафедра и проигрыватель держат один предмет: `Book:{id:…}` и
        // `RecordItem:{id:…}`.
        for key in ["Book:{", "RecordItem:{"] {
            if let Some(at) = text.find(key) {
                let inner = &text[at + key.len()..];
                let id = inner.find("id:")?;
                let name = inner[id + 3..].trim_start().trim_start_matches('"');
                let name = &name[..name.find(['"', ',', '}'])?];
                let name = name.strip_prefix("minecraft:").unwrap_or(name);

                return Some(vec![(0, Stack::new(blocks::item_named(name)?, 1))]);
            }
        }

        // Данных хранилища нет — значит, и ячеек нет.
        return Some(Vec::new());
    };
    let list = &text[start + "Items:[".len()..];
    let end = list.find(']')?;
    let mut found = Vec::new();

    for entry in list[..end].split('}') {
        let entry = entry.trim_start_matches([',', ' ', '{']).trim();

        if entry.is_empty() {
            continue;
        }

        let (mut slot, mut item, mut count) = (None, None, 1);

        for field in entry.split(',') {
            let Some((key, value)) = field.split_once(':').map(|(k, v)| (k.trim(), v.trim())) else {
                // «minecraft:stone» внутри id разбит двоеточием — его
                // собирает ветка id ниже.
                continue;
            };
            let number = || value.trim_end_matches(['b', 'B', 's', 'S']).parse::<i32>().ok();

            match key {
                "Slot" => slot = number(),
                "count" | "Count" => count = number()?,
                "id" => {
                    let name = field.split_once(':')?.1.trim().trim_matches('"');
                    let name = name.strip_prefix("minecraft:").unwrap_or(name);

                    item = blocks::item_named(name);
                }
                _ => {}
            }
        }

        let (slot, item) = (usize::try_from(slot?).ok()?, item?);

        if count > 0 {
            found.push((slot, Stack::new(item, count.min(blocks::stack_size(item)))));
        }
    }

    Some(found)
}

// ---------------------------------------------------------------------------
// Хранилища в мире: половины сундука, открытие и закрытие, окна
// ---------------------------------------------------------------------------

/// Место блока.
pub type Pos = (i32, i32, i32);

/// Сторона света по имени свойства `facing`.
pub fn facing_offset(facing: &str) -> Option<(i32, i32, i32)> {
    Some(match facing {
        "north" => (0, 0, -1),
        "south" => (0, 0, 1),
        "west" => (-1, 0, 0),
        "east" => (1, 0, 0),
        "up" => (0, 1, 0),
        "down" => (0, -1, 0),
        _ => return None,
    })
}

/// Значение свойства блока.
fn property(state: i32, name: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, name)
}

fn add((x, y, z): Pos, (dx, dy, dz): (i32, i32, i32)) -> Pos {
    (x + dx, y + dy, z + dz)
}

/// Поворот стороны по часовой стрелке (если смотреть сверху).
fn clockwise((dx, dy, dz): (i32, i32, i32)) -> (i32, i32, i32) {
    (-dz, dy, dx)
}

/// Места хранилища в порядке окна: у одиночного — одно, у большого
/// сундука — два, и первой идёт половина `type=right` (снято чёрным ящиком,
/// сценарий chest-slots: у сундука, смотрящего на север, правая половина —
/// восточная, и её ячейки в окне первые).
pub fn halves(world: &World, pos: Pos) -> Vec<Pos> {
    let state = world.get_block(pos.0, pos.1, pos.2);

    let Some(kind) = Kind::of(state) else {
        return Vec::new();
    };

    if !matches!(kind, Kind::Chest | Kind::TrappedChest) {
        return vec![pos];
    }

    if let Some(other) = partner(world, pos, state) {
        return match property(state, "type") {
            Some("right") => vec![pos, other],
            _ => vec![other, pos],
        };
    }

    vec![pos]
}

/// Вторая половина большого сундука, если она на месте.
fn partner(world: &World, pos: Pos, state: i32) -> Option<Pos> {
    let facing = facing_offset(property(state, "facing")?)?;
    let side = match property(state, "type")? {
        // «Левая» половина смотрит на соседа справа от себя (по часовой от
        // стороны, куда смотрит сундук), «правая» — наоборот.
        "left" => clockwise(facing),
        "right" => {
            let (dx, dy, dz) = clockwise(facing);
            (-dx, -dy, -dz)
        }
        _ => return None,
    };
    let other = add(pos, side);
    let other_state = world.get_block(other.0, other.1, other.2);

    let paired = Kind::of(other_state) == Kind::of(state)
        && property(other_state, "facing") == property(state, "facing")
        && property(other_state, "type") != property(state, "type")
        && property(other_state, "type") != Some("single");

    paired.then_some(other)
}

/// Можно ли открыть хранилище: сундук не открывается, если над ним (над
/// любой половиной) проводящий блок; бочка и прочие — всегда
/// (вики, Chest и Barrel).
pub fn can_open(world: &World, halves: &[Pos]) -> bool {
    halves.iter().all(|&(x, y, z)| {
        let state = world.get_block(x, y, z);

        !matches!(Kind::of(state), Some(Kind::Chest | Kind::TrappedChest)) || !blocks::conductive(world.get_block(x, y + 1, z))
    })
}

/// Номер вида окна (реестр `minecraft:menu`) и ключ заголовка.
pub fn menu(kind: Kind, halves: usize) -> (i32, &'static str) {
    match kind {
        Kind::Chest | Kind::TrappedChest if halves > 1 => (5, "container.chestDouble"),
        Kind::Chest | Kind::TrappedChest => (2, "container.chest"),
        Kind::Barrel => (2, "container.barrel"),
        Kind::Hopper => (16, "container.hopper"),
        Kind::Dropper => (6, "container.dropper"),
        Kind::Dispenser => (6, "container.dispenser"),
        Kind::Lectern => (17, "container.lectern"),
        Kind::Crafter => (7, "container.crafter"),
        // Своего окна у проигрывателя нет: сюда не доходит.
        Kind::Jukebox => (0, "container.jukebox"),
    }
}

/// Содержимое окна: ячейки половин подряд.
pub fn window_slots(world: &World, halves: &[Pos]) -> Vec<Option<Stack>> {
    halves
        .iter()
        .flat_map(|&(x, y, z)| world.container(x, y, z).map(|c| c.slots.clone()).unwrap_or_default())
        .collect()
}

/// Кладёт содержимое окна обратно по половинам. Сменившееся хранилище
/// шлёт обновление сравнителям.
pub fn store_window_slots(world: &mut World, halves: &[Pos], slots: &[Option<Stack>]) {
    let mut from = 0;

    for &(x, y, z) in halves {
        let Some(size) = world.container(x, y, z).map(|c| c.slots.len()) else {
            continue;
        };
        let part = slots.get(from..from + size).map(<[_]>::to_vec);

        from += size;

        if let Some(part) = part {
            world.with_container(x, y, z, |container| container.slots = part);
        }
    }

    after_change(world);
}

/// Середина хранилища — оттуда слышно открытие: у большого сундука между
/// половинами, у бочки — у крышки (на полблока в сторону `facing`).
fn sound_point(world: &World, halves: &[Pos]) -> (f64, f64, f64) {
    let count = halves.len().max(1) as f64;
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);

    for &(hx, hy, hz) in halves {
        x += hx as f64 + 0.5;
        y += hy as f64 + 0.5;
        z += hz as f64 + 0.5;
    }

    let (mut x, mut y, mut z) = (x / count, y / count, z / count);

    if let Some(&(bx, by, bz)) = halves.first() {
        let state = world.get_block(bx, by, bz);

        if Kind::of(state) == Some(Kind::Barrel)
            && let Some((dx, dy, dz)) = property(state, "facing").and_then(facing_offset)
        {
            x += dx as f64 * 0.5;
            y += dy as f64 * 0.5;
            z += dz as f64 * 0.5;
        }
    }

    (x, y, z)
}

/// Высота звука сундука и бочки: от 0,9 до 1,0 (вики, Chest).
fn lid_pitch(world: &mut World) -> f32 {
    world.random_unit() * 0.1 + 0.9
}

/// Звуки открытия и закрытия по виду хранилища.
fn lid_sounds(kind: Kind) -> Option<(&'static str, &'static str)> {
    match kind {
        Kind::Chest | Kind::TrappedChest => Some(("minecraft:block.chest.open", "minecraft:block.chest.close")),
        Kind::Barrel => Some(("minecraft:block.barrel.open", "minecraft:block.barrel.close")),
        _ => None,
    }
}

/// Сколько тактов между повторами крышки, пока сундук открыт (вики,
/// Block actions: «every 5 ticks»; снято чёрным ящиком, chest-open).
pub const LID_RECHECK: u64 = 5;

/// Игрок открыл хранилище — первая часть: +1 смотрящий у каждой половины,
/// первому — звук, бочка молча становится открытой (клиентам о ней скажут
/// повтор блока и общая рассылка — в этом порядке, как у оригинала).
/// Возвращает, был ли это первый смотрящий.
pub fn open_begin(world: &mut World, halves: &[Pos]) -> bool {
    let Some(&(x, y, z)) = halves.first() else {
        return false;
    };
    let Some(kind) = Kind::of(world.get_block(x, y, z)) else {
        return false;
    };

    let first = world.viewers(x, y, z) == 0;

    for &(hx, hy, hz) in halves {
        world.add_viewers(hx, hy, hz, 1);
    }

    if first && let Some((sound, _)) = lid_sounds(kind) {
        let pitch = lid_pitch(world);
        let at = sound_point(world, halves);

        world.note_sound_at_block(at, sound, 0.5, pitch);
    }

    if first && kind == Kind::Barrel {
        set_barrel(world, halves, true, false);
    }

    first
}

/// Вторая часть открытия — после повтора блока: у бочки общая рассылка
/// «открыта», у сундука — крышка и повтор раз в 5 тактов, у ловушки —
/// соседи узнают о смене силы.
pub fn open_finish(world: &mut World, halves: &[Pos], first: bool) {
    let Some(&(x, y, z)) = halves.first() else {
        return;
    };
    let Some(kind) = Kind::of(world.get_block(x, y, z)) else {
        return;
    };

    if kind == Kind::Barrel {
        if first {
            for &(hx, hy, hz) in halves {
                world.announce_change(hx, hy, hz, barrel_with(world.get_block(hx, hy, hz), false));
            }
        }

        return;
    }

    after_viewers(world, kind, halves, first);
}

/// Состояние бочки с `open`, равным `open`.
fn barrel_with(state: i32, open: bool) -> i32 {
    blocks::orientation_of(state)
        .and_then(|o| o.with(state, "open", if open { "true" } else { "false" }))
        .unwrap_or(state)
}

/// Открывает или закрывает бочку: молча или с общей рассылкой.
fn set_barrel(world: &mut World, halves: &[Pos], open: bool, loud: bool) {
    for &(hx, hy, hz) in halves {
        let next = barrel_with(world.get_block(hx, hy, hz), open);

        if loud {
            world.set_block(hx, hy, hz, next);
        } else {
            world.set_block_silently(hx, hy, hz, next);
        }
    }
}

/// Игрок закрыл хранилище.
pub fn close(world: &mut World, halves: &[Pos]) {
    let Some(&(x, y, z)) = halves.first() else {
        return;
    };
    let Some(kind) = Kind::of(world.get_block(x, y, z)) else {
        return;
    };

    let mut last = false;

    for &(hx, hy, hz) in halves {
        last = world.add_viewers(hx, hy, hz, -1) == 0;
    }

    if last && let Some((_, sound)) = lid_sounds(kind) {
        let pitch = lid_pitch(world);
        let at = sound_point(world, halves);

        world.note_sound_at_block(at, sound, 0.5, pitch);
    }

    after_viewers(world, kind, halves, last);
}

/// Что следует за сменой числа смотрящих.
fn after_viewers(world: &mut World, kind: Kind, halves: &[Pos], edge: bool) {
    match kind {
        Kind::Chest | Kind::TrappedChest => {
            for &(hx, hy, hz) in halves {
                queue_lid(world, (hx, hy, hz));

                if world.viewers(hx, hy, hz) > 0 {
                    world.schedule_block_once(hx, hy, hz, LID_RECHECK);
                }

                // Сила ловушки — число смотрящих: соседи и блок под ней
                // должны узнать, что она сменилась.
                if kind == Kind::TrappedChest {
                    world.note_changed(hx, hy, hz);
                }
            }
        }
        Kind::Barrel if edge => {
            let open = halves.iter().any(|&(hx, hy, hz)| world.viewers(hx, hy, hz) > 0);

            set_barrel(world, halves, open, true);
        }
        _ => {}
    }
}

/// Такт сундука: повторить крышку клиентам и, пока открыт, завести
/// следующий повтор.
pub fn recheck_lid(world: &mut World, (x, y, z): Pos) {
    if !matches!(Kind::of(world.get_block(x, y, z)), Some(Kind::Chest | Kind::TrappedChest)) {
        return;
    }

    queue_lid(world, (x, y, z));

    if world.viewers(x, y, z) > 0 {
        world.schedule_block_once(x, y, z, LID_RECHECK);
    }
}

/// Крышка сундука для клиентов — блочное событие 1 с числом смотрящих: у
/// оригинала оно уходит в фазе блочных событий.
fn queue_lid(world: &mut World, (x, y, z): Pos) {
    let viewers = world.viewers(x, y, z).min(255) as u8;

    world.queue_block_event(x, y, z, 1, viewers);
}

/// Блочное событие крышки исполнено: клиентам — Block Action 1 с числом
/// смотрящих, каким оно было, когда событие завели.
pub fn tell_lid(world: &mut World, (x, y, z): Pos, viewers: u8) {
    let state = world.get_block(x, y, z);

    if !matches!(Kind::of(state), Some(Kind::Chest | Kind::TrappedChest)) {
        return;
    }

    let Some(block) = blocks::block_at_state(state).and_then(blocks::block_id) else {
        return;
    };

    world.note_action(crate::world::BlockAction { x, y, z, action: 1, param: viewers, block });
}

/// Сила, которую сундук-ловушка отдаёт: число смотрящих, не больше 15.
pub fn trapped_power(world: &World, (x, y, z): Pos) -> u8 {
    if Kind::of(world.get_block(x, y, z)) != Some(Kind::TrappedChest) {
        return 0;
    }

    world.viewers(x, y, z).min(15) as u8
}

/// Разослать обновления сравнителям от хранилищ, чьё содержимое сменилось.
/// Проигрыватель, в который пластинку положили (или из которого вынули)
/// воронкой, выбрасывателем или командой, сперва начинает или бросает
/// играть.
pub fn after_change(world: &mut World) {
    for pos in world.take_contents_changed() {
        crate::redstone::jukebox_contents_changed(world, pos);
        crate::redstone::comparator_update(world, pos);
    }
}

// ---------------------------------------------------------------------------
// Перенос предметов
// ---------------------------------------------------------------------------

/// Кладёт один предмет в хранилище (все его половины подряд): в первую
/// слева ячейку, где есть место, — пустую или с тем же предметом.
/// Возвращает, куда положено (место половины), или None — некуда.
fn insert_one(world: &mut World, halves: &[Pos], like: Stack) -> Option<Pos> {
    for &(x, y, z) in halves {
        if !accepts(world, (x, y, z), like.item) {
            continue;
        }

        let container = world.container(x, y, z)?;
        let slot = if container.kind == Kind::Crafter {
            crate::redstone::crafter::slot_for(container, like.item)
        } else {
            container.slots.iter().position(|slot| match slot {
                None => true,
                Some(stack) => stack.stacks_with(&like) && stack.count < blocks::stack_size(like.item),
            })
        };

        if let Some(slot) = slot {
            world.with_container(x, y, z, |container| {
                let stack = &mut container.slots[slot];

                *stack = Some(match stack {
                    Some(there) => like.with_count(there.count + 1),
                    None => like.with_count(1),
                });
            });

            return Some((x, y, z));
        }
    }

    None
}

/// Кладёт стопку в хранилище перед крафтером (все его половины) по одной
/// штуке, как выбрасыватель; возвращает, что не влезло. Воронка, получившая
/// первый предмет в пустое нутро, ждёт, как от выбрасывателя.
pub fn insert_stack(world: &mut World, halves: &[Pos], stack: Stack) -> Option<Stack> {
    let was_empty = halves.iter().all(|&half| is_empty_at(world, half));
    let mut left = stack.count;

    while left > 0 {
        let Some(into) = insert_one(world, halves, stack) else {
            break;
        };

        cool_receiver(world, into, was_empty);
        left -= 1;
    }

    (left > 0).then(|| stack.with_count(left))
}

/// Берёт ли хранилище этот предмет снаружи — от воронки, выбрасывателя.
/// Проигрыватель — только пластинку и только пустой (вики, Hopper: «A
/// jukebox accepts a music disc if it is empty»), кафедра — ничего
/// (вики, Hopper: «Lectern — no interaction»).
fn accepts(world: &World, (x, y, z): Pos, item: i32) -> bool {
    match world.container(x, y, z).map(|c| c.kind) {
        Some(Kind::Jukebox) => crate::redstone::jukebox_song(item).is_some() && is_empty_at(world, (x, y, z)),
        Some(Kind::Lectern) => false,
        _ => true,
    }
}

/// Можно ли вытянуть предмет из хранилища снизу: из кафедры — нельзя.
fn gives(world: &World, (x, y, z): Pos) -> bool {
    world.container(x, y, z).is_some_and(|c| c.kind != Kind::Lectern)
}

/// Убирает один предмет из ячейки.
fn remove_one(world: &mut World, (x, y, z): Pos, slot: usize) {
    world.with_container(x, y, z, |container| {
        if let Some(stack) = container.slots.get_mut(slot) {
            *stack = stack.and_then(|there| (there.count > 1).then(|| there.with_count(there.count - 1)));
        }
    });
}

/// Пустое ли хранилище в этом месте.
fn is_empty_at(world: &World, (x, y, z): Pos) -> bool {
    world.container(x, y, z).is_none_or(Container::is_empty)
}

/// Воронке, получившей предмет в пустое нутро, — своё ожидание: 8 тактов,
/// а если она в этом такте уже отработала — 7. Так у оригинала выходит
/// «всегда 7» (вики, Hopper: «when an item is pushed into an empty hopper,
/// the cooldown of that hopper lasts 7 game ticks»), с какой бы стороны
/// очереди она ни стояла. Сверено сценарием hopper-clock.
fn cool_receiver(world: &mut World, (x, y, z): Pos, was_empty: bool) {
    if !was_empty || Kind::of(world.get_block(x, y, z)) != Some(Kind::Hopper) {
        return;
    }

    let now = world.tick();

    world.with_container(x, y, z, |hopper| {
        // Ожидание, заданное командой больше восьми, не перебивается.
        if hopper.cooldown <= HOPPER_COOLDOWN {
            hopper.cooldown = HOPPER_COOLDOWN - i32::from(hopper.ticked >= now);
        }
    });
}

/// Ожидание воронки после переноса.
pub const HOPPER_COOLDOWN: i32 = 8;

/// Такт всех воронок мира — фаза блочных сущностей, после сущностей.
/// `take_items` отдаёт предметы, лежащие в коробке воронки (и что из них
/// влезло, возвращает обратно).
pub fn tick_hoppers(world: &mut World, items: &mut crate::items::Items) {
    for pos in world.containers_in_order() {
        match world.container(pos.0, pos.1, pos.2).map(|c| c.kind) {
            Some(Kind::Hopper) => tick_hopper(world, items, pos),
            // Проигрыватель — тоже блок-сущность со своим тактом: песня
            // кончается в нём.
            Some(Kind::Jukebox) => crate::redstone::tick_jukebox(world, pos),
            // Крафтер отсчитывает такты `crafting=true`.
            Some(Kind::Crafter) => crate::redstone::crafter::tick(world, pos),
            _ => {}
        }
    }

    after_change(world);
}

fn tick_hopper(world: &mut World, items: &mut crate::items::Items, pos: Pos) {
    let now = world.tick();
    let (x, y, z) = pos;

    let ready = world
        .with_container(x, y, z, |hopper| {
            hopper.cooldown -= 1;
            hopper.ticked = now;

            if hopper.cooldown > 0 {
                return false;
            }

            hopper.cooldown = 0;
            true
        })
        .unwrap_or(false);

    let state = world.get_block(x, y, z);

    if !ready || property(state, "enabled") == Some("false") {
        return;
    }

    let mut moved = false;

    if !is_empty_at(world, pos) {
        moved = push(world, items, pos, state);
    }

    if !is_full_at(world, pos) {
        moved |= pull(world, items, pos);
    }

    if moved {
        world.with_container(x, y, z, |hopper| hopper.cooldown = HOPPER_COOLDOWN);
    }
}

/// Полна ли воронка: ни в одну ячейку ничего не добавить.
fn is_full_at(world: &World, (x, y, z): Pos) -> bool {
    world.container(x, y, z).is_none_or(|c| {
        c.slots.iter().all(|slot| slot.is_some_and(|stack| stack.count >= blocks::stack_size(stack.item)))
    })
}

/// Воронка кладёт один предмет в хранилище перед выходом: берётся первая
/// ячейка воронки, чей предмет влезает.
fn push(world: &mut World, items: &mut crate::items::Items, pos: Pos, state: i32) -> bool {
    let Some(facing) = property(state, "facing").and_then(facing_offset) else {
        return false;
    };
    let target = add(pos, facing);
    let target_halves = halves(world, target);

    if target_halves.is_empty() {
        return push_into_cart(world, items, pos, target);
    }

    let was_empty = target_halves.iter().all(|&half| is_empty_at(world, half));
    let slots = world.container(pos.0, pos.1, pos.2).map(|c| c.slots.clone()).unwrap_or_default();

    for (slot, stack) in slots.iter().enumerate() {
        let Some(stack) = stack else {
            continue;
        };

        if let Some(into) = insert_one(world, &target_halves, *stack) {
            remove_one(world, pos, slot);
            cool_receiver(world, into, was_empty);
            return true;
        }
    }

    false
}

/// Воронка тянет один предмет из хранилища над собой или, если его нет,
/// подбирает лежащие предметы.
fn pull(world: &mut World, items: &mut crate::items::Items, pos: Pos) -> bool {
    let above = add(pos, (0, 1, 0));
    let source = halves(world, above);

    if !source.is_empty() {
        for &half in &source {
            if !gives(world, half) {
                continue;
            }

            let slots = world.container(half.0, half.1, half.2).map(|c| c.slots.clone()).unwrap_or_default();

            for (slot, stack) in slots.iter().enumerate() {
                let Some(stack) = stack else {
                    continue;
                };

                if insert_one(world, &[pos], *stack).is_some() {
                    remove_one(world, half, slot);
                    return true;
                }
            }
        }

        return false;
    }

    // Хранилища-блока нет — тянет из вагонетки с сундуком или воронкой в
    // клетке над собой (вики, Hopper: «minecart with chest or hopper»).
    if let Some(pulled) = pull_from_cart(world, items, pos, above) {
        return pulled;
    }

    // Полный блок сверху подбирать не даёт (вики, Hopper).
    if blocks::full_cube(world.get_block(above.0, above.1, above.2)) {
        return false;
    }

    collect(world, items, pos)
}

/// Первая по появлению вагонетка-хранилище, чей ящик задевает клетку
/// `cell`. [проверить]: у оригинала — случайная из задевающих.
fn cart_in(items: &mut crate::items::Items, (x, y, z): Pos) -> Option<&mut crate::minecart::Minecart> {
    let half = crate::minecart::WIDTH / 2.0;

    items.carts_mut().iter_mut().find(|cart| {
        !cart.slots.is_empty()
            && cart.x + half > x as f64
            && cart.x - half < x as f64 + 1.0
            && cart.z + half > z as f64
            && cart.z - half < z as f64 + 1.0
            && cart.y + crate::minecart::HEIGHT > y as f64
            && cart.y < y as f64 + 1.0
    })
}

/// Ячейка, куда влезет ещё один такой предмет.
fn room_for(slots: &[Option<Stack>], like: &Stack) -> Option<usize> {
    slots.iter().position(|slot| match slot {
        None => true,
        Some(stack) => stack.stacks_with(like) && stack.count < blocks::stack_size(like.item),
    })
}

/// Воронка кладёт один предмет в вагонетку-хранилище в клетке `target`.
fn push_into_cart(world: &mut World, items: &mut crate::items::Items, pos: Pos, target: Pos) -> bool {
    let slots = world.container(pos.0, pos.1, pos.2).map(|c| c.slots.clone()).unwrap_or_default();

    let Some(cart) = cart_in(items, target) else {
        return false;
    };

    for (slot, stack) in slots.iter().enumerate() {
        let Some(stack) = stack else {
            continue;
        };

        if let Some(into) = room_for(&cart.slots, stack) {
            cart.slots[into] = Some(stack.with_count(cart.slots[into].map_or(0, |there| there.count) + 1));
            remove_one(world, pos, slot);
            return true;
        }
    }

    false
}

/// Воронка тянет один предмет из вагонетки-хранилища в клетке `above`.
/// None — вагонетки там нет.
fn pull_from_cart(world: &mut World, items: &mut crate::items::Items, pos: Pos, above: Pos) -> Option<bool> {
    let cart = cart_in(items, above)?;

    for slot in 0..cart.slots.len() {
        let Some(stack) = cart.slots[slot] else {
            continue;
        };

        if insert_one(world, &[pos], stack).is_some() {
            cart.slots[slot] = (stack.count > 1).then(|| stack.with_count(stack.count - 1));
            return Some(true);
        }
    }

    Some(false)
}

/// Подбор предметов, лежащих в чаше воронки и в блоке над ней. Первая
/// найденная сущность забирается целиком, сколько влезет.
fn collect(world: &mut World, items: &mut crate::items::Items, (x, y, z): Pos) -> bool {
    let inside = |at: (f64, f64, f64)| {
        at.0 >= x as f64
            && at.0 < x as f64 + 1.0
            && at.2 >= z as f64
            && at.2 < z as f64 + 1.0
            && at.1 >= y as f64 + 11.0 / 16.0
            && at.1 < y as f64 + 2.0
    };

    let Some(found) = items.first_inside(&inside) else {
        return false;
    };

    let mut taken = 0;

    while taken < found.count {
        if insert_one(world, &[(x, y, z)], found).is_none() {
            break;
        }

        taken += 1;
    }

    if taken == 0 {
        return false;
    }

    items.take_from_first_inside(&inside, taken);
    true
}

/// Вагонетка с воронкой (`slots` — её ячейки, `at` — середина низа ящика)
/// тянет один предмет за такт из хранилища, чей низ выше низа вагонетки
/// больше чем на 0,5 и не больше чем на 1,5 блока (вики, Minecart with
/// Hopper: «pulls items from a container with its bottom face 0.5
/// (exclusive) to 1.5 (inclusive) blocks directly above … one item every
/// game tick»; зонд minecart-hopper: по предмету за такт с первого такта).
/// Хранилища нет — подбирает лежащие рядом предметы. [проверить]: область
/// подбора.
pub fn cart_pull(world: &mut World, items: &mut crate::items::Items, slots: &mut [Option<Stack>], at: (f64, f64, f64)) -> bool {
    let above = (at.0.floor() as i32, (at.1 + 1.5).floor() as i32, at.2.floor() as i32);
    let source = halves(world, above);

    let fits = |slots: &[Option<Stack>], like: Stack| {
        slots.iter().position(|slot| match slot {
            None => true,
            Some(stack) => stack.stacks_with(&like) && stack.count < blocks::stack_size(like.item),
        })
    };

    let put = |slots: &mut [Option<Stack>], at: usize, like: Stack| {
        slots[at] = Some(match slots[at] {
            Some(there) => like.with_count(there.count + 1),
            None => like.with_count(1),
        });
    };

    if !source.is_empty() {
        for &half in &source {
            if !gives(world, half) {
                continue;
            }

            let there = world.container(half.0, half.1, half.2).map(|c| c.slots.clone()).unwrap_or_default();

            for (slot, stack) in there.iter().enumerate() {
                let Some(stack) = stack else {
                    continue;
                };

                if let Some(into) = fits(slots, *stack) {
                    put(slots, into, *stack);
                    remove_one(world, half, slot);
                    return true;
                }
            }
        }

        return false;
    }

    // Лежащие предметы: в ящике вагонетки, раздутом на 0,25 по бокам.
    let inside = |item: (f64, f64, f64)| {
        (item.0 - at.0).abs() < crate::minecart::WIDTH / 2.0 + 0.25
            && (item.2 - at.2).abs() < crate::minecart::WIDTH / 2.0 + 0.25
            && item.1 >= at.1
            && item.1 < at.1 + crate::minecart::HEIGHT
    };

    let Some(found) = items.first_inside(&inside) else {
        return false;
    };

    let mut taken = 0;

    while taken < found.count {
        let Some(into) = fits(slots, found) else {
            break;
        };

        put(slots, into, found);
        taken += 1;
    }

    if taken == 0 {
        return false;
    }

    items.take_from_first_inside(&inside, taken);
    true
}

// ---------------------------------------------------------------------------
// Выбрасыватель и раздатчик
// ---------------------------------------------------------------------------

/// Задержка выбрасывателя и раздатчика после фронта сигнала (вики, Dropper).
pub const DISPENSE_DELAY: u64 = 4;

/// Номер стороны для дыма (World Event 2000): 0 низ, 1 верх, 2 север,
/// 3 юг, 4 запад, 5 восток.
pub fn smoke_direction(facing: &str) -> i32 {
    match facing {
        "down" => 0,
        "up" => 1,
        "north" => 2,
        "south" => 3,
        "west" => 4,
        _ => 5,
    }
}

/// Такт выбрасывателя или раздатчика: случайная непустая ячейка — и
/// предмет из неё уходит в хранилище перед ним, выпадает или (у раздатчика)
/// применяется.
pub fn dispense(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let state = world.get_block(x, y, z);
    let Some(kind) = Kind::of(state) else {
        return;
    };
    let Some(facing_name) = property(state, "facing") else {
        return;
    };
    let Some(facing) = facing_offset(facing_name) else {
        return;
    };

    let filled: Vec<usize> = world
        .container(x, y, z)
        .map(|c| c.slots.iter().enumerate().filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect())
        .unwrap_or_default();

    // Пусто — щелчок повыше (вики: «a slightly higher-pitched click»).
    if filled.is_empty() {
        world.note_level_event(x, y, z, 1001, 0);
        return;
    }

    let pick = ((world.random_unit() * filled.len() as f32) as usize).min(filled.len() - 1);
    let slot = filled[pick];
    let Some(stack) = world.container(x, y, z).and_then(|c| c.slots[slot]) else {
        return;
    };
    let front = add(pos, facing);

    if kind == Kind::Dropper {
        let target = halves(world, front);

        // Перед ним хранилище — кладёт в него одну штуку, молча; не влезло —
        // не срабатывает вовсе.
        if !target.is_empty() {
            if !can_open_for_dropper(world, &target) {
                return;
            }

            let was_empty = target.iter().all(|&half| is_empty_at(world, half));

            if let Some(into) = insert_one(world, &target, stack) {
                remove_one(world, pos, slot);
                cool_receiver(world, into, was_empty);
            }

            after_change(world);
            return;
        }
    }

    if kind == Kind::Dispenser
        && let Some(done) = dispense_special(world, pos, facing, slot, stack)
    {
        // Удачное действие раздатчика — тот же щелчок и дым, что у выброса
        // (чёрный ящик, dispenser-water: после звука ведра — 1000 и 2000);
        // неудачное — щелчок неудачи.
        if done {
            world.note_level_event(x, y, z, 1000, 0);
            world.note_level_event(x, y, z, 2000, smoke_direction(facing_name));
        } else {
            world.note_level_event(x, y, z, 1001, 0);
        }

        after_change(world);
        return;
    }

    remove_one(world, pos, slot);
    throw_forward(world, pos, facing, stack.with_count(1), DROPPER_SPREAD);
    world.note_level_event(x, y, z, 1000, 0);
    world.note_level_event(x, y, z, 2000, smoke_direction(facing_name));
    after_change(world);
}

/// В заблокированный сундук выбрасыватель не кладёт (вики, Dropper).
fn can_open_for_dropper(world: &World, target: &[Pos]) -> bool {
    can_open(world, target)
}

/// Разброс скорости вылета у выбрасывателя и раздатчика.
const DROPPER_SPREAD: f64 = 0.0075 * 6.0;

/// Выброс предмета: из середины грани на 0,7 блока вперёд, со скоростью
/// около 0,2–0,3 по направлению и 0,2 вверх, с небольшим разбросом.
/// Снято чёрным ящиком (dropper-pulse: предмет у оригинала появился в
/// (1.2, 0.34, 3.5) от выбрасывателя, смотрящего на восток, со скоростью
/// около (0.20, 0.15, 0.02)).
pub fn throw_forward(world: &mut World, (x, y, z): Pos, (dx, dy, dz): (i32, i32, i32), stack: Stack, spread: f64) {
    let mut at = (
        x as f64 + 0.5 + 0.7 * dx as f64,
        y as f64 + 0.5 + 0.7 * dy as f64,
        z as f64 + 0.5 + 0.7 * dz as f64,
    );

    // Сбоку предмет вылетает чуть ниже середины, вверх и вниз — от середины.
    if dy == 0 {
        at.1 -= 0.15625;
    } else {
        at.1 -= 0.125;
    }

    let speed = world.random_unit() as f64 * 0.1 + 0.2;
    let mut spread = || (world.random_unit() as f64 - world.random_unit() as f64) * spread;
    let velocity = (
        dx as f64 * speed + spread(),
        dy as f64 * speed + spread() + 0.2,
        dz as f64 * speed + spread(),
    );

    world.eject(at, velocity, stack);
}

/// Особые действия раздатчика, которые у нас есть: вёдра, огниво и динамит.
/// Some(true) — применено, Some(false) — не вышло (щелчок неудачи, предмет
/// остаётся), None — предмет не особый, его надо выбросить.
fn dispense_special(world: &mut World, pos: Pos, facing: (i32, i32, i32), slot: usize, stack: Stack) -> Option<bool> {
    let name = blocks::item_name(stack.item).unwrap_or("");
    let front = add(pos, facing);
    let there = world.get_block(front.0, front.1, front.2);
    let (x, y, z) = pos;

    let replace_item = |world: &mut World, item: Option<&str>| {
        world.with_container(x, y, z, |container| {
            container.slots[slot] = item.and_then(blocks::item_named).map(|item| Stack::new(item, 1));
        });
    };

    match name {
        "water_bucket" | "lava_bucket" => {
            let fluid = if name == "water_bucket" { "water" } else { "lava" };

            if !blocks::replaceable(there) && crate::fluids::fluid_at(there).is_none() {
                return None;
            }

            if fluid == "water" && world.is_nether() {
                replace_item(world, Some("bucket"));
                return Some(true);
            }

            let state = blocks::state_by_name(fluid)?;

            world.set_block(front.0, front.1, front.2, state);
            world.note_sound(front.0, front.1, front.2, if fluid == "water" { "minecraft:item.bucket.empty" } else { "minecraft:item.bucket.empty_lava" }, 1.0, 1.0);
            replace_item(world, Some("bucket"));
            Some(true)
        }
        "bucket" => {
            let (kind, level) = crate::fluids::fluid_at(there)?;

            if level != 0 {
                return None;
            }

            // Звука наполнения у раздатчика нет — только его щелчок и дым
            // (чёрный ящик, dispenser-water).
            let filled = match kind {
                crate::fluids::Kind::Water => "water_bucket",
                crate::fluids::Kind::Lava => "lava_bucket",
            };

            world.set_block(front.0, front.1, front.2, crate::world::AIR);

            if stack.count == 1 {
                replace_item(world, Some(filled));
            } else {
                remove_one(world, pos, slot);

                if let Some(item) = blocks::item_named(filled) {
                    let placed = insert_one(world, &[pos], Stack::new(item, 1));

                    if placed.is_none() {
                        throw_forward(world, pos, facing, Stack::new(item, 1), DROPPER_SPREAD);
                    }
                }
            }

            Some(true)
        }
        // Динамит — подожжённым в блок перед собой (вики, Dispenser:
        // «Summons a TNT entity in the block the dispenser is facing»), со
        // щелчком и дымом, как у выброса. [проверить] порядок звуков
        // (tnt-dispenser).
        "tnt" => {
            crate::tnt::dispense(world, front, stack.tnt);
            remove_one(world, pos, slot);
            Some(true)
        }
        "flint_and_steel" => {
            // Перед ним динамит — поджигает (вики, Dispenser: «If the
            // dispenser is facing TNT, it ignites the TNT»).
            if crate::tnt::ignite(world, front) {
                return Some(true);
            }

            if there == crate::world::AIR
                && let Some(fire) = blocks::state_by_name("fire")
            {
                world.set_block(front.0, front.1, front.2, fire);
                return Some(true);
            }

            // Поджигать нечего: щелчок неудачи, огниво остаётся.
            Some(false)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Что читает сравнитель
// ---------------------------------------------------------------------------

/// Сила, которую сравнитель читает с блока в этом месте, если блок из тех,
/// что он читает. None — блок не читается (сравнитель берёт обычный сигнал).
pub fn comparator_reading(world: &World, pos: Pos) -> Option<u8> {
    let (x, y, z) = pos;
    let state = world.get_block(x, y, z);

    if let Some(kind) = Kind::of(state) {
        match kind {
            Kind::Jukebox => return Some(crate::redstone::jukebox_reading(world, pos)),
            Kind::Lectern => return Some(crate::redstone::lectern_reading(world, pos)),
            Kind::Crafter => return Some(crate::redstone::crafter::reading(world, pos)),
            _ => {}
        }

        let parts = halves(world, pos);

        // Сундук, который нельзя открыть, читается как пустой (Java).
        if matches!(kind, Kind::Chest | Kind::TrappedChest) && !can_open(world, &parts) {
            return Some(0);
        }

        return Some(Container::signal(&window_slots(world, &parts)) as u8);
    }

    let name = blocks::block_at_state(state)?;
    let number = |property_name: &str| property(state, property_name).and_then(|v| v.parse::<u8>().ok());

    Some(match name {
        "cake" => (7 - number("bites")?.min(7)) * 2,
        "water_cauldron" | "powder_snow_cauldron" => number("level")?,
        "lava_cauldron" => 3,
        "composter" => number("level")?,
        "end_portal_frame" => {
            if property(state, "eye") == Some("true") {
                15
            } else {
                0
            }
        }
        "respawn_anchor" => (number("charges")? as u32 * 15 / 4) as u8,
        "beehive" | "bee_nest" => number("honey_level")?,
        _ if name.ends_with("copper_bulb") => {
            if property(state, "lit") == Some("true") {
                15
            } else {
                0
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Хранилище переживает запись и чтение.
    #[test]
    fn containers_survive_the_disk() {
        let stone = blocks::item_named("stone").expect("камень");
        let pearl = blocks::item_named("ender_pearl").expect("жемчуг");
        let mut hopper = Container::new(Kind::Hopper);

        hopper.slots[0] = Some(Stack::new(stone, 64));
        hopper.slots[4] = Some(Stack::new(pearl, 3));
        hopper.cooldown = 7;

        let mut bytes = Vec::new();
        hopper.write((-17, -60, 33), &mut bytes);

        let mut reading = bytes.as_slice();
        let (place, read) = Container::read(&mut reading, -2, 2).expect("читается");

        assert_eq!(place, (-17, -60, 33));
        assert_eq!(read.slots, hopper.slots);
        assert_eq!(read.cooldown, 7);
        assert!(reading.is_empty());
    }

    /// Кафедра и проигрыватель хранят своё: страницу и «играет».
    #[test]
    fn lectern_and_jukebox_survive_the_disk() {
        let book = blocks::item_named("writable_book").expect("книга");
        let mut lectern = Container::new(Kind::Lectern);

        lectern.slots[0] = Some(Stack::new(book, 1));
        lectern.page = 3;
        lectern.pages = 5;

        let mut jukebox = Container::new(Kind::Jukebox);

        jukebox.started = 40;

        let mut bytes = Vec::new();
        lectern.write((1, 2, 3), &mut bytes);
        jukebox.write((4, 5, 6), &mut bytes);

        let mut reading = bytes.as_slice();
        let (_, read) = Container::read(&mut reading, 0, 0).expect("кафедра читается");

        assert_eq!((read.slots.clone(), read.page, read.pages), (lectern.slots.clone(), 3, 5));

        let (_, read) = Container::read(&mut reading, 0, 0).expect("проигрыватель читается");

        assert_eq!((read.kind, read.started), (Kind::Jukebox, 1));
        assert!(reading.is_empty());
    }

    /// Данные блока из команды разбираются в ячейки.
    #[test]
    fn items_come_from_command_text() {
        let stone = blocks::item_named("stone").expect("камень");
        let dirt = blocks::item_named("dirt").expect("земля");
        let items = items_from_text(r#"{Items:[{Slot:0b,id:"minecraft:stone",count:10},{Slot:26b,id:"minecraft:dirt",count:2}]}"#)
            .expect("разобрано");

        assert_eq!(items, vec![(0, Stack::new(stone, 10)), (26, Stack::new(dirt, 2))]);
        assert_eq!(items_from_text("{}"), Some(Vec::new()));

        let book = blocks::item_named("writable_book").expect("книга");
        let lectern = r#"{Book:{id:"minecraft:writable_book",count:1,components:{"minecraft:writable_book_content":{pages:["a"]}}},Page:0}"#;
        assert_eq!(items_from_text(lectern), Some(vec![(0, Stack::new(book, 1))]));
        assert_eq!(items_from_text("нет"), None);
    }

    /// Сила по заполненности — как в записи чёрного ящика comparator-chest:
    /// 1 камень → 1; ещё полная стопка камня и 16 жемчуга (полная) → 2;
    /// меч (стопка из одного) → по-прежнему 2.
    #[test]
    fn comparator_reads_fullness() {
        let stone = blocks::item_named("stone").expect("камень");
        let pearl = blocks::item_named("ender_pearl").expect("жемчуг");
        let sword = blocks::item_named("diamond_sword").expect("меч");
        let mut chest = Container::new(Kind::Chest);

        assert_eq!(Container::signal(&chest.slots), 0);
        chest.slots[0] = Some(Stack::new(stone, 1));
        assert_eq!(Container::signal(&chest.slots), 1);
        chest.slots[1] = Some(Stack::new(stone, 64));
        assert_eq!(Container::signal(&chest.slots), 1);
        chest.slots[2] = Some(Stack::new(pearl, 16));
        assert_eq!(Container::signal(&chest.slots), 2);
        chest.slots[3] = Some(Stack::new(sword, 1));
        assert_eq!(Container::signal(&chest.slots), 2);

        // Воронка на пять ячеек: один камень — уже 1.
        let mut hopper = Container::new(Kind::Hopper);
        hopper.slots[0] = Some(Stack::new(stone, 1));
        assert_eq!(Container::signal(&hopper.slots), 1);
    }
}
