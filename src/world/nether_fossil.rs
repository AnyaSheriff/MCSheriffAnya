// Окаменелости Незера: 14 форм из сохранённых блоков vanilla-struct 26.1.2.
// Вики, Nether Fossil: «no blocks are replaced with coal or diamond ore».
// Structure set: spacing 2, separation 1, salt 14357921. Поток выбора — 26.3.
// Высоты рамок 33..111, состав и оси: tools/measure/structfix_measure.py.
use std::sync::OnceLock;
use super::template::{self, Cell, Orient, Template};
use super::nether::{Nether, LegacyRandom};
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
    let mut random = LegacyRandom::for_chunk(nether.seed(),cx,cz);
    let (x,z) = (cx * 16 + random.below(16),cz * 16 + random.below(16));
    if nether.biome_at(x,z) != Biome::SoulSandValley { return None; }
    // Диапазон 32..125 и порядок бросков подтверждены всеми 119 записями
    // двух миров 26.3; высота проверяется по базовой колонке до карверов.
    let from = 32 + random.below(94);
    let floor = nether.fossil_floor_below(x,from,z)?;
    let orient = Orient { turns: random.below(4) as u8, mirror: false };
    // Библиотека отсортирована строками (1,10,11,...), выбор — числами 1..14.
    const NUMERIC: [usize;14] = [0,6,7,8,9,10,11,12,13,1,2,3,4,5];
    let template = NUMERIC[random.below(14) as usize];
    let t = &library()[template];
    // TPX/TPZ оригинала — ось вращения; наш Orient ждёт меньший угол рамки.
    let (dx,dz) = match orient.turns {
        0 => (0,0), 1 => (t.length-1,0), 2 => (t.width-1,t.length-1), _ => (0,t.width-1),
    };
    Some(Fossil { chunk:(cx,cz), origin:(x-dx,floor+1,z-dz), template, orient })
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
    for rx in (cx-1).div_euclid(SPACING)..=(cx+1).div_euclid(SPACING) {
        for rz in (cz-1).div_euclid(SPACING)..=(cz+1).div_euclid(SPACING) {
            if let Some(f) = start_in(nether,rx,rz) {
                paint(&f,cx,cz,&mut |x,y,z,state| { chunk.put_generated(x-cx*16,y,z-cz*16,state,true); });
            }
        }
    }
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;
    /// Сохранённые начала проверяют поток координат, числовой выбор образца
    /// и ось вращения. Высоты проверяются отдельно по базовой колонке.
    #[test]
    fn census_fossil_random_stream_and_rotation_anchor() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/nether-fossils.json"
        )).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let n=Nether::new(case["seed"].as_i64().unwrap());
            let mut matched=0;
            for row in case["starts"].as_array().unwrap() {
                let (cx,cz)=(row[0].as_i64().unwrap() as i32,row[1].as_i64().unwrap() as i32);
                let Some(f)=start_in(&n,cx.div_euclid(2),cz.div_euclid(2)) else { continue };
                assert_eq!(library()[f.template].name,row[5].as_str().unwrap());
                assert_eq!(f.orient.turns,row[6].as_u64().unwrap() as u8);
                assert_eq!(f.origin.0,row[7][0].as_i64().unwrap() as i32);
                assert_eq!(f.origin.2,row[7][2].as_i64().unwrap() as i32);
                matched+=1;
            }
            assert!(matched * 10 >= case["starts"].as_array().unwrap().len() * 8);
        }
    }

    /// Перепись 26.3: одинаковое окно 33×33 чанка, доля долины уже исправлена.
    #[test]
    fn census_fossil_frequency_follows_valley_area() {
        for (seed, expected) in [(1_792_932_773_345_722_119, 20i32), (100_554_032_945_340, 99i32)] {
            let n = Nether::new(seed);
            let fossils: Vec<_> = (-8..=8).flat_map(|rx| (-8..=8).map(move |rz| (rx,rz)))
                .filter_map(|(rx,rz)| start_in(&n,rx,rz)).collect();
            let tolerance = (expected as f64 * 0.25).max(1.0);
            assert!((fossils.len() as f64 - f64::from(expected)).abs() <= tolerance,
                "{seed}: {} / {expected}", fossils.len());
            let first = fossils.first().unwrap();
            let mut bones = 0;
            for cx in first.chunk.0-1..=first.chunk.0+1 {
                for cz in first.chunk.1-1..=first.chunk.1+1 {
                    let mut chunk = n.chunk(cx,cz);
                    build(&n,&mut chunk,cx,cz);
                    for y in 33..=120 {
                        for x in 0..16 {
                            for z in 0..16 {
                                bones += usize::from(crate::blocks::block_at_state(chunk.block(x,y,z)) == Some("bone_block"));
                            }
                        }
                    }
                }
            }
            assert!(bones > 0);
        }
    }

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
                let (mut x,y,mut z)=f.origin;
                let t=&library()[f.template];
                match f.orient.turns {
                    1=>x+=t.length-1,
                    2=>{x+=t.width-1;z+=t.length-1;},
                    3=>z+=t.width-1,
                    _=>{},
                }
                assert_eq!(f.chunk,(rx*2,rz*2));
                assert_eq!(n.biome_at(x,z),Biome::SoulSandValley);
                assert!((33..=112).contains(&y));
                assert_eq!(n.fossil_floor_below(x,y,z),Some(y-1));
            }
        }}
        assert!(count>0);
    }
}
