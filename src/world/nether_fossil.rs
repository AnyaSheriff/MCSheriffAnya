// Окаменелости Незера: 14 форм из сохранённых блоков vanilla-struct 26.1.2.
// Вики, Nether Fossil: «no blocks are replaced with coal or diamond ore».
// Structure set: spacing 2, separation 1, salt 14357921. Случай свой.
// Высоты рамок 33..111, состав и оси: tools/measure/structfix_measure.py.
use std::sync::OnceLock;
use super::template::{self, Cell, Orient, Template};
use super::nether::Nether;
use super::terrain::Biome;

pub const SPACING: i32 = 2;
const SALT: u64 = 14_357_921;

pub fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();
    LIBRARY.get_or_init(|| template::parse(include_str!("nether_fossil.txt")))
}

#[derive(Clone, Copy, Debug)]
pub struct Fossil {
    pub chunk: (i32, i32),
    pub origin: (i32, i32, i32),
    pub template: usize,
    pub orient: Orient,
}

pub fn start_in(nether: &Nether, rx: i32, rz: i32) -> Option<Fossil> {
    let (cx, cz) = template::attempt(nether.seed(), rx, rz, SPACING, 1, SALT);
    let dice = template::roll(nether.seed(), cx, cz, SALT);
    let (x, z) = (cx * 16 + ((dice >> 8) % 16) as i32, cz * 16 + ((dice >> 16) % 16) as i32);
    if nether.biome_at(x, z) != Biome::SoulSandValley { return None; }
    // Поиск открытого пола ниже случайной высоты. Верх диапазона выведен
    // из сохранённых рамок; распределение исходной высоты [проверить].
    let from = 33 + ((dice >> 24) % 80) as i32;
    let floor = (32..from).rev().find(|&y| nether.rock_at(x,y,z) && nether.open(x,y+1,z))?;
    Some(Fossil { chunk: (cx,cz), origin: (x,floor+1,z), template: (dice % 14) as usize,
        orient: Orient { turns: ((dice >> 40) % 4) as u8, mirror: false } })
}

#[cfg(not(seedmap_core))]
pub fn paint(fossil: &Fossil, cx: i32, cz: i32, put: &mut dyn FnMut(i32,i32,i32,i32)) {
    let t = &library()[fossil.template];
    for (a,y,b,cell) in t.cells() {
        let Cell::Block(i) = cell else { continue };
        let (dx,dz) = fossil.orient.offset(a,b,t.width,t.length);
        let (x,y,z) = (fossil.origin.0+dx,fossil.origin.1+y,fossil.origin.2+dz);
        if x.div_euclid(16) == cx && z.div_euclid(16) == cz {
            let text = template::orient_text(&t.texts[usize::from(i)],fossil.orient);
            put(x,y,z,crate::blocks::state_from_text(&text).expect("измеренный блок кости"));
        }
    }
}

#[cfg(not(seedmap_core))]
pub(super) fn build(nether: &Nether, chunk: &mut super::Chunk, cx: i32, cz: i32) {
    for rx in (cx-1).div_euclid(SPACING)..=cx.div_euclid(SPACING) {
        for rz in (cz-1).div_euclid(SPACING)..=cz.div_euclid(SPACING) {
            if let Some(f) = start_in(nether,rx,rz) {
                paint(&f,cx,cz,&mut |x,y,z,state| { chunk.put_generated(x-cx*16,y,z-cz*16,state,true); });
            }
        }
    }
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;
    #[test]
    fn measured_shapes_have_only_bones_and_fit_the_search_halo() {
        assert_eq!(library().len(),14);
        let counts: Vec<_> = library().iter().map(|t| {
            assert!(t.width<=16 && t.length<=16);
            assert!(t.texts.iter().all(|s| s.starts_with("bone_block[axis=")));
            t.cells().count()
        }).collect();
        assert_eq!(counts,[10,8,24,11,17,26,10,6,6,5,21,18,6,15]);
    }
    #[test]
    fn bones_cross_negative_chunk_edges_without_loss() {
        for template in 0..14 {
            for turns in 0..4 {
                let f = Fossil { chunk:(-2,-2),origin:(-17,60,-17),template,orient:Orient{turns,mirror:false} };
                let mut cells=std::collections::HashMap::new();
                for cx in -2..=0 { for cz in -2..=0 {
                    paint(&f,cx,cz,&mut |x,y,z,s| { assert!(cells.insert((x,y,z),s).is_none()); });
                }}
                assert_eq!(cells.len(),library()[template].cells().count());
            }
        }
    }
    #[test]
    fn starts_have_soul_valley_support_and_measured_height_range() {
        let n=Nether::new(100_554_032_945_340);
        let mut count=0;
        for rx in -16..16 { for rz in -16..16 {
            if let Some(f)=start_in(&n,rx,rz) {
                count+=1;
                let (x,y,z)=f.origin;
                assert_eq!(f.chunk,(rx*2,rz*2));
                assert_eq!(n.biome_at(x,z),Biome::SoulSandValley);
                assert!((33..=112).contains(&y));
                assert!(n.rock_at(x,y-1,z) && n.open(x,y,z));
            }
        }}
        assert!(count>0);
    }
}
