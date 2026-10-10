// Леса: minecraft.wiki (Scaffolding / Gerüst), «six blocks» по горизонтали.
// [проверить]: порядок разрушения и падения, постановка с Shift.
use crate::{blocks, fluids, world::{World, AIR}};
type Pos = (i32, i32, i32);
fn is(state: i32) -> bool { blocks::block_at_state(state) == Some("scaffolding") }
fn prop(state: i32, n: &str) -> Option<&'static str> { blocks::orientation_of(state)?.value(state, n) }

pub fn distance(world: &World, (x, y, z): Pos) -> u8 {
    let below = world.get_block(x, y - 1, z);
    if !is(below) && blocks::solid_top(below) { return 0; }
    let mut best = if is(below) { prop(below, "distance").and_then(|s| s.parse().ok()).unwrap_or(7) } else { 7 };
    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let next = world.get_block(x + dx, y, z + dz);
        if is(next) {
            best = best.min(prop(next, "distance").and_then(|s| s.parse::<u8>().ok()).unwrap_or(7).saturating_add(1));
        }
    }
    best.min(7)
}

pub fn placed(world: &World, p: Pos, state: i32) -> i32 {
    let rule = blocks::orientation_of(state).unwrap();
    let d = distance(world, p);
    let bottom = d > 0 && !is(world.get_block(p.0, p.1 - 1, p.2));
    let water = matches!(fluids::fluid_at(world.get_block(p.0, p.1, p.2)), Some((fluids::Kind::Water, 0)));
    rule.state(&[("distance", &d.to_string()), ("bottom", if bottom { "true" } else { "false" }), ("waterlogged", if water { "true" } else { "false" })]).unwrap()
}

pub fn changed(world: &mut World, p: Pos) { world.schedule_block_once(p.0, p.1, p.2, 1); }

pub fn ticked(world: &mut World, p: Pos) {
    let here = world.get_block(p.0, p.1, p.2);
    let d = distance(world, p);
    if d == 7 {
        if prop(here, "distance") == Some("7") {
            // Падающий блок несёт состояние, пересчитанное этим тактом: bottom=true
            // (чёрный ящик, scaffolding-support: falling_block 20721).
            let rule = blocks::orientation_of(here).unwrap();
            let bottom = !is(world.get_block(p.0, p.1 - 1, p.2));
            if let Some(next) = rule.with(here, "bottom", if bottom { "true" } else { "false" }) {
                world.set_block_silently(p.0, p.1, p.2, next);
            }
            world.note_falling(p.0, p.1, p.2);
        } else {
            // Рушится «как положено», с событием слома (чёрный ящик, scaffolding-support).
            world.note_level_event(p.0, p.1, p.2, 2001, here);
            world.note_destroyed(p.0, p.1, p.2);
            let left = if prop(here, "waterlogged") == Some("true") { blocks::state_by_name("water").unwrap() } else { AIR };
            world.set_block(p.0, p.1, p.2, left);
        }
        return;
    }
    let rule = blocks::orientation_of(here).unwrap();
    let bottom = d > 0 && !is(world.get_block(p.0, p.1 - 1, p.2));
    let next = rule.with(here, "distance", &d.to_string()).and_then(|s| rule.with(s, "bottom", if bottom { "true" } else { "false" })).unwrap();
    world.set_block(p.0, p.1, p.2, next);
}

/// Наращивание через цепочку лесов: боковая грань вверх, верхняя по yaw.
pub fn extension(world: &World, clicked: Pos, face: i32, yaw: f32) -> Option<Pos> {
    let direction = if face == 1 {
        match ((yaw / 90.0).round() as i32).rem_euclid(4) {
            0 => (0, 0, 1), 1 => (-1, 0, 0), 2 => (0, 0, -1), _ => (1, 0, 0),
        }
    } else { (0, 1, 0) };
    let mut p = clicked;
    for _ in 0..384 {
        p = (p.0 + direction.0, p.1 + direction.1, p.2 + direction.2);
        if !(-64..320).contains(&p.1) { return None; }
        if !is(world.get_block(p.0, p.1, p.2)) { return Some(p); }
        if direction.1 == 0 && (p.0 - clicked.0).abs() + (p.2 - clicked.2).abs() >= 7 { return None; }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(n: &str) -> i32 { blocks::state_from_text(n).unwrap() }
    fn run(w: &mut World, count: usize) {
        crate::redstone::settle(w);
        for _ in 0..count {
            for (p, _) in w.advance(10000) { crate::redstone::ticked(w, p); }
            crate::redstone::settle(w);
        }
    }
    #[test]
    fn horizontal_limit_vertical_inheritance_and_support_loss() {
        let mut w = World::in_memory();
        w.set_block(0, 9, 0, state("stone"));
        for x in 0..=7 {
            let s = placed(&w, (x, 10, 0), state("scaffolding"));
            w.set_block(x, 10, 0, s);
        }
        run(&mut w, 2);
        assert_eq!(w.take_falling(), vec![(7, 10, 0)]);
        for x in 0..=6 { assert_eq!(prop(w.get_block(x, 10, 0), "distance"), Some(x.to_string().as_str())); }
        assert_eq!(prop(w.get_block(1, 10, 0), "bottom"), Some("true"));
        let top = placed(&w, (3, 11, 0), state("scaffolding"));
        w.set_block(3, 11, 0, top);
        run(&mut w, 2);
        assert_eq!(prop(w.get_block(3, 11, 0), "distance"), Some("3"));
        assert_eq!(prop(w.get_block(3, 11, 0), "bottom"), Some("false"));
        w.set_block(0, 9, 0, AIR);
        run(&mut w, 20);
        for x in 0..=6 { assert_eq!(w.get_block(x, 10, 0), AIR); }
        assert_eq!(w.get_block(3, 11, 0), AIR);
    }
    #[test]
    fn extension_finds_top_and_placement_preserves_water() {
        let mut w = World::in_memory();
        for y in 10..=14 { w.set_block_silently(0, y, 0, state("scaffolding[distance=0]")); }
        assert_eq!(extension(&w, (0, 10, 0), 3, 0.0), Some((0, 15, 0)));
        assert_eq!(extension(&w, (0, 10, 0), 1, 90.0), Some((-1, 10, 0)));
        w.set_block_silently(0, 15, 0, state("water"));
        assert_eq!(prop(placed(&w, (0, 15, 0), state("scaffolding")), "waterlogged"), Some("true"));
    }
}
