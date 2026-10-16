// throw3-egg-hatch-wall.md:19–24: все 23 цыплёнка в (3,5; 3,8; 5,85),
// хотя яйцо бьёт (3,5; 4; 6). Центр освобождается от блока, ноги ниже на 0,2.
// throw3-egg-hatch-repeat: на полу ноги остаются на y=0.
use crate::world::World;

pub(super) fn position(world: &World, at: (f64, f64, f64)) -> (f64, f64, f64) {
    let point = [at.0, at.1, at.2];
    let radius = [0.15, 0.2, 0.15];
    let mut axes = point.map(|v| vec![v]);
    let mut obstacles = Vec::new();
    // Размер области освобождения подтверждён плоскими стеной и полом.
    // Углы и неполные блоки [проверить: throw4-egg-hatch-faces].
    for x in (at.0 - 0.5).floor() as i32..=(at.0 + 0.5).floor() as i32 {
        for y in (at.1 - 0.5).floor() as i32..=(at.1 + 0.5).floor() as i32 {
            for z in (at.2 - 0.5).floor() as i32..=(at.2 + 0.5).floor() as i32 {
                if !crate::mob::solid(world.get_block(x, y, z)) { continue; }
                let block = [f64::from(x), f64::from(y), f64::from(z)];
                let low = std::array::from_fn::<_, 3, _>(|i| block[i] - radius[i]);
                let high = std::array::from_fn::<_, 3, _>(|i| block[i] + 1.0 + radius[i]);
                for i in 0..3 {
                    for edge in [low[i], high[i]] {
                        if (edge - point[i]).abs() <= 0.5 { axes[i].push(edge); }
                    }
                }
                obstacles.push((low, high));
            }
        }
    }
    let mut best = None;
    for &x in &axes[0] { for &y in &axes[1] { for &z in &axes[2] {
        let candidate = [x, y, z];
        if obstacles.iter().any(|(lo, hi)| (0..3).all(|i| candidate[i] > lo[i] && candidate[i] < hi[i])) { continue; }
        let distance: f64 = (0..3).map(|i| (candidate[i] - point[i]).powi(2)).sum();
        if best.is_none_or(|(known, _)| distance < known) { best = Some((distance, candidate)); }
    } } }
    let centre = best.map_or(point, |(_, p)| p);
    (centre[0], centre[1] - radius[1], centre[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_wall_and_floor_placement() {
        let mut world = World::in_memory();
        let stone = crate::blocks::state_by_name("stone").unwrap();
        world.set_block_silently(3, 4, 6, stone);
        world.set_block_silently(3, 3, 6, stone);
        assert_eq!(position(&world, (3.5, 4.0, 6.0)), (3.5, 3.8, 5.85));
        world.set_block_silently(1, -1, 0, stone);
        assert_eq!(position(&world, (1.71396, 0.0, 0.74022)), (1.71396, 0.0, 0.74022));
    }
}
