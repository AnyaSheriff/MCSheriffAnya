// Общие числа Soul Speed: чёрный ящик enchant-soul-speed и walk,
// vanilla 26.1.2. Значение сначала вычисляется в f32, затем передаётся f64.
pub const ID: &str = "minecraft:enchantment.soul_speed/feet";
pub fn bonus(level: i32) -> f64 { if level > 0 { f64::from(0.03f32 + 0.0105f32 * level as f32) } else { 0.0 } }

/// Посимвольных/сетевых вызовов движения может быть несколько за такт.
/// Эффект появляется не чаще одного раза за пять тактов жизни сущности.
#[derive(Clone, Debug, Default)]
pub struct Trail {
    last_at: Option<(f64, f64, f64)>,
    emitted: Option<u64>,
}

impl Trail {
    pub fn step(&mut self, age: u64, at: (f64, f64, f64), active: bool) -> Option<(f64, f64)> {
        let from = self.last_at.replace(at)?;
        self.step_motion(age, (at.0 - from.0, at.2 - from.2), active)
    }

    pub fn step_motion(&mut self, age: u64, motion: (f64, f64), active: bool) -> Option<(f64, f64)> {
        let (dx, dz) = motion;
        // enchant-soul-effects: 21,26,31,36,41. Телепорт не является
        // шагом; точный нижний порог движения [проверить].
        if !active || !age.is_multiple_of(5) || self.emitted == Some(age)
            || dx * dx + dz * dz <= 1.0e-7 || dx * dx + dz * dz > 16.0
        { return None; }
        self.emitted = Some(age);
        Some((dx, dz))
    }
}

/// След души: направление совпадает с записанным soul packet (count=0).
pub fn particle(world: &mut crate::world::World, at: (f64, f64, f64), width: f64, motion: (f64, f64)) {
    let x = at.0 + (f64::from(world.random_unit()) - 0.5) * width;
    let z = at.2 + (f64::from(world.random_unit()) - 0.5) * width;
    world.note_particle(41, None, (x, at.1 + 0.1, z),
        ((-motion.0 * 0.2) as f32, 0.1, (-motion.1 * 0.2) as f32), 1.0, 0);
}

/// enchant-soul-effects: 12 звуков на 35 частиц, volume=.6,
/// pitch=.6245..9888. Шанс 1/3 и границы .6..1 — оценка, [проверить]
/// более длинной выборкой; это не доказательство точной вероятности.
pub fn sound_pitch(world: &mut crate::world::World) -> Option<f32> {
    (world.random_unit() < 1.0 / 3.0).then(|| 0.6 + world.random_unit() * 0.4)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn soul_particle_preserves_directed_packet_and_foot_height() {
        let mut world = crate::world::World::in_memory();
        let start = world.watch_events(0);
        particle(&mut world, (1.0, 64.0, 3.0), 0.6, (0.2, -0.1));
        let (events, _) = world.events_since(start, 0);
        assert_eq!(events.len(), 1);
        match events[0] {
            crate::world::WorldEvent::Particle { kind, at, offset, speed, count, .. } => {
                assert_eq!((kind, speed, count), (41, 1.0, 0));
                assert_eq!(at.1, 64.1);
                assert!((0.7..=1.3).contains(&at.0) && (2.7..=3.3).contains(&at.2));
                assert_eq!(offset, (-0.04, 0.1, 0.02));
            }
            _ => panic!("ожидалась частица души"),
        }
    }

    #[test]
    fn soul_trail_needs_motion_and_emits_once_per_cadence() {
        let mut trail = Trail::default();
        assert_eq!(trail.step(0, (0.0, 64.0, 0.0), true), None);
        assert_eq!(trail.step(1, (0.2, 64.0, 0.0), true), None);
        assert_eq!(trail.step(5, (0.4, 64.0, 0.0), true), Some((0.2, 0.0)));
        assert_eq!(trail.step(5, (0.6, 64.0, 0.0), true), None);
        assert_eq!(trail.step(10, (0.6, 64.0, 0.0), true), None);
        assert_eq!(trail.step(15, (0.8, 64.0, 0.0), false), None);
        assert_eq!(trail.step(20, (100.0, 64.0, 0.0), true), None);
    }
}
