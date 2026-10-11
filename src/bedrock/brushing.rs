// Луч кисти по общему миру. Вики, Brush: «10 ticks»; Java brush.rs:
// взмах на пятом такте, десять взмахов. [проверить] анимация клиента Bedrock.
use crate::world::World;
pub fn target(world: &World, eyes: (f32,f32,f32), rotation: (f32,f32)) -> Option<((i32,i32,i32),u8)> {
    let yaw = f64::from(rotation.0).to_radians(); let pitch = f64::from(rotation.1).to_radians();
    let ray = (-yaw.sin()*pitch.cos(), -pitch.sin(), yaw.cos()*pitch.cos());
    let origin = (f64::from(eyes.0), f64::from(eyes.1), f64::from(eyes.2));
    let mut prev = (origin.0.floor() as i32, origin.1.floor() as i32, origin.2.floor() as i32);
    for n in 1..=90 {
        let d = f64::from(n)*0.05;
        let at = ((origin.0+ray.0*d).floor() as i32,(origin.1+ray.1*d).floor() as i32,(origin.2+ray.2*d).floor() as i32);
        let state = world.get_block(at.0,at.1,at.2);
        if crate::archaeology::suspicious(state) {
            let moves = [(prev.1-at.1,ray.1.abs(),[0,1]),(prev.2-at.2,ray.2.abs(),[2,3]),(prev.0-at.0,ray.0.abs(),[4,5])];
            let face = moves.iter().filter(|m| m.0 != 0).max_by(|a,b| a.1.total_cmp(&b.1)).map_or(1,|m| if m.0 < 0 { m.2[0] } else { m.2[1] });
            return Some((at,face));
        }
        if crate::mob::solid(state) { return None; } prev = at;
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brush_cannot_pass_through_wall_or_reach_farther() {
        let mut w = World::in_memory();
        w.set_block(0,70,3,crate::blocks::state_by_name("suspicious_sand").unwrap());
        assert_eq!(target(&w,(0.5,70.5,0.5),(0.0,0.0)),Some(((0,70,3),2)));
        w.set_block(0,70,2,crate::blocks::state_by_name("stone").unwrap());
        assert_eq!(target(&w,(0.5,70.5,0.5),(0.0,0.0)),None);
        assert_eq!(target(&w,(0.5,70.5,-3.5),(0.0,0.0)),None);
    }
}
