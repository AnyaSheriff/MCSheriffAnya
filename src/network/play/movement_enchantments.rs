// Чары обуви. Смена блока под неподвижным игроком сама по себе не включает
// Soul Speed: это подтвердил enchant-soul-speed на vanilla 26.1.2.
use super::*;
use crate::attributes::{MOVEMENT_EFFICIENCY, MOVEMENT_SPEED, Modifier, Operation};

const SOUL: &str = "minecraft:enchantment.soul_speed/feet";

use crate::frost::Body;

/// Снимок препятствий до захвата мира: порядок world → players → items
/// не обращается. Предметы и шарики опыта не препятствуют установке блока.
/// [проверить] Особые сущности и позы дополнительно к enchant-frost-collision.
fn frost_obstacles(shared: &Shared, player: &PlayerState) -> Vec<Body> {
    let mut bodies = vec![Body { at: (player.x, player.y, player.z), size: (0.6, if player.sneaking { 1.5 } else { 1.8 }) }];
    {
        let players = shared.players.lock().expect("игроки");
        for other in players.members() {
            if other.entity_id == player.entity_id || other.dimension != player.dimension || other.game_mode == GAME_MODE_SPECTATOR { continue; }
            let gear = players.gear(&other.uuid);
            if gear.view.dead { continue; }
            let height = if gear.sleeping.is_some() { 0.2 }
                else if gear.living & (crate::players::SPIN_POSE_BIT | crate::players::GLIDE_BIT) != 0 { 0.6 }
                else if other.sneaking { 1.5 } else { 1.8 };
            bodies.push(Body { at: (other.position.x, other.position.y, other.position.z), size: (0.6, height) });
        }
    }
    {
        let items = shared.items_in(player.dimension).lock().expect("сущности");
        for mob in items.mobs() {
            // enchant-frost-collision: корова после kill продолжает мешать
            // заморозке до исчезновения тела (повторное equip на +30).
            if matches!(mob.kind, crate::mob::Kind::ShulkerBullet | crate::mob::Kind::LeashKnot | crate::mob::Kind::EvokerFangs) { continue; }
            bodies.push(Body { at: (mob.x, mob.y, mob.z), size: mob.size() });
        }
        for cart in items.carts() { bodies.push(Body { at: (cart.x, cart.y, cart.z), size: cart.dims() }); }
    }
    bodies.retain(|body| (body.at.0 - player.x).abs() < 18.0 && (body.at.2 - player.z).abs() < 18.0);
    bodies
}

#[derive(Default)]
pub(super) struct State {
    cell: Option<(Dimension, i32, i32, i32)>,
    boots: Option<(i32, i32, i32)>,
    grounded: bool,
    allowed: bool,
    trail: crate::soul::Trail,
}

pub(super) fn update(shared: &Shared, player: &mut PlayerState) {
    let stack = player.inventory.slot(8);
    let soul = stack.map_or(0, |s| s.extras().level("soul_speed"));
    let frost = stack.map_or(0, |s| s.extras().level("frost_walker"));
    // Износ той же обуви не является повторным надеванием: иначе собственный
    // расход прочности запускал бы чары ещё раз, пока игрок стоит.
    let boots = stack.map(|s| (s.item, soul, frost));
    let cell = (player.dimension, player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32);
    let allowed = !player.vit.dead && player.game_mode != GAME_MODE_SPECTATOR && player.ride.vehicle.is_none();
    let changed = player.foot_enchants.cell != Some(cell) || player.foot_enchants.boots != boots
        || player.foot_enchants.grounded != player.on_ground || player.foot_enchants.allowed != allowed;
    trail(shared, player, allowed && soul > 0);
    if !changed { return; }
    player.foot_enchants.cell = Some(cell);
    player.foot_enchants.boots = boots;
    player.foot_enchants.grounded = player.on_ground;
    player.foot_enchants.allowed = allowed;

    let freeze = allowed && frost > 0 && player.on_ground && !player.abilities.flying;
    let obstacles = if freeze { frost_obstacles(shared, player) } else { Vec::new() };
    let on_soul = {
        let mut world = shared.world_in(player.dimension).lock().expect("мир");
        if freeze {
            crate::frost::freeze_with_collision(&mut world, (player.x, player.y, player.z), true, frost, player.entity_id,
                |block| obstacles.iter().any(|body| body.overlaps(block)));
        }
        // Вики, Soul Speed: песок и почва душ. Промежуточные покрытия и
        // сохранение бонуса в прыжке дополнительно проверяются чёрным ящиком.
        let below = world.get_block(cell.1, (player.y - 0.5).floor() as i32, cell.3);
        matches!(blocks::block_at_state(below), Some("soul_sand" | "soul_soil"))
    };
    let active = allowed && soul > 0 && (on_soul || !player.on_ground && player.attrs.get(MOVEMENT_SPEED)
        .is_some_and(|a| a.modifiers.iter().any(|m| m.id == SOUL)));
    if active {
        // enchant-soul-speed: уровни II/III дают точно эти f32 → f64 значения.
        let amount = crate::soul::bonus(soul);
        player.attrs.set_modifier(MOVEMENT_SPEED, Modifier::new(SOUL, amount, Operation::AddValue));
        player.attrs.set_modifier(MOVEMENT_EFFICIENCY, Modifier::new(SOUL, 1.0, Operation::AddValue));
        // pt.minecraft.wiki/w/Velocidade_das_almas: «4% de chance»
        // расхода прочности при шаге по душам.
        // Unbreaking и unbreakable проходят через общий расход брони.
        if on_soul && matches!(player.game_mode, GAME_MODE_SURVIVAL | GAME_MODE_ADVENTURE) && vitals::world_random(shared, player.dimension) < 0.04
            && let Some(stack) = stack
        {
            pvp::wear_slot(shared, player, 8, stack, 1, true);
        }
    } else {
        player.attrs.remove_modifier(MOVEMENT_SPEED, SOUL);
        player.attrs.remove_modifier(MOVEMENT_EFFICIENCY, SOUL);
    }
}

fn trail(shared: &Shared, player: &mut PlayerState, active: bool) {
    let at = (player.x, player.y, player.z);
    let active = active && player.on_ground && !player.abilities.flying && !player.awaiting_teleport;
    // После остановки vanilla хранит последнюю сетевую скорость до нового
    // пакета позиции (enchant-soul-effects: частицы +196,+201,+206,+211).
    let motion = (player.net.last_delta.0, player.net.last_delta.2);
    if let Some(motion) = player.foot_enchants.trail.step_motion(*shared.ticks.borrow(), motion, active) {
        let mut world = shared.world_in(player.dimension).lock().expect("мир");
        let below = world.get_block(at.0.floor() as i32, (at.1 - 0.5).floor() as i32, at.2.floor() as i32);
        if matches!(blocks::block_at_state(below), Some("soul_sand" | "soul_soil")) {
            crate::soul::particle(&mut world, at, 0.6, motion);
            if let Some(pitch) = crate::soul::sound_pitch(&mut world) {
                world.note_sound_player_at(at, "minecraft:particle.soul_escape", 0.6, pitch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frost_equipment_hook_respects_entity_collision_and_touching_edges() {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(Path::new("tmp/frost-collision-ops.json")), crate::skins::Settings::default());
        let mut p = PlayerState::new([28; 16], "FrostTest".into(), None, None, 0, 1, (3.5, 64.0, 3.5));
        p.game_mode = GAME_MODE_SURVIVAL;
        p.on_ground = true;
        {
            let mut world = shared.world_in(p.dimension).lock().unwrap();
            for x in 0..=7 { for z in 0..=7 {
                world.set_block_silently(x, 62, z, blocks::state_by_name("stone").unwrap());
                world.set_block_silently(x, 63, z, fluids::state_of(fluids::Kind::Water, 0));
            } }
            world.set_block_silently(3, 63, 3, blocks::state_by_name("stone").unwrap());
        }
        shared.items_in(p.dimension).lock().unwrap().add_mob(crate::mob::Mob::new(2, crate::mob::Kind::Cow, (4.5, 63.5, 3.5)));
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("frost_walker").unwrap(), 2);
        let boots = Stack::new(blocks::item_named("diamond_boots").unwrap(), 1).with_extras(data);
        p.inventory.set(8, Some(boots));
        update(&shared, &mut p);
        {
            let world = shared.world_in(p.dimension).lock().unwrap();
            assert_eq!(blocks::block_at_state(world.get_block(4, 63, 3)), Some("water"));
            assert_eq!(blocks::block_at_state(world.get_block(3, 63, 4)), Some("frosted_ice"));
        }
        // Убитая корова всё ещё занимает место до удаления тела.
        shared.items_in(p.dimension).lock().unwrap().mobs_mut()[0].vitals.dead = true;
        p.inventory.set(8, None);
        update(&shared, &mut p);
        p.inventory.set(8, Some(boots));
        update(&shared, &mut p);
        assert_eq!(blocks::block_at_state(shared.world_in(p.dimension).lock().unwrap().get_block(4, 63, 3)), Some("water"));
        // Пересечение на высоте ног блокирует заморозку, касание верхней
        // грани не блокирует. Повторное надевание обходит неподвижность.
        shared.items_in(p.dimension).lock().unwrap().mobs_mut()[0].y = 64.0;
        p.inventory.set(8, None);
        update(&shared, &mut p);
        p.inventory.set(8, Some(boots));
        update(&shared, &mut p);
        assert_eq!(blocks::block_at_state(shared.world_in(p.dimension).lock().unwrap().get_block(4, 63, 3)), Some("frosted_ice"));
        assert!(!Body { at: (4.5, 63.5, 3.5), size: (0.0, 0.0) }.overlaps((4, 63, 3)));
    }

    #[test]
    fn soul_speed_tracks_steps_and_equipment_without_polling_stationary_floor() {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(Path::new("target/soul-speed-ops.json")), crate::skins::Settings::default());
        let mut p = PlayerState::new([27; 16], "SoulTest".into(), None, None, 0, 1, (3.5, 64.0, 3.5));
        p.game_mode = GAME_MODE_CREATIVE;
        p.on_ground = true;
        let boots = |level| {
            let mut data = crate::stackdata::Data::default();
            data.set_enchant(crate::stackdata::enchantment_number("soul_speed").unwrap(), level);
            Stack::new(blocks::item_named("diamond_boots").unwrap(), 1).with_extras(data)
        };
        p.inventory.set(8, Some(boots(1)));
        update(&shared, &mut p);
        let base = p.attrs.value(MOVEMENT_SPEED);
        shared.world_in(p.dimension).lock().unwrap().set_block_silently(3, 63, 3, blocks::state_by_name("soul_soil").unwrap());
        update(&shared, &mut p);
        assert_eq!(p.attrs.value(MOVEMENT_SPEED), base);
        p.inventory.set(8, Some(boots(2)));
        update(&shared, &mut p);
        assert_eq!(p.attrs.value(MOVEMENT_SPEED), base + 0.050999999046325684);
        assert_eq!(p.attrs.value(MOVEMENT_EFFICIENCY), 1.0);
        p.inventory.set(8, Some(boots(3)));
        update(&shared, &mut p);
        assert_eq!(p.attrs.value(MOVEMENT_SPEED), base + 0.06149999797344208);
        p.x += 1.0;
        update(&shared, &mut p);
        assert_eq!(p.attrs.value(MOVEMENT_SPEED), base);
        assert_eq!(p.attrs.value(MOVEMENT_EFFICIENCY), 0.0);
        p.x -= 1.0;
        update(&shared, &mut p);
        // Возрождение заново создаёт атрибуты. Даже в той же клетке с
        // keepInventory зачарование должно восстановить свой бонус.
        vitals::respawned(&mut p);
        update(&shared, &mut p);
        assert_eq!(p.attrs.value(MOVEMENT_SPEED), base + 0.06149999797344208);
        p.inventory.set(8, None);
        update(&shared, &mut p);
        assert_eq!(p.attrs.value(MOVEMENT_SPEED), base);
    }
}
