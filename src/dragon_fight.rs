//! Бой с драконом Края (вики, Ender Dragon; The End; End Crystal; End
//! Gateway). Когда игрок впервые приходит в Край, на столбах стоят
//! кристаллы, а через 20 тактов в (0, 128, 0) появляется дракон. Пока он
//! жив, игроки Края видят полосу босса. Кристаллы лечат его лучом; смерть
//! открывает портал выхода, кладёт яйцо (за первого) и ставит один шлюз
//! (не больше двадцати). Четыре кристалла на портале выхода возрождают
//! дракона. Состояние боя живёт в level.dat Края.

use crate::dimension::Dimension;
use crate::mob::{Kind, Mob};
use crate::shared::Shared;
use crate::world::the_end::{self, CENTER_TOP};
use crate::world::World;
use crate::{blocks, items};

/// Где появляется дракон (вики, Ender Dragon: «spawns at (0, 128, 0)»).
pub const DRAGON_SPAWN: (f64, f64, f64) = (0.0, 128.0, 0.0);

/// Через сколько тактов после прихода игрока появляется дракон (вики,
/// Ender Dragon: «spawns 20 ticks after»).
const SPAWN_DELAY: u64 = 20;

/// Сколько тактов занимает возрождение по четырём кристаллам (вики, The End:
/// «604 ticks»). Разбивка по стадиям — наша [проверить].
pub const RESPAWN_TICKS: u64 = 604;

/// Шлюзов вокруг острова (вики, End Gateway), радиус и высота.
pub const GATEWAYS: usize = 20;
const GATEWAY_RADIUS: f64 = 96.0;
const GATEWAY_Y: i32 = 75;

/// Состояние боя — в level.dat Края.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fight {
    /// Игрок уже приходил: кристаллы расставлены, дракон вызван.
    pub started: bool,

    /// Дракон побеждён — в Крае его нет.
    pub dragon_killed: bool,

    /// Дракона уже побеждали: следующие дают 500 опыта и без яйца.
    pub previously_killed: bool,

    /// Сделанные шлюзы: номера мест 0…19.
    pub gateways: Vec<u8>,

    /// Такт мира, на котором появится дракон.
    pub spawn_due: Option<u64>,

    /// Возрождение: такт мира, на котором оно началось.
    pub respawn_from: Option<u64>,

    /// Последняя разосланная доля здоровья полосы босса (None — полосы нет).
    pub boss: Option<f32>,
}

impl Fight {
    /// Строка для level.dat: `dragon-fight = started killed previously`.
    pub fn save(&self) -> String {
        let mut text = format!(
            "dragon-fight = {} {} {}\n",
            u8::from(self.started),
            u8::from(self.dragon_killed),
            u8::from(self.previously_killed)
        );

        if !self.gateways.is_empty() {
            text.push_str("gateways =");

            for gateway in &self.gateways {
                text.push_str(&format!(" {gateway}"));
            }

            text.push('\n');
        }

        text
    }

    /// Разбор строки `dragon-fight`.
    pub fn read_flags(&mut self, value: &str) {
        let mut parts = value.split_whitespace().map(|part| part == "1");

        self.started = parts.next().unwrap_or(false);
        self.dragon_killed = parts.next().unwrap_or(false);
        self.previously_killed = parts.next().unwrap_or(false);
    }

    /// Разбор строки `gateways`.
    pub fn read_gateways(&mut self, value: &str) {
        self.gateways = value.split_whitespace().filter_map(|part| part.parse::<u8>().ok()).filter(|n| usize::from(*n) < GATEWAYS).collect();
    }
}

/// Место шлюза номер `k` (0…19): `x = ⌊96·cos(πk/10)⌋`, `z = ⌊96·sin(πk/10)⌋`,
/// y = 75 (вики, End Gateway).
pub fn gateway_place(k: usize) -> (i32, i32, i32) {
    let angle = std::f64::consts::PI * (k as f64 + 1.0) / 10.0;

    ((GATEWAY_RADIUS * angle.cos()).floor() as i32, GATEWAY_Y, (GATEWAY_RADIUS * angle.sin()).floor() as i32)
}

/// Порядок шлюзов: «depends on the seed» (вики) — перемешиваем от семени
/// [проверить: у оригинала свой порядок].
pub fn gateway_order(seed: i64) -> [u8; GATEWAYS] {
    let mut order: [u8; GATEWAYS] = std::array::from_fn(|i| i as u8);
    let mut state = (seed as u64) ^ 0x6A7E_9A7E;

    for at in (1..GATEWAYS).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        order.swap(at, (state % (at as u64 + 1)) as usize);
    }

    order
}

/// Блоки шлюза: сам шлюз и 12 бедрока «vaguely octahedral» (вики): по
/// одному над и под через блок, по кресту из пяти над и под [проверить].
pub fn gateway_blocks(at: (i32, i32, i32)) -> Vec<((i32, i32, i32), &'static str)> {
    let (x, y, z) = at;
    let mut out = vec![((x, y, z), "end_gateway"), ((x, y + 2, z), "bedrock"), ((x, y - 2, z), "bedrock")];

    for dy in [-1, 1] {
        for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            out.push(((x + dx, y + dy, z + dz), "bedrock"));
        }
    }

    out
}

/// Клетки четырёх кристаллов возрождения: на бедроке по сторонам чаши
/// портала выхода (вики, End Crystal: «on each side of the exit portal»).
pub fn respawn_cells() -> [(i32, i32, i32); 4] {
    [(3, CENTER_TOP + 1, 0), (-3, CENTER_TOP + 1, 0), (0, CENTER_TOP + 1, 3), (0, CENTER_TOP + 1, -3)]
}

/// Блоки, которые дракон телом не ломает (вики, Ender Dragon: «cannot
/// destroy»).
fn resistant(name: &str) -> bool {
    matches!(
        name,
        "air"
            | "barrier"
            | "bedrock"
            | "command_block"
            | "chain_command_block"
            | "repeating_command_block"
            | "crying_obsidian"
            | "end_stone"
            | "end_portal"
            | "end_portal_frame"
            | "end_gateway"
            | "fire"
            | "soul_fire"
            | "iron_bars"
            | "jigsaw"
            | "light"
            | "moving_piston"
            | "obsidian"
            | "reinforced_deepslate"
            | "respawn_anchor"
            | "structure_block"
            | "structure_void"
            | "test_block"
            | "test_instance_block"
            | "end_crystal"
    )
}

/// Кристалл на вершине столба: стоит на бедроке, в клетке огня.
fn crystal_on(spike: &the_end::Spike, next_id: &mut dyn FnMut() -> i32) -> Mob {
    Mob::new(next_id(), Kind::EndCrystal, (f64::from(spike.x) + 0.5, f64::from(spike.height + 1), f64::from(spike.z) + 0.5))
}

/// Ставит кристаллы на столбы, где их нет.
fn restore_crystals(world: &World, items: &mut items::Items, next_id: &mut dyn FnMut() -> i32) {
    for spike in the_end::spikes(world.seed()) {
        let has = items.mobs().iter().any(|mob| mob.kind == Kind::EndCrystal && mob.cell() == (spike.x, spike.height + 1, spike.z));

        if !has {
            items.add_mob_late(crystal_on(&spike, next_id));
        }
    }
}

/// Новый дракон боя.
fn new_dragon(first: bool, next_id: &mut dyn FnMut() -> i32) -> Mob {
    let mut mob = Mob::new(next_id(), Kind::EnderDragon, DRAGON_SPAWN);

    for _ in 0..crate::mob::DRAGON_PARTS {
        next_id();
    }

    mob.dragon.fighting = true;
    mob.first_dragon = first;
    mob
}

/// Такт боя: вызывается после хода мобов Края.
pub fn tick(shared: &Shared, world: &mut World, players_here: usize, ticks: u64) {
    let mut items = shared.items_in(Dimension::End).lock().expect("предметы захвачены другим потоком");
    let mut next_id = || shared.next_entity_id();

    // Первый приход: кристаллы на столбы, дракон через 20 тактов.
    if !world.fight.started && players_here > 0 {
        world.fight.started = true;
        world.fight.spawn_due = Some(ticks + SPAWN_DELAY);
        restore_crystals(world, &mut items, &mut next_id);
    }

    if world.fight.spawn_due.is_some_and(|due| ticks >= due) {
        world.fight.spawn_due = None;

        if !world.fight.dragon_killed && !items.mobs().iter().any(|mob| mob.kind == Kind::EnderDragon) {
            let first = !world.fight.previously_killed;

            items.add_mob_late(new_dragon(first, &mut next_id));
        }
    }

    // Дракон, пришедший с чанком, снова в бою.
    for mob in items.mobs_mut().iter_mut().filter(|mob| mob.kind == Kind::EnderDragon && !mob.no_ai) {
        mob.dragon.fighting = true;
    }

    let crystals = items.mobs().iter().filter(|mob| mob.kind == Kind::EndCrystal).count() as u8;
    let dragon = items.mobs().iter().find(|mob| mob.kind == Kind::EnderDragon).map(|mob| (mob.entity_id, mob.healer, mob.cell(), mob.vitals.health, mob.is_dead(), mob.dead_for, (mob.x, mob.y, mob.z), mob.first_dragon));

    // Лучи кристаллов — в дракона, которого они лечат.
    if let Some((dragon_id, healer, cell, ..)) = dragon {
        for crystal in items.mobs_mut().iter_mut().filter(|mob| mob.kind == Kind::EndCrystal) {
            let beam = (crystal.entity_id == healer).then_some(cell);

            crystal.beam_target = beam;
        }

        for mob in items.mobs_mut().iter_mut().filter(|mob| mob.entity_id == dragon_id) {
            mob.dragon.crystals = crystals;
        }
    }

    match dragon {
        Some((_, _, _, health, false, _, at, _)) => {
            // Полоса босса: доля здоровья, пока она меняется.
            let share = health / Kind::EnderDragon.max_health();

            if world.fight.boss != Some(share) {
                world.fight.boss = Some(share);
                world.note_boss_bar(Some(share));
            }

            drop(items);
            crush_blocks(world, at);
        }
        Some((_, _, _, _, true, dead_for, at, first)) => {
            // Смерть: портал выхода, яйцо, шлюз — на 200-м такте (вики).
            if world.fight.boss.is_some() {
                world.fight.boss = None;
                world.note_boss_bar(None);
            }

            if dead_for == crate::mob::DRAGON_DEATH_TICKS {
                drop(items);
                world.fight.dragon_killed = true;
                world.fight.previously_killed = true;
                world.note_level_event(at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32, 1028, 0);
                open_exit_portal(world, first);
                spawn_gateway(world);
            }
        }
        None => {
            if world.fight.boss.is_some() {
                world.fight.boss = None;
                world.note_boss_bar(None);
            }

            if world.fight.dragon_killed {
                respawn_step(world, &mut items, &mut next_id, ticks);
            }
        }
    }
}

/// Открывает портал выхода: порталы Края в чаше, яйцо над столбом (только
/// за первого дракона — вики, Dragon Egg).
fn open_exit_portal(world: &mut World, first: bool) {
    if let Some(portal) = blocks::state_by_name("end_portal") {
        for (x, y, z) in the_end::exit_portal_blocks() {
            world.set_block(x, y, z, portal);
        }
    }

    if first && let Some(egg) = blocks::state_by_name("dragon_egg") {
        let top = (CENTER_TOP + 1..=CENTER_TOP + 5).rev().find(|y| world.get_block(0, *y, 0) != 0).unwrap_or(CENTER_TOP + 3);

        world.set_block(0, top + 1, 0, egg);
    }
}

/// Ставит следующий шлюз из порядка семени, пока их меньше двадцати.
fn spawn_gateway(world: &mut World) {
    let order = gateway_order(world.seed());
    let Some(next) = order.iter().copied().find(|k| !world.fight.gateways.contains(k)) else {
        return;
    };

    world.fight.gateways.push(next);

    for ((x, y, z), name) in gateway_blocks(gateway_place(usize::from(next))) {
        if let Some(state) = blocks::state_by_name(name) {
            world.set_block(x, y, z, state);
        }
    }
}

/// Тело дракона ломает блоки, кроме стойких к Краю [проверить: у оригинала
/// — по ящикам частей; у нас — короб 9×5×9 вокруг тела].
fn crush_blocks(world: &mut World, at: (f64, f64, f64)) {
    let (cx, cy, cz) = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);

    for x in cx - 4..=cx + 4 {
        for y in cy..=cy + 4 {
            for z in cz - 4..=cz + 4 {
                let state = world.get_block(x, y, z);

                if state != 0 && !blocks::block_at_state(state).is_some_and(resistant) {
                    world.set_block(x, y, z, 0);
                }
            }
        }
    }
}

/// Возрождение: четыре кристалла на портале выхода. Лучи сходятся в
/// (0, 128, 0); через 604 такта кристаллы на столбах восстановлены,
/// дракон появляется, четыре кристалла исчезают (вики) [проверить: стадии].
fn respawn_step(world: &mut World, items: &mut items::Items, next_id: &mut dyn FnMut() -> i32, ticks: u64) {
    let cells = respawn_cells();
    let mut placed: Vec<i32> = items.mobs().iter().filter(|mob| mob.kind == Kind::EndCrystal && cells.contains(&mob.cell())).map(|mob| mob.entity_id).collect();
    placed.sort_unstable();
    placed.dedup();

    if placed.len() < 4 {
        world.fight.respawn_from = None;
        return;
    }

    let from = *world.fight.respawn_from.get_or_insert(ticks);
    let beam = Some((0, 128, 0));

    for crystal in items.mobs_mut().iter_mut().filter(|mob| placed.contains(&mob.entity_id)) {
        if crystal.beam_target != beam {
            crystal.beam_target = beam;
        }
    }

    if ticks < from + RESPAWN_TICKS {
        return;
    }

    world.fight.respawn_from = None;
    world.fight.dragon_killed = false;
    restore_crystals(world, items, next_id);

    for crystal in items.remove_mobs(|mob| placed.contains(&mob.entity_id)) {
        world.note_level_event(crystal.x.floor() as i32, crystal.y.floor() as i32, crystal.z.floor() as i32, 2001, 0);
    }

    items.add_mob_late(new_dragon(false, next_id));
    // Вики, Advancement: рядом с призванным драконом [проверить: радиус].
    world.note_progress_near("end/respawn_dragon", (0.0, 128.0, 0.0), (192.0, 192.0, 192.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_places_follow_the_wiki_formula() {
        assert_eq!(gateway_place(0), (91, 75, 29));
        assert_eq!(gateway_place(4), (0, 75, 96));
        assert_eq!(gateway_place(19), (96, 75, -1));
    }

    #[test]
    fn gateway_order_is_a_permutation() {
        let mut order = gateway_order(42).to_vec();
        order.sort_unstable();
        assert_eq!(order, (0..20).collect::<Vec<u8>>());
        assert_ne!(gateway_order(1), gateway_order(2));
    }

    #[test]
    fn gateway_has_twelve_bedrock() {
        let blocks = gateway_blocks((91, 75, 29));
        assert_eq!(blocks.iter().filter(|(_, name)| *name == "bedrock").count(), 12);
        assert_eq!(blocks.iter().filter(|(_, name)| *name == "end_gateway").count(), 1);
    }

    #[test]
    fn fight_state_round_trips() {
        let fight = Fight { started: true, dragon_killed: true, previously_killed: true, gateways: vec![3, 7], ..Fight::default() };
        let mut back = Fight::default();

        for line in fight.save().lines() {
            let (name, value) = line.split_once('=').expect("строка с =");

            match name.trim() {
                "dragon-fight" => back.read_flags(value),
                "gateways" => back.read_gateways(value),
                _ => panic!("лишняя строка"),
            }
        }

        assert_eq!(back, fight);
    }

    #[test]
    fn end_stone_and_obsidian_resist() {
        assert!(resistant("end_stone"));
        assert!(resistant("obsidian"));
        assert!(!resistant("stone"));
    }
}
