// Хранилища для игроков Bedrock: пакеты окна, блок-акторы, крышка сундука.
//
// Само хранилище — общее с Java (crate::containers): те же половины сундука,
// смотрящие, звуки, крышка и сила ловушки. Здесь только то, как это
// выглядит в протоколе Bedrock. Поля пакетов — документация Mojang к r/26_u1
// (json/ContainerOpenPacket, ContainerClosePacket, BlockEventPacket,
// BlockActorDataPacket, html/enums.html: ContainerType, ContainerID) и
// описание протокола minecraft-data 1.26.10; строковые id блок-акторов и
// поля сундука — minecraft.wiki, «Bedrock Edition level format/Block entity
// format». Разбор — tools/research/container-protocol.md, часть 2.

use super::codec::Out;
use super::inventory::{BARREL, CRAFTER, LEVEL_ENTITY};
use super::session;
use crate::containers::{self, Kind, Pos};
use crate::world::World;

/// Номера окон хранилищ: CONTAINER_ID_FIRST … CONTAINER_ID_LAST (ContainerID).
pub const FIRST_WINDOW: u8 = 1;
pub const LAST_WINDOW: u8 = 100;

/// Вид экрана (ContainerType): у сундука, ловушки и бочки — общий
/// CONTAINER (0), у двойного сундука отдельного нет.
pub fn screen(kind: Kind) -> u8 {
    match kind {
        Kind::Chest | Kind::TrappedChest | Kind::Barrel | Kind::CopperChest => 0,
        Kind::Dispenser => 6,
        Kind::Dropper => 7,
        Kind::Hopper => 8,
        // CRAFTER (36). [проверить] живым клиентом: выключенные ячейки
        // Bedrock пока не видит.
        Kind::Crafter => 36,
        // Окна у них на Bedrock нет: щелчок разбирается раньше
        // (world.rs, use_block).
        // [проверить] у полки и резного шкафа на Bedrock окна тоже нет.
        Kind::Jukebox | Kind::Lectern | Kind::Shelf | Kind::ChiseledBookshelf => 0,
        // FURNACE (2), BREWING_STAND (4), BLAST_FURNACE (27), SMOKER (28).
        // [проверить] живым клиентом: ячейки печи и стойки на Bedrock.
        Kind::Furnace => 2,
        Kind::BrewingStand => 4,
        Kind::BlastFurnace => 27,
        Kind::Smoker => 28,
    }
}

/// Как клиент называет ячейки хранилища (ContainerEnumName): у бочки своё
/// имя, остальные — LevelEntityContainer.
pub fn slot_name(kind: Kind) -> u8 {
    match kind {
        Kind::Barrel => BARREL,
        Kind::Crafter => CRAFTER,
        _ => LEVEL_ENTITY,
    }
}

/// Container Open: номер окна, вид экрана, место блока и сущность — у блока
/// её нет (−1).
pub fn open_packet(window: u8, kind: Kind, (x, y, z): Pos) -> Vec<u8> {
    let mut out = Out::packet(session::CONTAINER_OPEN);
    out.u8(window).u8(screen(kind)).block_position(x, y, z).zigzag64(-1);
    out.bytes
}

/// Container Close: номер окна, вид экрана, закрыл ли сервер.
pub fn close_packet(window: u8, screen: u8, by_server: bool) -> Vec<u8> {
    let mut out = Out::packet(session::CONTAINER_CLOSE);
    out.u8(window).u8(screen).bool(by_server);
    out.bytes
}

/// Block Event «сменить состояние» (1) — крышка сундука: 1 открыта, 0
/// закрыта.
pub fn lid_packet((x, y, z): Pos, open: bool) -> Vec<u8> {
    const CHANGE_STATE: i32 = 1;

    let mut out = Out::packet(session::BLOCK_EVENT);
    out.block_position(x, y, z).zigzag32(CHANGE_STATE).zigzag32(i32::from(open));
    out.bytes
}

/// Строковый id блок-актора Bedrock (у сундука-ловушки тот же, что у сундука).
fn actor_id(kind: Kind) -> &'static str {
    match kind {
        Kind::Chest | Kind::TrappedChest => "Chest",
        // [проверить] имена блок-акторов меди и полки по bedrock-protocol-docs.
        Kind::CopperChest => "Chest",
        Kind::Shelf => "Shelf",
        Kind::ChiseledBookshelf => "ChiseledBookshelf",
        Kind::Barrel => "Barrel",
        Kind::Hopper => "Hopper",
        Kind::Dropper => "Dropper",
        Kind::Dispenser => "Dispenser",
        // [проверить] имена блок-акторов по bedrock-protocol-docs.
        Kind::Jukebox => "Jukebox",
        Kind::Lectern => "Lectern",
        Kind::Crafter => "Crafter",
        Kind::Furnace => "Furnace",
        Kind::BlastFurnace => "BlastFurnace",
        Kind::Smoker => "Smoker",
        Kind::BrewingStand => "BrewingStand",
    }
}

/// Вторая половина большого сундука для блок-актора: где она и ведущая ли
/// эта половина.
pub type Pair = (Pos, bool);

/// Block Actor Data хранилища: место и сетевой NBT-компаунд с id, местом,
/// isMovable (до 26.30 он есть у всех блок-акторов) и, у половины большого
/// сундука, pairx/pairz второй половины и pairlead. Содержимое не нужно —
/// оно приходит окном.
pub fn actor_packet(kind: Kind, (x, y, z): Pos, pair: Option<Pair>) -> Vec<u8> {
    const BYTE: u8 = 1;
    const INT: u8 = 3;
    const STRING: u8 = 8;
    const COMPOUND: u8 = 10;

    let mut out = Out::packet(session::BLOCK_ACTOR_DATA);
    out.block_position(x, y, z);

    // Сетевой NBT Bedrock: имя — строка с длиной varint, int — со знаком
    // переменной длины.
    out.u8(COMPOUND).string("");
    out.u8(STRING).string("id").string(actor_id(kind));
    out.u8(INT).string("x").zigzag32(x);
    out.u8(INT).string("y").zigzag32(y);
    out.u8(INT).string("z").zigzag32(z);
    out.u8(BYTE).string("isMovable").u8(1);

    if let Some(((pair_x, _, pair_z), lead)) = pair {
        out.u8(BYTE).string("pairlead").u8(u8::from(lead));
        out.u8(INT).string("pairx").zigzag32(pair_x);
        out.u8(INT).string("pairz").zigzag32(pair_z);
    }

    out.u8(0);
    out.bytes
}

/// Block Actor Data хранилища, которое стоит в мире в этом месте; None —
/// хранилища там нет. Ведущая половина большого сундука — та, чьи ячейки
/// в окне первые (`containers::halves`).
pub fn actor_at(world: &World, pos: Pos) -> Option<Vec<u8>> {
    let kind = Kind::of(world.get_block(pos.0, pos.1, pos.2))?;
    let halves = containers::halves(world, pos);
    let pair = match halves[..] {
        [first, second] => Some(if first == pos { (second, true) } else { (first, false) }),
        _ => None,
    };

    Some(actor_packet(kind, pos, pair))
}

/// Сундук ли (или ловушка) этот номер блока — у Block Action крышки.
pub fn is_chest_block(block: i32) -> bool {
    use std::sync::OnceLock;

    static CHESTS: OnceLock<[Option<i32>; 2]> = OnceLock::new();

    CHESTS
        .get_or_init(|| [crate::blocks::block_id("chest"), crate::blocks::block_id("trapped_chest")])
        .contains(&Some(block))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bedrock::codec::In;

    /// Container Open: окно, вид экрана одним байтом, место, сущность −1.
    #[test]
    fn container_open_reads_back() {
        let packet = open_packet(3, Kind::Hopper, (-5, 70, 12));
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(session::CONTAINER_OPEN));
        assert_eq!(reader.u8(), Some(3));
        assert_eq!(reader.u8(), Some(8));
        assert_eq!((reader.zigzag32(), reader.zigzag32(), reader.zigzag32()), (Some(-5), Some(70), Some(12)));
        assert_eq!(reader.zigzag64(), Some(-1));
        assert_eq!(reader.u8(), None);

        assert_eq!(screen(Kind::Chest), 0);
        assert_eq!(screen(Kind::Barrel), 0);
        assert_eq!(screen(Kind::Dispenser), 6);
        assert_eq!(screen(Kind::Dropper), 7);
    }

    #[test]
    fn container_close_and_lid_read_back() {
        assert_eq!(close_packet(7, 0, true), vec![0x2f, 7, 0, 1]);

        let packet = lid_packet((1, -60, -2), true);
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(session::BLOCK_EVENT));
        assert_eq!((reader.zigzag32(), reader.zigzag32(), reader.zigzag32()), (Some(1), Some(-60), Some(-2)));
        assert_eq!((reader.zigzag32(), reader.zigzag32()), (Some(1), Some(1)));
        assert_eq!(reader.u8(), None);
    }

    /// Блок-актор половины большого сундука: теги по порядку и конец
    /// компаунда.
    #[test]
    fn chest_actor_carries_its_pair() {
        let packet = actor_packet(Kind::TrappedChest, (10, 64, -3), Some(((11, 64, -3), true)));
        let mut reader = In::new(&packet);

        assert_eq!(reader.varint(), Some(session::BLOCK_ACTOR_DATA));
        assert_eq!((reader.zigzag32(), reader.zigzag32(), reader.zigzag32()), (Some(10), Some(64), Some(-3)));
        assert_eq!(reader.u8(), Some(10));
        assert_eq!(reader.string().as_deref(), Some(""));

        let mut tags = Vec::new();

        loop {
            let tag = reader.u8().expect("тег");

            if tag == 0 {
                break;
            }

            let name = reader.string().expect("имя");
            let value = match tag {
                1 => reader.u8().expect("байт").to_string(),
                3 => reader.zigzag32().expect("число").to_string(),
                8 => reader.string().expect("строка"),
                _ => panic!("неожиданный тег {tag}"),
            };

            tags.push((name, value));
        }

        let expected = [("id", "Chest"), ("x", "10"), ("y", "64"), ("z", "-3"), ("isMovable", "1"), ("pairlead", "1"), ("pairx", "11"), ("pairz", "-3")];
        assert_eq!(tags, expected.map(|(name, value)| (name.to_string(), value.to_string())));
        assert_eq!(reader.u8(), None);

        // У одиночного — без пары.
        let single = actor_packet(Kind::Barrel, (0, 0, 0), None);
        assert!(!single.windows(5).any(|w| w == b"pairx"));
        assert!(single.windows(6).any(|w| w == b"Barrel"));
    }

    /// Большой сундук в мире: обе половины называют друг друга, ведущая —
    /// правая (её ячейки в окне первые), одиночная бочка — без пары.
    #[test]
    fn a_double_chest_pairs_both_halves() {
        let state = |text: &str| crate::blocks::state_from_text(text).expect(text);
        let mut world = World::in_memory();

        // Смотрит на север: правая половина — восточная.
        world.set_block(1, 64, 0, state("chest[facing=north,type=right]"));
        world.set_block(0, 64, 0, state("chest[facing=north,type=left]"));
        world.set_block(5, 64, 0, state("barrel"));

        let pair_of = |packet: Vec<u8>| {
            let lead = packet.windows(9).position(|w| w == b"\x08pairlead").map(|at| packet[at + 9]);
            let x = packet.windows(6).position(|w| w == b"\x05pairx").map(|at| In::new(&packet[at + 6..]).zigzag32());
            (lead, x.flatten())
        };

        assert_eq!(pair_of(actor_at(&world, (1, 64, 0)).expect("правая")), (Some(1), Some(0)));
        assert_eq!(pair_of(actor_at(&world, (0, 64, 0)).expect("левая")), (Some(0), Some(1)));
        assert_eq!(pair_of(actor_at(&world, (5, 64, 0)).expect("бочка")), (None, None));
        assert_eq!(actor_at(&world, (9, 64, 0)), None);
    }

    #[test]
    fn chest_blocks_are_known() {
        assert!(is_chest_block(crate::blocks::block_id("chest").expect("сундук")));
        assert!(is_chest_block(crate::blocks::block_id("trapped_chest").expect("ловушка")));
        assert!(!is_chest_block(crate::blocks::block_id("piston").expect("поршень")));
    }
}
