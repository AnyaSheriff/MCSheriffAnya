// Поршни у Bedrock: у Java «выдвинут» — свойство блока `extended`, а у
// Bedrock такого свойства нет: основание — блок `piston` с одним лишь
// `facing_direction`, голова — блок `piston_arm_collision`, а сам рычаг
// рисует блок-актор `PistonArm` на основании (minecraft.wiki, Piston,
// Bedrock Edition: «piston block entity», поля State, NewState, Progress,
// LastProgress, Sticky, AttachedBlocks, BreakBlocks). Без актора клиент
// видит одно основание и невидимую голову — поршень «выдвигается» впустую.
//
// Здесь для каждого основания, которое у Java выдвинуто или втянуто,
// собирается Block Actor Data. Анимации толкания (moving_block с актором
// MovingBlock) нет: основание сразу в конечном положении — клиент видит
// выдвинутый рычаг и сдвинутые блоки сразу. [проверить] живым клиентом.

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

/// Block Actor Data основания: `PistonArm` в покое — выдвинут (состояние 2,
/// ход 1) или втянут (состояние 0, ход 0).
pub fn actor_packet((x, y, z): (i32, i32, i32), (sticky, extended): Base) -> Vec<u8> {
    let state = if extended { 2 } else { 0 };
    let progress = if extended { 1.0 } else { 0.0 };
    let mut out = Out::packet(session::BLOCK_ACTOR_DATA);

    out.block_position(x, y, z);
    out.u8(COMPOUND).string("");
    out.u8(STRING).string("id").string("PistonArm");
    out.u8(INT).string("x").zigzag32(x);
    out.u8(INT).string("y").zigzag32(y);
    out.u8(INT).string("z").zigzag32(z);
    out.u8(BYTE).string("isMovable").u8(1);
    out.u8(BYTE).string("Sticky").u8(u8::from(sticky));
    out.u8(BYTE).string("State").u8(state);
    out.u8(BYTE).string("NewState").u8(state);
    out.u8(FLOAT).string("Progress").lf32(progress);
    out.u8(FLOAT).string("LastProgress").lf32(progress);
    // Пустые списки: тип элемента — int, длина 0.
    out.u8(LIST).string("AttachedBlocks").u8(INT).zigzag32(0);
    out.u8(LIST).string("BreakBlocks").u8(INT).zigzag32(0);
    out.u8(0);
    out.bytes
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

    #[test]
    fn extended_base_says_so_in_its_actor() {
        let extended = actor_packet((1, 64, 2), (true, true));
        let retracted = actor_packet((1, 64, 2), (true, false));

        assert_ne!(extended, retracted);
        assert!(extended.windows(9).any(|w| w == b"PistonArm"));
    }
}
