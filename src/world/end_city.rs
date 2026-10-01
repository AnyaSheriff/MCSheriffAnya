// Энд-города: где стоят, из каких кусков складываются и чем кладутся.
//
// Всё — по замеру мира оригинала 26.1.2 (tools/measure/end_city.py): сервер
// оригинала только складывал мир, а куски, их размеры, сцепки и частоты
// сняты с region-файлов — с блоков и с записей о строениях в чанках. Кода
// игры здесь нет.
//
// Где: вики (End City) — «located only in chunks numbered 0-8 ± a multiple
// of 20», то есть регионы 20×20 чанков, старт в первых девяти чанках по
// каждой оси (замер: сдвиги 1..7). Только на внешних островах, где остров
// достаточно высок: низ города — на верхе эндемняка под основанием (замер:
// высота старта 60..62), и только в верховьях и серединах (вики: «midlands
// or highlands»). Порог высоты [проверить] — у оригинала города не стоят
// на низких островках.
//
// Как (замер 30 городов, 1569 кусков): корень — дом в три этажа (base_floor,
// second_floor_1|2, third_floor_1|2, third_roof); из дома в два-три этажа
// растёт башня (tower_base, 2–4 tower_piece), сверху tower_top или (в 30 %)
// толстая башня (fat_tower_base, 1–2 fat_tower_middle, fat_tower_top). С
// каждой стороны башни в половине случаев — дверь моста (bridge_end) на
// случайном ярусе; дальше мост из 2–5 кусков (bridge_piece 61 %, крутая и
// пологая лестницы по 20 %), в конце — дом в 1–3 этажа с дверью с обратной
// стороны или (не больше одного на город) корабль. Дома за мостом снова
// растят башни и мосты. Кусок не встаёт, если задевает уже поставленный:
// тогда мост обрывается на двери (замер: 47 дверей из 164 без моста).
//
// Чем: образцы кусков — src/world/end_city.txt, снятые с тех же городов.
// Сундуки стоят пустыми, шалкеров нет (сущностей-мобов Энда у нас нет),
// элитры в рамке на корабле нет (рамка — сущность, кладётся не при
// генерации).
//
// Раскладка зависит только от семени и региона, поэтому каждый чанк сам
// строит ближние города целиком и кладёт только то, что в него попало.

use std::sync::OnceLock;

use crate::blocks;

/// Сторона региона в чанках (вики: multiple of 20).
pub const REGION: i32 = 20;

/// В скольких первых чанках региона может стоять старт (вики: 0-8).
const SPREAD: i32 = 9;

/// На сколько чанков от чанка старта достаёт город (замер: размах до 96
/// блоков, корабль — ещё до 100 блоков за мостом).
pub const RADIUS_CHUNKS: i32 = 10;

/// Ниже этого верха эндемняка под основанием города нет [проверить].
pub const MIN_TOP: i32 = 59;

/// Больше этого кусков в городе не ставится (замер: до 97).
const MAX_PIECES: usize = 110;

/// Глубже этого по мостам дома не растят башен.
const MAX_DEPTH: u32 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    /// Рельеф остаётся.
    Keep,
    /// Пусто.
    Air,
    /// Блок палитры.
    Block(u16),
}

struct Template {
    name: &'static str,
    width: i32,
    length: i32,
    height: i32,
    cells: Vec<Cell>,
    /// Состояния палитры по поворотам 0..4 (по часовой на четверть).
    states: [Vec<i32>; 4],
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> Cell {
        self.cells[((y * self.length + b) * self.width + a) as usize]
    }
}

/// Образцы из src/world/end_city.txt — разбираются один раз.
fn templates() -> &'static [Template] {
    static TEMPLATES: OnceLock<Vec<Template>> = OnceLock::new();

    TEMPLATES.get_or_init(|| parse(include_str!("end_city.txt")))
}

fn template(name: &str) -> usize {
    templates().iter().position(|t| t.name == name).unwrap_or_else(|| panic!("кусок {} описан", name))
}

/// Направление образца, повёрнутое на `rot` четвертей по часовой.
fn turn(direction: &str, rot: u8) -> &'static str {
    const RING: [&str; 4] = ["north", "east", "south", "west"];
    let at = RING.iter().position(|d| *d == direction).unwrap_or(0);
    RING[(at + rot as usize) % 4]
}

/// Состояние с направлениями образца в мире при повороте `rot`.
fn orient(text: &str, rot: u8) -> String {
    let Some((name, rest)) = text.split_once('[') else {
        return text.to_string();
    };
    let is_direction = |word: &str| matches!(word, "north" | "south" | "east" | "west");
    let mut properties: Vec<String> = rest
        .trim_end_matches(']')
        .split(',')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));

            if is_direction(key) {
                format!("{}={}", turn(key, rot), value)
            } else if key == "facing" && is_direction(value) {
                format!("facing={}", turn(value, rot))
            } else if key == "rotation" {
                format!("rotation={}", (value.parse::<i32>().unwrap_or(0) + 4 * rot as i32) % 16)
            } else {
                pair.to_string()
            }
        })
        .collect();
    properties.sort();

    format!("{}[{}]", name, properties.join(","))
}

fn parse(text: &str) -> Vec<Template> {
    let named = |name: &str| blocks::state_from_text(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name));
    let mut pieces = Vec::new();
    let mut lines = text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).peekable();

    while let Some(line) = lines.next() {
        let words: Vec<&str> = line.split_whitespace().collect();

        if words.first() != Some(&"piece") {
            continue;
        }

        let name: &'static str = Box::leak(words[1].to_string().into_boxed_str());
        let number = |at: usize| words[at].parse::<i32>().expect("число в заголовке куска");
        let (width, length, height) = (number(2), number(3), number(4));
        let mut palette: Vec<(char, String)> = vec![('#', "purpur_block".into())];
        let mut cells = Vec::with_capacity((width * length * height) as usize);

        while let Some(line) = lines.next_if(|line| !line.starts_with("piece")) {
            let trimmed = line.trim();

            if trimmed.starts_with("layer") {
                for _ in 0..length {
                    let row = lines.next().expect("строка слоя").trim();

                    for symbol in row.chars() {
                        cells.push(match symbol {
                            '.' => Cell::Keep,
                            '_' => Cell::Air,
                            symbol => Cell::Block(
                                palette.iter().position(|(key, _)| *key == symbol).expect("знак есть в палитре") as u16,
                            ),
                        });
                    }
                }
            } else if let Some((symbol, state)) = trimmed.split_once(' ') {
                palette.push((symbol.chars().next().unwrap_or('?'), state.to_string()));
            }
        }

        assert_eq!(cells.len(), (width * length * height) as usize, "размер куска {}", name);

        let states = [0u8, 1, 2, 3].map(|rot| palette.iter().map(|(_, state)| named(&orient(state, rot))).collect());

        pieces.push(Template { name, width, length, height, cells, states });
    }

    pieces
}

/// Поставленный кусок.
#[derive(Clone, Debug)]
pub struct Placed {
    template: usize,
    /// Поворот: четвертей по часовой (0 — как образец).
    rot: u8,
    /// Точка образца (угол рамки) в мире.
    tp: (i32, i32, i32),
    /// Рамка: x0, y0, z0, x1, y1, z1.
    pub bounds: [i32; 6],
}

impl Placed {
    fn new(template: usize, rot: u8, tp: (i32, i32, i32)) -> Placed {
        let t = &templates()[template];
        let corner = |a: i32, b: i32| to_world(tp, rot, a, b);
        let (p, q) = (corner(0, 0), corner(t.width - 1, t.length - 1));

        Placed {
            template,
            rot,
            tp,
            bounds: [p.0.min(q.0), tp.1, p.1.min(q.1), p.0.max(q.0), tp.1 + t.height - 1, p.1.max(q.1)],
        }
    }

    /// Клетка образца в мире.
    fn world(&self, a: i32, y: i32, b: i32) -> (i32, i32, i32) {
        let (x, z) = to_world(self.tp, self.rot, a, b);
        (x, self.tp.1 + y, z)
    }

    /// Кусок, пристроенный к этому: сдвиг (a, dy, b) в рамке этого куска,
    /// поворот — относительно этого.
    fn attach(&self, name: &str, rel_rot: u8, a: i32, dy: i32, b: i32) -> Placed {
        let (x, z) = to_world(self.tp, self.rot, a, b);
        Placed::new(template(name), (self.rot + rel_rot) % 4, (x, self.tp.1 + dy, z))
    }

    pub fn name(&self) -> &'static str {
        templates()[self.template].name
    }
}

/// Клетка (a, b) образца с точкой `tp` и поворотом `rot` в мире (замер:
/// NONE (a,b)→(+x,+z), по часовой (a,b)→(−b,+a), на полоборота (−a,−b),
/// против часовой (+b,−a)).
fn to_world(tp: (i32, i32, i32), rot: u8, a: i32, b: i32) -> (i32, i32) {
    match rot {
        0 => (tp.0 + a, tp.2 + b),
        1 => (tp.0 - b, tp.2 + a),
        2 => (tp.0 - a, tp.2 - b),
        _ => (tp.0 + b, tp.2 - a),
    }
}

fn overlaps(p: &[i32; 6], q: &[i32; 6]) -> bool {
    p[0] <= q[3] && q[0] <= p[3] && p[1] <= q[4] && q[1] <= p[4] && p[2] <= q[5] && q[2] <= p[5]
}

/// Кости.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        scramble(self.0)
    }

    fn below(&mut self, bound: i32) -> i32 {
        (self.next() % bound.max(1) as u64) as i32
    }

    fn chance(&mut self, share: f64) -> bool {
        ((self.next() >> 11) as f64 / (1u64 << 53) as f64) < share
    }
}

fn scramble(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^ (value >> 33)
}

/// Чанк старта города в регионе.
pub fn region_start(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    let mut dice = Dice(scramble(
        (seed as u64) ^ (region_x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (region_z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ 0x10387313,
    ));

    (region_x * REGION + dice.below(SPREAD), region_z * REGION + dice.below(SPREAD))
}

/// Регионы, чьи города могут задевать чанк.
pub fn regions_near(chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();

    for region_x in (chunk_x - RADIUS_CHUNKS).div_euclid(REGION)..=(chunk_x + RADIUS_CHUNKS).div_euclid(REGION) {
        for region_z in (chunk_z - RADIUS_CHUNKS).div_euclid(REGION)..=(chunk_z + RADIUS_CHUNKS).div_euclid(REGION) {
            out.push((region_x, region_z));
        }
    }

    out
}

/// Угол основания города в чанке старта: x, z клетки образца base_floor.
pub fn root_corner(chunk_x: i32, chunk_z: i32) -> (i32, i32) {
    (chunk_x * 16 + 3, chunk_z * 16 + 3)
}

struct Plan {
    pieces: Vec<Placed>,
    /// Рамки, которые нельзя задевать (дома, башни, мосты, корабль).
    solid: Vec<[i32; 6]>,
    ship: bool,
    dice: Dice,
}

impl Plan {
    fn put(&mut self, piece: Placed, solid: bool) -> usize {
        if solid {
            self.solid.push(piece.bounds);
        }

        self.pieces.push(piece);
        self.pieces.len() - 1
    }

    fn blocked(&self, piece: &Placed) -> bool {
        self.pieces.len() >= MAX_PIECES || self.solid.iter().any(|b| overlaps(b, &piece.bounds))
    }

    /// Дом: основание и этажи. Возвращает основание и число этажей.
    fn house(&mut self, base: Placed, stories: i32) -> usize {
        let at = self.put(base.clone(), true);

        match stories {
            1 => {
                self.put(base.attach("base_roof", 0, -1, 4, -1), false);
            }
            2 => {
                let second = if self.dice.chance(0.33) { "second_floor_1" } else { "second_floor_2" };
                self.put(base.attach(second, 0, -1, 0, -1), false);
                self.put(base.attach("second_roof", 0, -2, 8, -2), false);
            }
            _ => {
                let second = if self.dice.chance(0.33) { "second_floor_1" } else { "second_floor_2" };
                let third = if self.dice.chance(0.55) { "third_floor_1" } else { "third_floor_2" };
                self.put(base.attach(second, 0, -1, 0, -1), false);
                self.put(base.attach(third, 0, -2, 4, -2), false);
                self.put(base.attach("third_roof", 0, -3, 12, -3), false);
            }
        }

        at
    }

    /// Башня из дома в два-три этажа, с мостами.
    fn tower(&mut self, house: usize, stories: i32, depth: u32) {
        let base = self.pieces[house].clone();
        let (shift, dy) = if stories >= 3 { (0, 9) } else { (1, 5) };
        let tower_base = base.attach("tower_base", 0, shift + self.dice.below(2), dy, shift + self.dice.below(2));
        // Ярусы башни: основание и 2–4 куска по 4 блока.
        let mut levels = vec![(self.put(tower_base.clone(), true), 6)];
        // Рамки самой башни: дверь сидит в её стене, это не помеха.
        let mut own = vec![tower_base.bounds];
        let count = 2 + self.dice.below(3);

        for step in 0..count {
            let piece = tower_base.attach("tower_piece", 0, 0, 7 + 4 * step, 0);
            own.push(piece.bounds);
            levels.push((self.put(piece, true), 3));
        }

        let top_y = 7 + 4 * count;

        // Мосты: с каждой стороны в половине случаев, ярус случайный.
        let mut doors: Vec<(usize, u8, i32)> = Vec::new();

        for side in 0..4u8 {
            if self.dice.chance(0.5) {
                let (level, door_y) = levels[self.dice.below(levels.len() as i32) as usize];
                doors.push((level, side, door_y));
            }
        }

        if self.dice.chance(0.3) {
            let fat_base = tower_base.attach("fat_tower_base", 0, -3, top_y, -3);
            own.push(fat_base.bounds);
            self.put(fat_base.clone(), true);
            let middles = 1 + self.dice.below(2);

            for step in 0..middles {
                let middle_piece = fat_base.attach("fat_tower_middle", 0, 0, 4 + 8 * step, 0);
                own.push(middle_piece.bounds);
                let middle = self.put(middle_piece, true);

                for side in 0..4u8 {
                    if self.dice.chance(0.25) {
                        doors.push((middle, side, 7));
                    }
                }
            }

            self.put(fat_base.attach("fat_tower_top", 0, -2, 4 + 8 * middles, -2), false);
        } else {
            self.put(tower_base.attach("tower_top", 0, -1, top_y, -1), false);
        }

        for (level, side, door_y) in doors {
            self.bridge(level, side, door_y, &own, depth);
        }
    }

    /// Дверь на стороне башни и мост от неё.
    fn bridge(&mut self, level: usize, side: u8, door_y: i32, own: &[[i32; 6]], depth: u32) {
        let parent = self.pieces[level].clone();
        let size = templates()[parent.template].width;
        // Дверь (5 вбок, 2 вдоль) на середине стороны, мост уходит в −b
        // её рамки (замер: от 7-блочной башни двери в (1,0), (6,1), (5,6), (0,5)).
        let (a, b) = match side {
            0 => (1 + (size - 7) / 2, 0),
            1 => (size - 1, 1 + (size - 7) / 2),
            2 => (size - 2 - (size - 7) / 2, size - 1),
            _ => (0, size - 2 - (size - 7) / 2),
        };
        let door = parent.attach("bridge_end", side, a, door_y, b);

        if self.solid.iter().any(|s| overlaps(s, &door.bounds) && !own.contains(s)) {
            return;
        }

        self.put(door.clone(), false);

        // Мост за дверью — в 71 % случаев (замер: 117 мостов на 164 двери).
        if self.pieces.len() >= MAX_PIECES || !self.dice.chance(0.71) {
            return;
        }

        // Куски моста: от двери в −b, лестницы поднимают на 4.
        let count = 2 + self.dice.below(4);
        let mut last = door;
        let mut parts = Vec::new();

        for _ in 0..count {
            let roll = self.dice.below(100);
            let (name, length) = if roll < 61 { ("bridge_piece", 4) } else if roll < 80 { ("bridge_steep_stairs", 4) } else { ("bridge_gentle_stairs", 8) };
            let rise = if parts.is_empty() { 0 } else { stair_rise(&self.pieces[*parts.last().unwrap_or(&0)]) };
            let part = last.attach(name, 0, 0, rise, -length);

            if self.blocked(&part) {
                return;
            }

            parts.push(self.put(part.clone(), true));
            last = part;
        }

        let rise = stair_rise(&last);

        if !self.ship && self.dice.chance(0.2) {
            // Корабль висит в стороне от конца моста (замер: сдвиг −4..−8 вбок,
            // −61..−70 вдоль, на 0 или 4 выше) [проверить].
            let ship = last.attach("ship", 0, -8 + self.dice.below(5), 4 * self.dice.below(2), -70 + self.dice.below(10));

            if !self.blocked(&ship) {
                self.ship = true;
                self.put(ship, true);
                return;
            }
        }

        let roll = self.dice.below(100);
        let stories = if roll < 38 { 1 } else if roll < 74 { 2 } else { 3 };
        let base = last.attach("base_floor", 0, -3, rise + 1, -11);

        if self.blocked(&base) {
            return;
        }

        let house = self.house(base.clone(), stories);
        self.put(base.attach("bridge_end", 2, 7, -1, 11), false);

        if stories >= 2 && depth < MAX_DEPTH {
            self.tower(house, stories, depth + 1);
        }
    }
}

/// На сколько поднимает следующий кусок лестница (замер: обе на 4).
fn stair_rise(piece: &Placed) -> i32 {
    if matches!(piece.name(), "bridge_steep_stairs" | "bridge_gentle_stairs") { 4 } else { 0 }
}

/// Раскладка города со стартом в чанке и низом на высоте `y`.
pub fn plan(seed: i64, chunk_x: i32, chunk_z: i32, y: i32) -> Vec<Placed> {
    let mut plan = Plan {
        pieces: Vec::new(),
        solid: Vec::new(),
        ship: false,
        dice: Dice(scramble(
            (seed as u64) ^ (chunk_x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (chunk_z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ 0xE4D_C17,
        )),
    };
    let rot = plan.dice.below(4) as u8;
    let (x, z) = root_corner(chunk_x, chunk_z);
    // Точка образца — угол рамки по повороту: основание всегда занимает
    // клетки x..x+9, z..z+9 чанка старта.
    let tp = match rot {
        0 => (x, y, z),
        1 => (x + 9, y, z),
        2 => (x + 9, y, z + 9),
        _ => (x, y, z + 9),
    };
    let root = Placed::new(template("base_floor"), rot, tp);
    let house = plan.house(root, 3);
    plan.tower(house, 3, 0);

    plan.pieces
}

/// Кладёт куски города, попавшие в чанк (x0, z0): `put` получает мировые
/// координаты и состояние (воздух — `air`).
pub fn paint(pieces: &[Placed], x0: i32, z0: i32, air: i32, put: &mut dyn FnMut(i32, i32, i32, i32)) {
    let templates = templates();

    for piece in pieces {
        let b = &piece.bounds;

        if b[0] > x0 + 15 || x0 > b[3] || b[2] > z0 + 15 || z0 > b[5] {
            continue;
        }

        let template = &templates[piece.template];
        let states = &template.states[piece.rot as usize];

        for y in 0..template.height {
            for bb in 0..template.length {
                for a in 0..template.width {
                    let (x, wy, z) = piece.world(a, y, bb);

                    if !(0..16).contains(&(x - x0)) || !(0..16).contains(&(z - z0)) {
                        continue;
                    }

                    match template.cell(a, y, bb) {
                        Cell::Keep => {}
                        Cell::Air => put(x, wy, z, air),
                        Cell::Block(index) => put(x, wy, z, states[index as usize]),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn templates_parse_with_measured_sizes() {
        let sizes: HashMap<&str, (i32, i32, i32)> = [
            ("base_floor", (10, 10, 4)),
            ("second_floor_1", (12, 12, 8)),
            ("third_roof", (16, 16, 2)),
            ("tower_base", (7, 7, 7)),
            ("tower_piece", (7, 7, 4)),
            ("tower_top", (9, 9, 5)),
            ("bridge_end", (5, 2, 6)),
            ("bridge_piece", (5, 4, 6)),
            ("bridge_gentle_stairs", (5, 8, 7)),
            ("fat_tower_top", (17, 17, 6)),
            ("ship", (13, 29, 24)),
        ]
        .into_iter()
        .collect();

        assert_eq!(templates().len(), 19);

        for t in templates() {
            if let Some(&(w, l, h)) = sizes.get(t.name) {
                assert_eq!((t.width, t.length, t.height), (w, l, h), "{}", t.name);
            }
        }
    }

    #[test]
    fn orient_turns_directions_and_banner_rotation() {
        assert_eq!(orient("purpur_stairs[facing=north,half=top]", 1), "purpur_stairs[facing=east,half=top]");
        assert_eq!(orient("purpur_stairs[facing=north,half=top]", 3), "purpur_stairs[facing=west,half=top]");
        assert_eq!(orient("magenta_banner[rotation=2]", 2), "magenta_banner[rotation=10]");
        assert_eq!(orient("end_rod[facing=up]", 1), "end_rod[facing=up]");
    }

    #[test]
    fn frame_matches_measured_corners() {
        // Замер: при повороте по часовой точка образца — угол (x1, z0).
        let p = Placed::new(template("base_floor"), 1, (-569, 60, 2023));
        assert_eq!(p.bounds, [-578, 60, 2023, -569, 63, 2032]);
        let p = Placed::new(template("ship"), 3, (-1896, 79, -1506));
        assert_eq!(p.bounds, [-1896, 79, -1518, -1868, 102, -1506]);
    }

    #[test]
    fn plan_is_deterministic_and_bounded() {
        let a = plan(7, 70, -40, 61);
        let b = plan(7, 70, -40, 61);
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| p.bounds == q.bounds && p.rot == q.rot));
        assert!(a.len() <= MAX_PIECES);
        assert!(a.iter().all(|p| (p.bounds[0] - 70 * 16).abs() <= RADIUS_CHUNKS * 16 + 16 && (p.bounds[5] - (-40 * 16)).abs() <= RADIUS_CHUNKS * 16 + 16), "размах города");
    }

    /// Доли кусков как у оригинала: на 30 городов 1569 кусков — в среднем
    /// 52 на город, башен 2,7, кораблей 0,67, дверей 8,7, домов 4,2.
    #[test]
    fn piece_counts_match_vanilla() {
        let mut total = 0;
        let mut count: HashMap<&str, usize> = HashMap::new();
        let cities = 400;

        for n in 0..cities {
            let pieces = plan(99, n * 20 + 3, (n % 7) * 20 + 2, 61);
            total += pieces.len();

            for p in &pieces {
                *count.entry(p.name()).or_default() += 1;
            }
        }

        let per = |name: &str| count.get(name).copied().unwrap_or(0) as f64 / cities as f64;
        let mean = total as f64 / cities as f64;
        let mut names: Vec<_> = count.iter().collect();
        names.sort();
        eprintln!("кусков на город {:.1}: {:?}", mean, names.iter().map(|(k, v)| (**k, **v as f64 / cities as f64)).collect::<Vec<_>>());
        assert!((38.0..68.0).contains(&mean), "кусков на город {}", mean);
        assert!((1.8..3.8).contains(&per("tower_base")), "башен {}", per("tower_base"));
        assert!((0.4..1.0).contains(&per("ship")), "кораблей {}", per("ship"));
        assert!((5.5..12.0).contains(&per("bridge_end")), "дверей {}", per("bridge_end"));
        assert!((2.8..6.0).contains(&per("base_floor")), "домов {}", per("base_floor"));
    }

    #[test]
    fn region_start_is_in_first_nine_chunks() {
        for region in [(0, 0), (-3, 5), (7, -2)] {
            let (x, z) = region_start(5, region.0, region.1);
            assert!((0..SPREAD).contains(&(x - region.0 * REGION)));
            assert!((0..SPREAD).contains(&(z - region.1 * REGION)));
        }
    }
}
