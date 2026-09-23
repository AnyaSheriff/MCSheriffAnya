// Деревья: какие растут где и какой они формы.
//
// Форма дерева — список блоков относительно земли под стволом. Решается
// числом от координат ствола, поэтому соседние чанки рисуют одно и то же
// дерево, каждый свою часть, не сговариваясь.
//
// Формы подогнаны под замер оригинала: tools/research/trees-measured.md —
// по 25 деревьев каждого вида, снятых с официального сервера по сети:
// высоты стволов, радиусы и форма каждого слоя кроны, ветви, лианы, какао,
// корни, мох и подзол. Проверка `trees_match_measurements` держит их
// в пределах замера.

use super::noise::Random;
use super::terrain::{chance, Biome, Column, Terrain, SEA};

/// Дерево: из чего ствол, из чего крона, какой высоты ствол и какой формы
/// дерево.
///
/// `log` и `leaves` — имя блока или полная запись состояния
/// (`mushroom_stem[up=false,down=false]`): у огромных грибов «ствол» и
/// «крона» — блоки гриба.
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub log: &'static str,
    pub leaves: &'static str,
    /// Высота прямого ствола — до макушки или до первого излома. У мангра —
    /// число брёвен над корнями, у акации — полная высота вместе с изгибом.
    pub trunk: i32,
    pub shape: Shape,
    /// Под кроной висит пчелиное гнездо: у дубов на равнине, у берёз на лугу.
    pub bees: bool,
}

/// Форма дерева.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Дуб: прямой ствол, широкие слои листвы почти до земли, над ними
    /// узкий слой и крест.
    Oak,
    /// Берёза: прямой ствол, два широких слоя листвы и два узких.
    Round,
    /// Болотный дуб: крона на блок шире обычной, с неё свисают лианы.
    Swamp,
    /// Большой дуб: высокий ствол, от него ветви, на концах ветвей —
    /// отдельные шапки листвы. Низкий — просто ком листвы вокруг ствола.
    Fancy,
    /// Ель: над макушкой шпиль, ниже гармошка — широкий ярус, узкий, снова
    /// широкий, всё шире книзу.
    Spruce,
    /// Сосна: голый ствол, листва зонтиком только у макушки.
    Pine,
    /// Большая ель 2×2: гармошка почти по всей высоте, под ней подзол.
    MegaSpruce,
    /// Большая сосна 2×2: голый ствол, зонтик в верхних слоях, подзол.
    MegaPine,
    /// Малое дерево джунглей: крона как у берёзы, лианы по стволу и листве,
    /// изредка на стволе какао.
    Jungle,
    /// Большое дерево джунглей 2×2: крона на макушке, ветви с шапками
    /// листвы, лианы.
    MegaJungle,
    /// Куст джунглей: одно бревно, вокруг — горка листвы.
    Bush,
    /// Акация: ствол изгибается вбок, наверху плоская крона, иногда вторая
    /// ветвь со своей кроной пониже.
    Acacia,
    /// Тёмный дуб 2×2: неровный верх ствола, короткие ветви, широкая
    /// плоская крона.
    DarkOak,
    /// Бледный дуб: как тёмный, но с кроны свисает бледный мох, под
    /// деревом — бледный мох на земле.
    PaleOak,
    /// Вишня: ствол расходится ветвями, над каждой — свой купол кроны.
    Cherry,
    /// Мангр: ствол стоит на корнях, расходящихся юбкой, с кроны свисают
    /// лианы и проростки.
    Mangrove,
    /// Высокий мангр: ствол выше, юбка шире, ветвей и лиан больше.
    TallMangrove,
    /// Дерево с азалией: короткий ствол, крона раздувается кверху.
    Azalea,
    /// Огромный коричневый гриб: ножка и плоская шляпка.
    BrownMushroom,
    /// Огромный красный гриб: ножка и шляпка-купол.
    RedMushroom,
}

/// Часть дерева.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    /// Лист: ставится только в воздух.
    Leaves,
    /// Бревно, лежащее вдоль своей оси: ставится поверх чего угодно.
    Log(Axis),
    /// Отдельный блок с полной записью состояния — лиана, какао, улей,
    /// корни. Ставится в воздух, а если блок бывает затоплен — и в воду.
    Block(&'static str),
    /// Замена земли под деревом (подзол, илистые корни): ищется верхний
    /// блок земли у этого столбца, и заменяется, только если это земля.
    /// Простая земля (`dirt`) ложится только на место травы.
    Ground(&'static str),
}

/// Вдоль какой оси лежит бревно.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Блок дерева: смещение от земли под стволом (`dy = 1` — первый блок над
/// землёй) и что там стоит. У толстых стволов 2×2 ствол занимает смещения
/// 0 и 1 по обеим осям.
pub type TreeBlock = (i32, i32, i32, Part);

/// Насколько далеко по горизонтали дерево может выйти за свой ствол.
pub const TREE_REACH: i32 = 8;

impl Tree {
    /// Все блоки дерева, выросшего в точке (x, z). Форма своя у каждого
    /// места, но одна и та же при каждом обращении: соседние чанки рисуют
    /// одно и то же дерево, каждый свою часть.
    pub fn blocks(&self, x: i32, z: i32) -> Vec<TreeBlock> {
        let mut random = Random::new(
            (x as i64).wrapping_mul(341_873_128_712).wrapping_add((z as i64).wrapping_mul(132_897_987_541)) + 5_003,
        );
        let mut blocks = Vec::with_capacity(256);

        match self.shape {
            Shape::Oak => self.oak(&mut random, &mut blocks),
            Shape::Round => self.round(&mut random, &mut blocks),
            Shape::Swamp => self.swamp(&mut random, &mut blocks),
            Shape::Fancy => self.fancy(&mut random, &mut blocks),
            Shape::Spruce => self.spruce(&mut random, &mut blocks),
            Shape::Pine => self.pine(&mut random, &mut blocks),
            Shape::MegaSpruce => self.mega_conifer(&mut random, &mut blocks, false),
            Shape::MegaPine => self.mega_conifer(&mut random, &mut blocks, true),
            Shape::Jungle => self.jungle(&mut random, &mut blocks),
            Shape::MegaJungle => self.mega_jungle(&mut random, &mut blocks),
            Shape::Bush => bush(&mut random, &mut blocks),
            Shape::Acacia => self.acacia(&mut random, &mut blocks),
            Shape::DarkOak => self.dark_oak(&mut random, &mut blocks, false),
            Shape::PaleOak => self.dark_oak(&mut random, &mut blocks, true),
            Shape::Cherry => self.cherry(&mut random, &mut blocks),
            Shape::Mangrove => self.mangrove(&mut random, &mut blocks, false),
            Shape::TallMangrove => self.mangrove(&mut random, &mut blocks, true),
            Shape::Azalea => self.azalea(&mut random, &mut blocks),
            Shape::BrownMushroom => self.brown_mushroom(&mut blocks),
            Shape::RedMushroom => self.red_mushroom(&mut blocks),
        }

        dirt_under_trunk(&mut blocks);

        blocks
    }

    /// Дуб. Над макушкой крест, на уровне макушки слой 3×3, ниже — слои
    /// 5×5 до второго бревна ствола, но не больше четырёх. Углы широких
    /// слоёв чаще целы, чем срезаны.
    fn oak(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;
        let lowest = (top - 4).max(2);

        square_layer(random, blocks, 0, top + 1, 0, 1, Corners::Cut);
        square_layer(random, blocks, 0, top, 0, 1, Corners::Random(2));

        for y in lowest..top {
            square_layer(random, blocks, 0, y, 0, 2, Corners::Random(3));
        }

        trunk(blocks, 0, 0, 1, top);

        // Гнездо висит под нижним слоем кроны, сбоку от ствола.
        if self.bees {
            blocks.push((0, lowest - 1, 1, Part::Block("bee_nest[facing=south]")));
        }
    }

    /// Берёза. Крона — четыре слоя: два нижних квадратом 5×5, два верхних
    /// 3×3. Углы всех слоёв, кроме самого верхнего, срезаны через раз,
    /// у верхнего — всегда: макушка получается крестом.
    fn round(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        round_crown(random, blocks, top, 0);
        trunk(blocks, 0, 0, 1, top);

        if self.bees && top >= 4 {
            blocks.push((0, top - 3, 1, Part::Block("bee_nest[facing=south]")));
        }
    }

    /// Болотный дуб: крона на блок шире, с нижних слоёв свисают лианы.
    fn swamp(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        round_crown(random, blocks, top, 1);
        trunk(blocks, 0, 0, 1, top);

        // Лианы — с верхнего из широких слоёв: под ним слой той же ширины,
        // и свисающая плеть не упрётся в листву.
        hang_vines(random, blocks, top - 1, 3, (2, 4), |dx, dz| square_without_corners(dx, dz, 3));
    }

    /// Большой дуб. Низкий — без ветвей: ком листвы радиусом два вокруг
    /// ствола от самой земли. Высокий — от ствола в стороны и вверх
    /// расходятся ветви, у каждой на конце своя шапка листвы, ещё одна —
    /// на макушке. Чем выше дерево, тем больше ветвей.
    fn fancy(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        if top <= 5 {
            disc(blocks, 0, 1, 0, 1, false);

            for y in 2..=top {
                disc(blocks, 0, y, 0, 2, true);
            }

            disc(blocks, 0, top + 1, 0, 1, false);
            trunk(blocks, 0, 0, 1, top);
            return;
        }

        let branches = 2 + top / 3 + roll(random, 2);

        cluster(blocks, 0, top + 2, 0);

        for index in 0..branches {
            // Направления разнесены по кругу, чтобы ветви не сбивались
            // в одну сторону.
            let angle = (index as f64 + fraction(random) * 0.6) / branches as f64 * std::f64::consts::TAU;
            let reach = 2.0 + fraction(random) * 3.0;
            // Ветви растут с третьего бревна и до самого верха.
            let from = 3 + roll(random, top - 3);
            let end_x = (angle.cos() * reach).round() as i32;
            let end_z = (angle.sin() * reach).round() as i32;
            let end_y = (from + 1 + roll(random, 3)).min(top);

            cluster(blocks, end_x, end_y, end_z);
            line(blocks, (0, from, 0), (end_x, end_y, end_z));
        }

        trunk(blocks, 0, 0, 1, top);
    }

    /// Ель. Шпиль — три слоя над макушкой, лист и крест через один; ниже
    /// до второго бревна гармошка: узкий ярус (крест вокруг ствола) через
    /// один с широким, и широкие всё шире книзу — 2, 2, 3. Углы широких
    /// ярусов срезаны всегда.
    fn spruce(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;
        let mut narrow = roll(random, 2) == 0;
        let mut wide = 0;

        for y in (2..=top + 3).rev() {
            let radius = if narrow {
                i32::from(y <= top)
            } else {
                wide += 1;
                [1, 2, 2, 3][(wide - 1).min(3) as usize]
            };

            layer_without_corners(blocks, 0, y, 0, radius);
            narrow = !narrow;
        }

        trunk(blocks, 0, 0, 1, top);
    }

    /// Сосна: длинный голый ствол, у макушки зонтик. Над стволом лист и
    /// крест, на уровне макушки и чуть ниже — один-три слоя шириной до
    /// семи блоков без углов.
    fn pine(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;
        let umbrella: &[i32] = match roll(random, 3) {
            0 => &[1],
            1 => &[2, 2],
            _ => &[2, 3, 2],
        };

        layer_without_corners(blocks, 0, top + 2, 0, 0);
        layer_without_corners(blocks, 0, top + 1, 0, 1);

        for (depth, radius) in umbrella.iter().enumerate() {
            layer_without_corners(blocks, 0, top - depth as i32, 0, *radius);
        }

        trunk(blocks, 0, 0, 1, top);
    }

    /// Большая ель и большая сосна 2×2. Верх ствола неровный: до макушки
    /// доходит один столбец из четырёх, над ним — шапка 2×2. У ели ярусы
    /// спускаются на шестнадцать-семнадцать слоёв и расходятся книзу до
    /// четырёх блоков от ствола; у сосны ствол голый, крона — зонтик в
    /// пять-шесть верхних слоёв. Под деревом пятно подзола.
    fn mega_conifer(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>, pine: bool) {
        let top = self.trunk;
        let peak = top + 1;

        if pine {
            let radii: &[i32] = if roll(random, 2) == 0 { &[0, 0, 1, 2, 3] } else { &[0, 0, 1, 2, 3, 2] };

            for (depth, radius) in radii.iter().enumerate() {
                round_layer(blocks, WIDE, peak - depth as i32, *radius);
            }
        } else {
            let bottom = (peak - 14 - roll(random, 4)).max(1);

            for y in bottom..=peak {
                let depth = peak - y;
                // Книзу крона шире на блок каждые четыре яруса, ярус через
                // ярус уже: так она выглядит ступенями.
                let radius = if depth <= 1 { 0 } else { ((1 + depth / 4).min(4) - depth % 2).max(0) };

                round_layer(blocks, WIDE, y, radius);
            }
        }

        trunk_wide(blocks, 1, top - 1);
        blocks.push((0, top, 0, Part::Log(Axis::Y)));
        podzol(random, blocks);
    }

    /// Малое дерево джунглей: крона как у берёзы, по стволу и с кроны
    /// свисают лианы, у одного дерева из двенадцати на нижней трети ствола
    /// висит стручок какао.
    fn jungle(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        round_crown(random, blocks, top, 0);
        trunk(blocks, 0, 0, 1, top);

        // Какао — раньше лиан: блоки ставятся в воздух, кто первый встал,
        // тот и остаётся.
        if roll(random, 12) == 0 {
            let (x, z, ages) = COCOA[roll(random, 4) as usize];
            let y = 1 + roll(random, (top / 3).max(1));

            blocks.push((x, y, z, Part::Block(ages[roll(random, 3) as usize])));
        }

        for y in 1..=top {
            hang_vines(random, blocks, y, 1, (3, 1), |dx, dz| (dx, dz) == (0, 0));
        }

        hang_vines(random, blocks, top + 1, 1, (2, top), |dx, dz| square_without_corners(dx, dz, 1));
        hang_vines(random, blocks, top - 1, 2, (2, top), |dx, dz| square_without_corners(dx, dz, 2));
    }

    /// Большое дерево джунглей 2×2: широкая шапка на макушке, по стволу
    /// через два-четыре блока — ветви с шапками поменьше, всё увито лианами.
    fn mega_jungle(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        for (dy, radius) in [(-1, 4), (0, 5), (1, 3)] {
            round_layer(blocks, WIDE, top + dy, radius);
        }

        let mut y = top - 3 - roll(random, 2);

        while y > top / 2 {
            let angle = fraction(random) * std::f64::consts::TAU;
            let length = 2.0 + fraction(random) * 3.0;
            // Ветвь выходит из того ряда ствола, что смотрит в её сторону.
            let (from_x, from_z) = (i32::from(angle.cos() > 0.0), i32::from(angle.sin() > 0.0));
            let end = (
                from_x + (angle.cos() * length).round() as i32,
                y + 1 + roll(random, 2),
                from_z + (angle.sin() * length).round() as i32,
            );

            line(blocks, (from_x, y, from_z), end);

            let at = Center { x: end.0, z: end.2, wide: false };

            round_layer(blocks, at, end.1, 2);
            round_layer(blocks, at, end.1 + 1, 1);
            hang_vines(random, blocks, end.1, 3, (2, 6), |dx, dz| inside_round(at, dx, dz, 2));

            y -= 2 + roll(random, 3);
        }

        trunk_wide(blocks, 1, top);

        for y in 1..=top {
            hang_vines(random, blocks, y, 1, (2, 1), |dx, dz| inside_round(WIDE, dx, dz, -1));
        }

        hang_vines(random, blocks, top - 1, 5, (2, 8), |dx, dz| inside_round(WIDE, dx, dz, 4));
        hang_vines(random, blocks, top, 6, (3, 6), |dx, dz| inside_round(WIDE, dx, dz, 5));
    }

    /// Акация: ствол прямо, потом наискосок в одну сторону; наверху плоская
    /// крона 7×7 без углов и ромб над ней. У трёх деревьев из четырёх из
    /// ствола пониже выходит вторая ветвь со своей кроной поменьше.
    fn acacia(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk.max(4);
        let bend = (top - 2 - roll(random, 3)).max(3);
        let main = roll(random, 4);
        let (step_x, step_z) = CARDINAL[main as usize];
        let (mut x, mut z) = (0, 0);

        for y in 1..=top {
            if y > bend {
                x += step_x;
                z += step_z;
            }

            blocks.push((x, y, z, Part::Log(Axis::Y)));
        }

        flat_crown(blocks, x, top, z, 3);

        // Вторая ветвь смотрит в другую сторону и кончается ниже первой.
        if roll(random, 4) != 0 {
            let (side_x, side_z) = CARDINAL[((main + 1 + roll(random, 3)) % 4) as usize];
            let from = (bend - roll(random, 2)).max(1);
            let length = 1 + roll(random, 3).min((top - 2 - from).max(0));

            for k in 1..=length {
                blocks.push((side_x * k, from + k, side_z * k, Part::Log(Axis::Y)));
            }

            flat_crown(blocks, side_x * length, from + length, side_z * length, 2);
        }
    }

    /// Тёмный и бледный дуб 2×2. Верх ствола неровный: один-два столбца из
    /// четырёх тянутся на один-три блока выше, оттуда наискосок вверх
    /// расходятся короткие ветви. Крона — над самым высоким столбцом:
    /// рваный слой, широкий круг, круг поменьше и шапка 2×2. С бледного
    /// дуба свисает бледный мох, под ним на земле — бледный мох и ковёр.
    fn dark_oak(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>, pale: bool) {
        let base = self.trunk.max(3);
        let columns = [(0, 0), (1, 0), (0, 1), (1, 1)];
        let mut extra = [0; 4];

        for more in &mut extra {
            *more = [0, 0, 0, 1, 2, 3][roll(random, 6) as usize];
        }

        if extra.iter().all(|&more| more == 0) {
            extra[roll(random, 4) as usize] = 1 + roll(random, 3);
        }

        let crown = base + extra.iter().max().copied().unwrap_or(0);

        // Низ ствола — от самой земли, чтобы на неровном месте под
        // соседними рядами не зияла дыра.
        for ((x, z), more) in columns.into_iter().zip(extra) {
            trunk(blocks, x, z, 0, base + more);
        }

        // Ветви: от верха ствола наискосок вверх, по бревну на ступень,
        // на конце — клок листвы.
        for _ in 0..(3 + roll(random, 3)) {
            let (step_x, step_z) = ALL_SIDES[roll(random, 8) as usize];
            let length = 1 + roll(random, 3);
            let (mut x, mut z) = (i32::from(step_x > 0), i32::from(step_z > 0));
            let mut y = crown - 1 - length - roll(random, 2);

            for _ in 0..length {
                x += step_x;
                z += step_z;
                y += 1;
                blocks.push((x, y, z, Part::Log(Axis::Y)));
            }

            square_layer(random, blocks, x, y, z, 1, Corners::Random(2));
        }

        // Рваный слой: край выщерблен через раз.
        for dx in -5..=6 {
            for dz in -5..=6 {
                let (ex, ez) = (span(dx, true), span(dz, true));
                let distance = ex * ex + ez * ez;

                if distance <= 25 && (distance <= 12 || roll(random, 2) == 0) {
                    blocks.push((dx, crown - 1, dz, Part::Leaves));
                }
            }
        }

        round_layer(blocks, WIDE, crown, 4);
        round_layer(blocks, WIDE, crown + 1, 2);
        round_layer(blocks, WIDE, crown + 2, 0);

        if !pale {
            return;
        }

        // Ещё один редкий слой под кроной: бледный дуб на слой выше.
        for dx in -3..=4 {
            for dz in -3..=4 {
                if inside_round(WIDE, dx, dz, 3) && roll(random, 2) == 0 {
                    blocks.push((dx, crown - 2, dz, Part::Leaves));
                }
            }
        }

        // Мох свисает из-под кроны плетями в один-три блока; кончик плети —
        // с пометкой tip.
        for dx in -5..=6 {
            for dz in -5..=6 {
                let (ex, ez) = (span(dx, true), span(dz, true));

                if ex * ex + ez * ez > 20 || (ex, ez) == (0, 0) || roll(random, 8) != 0 {
                    continue;
                }

                let length = 1 + roll(random, 3);
                let from = if inside_round(WIDE, dx, dz, 3) { crown - 3 } else { crown - 2 };

                for k in 0..length {
                    let part = if k == length - 1 { "pale_hanging_moss[tip=true]" } else { "pale_hanging_moss[tip=false]" };

                    blocks.push((dx, from - k, dz, Part::Block(part)));
                }
            }
        }

        // Бледный мох на земле под кроной — у четырёх деревьев из пяти,
        // местами на нём ковёр.
        if roll(random, 5) != 0 {
            for dx in -4..=5 {
                for dz in -4..=5 {
                    let (ex, ez) = (span(dx, true), span(dz, true));
                    let distance = ex * ex + ez * ez;

                    if (ex, ez) == (0, 0) || distance > 16 || (distance > 6 && roll(random, 3) == 0) {
                        continue;
                    }

                    blocks.push((dx, 0, dz, Part::Ground("pale_moss_block")));

                    if roll(random, 7) == 0 {
                        blocks.push((dx, 1, dz, Part::Block("pale_moss_carpet")));
                    }
                }
            }
        }
    }

    /// Вишня: прямой ствол, от его верха — одна-три ветви: вбок, потом
    /// вверх, над каждой — свой купол кроны. Одна ветвь короткая, и купол
    /// стоит почти над стволом; две-три расходятся в разные стороны.
    fn cherry(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;
        let branches = 1 + roll(random, 2) + i32::from(roll(random, 4) == 0);
        let first = roll(random, 4);

        for index in 0..branches {
            // Первые две смотрят в разные стороны, третья — поперёк им.
            let (step_x, step_z) = CARDINAL[((first + [0, 2, 1][index as usize]) % 4) as usize];
            let length = if branches == 1 { 1 + roll(random, 2) } else { 2 + roll(random, 4) };
            let axis = if step_x != 0 { Axis::X } else { Axis::Z };
            let (mut bx, mut bz) = (0, 0);

            for _ in 0..length {
                bx += step_x;
                bz += step_z;
                blocks.push((bx, top + 1, bz, Part::Log(axis)));
            }

            let rise = 1 + roll(random, 4);

            for dy in 1..=rise {
                blocks.push((bx, top + 1 + dy, bz, Part::Log(Axis::Y)));
            }

            dome(random, blocks, bx, top + 1 + rise, bz);
        }

        trunk(blocks, 0, 0, 1, top);
    }

    /// Мангр: ствол поднят на корнях, от его основания во все стороны
    /// расходится юбка корней — наискосок вниз до земли. Крона пышная, у
    /// двух деревьев из трёх (у высокого — почти у всех) от ствола отходят
    /// ветви со своими клоками листвы. С кроны свисают лианы и проростки,
    /// на корнях кое-где мох.
    fn mangrove(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>, tall: bool) {
        let rise = if tall { 2 + roll(random, 2) } else { 1 + roll(random, 2) };
        let top = rise + self.trunk.max(2);

        for y in 1..=rise {
            blocks.push((0, y, 0, Part::Block("mangrove_roots")));
        }

        blocks.push((0, 0, 0, Part::Ground("muddy_mangrove_roots")));

        let arms = if tall { 5 + roll(random, 3) } else { 2 + roll(random, 3) };
        let first = roll(random, 8);

        // Корни смотрят в разные стороны: шаг по кругу через три
        // направления обходит все восемь, не повторяясь.
        for arm in 0..arms {
            let (step_x, step_z) = ALL_SIDES[((first + arm * 3) % 8) as usize];
            let length = if tall { 3 + roll(random, 4) } else { 2 + roll(random, 3) };

            // Корень начинается у ствола и к концу опускается до земли.
            for s in 1..=length {
                let (x, z) = (step_x * s, step_z * s);
                let y = 1 + (rise * (length - s) + length - 1) / length;

                blocks.push((x, y, z, Part::Block("mangrove_roots")));

                // Мох — на четырёх корнях из пяти, у высокого мангра — на
                // каждом четвёртом.
                if (tall && roll(random, 4) == 0) || (!tall && roll(random, 5) != 0) {
                    blocks.push((x, y + 1, z, Part::Block("moss_carpet")));
                }
            }

            blocks.push((step_x * length, 0, step_z * length, Part::Ground("muddy_mangrove_roots")));
        }

        trunk(blocks, 0, 0, rise + 1, top);

        let center = Center { x: 0, z: 0, wide: false };
        let crown: &[(i32, i32)] = if tall {
            &[(-3, 3), (-2, 4), (-1, 5), (0, 5), (1, 3), (2, 1)]
        } else {
            &[(-2, 2), (-1, 3), (0, 3), (1, 2)]
        };

        for (dy, radius) in crown {
            round_layer(blocks, center, top + dy, *radius);
        }

        let branches = if tall {
            if roll(random, 25) == 0 { 0 } else { 2 + roll(random, 3) }
        } else if roll(random, 3) == 0 {
            0
        } else {
            1 + roll(random, 2)
        };

        for _ in 0..branches {
            let (step_x, step_z) = ALL_SIDES[roll(random, 8) as usize];
            let length = if tall { 2 + roll(random, 4) } else { 2 + roll(random, 2) };
            let from = rise + 2 + roll(random, (top - rise - 2).max(1));
            let end = (step_x * length, (from + length).min(top + 1), step_z * length);
            let at = Center { x: end.0, z: end.2, wide: false };

            line(blocks, (0, from, 0), end);
            round_layer(blocks, at, end.1, 2);
            round_layer(blocks, at, end.1 + 1, 1);
            hang_vines(random, blocks, end.1, 3, (2, top), |dx, dz| inside_round(at, dx, dz, 2));
        }

        for y in (rise + 1)..=top {
            hang_vines(random, blocks, y, 1, (if tall { 4 } else { 2 }, 1), |dx, dz| (dx, dz) == (0, 0));
        }

        let widest = crown.iter().map(|(_, radius)| *radius).max().unwrap_or(3);

        hang_vines(random, blocks, top - 1, widest, (2, top + 2), |dx, dz| {
            inside_round(center, dx, dz, widest)
        });

        // Проростки висят под нижними листьями.
        for dx in -widest..=widest {
            for dz in -widest..=widest {
                if !inside_round(center, dx, dz, widest) || (dx, dz) == (0, 0) || roll(random, 10) != 0 {
                    continue;
                }

                let y = if inside_round(center, dx, dz, crown[0].1) { top + crown[0].0 - 1 } else { top - 2 };

                blocks.push((dx, y, dz, Part::Block(PROPAGULES[roll(random, 5) as usize])));
            }
        }

        if self.bees {
            blocks.push((0, top + crown[0].0 - 1, 1, Part::Block("bee_nest[facing=south]")));
        }
    }

    /// Дерево с азалией: короткий ствол на корнистой земле, от макушки
    /// одна-две ветви наискосок вверх. Крона раздувается кверху: у макушки
    /// слой 5×5, над концами ветвей — шире. Листва простая и цветущая
    /// вперемешку.
    fn azalea(&self, random: &mut Random, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        blocks.push((0, 0, 0, Part::Ground("rooted_dirt")));
        square_layer(random, blocks, 0, top, 0, 2, Corners::Random(2));

        for _ in 0..(1 + roll(random, 2)) {
            let (step_x, step_z) = ALL_SIDES[roll(random, 8) as usize];
            let (mut x, mut y, mut z) = (0, top, 0);

            for _ in 0..(1 + roll(random, 3)) {
                x += step_x;
                z += step_z;
                y += 1;
                blocks.push((x, y, z, Part::Log(Axis::Y)));
            }

            for _ in 0..roll(random, 2) {
                y += 1;
                blocks.push((x, y, z, Part::Log(Axis::Y)));
            }

            square_layer(random, blocks, x, y, z, 3, Corners::Cut);
            square_layer(random, blocks, x, y + 1, z, 2, Corners::Random(2));
            square_layer(random, blocks, x, y + 2, z, 1, Corners::Cut);
        }

        trunk(blocks, 0, 0, 1, top);

        for block in blocks.iter_mut() {
            if block.3 == Part::Leaves && roll(random, 3) == 0 {
                block.3 = Part::Block("flowering_azalea_leaves");
            }
        }
    }

    /// Огромный коричневый гриб: ножка и плоская сплошная шляпка 7×7 без
    /// углов прямо над ней.
    fn brown_mushroom(&self, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        for dx in -3..=3 {
            for dz in -3..=3 {
                if square_without_corners(dx, dz, 3) {
                    blocks.push((dx, top + 1, dz, Part::Leaves));
                }
            }
        }

        trunk(blocks, 0, 0, 1, top);
    }

    /// Огромный красный гриб: ножка и купол — крышка 3×3 и три ряда стенок
    /// 5×5 без углов, внутри пусто. Изнанка стенок, что смотрит внутрь, —
    /// с порами.
    fn red_mushroom(&self, blocks: &mut Vec<TreeBlock>) {
        let top = self.trunk;

        square_top(blocks, top + 1);

        for y in (top - 2).max(1)..=top {
            for k in -1..=1 {
                blocks.push((2, y, k, Part::Block("red_mushroom_block[west=false,down=false]")));
                blocks.push((-2, y, k, Part::Block("red_mushroom_block[east=false,down=false]")));
                blocks.push((k, y, 2, Part::Block("red_mushroom_block[north=false,down=false]")));
                blocks.push((k, y, -2, Part::Block("red_mushroom_block[south=false,down=false]")));
            }
        }

        trunk(blocks, 0, 0, 1, top);
    }
}

/// Середина кроны: над каким столбцом она стоит и толстый ли ствол.
#[derive(Clone, Copy)]
struct Center {
    x: i32,
    z: i32,
    /// Ствол 2×2: крона считается от ближайшего из четырёх столбцов.
    wide: bool,
}

/// Середина кроны толстого ствола, стоящего на месте.
const WIDE: Center = Center { x: 0, z: 0, wide: true };

/// Четыре стороны света.
const CARDINAL: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// Все восемь направлений.
const ALL_SIDES: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];

/// Лиана по сторонам опоры: смещение от опоры к лиане и состояние лианы —
/// у неё поднято то свойство, с какой стороны опора.
const VINES: [(i32, i32, &str); 4] = [
    (1, 0, "vine[west=true]"),
    (-1, 0, "vine[east=true]"),
    (0, 1, "vine[north=true]"),
    (0, -1, "vine[south=true]"),
];

/// Какао по сторонам ствола: смещение от ствола и состояния по возрасту.
/// `facing` смотрит на ствол, к которому стручок прирос.
const COCOA: [(i32, i32, [&str; 3]); 4] = [
    (1, 0, ["cocoa[facing=west,age=0]", "cocoa[facing=west,age=1]", "cocoa[facing=west,age=2]"]),
    (-1, 0, ["cocoa[facing=east,age=0]", "cocoa[facing=east,age=1]", "cocoa[facing=east,age=2]"]),
    (0, 1, ["cocoa[facing=north,age=0]", "cocoa[facing=north,age=1]", "cocoa[facing=north,age=2]"]),
    (0, -1, ["cocoa[facing=south,age=0]", "cocoa[facing=south,age=1]", "cocoa[facing=south,age=2]"]),
];

/// Висячие проростки мангра разного возраста.
const PROPAGULES: [&str; 5] = [
    "mangrove_propagule[hanging=true,age=0]",
    "mangrove_propagule[hanging=true,age=1]",
    "mangrove_propagule[hanging=true,age=2]",
    "mangrove_propagule[hanging=true,age=3]",
    "mangrove_propagule[hanging=true,age=4]",
];

/// Как поступать с углами квадратного слоя листвы.
#[derive(Clone, Copy, PartialEq)]
enum Corners {
    Keep,
    Cut,
    /// Срезать каждый угол с шансом один к стольким.
    Random(i32),
}

/// Трава под нижним бревном любого дерева становится землёй: у каждого
/// столбца, где бревно стоит прямо на земле, а замены земли ещё нет.
fn dirt_under_trunk(blocks: &mut Vec<TreeBlock>) {
    let under: Vec<(i32, i32)> = blocks
        .iter()
        .filter(|(_, y, _, part)| *y == 1 && matches!(part, Part::Log(_)))
        .map(|&(x, _, z, _)| (x, z))
        .filter(|&(x, z)| !blocks.iter().any(|&(bx, by, bz, _)| (bx, by, bz) == (x, 0, z)))
        .collect();

    for (x, z) in under {
        if !blocks.iter().any(|&(bx, by, bz, _)| (bx, by, bz) == (x, 0, z)) {
            blocks.push((x, 0, z, Part::Ground("dirt")));
        }
    }
}

/// Ствол от `from` до `to` включительно.
fn trunk(blocks: &mut Vec<TreeBlock>, x: i32, z: i32, from: i32, to: i32) {
    for y in from..=to {
        blocks.push((x, y, z, Part::Log(Axis::Y)));
    }
}

/// Толстый ствол 2×2 от `from` до `to`.
fn trunk_wide(blocks: &mut Vec<TreeBlock>, from: i32, to: i32) {
    for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        trunk(blocks, x, z, from, to);
    }
}

/// Крона берёзы над стволом высотой `top`: слои 5×5, 5×5, 3×3 и крест.
/// `wider` расширяет все слои — у болотного дуба на блок.
fn round_crown(random: &mut Random, blocks: &mut Vec<TreeBlock>, top: i32, wider: i32) {
    for (dy, radius) in [(-2, 2), (-1, 2), (0, 1), (1, 1)] {
        let corners = if dy == 1 { Corners::Cut } else { Corners::Random(2) };

        square_layer(random, blocks, 0, top + dy, 0, radius + wider, corners);
    }
}

/// Куст джунглей: бревно у земли, вокруг листва горкой — слой 5×5, слой
/// 3×3 над ним, углы срезаны через раз, и у половины кустов лист на
/// макушке.
fn bush(random: &mut Random, blocks: &mut Vec<TreeBlock>) {
    square_layer(random, blocks, 0, 1, 0, 2, Corners::Random(2));
    square_layer(random, blocks, 0, 2, 0, 1, Corners::Random(2));

    if roll(random, 2) == 0 {
        blocks.push((0, 3, 0, Part::Leaves));
    }

    blocks.push((0, 1, 0, Part::Log(Axis::Y)));
}

/// Квадратный слой листвы.
fn square_layer(random: &mut Random, blocks: &mut Vec<TreeBlock>, x: i32, y: i32, z: i32, radius: i32, corners: Corners) {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let corner = radius > 0 && dx.abs() == radius && dz.abs() == radius;
            let cut = match corners {
                Corners::Keep => false,
                Corners::Cut => true,
                Corners::Random(odds) => roll(random, odds) == 0,
            };

            if corner && cut {
                continue;
            }

            blocks.push((x + dx, y, z + dz, Part::Leaves));
        }
    }
}

/// Квадратный слой листвы без углов; радиус 0 — один лист.
fn layer_without_corners(blocks: &mut Vec<TreeBlock>, x: i32, y: i32, z: i32, radius: i32) {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if radius == 0 || square_without_corners(dx, dz, radius) {
                blocks.push((x + dx, y, z + dz, Part::Leaves));
            }
        }
    }
}

/// Крышка красного гриба: квадрат 3×3.
fn square_top(blocks: &mut Vec<TreeBlock>, y: i32) {
    for dx in -1..=1 {
        for dz in -1..=1 {
            blocks.push((dx, y, dz, Part::Leaves));
        }
    }
}

/// Внутри ли квадрата без углов.
fn square_without_corners(dx: i32, dz: i32, radius: i32) -> bool {
    dx.abs() <= radius && dz.abs() <= radius && !(dx.abs() == radius && dz.abs() == radius)
}

/// Расстояние по одной оси от ствола. У толстого ствола — от ближайшего
/// из двух его рядов.
fn span(d: i32, wide: bool) -> i32 {
    if d < 0 {
        -d
    } else if wide {
        (d - 1).max(0)
    } else {
        d
    }
}

/// Внутри ли скруглённого слоя радиусом `radius` вокруг середины.
/// Радиус −1 — только сам ствол.
fn inside_round(at: Center, dx: i32, dz: i32, radius: i32) -> bool {
    let (ex, ez) = (span(dx - at.x, at.wide), span(dz - at.z, at.wide));

    if radius < 0 {
        return (ex, ez) == (0, 0);
    }

    ex * ex + ez * ez <= radius * radius + radius
}

/// Скруглённый слой листвы: без углов, но полнее ромба.
fn round_layer(blocks: &mut Vec<TreeBlock>, at: Center, y: i32, radius: i32) {
    for dx in (at.x - radius)..=(at.x + radius + i32::from(at.wide)) {
        for dz in (at.z - radius)..=(at.z + radius + i32::from(at.wide)) {
            if inside_round(at, dx, dz, radius) {
                blocks.push((dx, y, dz, Part::Leaves));
            }
        }
    }
}

/// Плоский круг листвы. `cut` — срезать углы квадрата.
fn disc(blocks: &mut Vec<TreeBlock>, x: i32, y: i32, z: i32, radius: i32, cut: bool) {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if radius > 0 && dx.abs() + dz.abs() > radius + i32::from(cut) {
                continue;
            }

            blocks.push((x + dx, y, z + dz, Part::Leaves));
        }
    }
}

/// Плоская крона акации: нижний слой — квадрат без углов радиусом
/// `radius`, верхний — ромб на блок уже.
fn flat_crown(blocks: &mut Vec<TreeBlock>, x: i32, y: i32, z: i32, radius: i32) {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if square_without_corners(dx, dz, radius) {
                blocks.push((x + dx, y, z + dz, Part::Leaves));
            }

            if dx.abs() + dz.abs() < radius {
                blocks.push((x + dx, y + 1, z + dz, Part::Leaves));
            }
        }
    }
}

/// Лианы вокруг слоя на высоте `y`: у каждого места рядом со слоем (но не
/// в нём) с шансом один к `odds.0` свисает плеть длиной до `odds.1`
/// блоков. `inside` — занято ли место слоем, `reach` — докуда слой
/// тянется от ствола.
fn hang_vines(
    random: &mut Random,
    blocks: &mut Vec<TreeBlock>,
    y: i32,
    reach: i32,
    (odds, longest): (i32, i32),
    inside: impl Fn(i32, i32) -> bool,
) {
    for dx in (-reach - 1)..=(reach + 2) {
        for dz in (-reach - 1)..=(reach + 2) {
            if inside(dx, dz) {
                continue;
            }

            let Some(&(_, _, vine)) = VINES.iter().find(|(sx, sz, _)| inside(dx - sx, dz - sz)) else {
                continue;
            };

            if roll(random, odds) != 0 {
                continue;
            }

            let length = 1 + roll(random, longest);

            // Ниже первого блока над землёй плеть не спускается.
            for k in 0..length.min(y) {
                blocks.push((dx, y - k, dz, Part::Block(vine)));
            }
        }
    }
}

/// Подзол вокруг толстого ствола: пятно радиусом около шести, край
/// неровный.
fn podzol(random: &mut Random, blocks: &mut Vec<TreeBlock>) {
    for dx in -6..=7 {
        for dz in -6..=7 {
            let (ex, ez) = (span(dx, true), span(dz, true));
            let distance = ex * ex + ez * ez;

            if (ex, ez) == (0, 0) || distance > 36 || (distance > 20 && roll(random, 3) == 0) {
                continue;
            }

            blocks.push((dx, 0, dz, Part::Ground("podzol")));
        }
    }
}

/// Шапка листвы большого дуба: сплюснутый шар четыре блока высотой.
fn cluster(blocks: &mut Vec<TreeBlock>, x: i32, y: i32, z: i32) {
    for (dy, radius) in [(-1, 2), (0, 3), (1, 3), (2, 2)] {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx * dx + dz * dz > radius * radius - 1 + i32::from(radius == 2) * 2 {
                    continue;
                }

                blocks.push((x + dx, y + dy, z + dz, Part::Leaves));
            }
        }
    }
}

/// Купол вишни над концом ветви: снизу редкий слой, два широких слоя 7×7
/// с неровными углами, слой 5×5 и крест; по нижнему краю листва местами
/// свисает на блок.
fn dome(random: &mut Random, blocks: &mut Vec<TreeBlock>, x: i32, y: i32, z: i32) {
    for dx in -3..=3 {
        for dz in -3..=3 {
            if square_without_corners(dx, dz, 3) && roll(random, 2) == 0 {
                blocks.push((x + dx, y - 1, z + dz, Part::Leaves));

                if (dx.abs() == 3 || dz.abs() == 3) && roll(random, 3) == 0 {
                    blocks.push((x + dx, y - 2, z + dz, Part::Leaves));
                }
            }
        }
    }

    square_layer(random, blocks, x, y, z, 3, Corners::Random(2));
    square_layer(random, blocks, x, y + 1, z, 3, Corners::Random(3));
    square_layer(random, blocks, x, y + 2, z, 2, Corners::Keep);
    square_layer(random, blocks, x, y + 3, z, 1, Corners::Random(2));
}

/// Ветвь бревнами из одной точки в другую — по ближайшим блокам прямой.
/// Бревно лежит вдоль той оси, по которой ветвь идёт дальше всего.
fn line(blocks: &mut Vec<TreeBlock>, from: (i32, i32, i32), to: (i32, i32, i32)) {
    let delta = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let steps = delta.0.abs().max(delta.1.abs()).max(delta.2.abs());
    let axis = if delta.1.abs() >= delta.0.abs().max(delta.2.abs()) {
        Axis::Y
    } else if delta.0.abs() >= delta.2.abs() {
        Axis::X
    } else {
        Axis::Z
    };

    for step in 1..=steps {
        let part = step as f64 / steps as f64;
        let at = |from: i32, delta: i32| from + (delta as f64 * part).round() as i32;

        blocks.push((at(from.0, delta.0), at(from.1, delta.1), at(from.2, delta.2), Part::Log(axis)));
    }
}

/// Случайное число от 0 до `below - 1`.
fn roll(random: &mut Random, below: i32) -> i32 {
    (random.next() % below.max(1) as u64) as i32
}

/// Случайная доля от 0 до 1.
fn fraction(random: &mut Random) -> f64 {
    (random.next() >> 11) as f64 / (1u64 << 53) as f64
}

/// Порода: заготовка дерева и разброс высоты ствола.
#[derive(Clone, Copy)]
struct Species {
    tree: Tree,
    /// Ствол — `tree.trunk` плюс два случайных добавка: до `spread - 1`
    /// и до `extra - 1`. Два добавка вместо одного дают высоты,
    /// сгущённые к середине, как у оригинала.
    spread: i32,
    extra: i32,
}

/// Порода с таким стволом, кроной, формой и высотой ствола от `low`
/// до `low + spread + extra - 2`.
const fn species(log: &'static str, leaves: &'static str, shape: Shape, low: i32, spread: i32, extra: i32) -> Species {
    Species { tree: Tree { log, leaves, trunk: low, shape, bees: false }, spread, extra }
}

// Высоты — по замеру: у каждой породы указан замеренный разброс и среднее.

/// 5–6, среднее 5.52.
const OAK: Species = species("oak_log", "oak_leaves", Shape::Oak, 5, 2, 1);
/// 4–12, среднее 8.2.
const FANCY_OAK: Species = species("oak_log", "oak_leaves", Shape::Fancy, 4, 5, 5);
const SWAMP_OAK: Species = species("oak_log", "oak_leaves", Shape::Swamp, 5, 4, 1);
/// 5–7, среднее 6.0.
const BIRCH: Species = species("birch_log", "birch_leaves", Shape::Round, 5, 3, 1);
/// 5–12, среднее 8.36.
const TALL_BIRCH: Species = species("birch_log", "birch_leaves", Shape::Round, 5, 3, 6);
/// 5–8, среднее 6.28.
const SPRUCE: Species = species("spruce_log", "spruce_leaves", Shape::Spruce, 5, 4, 1);
/// 6–10, среднее 7.88.
const PINE: Species = species("spruce_log", "spruce_leaves", Shape::Pine, 6, 5, 1);
/// 14–29, среднее 20.64.
const MEGA_SPRUCE: Species = species("spruce_log", "spruce_leaves", Shape::MegaSpruce, 14, 8, 8);
/// 13–28, среднее 21.16.
const MEGA_PINE: Species = species("spruce_log", "spruce_leaves", Shape::MegaPine, 13, 8, 9);
/// 4–12, среднее 8.64.
const JUNGLE: Species = species("jungle_log", "jungle_leaves", Shape::Jungle, 4, 5, 5);
/// 10–31, среднее 21.2.
const MEGA_JUNGLE: Species = species("jungle_log", "jungle_leaves", Shape::MegaJungle, 10, 11, 11);
/// Одно бревно; листва у куста дубовая, как у оригинала.
const JUNGLE_BUSH: Species = species("jungle_log", "oak_leaves", Shape::Bush, 1, 1, 1);
/// Полная высота 5–10; прямой участок до изгиба 3–8, среднее 4.36.
const ACACIA: Species = species("acacia_log", "acacia_leaves", Shape::Acacia, 5, 3, 4);
/// Прямой столб 2×2 до неровного верха; вместе с ним 5–9, среднее 7.32.
const DARK_OAK: Species = species("dark_oak_log", "dark_oak_leaves", Shape::DarkOak, 5, 4, 1);
/// 5–9, среднее 7.28.
const PALE_OAK: Species = species("pale_oak_log", "pale_oak_leaves", Shape::PaleOak, 5, 4, 1);
/// Прямой ствол до развилки 3–8, среднее 5.
const CHERRY: Species = species("cherry_log", "cherry_leaves", Shape::Cherry, 3, 3, 3);
/// Брёвна над корнями 2–7, среднее 4.48.
const MANGROVE: Species = species("mangrove_log", "mangrove_leaves", Shape::Mangrove, 2, 6, 1);
/// 5–15, среднее 10.8.
const TALL_MANGROVE: Species = species("mangrove_log", "mangrove_leaves", Shape::TallMangrove, 5, 6, 6);
/// 2–5, среднее 3.88. Растёт над пышными пещерами, которых у нас пока нет:
/// порода есть, в лесах её не выбирают.
#[cfg_attr(not(test), allow(dead_code))]
const AZALEA: Species = species("oak_log", "azalea_leaves", Shape::Azalea, 3, 3, 1);
/// 4–8, среднее 5.2.
const BROWN_MUSHROOM: Species = species(
    "mushroom_stem[up=false,down=false]",
    "brown_mushroom_block[down=false]",
    Shape::BrownMushroom,
    4,
    3,
    2,
);
/// 4–6, среднее 5.
const RED_MUSHROOM: Species = species(
    "mushroom_stem[up=false,down=false]",
    "red_mushroom_block[down=false]",
    Shape::RedMushroom,
    4,
    3,
    1,
);

/// Состав леса: доли пород, в сумме единица.
type Mix = &'static [(f64, Species)];

/// Лес: дуб и берёза, среди дубов — большие.
const FOREST: Mix = &[(0.1, FANCY_OAK), (0.2, BIRCH), (0.7, OAK)];
/// Тёмный лес: тёмный дуб, огромные грибы, берёза и дуб.
const DARK_FOREST: Mix = &[
    (0.66, DARK_OAK),
    (0.02, BROWN_MUSHROOM),
    (0.02, RED_MUSHROOM),
    (0.1, BIRCH),
    (0.02, FANCY_OAK),
    (0.18, OAK),
];
/// Тайга: ель и сосна.
const TAIGA: Mix = &[(0.33, PINE), (0.67, SPRUCE)];
/// Старовозрастная еловая тайга: треть — большие ели.
const SPRUCE_TAIGA: Mix = &[(0.33, MEGA_SPRUCE), (0.2, PINE), (0.47, SPRUCE)];
/// Старовозрастная сосновая тайга: треть — большие сосны.
const PINE_TAIGA: Mix = &[(0.33, MEGA_PINE), (0.2, PINE), (0.47, SPRUCE)];
/// Джунгли: половина — кусты, остальное большие и малые деревья и большие дубы.
const JUNGLE_MIX: Mix = &[(0.1, FANCY_OAK), (0.5, JUNGLE_BUSH), (0.13, MEGA_JUNGLE), (0.27, JUNGLE)];
/// Редкие джунгли: без больших деревьев джунглей.
const SPARSE_JUNGLE: Mix = &[(0.1, FANCY_OAK), (0.5, JUNGLE_BUSH), (0.4, JUNGLE)];
/// Саванна: акация и редкий дуб.
const SAVANNA: Mix = &[(0.8, ACACIA), (0.2, OAK)];
/// Равнины: одинокие дубы, треть из них большие.
const PLAINS: Mix = &[(0.3, FANCY_OAK), (0.7, OAK)];
/// Луг: одинокие берёзы, больше половины высокие.
const MEADOW: Mix = &[(0.6, TALL_BIRCH), (0.4, BIRCH)];
/// Ветреные холмы и лес: ели с дубами.
const WINDSWEPT: Mix = &[(0.67, SPRUCE), (0.3, OAK), (0.03, FANCY_OAK)];
/// Мангровое болото: среди обычных мангров изредка высокие.
const MANGROVES: Mix = &[(0.85, MANGROVE), (0.15, TALL_MANGROVE)];

/// Густота деревьев в биоме (доля столбцов со стволом), их состав и доля
/// деревьев с пчелиным гнездом.
fn woodland(biome: Biome) -> Option<(f64, Mix, f64)> {
    Some(match biome {
        Biome::Forest => (0.035, FOREST, 0.0),
        Biome::FlowerForest => (0.02, FOREST, 0.02),
        Biome::BirchForest => (0.035, &[(1.0, BIRCH)], 0.0),
        Biome::OldGrowthBirchForest => (0.045, &[(0.5, TALL_BIRCH), (0.5, BIRCH)], 0.0),
        Biome::DarkForest => (0.06, DARK_FOREST, 0.0),
        Biome::PaleGarden => (0.05, &[(1.0, PALE_OAK)], 0.0),
        Biome::Taiga | Biome::SnowyTaiga => (0.03, TAIGA, 0.0),
        Biome::OldGrowthSpruceTaiga => (0.045, SPRUCE_TAIGA, 0.0),
        Biome::OldGrowthPineTaiga => (0.045, PINE_TAIGA, 0.0),
        Biome::Grove => (0.03, &[(1.0, SPRUCE)], 0.0),
        Biome::SnowyPlains => (0.001, &[(1.0, SPRUCE)], 0.0),
        Biome::WindsweptForest => (0.025, WINDSWEPT, 0.0),
        Biome::WindsweptHills | Biome::WindsweptGravellyHills => (0.003, WINDSWEPT, 0.0),
        Biome::Jungle => (0.06, JUNGLE_MIX, 0.0),
        Biome::BambooJungle => (0.04, JUNGLE_MIX, 0.0),
        Biome::SparseJungle => (0.015, SPARSE_JUNGLE, 0.0),
        Biome::Savanna | Biome::SavannaPlateau | Biome::WindsweptSavanna => (0.004, SAVANNA, 0.0),
        Biome::Plains | Biome::SunflowerPlains => (0.002, PLAINS, 0.05),
        Biome::Meadow => (0.001, MEADOW, 1.0),
        Biome::Swamp => (0.012, &[(1.0, SWAMP_OAK)], 0.0),
        // Гнездо на мангре — у одного дерева из двадцати пяти, как в замере.
        Biome::MangroveSwamp => (0.04, MANGROVES, 0.04),
        Biome::CherryGrove => (0.03, &[(1.0, CHERRY)], 0.0),
        Biome::WoodedBadlands => (0.02, &[(1.0, OAK)], 0.0),
        _ => return None,
    })
}

impl Terrain {
    /// Растёт ли в этом месте дерево.
    ///
    /// Место должно быть подходящим: земля выше уровня моря (у болот —
    /// чуть ниже: их деревья стоят в воде), биом лесной или степной, и рядом
    /// не должно быть другого ствола — иначе деревья срастались бы в
    /// сплошную стену. Соседей спрашиваем тем же способом, каким решаем про
    /// себя: сговариваться чанкам не о чем.
    pub fn tree_at(&self, column: &Column, x: i32, z: i32) -> Option<Tree> {
        let lowest = match column.biome {
            Biome::MangroveSwamp => SEA - 3,
            Biome::Swamp => SEA - 1,
            _ => SEA,
        };

        if column.height < lowest || column.biome.is_ocean() {
            return None;
        }

        let (density, mix, bees) = woodland(column.biome)?;

        if !chance(x, z, 1_301, density) {
            return None;
        }

        // Уступаем соседу, который ближе к началу отсчёта: так из двух
        // стволов рядом остаётся один, и оба чанка решают одинаково.
        // Высоту соседа не спрашиваем нарочно: это стоило бы пересчёта всех
        // шумов на каждое дерево, а на густоту леса почти не влияет.
        for dx in -2..=2 {
            for dz in -2..=2 {
                if (dx, dz) == (0, 0) || (dz, dx) > (0, 0) {
                    continue;
                }

                if chance(x + dx, z + dz, 1_301, density) {
                    return None;
                }
            }
        }

        // Порода и высота ствола свои у каждого дерева.
        let mut random = Random::new(
            (x as i64).wrapping_mul(132_897_987_541).wrapping_add((z as i64).wrapping_mul(341_873_128_712)) + 97,
        );
        let mut pick = fraction(&mut random);
        let species = mix
            .iter()
            .find(|(share, _)| {
                pick -= share;
                pick < 0.0
            })
            .or(mix.last())
            .map(|(_, species)| *species)?;

        let trunk = species.tree.trunk + roll(&mut random, species.spread) + roll(&mut random, species.extra);
        let bees = fraction(&mut random) < bees;

        Some(Tree { trunk, bees, ..species.tree })
    }
}

/// Может ли в этом месте вообще стоять ствол.
///
/// Дешёвая проверка перед дорогой: самая густая чаща у нас реже одного ствола
/// на шестнадцать мест, поэтому почти для всех мест столбец считать незачем.
pub fn tree_possible(x: i32, z: i32) -> bool {
    chance(x, z, 1_301, DENSEST_FOREST)
}

/// Самая большая густота деревьев среди всех биомов.
const DENSEST_FOREST: f64 = 0.06;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Все породы — для проверок и профилей.
    const ALL: [(&str, Species); 21] = [
        ("дуб", OAK),
        ("большой дуб", FANCY_OAK),
        ("болотный дуб", SWAMP_OAK),
        ("берёза", BIRCH),
        ("высокая берёза", TALL_BIRCH),
        ("ель", SPRUCE),
        ("сосна", PINE),
        ("большая ель", MEGA_SPRUCE),
        ("большая сосна", MEGA_PINE),
        ("джунгли", JUNGLE),
        ("большое дерево джунглей", MEGA_JUNGLE),
        ("куст джунглей", JUNGLE_BUSH),
        ("акация", ACACIA),
        ("тёмный дуб", DARK_OAK),
        ("бледный дуб", PALE_OAK),
        ("вишня", CHERRY),
        ("мангр", MANGROVE),
        ("высокий мангр", TALL_MANGROVE),
        ("азалия", AZALEA),
        ("коричневый гриб", BROWN_MUSHROOM),
        ("красный гриб", RED_MUSHROOM),
    ];

    /// Деревья не выходят за полосу, которую чанк просматривает вокруг
    /// себя, — иначе крона обрезалась бы по краю чанка. И не уходят в землю
    /// глубже блока под стволом.
    #[test]
    fn trees_stay_within_reach() {
        for (name, species) in ALL {
            for trunk in species.tree.trunk..(species.tree.trunk + species.spread + species.extra - 1) {
                let tree = Tree { trunk, bees: true, ..species.tree };

                for x in -60..60 {
                    for (dx, dy, dz, _) in tree.blocks(x, x * 3 + 1) {
                        assert!(dx.abs() <= TREE_REACH && dz.abs() <= TREE_REACH, "{} вышло на {}, {}", name, dx, dz);
                        assert!(dy >= 0, "{} ушло в землю на {}", name, dy);
                        // Выше ствола поднимаются ветви вишни и мангра
                        // со своими кронами.
                        assert!(dy <= trunk + 8, "{} выросло на {} при стволе {}", name, dy, trunk);
                    }
                }
            }
        }
    }

    /// Все записи блоков в деревьях есть в таблице: иначе дерево вырастет
    /// с дырами.
    #[test]
    fn tree_blocks_are_known() {
        for (name, species) in ALL {
            let tree = Tree { bees: true, ..species.tree };

            assert!(crate::blocks::state_from_text(tree.leaves).is_some(), "{}: {}", name, tree.leaves);
            assert!(crate::blocks::state_from_text(tree.log).is_some(), "{}: {}", name, tree.log);

            for x in -30..30 {
                for (_, _, _, part) in tree.blocks(x, 7 - x) {
                    if let Part::Block(text) | Part::Ground(text) = part {
                        assert!(crate::blocks::state_from_text(text).is_some(), "{}: {}", name, text);
                    }
                }
            }
        }
    }

    /// Состав леса складывается в целое: иначе часть мест, где решено
    /// стоять дереву, оставалась бы пустой или доставалась последней породе.
    #[test]
    fn woodland_shares_add_up() {
        for biome in Biome::ALL {
            if let Some((density, mix, _)) = woodland(biome) {
                let total: f64 = mix.iter().map(|(share, _)| share).sum();

                assert!((total - 1.0).abs() < 1e-9, "{:?}: сумма долей {}", biome, total);
                assert!(density <= DENSEST_FOREST, "{:?} гуще самой густой чащи", biome);
            }
        }
    }

    /// Под деревом на траве трава становится землёй — у всех, кроме
    /// мангра (он стоит на корнях) и азалии (под ней корнистая земля).
    #[test]
    fn dirt_under_every_trunk() {
        for (name, species) in ALL {
            let tree = Tree { trunk: species.tree.trunk + 1, ..species.tree };
            let blocks = tree.blocks(3, 4);
            let under = |text: &'static str| blocks.contains(&(0, 0, 0, Part::Ground(text)));

            match tree.shape {
                Shape::Mangrove | Shape::TallMangrove => assert!(under("muddy_mangrove_roots"), "{}", name),
                Shape::Azalea => assert!(under("rooted_dirt"), "{}", name),
                // У 2×2 тёмных дубов ствол уходит в сам верхний блок земли.
                Shape::DarkOak | Shape::PaleOak => {}
                _ => assert!(under("dirt"), "{}", name),
            }
        }
    }

    /// Что сверяем с замером оригинала (tools/research/trees-measured.md).
    /// Слои считаются, как в замере, от верхнего бревна прямого ствола;
    /// радиус слоя — насколько далеко от оси ствола в нём листва.
    #[derive(Clone, Copy)]
    enum Check {
        /// Средняя высота прямого ствола.
        Trunk(f64),
        /// Средний радиус слоя у деревьев, где этот слой есть.
        Layer(i32, f64),
        /// Самый широкий слой среди всех деревьев.
        Widest(f64),
        /// Самый верхний и самый нижний слой листвы среди всех деревьев —
        /// с точностью до блока.
        Highest(i32),
        Lowest(i32),
        /// Сколько в среднем блоков этого рода у дерева, где они есть.
        Count(&'static str, f64),
        /// Доля деревьев, у которых они есть, — с точностью до десятой.
        Share(&'static str, f64),
    }

    use Check::*;

    /// Числа замера по видам. Где в замере диапазон («радиус 3.5–5.5»),
    /// сверяем его верхний край с самым широким слоем.
    ///
    /// 6.28 — средний ствол ели из замера, а не 2π.
    #[allow(clippy::approx_constant)]
    fn measured(shape: Shape) -> &'static [Check] {
        match shape {
            Shape::Oak => &[Trunk(5.52), Layer(1, 1.0), Layer(0, 1.0), Layer(-1, 2.0), Layer(-3, 2.0), Highest(1), Share("dirt", 1.0)],
            Shape::Fancy => &[Trunk(8.2), Widest(7.0), Highest(4), Lowest(-9), Share("branch", 0.84), Share("dirt", 1.0)],
            Shape::Round => &[Layer(1, 1.0), Layer(0, 1.0), Layer(-1, 2.0), Layer(-2, 2.0), Highest(1), Share("dirt", 1.0)],
            Shape::Spruce => &[Trunk(6.28), Widest(3.0), Highest(3), Lowest(-6), Share("branch", 0.0), Share("dirt", 1.0)],
            Shape::Pine => &[Trunk(7.88), Layer(2, 0.0), Layer(1, 1.0), Highest(2), Share("dirt", 1.0)],
            Shape::MegaSpruce => &[
                Trunk(20.64),
                Layer(1, 0.5),
                Widest(4.5),
                Highest(1),
                Lowest(-16),
                Count("podzol", 120.0),
                Share("podzol", 1.0),
            ],
            Shape::MegaPine => &[
                Trunk(21.16),
                Layer(1, 0.5),
                Layer(-1, 1.5),
                Widest(3.5),
                Highest(1),
                Count("podzol", 117.7),
                Share("podzol", 1.0),
            ],
            Shape::Jungle => &[
                Trunk(8.64),
                Layer(1, 1.0),
                Layer(0, 1.0),
                Layer(-1, 2.0),
                Layer(-2, 2.0),
                Count("vine", 60.0),
                Share("vine", 1.0),
                Share("cocoa", 0.08),
            ],
            Shape::MegaJungle => &[Trunk(21.2), Widest(7.5), Count("vine", 235.0), Share("vine", 1.0), Share("branch", 1.0)],
            Shape::Bush => &[Trunk(1.0), Layer(0, 2.0), Layer(1, 1.0), Highest(2), Share("vine", 0.0), Share("dirt", 1.0)],
            Shape::Acacia => &[Trunk(4.36), Widest(7.0), Count("stem", 4.44), Share("stem", 1.0)],
            Shape::DarkOak => &[Trunk(7.32), Widest(5.5), Count("stem", 13.1), Share("stem", 1.0)],
            Shape::PaleOak => &[
                Trunk(7.28),
                Widest(5.5),
                Count("stem", 15.8),
                Count("pale_hanging_moss", 19.0),
                Share("pale_hanging_moss", 1.0),
                Count("pale_moss_block", 53.0),
                Share("pale_moss_block", 0.8),
                Count("pale_moss_carpet", 7.0),
                Share("pale_moss_carpet", 0.8),
            ],
            Shape::Cherry => &[Trunk(5.0), Widest(8.0), Share("branch", 1.0)],
            Shape::Mangrove => &[
                Trunk(4.48),
                Count("roots", 13.6),
                Count("vine", 54.4),
                Share("vine", 1.0),
                Count("moss_carpet", 5.0),
                Share("moss_carpet", 1.0),
                Share("branch", 0.68),
            ],
            Shape::TallMangrove => &[
                Trunk(10.8),
                Widest(7.0),
                Count("roots", 33.9),
                Count("vine", 162.8),
                Share("vine", 1.0),
                Count("moss_carpet", 7.2),
                Share("moss_carpet", 1.0),
                Share("branch", 0.96),
            ],
            Shape::Azalea => &[Trunk(3.88), Layer(0, 2.0), Widest(6.0), Share("branch", 1.0), Share("rooted_dirt", 1.0)],
            Shape::RedMushroom => &[Trunk(5.0), Layer(1, 1.0), Layer(0, 2.0), Layer(-1, 2.0), Layer(-2, 2.0), Highest(1)],
            Shape::BrownMushroom => &[Trunk(5.2), Layer(1, 3.0), Highest(1), Lowest(1)],
            Shape::Swamp => &[],
        }
    }

    /// Высокая берёза отличается от обычной только стволом.
    fn measured_species(name: &str, shape: Shape) -> Vec<Check> {
        let mut checks = measured(shape).to_vec();

        match name {
            "берёза" => checks.push(Trunk(6.0)),
            "высокая берёза" => checks.push(Trunk(8.36)),
            _ => {}
        }

        checks
    }

    /// Дерево, выросшее на ровной травяной площадке: что где встало, в том
    /// же порядке, что и в мире, — земля, листва в воздух, брёвна поверх,
    /// прочее в оставшийся воздух.
    fn settle(tree: Tree, x: i32, z: i32) -> HashMap<(i32, i32, i32), Part> {
        let blocks = tree.blocks(x, z);
        let mut world = HashMap::new();

        for &(dx, _, dz, part) in &blocks {
            if let Part::Ground(_) = part {
                world.insert((dx, 0, dz), part);
            }
        }

        for &(dx, dy, dz, part) in &blocks {
            if part == Part::Leaves && dy >= 1 {
                world.entry((dx, dy, dz)).or_insert(part);
            }
        }

        for &(dx, dy, dz, part) in &blocks {
            if let Part::Log(_) = part {
                world.insert((dx, dy, dz), part);
            }
        }

        for &(dx, dy, dz, part) in &blocks {
            if let Part::Block(_) = part
                && dy >= 1
            {
                world.entry((dx, dy, dz)).or_insert(part);
            }
        }

        world
    }

    /// Род блока для подсчёта.
    fn kind(part: Part) -> Option<&'static str> {
        let text = match part {
            Part::Block(text) | Part::Ground(text) => text,
            _ => return None,
        };
        let name = text.split('[').next().unwrap_or(text);

        Some(match name {
            "mangrove_roots" | "muddy_mangrove_roots" => "roots",
            "flowering_azalea_leaves" => return None,
            other => other,
        })
    }

    /// Замер одного вида на двухстах деревьях.
    #[derive(Default)]
    struct Survey {
        trees: f64,
        trunk: f64,
        layers: HashMap<i32, (f64, f64)>,
        widest: f64,
        highest: i32,
        lowest: i32,
        counts: HashMap<&'static str, (f64, f64)>,
    }

    fn survey(species: Species) -> Survey {
        let mut result = Survey { highest: i32::MIN, lowest: i32::MAX, ..Survey::default() };
        let wide = tree_is_wide(species.tree);
        let middle = if wide { 0.5 } else { 0.0 };

        for index in 0..200 {
            let (x, z) = (index * 37 - 3_000, index * 91 + 17);
            let mut random = Random::new(index as i64 * 7 + 1);
            let trunk = species.tree.trunk + roll(&mut random, species.spread) + roll(&mut random, species.extra);
            let tree = Tree { trunk, bees: false, ..species.tree };
            let world = settle(tree, x, z);
            let is_log = |y: i32| matches!(world.get(&(0, y, 0)), Some(Part::Log(Axis::Y)));

            // Прямой ствол — сплошной столб брёвен над точкой посадки.
            let bottom = (1..=64).find(|&y| is_log(y)).unwrap_or(1);
            let height = (bottom..).take_while(|&y| is_log(y)).count() as i32;
            let top = bottom + height - 1;
            let main = |x: i32, y: i32, z: i32| {
                (bottom..=top).contains(&y) && ((x, z) == (0, 0) || (wide && (0..=1).contains(&x) && (0..=1).contains(&z)))
            };

            result.trees += 1.0;
            result.trunk += f64::from(height);

            let mut layers: HashMap<i32, f64> = HashMap::new();
            let mut counts: HashMap<&'static str, f64> = HashMap::new();

            for (&(x, y, z), &part) in &world {
                let leafy = part == Part::Leaves
                    || part == Part::Block("flowering_azalea_leaves")
                    || matches!(part, Part::Block(text) if text.starts_with("red_mushroom_block"));

                if leafy {
                    let radius = (f64::from(x) - middle).abs().max((f64::from(z) - middle).abs());
                    let layer = layers.entry(y - top).or_insert(0.0);

                    *layer = layer.max(radius);
                }

                if let Part::Log(axis) = part
                    && !main(x, y, z)
                {
                    *counts.entry("branch").or_default() += 1.0;

                    if axis == Axis::Y {
                        *counts.entry("stem").or_default() += 1.0;
                    }
                }

                if let Some(kind) = kind(part) {
                    *counts.entry(kind).or_default() += 1.0;
                }
            }

            for (layer, radius) in layers {
                let entry = result.layers.entry(layer).or_default();

                entry.0 += radius;
                entry.1 += 1.0;
                result.widest = result.widest.max(radius);
                result.highest = result.highest.max(layer);
                result.lowest = result.lowest.min(layer);
            }

            for (kind, count) in counts {
                let entry = result.counts.entry(kind).or_default();

                entry.0 += count;
                entry.1 += 1.0;
            }
        }

        result
    }

    /// Что получилось у нас по одной проверке.
    fn ours(survey: &Survey, check: Check) -> f64 {
        let count = |kind: &str| survey.counts.get(kind).copied().unwrap_or_default();

        match check {
            Trunk(_) => survey.trunk / survey.trees,
            Layer(layer, _) => survey.layers.get(&layer).map_or(-1.0, |(sum, trees)| sum / trees),
            Widest(_) => survey.widest,
            Highest(_) => f64::from(survey.highest),
            Lowest(_) => f64::from(survey.lowest),
            Count(kind, _) => {
                let (sum, trees) = count(kind);

                if trees > 0.0 { sum / trees } else { 0.0 }
            }
            Share(kind, _) => count(kind).1 / survey.trees,
        }
    }

    /// Сходится ли наше число с замером.
    fn agrees(check: Check, value: f64) -> bool {
        match check {
            Trunk(want) | Layer(_, want) | Widest(want) | Count(_, want) => (value - want).abs() <= want.abs() * 0.15 + 1e-9,
            Highest(want) | Lowest(want) => (value - f64::from(want)).abs() <= 1.0,
            Share(_, want) => (value - want).abs() <= 0.1 + 1e-9,
        }
    }

    /// Формы деревьев сходятся с замером оригинала: на двухстах деревьях
    /// каждого вида средняя высота ствола, радиусы слоёв кроны, число
    /// лиан, корней, подзола и мха — в пределах 15% от замеренных,
    /// крайние слои — до блока, доли деревьев — до десятой.
    #[test]
    fn trees_match_measurements() {
        let mut wrong = Vec::new();

        for (name, species) in ALL {
            let survey = survey(species);

            for check in measured_species(name, species.tree.shape) {
                let value = ours(&survey, check);

                if !agrees(check, value) {
                    let want = match check {
                        Trunk(want) | Layer(_, want) | Widest(want) | Count(_, want) | Share(_, want) => want,
                        Highest(want) | Lowest(want) => f64::from(want),
                    };
                    let what = match check {
                        Trunk(_) => "ствол".to_string(),
                        Layer(layer, _) => format!("слой {}", layer),
                        Widest(_) => "самый широкий слой".to_string(),
                        Highest(_) => "верхний слой".to_string(),
                        Lowest(_) => "нижний слой".to_string(),
                        Count(kind, _) => format!("{}, блоков", kind),
                        Share(kind, _) => format!("{}, доля", kind),
                    };

                    wrong.push(format!("{}: {} — у нас {:.2}, в замере {:.2}", name, what, value, want));
                }
            }
        }

        assert!(wrong.is_empty(), "расходится с замером:\n{}", wrong.join("\n"));
    }

    /// Сводка замера по всем видам — посмотреть глазами:
    /// `cargo test tree_survey -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn tree_survey() {
        for (name, species) in ALL {
            let survey = survey(species);
            let mut layers: Vec<_> = survey.layers.iter().map(|(layer, (sum, n))| (*layer, sum / n)).collect();
            let mut counts: Vec<_> =
                survey.counts.iter().map(|(kind, (sum, n))| format!("{} {:.1} ({:.0}%)", kind, sum / n, n / survey.trees * 100.0)).collect();

            layers.sort_by_key(|(layer, _)| -layer);
            counts.sort();

            println!("{}: ствол {:.2}, самый широкий {:.1}", name, survey.trunk / survey.trees, survey.widest);
            println!(
                "  слои: {}",
                layers.iter().map(|(layer, radius)| format!("{:+}:{:.2}", layer, radius)).collect::<Vec<_>>().join(" ")
            );
            println!("  блоки: {}", counts.join(", "));
        }
    }

    /// Профиль деревьев сбоку — посмотреть глазами:
    /// `cargo test tree_profiles -- --ignored --nocapture`.
    ///
    /// `#` — ствол, `=` — ветви, `o` — листва, `~` — прочие блоки (лианы,
    /// какао, корни, мох), `_` — замена земли.
    #[test]
    #[ignore]
    fn tree_profiles() {
        for (name, species) in ALL {
            let trunk = species.tree.trunk + (species.spread + species.extra - 2) / 2;
            let tree = Tree { trunk, bees: true, ..species.tree };
            let blocks = tree.blocks(5, 9);

            println!("{}, ствол {}:", name, trunk);

            for y in (0..=trunk + 8).rev() {
                let line: String = (-TREE_REACH..=TREE_REACH)
                    .map(|x| {
                        let here = || blocks.iter().filter(|b| b.0 == x && b.1 == y);

                        if here().any(|b| (b.2 == 0 || (b.2 == 1 && tree_is_wide(tree))) && matches!(b.3, Part::Log(_))) {
                            '#'
                        } else if here().any(|b| matches!(b.3, Part::Log(_))) {
                            '='
                        } else if here().any(|b| b.3 == Part::Leaves) {
                            'o'
                        } else if here().any(|b| matches!(b.3, Part::Block(_))) {
                            '~'
                        } else if here().any(|b| matches!(b.3, Part::Ground(_))) {
                            '_'
                        } else {
                            '.'
                        }
                    })
                    .collect();

                println!("  {:>2} {}", y, line);
            }
        }
    }

    /// Ствол 2×2.
    fn tree_is_wide(tree: Tree) -> bool {
        matches!(tree.shape, Shape::MegaSpruce | Shape::MegaPine | Shape::MegaJungle | Shape::DarkOak | Shape::PaleOak)
    }
}
