// Рамки для предметов у Bedrock: не сущности, а блоки `frame` и `glow_frame`
// с блок-актором (minecraft.wiki, Item Frame, Bedrock Edition: «the item
// frame functions as a block entity»; данные блока — `ItemRotation`,
// `ItemDropChance`, `Item`).
//
// У Java рамка — сущность (frame.rs), и её место в мире — одна клетка перед
// опорным блоком. Здесь та же клетка получает блок рамки, а данные ложатся
// в Block Actor Data. Игры сторон совпадают: то, что делает игрок Java,
// видит игрок Bedrock, и наоборот — состояние одно, в frame.rs.
//
// Кода игры и чужих ядер здесь нет: номера состояний блока — из таблицы
// blockStates.json minecraft-data (Bedrock 1.26.10), формат NBT — как у
// остальных блок-акторов (containers.rs).
//
// Что не сверено живым клиентом, помечено «[проверить]».

use std::collections::HashMap;
use std::sync::OnceLock;

use super::codec::Out;
use super::inventory::bedrock_item;
use super::session;
use crate::frame::Frame;
use crate::inventory::Stack;

/// Первое состояние блока `frame` и `glow_frame` в таблице Bedrock 1.26.10:
/// дальше идут состояния по возрастанию `facing_direction` (0..=5), затем
/// `item_frame_map_bit` (по шесть), затем `item_frame_photo_bit` (по 12)
/// — blockStates.json, состояния 6291–6314 и 1046–1069.
const FRAME_FIRST: u32 = 6291;
const GLOW_FIRST: u32 = 1046;

/// Сколько градусов в одном щелчке поворота: у Java поворот 0..=7 по 45°, у
/// Bedrock `ItemRotation` хранится в градусах.
const DEGREES_PER_TURN: f32 = 45.0;

/// Теги сетевого NBT.
const BYTE: u8 = 1;
const SHORT: u8 = 2;
const INT: u8 = 3;
const FLOAT: u8 = 5;
const STRING: u8 = 8;
const COMPOUND: u8 = 10;

/// Сетевой номер состояния блока рамки. `facing_direction` у Bedrock — те же
/// номера сторон, что у Java (0 низ, 1 верх, 2 север, 3 юг, 4 запад, 5
/// восток) и тот же смысл: куда смотрит лицо рамки. [проверить] живым клиентом:
/// вики называет только четыре стороны света.
pub fn block_state(frame: &Frame) -> u32 {
    let first = if frame.glow { GLOW_FIRST } else { FRAME_FIRST };

    first + u32::from(frame.facing.min(crate::frame::EAST))
}

/// Имя предмета Bedrock по его номеру (`item_names.txt`, делает
/// make_bedrock_tables.py).
pub(super) fn bedrock_item_name(id: i32) -> Option<&'static str> {
    static NAMES: OnceLock<HashMap<i32, &'static str>> = OnceLock::new();

    NAMES
        .get_or_init(|| {
            include_str!("item_names.txt")
                .lines()
                .filter_map(|line| {
                    let (id, name) = line.split_once('\t')?;

                    Some((id.parse().ok()?, name))
                })
                .collect()
        })
        .get(&id)
        .copied()
}

/// Предмет рамки как NBT-компаунд предмета: `Count`, `Damage`, `Name`,
/// `WasPickedUp`. У предметов, которых нет у Bedrock, — None: рамка тогда
/// показывается пустой. [проверить]: у блоков-предметов игра пишет ещё
/// компаунд `Block`; здесь его нет.
fn item_tag(out: &mut Out, stack: Stack) -> bool {
    let Some((id, metadata, _)) = bedrock_item(stack.item) else {
        return false;
    };
    let Some(name) = bedrock_item_name(id) else {
        return false;
    };

    out.u8(COMPOUND).string("Item");
    out.u8(BYTE).string("Count").u8(1);
    out.u8(SHORT).string("Damage").li16(metadata as i16);
    out.u8(STRING).string("Name").string(&format!("minecraft:{name}"));
    out.u8(BYTE).string("WasPickedUp").u8(0);
    out.u8(0);
    true
}

/// Block Actor Data рамки: место и сетевой NBT-компаунд — `id` (`ItemFrame`
/// или `GlowItemFrame`), место, `isMovable`, `ItemDropChance` (у рамки
/// предмет выпадает всегда — 1), `ItemRotation` в градусах и `Item`, если
/// он есть.
pub fn actor_packet(frame: &Frame) -> Vec<u8> {
    let (x, y, z) = frame.pos;
    let mut out = Out::packet(session::BLOCK_ACTOR_DATA);

    out.block_position(x, y, z);
    out.u8(COMPOUND).string("");
    out.u8(STRING).string("id").string(if frame.glow { "GlowItemFrame" } else { "ItemFrame" });
    out.u8(INT).string("x").zigzag32(x);
    out.u8(INT).string("y").zigzag32(y);
    out.u8(INT).string("z").zigzag32(z);
    out.u8(BYTE).string("isMovable").u8(1);
    out.u8(FLOAT).string("ItemDropChance").lf32(1.0);
    out.u8(FLOAT).string("ItemRotation").lf32(f32::from(frame.rotation) * DEGREES_PER_TURN);

    if let Some(stack) = frame.item {
        item_tag(&mut out, stack);
    }

    out.u8(0);
    out.bytes
}

/// Update Block с блоком рамки (основной слой, соседям по сети): как у
/// остальных блоков в `world::block_updates`.
pub fn block_packet(frame: &Frame) -> Vec<u8> {
    let (x, y, z) = frame.pos;
    let mut update = Out::packet(session::UPDATE_BLOCK);

    update.block_position(x, y, z).varint(block_state(frame)).varint(2).varint(0);
    update.bytes
}

/// Всё, что клиенту нужно знать о рамке: блок, потом его данные.
pub fn show(frame: &Frame) -> [Vec<u8>; 2] {
    [block_packet(frame), actor_packet(frame)]
}

/// Рамка, что висит в клетке `pos`: у Bedrock в клетке один блок рамки,
/// поэтому, если у Java их там несколько (на разных сторонах), берётся первая.
pub fn frame_at(frames: &[Frame], pos: (i32, i32, i32)) -> Option<&Frame> {
    frames.iter().find(|frame| frame.pos == pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bedrock::codec::In;
    use crate::frame::{DOWN, EAST, NORTH, SOUTH, UP, WEST};

    fn stone() -> Stack {
        Stack::new(crate::blocks::item_named("stone").expect("камень"), 1)
    }

    /// Номера состояний совпадают с таблицей blockStates.json Bedrock 1.26.10
    /// (`frame`: 6291.. по сторонам, `glow_frame`: 1046..).
    #[test]
    fn facing_and_glow_choose_the_block_state() {
        let state = |facing, glow| block_state(&Frame::new(1, (0, 0, 0), facing, glow));

        assert_eq!(state(DOWN, false), 6291);
        assert_eq!(state(UP, false), 6292);
        assert_eq!(state(NORTH, false), 6293);
        assert_eq!(state(SOUTH, false), 6294);
        assert_eq!(state(WEST, false), 6295);
        assert_eq!(state(EAST, false), 6296);
        assert_eq!(state(DOWN, true), 1046);
        assert_eq!(state(EAST, true), 1051);
    }

    /// Читает начало пакета блок-актора до первого тега компаунда.
    fn tags(packet: &[u8]) -> In<'_> {
        let mut reader = In::new(packet);

        assert_eq!(reader.varint(), Some(session::BLOCK_ACTOR_DATA));
        reader.zigzag32().expect("x");
        reader.zigzag32().expect("y");
        reader.zigzag32().expect("z");
        assert_eq!((reader.u8(), reader.string().as_deref()), (Some(COMPOUND), Some("")));

        reader
    }

    #[test]
    fn an_empty_frame_actor_has_no_item() {
        let frame = Frame::new(1, (10, 64, -3), WEST, false);
        let packet = actor_packet(&frame);
        let mut reader = tags(&packet);

        assert_eq!((reader.u8(), reader.string().as_deref(), reader.string().as_deref()), (Some(STRING), Some("id"), Some("ItemFrame")));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.zigzag32()), (Some(INT), Some("x"), Some(10)));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.zigzag32()), (Some(INT), Some("y"), Some(64)));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.zigzag32()), (Some(INT), Some("z"), Some(-3)));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.u8()), (Some(BYTE), Some("isMovable"), Some(1)));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.lf32()), (Some(FLOAT), Some("ItemDropChance"), Some(1.0)));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.lf32()), (Some(FLOAT), Some("ItemRotation"), Some(0.0)));
        assert_eq!(reader.u8(), Some(0), "конец компаунда");
        assert_eq!(reader.u8(), None);
    }

    /// Светящаяся рамка с камнем, повёрнутым на три щелчка: имя актора,
    /// градусы и предмет с именем Bedrock.
    #[test]
    fn a_glow_frame_actor_carries_rotation_in_degrees_and_the_item() {
        let mut frame = Frame::new(1, (0, 70, 0), UP, true);

        frame.item = Some(stone());
        frame.rotation = 3;

        let packet = actor_packet(&frame);
        let mut reader = tags(&packet);

        assert_eq!((reader.u8(), reader.string().as_deref(), reader.string().as_deref()), (Some(STRING), Some("id"), Some("GlowItemFrame")));

        for _ in 0..3 {
            reader.u8().expect("тег");
            reader.string().expect("имя");
            reader.zigzag32().expect("число");
        }

        reader.take(1 + 1 + 9 + 1).expect("isMovable");
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.lf32()), (Some(FLOAT), Some("ItemDropChance"), Some(1.0)));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.lf32()), (Some(FLOAT), Some("ItemRotation"), Some(135.0)));
        assert_eq!((reader.u8(), reader.string().as_deref()), (Some(COMPOUND), Some("Item")));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.u8()), (Some(BYTE), Some("Count"), Some(1)));
        assert_eq!((reader.u8(), reader.string().as_deref()), (Some(SHORT), Some("Damage")));
        reader.take(2).expect("метаданные");
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.string().as_deref()), (Some(STRING), Some("Name"), Some("minecraft:stone")));
        assert_eq!((reader.u8(), reader.string().as_deref(), reader.u8()), (Some(BYTE), Some("WasPickedUp"), Some(0)));
        assert_eq!(reader.u8(), Some(0), "конец предмета");
        assert_eq!(reader.u8(), Some(0), "конец компаунда");
        assert_eq!(reader.u8(), None);
    }

    /// Поворот 0..=7 — 0°, 45°, … 315°.
    #[test]
    fn rotation_goes_in_steps_of_45_degrees() {
        for turn in 0..8u8 {
            let mut frame = Frame::new(1, (0, 0, 0), SOUTH, false);

            frame.rotation = turn;

            let packet = actor_packet(&frame);
            let needle = f32::from(turn) * 45.0;
            let bytes = needle.to_le_bytes();

            assert!(packet.windows(4).any(|window| window == bytes), "угол {needle}");
        }
    }

    /// Update Block: место, номер состояния, флаги, основной слой.
    #[test]
    fn the_block_packet_places_the_frame_block() {
        let frame = Frame::new(1, (-4, 63, 9), NORTH, false);
        let packet = block_packet(&frame);
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(session::UPDATE_BLOCK));
        assert_eq!((reader.zigzag32(), reader.zigzag32(), reader.zigzag32()), (Some(-4), Some(63), Some(9)));
        assert_eq!((reader.varint(), reader.varint(), reader.varint()), (Some(6293), Some(2), Some(0)));
        assert_eq!(reader.u8(), None);
    }

    /// Имена предметов рамок в таблице: `frame` и `glow_frame` — по номерам
    /// реестра Bedrock, и предметы Java на них переводятся.
    #[test]
    fn frame_items_exist_on_both_sides() {
        assert_eq!(bedrock_item_name(553), Some("frame"));
        assert_eq!(bedrock_item_name(665), Some("glow_frame"));

        let plain = crate::blocks::item_named("item_frame").expect("рамка");
        let glow = crate::blocks::item_named("glow_item_frame").expect("светящаяся рамка");

        assert_eq!(bedrock_item(plain).map(|(id, _, _)| id), Some(553));
        assert_eq!(bedrock_item(glow).map(|(id, _, _)| id), Some(665));
    }

    #[test]
    fn a_cell_shows_the_first_frame_in_it() {
        let frames = [Frame::new(1, (0, 0, 0), WEST, false), Frame::new(2, (0, 0, 0), UP, true), Frame::new(3, (1, 0, 0), UP, true)];

        assert_eq!(frame_at(&frames, (0, 0, 0)).map(|frame| frame.entity_id), Some(1));
        assert_eq!(frame_at(&frames, (1, 0, 0)).map(|frame| frame.entity_id), Some(3));
        assert!(frame_at(&frames, (2, 0, 0)).is_none());
    }

    /// Сервер для проверок: два пустых мира и каменная стена в клетке 1,1,0.
    fn server() -> crate::shared::Shared {
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(),
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsheriffanya-bedframe-ops-{}.json", std::process::id()))),
            crate::skins::Settings::default(),
        );

        shared
            .world_in(crate::dimension::Dimension::Overworld)
            .lock()
            .unwrap()
            .set_block_silently(1, 1, 0, crate::blocks::state_by_name("stone").unwrap());
        shared
    }

    /// Действия, что приходят от игрока Bedrock, идут теми же вызовами
    /// frame.rs, что и от игрока Java: повесил рамку на грань, положил
    /// предмет, повернул, ударил, сломал — и что Bedrock при этом видит.
    #[test]
    fn bedrock_actions_change_the_shared_frame_and_what_bedrock_sees() {
        let shared = server();
        let dimension = crate::dimension::Dimension::Overworld;
        let mut world = shared.world_in(dimension).lock().unwrap();
        let mut items = shared.items_in(dimension).lock().unwrap();

        // Щелчок по западной грани камня в 1,1,0: рамка в клетке 0,1,0.
        assert!(crate::frame::place(&mut world, &mut items, &shared, (1, 1, 0), WEST, false));
        let id = items.frames()[0].entity_id;

        assert_eq!(items.frames()[0].pos, (0, 1, 0));
        assert_eq!(block_state(&items.frames()[0]), 6295);

        // Предмет ложится; клиенту уходит блок и актор с камнем.
        let stone = stone();

        assert_eq!(crate::frame::use_on(&mut world, &mut items, id, Some(stone)), crate::frame::Used::Added);
        let shown = show(&items.frames()[0]);
        let needle = b"minecraft:stone";

        assert!(shown[1].windows(needle.len()).any(|window| window == needle));

        // Второй щелчок — поворот на 45°.
        assert_eq!(crate::frame::use_on(&mut world, &mut items, id, Some(stone)), crate::frame::Used::Rotated);
        assert!(actor_packet(&items.frames()[0]).windows(4).any(|window| window == 45.0f32.to_le_bytes()));
        assert_eq!(crate::redstone::frame_power(&world, (0, 1, 0), WEST), Some(2), "сравнитель Java читает то же");

        // Удар вынимает предмет (рамка остаётся пустой), слом убирает рамку.
        assert!(crate::frame::strike(&mut world, &mut items, &shared, id, false));
        assert!(items.frames()[0].item.is_none());
        assert!(!actor_packet(&items.frames()[0]).windows(needle.len()).any(|window| window == needle));

        assert!(crate::frame::break_off(&mut world, &mut items, &shared, id, false));
        assert!(items.frames().is_empty());
        assert!(!crate::frame::break_off(&mut world, &mut items, &shared, id, false), "второй раз рамки уже нет");
    }
}
