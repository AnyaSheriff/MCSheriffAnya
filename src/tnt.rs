// Динамит: блок, подожжённый динамит и взрыв.
//
// Всё — по разбору tools/research/components-rest.md §8 (страницы
// minecraft.wiki: TNT, Explosion, Entity, Dispenser, Flint and Steel,
// Item (entity), Player). Кода игры и чужих ядер нет.
//
// Динамит — механизм редстоуна, но не проводящий: сигнал его поджигает, а
// соседний динамит от сигнала не загорается — только от взрыва (вики, TNT,
// «Redstone component»). Подожжённый динамит — сущность: летит и падает,
// как падающий блок, запал убывает на единицу за такт, на нуле — взрыв силы 4.
//
// Взрыв делается в три шага, как на вики (Explosion): сперва лучи решают,
// какие блоки рушатся; потом толкаются и ранятся сущности — пока блоки ещё
// стоят и заслоняют; потом блоки рушатся, из них выпадают предметы, а
// динамит в зоне загорается с коротким запалом.
//
// Чего вики не говорит прямо, помечено «[проверить]» — это сверит чёрный
// ящик (сценарии tnt-lever, tnt-chain, tnt-dispenser).

use std::collections::HashSet;

use crate::blocks;
use crate::custom_tnt::{self, KindId};
use crate::items::{self, Dropped, Items};
use crate::world::{AIR, Blast, CoordHash, MIN_Y, WORLD_HEIGHT, World};

/// Место в мире.
pub type Pos = (i32, i32, i32);

/// Запал при поджоге огнём, редстоуном, раздатчиком: 80 тактов (вики, TNT).
pub const FUSE: i32 = 80;

/// Запал при поджоге другим взрывом: случайно от 10 до 30 тактов (вики,
/// TNT: «a random number of game ticks between 10 and 30»). [проверить]:
/// входит ли 30.
const CHAIN_FUSE_MIN: i32 = 10;
const CHAIN_FUSE_MAX: i32 = 30;

/// Сила взрыва динамита (вики, TNT: «a power of 4»).
pub const POWER: f32 = 4.0;

/// Центр взрыва — на столько выше места сущности (вики, TNT: «0.06125
/// blocks above the entity's position»).
const CENTRE_LIFT: f64 = 0.06125;

/// Толчок подожжённого: 0.2 блока за такт вверх и 0.02 вбок в случайную
/// сторону (вики, TNT).
const KICK_UP: f64 = 0.2;
const KICK_SIDE: f64 = 0.02;

/// Ящик подожжённого динамита: 0.98 × 0.98 (вики, TNT, Primed TNT).
pub const SIZE: f64 = 0.98;

/// Звуки: поджог (1.0/1.0) и взрыв (4.0, высота 0.56–0.84; взрыв клиент
/// озвучивает сам по пакету). Раздел — «блоки» (вики, TNT, Sounds:
/// source=block).
pub const PRIMED_SOUND: &str = "minecraft:entity.tnt.primed";
pub const EXPLODE_SOUND: &str = "minecraft:entity.generic.explode";

/// Сколько блоков большой взрыв особого динамита рушит за такт; остальное
/// — в следующих тактах (`continue_blasts`). Замер (a_nuke_fits_into_a_tick):
/// ~3.5 мкс на блок со всеми соседями и редстоуном — полтысячи блоков
/// укладывается в несколько миллисекунд (первый такт ядерного — ~15 мс
/// вместе с лучами). У обычного динамита отложенного
/// не бывает: он рушит всё сразу, как оригинал.
pub const BLAST_BATCH: usize = 512;

/// Лучи — к точкам на поверхности куба 16 × 16 × 16 (вики, Explosion,
/// «Calculating which blocks to destroy»).
const GRID: i32 = 16;

/// Шаг луча и то, что он теряет на каждом шаге (там же).
const RAY_STEP: f64 = 0.3;
const RAY_FADE: f32 = 0.225_000_01;

/// Рост глаз игрока: 1.62, крадущегося — 1.27; ящик 0.6 × 1.8 (1.5)
/// (вики, Player).
const PLAYER_EYES: f64 = 1.62;
const SNEAKING_EYES: f64 = 1.27;

/// Глаза прочих сущностей — на 0.85 их высоты. [проверить]: у предмета и
/// падающего блока вики роста глаз не называет, это общее правило сущностей.
const EYES_SHARE: f64 = 0.85;

/// Динамит ли это.
pub fn is_tnt(state: i32) -> bool {
    blocks::block_at_state(state) == Some("tnt")
}

/// Где появляется подожжённый динамит из блока: середина низа блока (вики,
/// TNT: «offset from the bottom center … by [+0.5, +0.0, +0.5]»).
fn bottom_centre((x, y, z): Pos) -> (f64, f64, f64) {
    (x as f64 + 0.5, y as f64, z as f64 + 0.5)
}

/// Начальный толчок: вверх и чуть вбок в случайную сторону.
fn kick(world: &mut World) -> (f64, f64, f64) {
    let angle = world.random_unit() as f64 * std::f64::consts::TAU;

    (-angle.sin() * KICK_SIDE, KICK_UP, -angle.cos() * KICK_SIDE)
}

/// Поджигает динамит в этом месте — сигналом, огнивом, огненным зарядом,
/// огнивом из раздатчика: блок исчезает, на его месте появляется подожжённый
/// динамит с запалом 80. Сперва сущность и звук, потом блок убирается.
/// Возвращает, был ли там динамит.
///
/// Особый динамит (custom_tnt.rs) загорается своим видом — со своим
/// запалом.
pub fn ignite(world: &mut World, pos: Pos) -> bool {
    if !is_tnt(world.get_block(pos.0, pos.1, pos.2)) {
        return false;
    }

    let custom = world.custom_tnt_at(pos);

    spawn_primed(world, bottom_centre(pos), custom);
    world.set_block(pos.0, pos.1, pos.2, AIR);
    true
}

/// Выставляет подожжённый динамит в этой точке: раздатчик с динамитом
/// ставит его сразу подожжённым в блок перед собой, с тем же сдвигом и
/// толчком (вики, TNT). Сущность заведёт такт, звук поджога — здесь.
pub fn spawn_primed(world: &mut World, at: (f64, f64, f64), custom: Option<KindId>) {
    let velocity = kick(world);
    let fuse = custom.and_then(custom_tnt::by_id).map_or(FUSE, |kind| kind.fuse);

    world.prime(at, velocity, fuse, custom);
    world.note_sound_at_block(at, PRIMED_SOUND, 1.0, 1.0);
}

/// Раздатчик с динамитом: сущность в блоке перед ним.
pub fn dispense(world: &mut World, front: Pos, custom: Option<KindId>) {
    spawn_primed(world, bottom_centre(front), custom);
}

/// Кого из игроков задел взрыв и с какой силой удара (`impact`).
type Impact = (i32, f64);

/// Толчки игрокам: номер сущности и прибавка к скорости.
type Pushes = Vec<(i32, (f64, f64, f64))>;

/// Игрок, которого может толкнуть взрыв: номер сущности, ноги, крадётся ли.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub sneaking: bool,
}

/// Центр взрыва подожжённого динамита.
pub fn centre_of(tnt: &Dropped) -> (f64, f64, f64) {
    (tnt.x, tnt.y + CENTRE_LIFT, tnt.z)
}

/// Взрыв в этой точке: блоки, сущности, пакет клиентам. `next_id` выдаёт
/// номера новым сущностям — выпавшим предметам и загоревшемуся динамиту.
///
/// Каждый сломанный блок разбирается (`settle`) сразу, как у оригинала.
pub fn explode(
    world: &mut World,
    items: &mut Items,
    players: &[Target],
    centre: (f64, f64, f64),
    power: f32,
    spare_rails: bool,
    next_id: &mut dyn FnMut() -> i32,
) {
    blast(world, items, players, centre, power, true, false, spare_rails, next_id);
}

/// Взрыв догоревшего динамита: обычный — силы 4, особый — по своему виду
/// (custom_tnt.rs): своя сила, без разрушения блоков у безопасного, малые
/// заряды у кассетного.
pub fn explode_primed(
    world: &mut World,
    items: &mut Items,
    players: &[Target],
    primed: &Dropped,
    next_id: &mut dyn FnMut() -> i32,
) {
    let centre = centre_of(primed);

    world.blast_source = primed.entity_id;

    let Some(kind) = primed.custom.and_then(custom_tnt::by_id) else {
        explode(world, items, players, centre, POWER, false, next_id);
        return;
    };

    blast(world, items, players, centre, kind.power, kind.breaks_blocks, true, false, next_id);

    // Кассетный: малые обычные заряды во все стороны — из места взрыва,
    // вверх и вбок со случайной скоростью, с коротким запалом.
    let state = blocks::state_by_name("tnt").unwrap_or(AIR);

    for _ in 0..kind.cluster {
        let angle = world.random_unit() as f64 * std::f64::consts::TAU;
        let speed = 0.2 + world.random_unit() as f64 * 0.3;
        let up = 0.3 + world.random_unit() as f64 * 0.3;
        let spread = (custom_tnt::CLUSTER_FUSE_MAX - custom_tnt::CLUSTER_FUSE_MIN + 1) as f32;
        let fuse = custom_tnt::CLUSTER_FUSE_MIN + (world.random_unit() * spread) as i32;

        items.prime(
            next_id(),
            state,
            (primed.x, primed.y, primed.z),
            (-angle.sin() * speed, up, -angle.cos() * speed),
            fuse.min(custom_tnt::CLUSTER_FUSE_MAX),
            None,
        );
    }
}

/// Сам взрыв: лучи (если он рушит блоки), сущности, пакет, разрушение.
/// У взрыва, не рушащего блоков, лучей нет вовсе — и случай на них не
/// тратится. `defer` — большой список можно дорушить в следующих тактах
/// (только у особого динамита). `spare_rails` — взрыв вагонетки с
/// динамитом: рельсы и то, на чём они лежат, целы.
#[allow(clippy::too_many_arguments)]
fn blast(
    world: &mut World,
    items: &mut Items,
    players: &[Target],
    centre: (f64, f64, f64),
    power: f32,
    breaks_blocks: bool,
    defer: bool,
    spare_rails: bool,
    next_id: &mut dyn FnMut() -> i32,
) {
    let (mut doomed, reached) = if breaks_blocks { rays(world, centre, power) } else { (Vec::new(), 0) };

    // Вагонетка с динамитом рельсы и то, на чём они лежат, не рушит (вики,
    // Minecart with TNT: «it does not destroy its rails and the blocks the
    // rail is on»). Лучи они держат, как обычно. [проверить]
    if spare_rails {
        let rail = |state: i32| blocks::block_at_state(state).is_some_and(|name| name.ends_with("rail"));

        doomed.retain(|&(x, y, z)| !rail(world.get_block(x, y, z)) && !rail(world.get_block(x, y + 1, z)));
    }

    // Сущности — пока блоки стоят: заслоняют именно они (вики, Explosion:
    // «Exposure»).
    let (pushed, hurts) = push_entities(world, items, players, centre, power);

    // Урон игрокам — раньше разрушения и пакета взрыва (чёрный ящик,
    // tnt-damage: урон, смерть и её сообщение идут до вспышки и предметов).
    let culprit = world.blast_source;

    for (entity_id, impact) in hurts {
        world.note_hurt(entity_id, crate::survival::explosion_damage(impact, power), culprit);
    }

    break_carts(world, items, next_id);

    // Большой список особого динамита рушится частями по тактам.
    if defer && doomed.len() > BLAST_BATCH {
        world.defer_blast(doomed[BLAST_BATCH..].iter().copied());
        destroy_blocks(world, items, &doomed[..BLAST_BATCH], next_id);
    } else {
        destroy_blocks(world, items, &doomed, next_id);
    }

    // Пакет взрыва — после разрушения: у оригинала частицы отвалившегося
    // (2001) и выпавшие предметы приходят раньше него (чёрный ящик,
    // tnt-lever и tnt-powered-place).
    world.note_explosion(Blast {
        centre,
        power,
        blocks: reached as i32,
        pushed,
    });

    world.blast_source = 0;
}

/// Рушит следующую часть отложенного большим взрывом (`BLAST_BATCH`).
/// Возвращает, было ли что рушить: нет — такт ничего не делал.
pub fn continue_blasts(world: &mut World, items: &mut Items, next_id: &mut dyn FnMut() -> i32) -> bool {
    let batch = world.take_blast_batch(BLAST_BATCH);

    if batch.is_empty() {
        return false;
    }

    destroy_blocks(world, items, &batch, next_id);
    true
}

/// Какие блоки рушит взрыв — лучами (вики, Explosion, дословно):
/// «"rays" are created from the explosion center to the outer points of a 16
/// by 16 by 16 grid. Each of those rays gets an intensity of power · (random
/// value from 0.7 to 1.3)». Дальше по шагам: блок в точке (форма не важна);
/// не воздух — интенсивность теряет (стойкость + 0.3) · 0.3; осталась и блок
/// взрывается — в список; шаг 0.3 вдоль луча; −0.22500001; пока больше нуля.
/// Готовый список перемешивается («the list of blocks to blow up is
/// shuffled»).
///
/// Случай — из генератора мира: при одном и том же генераторе список один
/// и тот же.
#[cfg(test)]
pub fn blocks_to_destroy(world: &mut World, centre: (f64, f64, f64), power: f32) -> Vec<Pos> {
    rays(world, centre, power).0
}

/// Лучи взрыва: что рушится и сколько клеток лучи задели вообще, с воздухом
/// вместе, — это число оригинал кладёт в пакет взрыва (чёрный ящик,
/// tnt-lever: ~550 клеток при 11 сломанных блоках).
fn rays(world: &mut World, centre: (f64, f64, f64), power: f32) -> (Vec<Pos>, usize) {
    // Хеш — быстрый (`CoordHash`): у ядерного динамита клеток сотни тысяч.
    let mut seen: HashSet<Pos, CoordHash> = HashSet::default();
    let mut reached: HashSet<Pos, CoordHash> = HashSet::default();
    let mut doomed = Vec::new();
    let edge = |n: i32| n == 0 || n == GRID - 1;
    let last = (GRID - 1) as f64;

    for i in 0..GRID {
        for j in 0..GRID {
            for k in 0..GRID {
                if !(edge(i) || edge(j) || edge(k)) {
                    continue;
                }

                let raw = (
                    i as f64 / last * 2.0 - 1.0,
                    j as f64 / last * 2.0 - 1.0,
                    k as f64 / last * 2.0 - 1.0,
                );
                let length = (raw.0 * raw.0 + raw.1 * raw.1 + raw.2 * raw.2).sqrt();
                let step = (raw.0 / length * RAY_STEP, raw.1 / length * RAY_STEP, raw.2 / length * RAY_STEP);

                let mut intensity = power * (0.7 + world.random_unit() * 0.6);
                let mut at = centre;

                while intensity > 0.0 {
                    let cell = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);

                    // За краем мира блоков нет — луч дальше не идёт.
                    if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&cell.1) {
                        break;
                    }

                    let state = world.get_block(cell.0, cell.1, cell.2);

                    if state != AIR {
                        intensity -= (blocks::blast_resistance(state) + 0.3) * 0.3;
                    }

                    if intensity > 0.0 {
                        reached.insert(cell);

                        if can_blow_up(state) && seen.insert(cell) {
                            doomed.push(cell);
                        }
                    }

                    at = (at.0 + step.0, at.1 + step.1, at.2 + step.2);
                    intensity -= RAY_FADE;
                }
            }
        }
    }

    // Перемешать — Фишер–Йейтс на генераторе мира.
    for index in (1..doomed.len()).rev() {
        let other = world.random_bits() as usize % (index + 1);
        doomed.swap(index, other);
    }

    (doomed, reached.len())
}

/// Можно ли взорвать блок. Воздух — нечего; служебный блок едущего поршня
/// не трогаем: его убирает сам ход. [проверить] у оригинала. Прочее держит
/// лучи своей стойкостью (бедрок, барьер — 3 600 000).
fn can_blow_up(state: i32) -> bool {
    state != AIR && blocks::block_at_state(state) != Some("moving_piston")
}

/// Толкает и ранит сущности в радиусе 2 · сила (вики, Explosion):
/// `impact = (1 − distance/(2·power)) · exposure`; скорость прибавляется на
/// `impact` в сторону от центра к глазам, у динамита — к ногам («TNT
/// entities are the only exception, where the direction goes to the tnt's
/// feet instead of its eyes»); урон `(impact² + impact)/2 · 7 · (2·power) + 1`.
/// Расстояние — до ног.
///
/// Лежащий предмет от урона пропадает (здоровье 5; «Nether stars are immune
/// to explosions» — вики, Item (entity)). Падающий блок и динамит только
/// толкаются. Игроков двигает их клиент — возвращается, кого и как толкнуть.
/// [проверить]: не толкается ли летящий игрок в творческом (вики, Explosion,
/// 1.9: «only if they are not flying» — полёт сервер пока не знает).
fn push_entities(
    world: &World,
    items: &mut Items,
    players: &[Target],
    centre: (f64, f64, f64),
    power: f32,
) -> (Pushes, Vec<Impact>) {
    let reach = 2.0 * power as f64;
    let nether_star = blocks::item_named("nether_star");

    for item in items.items_mut() {
        let (width, height) = match (item.fuse, item.block) {
            (Some(_), _) | (None, Some(_)) => (SIZE, SIZE),
            (None, None) => (0.25, 0.25),
        };
        let eyes = if item.fuse.is_some() { 0.0 } else { height * EYES_SHARE };

        let Some(impact) = impact(world, centre, reach, (item.x, item.y, item.z), (width, height)) else {
            continue;
        };
        let Some(push) = push_towards(centre, (item.x, item.y + eyes, item.z), impact) else {
            continue;
        };

        item.vx += push.0;
        item.vy += push.1;
        item.vz += push.2;
        item.on_ground = false;

        if item.block.is_none() && Some(item.stack.item) != nether_star {
            item.health -= damage(impact, reach);
        }
    }

    items.remove_broken();

    // Вагонетки толкаются, как сущности, и получают урон: у них он
    // вдесятеро больше (как от удара рукой, minecart.rs). Глаза — на 0,85
    // высоты. [проверить]
    // Взрывающаяся сама себя не толкает.
    for cart in items.carts_mut().iter_mut().filter(|cart| cart.blast.is_none()) {
        let size = (crate::minecart::WIDTH, crate::minecart::HEIGHT);

        let Some(impact) = impact(world, centre, reach, (cart.x, cart.y, cart.z), size) else {
            continue;
        };
        let Some(push) = push_towards(centre, (cart.x, cart.y + size.1 * EYES_SHARE, cart.z), impact) else {
            continue;
        };

        cart.blasted(push, damage(impact, reach));
    }

    // Мобы получают урон и толчок, как игроки (вики, Explosion: «Damage»);
    // уходящий крипер сам себя не задевает. [проверить]
    for mob in items.mobs_mut().iter_mut().filter(|mob| !mob.is_dead() && !mob.h.despawn) {
        let size = mob.size();

        let Some(impact) = impact(world, centre, reach, (mob.x, mob.y, mob.z), size) else {
            continue;
        };

        if let Some(push) = push_towards(centre, (mob.x, mob.y + size.1 * EYES_SHARE, mob.z), impact) {
            mob.vx += push.0;
            mob.vy += push.1;
            mob.vz += push.2;
            mob.on_ground = false;
        }

        mob.hurt(crate::survival::explosion_damage(impact, power), crate::survival::Source::Explosion, None);
    }

    let mut pushed = Vec::new();
    let mut hurts = Vec::new();

    for player in players {
        let (height, eyes) = if player.sneaking { (1.5, SNEAKING_EYES) } else { (1.8, PLAYER_EYES) };
        let Some(impact) = impact(world, centre, reach, (player.x, player.y, player.z), (0.6, height)) else {
            continue;
        };

        // Урон идёт, даже когда толкать некуда (центр в самом игроке).
        hurts.push((player.entity_id, impact));

        if let Some(push) = push_towards(centre, (player.x, player.y + eyes, player.z), impact) {
            pushed.push((player.entity_id, push));
        }
    }

    (pushed, hurts)
}

/// Сила удара по сущности с ногами в `feet` и ящиком `size` (ширина,
/// высота): `(1 − distance/reach) · exposure`. None — дальше радиуса.
fn impact(world: &World, centre: (f64, f64, f64), reach: f64, feet: (f64, f64, f64), size: (f64, f64)) -> Option<f64> {
    let distance = distance(centre, feet) / reach;

    if distance > 1.0 {
        return None;
    }

    Some((1.0 - distance) * exposure(world, centre, feet, size))
}

/// Прибавка к скорости: длиной `impact`, от центра к точке `towards`.
/// Точка в самом центре — направления нет, толчка тоже.
fn push_towards(centre: (f64, f64, f64), towards: (f64, f64, f64), impact: f64) -> Option<(f64, f64, f64)> {
    let way = (towards.0 - centre.0, towards.1 - centre.1, towards.2 - centre.2);
    let length = (way.0 * way.0 + way.1 * way.1 + way.2 * way.2).sqrt();

    if length == 0.0 {
        return None;
    }

    Some((way.0 / length * impact, way.1 / length * impact, way.2 / length * impact))
}

/// Вагонетки, разбитые взрывом, пропадают, из них выпадает то же, что
/// при поломке рукой (вики, Minecart: «drops it as an item»). [проверить]
fn break_carts(world: &mut World, items: &mut Items, next_id: &mut dyn FnMut() -> i32) {
    for cart in items.carts_mut() {
        if cart.kind == crate::minecart::Kind::Tnt && cart.fuse < 0 {
            cart.blast_prime(world.random_unit());
        }
    }

    for broken in items.remove_carts(|cart| cart.wrecked()) {
        let at = (broken.x, broken.y, broken.z);

        for stack in broken.drops() {
            items.drop_item(next_id(), stack, at, (0.0, 0.0, 0.0), crate::items::MINED_DELAY);
        }
    }
}

/// Урон от взрыва (вики, Explosion, «Damage»).
fn damage(impact: f64, reach: f64) -> f32 {
    ((impact * impact + impact) / 2.0 * 7.0 * reach + 1.0) as f32
}

fn distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
}

/// Открытость сущности взрыву — доля лучей от точек в её ящике к центру,
/// которые не упёрлись в блок (вики, Explosion, «Exposure»). Точки — сетка
/// ⌈2·ширина+1⌉ × ⌈2·высота+1⌉ × ⌈2·длина+1⌉ с шагом ширина/(2·ширина+1);
/// угол сетки сдвинут на ½·(1 − ⌊2·ширина+1⌋/(2·ширина+1)) — в блоках, а не
/// в долях ширины: это ошибка оригинала (MC-232355), её и повторяем.
///
/// Луч заслоняет любой блок с ящиком столкновений, как целый куб.
/// [проверить]: у оригинала неполные блоки (плиты, ступени) заслоняют
/// только своей формой.
pub fn exposure(world: &World, centre: (f64, f64, f64), feet: (f64, f64, f64), (width, height): (f64, f64)) -> f64 {
    let step_x = 1.0 / (width * 2.0 + 1.0);
    let step_y = 1.0 / (height * 2.0 + 1.0);
    let shift = (1.0 - (1.0 / step_x).floor() * step_x) / 2.0;
    let low = (feet.0 - width / 2.0, feet.1, feet.2 - width / 2.0);

    let mut open = 0u32;
    let mut all = 0u32;
    let mut fx = 0.0;

    while fx <= 1.0 {
        let mut fy = 0.0;

        while fy <= 1.0 {
            let mut fz = 0.0;

            while fz <= 1.0 {
                let point = (low.0 + fx * width + shift, low.1 + fy * height, low.2 + fz * width + shift);

                if !ray_blocked(world, point, centre) {
                    open += 1;
                }

                all += 1;
                fz += step_x;
            }

            fy += step_y;
        }

        fx += step_x;
    }

    if all == 0 { 0.0 } else { open as f64 / all as f64 }
}

/// Упирается ли отрезок от `from` до `to` в блок с ящиком: проход по
/// клеткам, через которые он идёт, по порядку.
fn ray_blocked(world: &World, from: (f64, f64, f64), to: (f64, f64, f64)) -> bool {
    let cell = |v: f64| v.floor() as i32;
    let mut at = [cell(from.0), cell(from.1), cell(from.2)];
    let end = [cell(to.0), cell(to.1), cell(to.2)];
    let start = [from.0, from.1, from.2];
    let way = [to.0 - from.0, to.1 - from.1, to.2 - from.2];

    let mut step = [0i32; 3];
    let mut next = [f64::INFINITY; 3];
    let mut delta = [f64::INFINITY; 3];

    for axis in 0..3 {
        if way[axis] > 0.0 {
            step[axis] = 1;
            delta[axis] = 1.0 / way[axis];
            next[axis] = ((at[axis] + 1) as f64 - start[axis]) / way[axis];
        } else if way[axis] < 0.0 {
            step[axis] = -1;
            delta[axis] = -1.0 / way[axis];
            next[axis] = (at[axis] as f64 - start[axis]) / way[axis];
        }
    }

    // Отрезок короче радиуса взрыва — клеток на пути немного; предел на
    // всякий случай.
    for _ in 0..256 {
        if blocks::has_box(world.get_block(at[0], at[1], at[2])) {
            return true;
        }

        if at == end {
            return false;
        }

        let axis = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };

        if next[axis] > 1.0 {
            return false;
        }

        at[axis] += step[axis];
        next[axis] += delta[axis];
    }

    false
}

/// Рушит блоки по списку: динамит загорается с коротким запалом, прочее
/// выпадает предметом — у динамита всегда («Blocks destroyed by TNT have a
/// 100% chance of dropping», вики, Explosion; правило
/// `tnt_explosion_drop_decay` по умолчанию выключено). Содержимое хранилищ
/// высыплет мир, как при любой смене блока.
///
/// Одинаковое выпавшее оригинал сливает в одну стопку, пока она влезает,
/// и роняет её там, где выпала первая, — уже после всех блоков, позже
/// подожжённого взрывом динамита (чёрный ящик, tnt-lever и tnt-chain).
fn destroy_blocks(world: &mut World, items: &mut Items, doomed: &[Pos], next_id: &mut dyn FnMut() -> i32) {
    let tnt = blocks::state_by_name("tnt").unwrap_or(AIR);
    let mut dropped: Vec<(crate::inventory::Stack, (f64, f64, f64))> = Vec::new();

    for &(x, y, z) in doomed {
        let state = world.get_block(x, y, z);

        if state == AIR {
            continue;
        }

        // Динамит в зоне взрыва загорается сам, без звука поджога
        // ([проверить]), и сразу — сущностью в этом же такте.
        if is_tnt(state) {
            let fuse = CHAIN_FUSE_MIN + (world.random_unit() * (CHAIN_FUSE_MAX - CHAIN_FUSE_MIN + 1) as f32) as i32;
            let velocity = kick(world);
            // Особый динамит загорается своим видом — запал короткий, как у
            // всякого подожжённого взрывом.
            let custom = world.custom_tnt_at((x, y, z));

            items.prime(next_id(), tnt, bottom_centre((x, y, z)), velocity, fuse.min(CHAIN_FUSE_MAX), custom);
            world.set_block(x, y, z, AIR);
            crate::redstone::settle(world);
            drop_fallen_off(world, items, next_id);
            continue;
        }

        let luck = world.random_unit();

        if let Some(stack) = blocks::explosion_drops(state, luck) {
            let joined = dropped
                .iter_mut()
                .find(|(pile, _)| pile.item == stack.item && pile.count + stack.count <= pile.limit());

            match joined {
                Some((pile, _)) => pile.count += stack.count,
                None => dropped.push((stack, (x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5))),
            }
        }

        world.set_block(x, y, z, AIR);

        // Соседи узнают о сломе сразу, до следующего блока: висевшее на нём
        // отваливается с частицами (2001) ещё посреди взрыва (чёрный ящик,
        // tnt-dispenser и tnt-chain).
        crate::redstone::settle(world);
        drop_fallen_off(world, items, next_id);
    }

    for (stack, at) in dropped {
        items.drop_item(next_id(), stack, at, (0.0, 0.0, 0.0), items::MINED_DELAY);
    }
}

/// Отвалившееся посреди взрыва (рычаг без опоры) выпадает сразу, как при
/// обычном сломе, — раньше добычи самого взрыва, которая высыпается в конце:
/// у оригинала предмет рычага приходит первым (чёрный ящик,
/// tnt-powered-place).
fn drop_fallen_off(world: &mut World, items: &mut Items, next_id: &mut dyn FnMut() -> i32) {
    for ((x, y, z), state) in world.take_destroyed() {
        let luck = world.random_unit();

        if let Some(stack) = blocks::drops_by_luck(state, None, luck) {
            items.drop_item(next_id(), stack, (x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5), (0.0, 0.0, 0.0), items::MINED_DELAY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone;

    /// Читатель журналов в проверках.
    const READER: crate::journal::Reader = 1;

    fn state(name: &str) -> i32 {
        blocks::state_by_name(name).unwrap_or_else(|| panic!("нет блока {}", name))
    }

    /// Каменный куб 21 × 21 × 21 с центром в (0, 0, 0) и пустотой в самом
    /// центре — туда и ставится динамит.
    fn stone_cube() -> World {
        let mut world = World::in_memory();
        let stone = state("stone");

        for x in -10..=10 {
            for y in -10..=10 {
                for z in -10..=10 {
                    world.set_block_silently(x, y, z, stone);
                }
            }
        }

        world.set_block_silently(0, 0, 0, AIR);
        world
    }

    fn ids() -> impl FnMut() -> i32 {
        let mut next = 100;

        move || {
            next += 1;
            next
        }
    }

    /// При одном и том же генераторе случая взрыв рушит одно и то же — и
    /// в одном порядке.
    #[test]
    fn an_explosion_is_deterministic_for_a_fixed_random() {
        let mut first = stone_cube();
        let mut second = stone_cube();

        let a = blocks_to_destroy(&mut first, (0.5, 0.06125, 0.5), POWER);
        let b = blocks_to_destroy(&mut second, (0.5, 0.06125, 0.5), POWER);

        assert!(!a.is_empty());
        assert_eq!(a, b);
    }

    /// Динамит внутри камня: рушится только ближнее — у камня стойкость 6,
    /// и шаг луча по камню стоит (6 + 0.3) · 0.3 + 0.225 из самое большее
    /// 4 · 1.3. Дальше второго блока от центра не рушится ничего, а соседи
    /// центра рушатся все.
    #[test]
    fn stone_stops_the_rays_soon() {
        let mut world = stone_cube();
        let doomed = blocks_to_destroy(&mut world, (0.5, 0.06125, 0.5), POWER);

        for &(x, y, z) in &doomed {
            assert!(x.abs().max(y.abs()).max(z.abs()) <= 2, "рухнул далёкий блок {:?}", (x, y, z));
        }

        for near in [(1, 0, 0), (-1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, 1), (0, 0, -1)] {
            assert!(doomed.contains(&near), "устоял сосед {:?}", near);
        }
    }

    /// Бедрок взрыву не по зубам: стойкость 3 600 000.
    #[test]
    fn bedrock_survives() {
        let mut world = World::in_memory();
        let bedrock = state("bedrock");

        for x in -3..=3 {
            for z in -3..=3 {
                world.set_block_silently(x, -1, z, bedrock);
            }
        }

        let doomed = blocks_to_destroy(&mut world, (0.5, 0.06125, 0.5), POWER);

        assert!(doomed.is_empty(), "рухнуло: {:?}", doomed);
    }

    /// Вода гасит взрыв: у неё стойкость 100.
    #[test]
    fn water_dampens_the_explosion() {
        let mut world = stone_cube();

        world.set_block_silently(0, 0, 0, state("water"));

        assert!(blocks_to_destroy(&mut world, (0.5, 0.06125, 0.5), POWER).is_empty());
    }

    /// Взрыв ранит и толкает мобов рядом, дальним не вредит, уходящего
    /// крипера не трогает (mob/hostile.rs).
    #[test]
    fn a_blast_hurts_and_pushes_mobs_nearby_but_not_far_ones_or_the_leaving_creeper() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        for (id, x) in [(1, 2.5), (2, 30.5)] {
            items.add_mob(crate::mob::Mob::new(id, crate::mob::Kind::Cow, (x, 64.0, 0.5)));
        }

        let mut leaving = crate::mob::Mob::new(3, crate::mob::Kind::Creeper, (0.5, 64.0, 0.5));

        leaving.h.despawn = true;
        items.add_mob(leaving);

        explode(&mut world, &mut items, &[], (0.5, 64.0, 0.5), 3.0, false, &mut next);

        let by_id = |id: i32| items.mobs().iter().find(|mob| mob.entity_id == id).expect("моб");

        assert!(by_id(1).vitals.health < 10.0 && by_id(1).vx > 0.0, "рядом ранит и толкает от центра");
        assert_eq!(by_id(2).vitals.health, 10.0);
        assert_eq!(by_id(3).vitals.health, 20.0);
    }

    /// Запал убывает на единицу за такт: динамит с запалом 80 взрывается
    /// на восьмидесятом такте, не раньше.
    #[test]
    fn the_fuse_counts_down_to_the_explosion() {
        let mut items = Items::new();
        let loaded = |_: i32, _: i32| true;

        items.prime(1, state("tnt"), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), FUSE, None);

        for tick in 1..FUSE {
            assert!(items.burn_fuses(&loaded).is_empty(), "взорвался на такте {}", tick);
            assert_eq!(items.items()[0].fuse, Some(FUSE - tick));
        }

        let exploded = items.burn_fuses(&loaded);

        assert_eq!(exploded.len(), 1);
        assert!(items.items().is_empty());
    }

    /// Запал клиентам — каждый такт, каким он стал к концу прошлого;
    /// обычные 80 при поджоге не шлются, необычный запал повторяется.
    #[test]
    fn the_fuse_is_told_every_tick() {
        let loaded = |_: i32, _: i32| true;
        let fuses = |items: &mut Items, from: usize| -> Vec<i32> {
            items
                .changes_since(from, READER)
                .0
                .into_iter()
                .filter_map(|change| match change {
                    items::Change::Fuse { fuse, .. } => Some(fuse),
                    _ => None,
                })
                .collect()
        };

        let mut items = Items::new();
        items.prime(1, state("tnt"), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), FUSE, None);
        let (_, start) = items.changes_since(0, READER);

        items.burn_fuses(&loaded);
        items.burn_fuses(&loaded);
        items.burn_fuses(&loaded);
        assert_eq!(fuses(&mut items, start), vec![79, 78]);

        let mut items = Items::new();
        items.prime(1, state("tnt"), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), 15, None);
        let (_, start) = items.changes_since(0, READER);

        items.burn_fuses(&loaded);
        items.burn_fuses(&loaded);
        assert_eq!(fuses(&mut items, start), vec![15, 14]);
    }

    /// Подожжённый динамит не поднять и в блок он не превращается.
    #[test]
    fn primed_tnt_is_neither_picked_up_nor_landed() {
        let mut items = Items::new();
        let floor = |_: i32, y: i32, _: i32| y <= 63;
        let loaded = |_: i32, _: i32| true;

        items.prime(1, state("tnt"), (0.5, 64.0, 0.5), (0.0, 0.2, 0.0), FUSE, None);

        for _ in 0..30 {
            items.step_with_water(&floor, &floor, &loaded, &|_, _, _| None);
        }

        items.note_tick_start();
        assert!(items.take_near((0.5, 64.0, 0.5), 7, |stack| stack.count).is_empty());
        assert!(items.take_landed().is_empty());
        assert_eq!(items.items()[0].y, 64.0, "лёг не на пол");
    }

    /// Запитанный динамит загорается: рычаг рядом — блока нет, в очереди
    /// сущность с запалом 80 над серединой низа блока.
    #[test]
    fn powered_tnt_primes() {
        let mut world = World::in_memory();
        let stone = state("stone");

        for x in -2..=2 {
            for z in -2..=2 {
                world.set_block_silently(x, 63, z, stone);
            }
        }

        world.set_block(0, 64, 0, state("tnt"));
        redstone::settle(&mut world);
        assert!(world.take_primed().is_empty(), "загорелся без сигнала");

        let lever = blocks::state_from_text("lever[face=floor,facing=north,powered=true]").expect("рычаг");

        world.set_block(1, 64, 0, lever);
        redstone::settle(&mut world);

        assert_eq!(world.get_block(0, 64, 0), AIR);

        let primed = world.take_primed();

        assert_eq!(primed.len(), 1);
        let ((x, y, z), (vx, vy, vz), fuse, custom) = primed[0];
        assert_eq!(custom, None);
        assert_eq!((x, y, z), (0.5, 64.0, 0.5));
        assert_eq!(fuse, FUSE);
        assert_eq!(vy, KICK_UP);
        assert!(((vx * vx + vz * vz).sqrt() - KICK_SIDE).abs() < 1e-9);
    }

    /// Поставленный рядом с работающим рычагом динамит загорается сразу.
    #[test]
    fn tnt_placed_next_to_power_primes() {
        let mut world = World::in_memory();
        let lever = blocks::state_from_text("lever[face=floor,facing=north,powered=true]").expect("рычаг");

        world.set_block_silently(0, 63, 0, state("stone"));
        world.set_block_silently(1, 63, 0, state("stone"));
        world.set_block_silently(1, 64, 0, lever);

        world.set_block(0, 64, 0, state("tnt"));
        redstone::settle(&mut world);

        assert_eq!(world.get_block(0, 64, 0), AIR);
        assert_eq!(world.take_primed().len(), 1);
    }

    /// Динамит непроводящий: запитанный динамит соседний не поджигает, а
    /// его поджигает только взрыв — с коротким запалом.
    #[test]
    fn an_explosion_chains_other_tnt() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        items.watch_changes(READER);
        world.set_block_silently(2, 64, 0, state("tnt"));

        explode(&mut world, &mut items, &[], (0.5, 64.06125, 0.5), POWER, false, &mut next);

        assert_eq!(world.get_block(2, 64, 0), AIR);

        let primed: Vec<&Dropped> = items.items().iter().filter(|item| item.fuse.is_some()).collect();

        assert_eq!(primed.len(), 1);
        assert_eq!((primed[0].x, primed[0].y, primed[0].z), (2.5, 64.0, 0.5));

        let fuse = primed[0].fuse.unwrap_or(0);
        assert!((CHAIN_FUSE_MIN..=CHAIN_FUSE_MAX).contains(&fuse), "запал {}", fuse);
    }

    /// Разрушенный динамитом блок выпадает всегда, и камень — булыжником,
    /// как под киркой.
    #[test]
    fn destroyed_blocks_drop() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        world.set_block_silently(0, 63, 0, state("stone"));

        explode(&mut world, &mut items, &[], (0.5, 64.06125, 0.5), POWER, false, &mut next);

        assert_eq!(world.get_block(0, 63, 0), AIR);

        let cobblestone = blocks::item_named("cobblestone");
        assert!(items.items().iter().any(|item| Some(item.stack.item) == cobblestone));
    }

    /// Пакет взрыва уходит после разрушения, а отвалившийся без опоры рычаг
    /// выпадает сразу, до добычи взрыва (чёрный ящик, tnt-powered-place).
    #[test]
    fn the_blast_packet_follows_the_damage() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        world.set_block_silently(0, 63, 0, state("stone"));

        let world_reader = world.watch_events(READER);

        explode(&mut world, &mut items, &[], (0.5, 64.06125, 0.5), POWER, false, &mut next);

        let (events, _) = world.events_since(world_reader, READER);
        let blast = events
            .iter()
            .position(|event| matches!(event, crate::world::WorldEvent::Explosion(_)))
            .expect("пакета взрыва нет");
        let broken = events
            .iter()
            .position(|event| matches!(event, crate::world::WorldEvent::Change(_)))
            .expect("камень не сломан");

        assert!(broken < blast, "пакет взрыва раньше разрушения");

        // Рычаг на камне: камень убрали — рычаг отвалился и выпал сразу.
        let lever = blocks::state_from_text("lever[face=floor,facing=north]").unwrap();

        world.set_block_silently(3, 63, 0, state("stone"));
        world.set_block(3, 64, 0, lever);
        crate::redstone::settle(&mut world);
        world.set_block(3, 63, 0, AIR);
        crate::redstone::settle(&mut world);
        drop_fallen_off(&mut world, &mut items, &mut next);

        let lever_item = blocks::item_named("lever");
        assert_eq!(world.get_block(3, 64, 0), AIR, "рычаг висит без опоры");
        assert!(items.items().iter().any(|item| Some(item.stack.item) == lever_item), "рычаг не выпал");
    }

    /// Одинаковое выпавшее сливается в одну стопку.
    #[test]
    fn the_same_drops_join_into_one_stack() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        for x in -1..=1 {
            world.set_block_silently(x, 63, 0, state("stone"));
        }

        explode(&mut world, &mut items, &[], (0.5, 64.06125, 0.5), POWER, false, &mut next);

        let cobblestone = blocks::item_named("cobblestone");
        let piles: Vec<_> = items.items().iter().filter(|item| Some(item.stack.item) == cobblestone).collect();

        assert_eq!(piles.len(), 1);
        assert_eq!(piles[0].stack.count, 3);
    }

    /// Взрыв толкает прочь: предмет и игрок рядом получают скорость от
    /// центра, игроку она уходит в пакет. Предмет вблизи уничтожается.
    #[test]
    fn an_explosion_pushes_things_away() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        // Падающий блок в трёх блоках к востоку: он не ранится, только летит.
        items.drop_block(5, state("sand"), crate::inventory::Stack::new(0, 0), (3.5, 64.0, 0.5));
        // Предмет рядом с центром — пропадёт.
        items.drop_item(6, crate::inventory::Stack::new(1, 1), (1.0, 64.0, 0.5), (0.0, 0.0, 0.0), 0);

        let players = [Target { entity_id: 9, x: 0.5, y: 64.0, z: 3.5, sneaking: false }];
        let world_reader = world.watch_events(READER);

        explode(&mut world, &mut items, &players, (0.5, 64.06125, 0.5), POWER, false, &mut next);

        let sand = items.items().iter().find(|item| item.entity_id == 5).expect("песок пропал");
        assert!(sand.vx > 0.0 && sand.vy > 0.0, "не толкнуло: {:?}", (sand.vx, sand.vy));
        assert!(items.items().iter().all(|item| item.entity_id != 6), "предмет уцелел");

        let (events, _) = world.events_since(world_reader, READER);
        let blast = events
            .iter()
            .find_map(|event| match event {
                crate::world::WorldEvent::Explosion(blast) => Some(blast.clone()),
                _ => None,
            })
            .expect("пакета взрыва нет");

        assert_eq!(blast.power, POWER);
        let (id, push) = blast.pushed[0];
        assert_eq!(id, 9);
        assert!(push.2 > 0.0 && push.1 > 0.0);
    }

    fn kind(name: &str) -> &'static custom_tnt::Kind {
        custom_tnt::by_name(name).unwrap_or_else(|| panic!("нет вида {}", name))
    }

    /// Особый динамит на месте: поджигается своим видом, со своим запалом.
    #[test]
    fn custom_tnt_ignites_as_its_kind() {
        for (name, fuse) in [("mega", 80), ("nuke", 160), ("cluster", 80), ("safe", 80)] {
            let mut world = World::in_memory();
            let id = kind(name).id;

            world.set_block_silently(0, 64, 0, state("tnt"));
            world.set_custom_tnt((0, 64, 0), Some(id));

            assert!(ignite(&mut world, (0, 64, 0)));
            assert_eq!(world.custom_tnt_at((0, 64, 0)), None, "вид остался на пустом месте");

            let primed = world.take_primed();
            assert_eq!((primed[0].2, primed[0].3), (fuse, Some(id)), "{}", name);
        }
    }

    /// Сила взрыва особого — из таблицы: пакет несёт её, а мега-динамит
    /// в земле рушит дальше обычного.
    #[test]
    fn custom_power_reaches_further() {
        let dirt_cube = || {
            let mut world = World::in_memory();

            for x in -10..=10 {
                for y in -10..=10 {
                    for z in -10..=10 {
                        world.set_block_silently(x, y, z, state("dirt"));
                    }
                }
            }

            world.set_block_silently(0, 0, 0, AIR);
            world
        };
        let far = |world: &World| {
            (-10..=10i32)
                .flat_map(|x| (-10..=10i32).map(move |y| (x, y)))
                .filter(|&(x, y)| x.abs().max(y.abs()) >= 5 && world.get_block(x, y, 0) == AIR)
                .count()
        };

        let mut next = ids();
        let mut items = Items::new();
        let mut plain = dirt_cube();
        explode(&mut plain, &mut items, &[], (0.5, 0.06125, 0.5), POWER, false, &mut next);
        assert_eq!(far(&plain), 0, "обычный дотянулся до пятого блока");

        let mut items = Items::new();
        let mut world = dirt_cube();
        let world_reader = world.watch_events(READER);
        let primed = |custom| {
            let mut items = Items::new();
            items.prime(1, state("tnt"), (0.5, 0.0, 0.5), (0.0, 0.0, 0.0), 1, custom);
            items.burn_fuses(&|_, _| true).remove(0)
        };

        explode_primed(&mut world, &mut items, &[], &primed(Some(kind("mega").id)), &mut next);

        let (events, _) = world.events_since(world_reader, READER);
        let power = events.iter().find_map(|event| match event {
            crate::world::WorldEvent::Explosion(blast) => Some(blast.power),
            _ => None,
        });
        assert_eq!(power, Some(8.0));

        assert!(far(&world) > 0, "мега-динамит не дотянулся до пятого блока");
    }

    /// Безопасный динамит блоков не ломает и ничего не роняет, но толкает
    /// и ранит: предмет рядом пропадает, песок летит.
    #[test]
    fn safe_tnt_breaks_nothing() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        for x in -3..=3 {
            for z in -3..=3 {
                world.set_block_silently(x, 63, z, state("dirt"));
            }
        }
        world.set_block_silently(2, 64, 0, state("tnt"));

        items.drop_block(5, state("sand"), crate::inventory::Stack::new(0, 0), (0.5, 64.0, 3.5));
        items.drop_item(6, crate::inventory::Stack::new(1, 1), (1.0, 64.0, 0.5), (0.0, 0.0, 0.0), 0);

        let mut primed = Items::new();
        primed.prime(1, state("tnt"), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), 1, Some(kind("safe").id));
        let primed = primed.burn_fuses(&|_, _| true).remove(0);

        explode_primed(&mut world, &mut items, &[], &primed, &mut next);

        for x in -3..=3 {
            for z in -3..=3 {
                assert_eq!(world.get_block(x, 63, z), state("dirt"), "сломан {} 63 {}", x, z);
            }
        }
        assert_eq!(world.get_block(2, 64, 0), state("tnt"), "соседний динамит загорелся");
        assert!(items.items().iter().all(|item| item.entity_id != 6), "предмет уцелел");
        assert!(items.items().iter().all(|item| item.fuse.is_none()), "что-то подожглось");
        assert_eq!(items.items().len(), 1, "что-то выпало: {:?}", items.items().len());

        let sand = &items.items()[0];
        assert!(sand.vz > 0.0, "песок не толкнуло");
    }

    /// Кассетный: после взрыва — восемь малых обычных зарядов с запалом
    /// 20–40 тактов, летящих вверх.
    #[test]
    fn cluster_tnt_throws_small_charges() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();

        let mut primed = Items::new();
        primed.prime(1, state("tnt"), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), 1, Some(kind("cluster").id));
        let primed = primed.burn_fuses(&|_, _| true).remove(0);

        explode_primed(&mut world, &mut items, &[], &primed, &mut next);

        let small: Vec<&Dropped> = items.items().iter().filter(|item| item.fuse.is_some()).collect();

        assert_eq!(small.len(), 8);

        for charge in small {
            let fuse = charge.fuse.unwrap_or(0);
            assert!((custom_tnt::CLUSTER_FUSE_MIN..=custom_tnt::CLUSTER_FUSE_MAX).contains(&fuse), "запал {}", fuse);
            assert_eq!(charge.custom, None);
            assert!(charge.vy > 0.0);
        }
    }

    /// Особый динамит, задетый чужим взрывом, загорается своим видом.
    #[test]
    fn a_chained_custom_tnt_keeps_its_kind() {
        let mut world = World::in_memory();
        let mut items = Items::new();
        let mut next = ids();
        let nuke = kind("nuke").id;

        world.set_block_silently(2, 64, 0, state("tnt"));
        world.set_custom_tnt((2, 64, 0), Some(nuke));

        explode(&mut world, &mut items, &[], (0.5, 64.06125, 0.5), POWER, false, &mut next);

        let primed: Vec<&Dropped> = items.items().iter().filter(|item| item.fuse.is_some()).collect();
        assert_eq!(primed.len(), 1);
        assert_eq!(primed[0].custom, Some(nuke));
    }

    /// Ядерный динамит (сила 16) в толще земли и камня укладывается в такт:
    /// замер времени взрыва целиком — лучи, сущности, разрушение, выпадение.
    #[test]
    fn a_nuke_fits_into_a_tick() {
        let (dirt, stone) = (state("dirt"), state("stone"));
        let build = || {
            let mut world = World::in_memory();

            for x in -40..=40 {
                for z in -40..=40 {
                    for y in 20..64 {
                        world.set_block_silently(x, y, z, if y < 50 { stone } else { dirt });
                    }
                }
            }

            world
        };
        let mut world = build();
        let mut items = Items::new();
        let mut next = ids();

        let mut primed = Items::new();
        primed.prime(1, state("tnt"), (0.5, 63.0, 0.5), (0.0, 0.0, 0.0), 1, Some(kind("nuke").id));
        let primed = primed.burn_fuses(&|_, _| true).remove(0);

        // Первый такт: лучи, сущности и первая часть блоков; дальше — по
        // части за такт.
        let started = std::time::Instant::now();
        explode_primed(&mut world, &mut items, &[], &primed, &mut next);
        redstone::settle(&mut world);
        let mut slowest = started.elapsed();
        let first = slowest;
        let mut ticks = 1;

        while world.blast_pending() {
            let started = std::time::Instant::now();
            assert!(continue_blasts(&mut world, &mut items, &mut next));
            redstone::settle(&mut world);
            slowest = slowest.max(started.elapsed());
            ticks += 1;
        }

        assert!(!continue_blasts(&mut world, &mut items, &mut next));

        let broken = (-30..=30)
            .flat_map(|x| (20..64).flat_map(move |y| (-30..=30).map(move |z| (x, y, z))))
            .filter(|&(x, y, z)| world.get_block(x, y, z) == AIR)
            .count();

        println!(
            "ядерный динамит: первый такт {:?}, самый долгий {:?}, тактов {}, сломано блоков {}, предметов {}",
            first,
            slowest,
            ticks,
            broken,
            items.items().len()
        );
        assert!(broken > BLAST_BATCH, "сломано мало: {}", broken);
        assert!(ticks > 1, "всё сломано за один такт");
        assert!(slowest < std::time::Duration::from_millis(if cfg!(debug_assertions) { 2000 } else { 45 }), "такт взрыва занял {:?}", slowest);
    }

    /// Стена заслоняет: за сплошной стеной сущность взрыва не видит.
    #[test]
    fn a_wall_hides_from_the_explosion() {
        let mut world = World::in_memory();
        let obsidian = state("obsidian");

        for y in 60..70 {
            for z in -3..=3 {
                world.set_block_silently(2, y, z, obsidian);
            }
        }

        assert_eq!(exposure(&world, (0.5, 64.06, 0.5), (4.5, 64.0, 0.5), (SIZE, SIZE)), 0.0);
        assert_eq!(exposure(&world, (0.5, 64.06, 0.5), (-2.5, 64.0, 0.5), (SIZE, SIZE)), 1.0);
    }
}
