// Вики, Chorus Fruit (ревизия до 26.1.2): «up to 16 attempts», ±8,
// поиск пола вниз и запрет жидкости. Полные collision shapes [проверить].
use crate::{world::World, mob::Rng};

pub fn chorus_destination(world: &World, at: (f64,f64,f64), rng: &mut Rng) -> Option<(f64,f64,f64)> {
    for _ in 0..16 {
        let x = at.0 + (rng.unit() - 0.5) * 16.0;
        let mut y = (at.1 + f64::from(rng.below(16)) - 8.0).floor().clamp(-63.0,318.0) as i32;
        let z = at.2 + (rng.unit() - 0.5) * 16.0;
        if !world.has_chunk((x.floor() as i32).div_euclid(16),(z.floor() as i32).div_euclid(16)) { continue; }
        while y > -63 && !crate::mob::solid(world.get_block(x.floor() as i32,y-1,z.floor() as i32)) { y-=1; }
        if y <= -63 { continue; }
        // Ящик игрока шириной 0,6 и высотой 1,8, включая соседние клетки.
        let mut clear = true;
        for bx in (x-0.3).floor() as i32..=(x+0.3).floor() as i32 {
            for by in y..=y+1 {
                for bz in (z-0.3).floor() as i32..=(z+0.3).floor() as i32 {
                    let s=world.get_block(bx,by,bz);
                    if crate::mob::solid(s) || crate::fluids::fluid_at(s).is_some() { clear=false; }
                }
            }
        }
        if clear { return Some((x,f64::from(y),z)); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chorus_lands_on_floor_within_range_and_refuses_liquid_or_void() {
        let mut w=World::in_memory();
        assert_eq!(chorus_destination(&w,(0.5,10.0,0.5),&mut Rng::new(1)),None);
        for x in -12..=12 { for z in -12..=12 { w.set_block_silently(x,0,z,crate::blocks::state_by_name("stone").unwrap()); } }
        let p=chorus_destination(&w,(0.5,10.0,0.5),&mut Rng::new(1)).unwrap();
        assert_eq!(p.1,1.0);assert!((p.0-0.5).abs()<=8.0 && (p.2-0.5).abs()<=8.0);
        for x in -12..=12 { for z in -12..=12 { w.set_block_silently(x,1,z,crate::blocks::state_by_name("water").unwrap()); } }
        assert_eq!(chorus_destination(&w,(0.5,10.0,0.5),&mut Rng::new(1)),None);
    }
}
