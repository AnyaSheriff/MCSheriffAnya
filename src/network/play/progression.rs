// Привязанные к положению условия (вики, Advancement и Statistics).
// [проверить] точный такт выдачи: проверки позиции раз в 20 тактов.
use super::*;

#[derive(Default)]
pub(super) struct Location {
    nether_origin: Option<(f64, f64, f64)>,
    levitation_origin: Option<f64>,
    landed_from_world_height: bool,
}

pub(super) fn changed_dimension(player: &mut PlayerState, next: Dimension, at: (f64, f64, f64)) {
    if player.dimension == Dimension::Overworld && next == Dimension::Nether {
        player.progress_location.nether_origin = Some((player.x, player.y, player.z));
    } else if player.dimension == Dimension::Nether {
        if next == Dimension::Overworld
            && let Some(from) = player.progress_location.nether_origin
            && (at.0 - from.0).powi(2) + (at.1 - from.1).powi(2) + (at.2 - from.2).powi(2) >= 7000.0_f64.powi(2)
        {
            player.progress.trigger("nether/fast_travel");
        }
        player.progress_location.nether_origin = None;
    }
    player.progress_location.levitation_origin = None;
    player.progress_location.landed_from_world_height = false;
}

pub(super) fn landed(player: &mut PlayerState, fallen: f32) {
    // Проверяем выживание после обработки урона, включая спасение тотемом.
    if player.dimension == Dimension::Overworld && player.y <= -59.0
        && fallen > 379.0 && player.y + f64::from(fallen) >= 319.0
    {
        player.progress_location.landed_from_world_height = true;
    }
}

pub(super) fn effects(player: &mut PlayerState) {
    if std::mem::take(&mut player.progress_location.landed_from_world_height) && !player.vit.dead {
        player.progress.trigger("adventure/fall_from_world_height");
    }
    if player.vit.effects.iter().any(|e| e.kind == crate::survival::EffectKind::Levitation && e.ticks_left > 0) {
        let start = *player.progress_location.levitation_origin.get_or_insert(player.y);
        if (player.y - start).abs() >= 50.0 { player.progress.trigger("end/levitate"); }
    } else {
        player.progress_location.levitation_origin = None;
    }
}

pub(super) fn location(shared: &Shared, player: &mut PlayerState) {
    let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let (x, y, z) = (player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32);
    if player.sneaking && redstone::near_sculk_sensor(&world, (player.x, player.y, player.z)) { player.progress.trigger("adventure/avoid_vibration"); }
    player.progress.biome(player.dimension, world.mob_biome(x, y, z));
    if let Some(id) = world.progress_structure(x, y, z) { player.progress.trigger(id); }
    if player.on_ground && blocks::block_at_state(world.get_block(x, (player.y - 0.2).floor() as i32, z)) == Some("powder_snow")
        && player.inventory.slot(8).and_then(|s| blocks::item_name(s.item)) == Some("leather_boots")
    {
        player.progress.trigger("adventure/walk_on_powder_snow_with_leather_boots");
    }
}

/// Выдача только за установку участником, а не за нахождение рядом.
pub(super) fn placed(world: &World, player: &mut PlayerState, name: &str, at: (i32, i32, i32)) {
    if matches!(name, "comparator" | "chiseled_bookshelf") && redstone::reads_bookshelf(world, at) {
        player.progress.trigger("adventure/read_power_of_chiseled_bookshelf");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> PlayerState {
        let mut player = PlayerState::new([216; 16], "ProgressTest".to_string(), None, None, 0x7F, 11, (0.0, 64.0, 0.0));
        player.progress.stats = crate::stats::Stats::new();
        player.progress.adv = crate::advancements::AdvState::new();
        player
    }

    #[test]
    fn nether_travel_uses_the_two_overworld_endpoints() {
        for (distance, expected) in [(6999.0, false), (7000.0, true)] {
            let mut p = player();
            changed_dimension(&mut p, Dimension::Nether, (0.0, 64.0, 0.0));
            p.dimension = Dimension::Nether;
            changed_dimension(&mut p, Dimension::Overworld, (distance, 64.0, 0.0));
            assert_eq!(p.progress.adv.is_done("nether/fast_travel"), expected);
            assert!(p.progress_location.nether_origin.is_none());
        }
        let mut p = player();
        p.dimension = Dimension::Nether;
        changed_dimension(&mut p, Dimension::Overworld, (10000.0, 64.0, 0.0));
        assert!(!p.progress.adv.is_done("nether/fast_travel"));
    }

    #[test]
    fn levitation_distance_resets_when_effect_ends() {
        let mut p = player();
        p.vit.give_effect(crate::survival::EffectKind::Levitation, 0, 200);
        effects(&mut p);
        p.y += 49.0;
        effects(&mut p);
        assert!(!p.progress.adv.is_done("end/levitate"));
        p.vit.effects.clear();
        effects(&mut p);
        p.vit.give_effect(crate::survival::EffectKind::Levitation, 0, 200);
        effects(&mut p);
        p.y += 1.0;
        effects(&mut p);
        assert!(!p.progress.adv.is_done("end/levitate"));
        p.y += 49.0;
        effects(&mut p);
        assert!(p.progress.adv.is_done("end/levitate"));
    }

    #[test]
    fn world_height_fall_requires_landing_and_surviving() {
        for (y, distance, dead, dimension, expected) in [
            (-61.0, 380.0, false, Dimension::Overworld, true),
            (-60.0, 379.0, false, Dimension::Overworld, false),
            (-62.0, 380.0, false, Dimension::Overworld, false),
            (-61.0, 380.0, true, Dimension::Overworld, false),
            (-61.0, 380.0, false, Dimension::Nether, false),
        ] {
            let mut p = player();
            p.y = y;
            p.dimension = dimension;
            landed(&mut p, distance);
            assert!(!p.progress.adv.is_done("adventure/fall_from_world_height"));
            p.vit.dead = dead;
            effects(&mut p);
            assert_eq!(p.progress.adv.is_done("adventure/fall_from_world_height"), expected, "{y}/{distance}/{dead}");
            assert!(!p.progress_location.landed_from_world_height);
        }
    }
}
