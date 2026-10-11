// Поршни у Bedrock: у Java «выдвинут» — свойство блока `extended`, а у
// Bedrock такого свойства нет: основание — блок `piston` с одним лишь
// `facing_direction`, голова — блок `piston_arm_collision`, а сам рычаг
// рисует блок-актор `PistonArm` на основании (minecraft.wiki, Piston,
// Bedrock Edition: «piston block entity», поля State, NewState, Progress,
// LastProgress, Sticky, AttachedBlocks, BreakBlocks). Без актора клиент
// видит одно основание и невидимую голову — поршень «выдвигается» впустую.
//
// Покой: для каждого основания, которое у Java выдвинуто или втянуто,
// собирается Block Actor Data (`actor_packet`).
//
// Ход (снято с BDS 1.26.13.1 по сети, tmp/br-polish/bds-p3-*.jsonl):
// через два такта после питания основание получает актор с State 1
// (выдвигается) или 3 (втягивается), Progress 0 (или 1) и списком
// AttachedBlocks — клетками, откуда едут блоки (дальний первым); тут же
// Update SubChunk Blocks: на местах назначения блок `moving_block`, на
// старых — воздух, на месте головы — голова; следом Block Actor Data
// `MovingBlock` на каждом месте назначения (что за блок едет, откуда
// ведёт ход). Ещё через два такта — звук и актор основания в конечном
// состоянии (2 или 0, Progress 1 или 0), и ещё через такт — Update
// SubChunk Blocks с настоящими блоками на местах назначения (флаги 19).
// Блоки скользят, а не прыгают: клиент сам рисует ход между двумя
// актором. [проверить] живым клиентом.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::codec::Out;
use super::session;

const BYTE: u8 = 1;
const INT: u8 = 3;
const FLOAT: u8 = 5;
const LIST: u8 = 9;
const COMPOUND: u8 = 10;
const STRING: u8 = 8;

/// Состояние основания поршня Java: липкий ли, выдвинут ли.
type Base = (bool, bool);

fn bases() -> &'static HashMap<i32, Base> {
    static BASES: OnceLock<HashMap<i32, Base>> = OnceLock::new();

    BASES.get_or_init(|| {
        let mut table = HashMap::new();

        for (name, sticky) in [("piston", false), ("sticky_piston", true)] {
            for extended in [false, true] {
                for facing in ["north", "east", "south", "west", "up", "down"] {
                    if let Some(state) = crate::blocks::state_from_text(&format!("{name}[extended={extended},facing={facing}]")) {
                        table.insert(state, (sticky, extended));
                    }
                }
            }
        }

        table
    })
}

/// Промежуток состояний, в котором лежат основания (быстрая проверка при
/// разборе чанка: основание — редкость).
fn span() -> (i32, i32) {
    static SPAN: OnceLock<(i32, i32)> = OnceLock::new();

    *SPAN.get_or_init(|| {
        let keys = bases().keys();
        (keys.clone().copied().min().unwrap_or(0), keys.copied().max().unwrap_or(-1))
    })
}

/// Основание поршня: (липкий, выдвинут). None — другой блок.
pub fn base_of(state: i32) -> Option<Base> {
    let (low, high) = span();

    if state < low || state > high {
        return None;
    }

    bases().get(&state).copied()
}

/// Состояния актора `PistonArm` (State у BDS): покой втянут, выдвигается,
/// выдвинут, втягивается.
const ARM_RETRACTED: u8 = 0;
const ARM_EXTENDING: u8 = 1;
const ARM_EXTENDED: u8 = 2;
const ARM_RETRACTING: u8 = 3;

/// Block Actor Data основания: `PistonArm` в покое — выдвинут (состояние 2,
/// ход 1) или втянут (состояние 0, ход 0).
pub fn actor_packet(at: (i32, i32, i32), (sticky, extended): Base) -> Vec<u8> {
    let state = if extended { ARM_EXTENDED } else { ARM_RETRACTED };

    arm_packet(at, sticky, state, if extended { 1.0 } else { 0.0 }, &[])
}

/// Block Actor Data основания с ходом: поля и их порядок — как у BDS
/// (по алфавиту). `attached` — клетки, откуда едут блоки.
fn arm_packet((x, y, z): (i32, i32, i32), sticky: bool, state: u8, progress: f32, attached: &[(i32, i32, i32)]) -> Vec<u8> {
    let mut out = Out::packet(session::BLOCK_ACTOR_DATA);

    out.block_position(x, y, z);
    out.u8(COMPOUND).string("");

    // Список клеток — плоский список int по три на клетку; пустой — с
    // типом «конец», как у BDS.
    if attached.is_empty() {
        out.u8(LIST).string("AttachedBlocks").u8(0).zigzag32(0);
    } else {
        out.u8(LIST).string("AttachedBlocks").u8(INT).zigzag32(attached.len() as i32 * 3);

        for (ax, ay, az) in attached {
            out.zigzag32(*ax).zigzag32(*ay).zigzag32(*az);
        }
    }

    out.u8(LIST).string("BreakBlocks").u8(0).zigzag32(0);
    out.u8(FLOAT).string("LastProgress").lf32(progress);
    out.u8(BYTE).string("NewState").u8(state);
    out.u8(FLOAT).string("Progress").lf32(progress);
    out.u8(BYTE).string("State").u8(state);
    out.u8(BYTE).string("Sticky").u8(u8::from(sticky));
    out.u8(STRING).string("id").string("PistonArm");
    // Двигать основание можно, только пока оно втянуто (BDS: isMovable 1 при
    // State 0, иначе 0).
    out.u8(BYTE).string("isMovable").u8(u8::from(state == ARM_RETRACTED));
    out.u8(INT).string("x").zigzag32(x);
    out.u8(INT).string("y").zigzag32(y);
    out.u8(INT).string("z").zigzag32(z);
    out.u8(0);
    out.bytes
}

// ---------------------------------------------------------------------------
// Ход
// ---------------------------------------------------------------------------

/// Сетевой номер блока `moving_block` в списке состояний Bedrock 1.26.10
/// (minecraft-data, blockStates.json; проверяется тестом по block_nbt.bin).
const MOVING_BLOCK: u32 = 9669;

/// Флаги Update SubChunk Blocks у BDS: в начале хода «соседям и по сети»,
/// в конце ещё и 16.
const FLAGS_START: u32 = 3;
const FLAGS_END: u32 = 19;

/// Сколько тактов (от пакета начала) до конца хода у клиента и до
/// настоящих блоков на местах назначения.
pub const END_AFTER: u64 = 2;
pub const FINAL_AFTER: u64 = 3;

/// Имена, ключи и строки из block_nbt.bin и места состояний в нём.
struct StateTable {
    names: Vec<&'static str>,
    keys: Vec<&'static str>,
    texts: Vec<&'static str>,
    offsets: Vec<usize>,
}

static BLOCK_NBT: &[u8] = include_bytes!("block_nbt.bin");

fn state_table() -> &'static StateTable {
    static TABLE: OnceLock<StateTable> = OnceLock::new();

    TABLE.get_or_init(|| {
        let mut at = 0;
        let mut strings = || {
            let count = u16::from_le_bytes([BLOCK_NBT[at], BLOCK_NBT[at + 1]]) as usize;
            let mut list = Vec::with_capacity(count);

            at += 2;

            for _ in 0..count {
                let len = BLOCK_NBT[at] as usize;

                list.push(std::str::from_utf8(&BLOCK_NBT[at + 1..at + 1 + len]).unwrap_or(""));
                at += 1 + len;
            }

            list
        };
        let (names, keys, texts) = (strings(), strings(), strings());
        let count = u32::from_le_bytes([BLOCK_NBT[at], BLOCK_NBT[at + 1], BLOCK_NBT[at + 2], BLOCK_NBT[at + 3]]) as usize;
        let mut at = at + 4;
        let mut offsets = Vec::with_capacity(count);

        for _ in 0..count {
            offsets.push(at);
            at += 3 + 3 * BLOCK_NBT[at + 2] as usize;
        }

        StateTable { names, keys, texts, offsets }
    })
}

/// Имя блока Bedrock по сетевому номеру состояния.
#[cfg(test)]
fn state_name(runtime: u32) -> Option<&'static str> {
    let table = state_table();
    let at = *table.offsets.get(runtime as usize)?;

    table.names.get(u16::from_le_bytes([BLOCK_NBT[at], BLOCK_NBT[at + 1]]) as usize).copied()
}

/// Описание блока для NBT: `{name, states{…}, version}` — как у BDS в
/// `movingBlock` (имя с `minecraft:`, свойства по алфавиту).
pub(super) fn write_block(out: &mut Out, runtime: u32) {
    let table = state_table();
    let at = table.offsets.get(runtime as usize).copied();
    let name = at.and_then(|at| table.names.get(u16::from_le_bytes([BLOCK_NBT[at], BLOCK_NBT[at + 1]]) as usize)).copied().unwrap_or("air");

    out.u8(STRING).string("name").string(&format!("minecraft:{name}"));
    out.u8(COMPOUND).string("states");

    if let Some(at) = at {
        for prop in 0..BLOCK_NBT[at + 2] as usize {
            let at = at + 3 + prop * 3;
            let key = table.keys.get(BLOCK_NBT[at] as usize).copied().unwrap_or("");
            let value = BLOCK_NBT[at + 2];

            match BLOCK_NBT[at + 1] {
                BYTE => out.u8(BYTE).string(key).u8(value),
                INT => out.u8(INT).string(key).zigzag32(i32::from(value)),
                _ => out.u8(STRING).string(key).string(table.texts.get(value as usize).copied().unwrap_or("")),
            };
        }
    }

    out.u8(0);
    // Версия описания блоков — та же, что у всех состояний 1.26.10.
    out.u8(INT).string("version").zigzag32(18_168_865);
    out.u8(0);
}

fn air() -> u32 {
    super::world::bedrock_state(crate::world::AIR)
}

/// Клетка → границы подчанка (его блок-координаты, как ждёт пакет).
fn sub_chunk_of((x, y, z): (i32, i32, i32)) -> (i32, i32, i32) {
    (x >> 4 << 4, y >> 4 << 4, z >> 4 << 4)
}

/// Запись пакета: клетка, номер блока, слой, флаги.
type Entry = ((i32, i32, i32), u32, u8, u32);

/// Update SubChunk Blocks: пачки по подчанкам в порядке появления. Каждая
/// запись — (клетка, номер блока, слой, флаги).
fn sub_chunk_packets(entries: &[Entry]) -> Vec<Vec<u8>> {
    let mut order: Vec<(i32, i32, i32)> = Vec::new();

    for (at, ..) in entries {
        let sub = sub_chunk_of(*at);

        if !order.contains(&sub) {
            order.push(sub);
        }
    }

    order
        .into_iter()
        .map(|sub| {
            let mut out = Out::packet(UPDATE_SUBCHUNK_BLOCKS);

            out.zigzag32(sub.0).zigzag32(sub.1).zigzag32(sub.2);

            for layer in [0u8, 1] {
                let list: Vec<_> = entries.iter().filter(|(at, _, entry_layer, _)| sub_chunk_of(*at) == sub && *entry_layer == layer).collect();

                out.varint(list.len() as u32);

                for ((x, y, z), runtime, _, flags) in list {
                    out.block_position(*x, *y, *z).varint(*runtime).varint(*flags);
                    // Падающего блока нет: «сущность» с самым малым номером, как у BDS.
                    out.zigzag64(i64::MIN).varint(0);
                }
            }

            out.bytes
        })
        .collect()
}

pub const UPDATE_SUBCHUNK_BLOCKS: u32 = 0xac;

/// Block Actor Data `MovingBlock` на месте назначения едущего блока.
fn moving_block_packet(at: (i32, i32, i32), base: (i32, i32, i32), runtime: u32, extending: bool) -> Vec<u8> {
    let mut out = Out::packet(session::BLOCK_ACTOR_DATA);

    out.block_position(at.0, at.1, at.2);
    out.u8(COMPOUND).string("");
    out.u8(BYTE).string("expanding").u8(u8::from(extending));
    out.u8(STRING).string("id").string("MovingBlock");
    out.u8(BYTE).string("isMovable").u8(1);
    out.u8(COMPOUND).string("movingBlock");
    write_block(&mut out, runtime);
    out.u8(COMPOUND).string("movingBlockExtra");
    write_block(&mut out, air());
    out.u8(INT).string("pistonPosX").zigzag32(base.0);
    out.u8(INT).string("pistonPosY").zigzag32(base.1);
    out.u8(INT).string("pistonPosZ").zigzag32(base.2);
    out.u8(INT).string("x").zigzag32(at.0);
    out.u8(INT).string("y").zigzag32(at.1);
    out.u8(INT).string("z").zigzag32(at.2);
    out.u8(0);
    out.bytes
}

/// Начало хода: актор основания в пути, блоки хода и актор на каждом месте
/// назначения.
pub fn start_packets(mv: &crate::world::PistonMove) -> Vec<Vec<u8>> {
    let origins: Vec<_> = mv.cargo.iter().map(|(from, ..)| *from).collect();
    let (state, progress) = if mv.extending { (ARM_EXTENDING, 0.0) } else { (ARM_RETRACTING, 1.0) };
    let mut packets = vec![arm_packet(mv.base, mv.sticky, state, progress, &origins)];
    let mut entries = Vec::new();

    // Втягивание с грузом: голова уходит, на её месте поедет блок.
    if !mv.extending && !mv.cargo.is_empty() {
        entries.push((mv.head, air(), 0, FLAGS_START));
    }

    for (from, to, _) in &mv.cargo {
        entries.push((*to, MOVING_BLOCK, 0, FLAGS_START));
        entries.push((*from, air(), 0, FLAGS_START));
    }

    if let Some(head) = mv.head_state {
        entries.push((mv.head, super::world::bedrock_state(head), 0, FLAGS_START));
    }

    packets.extend(sub_chunk_packets(&entries));

    for (_, to, cargo) in &mv.cargo {
        packets.push(moving_block_packet(*to, mv.base, super::world::bedrock_state(*cargo), mv.extending));
    }

    packets
}

/// Конец хода у клиента: актор основания в конечном состоянии.
pub fn end_packets(mv: &crate::world::PistonMove) -> Vec<Vec<u8>> {
    let (state, progress) = if mv.extending { (ARM_EXTENDED, 1.0) } else { (ARM_RETRACTED, 0.0) };

    vec![arm_packet(mv.base, mv.sticky, state, progress, &[])]
}

/// Настоящие блоки на местах назначения (слой воды — воздух).
pub fn final_packets(mv: &crate::world::PistonMove) -> Vec<Vec<u8>> {
    let mut entries: Vec<_> = mv.cargo.iter().map(|(_, to, state)| (*to, super::world::bedrock_state(*state), 0, FLAGS_END)).collect();

    entries.extend(mv.cargo.iter().map(|(_, to, _)| (*to, air(), 1, FLAGS_END)));

    let mut packets = sub_chunk_packets(&entries);

    // Приехавший поршень — со своим актором в покое (общий путь изменений
    // блоков его скрыл вместе с остальным ходом).
    for (_, to, state) in &mv.cargo {
        if let Some(base) = base_of(*state) {
            packets.push(actor_packet(*to, base));
        }
    }

    packets
}

/// Скрыть ли изменение блока `state` в `at` от клиента: его уже
/// показывает ход (`mv`) своими пакетами. Поршень в покое, служебный блок
/// хода, голова и приехавший блок — свои; прочее проходит.
pub fn hides(mv: &crate::world::PistonMove, at: (i32, i32, i32), state: i32) -> bool {
    let moving = crate::blocks::block_at_state(state) == Some("moving_piston");

    if at == mv.base {
        return base_of(state).is_some();
    }

    if mv.cargo.iter().any(|(_, to, cargo)| *to == at && (moving || state == *cargo)) {
        return true;
    }

    // Старые места опустели — об этом сказал пакет начала.
    if mv.cargo.iter().any(|(from, ..)| *from == at) && state == crate::world::AIR {
        return true;
    }

    // Место головы: служебный блок и сама голова (она в пакете начала).
    at == mv.head && mv.extending && (moving || Some(state) == mv.head_state)
}

/// Основания поршней в секциях чанка (состояния Java, порядок Y, Z, X) и
/// их Block Actor Data. `lowest` — номер нижней секции в `sections`
/// (у нас она на MIN_Y/16).
pub fn chunk_actors(chunk_x: i32, chunk_z: i32, sections: &[Option<std::sync::Arc<[u16]>>], lowest_section: i32, visible: impl Fn(i32) -> bool) -> Vec<Vec<u8>> {
    let (low, high) = span();
    let mut packets = Vec::new();

    for (index, section) in sections.iter().enumerate() {
        let Some(blocks) = section else {
            continue;
        };

        for (at, &state) in blocks.iter().enumerate() {
            let state = i32::from(state);

            if state < low || state > high {
                continue;
            }

            let Some(base) = bases().get(&state) else {
                continue;
            };
            let (bx, bz, by) = ((at % 16) as i32, ((at / 16) % 16) as i32, (at / 256) as i32);
            let y = (lowest_section + index as i32) * 16 + by;

            if visible(y) {
                packets.push(actor_packet((chunk_x * 16 + bx, y, chunk_z * 16 + bz), *base));
            }
        }
    }

    packets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_are_told_from_their_heads() {
        let state = |text: &str| crate::blocks::state_from_text(text).expect("состояние");

        assert_eq!(base_of(state("piston[extended=true,facing=east]")), Some((false, true)));
        assert_eq!(base_of(state("sticky_piston[extended=false,facing=up]")), Some((true, false)));
        assert_eq!(base_of(state("piston_head[facing=east]")), None);
        assert_eq!(base_of(state("stone")), None);
    }

    fn has(packet: &[u8], needle: &[u8]) -> bool {
        packet.windows(needle.len()).any(|w| w == needle)
    }

    /// Ход как в записи BDS: поршень на (6,-60,6) толкает на запад камни
    /// с 5 и 4 на 4 и 3.
    fn two_stones() -> crate::world::PistonMove {
        let stone = crate::blocks::state_from_text("stone").expect("камень");
        let head = crate::blocks::state_from_text("piston_head[facing=west,short=false,type=normal]");

        crate::world::PistonMove {
            base: (6, -60, 6),
            extending: true,
            sticky: false,
            head: (5, -60, 6),
            head_state: head,
            cargo: vec![((4, -60, 6), (3, -60, 6), stone), ((5, -60, 6), (4, -60, 6), stone)],
        }
    }

    #[test]
    fn moving_block_is_what_the_table_says() {
        assert_eq!(state_name(MOVING_BLOCK), Some("moving_block"));
        assert_eq!(state_name(super::super::world::bedrock_state(crate::blocks::state_from_text("stone").expect("камень"))), Some("stone"));
        assert_eq!(state_name(air()), Some("air"));
    }

    #[test]
    fn block_states_are_written_with_their_properties() {
        let piston = super::super::world::bedrock_state(crate::blocks::state_from_text("piston[extended=false,facing=west]").expect("поршень"));
        let mut out = Out::packet(0);

        write_block(&mut out, piston);

        assert!(has(&out.bytes, b"minecraft:piston"));
        assert!(has(&out.bytes, b"facing_direction"));
        // Версия описания: zigzag-varint 18168865, как у BDS (c2f0a911).
        assert!(has(&out.bytes, &[0xc2, 0xf0, 0xa9, 0x11]));
    }

    #[test]
    fn arm_packet_follows_the_official_server_bytes() {
        // Начало хода у BDS (tmp/br-polish/bds-p4-pa.jsonl, p2-extend): поля
        // по алфавиту, два камня в AttachedBlocks, State 1.
        let packets = start_packets(&two_stones());
        let want = "380c770c0a00090e4174746163686564426c6f636b73030c08770c0a770c090b427265616b426c6f636b730000050c4c61737450726f67726573730000000001084e6577537461746501050850726f67726573730000000001055374617465010106537469636b79000802696409506973746f6e41726d010969734d6f7661626c65000301780c0301797703017a0c00";
        let hex: String = packets[0].iter().map(|byte| format!("{byte:02x}")).collect();

        // В записи Attached с 4 и 5 — «08 77 0c 0a 77 0c»: три числа на клетку.
        assert_eq!(hex, want);
    }

    #[test]
    fn start_has_blocks_in_the_order_of_the_official_server() {
        let packets = start_packets(&two_stones());

        // Актор основания, Update SubChunk Blocks, два актора MovingBlock.
        assert_eq!(packets.len(), 4);
        // Update SubChunk Blocks: id 0xac (varint ac 01), подчанк (0, -64, 0), 5 записей.
        assert_eq!(packets[1][..6], [0xac, 0x01, 0x00, 0x7f, 0x00, 0x05]);
        assert!(has(&packets[2], b"MovingBlock") && has(&packets[2], b"minecraft:stone"));
        assert!(has(&packets[2], b"pistonPosX"));
    }

    #[test]
    fn final_has_stones_and_empty_second_layer() {
        let packets = final_packets(&two_stones());

        assert_eq!(packets.len(), 1);
        // Два блока, два места второго слоя, как у BDS.
        assert_eq!(packets[0][..6], [0xac, 0x01, 0x00, 0x7f, 0x00, 0x02]);
        assert!(has(&packets[0], &[0xff; 9]));
    }

    #[test]
    fn a_pulled_block_starts_with_the_head_going_away() {
        let stone = crate::blocks::state_from_text("stone").expect("камень");
        let pull = crate::world::PistonMove {
            base: (6, -60, 6),
            extending: false,
            sticky: true,
            head: (5, -60, 6),
            head_state: None,
            cargo: vec![((4, -60, 6), (5, -60, 6), stone)],
        };
        let packets = start_packets(&pull);

        // Актор State 3, подчанк из трёх записей (голова пропала, блок едет,
        // старое место пусто), один MovingBlock с expanding 0.
        assert_eq!(packets.len(), 3);
        assert_eq!(packets[1][..6], [0xac, 0x01, 0x00, 0x7f, 0x00, 0x03]);
        assert!(has(&packets[2], &[0x01, 0x09, b'e', b'x', b'p', b'a', b'n', b'd', b'i', b'n', b'g', 0x00]));
    }

    #[test]
    fn the_move_hides_only_its_own_changes() {
        let mv = two_stones();
        let state = |text: &str| crate::blocks::state_from_text(text).expect(text);

        // Основание, служебные блоки, голова и приехавшие камни — скрыты.
        assert!(hides(&mv, (6, -60, 6), state("piston[extended=true,facing=west]")));
        assert!(hides(&mv, (3, -60, 6), state("moving_piston[facing=west,type=normal]")));
        assert!(hides(&mv, (3, -60, 6), state("stone")));
        assert!(hides(&mv, (5, -60, 6), state("piston_head[facing=west,short=false,type=normal]")));
        assert!(hides(&mv, (4, -60, 6), state("stone")));
        // Чужой блок в тех же клетках проходит.
        assert!(!hides(&mv, (3, -60, 6), state("dirt")));
        assert!(!hides(&mv, (9, -60, 6), state("stone")));
    }

    #[test]
    fn extended_base_says_so_in_its_actor() {
        let extended = actor_packet((1, 64, 2), (true, true));
        let retracted = actor_packet((1, 64, 2), (true, false));

        assert_ne!(extended, retracted);
        assert!(extended.windows(9).any(|w| w == b"PistonArm"));
    }
}
