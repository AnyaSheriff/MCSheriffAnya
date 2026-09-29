// Остатки бастионов: где стоят, из каких кусков и чем кладутся.
//
// Всё — по замеру мира оригинала 26.1.2 (tools/measure/nether_bastion.py,
// 40 бастионов): сервер оригинала только складывал мир, а куски, их
// образцы, точки и повороты сняты с region-файлов — с блоков и с записей
// о строениях в чанке старта. Кода игры здесь нет.
//
// Где: в регионах 27×27 чанков, где не выпала крепость (nether_fortress.rs;
// в дельтах бастионов нет). Первый кусок стоит углом чанка старта на y 32
// (замер: у всех 40), размах — до 49 блоков от угла.
//
// Как: бастион оригинала собирается из образцов по пулам. Замер: четыре
// вида поровну по 11 из 40, кроме жилых блоков (7) — «treasure» (сокровищница,
// в среднем 173 куска), «hoglin_stable» (конюшни, 97), «bridge» (мост, 49),
// «units» (жилые блоки, 80); 165 разных образцов. У нас раскладка
// бастиона — одна из 40 снятых целиком (образцы, точки, повороты — как у
// оригинала), повёрнутая на случайную четверть вокруг угла чанка старта:
// у оригинала поворот первого куска тоже случайный, а остальное от него
// строится так же. Своей сборки по пулам пока нет [проверить: частоты
// образцов у нас — те же 40 выборок].
//
// Чем: образцы — src/world/nether_bastion.txt, снятые с тех же бастионов
// в своём повороте: блок (самый частый по экземплярам), «пусто» (образец
// режет рельеф — сверено с миром того же семени без строений) или «рельеф
// остаётся». Порча как у оригинала — часть кирпича трескается, часть
// резного выпадает, позолота тускнеет — по замеренным долям. Сундуки
// пустые, существ нет — как у крепостей.
//
// Раскладка зависит только от семени и чанка старта, поэтому каждый чанк
// сам строит ближние бастионы и кладёт только то, что в него попало.

use std::sync::OnceLock;

use super::nether_fortress::{scramble, Canvas, Dice};
use crate::blocks;

/// На сколько чанков от чанка старта достаёт бастион (замер: до 49 блоков
/// от угла).
pub const RADIUS_CHUNKS: i32 = 4;

/// Высота первого куска у всех замеренных бастионов.
#[cfg(test)]
const BASE_Y: i32 = 32;

/// Направления по часовой стрелке — для поворота состояний.
const DIRECTIONS: [&str; 4] = ["north", "east", "south", "west"];

/// Клетка образца.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Keep,
    Air,
    Block(u16),
}

struct Template {
    size: (i32, i32, i32),
    cells: Vec<Cell>,
    /// Состояния палитры в четырёх поворотах.
    states: [Vec<i32>; 4],
    /// Порча палитры: (доля, во что — None значит пусто).
    decay: Vec<Vec<(f64, Option<i32>)>>,
}

struct Layout {
    /// Образец, dx, y, dz от угла чанка старта, поворот.
    pieces: Vec<(usize, i32, i32, i32, u8)>,
}

struct Library {
    templates: Vec<Template>,
    layouts: Vec<Layout>,
}

/// Поставленный кусок бастиона.
#[derive(Clone, Debug)]
pub struct Placed {
    template: usize,
    origin: (i32, i32, i32),
    rotation: u8,
    /// x0, y0, z0, x1, y1, z1 — включительно.
    pub bounds: [i32; 6],
}

/// Клетка (x, z) образца в мире при повороте и точке куска (замер по рамкам
/// BB: NONE (P+x, P+z), по часовой (P−z, P+x), 180° (P−x, P−z), против
/// часовой (P+z, P−x)).
fn to_world(rotation: u8, (px, pz): (i32, i32), x: i32, z: i32) -> (i32, i32) {
    match rotation & 3 {
        0 => (px + x, pz + z),
        1 => (px - z, pz + x),
        2 => (px - x, pz - z),
        _ => (px + z, pz - x),
    }
}

/// Обратно: клетка мира → клетка образца.
fn to_local(rotation: u8, (px, pz): (i32, i32), x: i32, z: i32) -> (i32, i32) {
    match rotation & 3 {
        0 => (x - px, z - pz),
        1 => (z - pz, px - x),
        2 => (px - x, pz - z),
        _ => (pz - z, x - px),
    }
}

/// Состояние, повёрнутое на `quarters` четвертей по часовой.
fn turn(text: &str, quarters: u8) -> String {
    let Some((name, rest)) = text.split_once('[') else {
        return text.to_string();
    };
    let rotate = |direction: &str| {
        DIRECTIONS
            .iter()
            .position(|known| *known == direction)
            .map(|at| DIRECTIONS[(at + quarters as usize) % 4])
    };
    let properties: Vec<String> = rest
        .trim_end_matches(']')
        .split(',')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));

            if let Some(key) = rotate(key) {
                format!("{}={}", key, value)
            } else if key == "facing" && rotate(value).is_some() {
                format!("facing={}", rotate(value).unwrap_or(value))
            } else if key == "axis" && quarters % 2 == 1 && (value == "x" || value == "z") {
                format!("axis={}", if value == "x" { "z" } else { "x" })
            } else if key == "rotation" {
                let turned = (value.parse::<u32>().unwrap_or(0) + 4 * u32::from(quarters)) % 16;
                format!("rotation={}", turned)
            } else {
                pair.to_string()
            }
        })
        .collect();

    format!("{}[{}]", name, properties.join(","))
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("nether_bastion.txt")))
}

/// Образец в разборе: размер, палитра, клетки.
type Raw = ((i32, i32, i32), Vec<String>, Vec<Cell>);

fn parse(text: &str) -> Library {
    let named = |name: &str| blocks::state_from_text(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name));
    let mut templates = Vec::new();
    let mut layouts: Vec<Layout> = Vec::new();
    let mut decays: Vec<(String, String, f64)> = Vec::new();
    let mut raw: Vec<Raw> = Vec::new();
    let mut inside = false;

    for line in text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()) {
        let words: Vec<&str> = line.split_whitespace().collect();

        match words[0] {
            "template" => {
                let number = |at: usize| words[at].parse::<i32>().expect("число в заголовке образца");
                raw.push(((number(2), number(3), number(4)), Vec::new(), Vec::new()));
                inside = true;
            }
            "degrade" => decays.push((words[1].to_string(), words[2].to_string(), words[3].parse().unwrap_or(0.0))),
            "layout" => {
                inside = false;
                layouts.push(Layout { pieces: Vec::new() });
            }
            "=" => {
                let (_, _, cells) = raw.last_mut().expect("клетки внутри образца");

                for run in &words[1..] {
                    let (symbol, count) = run.split_once('*').unwrap_or((run, "1"));
                    let cell = match symbol {
                        "." => Cell::Keep,
                        "_" => Cell::Air,
                        index => Cell::Block(index.parse().expect("номер в палитре")),
                    };
                    cells.extend(std::iter::repeat_n(cell, count.parse().expect("длина ряда")));
                }
            }
            state if inside => raw.last_mut().expect("палитра внутри образца").1.push(state.to_string()),
            _ => {
                let number = |at: usize| words[at].parse::<i32>().expect("число в раскладке");
                let layout = layouts.last_mut().expect("кусок внутри раскладки");
                layout.pieces.push((number(0) as usize, number(1), number(2), number(3), number(4) as u8));
            }
        }
    }
    for (size, palette, cells) in raw {
        assert_eq!(cells.len(), (size.0 * size.1 * size.2) as usize, "размер образца");
        let states = [0u8, 1, 2, 3].map(|quarters| palette.iter().map(|state| named(&turn(state, quarters))).collect());
        let decay = palette
            .iter()
            .map(|state| {
                decays
                    .iter()
                    .filter(|(from, _, _)| from == state)
                    .map(|(_, to, share)| (*share, (to != "air").then(|| named(to))))
                    .collect()
            })
            .collect();

        templates.push(Template { size, cells, states, decay });
    }

    Library { templates, layouts }
}

/// Раскладка бастиона со стартом в этом чанке.
pub fn plan(seed: i64, chunk_x: i32, chunk_z: i32) -> Vec<Placed> {
    let library = library();
    let mut dice = Dice::new(seed, 0xBA_5710, chunk_x, chunk_z);
    let layout = &library.layouts[dice.below(library.layouts.len() as u32) as usize];
    let quarters = dice.below(4) as u8;
    let corner = (chunk_x * 16, chunk_z * 16);

    layout
        .pieces
        .iter()
        .map(|&(template, dx, y, dz, rotation)| {
            let (px, pz) = to_world(quarters, corner, dx, dz);
            let rotation = (rotation + quarters) & 3;
            let (sx, sy, sz) = library.templates[template].size;
            let (x0, z0) = to_world(rotation, (px, pz), 0, 0);
            let (x1, z1) = to_world(rotation, (px, pz), sx - 1, sz - 1);

            Placed {
                template,
                origin: (px, y, pz),
                rotation,
                bounds: [x0.min(x1), y, z0.min(z1), x0.max(x1), y + sy - 1, z0.max(z1)],
            }
        })
        .collect()
}

/// Кладёт куски бастиона, попавшие в этот чанк, в порядке раскладки: у
/// оригинала поздний кусок перекрывает ранний.
pub fn paint(seed: i64, pieces: &[Placed], canvas: &mut Canvas) {
    let library = library();
    let (cx0, cz0) = (canvas.x0, canvas.z0);

    for piece in pieces {
        let b = piece.bounds;
        let (x0, x1, z0, z1) = (b[0].max(cx0), b[3].min(cx0 + 15), b[2].max(cz0), b[5].min(cz0 + 15));

        if x0 > x1 || z0 > z1 {
            continue;
        }

        let template = &library.templates[piece.template];
        let states = &template.states[piece.rotation as usize];
        let (sx, sy, sz) = template.size;

        for y in 0..sy {
            let wy = piece.origin.1 + y;

            for z in z0..=z1 {
                for x in x0..=x1 {
                    let (lx, lz) = to_local(piece.rotation, (piece.origin.0, piece.origin.2), x, z);
                    let Some(at) = canvas.at(x, wy, z) else {
                        continue;
                    };

                    match template.cells[((y * sz + lz) * sx + lx) as usize] {
                        Cell::Keep => {}
                        Cell::Air => canvas.blocks[at] = canvas.air,
                        Cell::Block(index) => {
                            let decay = &template.decay[index as usize];
                            let mut state = Some(states[index as usize]);

                            if !decay.is_empty() {
                                let roll = scramble((seed as u64) ^ scramble(((x as u64) << 40) ^ ((wy as u64) << 20) ^ (z as u64 & 0xF_FFFF)));
                                let mut unit = (roll >> 11) as f64 / (1u64 << 53) as f64;

                                for &(share, into) in decay {
                                    if unit < share {
                                        state = into;
                                        break;
                                    }
                                    unit -= share;
                                }
                            }
                            canvas.blocks[at] = state.unwrap_or(canvas.air);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все образцы разобрались, все состояния есть в таблице, раскладки
    /// ссылаются на существующие образцы и начинаются на y 32.
    #[test]
    fn library_parses() {
        let library = library();

        assert_eq!(library.templates.len(), 165);
        assert_eq!(library.layouts.len(), 40);

        for layout in &library.layouts {
            assert!(layout.pieces.iter().all(|piece| piece.0 < library.templates.len()));
            assert_eq!(layout.pieces[0].2, BASE_Y);
            assert_eq!((layout.pieces[0].1, layout.pieces[0].3), (0, 0));
        }
    }

    /// Поворот клетки и обратный к нему сходятся.
    #[test]
    fn rotation_round_trip() {
        for rotation in 0..4 {
            for (x, z) in [(0, 0), (3, 7), (-5, 2)] {
                let (wx, wz) = to_world(rotation, (100, -40), x, z);
                assert_eq!(to_local(rotation, (100, -40), wx, wz), (x, z));
            }
        }
    }

    /// Поворот состояний: лестница, столб, ограда.
    #[test]
    fn states_turn_clockwise() {
        assert_eq!(turn("blackstone_stairs[facing=north,half=top,shape=straight,waterlogged=false]", 1), "blackstone_stairs[facing=east,half=top,shape=straight,waterlogged=false]");
        assert_eq!(turn("basalt[axis=x]", 1), "basalt[axis=z]");
        assert_eq!(turn("basalt[axis=y]", 3), "basalt[axis=y]");
        assert_eq!(turn("iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]", 1), "iron_bars[south=true,east=false,west=false,waterlogged=false,north=true]");
    }

    /// Раскладка — только от семени и чанка; размах не выходит за радиус.
    #[test]
    fn plan_is_deterministic_and_bounded() {
        for (x, z) in [(0, 0), (7, -3), (-20, 41)] {
            let first = plan(42, x, z);
            let again = plan(42, x, z);
            assert_eq!(first.len(), again.len());
            assert!(first.iter().zip(&again).all(|(a, b)| a.bounds == b.bounds));

            for piece in &first {
                let b = piece.bounds;
                assert!(b[0] >= (x - RADIUS_CHUNKS) * 16 && b[3] < (x + RADIUS_CHUNKS + 1) * 16);
                assert!(b[2] >= (z - RADIUS_CHUNKS) * 16 && b[5] < (z + RADIUS_CHUNKS + 1) * 16);
                assert!(b[1] >= 1 && b[4] <= 126);
            }
        }
    }
}
