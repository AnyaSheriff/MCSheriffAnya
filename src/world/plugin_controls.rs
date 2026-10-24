// Управляемая доля руды: только новая генерация, без потребления общего RNG.
use super::{Chunk,MIN_Y,WORLD_HEIGHT};
use crate::blocks;

fn replacements() -> &'static std::collections::HashMap<i32,i32> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<i32,i32>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let mut map=std::collections::HashMap::new();
        for (ores,stone) in [
            (&["coal_ore","copper_ore","iron_ore","gold_ore","redstone_ore","lapis_ore","diamond_ore","emerald_ore"][..],"stone"),
            (&["deepslate_coal_ore","deepslate_copper_ore","deepslate_iron_ore","deepslate_gold_ore","deepslate_redstone_ore","deepslate_lapis_ore","deepslate_diamond_ore","deepslate_emerald_ore"][..],"deepslate"),
            (&["nether_gold_ore","nether_quartz_ore","ancient_debris"][..],"netherrack"),
        ] {
            let replacement=blocks::state_by_name(stone).expect("порода");
            for ore in ores {let (lo,hi)=blocks::states_of(ore).expect("руда");for state in lo..=hi {map.insert(state,replacement);}}
        }
        map
    })
}
fn roll(seed: i64,x: i32,y: i32,z: i32) -> u8 {
    let mut value=(seed as u64)^(x as u64).wrapping_mul(0x9e3779b97f4a7c15)^(y as u64).wrapping_mul(0xbf58476d1ce4e5b9)^(z as u64).wrapping_mul(0x94d049bb133111eb);
    value=(value^(value>>30)).wrapping_mul(0xbf58476d1ce4e5b9);value=(value^(value>>27)).wrapping_mul(0x94d049bb133111eb);((value^(value>>31))%100) as u8
}
pub(super) fn retain_ores(chunk: &mut Chunk,cx: i32,cz: i32,seed: i64,percent: u8) {
    if percent>=100 {return;}
    let replacements=replacements();let mut changed=false;
    for y in MIN_Y..MIN_Y+WORLD_HEIGHT {for z in 0..16 {for x in 0..16 {
        if let Some(&stone)=replacements.get(&chunk.block(x,y,z)) && roll(seed,cx*16+x,y,cz*16+z)>=percent {changed|=chunk.set(x,y,z,stone);}
    }}}
    if changed {chunk.light_up();}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn ore_retention_preserves_default_and_replaces_only_ores(){
        let mut chunk=Chunk::new();let stone=blocks::state_by_name("stone").unwrap();let ore=blocks::state_by_name("diamond_ore").unwrap();let debris=blocks::state_by_name("ancient_debris").unwrap();
        chunk.set(1,0,1,ore);chunk.set(2,0,1,debris);chunk.set(3,0,1,stone);
        retain_ores(&mut chunk,0,0,42,100);assert_eq!(chunk.block(1,0,1),ore);
        retain_ores(&mut chunk,0,0,42,0);assert_eq!(chunk.block(1,0,1),stone);assert_eq!(chunk.block(2,0,1),blocks::state_by_name("netherrack").unwrap());assert_eq!(chunk.block(3,0,1),stone);
    }
    #[test] fn ore_retention_is_deterministic_and_monotone(){
        let mut previous=0;for percent in [0,25,50,75,100] {let kept=(0..10000).filter(|x|roll(42,*x,-32,15)<percent).count();assert!(kept>=previous);assert!((kept as i32-percent as i32*100).abs()<250);previous=kept;}
    }
}
