// Чары обуви мобов и стоек. World/Items уже принадлежат такту: здесь нет
// захвата Shared или новых замков. Список коллизий снимается до изменения.
use super::{Mob, Kind};
use crate::{blocks, frost, items::Items, inventory::Stack, world::World};

#[derive(Clone, Debug, Default)]
pub struct State {
    cell: Option<(i32, i32, i32)>,
    boots: Option<(i32, i32, i32)>,
    grounded: bool,
    allowed: bool,
    pub soul: i32,
    trail: crate::soul::Trail,
}

pub fn boots(mob: &Mob) -> Option<Stack> { super::equipment::slot(mob, 2) }

pub fn step(world: &mut World, items: &mut Items, players: &[super::ai::Watcher], riders: &[i32]) {
    if !items.mobs().iter().any(|mob| boots(mob).is_some() || mob.foot_enchants.soul > 0) { return; }
    let mut obstacles: Vec<frost::Body> = items.mobs().iter()
        .filter(|mob| !matches!(mob.kind, Kind::ShulkerBullet | Kind::LeashKnot | Kind::EvokerFangs))
        .map(|mob| frost::Body { at: (mob.x, mob.y, mob.z), size: mob.size() }).collect();
    obstacles.extend(items.carts().iter().map(|cart| frost::Body { at: (cart.x, cart.y, cart.z), size: cart.dims() }));
    obstacles.extend(players.iter().filter(|p| !p.spectator)
        .map(|p| frost::Body { at: (p.x, p.y, p.z), size: (0.6, if p.eye < 1.5 { 1.5 } else { 1.8 }) }));
    for mob in items.mobs_mut() { update(world, mob, &obstacles, riders.contains(&mob.entity_id)); }
}

fn update(world: &mut World, mob: &mut Mob, obstacles: &[frost::Body], riding: bool) {
    let stack = boots(mob);
    let soul = stack.map_or(0, |s| s.extras().level("soul_speed"));
    let frost = stack.map_or(0, |s| s.extras().level("frost_walker"));
    let boots = stack.map(|s| (s.item, soul, frost));
    let cell = mob.cell();
    let allowed = !mob.is_dead() && !riding && !mob.kind.object();
    let below = world.get_block(cell.0, (mob.y - 0.5).floor() as i32, cell.2);
    let on_soul = matches!(blocks::block_at_state(below), Some("soul_sand" | "soul_soil"));
    let at = (mob.x, mob.y, mob.z);
    if let Some(motion) = mob.foot_enchants.trail.step(mob.ticks, at, allowed && soul > 0 && mob.on_ground && on_soul) {
        crate::soul::particle(world, at, mob.size().0, motion);
        if let Some(pitch) = crate::soul::sound_pitch(world) {
            mob.events.push(super::Event::Sound { name: "minecraft:particle.soul_escape", volume: 0.6, pitch });
        }
    }
    let old = &mob.foot_enchants;
    if old.cell == Some(cell) && old.boots == boots && old.grounded == mob.on_ground && old.allowed == allowed { return; }
    let was_soul = old.soul;
    if allowed && frost > 0 && mob.on_ground {
        frost::freeze_with_collision(world, (mob.x, mob.y, mob.z), true, frost, mob.entity_id,
            |p| obstacles.iter().any(|body| body.overlaps(p)));
    }
    let active = if allowed && soul > 0 && (on_soul || !mob.on_ground && was_soul > 0) { soul } else { 0 };
    mob.foot_enchants.cell = Some(cell);
    mob.foot_enchants.boots = boots;
    mob.foot_enchants.grounded = mob.on_ground;
    mob.foot_enchants.allowed = allowed;
    mob.foot_enchants.soul = active;
    if active != was_soul { mob.attr_changes = mob.attr_changes.wrapping_add(1); }
    // Вики, Soul Speed: «4% chance» расхода обуви при переходе в клетку.
    // [проверить] Износ при надевании и смене высоты отдельной выборкой.
    if active > 0 && on_soul && world.random_unit() < 0.04 && let Some(stack) = stack {
        let wear = crate::combat::wear(&stack, 1, true, || world.random_unit());
        if wear > 0 { super::equipment::set_slot(mob, 2, crate::combat::add_damage(stack, wear)); }
    }
}

/// enchant-soul-mobs: steady Δz за три такта stone/soil=1431..1432,
/// sand=831..832 единиц 1/4096. Множитель действует после сдвига,
/// вместе с трением; Soul III снимает его (soil/sand=2299..2300).
pub fn floor_drag(mob: &Mob, view: &dyn super::View) -> f64 {
    let below = view.block(mob.x.floor() as i32, (mob.y - 0.5).floor() as i32, mob.z.floor() as i32);
    if mob.foot_enchants.soul == 0 && blocks::block_at_state(below) == Some("soul_sand") { 0.4 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn enchanted(name: &str) -> Stack {
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), 2);
        Stack::new(blocks::item_named("diamond_boots").unwrap(), 1).with_extras(data)
    }

    #[test]
    fn stand_frost_freezes_on_equip_but_not_airborne_or_riding() {
        let mut world = World::in_memory();
        for x in 0..=8 { for z in 0..=8 {
            world.set_block_silently(x, 63, z, blocks::state_by_name("water").unwrap());
        } }
        let mut stand = Mob::new(17, Kind::ArmorStand, (4.5, 64.0, 4.5));
        super::super::equipment::set_slot(&mut stand, 2, Some(enchanted("frost_walker")));
        update(&mut world, &mut stand, &[], false);
        assert_eq!(blocks::block_at_state(world.get_block(4, 63, 4)), Some("water"));
        stand.on_ground = true;
        update(&mut world, &mut stand, &[], true);
        assert_eq!(blocks::block_at_state(world.get_block(4, 63, 4)), Some("water"));
        update(&mut world, &mut stand, &[], false);
        assert_eq!(blocks::block_at_state(world.get_block(4, 63, 4)), Some("frosted_ice"));
    }

    #[test]
    fn zombie_soul_boost_tracks_equipment_ground_and_death() {
        let mut world = World::in_memory();
        world.set_block_silently(4, 63, 4, blocks::state_by_name("soul_sand").unwrap());
        let mut mob = Mob::new(18, Kind::Zombie, (4.5, 64.0, 4.5));
        mob.on_ground = true;
        let base = mob.movement_speed();
        super::super::equipment::set_slot(&mut mob, 2, Some(enchanted("soul_speed")));
        update(&mut world, &mut mob, &[], false);
        assert_eq!(mob.movement_speed(), base + crate::soul::bonus(2));
        mob.on_ground = false; mob.y += 1.0;
        update(&mut world, &mut mob, &[], false);
        assert_eq!(mob.foot_enchants.soul, 2);
        mob.on_ground = true;
        update(&mut world, &mut mob, &[], false);
        assert_eq!(mob.movement_speed(), base);
        mob.y -= 1.0;
        update(&mut world, &mut mob, &[], false);
        mob.vitals.dead = true;
        update(&mut world, &mut mob, &[], false);
        assert_eq!(mob.movement_speed(), base);
    }

    #[test]
    fn measured_zombie_stride_on_sand_soil_and_stone_with_soul_speed() {
        struct Floor(i32);
        impl super::super::View for Floor {
            fn block(&self, _: i32, y: i32, _: i32) -> i32 { if y == 63 { self.0 } else { crate::world::AIR } }
            fn loaded(&self, _: i32, _: i32) -> bool { true }
        }
        // Запись vanilla, установившиеся пакеты движения после разгона.
        for (floor, level, expected) in [("stone", 0, 1432.0), ("soul_soil", 0, 1432.0),
            ("soul_sand", 0, 832.0), ("stone", 3, 1432.0),
            ("soul_soil", 3, 2300.0), ("soul_sand", 3, 2300.0)]
        {
            let view = Floor(blocks::state_by_name(floor).unwrap());
            let mut mob = Mob::new(30, Kind::Zombie, (0.5, 64.0, 0.5));
            mob.on_ground = true;
            mob.foot_enchants.soul = if floor == "stone" { 0 } else { level };
            for _ in 0..60 { mob.physics(&view, 1.0, false); }
            let before = mob.z;
            for _ in 0..3 { mob.physics(&view, 1.0, false); }
            let measured = (mob.z - before) * 4096.0;
            assert!((measured - expected).abs() < 1.0, "{floor}, Soul{level}: {measured}");
        }
    }

    #[test]
    fn naturally_fireproof_mobs_and_fire_resistance_ignore_hot_blocks() {
        use crate::survival::{Effect, EffectKind, Source};
        for kind in [Kind::Blaze, Kind::Ghast, Kind::MagmaCube, Kind::Strider, Kind::WitherSkeleton,
            Kind::ZombifiedPiglin, Kind::Wither, Kind::EnderDragon, Kind::Zoglin, Kind::Warden]
        {
            let mut mob = Mob::new(20, kind, (0.5, 64.0, 0.5));
            let health = mob.vitals.health;
            for source in [Source::Campfire, Source::HotFloor, Source::Lava] {
                assert!(mob.hurt(2.0, source, None).is_none(), "{kind:?} {source:?}");
                assert_eq!(mob.vitals.health, health);
            }
        }
        let mut mob = Mob::new(21, Kind::Cow, (0.5, 64.0, 0.5));
        mob.vitals.give_effect_shown(Effect { kind: EffectKind::FireResistance, amplifier: 0, ticks_left: 100,
            ambient: false, visible: true, show_icon: true });
        assert!(mob.hurt(2.0, Source::HotFloor, None).is_none());
    }

    #[test]
    fn frost_mob_boots_block_hot_floor_and_campfire_without_blocking_lava() {
        let mut mob = Mob::new(19, Kind::Zombie, (0.5, 64.0, 0.5));
        super::super::equipment::set_slot(&mut mob, 2, Some(enchanted("frost_walker")));
        for source in [crate::survival::Source::HotFloor, crate::survival::Source::Campfire] {
            assert!(mob.hurt(20.0, source, None).is_none());
            assert_eq!(mob.vitals.health, 20.0);
        }
        assert!(mob.hurt(2.0, crate::survival::Source::Lava, None).is_some());
    }
}
