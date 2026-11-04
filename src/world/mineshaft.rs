// Заброшенные шахты (mineshaft и mineshaft_mesa): где стоят, из каких кусков
// складываются и чем кладутся.
//
// Где — по вики (Mineshaft, «Generation»): «0.4% chance to attempt to begin
// generating in every chunk», в любом биоме обычного мира, кроме deep dark;
// в бесплодных землях (badlands, eroded badlands, wooded badlands) — шахта
// из тёмного дуба и выше. Случай свой (своя генерация), закон — оригинала.
//
// Как — замер мира оригинала 26.1.2 (tools/measure/mineshaft.py): сервер
// оригинала только складывал мир, а куски, их размеры, связи и частоты сняты
// с записей о строениях в чанках (structures.starts minecraft:mineshaft,
// 85 шахт, 10 822 куска) и с блоков в их рамках. Кода игры здесь нет.
//
// - Начало — комната (msroom): угол в клетке (2, 2) чанка старта, ширина и
//   длина 8…13, высота 5…10. Выходы по стенам в порядке север, юг, запад,
//   восток: шаг по стене на случайное 0…длина−1, выход, если влезает 3
//   клетки; после выхода шаг ещё на 4. Высота выхода — 1 + случайное
//   0…высота−5 над полом (замер: при высоте 5 всегда 1, при 10 — до 6).
// - К выходу пристраивается кусок: коридор 70 %, лестница 10 %, развилка
//   20 % (замер у концов коридоров и развилок: 72/9/19). Кусок не встаёт,
//   если задевает уже стоящий, если до него дальше 80 блоков от угла комнаты
//   по x или z (замер: ровно 80) и глубже 9-го колена (замер: GD 0…9).
//   Порядок — вглубь (запись строения: потомки идут сразу за родителем).
// - Коридор (mscorridor): 3×3, длина 5·n, n — 2…4, а если не влез — короче
//   до 1 (замер n: 1 — 616, 2 — 2735, 3 — 2383, 4 — 2054). Продолжение
//   одно: прямо (половина) со сдвигом по высоте −1…1, либо вбок налево или
//   направо (по четверти) с серединой в 2–3 клетках от конца (замер:
//   прямо 39 %, налево 19 %, направо 21 %, ничего 21 % — упёрлись).
//   Кроме того, у каждого стыка звеньев с каждой стороны в четверти случаев
//   боковая ветка на два колена глубже (замер: 23 % на сторону, GD +2).
//   Рельсы у трети коридоров (замер 33,7 %), паутина с пауками-спавнером —
//   у 3,2 %, те и другие вместе не бывают.
// - Развилка (mscrossing): крест 5×5 высотой 3, в четверти случаев 7 (два
//   этажа, замер 23,6 %); выходы на трёх сторонах, кроме входа, в порядке
//   север, юг, запад, восток; у двухэтажной ещё по половине на каждой из
//   четырёх сторон верхнего этажа.
// - Лестница (msstairs): 3×8×9, вниз на 5; выход на нижнем конце.
// - Шахта целиком сдвигается по высоте: верх не выше 52, низ не ниже −62
//   (замер: верх −44…52, низ −62…31).
//
// Чем — замер блоков в рамках (чистые коридоры, где пол и проход целы):
// - проход 3×3 вырезан, верхний ряд — в 86 % (иначе остаётся камень);
// - через каждые 5 блоков (третья клетка звена) опора: два столба забора
//   по бокам и балка из трёх досок, средняя доска в 75 %; опоры нет в 14 %,
//   а если над балкой пусто — в 72 % (замер 372 из 2680 и 143 из 198);
// - факел на балке с той или другой стороны — 3,3 % на сторону;
// - рельсы посередине пола — 70 % клеток у коридора с рельсами;
// - паутина под потолком по бокам — 6 % (замер 7 %, у нас без соседа-камня
//   её чаще нет), у коридора с пауками внизу 60 %;
// - вагонетка с сундуком — на боковой клетке 2-й или 4-й клетки звена с
//   рельсой под ней, 0,75 % на место (замер: 4,4 % коридоров с вагонеткой);
// - развилка — четыре столба из досок на внутренних углах креста;
// - над пустотой пол из досок (мост), под опорой над пустотой — столб из
//   брёвен вниз [проверить: длина и цепи сверху].
//
// Раскладка зависит только от семени и чанка старта, поэтому каждый чанк сам
// находит ближние шахты и кладёт только то, что в него попало.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::blocks;
use crate::loot::Dice;

/// Как далеко от угла комнаты встают куски (замер: 80 блоков).
const REACH: i32 = 80;

/// Самое глубокое колено (замер: GD 0…9).
const DEPTH: u32 = 9;

/// На сколько чанков от старта может дотянуться шахта: 80 блоков и кусок.
pub const RADIUS_CHUNKS: i32 = 7;

/// Пределы по высоте (замер: верх до 52, низ от −62).
const TOP: i32 = 52;
const BOTTOM: i32 = -62;

/// Вид куска.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Room,
    Corridor,
    Crossing,
    Stairs,
}

/// Кусок шахты: рамка (x0, y0, z0, x1, y1, z1 включительно) и куда смотрит
/// (0 юг +z, 1 запад −x, 2 север −z, 3 восток +x).
#[derive(Clone, Debug)]
pub struct Piece {
    pub kind: Kind,
    pub bb: [i32; 6],
    pub facing: u8,
    /// Колено (у комнаты 0).
    pub depth: u32,
    /// Звеньев у коридора.
    pub sections: i32,
    /// Два этажа у развилки.
    pub two_floors: bool,
    /// Рельсы и паутина с пауками у коридора.
    pub rails: bool,
    pub spiders: bool,
}

impl Piece {
    fn hits(&self, bb: &[i32; 6]) -> bool {
        self.bb[0] <= bb[3] && bb[0] <= self.bb[3] && self.bb[1] <= bb[4] && bb[1] <= self.bb[4] && self.bb[2] <= bb[5] && bb[2] <= self.bb[5]
    }

    /// Клетка куска (f — вперёд от входа, s — вбок, y — от низа рамки).
    fn at(&self, f: i32, s: i32, y: i32) -> (i32, i32, i32) {
        let b = &self.bb;

        match self.facing {
            0 => (b[0] + s, b[1] + y, b[2] + f),
            2 => (b[0] + s, b[1] + y, b[5] - f),
            1 => (b[3] - f, b[1] + y, b[2] + s),
            _ => (b[0] + f, b[1] + y, b[2] + s),
        }
    }

    /// Длина вдоль взгляда.
    fn length(&self) -> i32 {
        if self.facing.is_multiple_of(2) { self.bb[5] - self.bb[2] + 1 } else { self.bb[3] - self.bb[0] + 1 }
    }
}

/// Шахта: куски и проёмы комнаты.
pub struct Plan {
    pub pieces: Vec<Piece>,
    pub entrances: Vec<[i32; 6]>,
    pub mesa: bool,
}

const FORWARD: [(i32, i32); 4] = [(0, 1), (-1, 0), (0, -1), (1, 0)];

/// Порядок сторон у комнаты и развилки (замер: север, юг, запад, восток).
const ORDER: [u8; 4] = [2, 0, 1, 3];

/// Рамка куска шириной `width`, длиной `length`, высотой `height`, если его
/// ближний конец серединой в клетке (x, y, z) и он смотрит в `facing`.
fn frame(x: i32, y: i32, z: i32, facing: u8, width: i32, length: i32, height: i32) -> [i32; 6] {
    let half = width / 2;

    match facing {
        0 => [x - half, y, z, x + half, y + height - 1, z + length - 1],
        2 => [x - half, y, z - length + 1, x + half, y + height - 1, z],
        1 => [x - length + 1, y, z - half, x, y + height - 1, z + half],
        _ => [x, y, z - half, x + length - 1, y + height - 1, z + half],
    }
}

struct Builder<'a> {
    dice: &'a mut Dice,
    pieces: Vec<Piece>,
    corner: (i32, i32),
}

impl Builder<'_> {
    fn free(&self, bb: &[i32; 6]) -> bool {
        !self.pieces.iter().any(|piece| piece.hits(bb))
    }

    /// Пристраивает кусок к выходу (x, y, z) в сторону `facing`; true — встал.
    fn attach(&mut self, x: i32, y: i32, z: i32, facing: u8, depth: u32) -> bool {
        if depth > DEPTH || (x - self.corner.0).abs() > REACH || (z - self.corner.1).abs() > REACH {
            return false;
        }

        let pick = self.dice.next() % 100;
        let piece = if pick >= 80 {
            let two_floors = self.dice.next().is_multiple_of(4);
            let bb = frame(x, y, z, facing, 5, 5, if two_floors { 7 } else { 3 });

            self.free(&bb).then_some(Piece { kind: Kind::Crossing, bb, facing, depth, sections: 0, two_floors, rails: false, spiders: false })
        } else if pick >= 70 {
            let bb = frame(x, y - 5, z, facing, 3, 9, 8);

            self.free(&bb).then_some(Piece { kind: Kind::Stairs, bb, facing, depth, sections: 0, two_floors: false, rails: false, spiders: false })
        } else {
            let wanted = 2 + (self.dice.next() % 3) as i32;
            let rails = self.dice.next() % 1000 < 337;
            let spiders = !rails && self.dice.next() % 1000 < 48;

            (1..=wanted).rev().map(|sections| frame(x, y, z, facing, 3, sections * 5, 3)).find(|bb| self.free(bb)).map(|bb| {
                let sections = if facing.is_multiple_of(2) { bb[5] - bb[2] + 1 } else { bb[3] - bb[0] + 1 } / 5;

                Piece { kind: Kind::Corridor, bb, facing, depth, sections, two_floors: false, rails, spiders }
            })
        };

        let Some(piece) = piece else {
            return false;
        };

        self.pieces.push(piece);
        self.grow(self.pieces.len() - 1);
        true
    }

    /// Выходы куска — вглубь, сразу после постановки.
    fn grow(&mut self, index: usize) {
        let piece = self.pieces[index].clone();
        let depth = piece.depth + 1;

        match piece.kind {
            Kind::Room => {}
            Kind::Corridor => {
                let length = piece.length();
                let lift = (self.dice.next() % 3) as i32 - 1;

                match self.dice.next() % 4 {
                    0 | 1 => {
                        let (x, y, z) = piece.at(length, 1, lift);

                        self.attach(x, y, z, piece.facing, depth);
                    }
                    side => {
                        let middle = length - 2 - (self.dice.next() % 2) as i32;
                        let (s, facing) = sideways(piece.facing, side == 2);
                        let (x, y, z) = piece.at(middle, s, lift);

                        self.attach(x, y, z, facing, depth);
                    }
                }

                // Побочные ветки у стыков звеньев: с каждой стороны в
                // четверти случаев, серединой в 5k−1…5k, на два колена
                // глубже (замер: у звена k ~23 % на сторону, GD +2,
                // высота та же).
                for section in 1..piece.sections {
                    for left in [true, false] {
                        if !self.dice.next().is_multiple_of(4) {
                            continue;
                        }

                        let middle = section * 5 - 1 + (self.dice.next() % 2) as i32;
                        let (s, facing) = sideways(piece.facing, left);
                        let (x, y, z) = piece.at(middle, s, 0);

                        self.attach(x, y, z, facing, depth + 1);
                    }
                }
            }
            Kind::Crossing => {
                let (cx, cz) = ((piece.bb[0] + piece.bb[3]) / 2, (piece.bb[2] + piece.bb[5]) / 2);
                let back = (piece.facing + 2) % 4;

                for facing in ORDER {
                    if facing == back {
                        continue;
                    }

                    let (dx, dz) = FORWARD[usize::from(facing)];

                    self.attach(cx + 3 * dx, piece.bb[1], cz + 3 * dz, facing, depth);
                }

                if piece.two_floors {
                    for facing in ORDER {
                        if self.dice.next().is_multiple_of(2) {
                            let (dx, dz) = FORWARD[usize::from(facing)];

                            self.attach(cx + 3 * dx, piece.bb[1] + 4, cz + 3 * dz, facing, depth);
                        }
                    }
                }
            }
            Kind::Stairs => {
                let (x, y, z) = piece.at(9, 1, 0);

                self.attach(x, y, z, piece.facing, depth);
            }
        }
    }
}

/// Боковая клетка и сторона для бокового выхода: `left` — к меньшей
/// координате оси поперёк.
fn sideways(facing: u8, left: bool) -> (i32, u8) {
    match (facing.is_multiple_of(2), left) {
        (true, true) => (-1, 1),
        (true, false) => (3, 3),
        (false, true) => (-1, 2),
        (false, false) => (3, 0),
    }
}

/// Шахта со стартом в чанке: куски по замеру, сдвинутые по высоте.
pub fn plan(seed: i64, chunk_x: i32, chunk_z: i32, mesa: bool, surface: i32) -> Plan {
    let mut dice = Dice::new(roll(seed, chunk_x, chunk_z, 0x5AF7_0001));
    let (x0, z0) = (chunk_x * 16 + 2, chunk_z * 16 + 2);
    let width = 8 + (dice.next() % 6) as i32;
    let height = 5 + (dice.next() % 6) as i32;
    let depth_z = 8 + (dice.next() % 6) as i32;
    let room = Piece { kind: Kind::Room, bb: [x0, 50, z0, x0 + width - 1, 50 + height - 1, z0 + depth_z - 1], facing: 0, depth: 0, sections: 0, two_floors: false, rails: false, spiders: false };
    let mut builder = Builder { dice: &mut dice, pieces: vec![room.clone()], corner: (x0, z0) };
    let mut entrances = Vec::new();

    for facing in ORDER {
        let along = if facing.is_multiple_of(2) { width } else { depth_z };
        let mut at = 0;

        loop {
            at += (builder.dice.next() % along as u64) as i32;

            if at + 3 > along {
                break;
            }

            let y = room.bb[1] + 1 + (builder.dice.next() % (height - 4).max(1) as u64) as i32;
            let (x, z) = match facing {
                2 => (x0 + at + 1, z0 - 1),
                0 => (x0 + at + 1, room.bb[5] + 1),
                1 => (x0 - 1, z0 + at + 1),
                _ => (room.bb[3] + 1, z0 + at + 1),
            };
            let before = builder.pieces.len();

            if builder.attach(x, y, z, facing, 1) {
                let child = &builder.pieces[before];
                let (a, b) = if facing.is_multiple_of(2) { (child.bb[0], child.bb[3]) } else { (child.bb[2], child.bb[5]) };
                let r = &room.bb;

                // Проём — две клетки комнаты у стены, во всю ширину куска.
                entrances.push(match facing {
                    2 => [a, y, r[2], b, y + 2, r[2] + 1],
                    0 => [a, y, r[5] - 1, b, y + 2, r[5]],
                    1 => [r[0], y, a, r[0] + 1, y + 2, b],
                    _ => [r[3] - 1, y, a, r[3], y + 2, b],
                });
            }

            at += 4;
        }
    }

    let mut pieces = builder.pieces;
    let low = pieces.iter().map(|piece| piece.bb[1]).min().unwrap_or(50);
    let high = pieces.iter().map(|piece| piece.bb[4]).max().unwrap_or(50);
    let span = high - low;
    // Обычная — целиком ниже: верх случайно между низом мира и 52. В
    // бесплодных землях — у поверхности [проверить: замер mesa].
    let shift = if mesa {
        // Бесплодные земли: пол комнаты между уровнем моря и поверхностью
        // (замер 33 шахт: пол комнаты 59…127, чаще всего вровень с
        // поверхностью до 66 или 63…83 под горой) [проверить: закон].
        let floor = 63 + (dice.next() % (surface - 62).max(1) as u64) as i32;

        floor - room.bb[1]
    } else {
        let lowest = BOTTOM + span;
        let top = if lowest >= TOP { TOP } else { lowest + (dice.next() % (TOP - lowest + 1) as u64) as i32 };

        top - high
    };

    for piece in &mut pieces {
        piece.bb[1] += shift;
        piece.bb[4] += shift;
    }

    for entrance in &mut entrances {
        entrance[1] += shift;
        entrance[4] += shift;
    }

    Plan { pieces, entrances, mesa }
}

include!("mineshaft_start.rs");

/// Шахта со стартом в чанке — из памяти, если уже считана.
fn cached(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Option<std::sync::Arc<Plan>> {
    type Plans = Mutex<HashMap<(i64, u8, i32, i32), Option<std::sync::Arc<Plan>>>>;
    static CACHE: OnceLock<Plans> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (terrain.seed(), terrain.generation_profile(), chunk_x, chunk_z);

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let found = start_at(terrain, chunk_x, chunk_z).map(|(mesa, surface)| std::sync::Arc::new(plan(terrain.seed(), chunk_x, chunk_z, mesa, surface)));
    let mut cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());

    if cache.len() > 4096 {
        cache.clear();
    }

    cache.insert(key, found.clone());
    found
}

/// Шахты, чьи куски могут задеть чанк.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<std::sync::Arc<Plan>> {
    let mut out = Vec::new();

    for x in chunk_x - RADIUS_CHUNKS..=chunk_x + RADIUS_CHUNKS {
        for z in chunk_z - RADIUS_CHUNKS..=chunk_z + RADIUS_CHUNKS {
            if !eligible_for(terrain, x, z) {
                continue;
            }

            if let Some(plan) = cached(terrain, x, z) {
                out.push(plan);
            }
        }
    }

    out
}

/// Что ставится в клетку.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mark {
    /// Вырезать.
    Air,
    /// Доска (столб развилки).
    Planks,
    /// Доска балки опоры; с клеткой — опора хрупкая: не ставится, если
    /// клетка над балкой не твёрдая.
    Beam(Option<(i32, i32, i32)>),
    /// Столб забора опоры: смотрит наружу в сторону (0..4); клетка — как у
    /// балки.
    Fence(u8, Option<(i32, i32, i32)>),
    /// Рельса под вагонеткой с сундуком.
    Cart(bool),
    /// Рельса вдоль оси (true — север-юг).
    Rail(bool),
    /// Факел на стене, смотрит в сторону.
    Torch(u8),
    Cobweb,
    Spawner,
    /// Пол-мост: доска, если под проходом пусто.
    Floor,
    /// Столб из брёвен вниз до твёрдого, если под опорой пусто.
    Pillar,
}

/// Клетки куска по порядку: что и куда (случай — только от семени и рамки,
/// чтобы соседние чанки видели одно и то же).
fn marks(seed: i64, piece: &Piece, entrances: &[[i32; 6]]) -> Vec<((i32, i32, i32), Mark)> {
    let mut dice = Dice::new(roll(seed, piece.bb[0] ^ (piece.bb[1] << 12), piece.bb[2], 0x5AF7_0002));
    let mut out = Vec::new();

    match piece.kind {
        Kind::Room => {
            let b = &piece.bb;
            let (w, d) = (b[3] - b[0] + 1, b[5] - b[2] + 1);
            let dome = (b[4] - b[1] + 1 - 5).max(0);
            let (mx, mz) = (f64::from(b[0] + b[3]) / 2.0, f64::from(b[2] + b[5]) / 2.0);

            for x in b[0]..=b[3] {
                for z in b[2]..=b[5] {
                    for y in 1..=4 {
                        out.push(((x, b[1] + y, z), Mark::Air));
                    }

                    // Свод — верхняя половина эллипсоида над коробом.
                    for up in 1..=dome {
                        let (ex, ez, ey) = ((f64::from(x) - mx) / (f64::from(w) / 2.0), (f64::from(z) - mz) / (f64::from(d) / 2.0), f64::from(up) / f64::from(dome + 1));

                        if ex * ex + ez * ez + ey * ey <= 1.0 {
                            out.push(((x, b[1] + 4 + up, z), Mark::Air));
                        }
                    }
                }
            }

            for e in entrances {
                for x in e[0]..=e[3] {
                    for y in e[1]..=e[4] {
                        for z in e[2]..=e[5] {
                            out.push(((x, y, z), Mark::Air));
                        }
                    }
                }
            }
        }
        Kind::Corridor => {
            let length = piece.sections * 5;
            let along_z = piece.facing.is_multiple_of(2);
            let (left, right) = if along_z { (1u8, 3u8) } else { (2u8, 0u8) };

            for f in 0..length {
                let support = f % 5 == 2;

                for s in 0..3 {
                    out.push((piece.at(f, s, -1), Mark::Floor));
                    out.push((piece.at(f, s, 0), Mark::Air));
                    out.push((piece.at(f, s, 1), Mark::Air));

                    if support || dice.next() % 100 < 86 {
                        out.push((piece.at(f, s, 2), Mark::Air));
                    }
                }
            }

            for section in 0..piece.sections {
                let f = section * 5 + 2;
                let skip = dice.next() % 100;
                let open_skip = dice.next() % 100;

                if skip < 14 {
                    continue;
                }

                // Над балкой пусто — опора пропадает в 72 % (замер).
                let fragile = (open_skip < 72).then(|| piece.at(f, 1, 3));

                out.push((piece.at(f, 0, 0), Mark::Fence(left, fragile)));
                out.push((piece.at(f, 0, 1), Mark::Fence(left, fragile)));
                out.push((piece.at(f, 2, 0), Mark::Fence(right, fragile)));
                out.push((piece.at(f, 2, 1), Mark::Fence(right, fragile)));
                out.push((piece.at(f, 0, -1), Mark::Pillar));
                out.push((piece.at(f, 2, -1), Mark::Pillar));
                out.push((piece.at(f, 0, 2), Mark::Beam(fragile)));
                out.push((piece.at(f, 2, 2), Mark::Beam(fragile)));

                if !dice.next().is_multiple_of(4) {
                    out.push((piece.at(f, 1, 2), Mark::Beam(fragile)));
                }

                // Факелы на балке: смотрят от неё вдоль коридора.
                let ahead = piece.facing;
                let behind = (piece.facing + 2) % 4;

                if dice.next() % 1000 < 33 {
                    out.push((piece.at(f - 1, 1, 2), Mark::Torch(behind)));
                }

                if dice.next() % 1000 < 33 {
                    out.push((piece.at(f + 1, 1, 2), Mark::Torch(ahead)));
                }
            }

            for f in 0..length {
                if piece.spiders {
                    for s in 0..3 {
                        for y in 0..2 {
                            if dice.next() % 100 < 60 {
                                out.push((piece.at(f, s, y), Mark::Cobweb));
                            }
                        }
                    }
                }

                for s in [0, 2] {
                    if dice.next() % 100 < if piece.spiders { 10 } else { 6 } {
                        out.push((piece.at(f, s, 2), Mark::Cobweb));
                    }
                }

                if piece.rails && dice.next() % 100 < 70 {
                    out.push((piece.at(f, 1, 0), Mark::Rail(along_z)));
                }
            }

            if piece.spiders {
                let f = 5 * (dice.next() % piece.sections as u64) as i32 + 1 + (dice.next() % 3) as i32;

                out.push((piece.at(f, 1, 0), Mark::Spawner));
            }

            for f in 0..length {
                if matches!(f % 5, 1 | 3) && dice.next() % 10_000 < 75 {
                    let s = if dice.next().is_multiple_of(2) { 0 } else { 2 };

                    out.push((piece.at(f, s, 0), Mark::Cart(along_z)));
                }
            }
        }
        Kind::Crossing => {
            let height = if piece.two_floors { 7 } else { 3 };

            for f in 0..5 {
                for s in 0..5 {
                    let inside = (1..=3).contains(&s) || (1..=3).contains(&f);

                    if !inside {
                        continue;
                    }

                    out.push((piece.at(f, s, -1), Mark::Floor));

                    for y in 0..height {
                        let pillar = matches!(f, 1 | 3) && matches!(s, 1 | 3);

                        out.push((piece.at(f, s, y), if pillar { Mark::Planks } else { Mark::Air }));
                    }
                }
            }
        }
        Kind::Stairs => {
            // Замер: пол и верх прохода по клеткам вдоль лестницы.
            const PROFILE: [(i32, i32); 9] = [(5, 7), (5, 7), (4, 7), (3, 6), (2, 5), (1, 4), (1, 3), (0, 2), (0, 2)];

            for (f, (low, high)) in PROFILE.into_iter().enumerate() {
                for s in 0..3 {
                    for y in low..=high {
                        out.push((piece.at(f as i32, s, y), Mark::Air));
                    }
                }
            }
        }
    }

    out
}

/// Вагонетки с сундуком куска: где (клетка с рельсой) и случай добычи.
fn carts_of(seed: i64, piece: &Piece) -> Vec<((i32, i32, i32), u64)> {
    if piece.kind != Kind::Corridor {
        return Vec::new();
    }

    marks(seed, piece, &[])
        .into_iter()
        .filter(|(_, mark)| matches!(mark, Mark::Cart(_)))
        .map(|(at, _)| (at, roll(seed, at.0, at.2, 0x5AF7_0003 ^ (at.1 as u64))))
        .collect()
}

/// Материалы: дуб или тёмный дуб (бесплодные земли).
struct Wood {
    planks: i32,
    log: i32,
    fence: String,
}

fn wood(mesa: bool) -> Wood {
    let name = if mesa { "dark_oak" } else { "oak" };

    Wood { planks: blocks::state_by_name(&format!("{name}_planks")).unwrap_or(0), log: blocks::state_by_name(&format!("{name}_log")).unwrap_or(0), fence: format!("{name}_fence") }
}

/// Хрупкая опора без твёрдого над балкой (клетка в другом чанке — считаем
/// твёрдой).
fn weak(chunk: &super::Chunk, (x0, z0): (i32, i32), above: Option<(i32, i32, i32)>) -> bool {
    let Some((x, y, z)) = above else {
        return false;
    };

    (x0..x0 + 16).contains(&x) && (z0..z0 + 16).contains(&z) && !solid(chunk.block(x - x0, y, z - z0))
}

fn liquid(state: i32) -> bool {
    blocks::block_at_state(state).is_some_and(|name| matches!(name, "water" | "lava"))
}

fn solid(state: i32) -> bool {
    blocks::full_cube(state)
}

/// Кладёт в сложенный чанк части ближних шахт — после пещер.
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let plans = near(terrain, chunk_x, chunk_z);

    if plans.is_empty() {
        return;
    }

    let seed = terrain.seed();
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);
    let edge = [x0, z0, x0 + 15, z0 + 15];
    let cave_air = blocks::state_by_name("cave_air").unwrap_or(super::AIR);
    let cobweb = blocks::state_by_name("cobweb").unwrap_or(0);
    let spawner = blocks::state_by_name("spawner").unwrap_or(0);
    let chain = blocks::state_by_name("iron_chain").unwrap_or(0);
    // Верх рельефа по столбцам — до шахт: дерево под открытым небом не
    // ставится (вики: «If directly exposed to the sky during generation, no
    // wooden parts spawn at that exact location»).
    let mut tops = [super::MIN_Y; 256];

    for lx in 0..16 {
        for lz in 0..16 {
            let mut y = super::MIN_Y + super::WORLD_HEIGHT - 1;

            while y > super::MIN_Y && chunk.block(lx, y, lz) == super::AIR {
                y -= 1;
            }

            tops[(lz * 16 + lx) as usize] = y;
        }
    }

    for plan in plans {
        let wood = wood(plan.mesa);

        for piece in &plan.pieces {
            if piece.bb[0] > edge[2] || piece.bb[3] < edge[0] || piece.bb[2] > edge[3] || piece.bb[5] < edge[1] {
                continue;
            }

            // Кусок у воды или лавы не кладётся (замер: такие куски пусты)
            // [проверить: у оригинала стенка от водоносного слоя].
            let wet = (piece.bb[0].max(edge[0])..=piece.bb[3].min(edge[2])).any(|x| {
                (piece.bb[2].max(edge[1])..=piece.bb[5].min(edge[3])).any(|z| (piece.bb[1]..=piece.bb[4]).any(|y| liquid(chunk.block(x - x0, y, z - z0))))
            });

            if wet {
                continue;
            }

            let marks = marks(seed, piece, if piece.kind == Kind::Room { &plan.entrances } else { &[] });
            // Висит ли кусок в воздухе целиком: под полом в этом чанке нет
            // ни одной твёрдой клетки (вики, 21w06a: «no longer generate if
            // they were fully floating in the air»).
            let floor: Vec<(i32, i32, i32)> = marks.iter().filter(|(at, mark)| *mark == Mark::Floor && (x0..x0 + 16).contains(&at.0) && (z0..z0 + 16).contains(&at.2)).map(|(at, _)| *at).collect();

            if !floor.is_empty() && !floor.iter().any(|&(x, y, z)| solid(chunk.block(x - x0, y, z - z0))) {
                continue;
            }

            let open = |x: i32, y: i32, z: i32| y > tops[((z - z0) * 16 + (x - x0)) as usize];

            for &((x, y, z), mark) in &marks {
                if !(x0..x0 + 16).contains(&x) || !(z0..z0 + 16).contains(&z) || !(super::MIN_Y + 1..super::MIN_Y + super::WORLD_HEIGHT).contains(&y) {
                    continue;
                }

                let (lx, lz) = (x - x0, z - z0);
                let here = chunk.block(lx, y, lz);

                if blocks::block_at_state(here) == Some("bedrock") {
                    continue;
                }

                match mark {
                    Mark::Air => {
                        if here != super::AIR {
                            chunk.put_generated(lx, y, lz, cave_air, true);
                        }
                    }
                    Mark::Planks if !open(x, y, z) => chunk.put_generated(lx, y, lz, wood.planks, true),
                    Mark::Beam(fragile) if !open(x, y, z) && !weak(chunk, (x0, z0), fragile) => chunk.put_generated(lx, y, lz, wood.planks, true),
                    Mark::Fence(outward, fragile) if !open(x, y, z) && !weak(chunk, (x0, z0), fragile) => {
                        let (dx, dz) = FORWARD[usize::from(outward)];
                        let side = ["south", "west", "north", "east"][usize::from(outward)];
                        let joined = (0..16).contains(&(lx + dx)) && (0..16).contains(&(lz + dz)) && solid(chunk.block(lx + dx, y, lz + dz));
                        let text = if joined { format!("{}[{side}=true]", wood.fence) } else { wood.fence.clone() };

                        chunk.put_generated(lx, y, lz, blocks::state_from_text(&text).unwrap_or(wood.planks), true);
                    }
                    Mark::Rail(along_z) | Mark::Cart(along_z) => {
                        let text = if along_z { "rail[shape=north_south]" } else { "rail[shape=east_west]" };

                        if solid(chunk.block(lx, y - 1, lz)) {
                            chunk.put_generated(lx, y, lz, blocks::state_from_text(text).unwrap_or(0), true);
                        }
                    }
                    Mark::Torch(facing) => {
                        let side = ["south", "west", "north", "east"][usize::from(facing)];

                        if !open(x, y, z) {
                            chunk.put_generated(lx, y, lz, blocks::state_from_text(&format!("wall_torch[facing={side}]")).unwrap_or(0), true);
                        }
                    }
                    Mark::Cobweb => {
                        // Паутина — только при твёрдом соседе (вики: «Cobwebs
                        // do not generate if they would not be connected to
                        // solid blocks»).
                        let touching = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]
                            .iter()
                            .any(|(dx, dy, dz)| (0..16).contains(&(lx + dx)) && (0..16).contains(&(lz + dz)) && solid(chunk.block(lx + dx, y + dy, lz + dz)));

                        if touching {
                            chunk.put_generated(lx, y, lz, cobweb, true);
                        }
                    }
                    Mark::Spawner => {
                        chunk.put_generated(lx, y, lz, spawner, true);
                        chunk.put_generated_spawner(lx, y, lz, crate::spawner::Data::of("cave_spider"));
                    }
                    Mark::Floor => {
                        if !solid(here) && !open(x, y, z) {
                            chunk.put_generated(lx, y, lz, wood.planks, true);
                        }
                    }
                    Mark::Pillar => {
                        // Под опорой над пустотой — брёвна вниз до твёрдого
                        // (вики: «supported by log pillars below»), а нет
                        // дна в 9 блоках — цепи над балкой до потолка
                        // («suspended with iron chains above») [проверить:
                        // длины]. Клетка под полом уже может быть мостом.
                        // Только под мостом: пол опоры был пуст и стал доской.
                        if here != wood.planks || solid(chunk.block(lx, y - 1, lz)) || open(x, y, z) {
                            continue;
                        }

                        let bottom = (1..10).map(|depth| y - depth).find(|&down| down > super::MIN_Y && solid(chunk.block(lx, down, lz)));

                        if let Some(bottom) = bottom {
                            for down in bottom + 1..y {
                                chunk.put_generated(lx, down, lz, wood.log, true);
                            }
                        } else if let Some(ceiling) = (y + 4..y + 14).find(|&up| up < super::MIN_Y + super::WORLD_HEIGHT && solid(chunk.block(lx, up, lz))) {
                            for up in y + 4..ceiling {
                                chunk.put_generated(lx, up, lz, chain, true);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Вагонетки с сундуком, стоящие в чанке: середина клетки с рельсой и
/// случай добычи (их ставит такт при сложении чанка — tick.rs).
pub fn carts(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), u64)> {
    let mut out = Vec::new();

    for plan in near(terrain, chunk_x, chunk_z) {
        for piece in plan.pieces.iter().filter(|piece| piece.bb[0] < chunk_x * 16 + 16 && piece.bb[3] >= chunk_x * 16 && piece.bb[2] < chunk_z * 16 + 16 && piece.bb[5] >= chunk_z * 16) {
            for (at, luck) in carts_of(terrain.seed(), piece) {
                if at.0.div_euclid(16) == chunk_x && at.2.div_euclid(16) == chunk_z {
                    out.push((at, luck));
                }
            }
        }
    }

    out
}

/// Ближайшая шахта (`locate structure minecraft:mineshaft` или
/// `mineshaft_mesa`): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, mesa: bool, chunk_x: i32, chunk_z: i32, radius: i32) -> Option<(i32, i32)> {
    for ring in 0..=radius {
        let mut best: Option<(i32, i32)> = None;
        let distance = |(x, z): (i32, i32)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2);

        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                let (x, z) = (chunk_x + dx, chunk_z + dz);

                if start_at(terrain, x, z).is_some_and(|(kind, _)| kind == mesa) && best.is_none_or(|known| distance((x, z)) < distance(known)) {
                    best = Some((x, z));
                }
            }
        }

        if best.is_some() {
            return best;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Куски не налезают друг на друга, все в 80 блоках от угла комнаты и
    /// не глубже 9-го колена; шахта решается семенем.
    #[test]
    fn plans_keep_the_measured_rules() {
        for (x, z) in [(0, 0), (5, -7), (-40, 13), (100, 100)] {
            let plan = plan(42, x, z, false, 70);
            let again = super::plan(42, x, z, false, 70);
            let corner = (x * 16 + 2, z * 16 + 2);

            assert_eq!(plan.pieces.len(), again.pieces.len());
            assert_eq!(plan.pieces[0].kind, Kind::Room);

            for (i, a) in plan.pieces.iter().enumerate() {
                assert!(a.depth <= DEPTH);

                for b in &plan.pieces[i + 1..] {
                    assert!(!a.hits(&b.bb), "{:?} и {:?}", a.bb, b.bb);
                }

                let nearest = |low: i32, high: i32, at: i32| if (low..=high).contains(&at) { 0 } else { (low - at).abs().min((high - at).abs()) };

                assert!(nearest(a.bb[0], a.bb[3], corner.0) <= REACH + 1 && nearest(a.bb[2], a.bb[5], corner.1) <= REACH + 1);
                assert!(a.bb[1] >= BOTTOM && a.bb[4] <= TOP);
            }
        }
    }

    /// Правило места: 0,4 % чанков (вики), шахт в бесплодных землях — только
    /// там.
    #[test]
    fn starts_are_rare() {
        let starts = (-200..200).flat_map(|x| (-50..50).map(move |z| (x, z))).filter(|&(x, z)| eligible(7, x, z)).count();
        let share = starts as f64 / 40_000.0;

        assert!((share - 0.004).abs() < 0.001, "{share}");
    }

    /// Регрессия: новая проверка начала должна доходить до блоков, а не
    /// только до адаптера переписи (minecraft.wiki/w/Mineshaft: «0.4%»).
    #[test]
    fn official_start_places_wood_in_generated_chunks() {
        use super::super::{terrain, Chunk, Generator};
        let generator = Generator::Normal(terrain::Terrain::new(1_792_932_773_345_722_119, terrain::Style::Vanilla).with_preview(true));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let (cx, cz) = (-21, -31);
        assert!(start_at(terrain, cx, cz).is_some());
        let expected = cached(terrain, cx, cz).unwrap();
        assert!(near(terrain, cx, cz).iter().any(|plan| std::sync::Arc::ptr_eq(plan, &expected)));
        let mut wood = 0;
        for x in cx-1..=cx+1 {
            for z in cz-1..=cz+1 {
                let chunk = Chunk::generated(&generator, x, z);
                for y in super::super::MIN_Y..60 {
                    for bz in 0..16 {
                        for bx in 0..16 {
                            if matches!(blocks::block_at_state(chunk.block(bx, y, bz)), Some("oak_planks" | "oak_fence" | "rail")) { wood += 1; }
                        }
                    }
                }
            }
        }
        assert!(wood > 10, "{wood}");
    }

    /// Перепись кусков шахты — против замера оригинала (tools/measure/
    /// mineshaft.py stats, 85 шахт): кусков на шахту 127 (44…233), из них
    /// коридоров 72 %, развилок 19,5 %, лестниц 7,7 %; длины коридоров
    /// 1…4 звена — 8/35/31/26 %; двухэтажных развилок 23,6 %.
    #[test]
    fn mineshaft_piece_census() {
        let (mut total, mut corridors, mut crossings, mut stairs, mut two, mut rails, mut spiders) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        let mut lengths = [0usize; 5];
        let mut depths = [0usize; 10];
        let runs = 300;

        for n in 0..runs {
            let plan = plan(1234, n * 37, -n * 11, false, 70);

            total += plan.pieces.len();

            for piece in &plan.pieces {
                depths[piece.depth as usize] += 1;
            }

            for piece in &plan.pieces {
                match piece.kind {
                    Kind::Corridor => {
                        corridors += 1;
                        lengths[piece.sections as usize] += 1;
                        rails += usize::from(piece.rails);
                        spiders += usize::from(piece.spiders);
                    }
                    Kind::Crossing => {
                        crossings += 1;
                        two += usize::from(piece.two_floors);
                    }
                    Kind::Stairs => stairs += 1,
                    Kind::Room => {}
                }
            }
        }

        let per = total as f64 / f64::from(runs);
        let share = |count: usize| count as f64 / (total - runs as usize) as f64;

        println!("колена {depths:?}");
        println!("кусков {per:.1}, коридоры {:.3}, развилки {:.3}, лестницы {:.3}, длины {lengths:?}", share(corridors), share(crossings), share(stairs));
        assert!((per - 127.3).abs() < 25.0, "кусков на шахту {per}");
        assert!((share(corridors) - 0.72).abs() < 0.04);
        assert!((share(crossings) - 0.195).abs() < 0.04);
        assert!((share(stairs) - 0.077).abs() < 0.03);
        assert!((two as f64 / crossings as f64 - 0.236).abs() < 0.04);
        assert!((rails as f64 / corridors as f64 - 0.337).abs() < 0.03);
        assert!((spiders as f64 / corridors as f64 - 0.032).abs() < 0.015);

        let one = lengths[1] as f64 / corridors as f64;

        assert!((0.03..0.15).contains(&one), "коротких {one}");
    }

    /// Шахты в нашем мире: перепись блоков в рамках кусков — против замера
    /// оригинала (tools/measure/mineshaft.py census, 44 целиком сложенные
    /// шахты, обычные и бесплодные вместе, на шахту): доски 1062, забор 700,
    /// рельсы 266, паутина 226, факелы 11,7, спавнеры 2,9, брёвна 5,5
    /// (`cargo test --release mineshaft_world -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn mineshaft_world() {
        use super::super::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let starts: Vec<(i32, i32)> = (-120..120).flat_map(|x| (-12..12).map(move |z| (x, z))).filter(|&(x, z)| start_at(terrain, x, z).is_some()).take(10).collect();
        let names = ["planks", "fence", "rail", "cobweb", "wall_torch", "spawner", "log", "chain"];
        let mut total = [0u32; 8];
        let mut carts = 0;

        for &(start_x, start_z) in &starts {
            let plan = cached(terrain, start_x, start_z).expect("шахта есть");
            let mut counts = [0u32; 8];
            let mut seen = std::collections::HashSet::new();
            let mut chunks = HashMap::new();

            for piece in &plan.pieces {
                for x in piece.bb[0]..=piece.bb[3] {
                    for z in piece.bb[2]..=piece.bb[5] {
                        let chunk = chunks.entry((x.div_euclid(16), z.div_euclid(16))).or_insert_with(|| Chunk::generated(&generator, x.div_euclid(16), z.div_euclid(16)));

                        for y in piece.bb[1] - 1..=piece.bb[4] {
                            if !seen.insert((x, y, z)) {
                                continue;
                            }

                            let name = blocks::block_at_state(chunk.block(x.rem_euclid(16), y, z.rem_euclid(16))).unwrap_or("air");

                            if let Some(at) = names.iter().position(|known| name.ends_with(known) && (name.contains("oak") || !matches!(*known, "planks" | "fence" | "log"))) {
                                counts[at] += 1;
                            }
                        }
                    }
                }
            }

            for (x, z) in chunks.keys() {
                carts += carts_of_chunk(terrain, *x, *z);
            }

            println!("шахта {start_x},{start_z} ({} кусков, бесплодная {}): {}", plan.pieces.len(), plan.mesa, names.iter().zip(counts).map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(" "));

            for (sum, count) in total.iter_mut().zip(counts) {
                *sum += count;
            }
        }

        let n = starts.len() as f64;
        let pieces: f64 = starts.iter().map(|&(x, z)| cached(terrain, x, z).map_or(0, |plan| plan.pieces.len()) as f64).sum();
        // Замер оригинала на кусок (44 шахты, 129,1 куска на шахту).
        // Брёвна и цепи зависят от пещер под шахтой, а пещеры у нас свои, —
        // сверяются вместе и грубо [проверить].
        const PER_PIECE: [f64; 8] = [8.22, 5.42, 2.06, 1.75, 0.091, 0.022, 0.043, 0.056];

        for ((name, count), measured) in names.iter().zip(total).zip(PER_PIECE) {
            let ours = f64::from(count) / pieces;

            println!("{name}: на кусок у нас {ours:.3}, у оригинала {measured:.3}");

            if !matches!(*name, "log" | "chain") {
                assert!((ours - measured).abs() <= measured * 0.25 + 0.02, "{name}: у нас {ours:.3}, оригинал {measured:.3}");
            }
        }

        let props = f64::from(total[6] + total[7]) / pieces;

        assert!((props - 0.099).abs() <= 0.06, "брёвна и цепи: у нас {props:.3}, оригинал 0,099");

        println!("на шахту: {} вагонеток {:.2}", names.iter().zip(total).map(|(name, count)| format!("{name}={:.1}", f64::from(count) / n)).collect::<Vec<_>>().join(" "), f64::from(carts) / n);
        assert!(!starts.is_empty());
    }

    fn carts_of_chunk(terrain: &super::super::terrain::Terrain, x: i32, z: i32) -> u32 {
        carts(terrain, x, z).len() as u32
    }
}
