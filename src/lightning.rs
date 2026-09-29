// Молния (`lightning_bolt`): сущность, удар в громоотвод, чистка меди, огонь.
//
// Источники — страницы minecraft.wiki (Lightning, Lightning Rod, Oxidation)
// и чёрный ящик (сценарии lightning-rod, lightning-lamp и пробы оригинала
// против оригинала). Кода игры и чужих ядер нет.
//
// Что снято с оригинала 26.1.2 чёрным ящиком:
// - сущность появляется сразу по команде; удар — в первом же такте молнии:
//   искры (событие мира 3002) уходят в том же такте, что и сама сущность,
//   а громоотвод и лампа — в рассылке следующего (изменения фазы сущностей);
// - громоотвод гаснет через 8 тактов после удара, лампа — ещё через 4;
// - гром и треск удара клиент играет сам: звуков сервер не шлёт;
// - искра громоотвода — 3002 с осью его направления (0 — X, 1 — Y, 2 — Z:
//   вверх — 1, на восток — 0), искры чистки меди — 3002 с −1 на каждом
//   найденном медном блоке;
// - сущность живёт случайно долго: убрана через 5…18 тактов в 16 записях.

use crate::blocks;
use crate::world::{AIR, World};

/// Место в мире.
pub type Pos = (i32, i32, i32);

/// Событие мира «электрическая искра» (World Event 3002).
pub const SPARK_EVENT: i32 = 3002;

/// Данные искры чистки меди: без оси, во все стороны.
const SPARK_ANY: i32 = -1;

/// С какой «жизни» начинает молния. Удар — в первом такте, потом вспышка
/// гаснет за два такта.
const START_LIFE: i32 = 2;

/// Сколько раз молния ещё вспыхивает после удара: 1…3. Пауза перед
/// вспышкой — случайная, до 10 тактов. [проверить]: модель подобрана по
/// записям оригинала (сущность убрана через 5…18 тактов), вики срок жизни
/// не называет.
const MAX_FLASHES: u32 = 3;
const FLASH_PAUSE: u32 = 10;

/// Чистка меди: «3 to 5 walks», «between 1 and 8 (inclusive)» шагов, на
/// каждом — «10 blocks from a 3×3×3 volume» (вики, Oxidation).
const WALKS_MIN: u32 = 3;
const WALKS_SPREAD: u32 = 3;
const STEPS_MAX: u32 = 8;
const TRIES_PER_STEP: u32 = 10;

/// Сколько огней, кроме места удара, пробует зажечь молния: «creates fires
/// ... in a 2 block radius» (вики, Lightning). [проверить]: число попыток
/// и разброс не названы.
const EXTRA_FIRES: u32 = 4;

/// Молния в мире.
#[derive(Clone, Debug)]
pub struct Bolt {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Сколько тактов ещё светит текущая вспышка; ниже нуля — ждёт
    /// следующей.
    life: i32,

    /// Сколько вспышек осталось; None — удара ещё не было.
    flashes: Option<i32>,
}

impl Bolt {
    pub fn new(entity_id: i32, (x, y, z): (f64, f64, f64)) -> Self {
        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            x,
            y,
            z,
            life: START_LIFE,
            flashes: None,
        }
    }

    /// Блок, в который бьёт молния: тот, на котором она стоит. Призванная
    /// на высоте 2 над громоотводом на высоте 1 бьёт в него (чёрный ящик,
    /// lightning-rod).
    pub fn struck_block(&self) -> Pos {
        (self.x.floor() as i32, (self.y - 1.0e-6).floor() as i32, self.z.floor() as i32)
    }

    /// Клетка, где стоит сама молния, — там загорается огонь.
    fn cell(&self) -> Pos {
        (self.x.floor() as i32, self.y.floor() as i32, self.z.floor() as i32)
    }
}

/// Что сделала молния за такт.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Первый такт: удар.
    Strike,
    /// Ещё одна вспышка.
    Flash,
    /// Светит или ждёт.
    Glow,
    /// Погасла совсем: сущность убирается.
    Gone,
}

/// Счёт жизни молнии за такт; `roll(n)` — случайное от 0 до n−1.
pub fn advance(bolt: &mut Bolt, roll: &mut impl FnMut(u32) -> u32) -> Phase {
    let Some(flashes) = bolt.flashes else {
        bolt.flashes = Some(1 + roll(MAX_FLASHES) as i32);
        bolt.life -= 1;
        return Phase::Strike;
    };

    bolt.life -= 1;

    if bolt.life >= 0 {
        return Phase::Glow;
    }

    if flashes == 0 {
        return Phase::Gone;
    }

    if bolt.life < -(roll(FLASH_PAUSE) as i32) {
        bolt.flashes = Some(flashes - 1);
        bolt.life = 1;
        return Phase::Flash;
    }

    Phase::Glow
}

/// Такт молнии в мире. `fire` — зажигает ли она огонь: только на обычной
/// и сложной сложности (вики, Lightning: «only on normal and hard
/// difficulty»). Возвращает false, когда молния погасла.
pub fn step(world: &mut World, bolt: &mut Bolt, fire: bool) -> bool {
    let phase = {
        let mut roll = |n: u32| world.random_bits() % n;
        advance(bolt, &mut roll)
    };

    match phase {
        Phase::Strike => {
            if fire {
                ignite(world, bolt);
            }

            strike(world, bolt.struck_block());
            true
        }
        // [проверить]: зажигают ли огонь повторные вспышки.
        Phase::Flash => {
            if fire {
                ignite(world, bolt);
            }

            true
        }
        Phase::Glow => true,
        Phase::Gone => false,
    }
}

/// Удар в блок: громоотвод даёт искру по своей оси и сигнал на 8 тактов,
/// медь (и медный громоотвод) очищается от окисления.
pub fn strike(world: &mut World, pos: Pos) {
    let state = world.get_block(pos.0, pos.1, pos.2);
    let name = blocks::block_at_state(state).unwrap_or("");

    // Удар снимает с блока всё окисление: «removes all oxidation from the
    // block» (вики, Oxidation). У громоотвода это видно в той же рассылке,
    // что и `powered=true` (проба оригинала: выветренный громоотвод стал
    // чистым и запитанным одним изменением).
    let clean = copper_stage(name).is_some();

    if clean && let Some(fresh) = cleaned(state, 3) {
        world.set_block(pos.0, pos.1, pos.2, fresh);
    }

    if name.ends_with("lightning_rod") {
        let state = world.get_block(pos.0, pos.1, pos.2);
        let axis = match blocks::orientation_of(state).and_then(|rule| rule.value(state, "facing")) {
            Some("east" | "west") => 0,
            Some("north" | "south") => 2,
            _ => 1,
        };

        world.note_level_event(pos.0, pos.1, pos.2, SPARK_EVENT, axis);
        crate::redstone::lightning_struck(world, pos);
    }

    if clean {
        clean_around(world, pos);
    }

    crate::redstone::settle(world);
}

/// Случайные блуждания от места удара (вики, Oxidation): «the game performs
/// 3 to 5 walks», в каждом 1…8 шагов; на шаге из куба 3×3×3 вокруг точки
/// берутся 10 случайных мест, и первое медное без воска становится новой
/// точкой — с него снимается слой окисления и летит искра.
///
/// Не нашлось меди — блуждание кончается. Вики об этом молчит; так выходит
/// по записям оригинала: у одинокого громоотвода искр 0…3 (в среднем около
/// 1,7 на 10 записей) — ровно столько даёт обрыв, а продолжение шагов дало
/// бы около 5,6. [проверить]
fn clean_around(world: &mut World, start: Pos) {
    let walks = WALKS_MIN + world.random_bits() % WALKS_SPREAD;

    for _ in 0..walks {
        let steps = 1 + world.random_bits() % STEPS_MAX;
        let mut point = start;

        for _ in 0..steps {
            let Some(found) = find_copper(world, point) else {
                break;
            };

            point = found;

            let state = world.get_block(found.0, found.1, found.2);

            if let Some(fresh) = cleaned(state, 1) {
                world.set_block(found.0, found.1, found.2, fresh);
            }

            world.note_level_event(found.0, found.1, found.2, SPARK_EVENT, SPARK_ANY);
        }
    }
}

/// Первое медное место среди 10 случайных в кубе 3×3×3 вокруг `point`.
fn find_copper(world: &mut World, point: Pos) -> Option<Pos> {
    for _ in 0..TRIES_PER_STEP {
        let bits = world.random_bits();
        let place = (
            point.0 + (bits % 3) as i32 - 1,
            point.1 + ((bits / 3) % 3) as i32 - 1,
            point.2 + ((bits / 9) % 3) as i32 - 1,
        );
        let state = world.get_block(place.0, place.1, place.2);

        if blocks::block_at_state(state).and_then(copper_stage).is_some() {
            return Some(place);
        }
    }

    None
}

/// Стадия окисления медного блока без воска (0 — чистый, 3 — окисленный)
/// и его «основа» — имя без приставки стадии. None — не медь, вощёная
/// медь или блок, который не окисляется (медная руда, медный факел).
pub fn copper_stage(name: &str) -> Option<(usize, &str)> {
    if name.starts_with("waxed_") {
        return None;
    }

    for (stage, prefix) in [(1, "exposed_"), (2, "weathered_"), (3, "oxidized_")] {
        if let Some(base) = name.strip_prefix(prefix) {
            return Some((stage, base));
        }
    }

    // Чистая медь окисляется, если у неё есть «потускневшая» стадия.
    let base = if name == "copper_block" { "copper" } else { name };

    blocks::states_of(&format!("exposed_{base}")).map(|_| (0, base))
}

/// Имя блока этой стадии: у чистой меди — `copper_block`.
fn stage_name(stage: usize, base: &str) -> String {
    match stage {
        0 if base == "copper" => "copper_block".to_string(),
        0 => base.to_string(),
        _ => format!("{}{base}", ["", "exposed_", "weathered_", "oxidized_"][stage]),
    }
}

/// То же состояние, на `layers` слоёв окисления чище. None — чистить нечего.
/// Свойства переносятся как есть: у стадий одного блока они одинаковые.
pub fn cleaned(state: i32, layers: usize) -> Option<i32> {
    let name = blocks::block_at_state(state)?;
    let (stage, base) = copper_stage(name)?;

    if stage == 0 {
        return None;
    }

    let (low, _) = blocks::states_of(name)?;
    let (fresh_low, _) = blocks::states_of(&stage_name(stage.saturating_sub(layers), base))?;

    Some(fresh_low + (state - low))
}

/// Огонь от молнии: в клетке молнии и в нескольких случайных рядом —
/// где пусто и есть твёрдый верх снизу. Громоотвод и блок под ним не
/// загораются сами собой: они не пустые.
fn ignite(world: &mut World, bolt: &Bolt) {
    let Some(fire) = blocks::state_by_name("fire") else {
        return;
    };

    let cell = bolt.cell();
    let mut places = vec![cell];

    for _ in 0..EXTRA_FIRES {
        let bits = world.random_bits();

        places.push((
            cell.0 + (bits % 3) as i32 - 1,
            cell.1 + ((bits / 3) % 3) as i32 - 1,
            cell.2 + ((bits / 9) % 3) as i32 - 1,
        ));
    }

    for (x, y, z) in places {
        if world.get_block(x, y, z) == AIR && blocks::solid_top(world.get_block(x, y - 1, z)) {
            world.set_block(x, y, z, fire);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str) -> i32 {
        blocks::state_from_text(text).expect("состояние есть в таблице")
    }

    fn name(world: &World, pos: Pos) -> &'static str {
        blocks::block_at_state(world.get_block(pos.0, pos.1, pos.2)).unwrap_or("")
    }

    /// Мир с каменным полом на высоте 0.
    fn world() -> World {
        let mut world = World::in_memory();

        for x in -8..=8 {
            for z in -8..=8 {
                world.set_block_silently(x, 0, z, state("stone"));
            }
        }

        world
    }

    fn sparks(world: &mut World) -> Vec<(Pos, i32)> {
        let (events, _) = world.events_since(0, 0);

        events
            .into_iter()
            .filter_map(|event| match event {
                crate::world::WorldEvent::Level { x, y, z, effect: SPARK_EVENT, data } => Some(((x, y, z), data)),
                _ => None,
            })
            .collect()
    }

    /// Стадии меди: вощёная и неокисляющаяся — не медь для молнии.
    #[test]
    fn copper_stages_are_read_from_names() {
        assert_eq!(copper_stage("copper_block"), Some((0, "copper")));
        assert_eq!(copper_stage("exposed_copper"), Some((1, "copper")));
        assert_eq!(copper_stage("oxidized_cut_copper_stairs"), Some((3, "cut_copper_stairs")));
        assert_eq!(copper_stage("lightning_rod"), Some((0, "lightning_rod")));
        assert_eq!(copper_stage("weathered_lightning_rod"), Some((2, "lightning_rod")));
        assert_eq!(copper_stage("waxed_oxidized_copper"), None);
        assert_eq!(copper_stage("copper_ore"), None);
        assert_eq!(copper_stage("stone"), None);
    }

    /// Чистка снимает слои и сохраняет свойства.
    #[test]
    fn cleaning_keeps_the_properties() {
        let rod = state("weathered_lightning_rod[facing=east,powered=false,waterlogged=true]");

        assert_eq!(cleaned(rod, 1), Some(state("exposed_lightning_rod[facing=east,powered=false,waterlogged=true]")));
        assert_eq!(cleaned(rod, 3), Some(state("lightning_rod[facing=east,powered=false,waterlogged=true]")));
        assert_eq!(cleaned(state("oxidized_copper"), 1), Some(state("weathered_copper")));
        assert_eq!(cleaned(state("exposed_copper"), 1), Some(state("copper_block")));
        assert_eq!(cleaned(state("copper_block"), 1), None);
    }

    /// Жизнь молнии: удар в первом такте, потом 1…3 вспышки и конец. Срок
    /// укладывается в записанное у оригинала (5…18 тактов) и сверх того
    /// не длится дольше 3·(11 + 2) + 3 тактов.
    #[test]
    fn a_bolt_strikes_once_then_flickers_out() {
        for seed in 0..50u32 {
            let mut bolt = Bolt::new(1, (0.5, 1.0, 0.5));
            let mut state = seed.wrapping_mul(2_654_435_761).max(1);
            let mut roll = |n: u32| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state % n
            };

            assert_eq!(advance(&mut bolt, &mut roll), Phase::Strike);

            let mut ticks = 1;

            loop {
                let phase = advance(&mut bolt, &mut roll);

                assert_ne!(phase, Phase::Strike, "второй удар");
                ticks += 1;

                if phase == Phase::Gone {
                    break;
                }

                assert!(ticks < 50, "молния не гаснет");
            }

            assert!(ticks >= 5, "молния погасла слишком рано: {ticks}");
        }
    }

    /// Удар в громоотвод: искра по оси, чистка окисления, сигнал на 8 тактов
    /// (проверяется в redstone.rs), без огня на мирной сложности.
    #[test]
    fn a_struck_rod_sparks_along_its_axis_and_is_cleaned() {
        let mut world = world();
        world.watch_events(0);
        world.set_block(0, 1, 0, state("weathered_lightning_rod[facing=up,powered=false,waterlogged=false]"));
        world.set_block(3, 1, 3, state("oxidized_copper"));

        let mut bolt = Bolt::new(1, (0.5, 2.0, 0.5));
        assert_eq!(bolt.struck_block(), (0, 1, 0));
        assert!(step(&mut world, &mut bolt, false));

        let rod = world.get_block(0, 1, 0);
        assert_eq!(rod, state("lightning_rod[facing=up,powered=true,waterlogged=false]"));

        let sparks = sparks(&mut world);
        assert_eq!(sparks.first(), Some(&((0, 1, 0), 1)), "первая искра — громоотвода, по оси Y");
        assert!(sparks[1..].iter().all(|&(at, data)| at == (0, 1, 0) && data == SPARK_ANY), "искры чистки — только на меди");

        // Далёкая медь не тронута, огня нет.
        assert_eq!(name(&world, (3, 1, 3)), "oxidized_copper");
        assert_eq!(name(&world, (0, 2, 0)), "air");
    }

    /// Громоотвод на восток даёт искру с осью X (0).
    #[test]
    fn a_sideways_rod_sparks_along_x() {
        let mut world = world();
        world.watch_events(0);
        world.set_block(0, 1, 0, state("lightning_rod[facing=east,powered=false,waterlogged=false]"));

        strike(&mut world, (0, 1, 0));

        assert_eq!(sparks(&mut world).first(), Some(&((0, 1, 0), 0)));
    }

    /// Блуждание чистит медь вокруг по слою за искру; вощёную не трогает.
    #[test]
    fn walks_clean_nearby_copper_one_layer_per_spark() {
        let mut world = world();
        world.watch_events(0);
        world.set_block(0, 1, 0, state("copper_block"));
        world.set_block(1, 1, 0, state("oxidized_copper"));
        world.set_block(0, 1, 1, state("waxed_oxidized_copper"));

        for _ in 0..20 {
            strike(&mut world, (0, 1, 0));
        }

        let sparks = sparks(&mut world);
        let at_neighbour = sparks.iter().filter(|(at, _)| *at == (1, 1, 0)).count();

        assert!(at_neighbour > 0, "за 20 ударов соседняя медь ни разу не найдена");
        assert!(sparks.iter().all(|(at, _)| *at != (0, 1, 1)), "вощёная медь дала искру");
        assert_eq!(name(&world, (0, 1, 1)), "waxed_oxidized_copper");

        let expected = ["oxidized_copper", "weathered_copper", "exposed_copper", "copper_block"][at_neighbour.min(3)];
        assert_eq!(name(&world, (1, 1, 0)), expected);
    }

    /// Огонь — только если разрешён, и только на твёрдом верху.
    #[test]
    fn fire_only_when_allowed() {
        let mut world = world();
        let mut bolt = Bolt::new(1, (0.5, 1.0, 0.5));

        step(&mut world, &mut bolt, false);
        assert_eq!(name(&world, (0, 1, 0)), "air");

        let mut bolt = Bolt::new(2, (0.5, 1.0, 0.5));

        step(&mut world, &mut bolt, true);
        assert_eq!(name(&world, (0, 1, 0)), "fire");
    }
}
