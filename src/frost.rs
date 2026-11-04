// Ледоход: minecraft.wiki/w/Frost_Walker — «still water blocks with air
// above», «2 + level». Механика мира общая для Java и Bedrock.
use crate::{blocks, fluids, redstone, world::{AIR, World}};

pub struct Body {
    pub at: (f64, f64, f64),
    pub size: (f64, f64),
}

impl Body {
    pub fn overlaps(&self, (x, y, z): (i32, i32, i32)) -> bool {
        let (width, height) = self.size;
        width > 0.0 && height > 0.0
            && self.at.0 + width / 2.0 > f64::from(x) && self.at.0 - width / 2.0 < f64::from(x) + 1.0
            && self.at.1 + height > f64::from(y) && self.at.1 < f64::from(y) + 1.0
            && self.at.2 + width / 2.0 > f64::from(z) && self.at.2 - width / 2.0 < f64::from(z) + 1.0
    }
}

type Pos = (i32, i32, i32);
const SIDES: [Pos; 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];

fn cell((x, y, z): (f64, f64, f64)) -> Pos { (x.floor() as i32, y.floor() as i32, z.floor() as i32) }
fn ice(world: &World, (x, y, z): Pos) -> bool { blocks::block_at_state(world.get_block(x, y, z)) == Some("frosted_ice") }
fn neighbours(world: &World, (x, y, z): Pos) -> usize {
    SIDES.iter().filter(|&&(dx, dy, dz)| ice(world, (x + dx, y + dy, z + dz))).count()
}

/// enchant-frost-immunity, vanilla 26.1.2: даже /damage hot_floor и
/// campfire не проходят с Ледоходом I; lava проходит как обычно.
pub fn protects(inventory: &crate::inventory::Inventory, source: crate::survival::Source) -> bool {
    protects_boots(inventory.slot(8), source)
}

pub fn protects_boots(boots: Option<crate::inventory::Stack>, source: crate::survival::Source) -> bool {
    matches!(source, crate::survival::Source::HotFloor | crate::survival::Source::Campfire)
        && boots.is_some_and(|stack| stack.extras().level("frost_walker") > 0)
}

/// enchant-frost-break, vanilla 26.1.2: обычная кирка над камнем оставляет
/// воду, Silk Touch, творческий режим и воздух снизу — воздух. Это верно
/// и для обычного льда; подмороженный при этом никогда не выпадает вещью.
/// Вызывается после успешного удаления блока, до рассылки его состояния.
pub fn broken(world: &mut World, (x, y, z): Pos, state: i32, creative: bool, silk: bool) {
    if creative || silk || world.is_nether()
        || !matches!(blocks::block_at_state(state), Some("ice" | "frosted_ice"))
    { return; }
    let below = world.get_block(x, y - 1, z);
    // Вики, Block properties/Solid (legacy): «motion-blocking block or
    // a fluid». Частичные формы опоры [проверить] отдельным замером.
    if blocks::has_box(below) || fluids::fluid_at(below).is_some() {
        world.set_block(x, y, z, fluids::state_of(fluids::Kind::Water, fluids::SOURCE));
    }
}

/// Шаг, а не постоянное обновление таймера льда под стоящим игроком.
/// Телепорт, полёт, транспорт и смерть отсекаются вызывающим кодом.
/// Рабочий обработчик отслеживает смену клетки/ботинок и зовёт freeze;
/// здесь проверяется отдельно условие повторного движения.
#[cfg(test)]
pub fn walk(world: &mut World, from: (f64, f64, f64), to: (f64, f64, f64), on_ground: bool, level: i32, entity_id: i32) -> usize {
    if level <= 0 || !on_ground || cell(from) == cell(to) { return 0; }
    freeze(world, to, on_ground, level, entity_id)
}

/// Надевание ботинок также замораживает воду, даже без шага
/// (enchant-frost-melt, vanilla 26.1.2: первая заморозка equip+1).
#[cfg(test)]
pub fn freeze(world: &mut World, to: (f64, f64, f64), on_ground: bool, level: i32, entity_id: i32) -> usize {
    freeze_with_collision(world, to, on_ground, level, entity_id, |_| false)
}

/// Проверка коллизий передана от владельца списка сущностей: сам World
/// не захватывает Items/Players и не создаёт обратного порядка замков.
pub fn freeze_with_collision(world: &mut World, to: (f64, f64, f64), on_ground: bool, level: i32, entity_id: i32, mut blocked: impl FnMut(Pos) -> bool) -> usize {
    if level <= 0 || !on_ground { return 0; }
    let radius = (2 + level.clamp(0, 255)).min(16);
    let (cx, cy, cz) = cell(to);
    let y = cy - 1;
    let water = fluids::state_of(fluids::Kind::Water, fluids::SOURCE);
    let frozen = blocks::state_by_name("frosted_ice").expect("подмороженный лёд");
    let actor = world.actor.replace(entity_id);
    let mut changed = 0;
    for x in cx - radius..=cx + radius {
        for z in cz - radius..=cz + radius {
            let dx = x as f64 + 0.5 - to.0;
            let dy = y as f64 + 0.5 - to.1;
            let dz = z as f64 + 0.5 - to.2;
            if dx * dx + dy * dy + dz * dz >= f64::from(radius * radius)
                || world.get_block_loaded(x, y, z) != Some(water)
                || !matches!(blocks::block_at_state(world.get_block(x, y + 1, z)), Some("air" | "cave_air" | "void_air"))
            { continue; }
            if blocked((x, y, z)) { continue; }
            if world.set_block(x, y, z, frozen) {
                let delay = 60 + u64::from(world.random_bits() % 61);
                world.schedule_block_once(x, y, z, delay);
                redstone::game_event(world, (x, y, z), redstone::FREQ_PLACE);
                changed += 1;
            }
        }
    }
    world.actor = actor;
    changed
}

fn schedule(world: &mut World, (x, y, z): Pos) {
    // enchant-frost-melt, две записи vanilla 26.1.2: и первая, и
    // следующие стадии ждут 60..120 тактов. Пропуск старения оставляет
    // ещё один интервал; соседнее таяние может состарить раньше срока.
    let delay = 60 + u64::from(world.random_bits() % 61);
    world.schedule_block_once(x, y, z, delay);
}

/// Исчезновение соседа разрушает остатки пятна с менее чем двумя
/// соседями; событие запускается только при удалении подмороженного льда.
/// [проверить] Порог двумя vanilla-записями подтверждён не отдельно.
pub fn removed(world: &mut World, p: Pos) {
    for (dx, dy, dz) in SIDES {
        let beside = (p.0 + dx, p.1 + dy, p.2 + dz);
        if ice(world, beside) && neighbours(world, beside) < 2 {
            let next = if world.is_nether() { AIR } else { fluids::state_of(fluids::Kind::Water, fluids::SOURCE) };
            world.set_block(beside.0, beside.1, beside.2, next);
        }
    }
}

/// Возвращает true, только если лёд превратился в воду.
fn age_once(world: &mut World, (x, y, z): Pos) -> bool {
    let state = world.get_block(x, y, z);
    let Some(block) = blocks::orientation_of(state).filter(|block| block.name == "frosted_ice") else { return false; };
    let age = block.value(state, "age").and_then(|value| value.parse::<u8>().ok()).unwrap_or(0);
    if age < 3 {
        if let Some(next) = block.with(state, "age", &(age + 1).to_string()) { world.set_block(x, y, z, next); }
        false
    } else {
        let next = if world.is_nether() { AIR } else { fluids::state_of(fluids::Kind::Water, fluids::SOURCE) };
        world.set_block(x, y, z, next);
        // [проверить] Вики не уточняет частоту и подмороженный вариант льда.
        if world.catalog().changed_rules() { redstone::game_event(world, (x, y, z), redstone::FREQ_DESTROY); }
        true
    }
}

/// Запланированный или случайный такт: ночь и темнота сохраняют лёд.
/// [проверить] Порог света/соседей и каскад на границе пятна чёрным ящиком.
pub fn ticked(world: &mut World, p: Pos) {
    let state = world.get_block(p.0, p.1, p.2);
    let Some(block) = blocks::orientation_of(state).filter(|block| block.name == "frosted_ice") else { return; };
    let age = block.value(state, "age").and_then(|value| value.parse::<u8>().ok()).unwrap_or(0);
    let (sky, emitted) = world.light_full(p.0, p.1, p.2);
    let sky = if world.is_nether() || world.is_end() { 0 } else {
        let day = world.weather.dim_sky(redstone::ambient_sky(world.time_of_day()));
        sky.saturating_sub((15 - day).clamp(0, 15) as u8)
    };
    let selected = world.random_bits().is_multiple_of(3) || neighbours(world, p) < 4;
    if selected && sky.max(emitted) > 11 - age && age_once(world, p) {
        // Один растаявший блок старит соседей на одну стадию; этот цикл
        // не вызывает сам себя рекурсивно.
        for (dx, dy, dz) in SIDES {
            let beside = (p.0 + dx, p.1 + dy, p.2 + dz);
            if ice(world, beside) && !age_once(world, beside) { schedule(world, beside); }
        }
    } else if ice(world, p) {
        schedule(world, p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn breaking_ice_obeys_silk_creative_and_support() {
        for name in ["ice", "frosted_ice"] {
            for (support, creative, silk, water) in [
                ("stone", false, false, true), ("water", false, false, true),
                ("air", false, false, false), ("stone", true, false, false),
                ("stone", false, true, false),
            ] {
                let mut world = World::in_memory();
                world.set_block_silently(0, 63, 0, blocks::state_by_name(support).unwrap());
                world.set_block_silently(0, 64, 0, blocks::state_by_name(name).unwrap());
                let state = crate::placing::break_block(&mut world, 0, 64, 0).unwrap();
                broken(&mut world, (0, 64, 0), state, creative, silk);
                assert_eq!(blocks::block_at_state(world.get_block(0, 64, 0)), Some(if water { "water" } else { "air" }),
                    "{name}, {support}, creative={creative}, silk={silk}");
            }
        }
    }

    fn pool() -> World {
        let mut world = World::in_memory();
        for x in 0..=10 { for z in 0..=10 {
            world.set_block_silently(x, 63, z, blocks::state_by_name("stone").unwrap());
            world.set_block_silently(x, 64, z, fluids::state_of(fluids::Kind::Water, 0));
        } }
        world.set_time_of_day(6000);
        world
    }

    #[test]
    fn walk_freezes_only_uncovered_sources_on_ground_and_never_refreshes_standing_ice() {
        let mut world = pool();
        world.set_block_silently(5, 65, 5, blocks::state_by_name("glass").unwrap());
        world.set_block_silently(6, 64, 5, fluids::state_of(fluids::Kind::Water, 1));
        world.set_block_silently(4, 64, 5, blocks::state_by_name("oak_slab").unwrap());
        assert_eq!(walk(&mut world, (3.5,65.0,5.5), (4.5,65.0,5.5), false, 2, 7), 0);
        assert_eq!(walk(&mut world, (3.5,65.0,5.5), (4.5,65.0,5.5), true, 0, 7), 0);
        assert!(walk(&mut world, (3.5,65.0,5.5), (4.5,65.0,5.5), true, 2, 7) > 0);
        assert!(!ice(&world, (5,64,5)));
        assert!(!ice(&world, (6,64,5)));
        assert!(!ice(&world, (4,64,5)));
        assert!(ice(&world, (4,64,4)));
        assert!(!ice(&world, (9,64,5)));
        let scheduled = world.scheduled_count();
        assert_eq!(walk(&mut world, (4.5,65.0,5.5), (4.7,65.0,5.5), true, 2, 7), 0);
        assert_eq!(world.scheduled_count(), scheduled);
        assert_eq!(world.actor, None);
    }

    #[test]
    fn isolated_ice_ages_in_daylight_but_survives_darkness_and_stale_ticks() {
        let mut world = pool();
        let p = (5,64,5);
        let fresh = blocks::state_by_name("frosted_ice").unwrap();
        world.set_block_silently(p.0,p.1,p.2,fresh);
        world.set_time_of_day(18000);
        for _ in 0..12 { ticked(&mut world,p); }
        assert_eq!(world.get_block(p.0,p.1,p.2),fresh);
        world.set_time_of_day(6000);
        for age in 1..=3 {
            ticked(&mut world,p);
            assert_eq!(blocks::orientation("frosted_ice").unwrap().value(world.get_block(p.0,p.1,p.2), "age"), Some(age.to_string().as_str()));
        }
        ticked(&mut world,p);
        assert_eq!(world.get_block(p.0,p.1,p.2),fluids::state_of(fluids::Kind::Water,0));
        let stone = blocks::state_by_name("stone").unwrap();
        world.set_block_silently(p.0,p.1,p.2,stone);
        ticked(&mut world,p);
        assert_eq!(world.get_block(p.0,p.1,p.2),stone);
    }

    #[test]
    fn removing_ice_only_collapses_unsupported_neighbours_and_collision_keeps_water() {
        let mut world = pool();
        let p = (4,64,5);
        let count = freeze_with_collision(&mut world, (4.5,65.0,5.5), true, 2, 7, |candidate| candidate == p);
        assert!(count > 0);
        assert!(!ice(&world, p));
        assert!(ice(&world, (4,64,4)));
        let fresh = blocks::state_by_name("frosted_ice").unwrap();
        let mut isolated = World::in_memory();
        for x in 0..=2 { isolated.set_block_silently(x,64,0,fresh); }
        isolated.set_block(1,64,0,fluids::state_of(fluids::Kind::Water,0));
        redstone::settle(&mut isolated);
        assert!(!ice(&isolated,(0,64,0)));
        assert!(!ice(&isolated,(2,64,0)));
        let mut square = World::in_memory();
        for (x,z) in [(0,0),(1,0),(0,1),(1,1)] { square.set_block_silently(x,64,z,fresh); }
        // У всех четырёх есть два соседа, постороннее уведомление ничего
        // не растворяет. Верхнее расположение источника исключает сдвиг.
        removed(&mut square, (0,65,0));
        assert!(ice(&square,(0,64,0)));
    }
}
