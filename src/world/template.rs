// Образцы строений из одного куска (разрушенные порталы, кораблекрушения,
// руины в океане, руины тропы): разбор текстовых образцов, снятых с миров
// оригинала (tools/measure/template_merge.py), и их постановка с поворотом
// и отражением.
//
// Образец — рамка a × b × y (a — x, b — z без поворота), клетка — блок
// палитры, «_» — воздух, «.» — не трогать (рельеф остаётся). Каждая клетка —
// то, что стоит в ней у большинства экземпляров оригинала: случайные замены
// (плачущий обсидиан, мох и трещины…) кладут уже сами строения по правилам
// вики.
//
// Поворот и отражение — как в записи о строении оригинала (Rotation, Mirror;
// tools/measure/template_structs.py): сперва отражение (a → −a), потом
// поворот по часовой на `turns` четвертей. Начало рамки в мире — её
// наименьший угол.

use std::collections::HashMap;

/// Клетка образца.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Keep,
    Air,
    Block(u16),
}

pub struct Template {
    pub name: String,
    pub width: i32,
    pub length: i32,
    pub height: i32,
    cells: Vec<Cell>,
    /// Палитра: записи состояний, как в образце.
    pub texts: Vec<String>,
    /// Таблицы добычи сундуков: клетка образца (a, y, b) → имя таблицы
    /// (`loot a y b таблица` — поле LootTable у экземпляров оригинала).
    pub loot: Vec<((i32, i32, i32), String)>,
    /// Доля, с которой кладётся блок палитры (руины с неполной целостью;
    /// `<знак> <состояние> <доля>`), по умолчанию 1.
    pub chance: Vec<f64>,
    /// Места мобов при сложении и доля экземпляров с ними (`drowned a y b доля`).
    pub mobs: Vec<((i32, i32, i32), f64)>,
}

impl Template {
    pub fn cell(&self, a: i32, y: i32, b: i32) -> Cell {
        if !(0..self.width).contains(&a) || !(0..self.height).contains(&y) || !(0..self.length).contains(&b) {
            return Cell::Keep;
        }

        self.cells[((y * self.length + b) * self.width + a) as usize]
    }

    /// Имя блока палитры без свойств.
    #[cfg(test)]
    pub fn block_name(&self, index: u16) -> &str {
        let text = &self.texts[usize::from(index)];

        text.split('[').next().unwrap_or(text)
    }

    /// Сторона рамки в мире по x и по z в этом положении.
    pub fn footprint(&self, orient: Orient) -> (i32, i32) {
        if orient.turns % 2 == 1 { (self.length, self.width) } else { (self.width, self.length) }
    }

    /// Таблица добычи сундука в клетке образца, если записана.
    pub fn loot_at(&self, a: i32, y: i32, b: i32) -> Option<&str> {
        self.loot.iter().find(|(at, _)| *at == (a, y, b)).map(|(_, table)| table.as_str())
    }

    /// Все клетки, кроме «не трогать»: (a, y, b, клетка).
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32, i32, Cell)> + '_ {
        self.cells.iter().enumerate().filter(|(_, cell)| **cell != Cell::Keep).map(|(at, cell)| {
            let at = at as i32;

            (at % self.width, at / (self.width * self.length), (at / self.width) % self.length, *cell)
        })
    }
}

/// Положение образца: поворот по часовой и отражение (a → −a до поворота).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Orient {
    pub turns: u8,
    pub mirror: bool,
}

impl Orient {
    /// Из числа 0…7 (случай строения).
    pub fn from_roll(roll: u64) -> Orient {
        Orient { turns: (roll % 4) as u8, mirror: (roll >> 2) % 2 == 1 }
    }

    /// Смещение клетки (a, b) от наименьшего угла рамки в мире.
    pub fn offset(self, a: i32, b: i32, width: i32, length: i32) -> (i32, i32) {
        let a = if self.mirror { width - 1 - a } else { a };

        match self.turns % 4 {
            0 => (a, b),
            1 => (length - 1 - b, a),
            2 => (width - 1 - a, length - 1 - b),
            _ => (b, width - 1 - a),
        }
    }

    /// Обратное к `offset`: клетка образца для смещения от угла рамки.
    pub fn local(self, dx: i32, dz: i32, width: i32, length: i32) -> (i32, i32) {
        let (a, b) = match self.turns % 4 {
            0 => (dx, dz),
            1 => (dz, length - 1 - dx),
            2 => (width - 1 - dx, length - 1 - dz),
            _ => (width - 1 - dz, dx),
        };

        (if self.mirror { width - 1 - a } else { a }, b)
    }
}

const SIDES: [&str; 4] = ["north", "east", "south", "west"];

/// Запись состояния в положении `orient`: стороны (facing, north=…),
/// оси, деление поворота табличек; при отражении — ещё левое и правое.
pub fn orient_text(text: &str, orient: Orient) -> String {
    let Some((name, rest)) = text.split_once('[') else {
        return text.to_string();
    };
    let turn = |side: &str| -> Option<&'static str> {
        let at = SIDES.iter().position(|known| *known == side)?;
        // Отражение a → −a меняет восток и запад.
        let at = if orient.mirror && at % 2 == 1 { (at + 2) % 4 } else { at };

        Some(SIDES[(at + usize::from(orient.turns)) % 4])
    };
    let swap = |value: &str| match value {
        "left" => "right".to_string(),
        "right" => "left".to_string(),
        "inner_left" => "inner_right".to_string(),
        "inner_right" => "inner_left".to_string(),
        "outer_left" => "outer_right".to_string(),
        "outer_right" => "outer_left".to_string(),
        other => other.to_string(),
    };
    let mut props: Vec<(String, String)> = Vec::new();

    for pair in rest.trim_end_matches(']').split(',') {
        let Some((key, value)) = pair.split_once('=') else { continue };
        let (key, value) = match key {
            side if SIDES.contains(&side) => (turn(side).unwrap_or(side).to_string(), value.to_string()),
            "facing" | "horizontal_facing" => (key.to_string(), turn(value).unwrap_or(value).to_string()),
            "axis" if orient.turns % 2 == 1 && (value == "x" || value == "z") => (key.to_string(), if value == "x" { "z" } else { "x" }.to_string()),
            "rotation" => {
                let turned = value.parse::<u8>().map_or(value.to_string(), |r| {
                    let r = if orient.mirror { (16 - r) % 16 } else { r };

                    ((r + 4 * orient.turns) % 16).to_string()
                });

                (key.to_string(), turned)
            }
            "shape" | "hinge" | "type" if orient.mirror => (key.to_string(), swap(value)),
            _ => (key.to_string(), value.to_string()),
        };

        props.push((key, value));
    }

    props.sort();
    format!("{}[{}]", name, props.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(","))
}

/// Разбор файла образцов: `template <имя> <a> <b> <y> [экземпляров]`,
/// палитра `  <знак> <состояние>`, слои `layer <y>` по `b` строк из `a` знаков.
pub fn parse(text: &str) -> Vec<Template> {
    let mut out: Vec<Template> = Vec::new();
    let mut lines = text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).peekable();

    while let Some(line) = lines.next() {
        let words: Vec<&str> = line.split_whitespace().collect();

        if words.first() != Some(&"template") || words.len() < 5 {
            continue;
        }

        let number = |at: usize| words[at].parse::<i32>().unwrap_or(0);
        let (width, length, height) = (number(2), number(3), number(4));
        let mut palette: HashMap<char, u16> = HashMap::new();
        let mut texts: Vec<String> = Vec::new();
        let mut cells = vec![Cell::Keep; (width * length * height).max(0) as usize];
        let mut loot = Vec::new();
        let mut chance = Vec::new();
        let mut mobs = Vec::new();

        while let Some(line) = lines.next_if(|line| !line.starts_with("template")) {
            let trimmed = line.trim();

            if let Some(y) = trimmed.strip_prefix("layer ") {
                let y: i32 = y.trim().parse().unwrap_or(0);

                for b in 0..length {
                    let Some(row) = lines.next() else { break };

                    for (a, symbol) in row.trim().chars().enumerate().take(width as usize) {
                        let cell = match symbol {
                            '.' => Cell::Keep,
                            '_' => Cell::Air,
                            symbol => palette.get(&symbol).map_or(Cell::Keep, |at| Cell::Block(*at)),
                        };

                        if (0..height).contains(&y) {
                            cells[((y * length + b) * width + a as i32) as usize] = cell;
                        }
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("loot ") {
                let words: Vec<&str> = rest.split_whitespace().collect();

                if let [a, y, b, table] = words[..] {
                    loot.push(((a.parse().unwrap_or(0), y.parse().unwrap_or(0), b.parse().unwrap_or(0)), table.to_string()));
                }
            } else if let Some(rest) = trimmed.strip_prefix("drowned ") {
                let words: Vec<&str> = rest.split_whitespace().collect();

                if let [a, y, b, share] = words[..] {
                    mobs.push(((a.parse().unwrap_or(0), y.parse().unwrap_or(0), b.parse().unwrap_or(0)), share.parse().unwrap_or(1.0)));
                }
            } else if let Some((symbol, state)) = trimmed.split_once(' ') {
                let (state, share) = match state.trim().split_once(' ') {
                    Some((state, share)) => (state, share.trim().parse().unwrap_or(1.0)),
                    None => (state.trim(), 1.0),
                };

                palette.insert(symbol.chars().next().unwrap_or('?'), texts.len() as u16);
                texts.push(state.replace("waterlogged=true", "waterlogged=false"));
                chance.push(share);
            }
        }

        out.push(Template { name: words[1].to_string(), width, length, height, cells, texts, loot, chance, mobs });
    }

    out
}

fn scramble(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Случай по семени, месту и соли: одно 64-битное число.
pub fn roll(seed: i64, x: i32, z: i32, salt: u64) -> u64 {
    scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt)
}

/// Случай блока: число от 0 до 1 по семени, клетке и соли.
pub fn chance(seed: i64, x: i32, y: i32, z: i32, salt: u64) -> f64 {
    let dice = roll(seed, x, z, salt ^ (y as u64).wrapping_mul(0x85EB_CA6B_27D4_EB2F));

    (dice >> 11) as f64 / (1u64 << 53) as f64
}

/// Чанк попытки в области: сдвиг 0…spacing−separation−1 по каждой оси
/// (вики, Structure set: «linear»).
pub fn attempt(seed: i64, region_x: i32, region_z: i32, spacing: i32, separation: i32, salt: u64) -> (i32, i32) {
    let dice = roll(seed, region_x, region_z, salt);
    let span = (spacing - separation) as u64;

    (region_x * spacing + (dice % span) as i32, region_z * spacing + ((dice >> 24) % span) as i32)
}

/// Ближайший старт среди областей вокруг чанка (`locate structure`): по
/// кольцам областей, пока следующее кольцо не может дать ближе.
pub fn nearest(chunk_x: i32, chunk_z: i32, spacing: i32, rings: i32, start: &dyn Fn(i32, i32) -> Option<(i32, i32)>) -> Option<(i32, i32)> {
    let (home_x, home_z) = (chunk_x.div_euclid(spacing), chunk_z.div_euclid(spacing));
    let distance = |(x, z): (i32, i32)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2);
    let mut best: Option<(i32, i32)> = None;

    for ring in 0..=rings {
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                if let Some(found) = start(home_x + dx, home_z + dz)
                    && best.is_none_or(|known| distance(found) < distance(known))
                {
                    best = Some(found);
                }
            }
        }

        if let Some(found) = best
            && (distance(found) as f64).sqrt() < f64::from(ring * spacing)
        {
            break;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Смещение и обратное к нему сходятся во всех восьми положениях.
    #[test]
    fn offset_and_local_round_trip() {
        for index in 0..8 {
            let orient = Orient { turns: index % 4, mirror: index >= 4 };
            let (width, length) = (5, 3);
            let (fx, fz) = if orient.turns % 2 == 1 { (length, width) } else { (width, length) };

            for a in 0..width {
                for b in 0..length {
                    let (dx, dz) = orient.offset(a, b, width, length);

                    assert!((0..fx).contains(&dx) && (0..fz).contains(&dz), "{orient:?} {a} {b}");
                    assert_eq!(orient.local(dx, dz, width, length), (a, b), "{orient:?}");
                }
            }
        }
    }

    /// Направления в состояниях поворачиваются вместе с клетками: ступень,
    /// смотрящая к +a (восток), после поворота смотрит туда же, куда ушло +a.
    #[test]
    fn facing_follows_the_cells() {
        let side = |dx: i32, dz: i32| match (dx.signum(), dz.signum()) {
            (1, 0) => "east",
            (-1, 0) => "west",
            (0, 1) => "south",
            _ => "north",
        };

        for index in 0..8u8 {
            let orient = Orient { turns: index % 4, mirror: index >= 4 };
            let (x0, z0) = orient.offset(1, 1, 3, 3);
            let (x1, z1) = orient.offset(2, 1, 3, 3);
            let text = orient_text("oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]", orient);

            assert!(text.contains(&format!("facing={}", side(x1 - x0, z1 - z0))), "{orient:?}: {text}");
        }
    }

    #[test]
    fn shares_and_mobs_are_read() {
        let parsed = parse("template t 1 1 1\n  A gravel 0.7\n  drowned 0 1 0 0.5\n  loot 0 0 0 x\nlayer 0\n  A\n");

        assert_eq!(parsed[0].chance, vec![0.7]);
        assert_eq!(parsed[0].mobs, vec![((0, 1, 0), 0.5)]);
        assert_eq!(parsed[0].loot_at(0, 0, 0), Some("x"));
    }

    #[test]
    fn templates_parse_with_palette_and_layers() {
        let parsed = parse("template test 2 2 2\n  A stone\nlayer 0\n  A.\n  _A\nlayer 1\n  ..\n  .A\n");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].cell(0, 0, 0), Cell::Block(0));
        assert_eq!(parsed[0].cell(0, 0, 1), Cell::Air);
        assert_eq!(parsed[0].cell(1, 1, 1), Cell::Block(0));
        assert_eq!(parsed[0].cells().count(), 4);
    }
}
