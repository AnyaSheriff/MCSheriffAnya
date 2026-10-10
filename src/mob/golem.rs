// Големы: железный и снежный (вики, Iron Golem, Snow Golem).
//
// Кода игры и чужих ядер здесь нет: правила — со страниц minecraft.wiki, числа,
// которых там нет, сняты с официального сервера чёрным ящиком (сценарии
// iron-golem-build, iron-golem-heal, iron-golem-attack, snow-golem-build,
// snow-golem-shear, snow-golem-shoot).
//
// Постройка: тыква (вырезанная или фонарь Джека), поставленная на столб из
// блоков, — и вместо блоков встаёт голем. Порядок, снятый с оригинала
// (iron-golem-build): событие мира 2001 (частицы ломания) по каждой клетке
// узора — и по пустым, с данными 0, — столбец за столбцом сверху вниз; потом
// голем в клетке ног (+0,05 по высоте, поворот 0); блоки уходят воздухом.
//
// Чего нет: появления железных големов в деревнях (у наших жителей нет сна,
// сплетен и паники) и предложения мака жителю — у нас некому его предлагать.

use super::ai::{self, Ctx, Plan};
use super::tame::Target;
use super::{path, Event, Kind, Mob, Rng, View};
use crate::blocks;
use crate::survival::Source;

/// Здоровье (вики: железный 100, снежный 4).
pub const IRON_HEALTH: f32 = 100.0;
pub const SNOW_HEALTH: f32 = 4.0;

/// Лечение слитком: 25 (вики, Iron Golem, «Healing»).
pub const INGOT_HEAL: f32 = 25.0;

/// Сила удара железного голема (атрибут attack_damage 15): урон —
/// половина плюс случайное целое до силы, 7,5–21,5 на обычной (вики, Iron
/// Golem: «Normal 7.5–21.5»).
pub const IRON_DAMAGE: u32 = 15;

/// Подброс цели ударом железного голема (чёрный ящик, iron-golem-attack:
/// зомби NoAI в воздухе получил скорость вверх 0,4).
pub const FLING: f64 = 0.4;

/// Пауза между ударами (вики, Mob AI, «Melee attack»: 20 тактов).
pub const IRON_COOLDOWN: i32 = 20;

/// Снежок: раз в 20 тактов, дальность 10 (вики, Snow Golem: «one snowball
/// per second», «up to 10 blocks»).
pub const SHOT_EVERY: i32 = 20;
pub const SHOT_RANGE: f64 = 10.0;

/// Скорость снежка голема и его разброс (как у брошенного игроком: 1,6)
/// [проверить: разброс].
pub const SNOWBALL_SPEED: f64 = 1.6;
pub const SNOWBALL_SPREAD: f64 = 12.0;

/// Глаза снежного голема: снежок вылетает на 0,1 ниже (вики, Snow Golem:
/// «eye height 1.7»; чёрный ящик, snow-golem-shear: тыква выпала на высоте 1,7).
pub const SNOW_EYE: f64 = 1.7;

/// Ищет цель раз в 10 тактов в среднем (вики, Mob AI, «Nearest attackable
/// target»: шанс 1/10 за такт).
const SEEK_CHANCE: u32 = 10;

/// Дальность поиска цели железного голема (атрибут follow_range 16).
const IRON_SEEK: f64 = 16.0;

/// Состояние голема.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// Железный: построен игроком (свойство 16, бит 1) — игроков не трогает.
    pub player_made: bool,

    /// Снежный: тыква на голове (свойство 16, бит 0x10); ножницы её снимают.
    pub pumpkin: bool,

    /// Цель.
    pub target: Option<Target>,

    /// Тактов до следующего удара или снежка.
    pub cooldown: i32,

    /// Сколько тактов цель на виду в дальности (снежный стоит и бросает).
    pub seen: i32,
}

impl Default for State {
    fn default() -> Self {
        Self { player_made: false, pumpkin: true, target: None, cooldown: 0, seen: 0 }
    }
}

/// Бросок снежка: откуда и с какой скоростью.
pub type Throw = ((f64, f64, f64), (f64, f64, f64));

/// Голем ли это.
pub fn golem(kind: Kind) -> bool {
    matches!(kind, Kind::IronGolem | Kind::SnowGolem)
}

/// Свойство 16 голема: у железного — «построен игроком», у снежного — тыква.
pub fn flags(mob: &Mob) -> u8 {
    match mob.kind {
        Kind::IronGolem => u8::from(mob.golem.player_made),
        Kind::SnowGolem if mob.golem.pumpkin => 0x10,
        _ => 0,
    }
}

/// Насколько треснул железный голем: 0 — цел (от 75 %), 1 — слегка (от
/// 50 %), 2 — средне (от 25 %), 3 — сильно (вики, Iron Golem, «Cracks»).
pub fn cracks(health: f32) -> u8 {
    let part = health / IRON_HEALTH;

    if part >= 0.75 {
        0
    } else if part >= 0.5 {
        1
    } else if part >= 0.25 {
        2
    } else {
        3
    }
}

/// Урон удара железного голема до поправки на сложность.
pub fn iron_damage(rng: &mut Rng) -> f32 {
    IRON_DAMAGE as f32 / 2.0 + rng.below(IRON_DAMAGE) as f32
}

/// Найденная постройка: кто встанет, события 2001 по клеткам узора (клетка,
/// состояние — в порядке оригинала), клетки, что уйдут воздухом, и где встанет.
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub kind: Kind,
    pub cells: Vec<((i32, i32, i32), i32)>,
    pub clear: Vec<(i32, i32, i32)>,
    pub at: (f64, f64, f64),
}

/// Клетка узора: тыква, блок тела, пусто (воздух) или что угодно.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Head,
    Body,
    Air,
}

/// Узоры (верхний ряд — первым): железный «~^~ / ### / ~#~», снежный «^/#/#»
/// (вики: «The four empty spaces … must be air blocks»).
const IRON: [[Cell; 3]; 3] = [[Cell::Air, Cell::Head, Cell::Air], [Cell::Body, Cell::Body, Cell::Body], [Cell::Air, Cell::Body, Cell::Air]];
const SNOW: [[Cell; 1]; 3] = [[Cell::Head], [Cell::Body], [Cell::Body]];

fn is_head(state: i32) -> bool {
    matches!(blocks::block_at_state(state), Some("carved_pumpkin" | "jack_o_lantern"))
}

fn is_air(state: i32) -> bool {
    matches!(blocks::block_at_state(state), Some("air" | "cave_air" | "void_air"))
}

/// Направления «вверх» узора (от ног к голове) и «вбок» (по ширине). Голем
/// строится и лёжа, и вниз головой (вики: «upright, lying down, or
/// upside-down»). Порядок «вбок»: первым −x — у оригинала события 2001 идут
/// с x+1 к x−1 (iron-golem-build) [проверить: порядок для оси z и лёжа].
const UPS: [(i32, i32, i32); 6] = [(0, 1, 0), (0, -1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];
const SIDES: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1), (0, -1, 0), (0, 1, 0)];

/// Ищет голема, голова которого — только что поставленная тыква в `head`.
/// Сперва снежный, потом железный [проверить: что раньше, если подходят оба].
/// Череп скелета-иссушителя ищет иссушителя (mob/wither.rs) тем же обходом.
pub fn find(view: &dyn View, head: (i32, i32, i32)) -> Option<Build> {
    let head_state = view.block(head.0, head.1, head.2);

    if super::wither::is_skull(head_state) {
        let rows: Vec<Vec<Cell>> = WITHER.iter().map(|row| row.to_vec()).collect();

        return find_pattern(view, head, Kind::Wither, &rows, &super::wither::is_skull, &super::wither::is_soul);
    }

    if !is_head(head_state) {
        return None;
    }

    let snow = blocks::state_by_name("snow_block");
    let iron = blocks::state_by_name("iron_block");

    for (kind, body) in [(Kind::SnowGolem, snow), (Kind::IronGolem, iron)] {
        let body = body?;
        let rows: Vec<Vec<Cell>> = if kind == Kind::SnowGolem { SNOW.iter().map(|row| row.to_vec()).collect() } else { IRON.iter().map(|row| row.to_vec()).collect() };

        if let Some(build) = find_pattern(view, head, kind, &rows, &is_head, &|state| state == body) {
            return Some(build);
        }
    }

    None
}

/// Узор иссушителя «^^^ / ### / ~#~»: три черепа на Т из песка или почвы душ
/// (вики, Wither: «air must be on both sides of the base block»).
const WITHER: [[Cell; 3]; 3] = [[Cell::Head, Cell::Head, Cell::Head], [Cell::Body, Cell::Body, Cell::Body], [Cell::Air, Cell::Body, Cell::Air]];

/// Обход узора во всех положениях: голова узора — `head` (у иссушителя
/// черепов три: годится любой из них).
fn find_pattern(view: &dyn View, head: (i32, i32, i32), kind: Kind, rows: &[Vec<Cell>], is_head: &dyn Fn(i32) -> bool, is_body: &dyn Fn(i32) -> bool) -> Option<Build> {
    let width = rows[0].len() as i32;
    let heads: Vec<i32> = rows[0].iter().enumerate().filter(|(_, cell)| **cell == Cell::Head).map(|(i, _)| i as i32).collect();
    // Ноги — под серединой нижнего ряда.
    let middle = rows[rows.len() - 1].iter().position(|cell| *cell != Cell::Air).unwrap_or(0) as i32;

    for head_i in heads {
        for up in UPS {
            for side in SIDES {
                let dot = up.0 * side.0 + up.1 * side.1 + up.2 * side.2;

                if dot != 0 {
                    continue;
                }

                // Клетка (i, j): j — ряд сверху вниз, i — по ширине.
                let at = |i: i32, j: i32| (head.0 + (i - head_i) * side.0 - j * up.0, head.1 + (i - head_i) * side.1 - j * up.1, head.2 + (i - head_i) * side.2 - j * up.2);
                let fits = rows.iter().enumerate().all(|(j, row)| {
                    row.iter().enumerate().all(|(i, cell)| {
                        let (x, y, z) = at(i as i32, j as i32);
                        let state = view.block(x, y, z);

                        match cell {
                            Cell::Head => is_head(state),
                            Cell::Body => is_body(state),
                            Cell::Air => is_air(state),
                        }
                    })
                });

                if !fits {
                    continue;
                }

                let mut cells = Vec::new();
                let mut clear = Vec::new();

                for i in 0..width {
                    for (j, row) in rows.iter().enumerate() {
                        let pos = at(i, j as i32);

                        cells.push((pos, view.block(pos.0, pos.1, pos.2)));

                        if row[i as usize] != Cell::Air {
                            clear.push(pos);
                        }
                    }
                }

                let feet = at(middle, rows.len() as i32 - 1);

                return Some(Build { kind, cells, clear, at: (f64::from(feet.0) + 0.5, f64::from(feet.1) + 0.05, f64::from(feet.2) + 0.5) });
            }
        }
    }

    None
}

/// Удар по голему: железный мстит ударившему (построенный игроком — только
/// не игрокам), снежный цели не меняет [проверить].
pub fn on_hurt(mob: &mut Mob, cause: i32, by_player: bool) {
    if mob.kind != Kind::IronGolem || mob.no_ai || cause == 0 {
        return;
    }

    if by_player {
        if !mob.golem.player_made {
            mob.golem.target = Some(Target::Player(cause));
        }
    } else {
        mob.golem.target = Some(Target::Mob(cause));
    }
}

/// Звук треска: урон перевёл железного голема в худшую стадию (чёрный ящик,
/// iron-golem-heal: 75 → 74 — entity.iron_golem.damage сразу за hurt).
pub fn crack_sound(mob: &mut Mob, before: f32) {
    if mob.kind == Kind::IronGolem && cracks(mob.vitals.health.max(0.0)) > cracks(before) {
        mob.events.push(Event::Sound { name: "minecraft:entity.iron_golem.damage", volume: 1.0, pitch: 1.0 });
    }
}

/// Такт снежного голема, и без ИИ: след снега и таяние (чёрный ящик,
/// snow-golem-shear: NoAI-голем тоже оставил снег под собой).
/// Возвращает клетки, куда лечь снегу.
pub fn snow_tick(mob: &mut Mob, view: &dyn View, griefing: bool) -> Vec<(i32, i32, i32)> {
    if mob.kind != Kind::SnowGolem || mob.is_dead() {
        return Vec::new();
    }

    let (cx, cy, cz) = mob.cell();
    let here = view.block(cx, cy, cz);

    // Тает: в жарком биоме (выше 1,0), под дождём и в воде — 1 за такт (вики,
    // Snow Golem: «1HP damage per tick … rain or water … greater than 1.0»).
    if view.temperature(cx, cy, cz) > 1.0 {
        mob.hurt(1.0, Source::OnFire, None);
    } else if super::is_water(here) || view.rain(cx, cy, cz) {
        mob.hurt(1.0, Source::Drown, None);
    }

    if mob.is_dead() || !griefing {
        return Vec::new();
    }

    // След: в четырёх точках ±0,25 вокруг ног, где воздух и снег устоит (вики:
    // «in any biome»; snow-golem-build: слой снега в клетке ног на следующем такте).
    let mut out: Vec<(i32, i32, i32)> = Vec::new();

    for corner in 0..4 {
        let dx = f64::from(corner % 2 * 2 - 1) * 0.25;
        let dz = f64::from(corner / 2 % 2 * 2 - 1) * 0.25;
        let cell = ((mob.x + dx).floor() as i32, mob.y.floor() as i32, (mob.z + dz).floor() as i32);

        if out.contains(&cell) || !is_air(view.block(cell.0, cell.1, cell.2)) {
            continue;
        }

        let below = view.block(cell.0, cell.1 - 1, cell.2);

        if blocks::solid_top(below) && !matches!(blocks::block_at_state(below), Some("ice" | "packed_ice" | "barrier")) {
            out.push(cell);
        }
    }

    out
}

/// Враг ли это для голема: враждебные, кроме гаста; железный не трогает
/// криперов (вики: снежный «also attack creepers», железный — нет).
fn foe(golem: Kind, kind: Kind) -> bool {
    kind.hostile() && kind != Kind::Ghast && !(golem == Kind::IronGolem && kind == Kind::Creeper)
}

/// Место цели: x, y, z, полуширина, высота. Номер ищется и среди игроков, и
/// среди мобов: удар снарядом не говорит, кто бросил (`on_hurt`).
fn place(ctx: &Ctx, target: Target) -> Option<(f64, f64, f64, f64, f64)> {
    let id = match target {
        Target::Player(id) | Target::Mob(id) => id,
    };

    ctx.players
        .iter()
        .find(|player| player.id == id && player.targetable)
        .map(|player| (player.x, player.y, player.z, 0.3, 1.8))
        .or_else(|| ctx.mates.iter().find(|mate| mate.id == id).map(|mate| (mate.x, mate.y, mate.z, mate.kind.width() / 2.0, mate.kind.height())))
}

/// Цель — игрок (а не моб): для построенного игроком железного голема запретна.
fn is_player(ctx: &Ctx, target: Target) -> bool {
    let id = match target {
        Target::Player(id) | Target::Mob(id) => id,
    };

    ctx.players.iter().any(|player| player.id == id)
}

/// Мысль голема: цель, удар или снежок, иначе — бродить, как животное.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let plan = Plan::default();

    mob.golem.cooldown = (mob.golem.cooldown - 1).max(0);

    let range = if mob.kind == Kind::IronGolem { IRON_SEEK } else { SHOT_RANGE };

    // Цель пропала или далеко — забыть.
    if let Some(target) = mob.golem.target
        && ((mob.golem.player_made && is_player(ctx, target))
            || place(ctx, target).is_none_or(|at| (at.0 - mob.x).powi(2) + (at.1 - mob.y).powi(2) + (at.2 - mob.z).powi(2) > (2.0 * range).powi(2)))
    {
        mob.golem.target = None;
        mob.golem.seen = 0;
    }

    // Новая цель: ближайший враг в дальности.
    if mob.golem.target.is_none() && rng.below(SEEK_CHANCE) == 0 {
        let kind = mob.kind;
        let near = ctx
            .mates
            .iter()
            .filter(|mate| mate.id != mob.entity_id && foe(kind, mate.kind))
            .map(|mate| (mate.id, (mate.x - mob.x).powi(2) + (mate.y - mob.y).powi(2) + (mate.z - mob.z).powi(2)))
            .filter(|(_, dist)| *dist <= range * range)
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((id, _)) = near {
            mob.golem.target = Some(Target::Mob(id));

            // Первый снежок — через 20 тактов после выбора цели (оригинал против
            // оригинала, snow-golem-shoot: ход с +5…+8, бросок на +23…+29).
            if kind == Kind::SnowGolem {
                mob.golem.cooldown = SHOT_EVERY;
            }
        }
    }

    let Some(target) = mob.golem.target else {
        return ai::think(mob, view, ctx, rng);
    };
    let Some(at) = place(ctx, target) else {
        return ai::think(mob, view, ctx, rng);
    };

    if mob.kind == Kind::IronGolem { melee(mob, view, target, is_player(ctx, target), at, plan, rng) } else { shoot(mob, view, target, at, plan, rng) }
}

/// Ближний бой железного голема: идти к цели, вплотную — ударить.
fn melee(mob: &mut Mob, view: &dyn View, target: Target, player: bool, at: (f64, f64, f64, f64, f64), mut plan: Plan, rng: &mut Rng) -> Plan {
    ai::look_at_point(mob, at.0, at.1 + at.4 * 0.85, at.2);

    // Досягаемость: ящик голема, расширенный вбок на √2,04 − 0,6, задевает
    // ящик цели (вики, Mob AI, «Melee attack») [проверить].
    let reach = mob.kind.width() / 2.0 + (2.04f64.sqrt() - 0.6) + at.3;
    let close = (at.0 - mob.x).abs() <= reach && (at.2 - mob.z).abs() <= reach && at.1 < mob.y + mob.kind.height() && mob.y < at.1 + at.4;

    if close && mob.golem.cooldown == 0 {
        mob.golem.cooldown = IRON_COOLDOWN;
        let damage = iron_damage(rng);
        let (dx, dz) = (at.0 - mob.x, at.2 - mob.z);
        let flat = dx.hypot(dz);
        let push = if flat > 1.0e-6 { (dx / flat, dz / flat) } else { (0.0, 0.0) };

        // Взмах, событие 4 (руки вверх), удар, звук (чёрный ящик, iron-golem-attack).
        mob.events.push(Event::Swing);
        mob.events.push(Event::Status(4));

        let (Target::Player(id) | Target::Mob(id)) = target;

        if player {
            plan.melee = Some(super::hostile::Melee { target: id, damage, push, wither: false, hunger: false });
        } else {
            plan.bite = Some((id, damage, push));
        }

        // [проверить] подброс игрока: у нас — только мобов. Звук удара по мобу
        // играет `step_all` после его урона и смерти (iron-golem-attack).
        plan.fling = true;

        if player {
            mob.events.push(Event::Sound { name: "minecraft:entity.iron_golem.attack", volume: 1.0, pitch: 1.0 });
        }
    }

    mob.brain.repath -= 1;

    if !mob.brain.walking() || mob.brain.repath <= 0 {
        mob.brain.repath = 4;
        let route = path::find(view, mob.cell(), (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32), path::MAX_NODES);

        mob.brain.go(route, 1.0);
    }

    ai::walk(mob, plan)
}

/// Снежный голем: в дальности и на виду — стоит и бросает снежок раз в 20
/// тактов; иначе идёт к цели (вики, Snow Golem) [проверить: скорость 1,25].
fn shoot(mob: &mut Mob, view: &dyn View, _target: Target, at: (f64, f64, f64, f64, f64), mut plan: Plan, rng: &mut Rng) -> Plan {
    let eye = (at.0, at.1 + at.4 * 0.85, at.2);

    ai::look_at_point(mob, eye.0, eye.1, eye.2);

    let dist2 = (at.0 - mob.x).powi(2) + (at.1 - mob.y).powi(2) + (at.2 - mob.z).powi(2);
    let sight = super::hostile::sees(view, (mob.x, mob.y + SNOW_EYE, mob.z), eye);

    mob.golem.seen = if sight { mob.golem.seen + 1 } else { 0 };

    if dist2 <= SHOT_RANGE * SHOT_RANGE && mob.golem.seen >= 5 {
        mob.brain.stop();
    } else {
        mob.brain.repath -= 1;

        if !mob.brain.walking() || mob.brain.repath <= 0 {
            mob.brain.repath = 4;
            let route = path::find(view, mob.cell(), (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32), path::MAX_NODES);

            mob.brain.go(route, 1.25);
        }

        plan = ai::walk(mob, plan);
    }

    if sight && dist2 <= SHOT_RANGE * SHOT_RANGE && mob.golem.cooldown == 0 {
        mob.golem.cooldown = SHOT_EVERY;

        let from = (mob.x, mob.y + SNOW_EYE - 0.1, mob.z);
        let (dx, dz) = (at.0 - from.0, at.2 - from.2);
        let dy = at.1 + at.4 * 0.85 - 1.1 - from.1;
        let flat = dx.hypot(dz);

        plan.snowball = Some((from, throw(dx, dy + flat * 0.2, dz, rng)));

        // Вики, Snow Golem, «Sounds»: entity.snow_golem.shoot, высота 0,4/(0,8–1,2).
        let pitch = (0.4 / (rng.unit() * 0.4 + 0.8)) as f32;

        mob.events.push(Event::Sound { name: "minecraft:entity.snow_golem.shoot", volume: 1.0, pitch });
    }

    plan
}

/// Скорость брошенного снежка по направлению с разбросом (как у снаряда:
/// гауссов шум 0,0075 × разброс на единицу направления) [проверить].
fn throw(dx: f64, dy: f64, dz: f64, rng: &mut Rng) -> (f64, f64, f64) {
    let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);
    let mut noise = || {
        // Сумма двенадцати равномерных − 6 ≈ нормальное распределение.
        let sum: f64 = (0..12).map(|_| rng.unit()).sum();

        (sum - 6.0) * 0.0075 * SNOWBALL_SPREAD
    };
    let (nx, ny, nz) = (dx / length + noise(), dy / length + noise(), dz / length + noise());

    (nx * SNOWBALL_SPEED, ny * SNOWBALL_SPEED, nz * SNOWBALL_SPEED)
}

/// Запись для файла региона: флаги голема.
pub fn write(state: &State, out: &mut Vec<u8>) {
    out.push(u8::from(state.player_made) | (u8::from(state.pumpkin) << 1));
}

pub fn read(bytes: &mut &[u8]) -> Option<State> {
    let (&flags, rest) = bytes.split_first()?;

    *bytes = rest;

    Some(State { player_made: flags & 1 != 0, pumpkin: flags & 2 != 0, ..State::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Grid(HashMap<(i32, i32, i32), i32>);

    impl View for Grid {
        fn block(&self, x: i32, y: i32, z: i32) -> i32 {
            self.0.get(&(x, y, z)).copied().unwrap_or(0)
        }

        fn loaded(&self, _: i32, _: i32) -> bool {
            true
        }
    }

    fn grid(blocks: &[((i32, i32, i32), &str)]) -> Grid {
        Grid(blocks.iter().map(|(at, name)| (*at, blocks::state_by_name(name).unwrap())).collect())
    }

    #[test]
    fn iron_golem_t_along_x_matches_vanilla_order() {
        let view = grid(&[((3, 0, 3), "iron_block"), ((3, 1, 3), "iron_block"), ((2, 1, 3), "iron_block"), ((4, 1, 3), "iron_block"), ((3, 2, 3), "carved_pumpkin")]);
        let build = find(&view, (3, 2, 3)).unwrap();

        assert_eq!(build.kind, Kind::IronGolem);
        // Чёрный ящик, iron-golem-build: 2001 по столбцам x 4, 3, 2 сверху вниз.
        let order: Vec<(i32, i32, i32)> = build.cells.iter().map(|(at, _)| *at).collect();
        assert_eq!(order, vec![(4, 2, 3), (4, 1, 3), (4, 0, 3), (3, 2, 3), (3, 1, 3), (3, 0, 3), (2, 2, 3), (2, 1, 3), (2, 0, 3)]);
        assert_eq!(build.cells[0].1, 0);
        assert_eq!(build.clear.len(), 5);
        assert_eq!(build.at, (3.5, 0.05, 3.5));
    }

    #[test]
    fn iron_golem_needs_air_corners_and_any_axis() {
        let along_z = grid(&[((3, 0, 3), "iron_block"), ((3, 1, 3), "iron_block"), ((3, 1, 2), "iron_block"), ((3, 1, 4), "iron_block"), ((3, 2, 3), "jack_o_lantern")]);
        assert_eq!(find(&along_z, (3, 2, 3)).map(|build| build.kind), Some(Kind::IronGolem));

        let blocked = grid(&[((3, 0, 3), "iron_block"), ((3, 1, 3), "iron_block"), ((3, 1, 2), "iron_block"), ((3, 1, 4), "iron_block"), ((3, 2, 3), "jack_o_lantern"), ((3, 0, 2), "stone")]);
        assert_eq!(find(&blocked, (3, 2, 3)), None);

        // Вниз головой (вики: «upside-down»).
        let down = grid(&[((3, 2, 3), "iron_block"), ((3, 1, 3), "iron_block"), ((2, 1, 3), "iron_block"), ((4, 1, 3), "iron_block"), ((3, 0, 3), "carved_pumpkin")]);
        let build = find(&down, (3, 0, 3)).unwrap();
        assert_eq!((build.kind, build.at), (Kind::IronGolem, (3.5, 2.05, 3.5)));
    }

    #[test]
    fn snow_golem_from_two_snow_blocks() {
        let view = grid(&[((3, 0, 3), "snow_block"), ((3, 1, 3), "snow_block"), ((3, 2, 3), "carved_pumpkin")]);
        let build = find(&view, (3, 2, 3)).unwrap();

        assert_eq!(build.kind, Kind::SnowGolem);
        assert_eq!(build.cells.iter().map(|(at, _)| *at).collect::<Vec<_>>(), vec![(3, 2, 3), (3, 1, 3), (3, 0, 3)]);
        assert_eq!(build.at, (3.5, 0.05, 3.5));
        assert_eq!(find(&grid(&[((3, 1, 3), "snow_block"), ((3, 2, 3), "carved_pumpkin")]), (3, 2, 3)), None);
    }

    #[test]
    fn cracks_follow_quarters_and_damage_range() {
        assert_eq!([cracks(100.0), cracks(75.0), cracks(74.0), cracks(50.0), cracks(49.0), cracks(24.0)], [0, 0, 1, 1, 2, 3]);

        let mut rng = Rng::new(5);
        for _ in 0..200 {
            let damage = iron_damage(&mut rng);
            assert!((7.5..=21.5).contains(&damage));
        }
    }

    #[test]
    fn golem_flags_survive_save_and_metadata() {
        let mut iron = Mob::new(7, Kind::IronGolem, (1.5, 2.0, 3.5));
        iron.golem.player_made = true;
        let mut snow = Mob::new(8, Kind::SnowGolem, (1.5, 2.0, 3.5));
        snow.golem.pumpkin = false;

        for mob in [iron, snow] {
            let mut out = Vec::new();
            mob.write(&mut out);
            let back = Mob::read(&mut out.as_slice()).unwrap();
            assert_eq!((back.kind, back.golem.player_made, back.golem.pumpkin), (mob.kind, mob.golem.player_made, mob.golem.pumpkin));
            assert_eq!(flags(&back), flags(&mob));
        }

        assert_eq!(flags(&Mob::new(9, Kind::SnowGolem, (0.0, 0.0, 0.0))), 0x10);
    }

    #[test]
    fn iron_golem_ignores_knockback_and_cracks_audibly() {
        let mut mob = Mob::new(3, Kind::IronGolem, (0.0, 0.0, 0.0));
        mob.vitals.health = 75.0;
        mob.on_ground = true;
        mob.hurt(1.0, Source::PlayerAttack, Some(super::super::Push { dir: (1.0, 0.0), strength: 0.4, cause: 42 }));

        assert_eq!((mob.vx, mob.vy, mob.vz), (0.0, 0.0, 0.0));
        assert!(mob.events.iter().any(|event| matches!(event, Event::Sound { name: "minecraft:entity.iron_golem.damage", .. })));
        // Построенный игроком — не мстит игроку; призванный — мстит.
        assert_eq!(mob.golem.target, Some(Target::Player(42)));
        assert!(!mob.brain.is_panicking());
    }

    #[test]
    fn snow_trail_lands_on_solid_ground() {
        let view = grid(&[((3, -1, 3), "stone")]);
        let mut mob = Mob::new(1, Kind::SnowGolem, (3.5, 0.0, 3.5));

        assert_eq!(snow_tick(&mut mob, &view, true), vec![(3, 0, 3)]);
        assert!(snow_tick(&mut mob, &view, false).is_empty());
    }
}
