// Серный шип 26.2: minecraft.wiki, Sulfur Spike. Форма, опора и обрушение повторяют капельник
// (те же свойства и их порядок), рост — по вики: «grows downward ... maximum length of 3 blocks»,
// «1.138% (64/5625)» на случайный такт кончика сталактита под блоком серы.
// [проверить] на официальном 26.3: падение сталактита как сущности с уроном «skewered by a falling
// stalactite» (здесь шип просто ломается и выпадает предметом), выбор между ростом сталактита и
// сталагмита (здесь поровну), форма при установке игроком по взгляду, урон при падении на сталагмит.
use crate::gameplay_catalog::{BlockForm, SlabDescriptor, MINERALS};
use crate::{blocks, fluids, world::{World, AIR}};

type Pos = (i32, i32, i32);

/// Порядок свойств шипа: thickness (tip_merge, tip, frustum, middle, base), vertical_direction (up, down),
/// waterlogged (true, false). Номер = ((толщина * 2 + направление) * 2) + (не налит).
const TIP_MERGE: u8 = 0;
const TIP: u8 = 1;
const FRUSTUM: u8 = 2;
const MIDDLE: u8 = 3;
const BASE: u8 = 4;
/// Вики: сталактит растёт до трёх блоков, сталагмит ищет опору не дальше 11 блоков под кончиком.
const MAX_LENGTH: i32 = 3;
const REACH: i32 = 11;

fn row() -> &'static SlabDescriptor {
    static ROW: std::sync::OnceLock<&'static SlabDescriptor> = std::sync::OnceLock::new();
    ROW.get_or_init(|| MINERALS.iter().find(|row| row.form == BlockForm::Spike).expect("шип в каталоге"))
}

pub fn is_spike(state: i32) -> bool { (row().first_state..=row().last_state).contains(&state) }

/// (толщина, смотрит ли вниз, налита ли вода).
fn parts(state: i32) -> Option<(u8, bool, bool)> {
    if !is_spike(state) { return None; }
    let offset = (state - row().first_state) as u8;
    Some((offset / 4, (offset / 2) % 2 == 1, offset.is_multiple_of(2)))
}

fn make(thickness: u8, down: bool, wet: bool) -> i32 {
    row().first_state + i32::from((thickness * 2 + u8::from(down)) * 2 + u8::from(!wet))
}

fn at(w: &World, p: Pos) -> i32 { w.get_block(p.0, p.1, p.2) }
fn shift(p: Pos, dy: i32) -> Pos { (p.0, p.1 + dy, p.2) }
/// +1 — стоит на полу (смотрит вверх), -1 — висит (смотрит вниз).
fn direction(state: i32) -> Option<i32> { parts(state).map(|(_, down, _)| if down { -1 } else { 1 }) }
fn thickness(state: i32) -> Option<u8> { parts(state).map(|(thickness, ..)| thickness) }

fn water(wet: bool) -> i32 { if wet { fluids::state_of(fluids::Kind::Water, fluids::SOURCE) } else { AIR } }

pub fn supported(w: &World, p: Pos, state: i32) -> bool {
    let Some(dy) = direction(state) else { return true; };
    let behind = at(w, shift(p, -dy));
    direction(behind) == Some(dy) || if dy == 1 { blocks::solid_top(behind) } else { blocks::full_cube(behind) }
}

fn shaped(w: &World, p: Pos, state: i32) -> i32 {
    let (_, down, wet) = parts(state).expect("шип");
    let dy = if down { -1 } else { 1 };
    let ahead = at(w, shift(p, dy));
    let behind = at(w, shift(p, -dy));
    let next = if direction(ahead) == Some(-dy) { TIP_MERGE }
        else if direction(ahead) != Some(dy) { TIP }
        else if matches!(thickness(ahead), Some(TIP | TIP_MERGE)) { FRUSTUM }
        else if direction(behind) == Some(dy) { MIDDLE } else { BASE };
    make(next, down, wet)
}

/// Установка игроком: щелчок по нижней грани — сталактит, иначе сталагмит; вода берётся из клетки.
pub fn placed(w: &World, p: Pos, state: i32, face: i32) -> Option<i32> {
    if !is_spike(state) { return None; }
    let wet = matches!(fluids::fluid_at(at(w, p)), Some((fluids::Kind::Water, 0)));
    Some(shaped(w, p, make(TIP, face == 0, wet)))
}

/// Сосед изменился: пересчитываем форму; без опоры шип рухнет через два такта.
pub fn changed(w: &mut World, p: Pos) {
    let state = at(w, p);
    if !is_spike(state) { return; }
    if !supported(w, p, state) { w.schedule_block_once(p.0, p.1, p.2, 2); }
    let next = shaped(w, p, state);
    if next != state { w.set_block(p.0, p.1, p.2, next); }
}

/// Отложенный такт: без опоры сталактит ломается целиком вниз по цепочке, сталагмит — сам.
pub fn ticked(w: &mut World, p: Pos) {
    let state = at(w, p);
    if !is_spike(state) || supported(w, p, state) { return; }
    let mut cell = p;
    loop {
        let here = at(w, cell);
        let Some((_, down, wet)) = parts(here) else { break; };
        w.note_destroyed(cell.0, cell.1, cell.2);
        w.set_block(cell.0, cell.1, cell.2, water(wet));
        if !down { break; }
        cell = shift(cell, -1);
    }
}

/// Случайный такт кончика сталактита под блоком серы.
pub fn random_tick(w: &mut World, p: Pos, state: i32) {
    let Some((TIP, true, false)) = parts(state) else { return; };
    if w.random_unit() >= 64.0 / 5625.0 { return; }
    // Длина цепочки сверху вниз и корень под блоком серы.
    let mut root = p;
    let mut length = 1;
    while direction(at(w, shift(root, 1))) == Some(-1) { root = shift(root, 1); length += 1; }
    let sulfur = MINERALS.iter().find(|row| row.name == "sulfur").expect("сера").default_state;
    if at(w, shift(root, 1)) != sulfur { return; }
    if w.random_unit() < 0.5 {
        // Сталактит удлиняется вниз, если там свободно.
        let below = shift(p, -1);
        if length >= MAX_LENGTH || !free(at(w, below)) { return; }
        w.set_block(below.0, below.1, below.2, make(TIP, true, false));
        changed(w, p);
        changed(w, below);
        return;
    }
    // Сталагмит тянется вверх к сталактиту; при длине три и больше не растёт.
    if length >= MAX_LENGTH { return; }
    for depth in 1..=REACH {
        let cell = shift(p, -depth);
        let here = at(w, cell);
        if direction(here) == Some(1) {
            // Уже стоит сталагмит: наращиваем его вершину, пока над ней пусто.
            let mut top = cell;
            while direction(at(w, shift(top, 1))) == Some(1) { top = shift(top, 1); }
            let above = shift(top, 1);
            if free(at(w, above)) && above.1 < p.1 {
                w.set_block(above.0, above.1, above.2, make(TIP, false, false));
                changed(w, top);
                changed(w, above);
            }
            return;
        }
        if free(here) { continue; }
        // Первая преграда: на её верхней грани начинает расти новый сталагмит.
        if blocks::solid_top(here) {
            let above = shift(cell, 1);
            if free(at(w, above)) {
                w.set_block(above.0, above.1, above.2, make(TIP, false, false));
                changed(w, above);
            }
        }
        return;
    }
}

fn free(state: i32) -> bool { state == AIR || (blocks::replaceable(state) && fluids::fluid_at(state).is_none()) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile};

    fn world() -> World {
        let mut w = World::in_memory_with_catalog(Catalog::new(Profile::Preview26_3));
        for x in -2..=2 { for z in -2..=2 { w.set_block_silently(x, 60, z, blocks::state_by_name("stone").unwrap()); } }
        w
    }
    fn sulfur() -> i32 { MINERALS.iter().find(|row| row.name == "sulfur").unwrap().default_state }

    #[test]
    fn state_layout_matches_the_catalog_properties() {
        let catalog = Catalog::new(Profile::Preview26_3);
        let rule = catalog.orientation("sulfur_spike").unwrap().unwrap();
        for (index, name) in ["tip_merge", "tip", "frustum", "middle", "base"].into_iter().enumerate() {
            for (down, dir) in [(false, "up"), (true, "down")] {
                for wet in [false, true] {
                    let state = make(index as u8, down, wet);
                    assert_eq!(rule.value(state, "thickness"), Some(name));
                    assert_eq!(rule.value(state, "vertical_direction"), Some(dir));
                    assert_eq!(rule.value(state, "waterlogged"), Some(if wet { "true" } else { "false" }));
                    assert_eq!(parts(state), Some((index as u8, down, wet)));
                }
            }
        }
        assert_eq!(row().default_state, make(TIP, false, false));
    }

    #[test]
    fn hanging_spikes_reshape_and_fall_apart_without_support() {
        let mut w = world();
        w.set_block_silently(0, 64, 0, sulfur());
        for y in [63, 62] { w.set_block_silently(0, y, 0, placed(&w, (0, y, 0), row().default_state, 0).unwrap()); }
        changed(&mut w, (0, 63, 0));
        assert_eq!(thickness(at(&w, (0, 62, 0))), Some(TIP));
        assert_eq!(thickness(at(&w, (0, 63, 0))), Some(FRUSTUM));
        assert!(supported(&w, (0, 63, 0), at(&w, (0, 63, 0))));
        // Убрали блок серы: сталактит осыпается целиком и даёт предметы.
        w.set_block_silently(0, 64, 0, AIR);
        ticked(&mut w, (0, 63, 0));
        assert_eq!((at(&w, (0, 63, 0)), at(&w, (0, 62, 0))), (AIR, AIR));
        assert_eq!(w.take_destroyed().len(), 2);
    }

    #[test]
    fn stalagmite_needs_a_solid_top_and_water_is_kept() {
        let mut w = world();
        w.set_block_silently(0, 61, 0, fluids::state_of(fluids::Kind::Water, fluids::SOURCE));
        let state = placed(&w, (0, 61, 0), row().default_state, 1).unwrap();
        assert_eq!(parts(state), Some((TIP, false, true)));
        assert!(supported(&w, (0, 61, 0), state));
        assert!(!supported(&w, (0, 63, 0), state));
        w.set_block_silently(0, 61, 0, state);
        w.set_block_silently(0, 60, 0, AIR);
        ticked(&mut w, (0, 61, 0));
        assert_eq!(at(&w, (0, 61, 0)), fluids::state_of(fluids::Kind::Water, fluids::SOURCE));
    }

    #[test]
    fn tip_grows_only_under_sulfur_and_stops_at_three() {
        let mut w = world();
        w.set_block_silently(0, 64, 0, sulfur());
        w.set_block_silently(0, 63, 0, make(TIP, true, false));
        let mut grew = false;
        for _ in 0..200_000 {
            let tip = (0..3).map(|d| (0, 63 - d, 0)).filter(|p| thickness(at(&w, *p)) == Some(TIP) && direction(at(&w, *p)) == Some(-1)).last().unwrap();
            let state = at(&w, tip);
            random_tick(&mut w, tip, state);
            grew |= at(&w, (0, 62, 0)) != AIR;
            let length = (0..6).filter(|d| direction(at(&w, (0, 63 - d, 0))) == Some(-1)).count();
            assert!(length <= 3);
        }
        assert!(grew);
        // Под обычным камнем шип не растёт.
        let mut w = world();
        w.set_block_silently(0, 64, 0, blocks::state_by_name("stone").unwrap());
        w.set_block_silently(0, 63, 0, make(TIP, true, false));
        let state = at(&w, (0, 63, 0));
        for _ in 0..50_000 { random_tick(&mut w, (0, 63, 0), state); }
        assert_eq!(at(&w, (0, 62, 0)), AIR);
    }
}
