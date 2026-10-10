// Земледелие и рост растений: случайный такт, грядки, посевы, саженцы,
// тростник, кактус, ягоды, грибы, трава, листва, мотыга и костная мука.
//
// Правила — по вики (Farmland, Hoe, Tutorial:Crop farming, Wheat Seeds,
// Carrot, Potato, Beetroot Seeds, Melon Seeds, Pumpkin Seeds, Sapling, Sugar
// Cane, Cactus, Kelp, Sweet Berry Bush, Nether Wart, Cocoa Beans, Vines,
// Brown Mushroom, Grass Block, Mycelium, Leaves, Bone Meal, Game rule, Tick);
// пакеты и порядок — по чёрному ящику (сценарии farm-*).
//
// Случайный такт («random tick»): каждый такт в каждой секции 16×16×16
// чанков у игроков выбирается `random_tick_speed` случайных мест (по
// умолчанию 3, вики, Game rule: «per chunk section per game tick»), и блок
// в выбранном месте делает свой ход: растёт, сохнет, гниёт. Горящая руда
// гаснет своим путём (redstone/components.rs), но с той же скоростью.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::blocks;
use crate::fluids;
use crate::inventory::Stack;
use crate::world::trees::{self, Part};
use crate::world::{World, AIR};

/// Правило `random_tick_speed` (26.1; прежнее randomTickSpeed): сколько
/// мест на секцию за такт. Правило одно на все измерения, как у оригинала.
static RANDOM_TICK_SPEED: AtomicU32 = AtomicU32::new(3);

/// Нижняя граница мира и число секций по высоте.
const MIN_Y: i32 = -64;
const SECTIONS: usize = 24;

/// Скорость случайного такта.
pub fn random_tick_speed() -> u32 {
    RANDOM_TICK_SPEED.load(Ordering::Relaxed)
}

/// Команда `gamerule random_tick_speed <n>`: отрицательное выключает такты
/// так же, как 0 (вики, Game rule: «0 and negative values disable»).
pub fn set_random_tick_speed(value: i32) {
    RANDOM_TICK_SPEED.store(value.max(0) as u32, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Свойства блоков
// ---------------------------------------------------------------------------

fn name_of(state: i32) -> &'static str {
    blocks::block_at_state(state).unwrap_or("air")
}

fn value(state: i32, property: &str) -> Option<&'static str> {
    blocks::orientation_of(state).and_then(|block| block.value(state, property))
}

fn number(state: i32, property: &str) -> i32 {
    value(state, property).and_then(|text| text.parse().ok()).unwrap_or(0)
}

fn with(state: i32, property: &str, to: impl ToString) -> Option<i32> {
    blocks::orientation_of(state).and_then(|block| block.with(state, property, &to.to_string()))
}

fn state_of(name: &str) -> i32 {
    blocks::state_by_name(name).unwrap_or(AIR)
}

/// Случайное от 0 до n−1.
fn roll(world: &mut World, n: i32) -> i32 {
    (world.random_bits() % n.max(1) as u32) as i32
}

/// Случайное событие с вероятностью `p`.
fn chance(world: &mut World, p: f32) -> bool {
    (world.random_bits() % 1_000_000) as f32 / 1_000_000.0 < p
}

/// Посев на грядке: имя и наибольший возраст.
fn crop_max_age(name: &str) -> Option<i32> {
    Some(match name {
        "wheat" | "carrots" | "potatoes" | "melon_stem" | "pumpkin_stem" => 7,
        "beetroots" => 3,
        "torchflower_crop" => 1,
        _ => return None,
    })
}

fn is_sapling(name: &str) -> bool {
    name.ends_with("_sapling") && name != "bamboo_sapling" || name == "mangrove_propagule"
}

fn is_leaves(name: &str) -> bool {
    name.ends_with("_leaves")
}

/// Ходит ли блок этого состояния случайным тактом. Таблица на все
/// состояния — выбор места идёт тысячами за такт, а имя ищется долго.
fn ticks_randomly(state: u16) -> bool {
    static TABLE: OnceLock<Vec<bool>> = OnceLock::new();

    TABLE.get_or_init(|| {
        (0..=u16::MAX as i32)
            .map(|state| {
                let Some(name) = blocks::block_at_state(state) else {
                    return false;
                };

                crop_max_age(name).is_some()
                    || is_sapling(name)
                    || (is_leaves(name) && value(state, "persistent") == Some("false"))
                    || matches!(
                        name,
                        "turtle_egg"
                            | "farmland"
                            | "sugar_cane"
                            | "cactus"
                            | "kelp"
                            | "sweet_berry_bush"
                            | "nether_wart"
                            | "cocoa"
                            | "vine"
                            | "brown_mushroom"
                            | "red_mushroom"
                            | "grass_block"
                            | "mycelium"
                            | "bamboo_sapling"
                    )
            })
            .collect()
    })[state as usize]
}

// ---------------------------------------------------------------------------
// Случайный такт
// ---------------------------------------------------------------------------

/// Случайные такты мира: в каждой секции чанков, где стоят игроки (в
/// пределах дальности симуляции `distance`), — `speed` случайных мест.
/// Пустые секции (одного воздуха) пропускаются, как у оригинала.
/// `players` — чанки игроков этого мира.
pub fn random_ticks(world: &mut World, players: &[(i32, i32)], distance: i32, speed: u32) {
    if speed == 0 || players.is_empty() {
        return;
    }

    let mut chunks: Vec<(i32, i32)> = Vec::new();

    for (px, pz) in players {
        for cx in px - distance..=px + distance {
            for cz in pz - distance..=pz + distance {
                chunks.push((cx, cz));
            }
        }
    }

    chunks.sort_unstable();
    chunks.dedup();

    let mut due: Vec<((i32, i32, i32), i32)> = Vec::new();

    for (cx, cz) in chunks {
        let Some(mask) = tickable_sections(world, cx, cz) else {
            continue;
        };

        for section in 0..SECTIONS {
            // Секции без блоков со случайным тактом пропускаются: выбранное
            // в них место ничего бы не сделало. Без этого скорость 4096
            // стоила бы миллионов выборов за такт.
            if mask & (1 << section) == 0 {
                continue;
            }

            for _ in 0..speed {
                let index = (world.random_bits() % 4096) as usize;
                let Some(blocks) = world.section_blocks(cx, cz, section) else {
                    break;
                };
                let state = blocks[index];

                if ticks_randomly(state) {
                    let x = cx * 16 + (index % 16) as i32;
                    let z = cz * 16 + (index / 16 % 16) as i32;
                    let y = MIN_Y + section as i32 * 16 + (index / 256) as i32;

                    due.push(((x, y, z), i32::from(state)));
                }
            }
        }
    }

    for (pos, state) in due {
        // Место могло смениться ходом соседа в этом же такте.
        let now = world.get_block(pos.0, pos.1, pos.2);
        if now == state {
            random_tick(world, pos, state);
        } else if name_of(now) == "turtle_egg" && name_of(state) == "turtle_egg" {
            // Несколько случайных тактов на одно яйцо за такт мира: каждый видит
            // уже треснувшее (чёрный ящик turtle-egg-hatch: трещина и вылупление в одном такте).
            random_tick(world, pos, now);
        }
    }
}

/// Секции чанка, где есть блоки со случайным тактом (бит на секцию). Ищется
/// заново, только когда чанк менялся. None — чанка нет в памяти.
fn tickable_sections(world: &mut World, cx: i32, cz: i32) -> Option<u32> {
    let Some(revision) = world.chunk_revision(cx, cz) else {
        world.random_sections.remove(&(cx, cz));
        return None;
    };

    if let Some((seen, mask)) = world.random_sections.get(&(cx, cz))
        && *seen == revision
    {
        return Some(*mask);
    }

    let mask = (0..SECTIONS)
        .filter(|&section| world.section_blocks(cx, cz, section).is_some_and(|blocks| blocks.iter().any(|state| ticks_randomly(*state))))
        .fold(0, |mask, section| mask | 1 << section);

    world.random_sections.insert((cx, cz), (revision, mask));
    Some(mask)
}

/// Ход одного блока по случайному такту.
pub fn random_tick(world: &mut World, pos: (i32, i32, i32), state: i32) {
    let name = name_of(state);

    match name {
        "turtle_egg" => crate::mob::water::eggs::turtle_tick(world,pos,state),
        "farmland" => farmland_tick(world, pos, state),
        "sugar_cane" | "cactus" => column_tick(world, pos, state, name),
        "kelp" => kelp_tick(world, pos, state),
        "sweet_berry_bush" => berry_tick(world, pos, state),
        "nether_wart" => {
            // Вики, Nether Wart: «10% chance» за случайный такт.
            if number(state, "age") < 3 && chance(world, 0.1) {
                grow_to(world, pos, with(state, "age", number(state, "age") + 1));
            }
        }
        "cocoa" => {
            // Вики, Cocoa Beans: 20 % за такт.
            if number(state, "age") < 2 && roll(world, 5) == 0 {
                grow_to(world, pos, with(state, "age", number(state, "age") + 1));
            }
        }
        "brown_mushroom" | "red_mushroom" => mushroom_tick(world, pos, state),
        "grass_block" | "mycelium" => spread_tick(world, pos, state, name),
        "vine" => {}
        "bamboo_sapling" => {
            // Вики, Bamboo: «1/3» за такт, при свете у верхушки 9 и выше.
            if world.get_block(pos.0, pos.1 + 1, pos.2) == AIR && light(world, pos.0, pos.1 + 1, pos.2) >= 9 && roll(world, 3) == 0 {
                grow_bamboo_shoot(world, pos);
            }
        }
        name if is_leaves(name) => leaves_tick(world, pos, state),
        name if is_sapling(name) => {
            // Вики, Sapling: свет в блоке над саженцем 9 и выше; шанс 1/7
            // за такт [проверить].
            if light(world, pos.0, pos.1 + 1, pos.2) >= 9 && roll(world, 7) == 0 {
                advance_sapling(world, pos, state);
            }
        }
        name if crop_max_age(name).is_some() => crop_tick(world, pos, state, name),
        _ => {}
    }
}

/// Свет в клетке: наибольший из небесного и блочного, без поправки на
/// время суток (вики, Crops: «light level of 9 or higher at the plant
/// block»). Под открытым небом — 15 без подсчёта.
fn light(world: &mut World, x: i32, y: i32, z: i32) -> u8 {
    if world.open_sky(x, y, z) {
        return 15;
    }

    let (sky, block) = world.light_full(x, y, z);

    sky.max(block)
}

/// Ставит выросшее состояние (None — свойства нет, ничего).
fn grow_to(world: &mut World, (x, y, z): (i32, i32, i32), state: Option<i32>) {
    if let Some(state) = state {
        world.set_block(x, y, z, state);
    }
}

// ---------------------------------------------------------------------------
// Грядка
// ---------------------------------------------------------------------------

/// Вода в пределах 4 блоков по горизонтали на уровне грядки и на блок выше
/// (вики, Farmland: «up to four blocks away horizontally (including
/// diagonally)… at the same level or one block above»). Затопленные блоки
/// тоже считаются водой [проверить].
fn water_near(world: &World, (x, y, z): (i32, i32, i32)) -> bool {
    for dy in 0..=1 {
        for dx in -4..=4 {
            for dz in -4..=4 {
                let state = world.get_block(x + dx, y + dy, z + dz);

                if matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, _))) || value(state, "waterlogged") == Some("true") {
                    return true;
                }
            }
        }
    }

    false
}

/// Держит ли грядка посев над собой: тогда сухая не становится землёй.
fn crop_on(world: &World, (x, y, z): (i32, i32, i32)) -> bool {
    let above = name_of(world.get_block(x, y + 1, z));

    crop_max_age(above).is_some() || matches!(above, "attached_melon_stem" | "attached_pumpkin_stem" | "pitcher_crop")
}

/// Случайный такт грядки: вода рядом или дождь — влажность 7; иначе
/// влажность убывает на 1, а совсем сухая без посева становится землёй
/// (вики, Farmland: «counts down to 0… jumps from 0 to 7», «decays into
/// normal dirt if it's dehydrated and nothing is planted in it»).
fn farmland_tick(world: &mut World, pos: (i32, i32, i32), state: i32) {
    let moisture = number(state, "moisture");

    if water_near(world, pos) || world.rain_at(pos.0, pos.1 + 1, pos.2) {
        if moisture < 7 {
            grow_to(world, pos, with(state, "moisture", 7));
        }
    } else if moisture > 0 {
        grow_to(world, pos, with(state, "moisture", moisture - 1));
    } else if !crop_on(world, pos) {
        to_dirt(world, pos);
    }
}

/// Грядка становится землёй; посев над ней выпадает соседским обновлением
/// (plants.rs: посеву нужна грядка). [проверить] сдвиг стоящих на ней
/// сущностей вверх.
pub fn to_dirt(world: &mut World, (x, y, z): (i32, i32, i32)) {
    world.set_block(x, y, z, state_of("dirt"));
}

/// Сосед грядки изменился: твёрдый блок над ней превращает её в землю
/// (вики, Farmland: «a solid block covers the top surface»). Возвращает
/// true, если грядки больше нет.
pub fn neighbour_changed(world: &mut World, pos: (i32, i32, i32)) -> bool {
    let state = world.get_block(pos.0, pos.1, pos.2);

    if name_of(state) != "farmland" {
        return false;
    }

    let above = world.get_block(pos.0, pos.1 + 1, pos.2);

    if solid_cover(above) {
        to_dirt(world, pos);
        return true;
    }

    false
}

/// Твёрдый ли блок над грядкой: с полной нижней гранью и не жидкость.
/// Калитка, поршень и прочее с неполным ящиком грядку не губят
/// [проверить].
fn solid_cover(state: i32) -> bool {
    state != AIR && fluids::fluid_at(state).is_none() && blocks::full_cube(state)
}

/// Приземление на грядку: вытаптывается с вероятностью «высота падения −
/// 0,5» (вики, Farmland: «distance fallen − 0.5»). Игрок вытаптывает всегда
/// (объём 0,6·0,6·1,8 больше 0,512), мобам нужно ещё `mob_griefing`.
/// Возвращает true, если грядка стала землёй.
pub fn trample(world: &mut World, pos: (i32, i32, i32), fall: f32) -> bool {
    if fall <= 0.5 || name_of(world.get_block(pos.0, pos.1, pos.2)) != "farmland" {
        return false;
    }

    if world.random_unit() < fall - 0.5 {
        to_dirt(world, pos);
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Посевы и стебли
// ---------------------------------------------------------------------------

/// Скорость роста посева — «очки» (вики, Tutorial:Crop farming): грядка
/// под ним даёт 2 (сухая) или 4 (влажная), каждая из восьми соседних —
/// 0,25 или 0,75; такой же посев по диагонали или с обеих осей сразу
/// делит очки пополам.
fn growth_points(world: &World, (x, y, z): (i32, i32, i32), name: &str) -> f32 {
    let mut points = 1.0;

    for dx in -1..=1 {
        for dz in -1..=1 {
            let below = world.get_block(x + dx, y - 1, z + dz);
            let mut add = 0.0;

            if name_of(below) == "farmland" {
                add = if number(below, "moisture") > 0 { 3.0 } else { 1.0 };
            }

            if dx != 0 || dz != 0 {
                add /= 4.0;
            }

            points += add;
        }
    }

    let same = |dx: i32, dz: i32| name_of(world.get_block(x + dx, y, z + dz)) == name;
    let north_south = same(0, -1) || same(0, 1);
    let west_east = same(-1, 0) || same(1, 0);
    let diagonal = same(-1, -1) || same(1, -1) || same(1, 1) || same(-1, 1);

    if (north_south && west_east) || diagonal {
        points /= 2.0;
    }

    points
}

/// Случайный такт посева: при свете 9 и выше растёт с шансом
/// 1/(⌊25/очки⌋ + 1). Свёкла в трети тактов не растёт вовсе (вики,
/// Beetroot Seeds: «1/3 chance of not advancing»). Спелый стебель
/// пробует поставить плод рядом.
fn crop_tick(world: &mut World, pos: (i32, i32, i32), state: i32, name: &str) {
    if light(world, pos.0, pos.1, pos.2) < 9 {
        return;
    }

    if name == "beetroots" && roll(world, 3) == 0 {
        return;
    }

    let age = number(state, "age");
    let max = crop_max_age(name).unwrap_or(7);
    let points = growth_points(world, pos, name);

    if roll(world, (25.0 / points) as i32 + 1) != 0 {
        return;
    }

    if age < max {
        grow_to(world, pos, with(state, "age", age + 1));
    } else if name == "melon_stem" || name == "pumpkin_stem" {
        place_fruit(world, pos, name);
    } else if name == "torchflower_crop" {
        world.set_block(pos.0, pos.1, pos.2, state_of("torchflower"));
    }
}

/// Плод у спелого стебля: случайная сторона; место пусто, а под ним
/// земля, дёрн, грядка, мох или грязь (вики, Melon Seeds). Стебель
/// становится прикреплённым и смотрит на плод.
fn place_fruit(world: &mut World, (x, y, z): (i32, i32, i32), stem: &str) {
    let (fruit, attached) = if stem == "melon_stem" { ("melon", "attached_melon_stem") } else { ("pumpkin", "attached_pumpkin_stem") };
    let (dx, dz, facing) = [(0, -1, "north"), (0, 1, "south"), (-1, 0, "west"), (1, 0, "east")][roll(world, 4) as usize];
    let (fx, fz) = (x + dx, z + dz);

    let below = name_of(world.get_block(fx, y - 1, fz));
    let ground = matches!(
        below,
        "dirt" | "coarse_dirt" | "rooted_dirt" | "grass_block" | "farmland" | "podzol" | "mycelium" | "moss_block" | "pale_moss_block" | "mud" | "muddy_mangrove_roots"
    );

    if world.get_block(fx, y, fz) != AIR || !ground {
        return;
    }

    world.set_block(fx, y, fz, state_of(fruit));

    if let Some(state) = blocks::orientation(attached).and_then(|block| block.state(&[("facing", facing)])) {
        world.set_block(x, y, z, state);
    }
}

// ---------------------------------------------------------------------------
// Тростник, кактус, ламинария, ягоды, грибы
// ---------------------------------------------------------------------------

/// Тростник и кактус: верхний блок столба копит возраст до 15 и тогда
/// ставит над собой новый блок, пока столб ниже трёх (вики, Sugar Cane,
/// Cactus). Возраст клиенту не сообщается [проверить чёрным ящиком].
fn column_tick(world: &mut World, (x, y, z): (i32, i32, i32), state: i32, name: &str) {
    if world.get_block(x, y + 1, z) != AIR {
        return;
    }

    let mut height = 1;

    while height < 3 && name_of(world.get_block(x, y - height, z)) == name {
        height += 1;
    }

    let age = number(state, "age");

    // Цветок кактуса: на девятом такте блока (возраст 8 → 9) с шансом 10 %
    // у столба в один-два блока и 25 % у столба от трёх, если клетки вокруг
    // места цветка пусты (вики, Cactus: «after a cactus block has received
    // exactly 9 random ticks»). Кактус с цветком уже не растёт — над ним
    // не воздух.
    if name == "cactus" && age == 8 {
        let tall = height >= 3 || name_of(world.get_block(x, y - 3, z)) == name;
        let p = if tall { 0.25 } else { 0.1 };
        let free = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dz)| world.get_block(x + dx, y + 1, z + dz) == AIR);

        if free && chance(world, p) {
            world.set_block(x, y + 1, z, state_of("cactus_flower"));
            return;
        }
    }

    if height >= 3 {
        if name == "cactus"
            && age < 15
            && let Some(older) = with(state, "age", age + 1)
        {
            world.set_block_quietly(x, y, z, older);
        }

        return;
    }

    if age < 15 {
        if let Some(older) = with(state, "age", age + 1) {
            world.set_block_quietly(x, y, z, older);
        }

        return;
    }

    let young = state_of(name);

    // Кактус не растёт в место, где ему не выжить (сосед сбоку).
    if !crate::plants::can_place(world, young, x, y + 1, z) {
        return;
    }

    world.set_block(x, y + 1, z, young);

    if let Some(fresh) = with(state, "age", 0) {
        world.set_block_quietly(x, y, z, fresh);
    }
}

/// Ламинария: верхушка с возрастом меньше 25 растёт вверх в воду с шансом
/// 14 % (вики, Kelp: «14% chance»), новая верхушка на год старше.
fn kelp_tick(world: &mut World, (x, y, z): (i32, i32, i32), state: i32) {
    let age = number(state, "age");
    let above = world.get_block(x, y + 1, z);

    if age >= 25 || !matches!(fluids::fluid_at(above), Some((fluids::Kind::Water, fluids::SOURCE))) || !chance(world, 0.14) {
        return;
    }

    if let Some(top) = with(state, "age", age + 1) {
        world.set_block(x, y + 1, z, top);
    }

    world.set_block(x, y, z, state_of("kelp_plant"));
}

/// Куст сладких ягод: при свете над ним 9 и выше растёт с шансом 20 %
/// (вики, Sweet Berry Bush) [проверить: порог света].
fn berry_tick(world: &mut World, pos: (i32, i32, i32), state: i32) {
    let age = number(state, "age");

    if age < 3 && light(world, pos.0, pos.1 + 1, pos.2) >= 9 && roll(world, 5) == 0 {
        grow_to(world, pos, with(state, "age", age + 1));
    }
}

/// Гриб расползается: с шансом 1/25, если в 9×3×9 вокруг меньше пяти таких
/// же; новое место — случайное рядом, пустое, тёмное (свет не выше 12) и
/// годное для гриба (вики, Brown Mushroom) [проверить: порог 12 или 13].
fn mushroom_tick(world: &mut World, (x, y, z): (i32, i32, i32), state: i32) {
    if roll(world, 25) != 0 {
        return;
    }

    let mut left = 5;

    for dx in -4..=4 {
        for dz in -4..=4 {
            for dy in -1..=1 {
                if world.get_block(x + dx, y + dy, z + dz) == state {
                    left -= 1;

                    if left <= 0 {
                        return;
                    }
                }
            }
        }
    }

    let mut target = (x + roll(world, 3) - 1, y + roll(world, 2) - roll(world, 2), z + roll(world, 3) - 1);
    let mut from = (x, y, z);

    for _ in 0..4 {
        if world.get_block(target.0, target.1, target.2) == AIR && crate::plants::can_place(world, state, target.0, target.1, target.2) {
            from = target;
        }

        target = (from.0 + roll(world, 3) - 1, from.1 + roll(world, 2) - roll(world, 2), from.2 + roll(world, 3) - 1);
    }

    if world.get_block(target.0, target.1, target.2) == AIR
        && crate::plants::can_place(world, state, target.0, target.1, target.2)
        && light(world, target.0, target.1, target.2) <= 12
    {
        world.set_block(target.0, target.1, target.2, state);
    }
}

// ---------------------------------------------------------------------------
// Трава и мицелий
// ---------------------------------------------------------------------------

/// Пропускает ли блок над дёрном свет и жизнь: не жидкость и не полный
/// непрозрачный блок (снежный слой в один блок тоже не мешает).
fn lets_live(state: i32) -> bool {
    if state == AIR {
        return true;
    }

    if fluids::fluid_at(state).is_some() || value(state, "waterlogged") == Some("true") {
        return false;
    }

    name_of(state) == "snow" && number(state, "layers") == 1 || crate::world::light::filter(state) < 15
}

/// Трава и мицелий: под непрозрачным блоком гибнут в землю; при свете над
/// собой 9 и выше расползаются на землю в 3×5×3 — четыре попытки за такт
/// (вики, Grass Block: «checked 4 blocks», Mycelium).
fn spread_tick(world: &mut World, (x, y, z): (i32, i32, i32), state: i32, name: &str) {
    let above = world.get_block(x, y + 1, z);

    if !lets_live(above) {
        world.set_block(x, y, z, state_of("dirt"));
        return;
    }

    if light(world, x, y + 1, z) < 9 {
        return;
    }

    for _ in 0..4 {
        let (tx, ty, tz) = (x + roll(world, 3) - 1, y + roll(world, 5) - 3, z + roll(world, 3) - 1);

        if name_of(world.get_block(tx, ty, tz)) != "dirt" {
            continue;
        }

        let over = world.get_block(tx, ty + 1, tz);

        if lets_live(over) && light(world, tx, ty + 1, tz) >= 4 {
            let snowy = name_of(over) == "snow";
            let grown = with(state_of(name), "snowy", snowy).unwrap_or(state);

            world.set_block(tx, ty, tz, grown);
        }
    }
}

// ---------------------------------------------------------------------------
// Листва
// ---------------------------------------------------------------------------

/// Расстояние листа до бревна по соседям: бревно — 1, лист — его
/// расстояние + 1, не больше 7 (вики, Leaves).
fn leaf_distance(world: &World, (x, y, z): (i32, i32, i32)) -> i32 {
    let mut best = 7;

    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
        let near = world.get_block(x + dx, y + dy, z + dz);
        let near_name = name_of(near);

        if near_name.ends_with("_log") || near_name.ends_with("_wood") || near_name.ends_with("_stem") && !near_name.contains("melon") && !near_name.contains("pumpkin") && near_name != "mushroom_stem" || near_name.ends_with("_hyphae") {
            return 1;
        }

        if is_leaves(near_name) {
            best = best.min(number(near, "distance") + 1);
        }
    }

    best
}

/// Запланированный такт листа (после смены соседа): пересчёт расстояния.
fn leaves_recount(world: &mut World, pos: (i32, i32, i32), state: i32) {
    let distance = leaf_distance(world, pos);

    if distance != number(state, "distance") {
        grow_to(world, pos, with(state, "distance", distance));
    }
}

/// Случайный такт листа: оторванный от брёвен (расстояние 7) и не
/// поставленный игроком гниёт с выпадением (вики, Leaves).
fn leaves_tick(world: &mut World, pos: (i32, i32, i32), state: i32) {
    if number(state, "distance") >= 7 && value(state, "persistent") == Some("false") {
        world.note_destroyed(pos.0, pos.1, pos.2);
        world.set_block(pos.0, pos.1, pos.2, AIR);
    }
}

/// Такт блока, запланированный соседским обновлением (redstone::ticked).
pub fn ticked(world: &mut World, pos: (i32, i32, i32), state: i32) {
    if name_of(state) == "frogspawn" { crate::mob::water::eggs::frog_tick(world,pos); return; }
    if is_leaves(name_of(state)) {
        leaves_recount(world, pos, state);
    }
}

/// Соседское обновление: лист заводит себе такт пересчёта расстояния через
/// такт, грядка проверяет блок над собой. Возвращает true, если блок
/// сменился и дальше его разбирать не надо.
pub fn on_neighbour(world: &mut World, pos: (i32, i32, i32)) -> bool {
    let state = world.get_block(pos.0, pos.1, pos.2);
    let name = name_of(state);

    if name == "frogspawn" && !crate::mob::water::eggs::supports_frogspawn(world.get_block(pos.0,pos.1-1,pos.2)) {
        world.set_block(pos.0,pos.1,pos.2,AIR);
        return true;
    }
    if is_leaves(name) {
        if leaf_distance(world, pos) != number(state, "distance") {
            world.schedule_once(pos.0, pos.1, pos.2, 1, 0);
        }

        return false;
    }

    neighbour_changed(world, pos)
}

/// Состояние блока, который ставит игрок: лист — «поставлен игроком» и с
/// расстоянием по соседям (вики, Leaves: «placed by players… never
/// decay»).
pub fn placed(world: &World, state: i32, (x, y, z): (i32, i32, i32)) -> i32 {
    let name = name_of(state);

    if is_leaves(name) {
        let distance = leaf_distance(world, (x, y, z));

        return with(state, "persistent", true).and_then(|state| with(state, "distance", distance)).unwrap_or(state);
    }

    state
}

// ---------------------------------------------------------------------------
// Бамбук
// ---------------------------------------------------------------------------

/// Росток бамбука становится бамбуком в два блока: ствол на месте ростка и
/// побег над ним [проверить: листья побега].
fn grow_bamboo_shoot(world: &mut World, (x, y, z): (i32, i32, i32)) {
    let Some(block) = blocks::orientation("bamboo") else {
        return;
    };

    if let (Some(stalk), Some(top)) = (block.state(&[("leaves", "none")]), block.state(&[("leaves", "small")])) {
        world.set_block(x, y, z, stalk);
        world.set_block(x, y + 1, z, top);
    }
}

// ---------------------------------------------------------------------------
// Саженцы и деревья
// ---------------------------------------------------------------------------

/// Саженец растёт: стадия 0 → 1, со стадии 1 — дерево.
fn advance_sapling(world: &mut World, pos: (i32, i32, i32), state: i32) {
    if value(state, "stage") == Some("0") {
        grow_to(world, pos, with(state, "stage", 1));
        return;
    }

    grow_tree(world, pos, state);
}

/// Угол квадрата 2×2 из таких же саженцев, куда входит этот (меньшие x и
/// z). None — квадрата нет.
fn square_of(world: &World, (x, y, z): (i32, i32, i32), name: &str) -> Option<(i32, i32)> {
    let same = |sx: i32, sz: i32| name_of(world.get_block(sx, y, sz)) == name;

    [(0, 0), (-1, 0), (0, -1), (-1, -1)]
        .into_iter()
        .map(|(dx, dz)| (x + dx, z + dz))
        .find(|&(cx, cz)| same(cx, cz) && same(cx + 1, cz) && same(cx, cz + 1) && same(cx + 1, cz + 1))
}

/// Можно ли ставить часть дерева сюда: воздух, листва, трава, вода
/// (вики, Sapling: «space above»).
fn tree_may_replace(state: i32) -> bool {
    let name = name_of(state);

    state == AIR
        || is_leaves(name)
        || is_sapling(name)
        || matches!(name, "short_grass" | "fern" | "tall_grass" | "large_fern" | "vine" | "snow" | "dead_bush")
        || matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, _)))
}

/// Растит дерево из саженца (и из соседних, если стоят квадратом 2×2).
/// Форма — та же, что у деревьев генерации (world/trees.rs). Не хватает
/// места — саженец остаётся. Возвращает true, если дерево выросло.
pub fn grow_tree(world: &mut World, (x, y, z): (i32, i32, i32), state: i32) -> bool {
    let name = name_of(state);
    let square = square_of(world, (x, y, z), name);
    let seed = i64::from(world.random_bits()) << 32 | i64::from(world.random_bits());

    let (tree, origin) = match (square, trees::from_sapling(name, true, seed)) {
        (Some(corner), Some(tree)) if tree.wide() => (tree, corner),
        _ => match trees::from_sapling(name, false, seed) {
            Some(tree) if !tree.wide() => (tree, (x, z)),
            _ => return false,
        },
    };

    let ground = y - 1;
    let parts = tree.blocks_in(seed, origin.0, origin.1);

    // Ствол должен пройти: брёвна только сквозь воздух, листву и саженцы.
    for (dx, dy, dz, part) in &parts {
        if matches!(part, Part::Log(_)) && !tree_may_replace(world.get_block(origin.0 + dx, ground + dy, origin.1 + dz)) {
            return false;
        }
    }

    // Саженцы уходят, дальше — земля, брёвна, листва и прочее.
    let saplings: Vec<(i32, i32)> = if tree.wide() { vec![(0, 0), (1, 0), (0, 1), (1, 1)] } else { vec![(0, 0)] };

    for (dx, dz) in &saplings {
        world.set_block_silently(origin.0 + dx, y, origin.1 + dz, AIR);
    }

    let distances = crate::world::leaf_distances(&parts);
    let log_state = |axis: &trees::Axis| {
        let axis = match axis {
            trees::Axis::X => "x",
            trees::Axis::Y => "y",
            trees::Axis::Z => "z",
        };

        if tree.log.contains('[') {
            blocks::state_from_text(tree.log)
        } else {
            blocks::state_from_text(&format!("{}[axis={}]", tree.log, axis))
        }
    };

    for (dx, dy, dz, part) in &parts {
        let (bx, by, bz) = (origin.0 + dx, ground + dy, origin.1 + dz);
        let here = world.get_block(bx, by, bz);

        let put = match part {
            Part::Ground(text) => {
                let soil = matches!(name_of(here), "grass_block" | "dirt" | "podzol" | "coarse_dirt" | "rooted_dirt" | "mycelium" | "moss_block" | "farmland");

                if soil { blocks::state_from_text(text) } else { None }
            }
            Part::Log(axis) if tree_may_replace(here) || name_of(here).ends_with("_log") => log_state(axis),
            Part::Leaves if here == AIR || is_sapling(name_of(here)) => {
                let distance = distances.get(&(*dx, *dy, *dz)).copied().unwrap_or(7);

                blocks::state_from_text(&format!("{}[distance={}]", tree.leaves, distance))
                    .or_else(|| blocks::state_from_text(tree.leaves))
            }
            Part::Block(text) if here == AIR => blocks::state_from_text(text),
            _ => None,
        };

        if let Some(put) = put {
            world.set_block(bx, by, bz, put);
        }
    }

    // Саженцы ушли молча — клиенту их места сообщаются общей пачкой.
    for (dx, dz) in saplings {
        let (bx, bz) = (origin.0 + dx, origin.1 + dz);

        if world.get_block(bx, y, bz) == AIR {
            world.set_block_silently(bx, y, bz, state);
            world.set_block(bx, y, bz, AIR);
        } else {
            world.tell_later(bx, y, bz, 0);
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Мотыга
// ---------------------------------------------------------------------------

/// Что даёт мотыга этому блоку: новое состояние и выпавший предмет (вики,
/// Hoe): земля, дёрн, тропа — грядка; каменистая земля — земля; земля с
/// корнями — земля и висячие корни. Над блоком должен быть воздух, а
/// щёлкать — не по нижней грани.
pub fn till(world: &mut World, (x, y, z): (i32, i32, i32), face: i32) -> Option<Option<Stack>> {
    if face == 0 || world.get_block(x, y + 1, z) != AIR {
        return None;
    }

    let (to, drop) = match name_of(world.get_block(x, y, z)) {
        "dirt" | "grass_block" | "dirt_path" => ("farmland", None),
        "coarse_dirt" => ("dirt", None),
        "rooted_dirt" => ("dirt", blocks::item_named("hanging_roots").map(|item| Stack::new(item, 1))),
        _ => return None,
    };

    world.note_sound_by_hand(x, y, z, "minecraft:item.hoe.till", 1.0, 1.0);
    world.set_block(x, y, z, state_of(to));

    Some(drop)
}

// ---------------------------------------------------------------------------
// Костная мука
// ---------------------------------------------------------------------------

/// Событие мира «костная мука»: частицы роста и звук у клиента (вики,
/// Java Edition protocol: World Event 1505 «Bone meal particles and
/// sound», данные — число частиц).
pub const BONE_MEAL_EVENT: i32 = 1505;

/// Применяет костную муку к блоку. None — блок к ней глух, мука не
/// тратится; Some — мука потрачена (рост мог и не случиться: у саженца
/// шанс 45 %).
pub fn bone_meal(world: &mut World, (x, y, z): (i32, i32, i32)) -> Option<()> {
    let state = world.get_block(x, y, z);
    let name = name_of(state);
    let pos = (x, y, z);

    match name {
        // Пшеница, морковь, картофель, стебли — на 2…5 стадий (вики, Bone
        // Meal); свёкла — на одну с шансом 75 %; цветок-факел — на одну.
        name if crop_max_age(name).is_some() => {
            let age = number(state, "age");
            let max = crop_max_age(name).unwrap_or(7);

            if age >= max {
                return None;
            }

            let step = match name {
                "beetroots" => i32::from(chance(world, 0.75)),
                "torchflower_crop" => 1,
                _ => 2 + roll(world, 4),
            };

            world.note_level_event(x, y, z, BONE_MEAL_EVENT, 15);

            if step > 0 {
                grow_to(world, pos, with(state, "age", (age + step).min(max)));
            }
        }
        name if is_sapling(name) => {
            world.note_level_event(x, y, z, BONE_MEAL_EVENT, 15);

            if chance(world, 0.45) {
                advance_sapling(world, pos, state);
            }
        }
        "sweet_berry_bush" if number(state, "age") < 3 => {
            world.note_level_event(x, y, z, BONE_MEAL_EVENT, 15);
            grow_to(world, pos, with(state, "age", number(state, "age") + 1));
        }
        "cocoa" if number(state, "age") < 2 => {
            world.note_level_event(x, y, z, BONE_MEAL_EVENT, 15);
            grow_to(world, pos, with(state, "age", number(state, "age") + 1));
        }
        "kelp" | "kelp_plant" => {
            // Вики, Kelp: мука добавляет блок к верхушке, если над ней вода.
            let mut top = y;

            while matches!(name_of(world.get_block(x, top + 1, z)), "kelp" | "kelp_plant") {
                top += 1;
            }

            let tip = world.get_block(x, top, z);

            if !matches!(fluids::fluid_at(world.get_block(x, top + 1, z)), Some((fluids::Kind::Water, fluids::SOURCE))) || number(tip, "age") >= 25 {
                return None;
            }

            world.note_level_event(x, y, z, BONE_MEAL_EVENT, 15);
            world.set_block(x, top + 1, z, with(tip, "age", number(tip, "age") + 1).unwrap_or(tip));
            world.set_block(x, top, z, state_of("kelp_plant"));
        }
        "grass_block" if world.get_block(x, y + 1, z) == AIR => {
            world.note_level_event(x, y, z, BONE_MEAL_EVENT, 3);
            grass_bloom(world, pos);
        }
        _ => return None,
    }

    Some(())
}

/// Мука на дёрне: вокруг вырастают трава и изредка цветы — 128 попыток,
/// каждая бредёт от блока над дёрном на i/16 шагов (вики, Bone Meal:
/// «grass… flowers… depending on the biome») [проверить: доли цветов].
fn grass_bloom(world: &mut World, (x, y, z): (i32, i32, i32)) {
    let grass = state_of("short_grass");

    'attempt: for attempt in 0..128 {
        let mut at = (x, y + 1, z);

        for _ in 0..attempt / 16 {
            at = (at.0 + roll(world, 3) - 1, at.1 + (roll(world, 3) - 1) * roll(world, 3) / 2, at.2 + roll(world, 3) - 1);

            if name_of(world.get_block(at.0, at.1 - 1, at.2)) != "grass_block" || blocks::full_cube(world.get_block(at.0, at.1, at.2)) {
                continue 'attempt;
            }
        }

        if world.get_block(at.0, at.1, at.2) != AIR || name_of(world.get_block(at.0, at.1 - 1, at.2)) != "grass_block" {
            continue;
        }

        let plant = if roll(world, 8) == 0 { state_of(if roll(world, 2) == 0 { "dandelion" } else { "poppy" }) } else { grass };

        world.set_block(at.0, at.1, at.2, plant);
    }
}

// ---------------------------------------------------------------------------
// Урожай
// ---------------------------------------------------------------------------

/// Сколько успехов из `tries` бросков с вероятностью `p`.
fn binomial(world: &mut World, tries: i32, p: f32) -> i32 {
    (0..tries).filter(|_| chance(world, p)).count() as i32
}

/// Что выпадает из посева — несколько стопок (вики, Wheat Seeds, Carrot,
/// Potato, Beetroot Seeds, Nether Wart, Cocoa Beans, Sweet Berry Bush).
/// `fortune` — уровень «удачи» инструмента: добавляет броски. None — блок
/// не посев, решает общая таблица.
pub fn harvest(world: &mut World, state: i32, fortune: i32) -> Option<Vec<Stack>> {
    let name = name_of(state);
    let age = number(state, "age");
    let item = |name: &str, count: i32| blocks::item_named(name).filter(|_| count > 0).map(|item| Stack::new(item, count));
    // Три броска 4/7 и по одному на уровень удачи (вики: «57.14%»).
    let tries = 3 + fortune;
    let p = 4.0 / 7.0;

    let drops = match name {
        "wheat" if age >= 7 => {
            let seeds = 1 + binomial(world, tries, p);

            vec![item("wheat", 1), item("wheat_seeds", seeds)]
        }
        "wheat" => vec![item("wheat_seeds", 1)],
        "beetroots" if age >= 3 => {
            let seeds = 1 + binomial(world, tries, p);

            vec![item("beetroot", 1), item("beetroot_seeds", seeds)]
        }
        "beetroots" => vec![item("beetroot_seeds", 1)],
        "carrots" if age >= 7 => vec![item("carrot", 2 + binomial(world, tries, p))],
        "carrots" => vec![item("carrot", 1)],
        "potatoes" if age >= 7 => {
            let poison = chance(world, 0.02);

            vec![item("potato", 2 + binomial(world, tries, p)), item("poisonous_potato", i32::from(poison))]
        }
        "potatoes" => vec![item("potato", 1)],
        "nether_wart" if age >= 3 => vec![item("nether_wart", (2 + roll(world, 3) + roll(world, fortune + 1)).min(7))],
        "nether_wart" => vec![item("nether_wart", 1)],
        "cocoa" => vec![item("cocoa_beans", if age >= 2 { 3 } else { 1 })],
        "sweet_berry_bush" => match age {
            3 => vec![item("sweet_berries", 2 + roll(world, 2) + fortune)],
            2 => vec![item("sweet_berries", 1 + roll(world, 2) + fortune)],
            _ => Vec::new(),
        },
        "torchflower_crop" => vec![item("torchflower_seeds", 1)],
        _ => return None,
    };

    Some(drops.into_iter().flatten().collect())
}

/// Щелчок по кусту ягод от второй стадии: ягоды выпадают, куст
/// возвращается к стадии 1 (вики, Sweet Berry Bush: стадия 3 даёт 1–2,
/// стадия 4 — 2–3), звук сбора. None — куст не готов или это не куст.
pub fn pick_berries(world: &mut World, (x, y, z): (i32, i32, i32)) -> Option<Stack> {
    let state = world.get_block(x, y, z);

    if name_of(state) != "sweet_berry_bush" || number(state, "age") < 2 {
        return None;
    }

    let count = if number(state, "age") >= 3 { 2 } else { 1 } + roll(world, 2);

    world.note_sound(x, y, z, "minecraft:block.sweet_berry_bush.pick_berries", 1.0, 1.0);
    grow_to(world, (x, y, z), with(state, "age", 1));

    blocks::item_named("sweet_berries").map(|item| Stack::new(item, count))
}

/// Предмет посадки: семена ставят посев (вики, Wheat Seeds: «planted on
/// farmland»). Сама пшеница и прочий урожай, кроме моркови и картофеля,
/// не сажается.
pub fn planted_by(item: &str) -> Option<&'static str> {
    Some(match item {
        "wheat_seeds" => "wheat",
        "beetroot_seeds" => "beetroots",
        "carrot" => "carrots",
        "potato" => "potatoes",
        "melon_seeds" => "melon_stem",
        "pumpkin_seeds" => "pumpkin_stem",
        "torchflower_seeds" => "torchflower_crop",
        "sweet_berries" => "sweet_berry_bush",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone::settle;

    fn state(text: &str) -> i32 {
        blocks::state_from_text(text).unwrap_or_else(|| panic!("нет состояния {}", text))
    }

    /// Пол из земли на высоте 0, 9×9.
    fn world() -> World {
        let mut world = World::in_memory();

        for x in -6..=6 {
            for z in -6..=6 {
                world.set_block_silently(x, 0, z, state("dirt"));
            }
        }

        world
    }

    #[test]
    fn hoe_tills_dirt_and_grass_but_not_under_a_block() {
        let mut world = world();

        assert_eq!(till(&mut world, (0, 0, 0), 1), Some(None));
        assert_eq!(name_of(world.get_block(0, 0, 0)), "farmland");

        world.set_block(1, 0, 0, state("coarse_dirt"));
        assert_eq!(till(&mut world, (1, 0, 0), 1), Some(None));
        assert_eq!(name_of(world.get_block(1, 0, 0)), "dirt");

        world.set_block(2, 1, 0, state("stone"));
        assert_eq!(till(&mut world, (2, 0, 0), 1), None);
        assert_eq!(till(&mut world, (3, 0, 0), 0), None, "снизу не пашут");

        world.set_block(4, 0, 0, state("rooted_dirt"));
        let drop = till(&mut world, (4, 0, 0), 1).flatten();
        assert_eq!(drop.map(|stack| stack.item), blocks::item_named("hanging_roots"));
    }

    #[test]
    fn farmland_gets_wet_by_water_and_dries_to_dirt() {
        let mut world = world();

        world.set_block(0, 0, 0, state("farmland"));
        world.set_block(4, 0, 4, state("water"));
        { let here = world.get_block(0, 0, 0); farmland_tick(&mut world, (0, 0, 0), here) };
        assert_eq!(number(world.get_block(0, 0, 0), "moisture"), 7);

        // Вода в пяти блоках не достаёт.
        world.set_block(4, 0, 4, state("dirt"));
        world.set_block(5, 0, 0, state("water"));

        for expected in (0..7).rev() {
            { let here = world.get_block(0, 0, 0); farmland_tick(&mut world, (0, 0, 0), here) };
            assert_eq!(number(world.get_block(0, 0, 0), "moisture"), expected);
        }

        // С посевом сухая грядка стоит, без него — земля.
        world.set_block(0, 1, 0, state("wheat"));
        { let here = world.get_block(0, 0, 0); farmland_tick(&mut world, (0, 0, 0), here) };
        assert_eq!(name_of(world.get_block(0, 0, 0)), "farmland");

        world.set_block(0, 1, 0, AIR);
        { let here = world.get_block(0, 0, 0); farmland_tick(&mut world, (0, 0, 0), here) };
        assert_eq!(name_of(world.get_block(0, 0, 0)), "dirt");
    }

    #[test]
    fn a_solid_block_on_farmland_makes_dirt_and_the_crop_pops() {
        let mut world = world();

        world.set_block(0, 0, 0, state("farmland"));
        world.set_block(0, 1, 0, state("stone"));
        settle(&mut world);
        assert_eq!(name_of(world.get_block(0, 0, 0)), "dirt");

        world.set_block(1, 0, 0, state("farmland"));
        world.set_block(1, 1, 0, state("wheat[age=7]"));
        settle(&mut world);
        world.take_destroyed();
        to_dirt(&mut world, (1, 0, 0));
        settle(&mut world);
        assert_eq!(world.get_block(1, 1, 0), AIR);
        assert_eq!(world.take_destroyed().len(), 1, "пшеница выпала");
    }

    #[test]
    fn growth_points_follow_the_wiki() {
        let mut world = world();

        world.set_block(0, 0, 0, state("farmland"));
        assert_eq!(growth_points(&world, (0, 1, 0), "wheat"), 2.0);

        world.set_block(0, 0, 0, state("farmland[moisture=7]"));
        assert_eq!(growth_points(&world, (0, 1, 0), "wheat"), 4.0);

        for dx in -1..=1 {
            for dz in -1..=1 {
                world.set_block(dx, 0, dz, state("farmland[moisture=7]"));
            }
        }

        // 4 + 8·0,75 = 10 — наибольшее (вики: «points from 1 to 10»).
        assert_eq!(growth_points(&world, (0, 1, 0), "wheat"), 10.0);

        // Пшеница по диагонали делит пополам.
        world.set_block(1, 1, 1, state("wheat"));
        assert_eq!(growth_points(&world, (0, 1, 0), "wheat"), 5.0);

        // Ряд по одной оси — нет.
        world.set_block(1, 1, 1, AIR);
        world.set_block(1, 1, 0, state("wheat"));
        world.set_block(-1, 1, 0, state("wheat"));
        assert_eq!(growth_points(&world, (0, 1, 0), "wheat"), 10.0);
    }

    #[test]
    fn bone_meal_raises_wheat_by_two_to_five() {
        for _ in 0..20 {
            let mut world = world();

            world.set_block(0, 0, 0, state("farmland"));
            world.set_block(0, 1, 0, state("wheat"));
            assert!(bone_meal(&mut world, (0, 1, 0)).is_some());

            let age = number(world.get_block(0, 1, 0), "age");
            assert!((2..=5).contains(&age), "возраст {}", age);
        }

        let mut world = world();
        world.set_block(0, 0, 0, state("farmland"));
        world.set_block(0, 1, 0, state("wheat[age=7]"));
        assert!(bone_meal(&mut world, (0, 1, 0)).is_none(), "спелой мука не нужна");
        assert!(bone_meal(&mut world, (5, 0, 5)).is_none(), "земле тоже");
    }

    #[test]
    fn ripe_wheat_drops_wheat_and_seeds() {
        let mut world = world();

        for _ in 0..50 {
            let drops = harvest(&mut world, state("wheat[age=7]"), 0).unwrap();
            assert_eq!(drops[0].item, blocks::item_named("wheat").unwrap());
            assert_eq!(drops[1].item, blocks::item_named("wheat_seeds").unwrap());
            assert!((1..=4).contains(&drops[1].count));
        }

        let young = harvest(&mut world, state("wheat[age=3]"), 0).unwrap();
        assert_eq!(young.len(), 1);
        assert_eq!(young[0].item, blocks::item_named("wheat_seeds").unwrap());
        assert!(harvest(&mut world, state("stone"), 0).is_none());
    }

    #[test]
    fn sugar_cane_grows_to_three_blocks() {
        let mut world = world();

        world.set_block(0, 0, 0, state("sand"));
        world.set_block(1, 0, 0, state("water"));
        world.set_block(0, 1, 0, state("sugar_cane"));

        for _ in 0..200 {
            for y in 1..=4 {
                let here = world.get_block(0, y, 0);

                if name_of(here) == "sugar_cane" {
                    column_tick(&mut world, (0, y, 0), here, "sugar_cane");
                }
            }
        }

        assert_eq!(name_of(world.get_block(0, 3, 0)), "sugar_cane");
        assert_eq!(world.get_block(0, 4, 0), AIR, "выше трёх не растёт");
    }

    #[test]
    fn cactus_grows_and_stands() {
        let mut world = world();

        world.set_block(0, 0, 0, state("sand"));
        world.set_block(0, 1, 0, state("cactus"));
        settle(&mut world);

        for _ in 0..300 {
            random_ticks(&mut world, &[(0, 0)], 0, 4096);
            settle(&mut world);
        }

        // Над первым блоком — кактус или (в одном случае из десяти) цветок.
        assert!(matches!(name_of(world.get_block(0, 2, 0)), "cactus" | "cactus_flower"));
        assert_eq!(name_of(world.get_block(0, 1, 0)), "cactus");
    }

    /// Как площадка farm-cane-cactus на сервере: каменный пол, вода в нём,
    /// тростник и кактус; такт — как у сервера: очередь, случайные такты,
    /// разбор. Кактус не ломается, тростник растёт.
    #[test]
    fn cane_and_cactus_grow_like_on_the_server() {
        let mut world = World::in_memory();

        for x in -8..=8 {
            for z in -8..=8 {
                world.set_block_silently(x, -2, z, state("grass_block"));
                world.set_block_silently(x, -1, z, state("stone"));
            }
        }

        for (at, text) in [((0, -1, 0), "sand"), ((1, -1, 0), "water"), ((0, 0, 0), "sugar_cane"), ((-3, -1, 0), "sand"), ((-3, 0, 0), "cactus")] {
            world.set_block(at.0, at.1, at.2, state(text));
            settle(&mut world);
        }

        for _ in 0..200 {
            for ((x, y, z), kind) in world.advance(65_536) {
                match kind {
                    crate::world::TickKind::Block => crate::redstone::ticked(&mut world, (x, y, z)),
                    crate::world::TickKind::Fluid => crate::tick::flow(&mut world, x, y, z),
                }

                settle(&mut world);
            }

            random_ticks(&mut world, &[(0, 0), (-1, 0)], 1, 4096);
            settle(&mut world);
        }

        assert_eq!(name_of(world.get_block(-3, 0, 0)), "cactus");
        // Цветок (случайный, 10 %) может встать уже на втором блоке.
        assert!(matches!(name_of(world.get_block(-3, 1, 0)), "cactus" | "cactus_flower"));
        assert_eq!(name_of(world.get_block(0, 2, 0)), "sugar_cane");
    }

    #[test]
    fn trampling_needs_a_fall_over_half_a_block() {
        let mut world = world();

        world.set_block(0, 0, 0, state("farmland"));
        assert!(!trample(&mut world, (0, 0, 0), 0.5));
        assert!(trample(&mut world, (0, 0, 0), 5.0));
        assert_eq!(name_of(world.get_block(0, 0, 0)), "dirt");
    }

    #[test]
    fn an_oak_sapling_grows_into_a_tree() {
        let mut world = world();

        world.set_block(0, 0, 0, state("grass_block"));
        world.set_block(0, 1, 0, state("oak_sapling[stage=1]"));
        assert!({ let here = world.get_block(0, 1, 0); grow_tree(&mut world, (0, 1, 0), here) });
        assert!(name_of(world.get_block(0, 1, 0)).ends_with("_log"));
        assert_eq!(name_of(world.get_block(0, 0, 0)), "dirt", "под стволом земля");
    }

    #[test]
    fn four_spruce_saplings_grow_a_giant_and_one_dark_oak_does_not() {
        let mut world = world();

        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            world.set_block(dx, 1, dz, state("spruce_sapling[stage=1]"));
        }

        assert!({ let here = world.get_block(1, 1, 1); grow_tree(&mut world, (1, 1, 1), here) });

        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(name_of(world.get_block(dx, 1, dz)), "spruce_log");
        }

        world.set_block(5, 1, 5, state("dark_oak_sapling[stage=1]"));
        assert!(!{ let here = world.get_block(5, 1, 5); grow_tree(&mut world, (5, 1, 5), here) });
    }

    #[test]
    fn placed_leaves_are_persistent_and_detached_leaves_decay() {
        let mut world = world();
        let placed_leaf = placed(&world, state("oak_leaves"), (0, 1, 0));

        assert_eq!(value(placed_leaf, "persistent"), Some("true"));

        world.set_block(0, 1, 0, state("oak_leaves[distance=7,persistent=false]"));
        { let here = world.get_block(0, 1, 0); leaves_tick(&mut world, (0, 1, 0), here) };
        assert_eq!(world.get_block(0, 1, 0), AIR);
    }

    #[test]
    fn seeds_plant_crops() {
        assert_eq!(planted_by("wheat_seeds"), Some("wheat"));
        assert_eq!(planted_by("wheat"), None);
        assert_eq!(planted_by("carrot"), Some("carrots"));
    }

    #[test]
    fn random_ticks_reach_crops_with_a_high_speed() {
        let mut world = world();

        world.set_block(0, 0, 0, state("farmland[moisture=7]"));
        world.set_block(0, 1, 0, state("wheat"));

        for _ in 0..200 {
            random_ticks(&mut world, &[(0, 0)], 0, 4096);
        }

        assert_eq!(number(world.get_block(0, 1, 0), "age"), 7);
    }
}
