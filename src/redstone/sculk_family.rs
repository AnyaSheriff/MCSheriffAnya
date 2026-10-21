// Крикуны, катализаторы и общая доставка вибраций хранителю.
// minecraft.wiki/w/Sculk_Shrieker, /Sculk_Catalyst, /Vibration (до 26.1.2).
// [проверить] тактовый порядок и случайное распространение: без чёрного ящика.

use std::collections::{BTreeMap, BTreeSet};
use super::{Block, Pos, Dir, NEIGHBOUR_ORDER, step, set};
use crate::{blocks, world::World};

pub type Point = (f64, f64, f64);

#[derive(Clone, Copy, Debug)]
pub struct DeathCharge { pub catalyst: Pos, pub at: Point, pub amount: i32 }

#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub id: i32,
    pub uuid: [u8; 16],
    pub at: Point,
    pub survival: bool,
    pub sneaking: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Warning {
    pub level: u8,
    pub cooldown: u32,
    pub quiet: u32,
}

impl Warning {
    pub fn tick(&mut self) {
        self.cooldown = self.cooldown.saturating_sub(1);
        self.quiet += 1;
        if self.quiet >= 12000 {
            self.level = self.level.saturating_sub(1);
            self.quiet = 0;
        }
    }
}

/// Отдельный маленький файл сохраняет предупреждение независимо от измерения.
/// MCPD не меняется: старые записи игроков остаются читаемыми.
pub fn load_warning(directory: &std::path::Path, uuid: &[u8; 16]) -> Warning {
    let path = crate::playerdata::path_for(directory, uuid).with_extension("sculk");
    let Ok(bytes) = std::fs::read(path) else { return Warning::default(); };
    if bytes.len() != 9 { return Warning::default(); }
    Warning { level: bytes[0].min(4), cooldown: u32::from_be_bytes(bytes[1..5].try_into().unwrap()).min(200),
        quiet: u32::from_be_bytes(bytes[5..9].try_into().unwrap()).min(11999) }
}

pub fn save_warning(directory: &std::path::Path, uuid: &[u8; 16], warning: Warning) -> std::io::Result<()> {
    let path = crate::playerdata::path_for(directory, uuid).with_extension("sculk");
    let mut bytes = vec![warning.level];
    bytes.extend_from_slice(&warning.cooldown.to_be_bytes());
    bytes.extend_from_slice(&warning.quiet.to_be_bytes());
    let temporary = path.with_extension("sculk-new");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)
}

#[derive(Clone, Copy, Debug)]
pub struct GameEvent {
    pub at: Point,
    pub source: Option<i32>,
    pub frequency: u8,
    pub projectile: bool,
}

#[derive(Clone, Copy, Debug)]
struct Signal { source: i32, due: u64 }

#[derive(Clone, Copy, Debug)]
struct Charge { at: Pos, catalyst: Pos, amount: i32, age: u32 }

#[derive(Default)]
pub struct Memory {
    pub players: Vec<Player>,
    pub wardens: Vec<Point>,
    pub footsteps: BTreeMap<i32, (Point, f64, bool)>,
    pub warnings: BTreeMap<[u8; 16], Warning>,
    pub events: Vec<GameEvent>,
    pub summons: Vec<Pos>,
    pub enabled: bool,
    pub catalysts: BTreeSet<Pos>,
    shriekers: BTreeSet<Pos>,
    signals: BTreeMap<Pos, Signal>,
    shrieks: BTreeMap<Pos, u8>,
    charges: Vec<Charge>,
}

impl Memory {
    pub fn note_state(&mut self, pos: Pos, state: i32) {
        if blocks::block_at_state(state) == Some("sculk_shrieker") {
            self.shriekers.insert(pos);
        } else {
            self.shriekers.remove(&pos);
            self.signals.remove(&pos);
            self.shrieks.remove(&pos);
        }
        if blocks::block_at_state(state) == Some("sculk_catalyst") {
            self.catalysts.insert(pos);
        } else { self.catalysts.remove(&pos); }
    }
}

pub fn center(pos: Pos) -> Point { (f64::from(pos.0) + 0.5, f64::from(pos.1) + 0.5, f64::from(pos.2) + 0.5) }
fn distance(a: Point, b: Point) -> f64 { crate::mob::warden::distance(a, b) }

pub fn dampening(state: i32) -> bool {
    blocks::block_at_state(state).is_some_and(|name| name.ends_with("_wool"))
}

/// Вики, Vibration: «wool and wool carpets»; моховой ковёр не глушит.
pub fn quiet_block(name: &str) -> bool {
    name.ends_with("_wool") || (name.ends_with("_carpet") && !name.ends_with("moss_carpet"))
}

/// Только испускание событий установки/разрушения. Шаги, приземления и
/// перекрытие луча здесь не определяются: для плит это отдельные наблюдения.
pub fn quiet_state(catalog: crate::gameplay_catalog::Catalog, state: i32) -> bool {
    if let Ok(Some(row)) = catalog.slab_entry(state) {
        return row.family == crate::gameplay_catalog::SlabFamily::Wool;
    }
    catalog.name(crate::gameplay_catalog::Kind::State, state).ok().is_some_and(quiet_block)
}

/// Шерсть перекрывает луч, ковёр только гасит шаги. Шесть лучей от малых
/// смещений источника устраняют ложное перекрытие на границе блока.
/// [проверить] точность на рёбрах, совпадающих с линией слушателя.
pub fn occluded(world: &World, from: Point, to: Point) -> bool {
    const EPS: f64 = 1.0e-5;
    [(EPS, 0.0, 0.0), (-EPS, 0.0, 0.0), (0.0, EPS, 0.0), (0.0, -EPS, 0.0), (0.0, 0.0, EPS), (0.0, 0.0, -EPS)]
        .into_iter().all(|offset| wool_ray(world, (from.0 + offset.0, from.1 + offset.1, from.2 + offset.2), to))
}

fn wool_ray(world: &World, from: Point, to: Point) -> bool {
    let start = [from.0, from.1, from.2];
    let delta = [to.0 - from.0, to.1 - from.1, to.2 - from.2];
    let mut cell = start.map(|n| n.floor() as i32);
    let mut next = [f64::INFINITY; 3];
    let mut stride = [f64::INFINITY; 3];
    let mut direction = [0; 3];
    for axis in 0..3 {
        if delta[axis] != 0.0 {
            direction[axis] = if delta[axis] > 0.0 { 1 } else { -1 };
            stride[axis] = 1.0 / delta[axis].abs();
            let boundary = f64::from(cell[axis] + i32::from(direction[axis] > 0));
            next[axis] = (boundary - start[axis]) / delta[axis];
        }
    }
    for _ in 0..128 {
        if dampening(world.get_block(cell[0], cell[1], cell[2])) { return true; }
        let axis = (0..3).min_by(|a, b| next[*a].total_cmp(&next[*b])).unwrap();
        if next[axis] > 1.0 { return false; }
        cell[axis] += direction[axis];
        next[axis] += stride[axis];
    }
    false
}

/// «warning level ... all players within 16 blocks»: общий максимум,
/// затем +1 всем участникам. Откат любого из них запрещает предупреждение.
pub fn warn(world: &mut World, pos: Pos, source: i32) -> bool {
    let memory = &mut world.redstone.sculk;
    if !memory.enabled || !memory.players.iter().any(|p| p.id == source && p.survival) { return false; }
    if memory.wardens.iter().any(|p| (p.0 - center(pos).0).abs() <= 48.0 && (p.1 - center(pos).1).abs() <= 48.0 && (p.2 - center(pos).2).abs() <= 48.0) { return false; }
    let participants: Vec<_> = memory.players.iter().filter(|p| p.survival && (p.id == source || distance(p.at, center(pos)) <= 16.0)).map(|p| p.uuid).collect();
    if participants.iter().any(|id| memory.warnings.get(id).is_some_and(|w| w.cooldown > 0)) { return false; }
    let level = (participants.iter().filter_map(|id| memory.warnings.get(id)).map(|w| w.level).max().unwrap_or(0) + 1).min(4);
    for id in participants { memory.warnings.insert(id, Warning { level, cooldown: 200, quiet: 0 }); }
    memory.shrieks.insert(pos, level);
    true
}

pub fn finish_shriek(world: &mut World, pos: Pos) {
    let Some(level) = world.redstone.sculk.shrieks.remove(&pos) else { return; };
    let players: Vec<_> = world.redstone.sculk.players.iter().filter(|p| p.survival && distance(p.at, center(pos)) <= 40.0).map(|p| p.id).collect();
    for id in players { darkness(world, id); }
    if level == 4 { world.redstone.sculk.summons.push(pos); }
    else {
        // Сценарий shrieker-warning: предупреждение звучит в случайной точке
        // в ±10 блоках от крикуна (целые координаты) [проверить: у третьего
        // предупреждения оба замера лежат ближе, (1,1,-4) и (1,1,1)].
        let name = match level { 1 => "minecraft:entity.warden.nearby_close", 2 => "minecraft:entity.warden.nearby_closer", _ => "minecraft:entity.warden.nearby_closest" };
        let mut offset = || (world.random_unit() * 21.0).floor() as i32 - 10;
        let (dx, dy, dz) = (offset(), offset(), offset());
        world.note_sound_at((f64::from(pos.0 + dx), f64::from(pos.1 + dy), f64::from(pos.2 + dz)), name, 5.0, 1.0);
    }
}

pub fn darkness(world: &mut World, player: i32) {
    world.note_mob_effect(player, crate::survival::Effect { kind: crate::survival::EffectKind::Darkness,
        amplifier: 0, ticks_left: 260, ambient: false, visible: false, show_icon: false });
}

pub fn tick(world: &mut World) {
    let now = world.tick();
    let ready: Vec<_> = world.redstone.sculk.signals.iter().filter(|(_, signal)| signal.due <= now).map(|(p, signal)| (*p, signal.source)).collect();
    for (pos, source) in ready {
        world.redstone.sculk.signals.remove(&pos);
        super::sculk::shriek_from(world, pos, source);
    }
    spread(world);
}

/// Смерть даёт заряд и без участия игрока, но лишь одному ближайшему
/// катализатору в радиусе 8 (вики, Sculk Catalyst). Бюджет — опыт жертвы.
pub fn feed(world: &mut World, catalyst: Pos, at: Point, amount: i32) {
    if amount <= 0 || Block::at(world, catalyst).name != "sculk_catalyst" { return; }
    let block = Block::at(world, catalyst);
    if let Some(state) = block.with("bloom", "true") { set(world, catalyst, state); }
    world.schedule_block(catalyst.0, catalyst.1, catalyst.2, 8, super::PRIORITY_NORMAL);
    let pitch = 0.6 + world.random_unit() * 0.4;
    world.note_sound(catalyst.0, catalyst.1, catalyst.2, "minecraft:block.sculk_catalyst.bloom", 2.0, pitch);
    // minecraft-data 26.1: sculk_soul=37 [проверить] на клиенте.
    world.note_particle(37, None, center(catalyst), (0.2, 0.0, 0.2), 0.0, 2);
    let at = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);
    // Ограничение числа курсоров предотвращает неограниченную работу за такт.
    if world.redstone.sculk.charges.len() < 32 { world.redstone.sculk.charges.push(Charge { at, catalyst, amount: amount.min(1000), age: 0 }); }
}

pub(super) fn catalyst_ticked(world: &mut World, pos: Pos, block: &Block) {
    if let Some(state) = block.with("bloom", "false") { set(world, pos, state); }
}

fn replaceable(name: &str) -> bool {
    matches!(name, "stone" | "granite" | "diorite" | "andesite" | "deepslate" | "tuff" | "calcite" | "dirt" | "grass_block" | "podzol" | "coarse_dirt" | "rooted_dirt" | "mycelium" | "moss_block" | "mud" | "clay" | "gravel" | "sand" | "red_sand" | "terracotta" | "netherrack" | "soul_sand" | "soul_soil" | "basalt" | "smooth_basalt" | "blackstone" | "end_stone")
}

/// [проверить] курсорный случайный обход — приближение; вероятности распада,
/// углы обхода жилок и плотность сенсоров требуют измерений ванили.
fn spread(world: &mut World) {
    let mut charges = std::mem::take(&mut world.redstone.sculk.charges);
    for charge in &mut charges {
        if !world.has_chunk(charge.at.0.div_euclid(16), charge.at.2.div_euclid(16)) { continue; }
        charge.age += 1;
        if distance(center(charge.at), center(charge.catalyst)) > 24.0 || charge.age > 600 { charge.amount = 0; continue; }
        let start = (world.random_unit() * 6.0) as usize % 6;
        let mut moved = false;
        for i in 0..6 {
            let dir = NEIGHBOUR_ORDER[(start + i) % 6];
            let next = step(charge.at, dir);
            let block = Block::at(world, next);
            if replaceable(block.name) && NEIGHBOUR_ORDER.iter().any(|dir| {
                matches!(Block::at(world, step(next, *dir)).name, "air" | "cave_air" | "water" | "sculk_vein")
            }) {
                if let Some(sculk) = blocks::state_by_name("sculk") { set(world, next, sculk); }
                charge.amount -= 1;
                charge.at = next;
                moved = true;
                break;
            }
            if matches!(block.name, "sculk" | "sculk_vein") {
                charge.at = next;
                moved = true;
                break;
            }
        }
        // Жилка покрывает открытые грани твёрдого скалка; сохраняет воду.
        if Block::at(world, charge.at).name == "sculk" {
            for dir in NEIGHBOUR_ORDER {
                let pos = step(charge.at, dir);
                let block = Block::at(world, pos);
                if !matches!(block.name, "air" | "cave_air" | "water" | "sculk_vein") { continue; }
                let state = if block.name == "sculk_vein" { block.state } else { blocks::state_by_name("sculk_vein").unwrap_or(0) };
                let face = match dir { Dir::Up => "down", Dir::Down => "up", Dir::North => "south", Dir::South => "north", Dir::East => "west", Dir::West => "east" };
                if let Some(rule) = blocks::orientation_of(state)
                    && let Some(state) = rule.with(state, face, "true")
                    && let Some(state) = rule.with(state, "waterlogged", if block.name == "water" || block.is("waterlogged", "true") { "true" } else { "false" })
                { set(world, pos, state); }
            }
        }
        if !moved || charge.age.is_multiple_of(20) { charge.amount -= 1; }
        if charge.amount <= 0 && distance(center(charge.at), center(charge.catalyst)) > 4.0
            && Block::at(world, charge.at).name == "sculk"
            && matches!(Block::at(world, step(charge.at, Dir::Up)).name, "air" | "cave_air")
            && world.random_unit() < 0.1
        {
            let name = if world.random_unit() < 0.1 { "sculk_shrieker" } else { "sculk_sensor" };
            if let Some(state) = blocks::state_by_name(name) { set(world, step(charge.at, Dir::Up), state); }
        }
    }
    charges.retain(|charge| charge.amount > 0);
    world.redstone.sculk.charges = charges;
}

/// Вики, Sculk/Sculk Vein/Sculk Catalyst/Sculk Shrieker: добыча только
/// «Silk Touch»; у жилки по предмету за прикреплённую грань.
pub fn mined_drop(state: i32, silk: bool) -> Option<Option<crate::inventory::Stack>> {
    let name = blocks::block_at_state(state)?;
    if !matches!(name, "sculk" | "sculk_vein" | "sculk_catalyst" | "sculk_shrieker" | "sculk_sensor") { return None; }
    let count = if name == "sculk_vein" {
        let rule = blocks::orientation(name)?;
        ["up", "down", "north", "south", "east", "west"].iter().filter(|face| rule.value(state, face) == Some("true")).count() as i32
    } else { 1 };
    Some(if silk && count > 0 { blocks::item_named(name).map(|item| crate::inventory::Stack::new(item, count)) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warning_decays_after_ten_minutes() {
        let mut warning = Warning { level: 4, cooldown: 200, quiet: 0 };
        for _ in 0..11999 { warning.tick(); }
        assert_eq!(warning.level, 4);
        assert_eq!(warning.cooldown, 0);
        warning.tick();
        assert_eq!(warning.level, 3);
        for _ in 0..48000 { warning.tick(); }
        assert_eq!(warning.level, 0);
    }

    fn player(id: u8, x: f64) -> Player {
        Player { id: i32::from(id), uuid: [id; 16], at: (x, 1.5, 0.5), survival: true, sneaking: false }
    }

    fn test_world() -> World {
        let mut world = World::in_memory();
        for x in -2..=10 { world.set_block_silently(x, 0, 0, blocks::state_by_name("stone").unwrap()); }
        world.redstone.sculk.enabled = true;
        world.redstone.sculk.players = vec![player(1, 0.5), player(2, 10.5), player(3, 30.5)];
        world
    }

    fn shrieker(world: &mut World, pos: Pos, summon: bool) {
        let state = blocks::orientation("sculk_shrieker").unwrap().state(&[("can_summon", if summon { "true" } else { "false" }), ("shrieking", "false"), ("waterlogged", "false")]).unwrap();
        world.set_block(pos.0, pos.1, pos.2, state);
    }

    #[test]
    fn warnings_share_maximum_and_enforce_group_cooldown() {
        let mut world = test_world();
        let pos = (0, 1, 0);
        world.redstone.sculk.warnings.insert([2; 16], Warning { level: 2, ..Warning::default() });
        assert!(warn(&mut world, pos, 1));
        assert_eq!(world.redstone.sculk.warnings[&[1; 16]].level, 3);
        assert_eq!(world.redstone.sculk.warnings[&[2; 16]].level, 3);
        assert!(!world.redstone.sculk.warnings.contains_key(&[3; 16]));
        assert!(!warn(&mut world, (10, 1, 0), 2));
        for _ in 0..200 { for warning in world.redstone.sculk.warnings.values_mut() { warning.tick(); } }
        assert!(warn(&mut world, pos, 1));
        assert_eq!(world.redstone.sculk.warnings[&[1; 16]].level, 4);
        finish_shriek(&mut world, pos);
        assert_eq!(world.redstone.sculk.summons, vec![pos]);
    }

    #[test]
    fn cosmetic_shrieker_and_mobs_do_not_warn() {
        let mut world = test_world();
        shrieker(&mut world, (0, 1, 0), false);
        assert!(super::super::sculk::shriek_from(&mut world, (0, 1, 0), 1));
        assert!(world.redstone.sculk.warnings.is_empty());
        shrieker(&mut world, (0, 1, 0), true);
        let mob = super::super::components::Occupant::mob(0.5, 1.5, 0.5, 0.6, 1.8, true);
        super::super::sculk::step_on_shriekers(&mut world, &[mob]);
        assert_eq!(Block::at(&world, (0, 1, 0)).value("shrieking"), Some("false"));
        assert!(world.redstone.sculk.warnings.is_empty());
    }

    #[test]
    fn peaceful_creative_and_nearby_warden_prevent_warning() {
        let mut world = test_world();
        world.redstone.sculk.enabled = false;
        assert!(!warn(&mut world, (0, 1, 0), 1));
        world.redstone.sculk.enabled = true;
        world.redstone.sculk.players[0].survival = false;
        assert!(!warn(&mut world, (0, 1, 0), 1));
        world.redstone.sculk.players[0].survival = true;
        world.redstone.sculk.wardens.push((48.5, 1.5, 0.5));
        assert!(!warn(&mut world, (0, 1, 0), 1));
        assert!(world.redstone.sculk.warnings.is_empty());
    }

    #[test]
    fn wool_occludes_but_stone_and_carpet_do_not() {
        let mut world = test_world();
        assert!(quiet_block("white_wool"));
        assert!(quiet_block("white_carpet"));
        assert!(!quiet_block("moss_carpet"));
        assert!(!quiet_block("pale_moss_carpet"));
        for (name, blocked) in [("stone", false), ("white_wool", true), ("white_carpet", false)] {
            world.set_block(2, 1, 0, blocks::state_by_name(name).unwrap());
            assert_eq!(occluded(&world, (0.5, 1.5, 0.5), (4.5, 1.5, 0.5)), blocked, "{name}");
        }
    }

    #[test]
    fn block_change_quiet_state_requires_enabled_wool_family_and_preserves_native_names() {
        use crate::gameplay_catalog::{Catalog, Profile, SlabFamily, slabs};
        let base = Catalog::new(Profile::Java26_1_2);
        let preview = Catalog::new(Profile::Preview26_3);
        for row in slabs() {
            for state in row.first_state..=row.last_state {
                assert_eq!(quiet_state(preview, state), row.family == SlabFamily::Wool);
                assert!(!quiet_state(base, state), "выключенный extension не получает правила шерсти");
            }
        }
        for catalog in [base, preview] {
            for name in ["white_wool", "black_wool", "white_carpet", "stone", "moss_carpet", "pale_moss_carpet"] {
                assert_eq!(quiet_state(catalog, blocks::state_by_name(name).unwrap()), quiet_block(name));
            }
            for state in [-1, i32::MAX] { assert!(!quiet_state(catalog, state)); }
        }
    }

    #[test]
    fn container_lid_events_keep_player_source() {
        let mut world = test_world();
        world.set_block(0, 1, 0, blocks::state_by_name("chest").unwrap());
        world.actor = Some(1);
        assert!(crate::containers::open_begin(&mut world, &[(0, 1, 0)]));
        assert!(!crate::containers::open_begin(&mut world, &[(0, 1, 0)]));
        crate::containers::close(&mut world, &[(0, 1, 0)]);
        crate::containers::close(&mut world, &[(0, 1, 0)]);
        let events = &world.redstone.sculk.events;
        assert_eq!(events.len(), 2);
        assert_eq!((events[0].frequency, events[0].source), (10, Some(1)));
        assert_eq!((events[1].frequency, events[1].source), (9, Some(1)));
    }

    #[test]
    fn catalyst_charge_consumes_budget_and_bloom_ends() {
        let mut world = test_world();
        world.set_block(0, 1, 0, blocks::state_by_name("sculk_catalyst").unwrap());
        feed(&mut world, (0, 1, 0), (3.5, 1.0, 0.5), 1);
        assert_eq!(world.redstone.sculk.charges.len(), 1);
        assert!(Block::at(&world, (0, 1, 0)).is("bloom", "true"));
        spread(&mut world);
        assert!(world.redstone.sculk.charges.is_empty());
        assert_eq!(Block::at(&world, (3, 0, 0)).name, "sculk");
        assert_eq!(Block::at(&world, (3, 1, 0)).name, "sculk_vein");
        let block = Block::at(&world, (0, 1, 0));
        catalyst_ticked(&mut world, (0, 1, 0), &block);
        assert!(Block::at(&world, (0, 1, 0)).is("bloom", "false"));
    }


    #[test]
    fn sculk_requires_silk_touch_and_vein_drops_each_face() {
        let state = blocks::orientation("sculk_vein").unwrap().state(&[("up", "true"), ("down", "true"), ("north", "true")]).unwrap();
        assert_eq!(mined_drop(state, false), Some(None));
        assert_eq!(mined_drop(state, true).unwrap().unwrap().count, 3);
        assert_eq!(crate::survival::ore_xp("sculk", 0.5), 1);
        assert_eq!(crate::survival::ore_xp("sculk_catalyst", 0.5), 5);
        assert_eq!(crate::survival::ore_xp("sculk_vein", 0.5), 0);
    }

}
