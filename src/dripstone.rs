// Капельник: minecraft.wiki, Pointed Dripstone / Spitzer Tropfstein.
// «water: 17.6%, lava: 5.9%», котёл в пределах десяти пустых блоков.
// [проверить]: очереди капель, неполные опоры, рост и урон падения отдельно.
use crate::{blocks, fluids, world::{World, AIR}};
type Pos = (i32, i32, i32);
fn at(w: &World, p: Pos) -> i32 { w.get_block(p.0, p.1, p.2) }
fn prop(s: i32, n: &str) -> Option<&'static str> { blocks::orientation_of(s)?.value(s, n) }
fn direction(s: i32) -> Option<i32> {
    if blocks::block_at_state(s) != Some("pointed_dripstone") { return None; }
    Some(if prop(s, "vertical_direction") == Some("down") { -1 } else { 1 })
}
fn shift(p: Pos, dy: i32) -> Pos { (p.0, p.1 + dy, p.2) }

pub fn supported(w: &World, p: Pos, state: i32) -> bool {
    let Some(dy) = direction(state) else { return true; };
    let behind = at(w, shift(p, -dy));
    direction(behind) == Some(dy) || if dy == 1 { blocks::solid_top(behind) } else { blocks::full_cube(behind) }
}

pub fn shaped(w: &World, p: Pos, s: i32) -> i32 {
    let dy = direction(s).unwrap();
    let ahead = at(w, shift(p, dy));
    let behind = at(w, shift(p, -dy));
    let thickness = if direction(ahead) == Some(-dy) { "tip_merge" }
        else if direction(ahead) != Some(dy) { "tip" }
        else if matches!(prop(ahead, "thickness"), Some("tip" | "tip_merge")) { "frustum" }
        else if direction(behind) == Some(dy) { "middle" } else { "base" };
    blocks::orientation_of(s).unwrap().with(s, "thickness", thickness).unwrap()
}

pub fn placed(w: &World, p: Pos, s: i32, face: i32) -> i32 {
    let rule = blocks::orientation_of(s).unwrap();
    let mut next = rule.with(s, "vertical_direction", if face == 0 { "down" } else { "up" }).unwrap();
    let water = matches!(fluids::fluid_at(at(w, p)), Some((fluids::Kind::Water, 0)));
    next = rule.with(next, "waterlogged", if water { "true" } else { "false" }).unwrap();
    shaped(w, p, next)
}

pub fn changed(w: &mut World, p: Pos) {
    let s = at(w, p);
    if !supported(w, p, s) { w.schedule_block_once(p.0, p.1, p.2, 2); }
    let next = shaped(w, p, s);
    if next != s { w.set_block(p.0, p.1, p.2, next); }
}

pub fn ticked(w: &mut World, p: Pos) {
    let s = at(w, p);
    if supported(w, p, s) { return; }
    if direction(s) == Some(-1) {
        let mut next = p;
        while direction(at(w, next)) == Some(-1) {
            w.note_falling(next.0, next.1, next.2);
            next.1 -= 1;
        }
    } else {
        w.note_destroyed(p.0, p.1, p.2);
        w.set_block(p.0, p.1, p.2, if prop(s, "waterlogged") == Some("true") { blocks::state_by_name("water").unwrap() } else { AIR });
    }
}

/// Только корень сталактита выбирает случайный такт для всей капли.
fn tip(w: &World, root: Pos) -> Option<Pos> {
    for depth in 0..11 {
        let p = shift(root, -depth);
        let state = at(w, p);
        if direction(state) != Some(-1) { return None; }
        if prop(state, "thickness") == Some("tip") {
            return (prop(state, "waterlogged") == Some("false")).then_some(p);
        }
    }
    None
}

fn fluid_above_tip(w: &World, tip: Pos) -> Option<fluids::Kind> {
    for length in 1..=11 {
        let p = shift(tip, length);
        if direction(at(w, p)) == Some(-1) { continue; }
        if !blocks::full_cube(at(w, p)) { return None; }
        return fluids::fluid_at(at(w, shift(p, 1))).filter(|(_, level)| *level == 0).map(|(kind, _)| kind);
    }
    None
}

fn filled(s: i32, kind: fluids::Kind) -> Option<i32> {
    match (blocks::block_at_state(s)?, kind) {
        ("cauldron", fluids::Kind::Lava) => blocks::state_by_name("lava_cauldron"),
        ("cauldron", fluids::Kind::Water) => blocks::orientation("water_cauldron")?.state(&[("level", "1")]),
        ("water_cauldron", fluids::Kind::Water) => {
            let level = prop(s, "level")?.parse::<u8>().ok()?;
            if level >= 3 { return None; }
            blocks::orientation_of(s)?.with(s, "level", &(level + 1).to_string())
        }
        _ => None,
    }
}

pub fn random_tick(w: &mut World, p: Pos, s: i32) {
    if direction(s) != Some(-1) || direction(at(w, shift(p, 1))) == Some(-1) { return; }
    let Some(tip) = tip(w, p) else { return; };
    let source = shift(p, 2);
    if !blocks::full_cube(at(w, shift(p, 1))) { return; }
    // Mud: в Незере высушивание капельником не происходит.
    if blocks::block_at_state(at(w, source)) == Some("mud") && !w.is_nether() {
        if w.random_unit() < 45.0 / 256.0 { w.set_block(source.0, source.1, source.2, blocks::state_by_name("clay").unwrap()); }
        return;
    }
    let Some(kind) = fluid_above_tip(w, tip) else { return; };
    if w.random_unit() >= if kind == fluids::Kind::Water { 45.0 / 256.0 } else { 15.0 / 256.0 } { return; }
    for distance in 1..=11 {
        let target = shift(tip, -distance);
        let state = at(w, target);
        if filled(state, kind).is_some() {
            w.schedule_block_once(target.0, target.1, target.2, 50 + distance as u64);
            return;
        }
        if !matches!(blocks::block_at_state(state), Some("air" | "cave_air" | "void_air")) { return; }
    }
}

/// К моменту капли источник/сталактит мог исчезнуть: проверяем заново.
pub fn fill_cauldron(w: &mut World, p: Pos) {
    for distance in 1..=11 {
        let above = shift(p, distance);
        let s = at(w, above);
        if direction(s) == Some(-1) && prop(s, "thickness") == Some("tip") && prop(s, "waterlogged") == Some("false") {
            if let Some(kind) = fluid_above_tip(w, above)
                && let Some(next) = filled(at(w, p), kind)
            {
                w.set_block(p.0, p.1, p.2, next);
                w.note_level_event(p.0, p.1, p.2, if kind == fluids::Kind::Water { 1047 } else { 1046 }, 0);
            }
            return;
        }
        if !matches!(blocks::block_at_state(s), Some("air" | "cave_air" | "void_air")) { return; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(n: &str) -> i32 { blocks::state_from_text(n).unwrap() }
    fn rig(w: &mut World, source: &str) {
        w.set_block_silently(0, 10, 0, state("cauldron"));
        w.set_block_silently(0, 13, 0, state("pointed_dripstone[vertical_direction=down]"));
        w.set_block_silently(0, 14, 0, state("dripstone_block"));
        w.set_block_silently(0, 15, 0, state(source));
    }
    #[test]
    fn water_adds_three_levels_lava_fills_once_and_fluids_do_not_mix() {
        let mut w = World::in_memory();
        rig(&mut w, "water");
        for level in 1..=3 {
            fill_cauldron(&mut w, (0, 10, 0));
            assert_eq!(prop(w.get_block(0, 10, 0), "level"), Some(level.to_string().as_str()));
        }
        fill_cauldron(&mut w, (0, 10, 0));
        assert_eq!(prop(w.get_block(0, 10, 0), "level"), Some("3"));
        w.set_block_silently(0, 15, 0, state("lava"));
        fill_cauldron(&mut w, (0, 10, 0));
        assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some("water_cauldron"));
        rig(&mut w, "lava");
        fill_cauldron(&mut w, (0, 10, 0));
        assert_eq!(blocks::block_at_state(w.get_block(0, 10, 0)), Some("lava_cauldron"));
    }
    #[test]
    fn obstructed_removed_and_waterlogged_tips_cannot_fill() {
        let mut w = World::in_memory();
        rig(&mut w, "water");
        w.set_block_silently(0, 12, 0, state("stone"));
        fill_cauldron(&mut w, (0, 10, 0));
        assert_eq!(w.get_block(0, 10, 0), state("cauldron"));
        w.set_block_silently(0, 12, 0, AIR);
        w.set_block_silently(0, 15, 0, AIR);
        fill_cauldron(&mut w, (0, 10, 0));
        assert_eq!(w.get_block(0, 10, 0), state("cauldron"));
        rig(&mut w, "water");
        w.set_block_silently(0, 13, 0, state("pointed_dripstone[vertical_direction=down,waterlogged=true]"));
        fill_cauldron(&mut w, (0, 10, 0));
        assert_eq!(w.get_block(0, 10, 0), state("cauldron"));
    }
    #[test]
    fn random_ticks_schedule_a_real_delayed_fill_and_dry_mud() {
        let mut w = World::in_memory();
        rig(&mut w, "water");
        let s = w.get_block(0, 13, 0);
        for _ in 0..100 { random_tick(&mut w, (0, 13, 0), s); }
        assert!(w.scheduled_count() > 0);
        for _ in 0..52 {
            for (p, _) in w.advance(1000) { crate::redstone::ticked(&mut w, p); }
        }
        assert_eq!(w.get_block(0, 10, 0), state("cauldron"));
        for (p, _) in w.advance(1000) { crate::redstone::ticked(&mut w, p); }
        assert_eq!(prop(w.get_block(0, 10, 0), "level"), Some("1"));
        rig(&mut w, "mud");
        for _ in 0..100 { random_tick(&mut w, (0, 13, 0), s); }
        assert_eq!(w.get_block(0, 15, 0), state("clay"));
    }
    #[test]
    fn hanging_placement_direction_shape_and_falling_chain() {
        let mut w = World::in_memory();
        w.set_block_silently(0, 14, 0, state("stone"));
        let s = placed(&w, (0, 13, 0), state("pointed_dripstone"), 0);
        assert_eq!(prop(s, "vertical_direction"), Some("down"));
        assert!(supported(&w, (0, 13, 0), s));
        w.set_block_silently(0, 13, 0, s);
        w.set_block_silently(0, 12, 0, s);
        changed(&mut w, (0, 13, 0));
        assert_eq!(prop(at(&w, (0, 13, 0)), "thickness"), Some("frustum"));
        w.set_block_silently(0, 14, 0, AIR);
        ticked(&mut w, (0, 13, 0));
        assert_eq!(w.take_falling(), vec![(0, 13, 0), (0, 12, 0)]);
    }
}
