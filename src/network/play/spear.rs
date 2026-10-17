// Рывок копья «Порыв» (Lunge), Java Player Action 7.
// Собственные чёрные ящики 26.1.2: enchant-lunge-probe/conditions/
// cost-motion/cooldown. Клиент отдельно присылает атаку сущности: сам
// status7 не ищет жертв и не наносит дополнительный урон.

use super::*;

/// Наземный, ледяной и воздушный замеры дают один горизонтальный импульс:
/// после сетевого квантования 0.45797473, 0.91601050, 1.37398523.
/// Наклон 60° уменьшает его вдвое, вертикальный взгляд обнуляет.
/// [проверить] Сложение с горизонтальной скоростью бегущего игрока.
const IMPULSE_PER_LEVEL: f64 = 0.458;

fn impulse(player: &PlayerState, level: i32) -> (f64, f64, f64) {
    let yaw = f64::from(player.yaw).to_radians();
    let pitch = f64::from(player.pitch).to_radians();
    let boost = IMPULSE_PER_LEVEL * f64::from(level) * pitch.cos();
    let old = player.net.last_delta;
    let vertical = if player.on_ground { 0.0 } else { old.1 };
    (old.0 - yaw.sin() * boost,
        (vertical - player.attrs.value(crate::attributes::GRAVITY)) * 0.98,
        old.2 + yaw.cos() * boost)
}

/// cost-motion при difficulty normal: I/II/III снимают 1/2/3 насыщенности,
/// остаток exhaustion не меняется: четыре единицы за уровень.
fn exhaustion(level: i32) -> f32 { 4.0 * level as f32 }

pub(super) fn jab(shared: &Shared, player: &mut PlayerState) -> bool {
    if player.vit.dead || player.game_mode == GAME_MODE_SPECTATOR || player.ride.vehicle.is_some()
        || player.net.in_water || player.net.in_lava { return false; }
    let Some(stack) = player.inventory.held() else { return false; };
    if !blocks::item_name(stack.item).is_some_and(|name| name.ends_with("_spear")) { return false; }
    let level = stack.extras().level("lunge");
    if level <= 0 { return false; }
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    // food-threshold (normal, LungeI): еда7 разрешает, еда6 запрещает.
    // food-drain (LungeIII) также прекращает рывки при еде4.
    if !creative && player.vit.food <= 6 { return false; }
    let now = *shared.ticks.borrow();
    vitals::update_equipment(player, now);
    let since = u32::try_from(now.saturating_sub(player.last_attack)).unwrap_or(u32::MAX);
    // threshold: железное копьё I/III отвергает 10–13 тактов и принимает 14;
    // cooldown: при attack_speed base10 достаточно 3 тактов.
    // [проверить] Промежуточные скорости вне этих двух замеров.
    if combat::charge(since, player.attrs.value(crate::attributes::ATTACK_SPEED)) < 0.75 { return false; }

    player.last_attack = now;
    player.net.velocity = Some(impulse(player, level));
    player.net.velocity_due = false;
    let sound = match level {
        1 => "minecraft:item.spear.lunge_1",
        2 => "minecraft:item.spear.lunge_2",
        _ => "minecraft:item.spear.lunge_3",
    };
    shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x, player.y, player.z), sound, 1.0, 1.0);
    if !creative {
        player.vit.add_exhaustion(exhaustion(level));
        let slot = player.inventory.selected_slot();
        pvp::wear_slot(shared, player, slot, stack, 1, false);
    }
    player.progress.used(stack.item);
    publish_player(shared, player);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(level: i32) -> (Shared, PlayerState) {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(Path::new("tmp/lunge/ops.json")), crate::skins::Settings::default());
        shared.ticks.send_replace(100);
        let mut player = PlayerState::new([72; 16], "LungeTest".into(), None, None, 0, 72, (0.5, 64.0, 0.5));
        player.game_mode = GAME_MODE_SURVIVAL;
        player.on_ground = true;
        player.yaw = 180.0;
        player.pitch = 0.0;
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("lunge").unwrap(), level);
        player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("iron_spear").unwrap(), 1).with_extras(data)));
        (shared, player)
    }

    #[test]
    fn measured_levels_boost_wear_and_exhaustion() {
        for (level, horizontal, food_cost) in [(1, -0.458, 4.0), (2, -0.916, 8.0), (3, -1.374, 12.0)] {
            let (shared, mut player) = setup(level);
            player.vit.exhaustion = 1.0;
            player.net.velocity_due = true;
            assert!(jab(&shared, &mut player));
            let velocity = player.net.velocity.unwrap();
            assert!(velocity.0.abs() < 1e-12);
            assert!((velocity.1 + 0.0784).abs() < 1e-9);
            assert!((velocity.2 - horizontal).abs() < 1e-9);
            assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
            assert_eq!(player.vit.exhaustion, 1.0 + food_cost);
            assert_eq!(player.last_attack, 100);
            assert!(!player.net.velocity_due);
        }
    }

    #[test]
    fn measured_pitch_projection_and_airborne_gravity() {
        for (pitch, horizontal) in [(60.0, -0.687), (90.0, 0.0)] {
            let (shared, mut player) = setup(3);
            player.pitch = pitch;
            assert!(jab(&shared, &mut player));
            let velocity = player.net.velocity.unwrap();
            assert!((velocity.2 - horizontal).abs() < 1e-9);
            assert!((velocity.1 + 0.0784).abs() < 1e-9);
        }
        let (shared, mut player) = setup(3);
        player.on_ground = false;
        player.net.last_delta = (0.0, -0.0784, 0.0);
        assert!(jab(&shared, &mut player));
        let velocity = player.net.velocity.unwrap();
        assert!((velocity.1 + 0.155232).abs() < 1e-9);
        assert!((velocity.2 + 1.374).abs() < 1e-9);
    }

    #[test]
    fn measured_cooldown_rejections_do_not_spend_or_restart_it() {
        for level in [1, 3] {
            let (shared, mut player) = setup(level);
            assert!(jab(&shared, &mut player));
            let exhaustion = player.vit.exhaustion;
            for elapsed in 1..14 {
                shared.ticks.send_replace(100 + elapsed);
                assert!(!jab(&shared, &mut player));
                assert_eq!(player.last_attack, 100);
                assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
                assert_eq!(player.vit.exhaustion, exhaustion);
            }
            shared.ticks.send_replace(114);
            assert!(jab(&shared, &mut player));
            assert_eq!(player.inventory.held().unwrap().extras().damage, 2);
        }
        let (shared, mut player) = setup(1);
        player.attrs.set_base(crate::attributes::ATTACK_SPEED, 10.0);
        assert!(jab(&shared, &mut player));
        shared.ticks.send_replace(103);
        assert!(jab(&shared, &mut player));
    }

    #[test]
    fn measured_food_gate_rejects_without_consuming_the_action() {
        let (shared, mut player) = setup(3);
        player.vit.food = 6;
        let before = player.vit.exhaustion;
        assert!(!jab(&shared, &mut player));
        assert_eq!(player.last_attack, 0);
        assert_eq!(player.vit.exhaustion, before);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
        player.vit.food = 7;
        assert!(jab(&shared, &mut player));
    }

    #[test]
    fn creative_and_unbreakable_keep_the_spear() {
        for creative in [false, true] {
            let (shared, mut player) = setup(3);
            let stack = player.inventory.held().unwrap();
            let mut data = stack.extras();
            data.unbreakable = !creative;
            player.inventory.set(inventory::FIRST_HOTBAR, Some(stack.with_extras(data)));
            if creative { player.game_mode = GAME_MODE_CREATIVE; player.vit.food = 0; }
            let before = player.vit.exhaustion;
            assert!(jab(&shared, &mut player));
            assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
            assert_eq!(player.vit.exhaustion, before + if creative { 0.0 } else { 12.0 });
        }
    }

    #[test]
    fn rejects_wrong_item_offhand_mount_and_water_without_side_effects() {
        for case in 0..7 {
            let (shared, mut player) = setup(3);
            match case {
                0 => { let data = player.inventory.held().unwrap().extras(); player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("stick").unwrap(), 1).with_extras(data))); }
                1 => { let stack = player.inventory.held(); player.inventory.set(inventory::OFFHAND, stack); player.inventory.set(inventory::FIRST_HOTBAR, None); }
                2 => player.ride.vehicle = Some(81),
                3 => player.net.in_water = true,
                4 => player.game_mode = GAME_MODE_SPECTATOR,
                5 => player.vit.dead = true,
                _ => player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("iron_spear").unwrap(), 1))),
            }
            let stack = player.inventory.held();
            let exhaustion = player.vit.exhaustion;
            assert!(!jab(&shared, &mut player));
            assert_eq!(player.inventory.held(), stack);
            assert_eq!(player.vit.exhaustion, exhaustion);
            assert!(player.net.velocity.is_none());
        }
    }

    #[test]
    fn player_action_seven_works_far_from_origin_and_does_not_attack_mobs() {
        let (shared, mut player) = setup(3);
        shared.items_in(player.dimension).lock().unwrap().add_mob(crate::mob::Mob::new(73, crate::mob::Kind::Cow, (0.5, 64.0, -1.5)));
        // Status7, BlockPos(0,0,0), face0, sequence9. Нулевая позиция не
        // должна отвергаться проверкой дальности обычного копания.
        let payload = [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];
        let reaction = break_block(&payload, &mut player, &shared).unwrap();
        assert_eq!(reaction.acknowledge, Some(9));
        assert!(player.net.velocity.is_some());
        assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
        assert_eq!(shared.items_in(player.dimension).lock().unwrap().mobs()[0].vitals.health, 10.0);
    }
}
