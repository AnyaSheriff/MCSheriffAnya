// Офлайн-проверки причин расхождений: биомы попыток и сохранённые жители.
use super::*;
use serde_json::json;
const SEED:i64=100_554_032_945_340;

#[test]
#[ignore]
fn offline_structure_rule_audit() {
    let dir=std::path::PathBuf::from(std::env::var("AUDIT_OUT").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let t=Terrain::new(SEED,terrain::Style::Vanilla);
    let n=nether::Nether::new(SEED);
    let e=the_end::End::new(SEED);
    let mut rows=Vec::new();
    for gx in -1..=1 { for gz in -1..=1 {
        let (wx,wz,h)=(gx*512,gz*512,if gx==0 && gz==0 {64} else {32});
        let inside=|x:i32,z:i32| (wx-h..wx+h).contains(&x) && (wz-h..wz+h).contains(&z);
        let mut candidates=std::collections::BTreeMap::<String,std::collections::BTreeMap<String,u32>>::new();
        for (name,spacing) in [("village",34),("desert_pyramid",32),("ruined_portal",40),("shipwreck",24),("ancient_city",24)] {
            for rx in (wx-h).div_euclid(spacing)..=(wx+h-1).div_euclid(spacing) {
                for rz in (wz-h).div_euclid(spacing)..=(wz+h-1).div_euclid(spacing) {
                    let (cx,cz)=match name {
                        "village"=>village::attempt(SEED,rx,rz),
                        "desert_pyramid"=>temples::attempt(temples::Kind::DesertPyramid,SEED,rx,rz),
                        "ruined_portal"=>ruined_portal::attempt(SEED,rx,rz),
                        "shipwreck"=>shipwreck::attempt(SEED,rx,rz),
                        _=>ancient_city::attempt(SEED,rx,rz),
                    };
                    if !inside(cx,cz) {continue;}
                    let c=t.column_at(cx*16+8,cz*16+8);
                    let biome=if name=="ancient_city" {cave_biomes::biome_at(&c,-51).unwrap_or(c.biome)} else {c.biome};
                    *candidates.entry(name.into()).or_default().entry(biome.name().into()).or_default()+=1;
                }
            }
        }
        let mut nb=std::collections::BTreeMap::<String,u32>::new();
        let mut eb=std::collections::BTreeMap::<String,u32>::new();
        for cx in wx-h..wx+h { for cz in wz-h..wz+h {
            *nb.entry(n.biome_at(cx*16+8,cz*16+8).name().into()).or_default()+=1;
            *eb.entry(e.biome_at(cx*16+8,cz*16+8).name().into()).or_default()+=1;
        }}
        rows.push(json!({"window":format!("{gx},{gz}"),"candidates":candidates,"nether_chunk_biomes":nb,"end_chunk_biomes":eb}));
    }}
    std::fs::write(dir.join("rules.json"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
fn treasure_probability_gate_and_beach_gate_are_independent() {
    let t=Terrain::new(SEED,terrain::Style::Vanilla);
    for cx in -64..64 { for cz in -64..64 {
        if shipwreck::treasure_in(&t,cx,cz).is_some() {
            assert!(template::chance(SEED,cx,0,cz,0)<0.01);
            assert!(matches!(t.column_at(cx*16+9,cz*16+9).biome,terrain::Biome::Beach|terrain::Biome::SnowyBeach));
        }
    }}
}

#[test]
fn generated_fossil_bones_survive_chunk_serialization() {
    let g=Generator::Nether(nether::Nether::new(SEED));
    let Generator::Nether(n)=&g else {unreachable!()};
    let f=(-16..16).flat_map(|x|(-16..16).map(move |z|(x,z))).find_map(|(x,z)|nether_fossil::start_in(n,x,z)).unwrap();
    let (cx,cz)=(f.origin.0.div_euclid(16),f.origin.2.div_euclid(16));
    let c=Chunk::generated(&g,cx,cz);
    let back=Chunk::from_bytes(&c.to_bytes()).unwrap();
    let mut bones=0;
    nether_fossil::paint(&f,cx,cz,&mut |x,y,z,state| {
        if c.block(x-cx*16,y,z-cz*16)==state {bones+=1;}
        assert_eq!(back.block(x-cx*16,y,z-cz*16),c.block(x-cx*16,y,z-cz*16));
    });
    assert!(bones>0);
}

#[test]
fn desert_and_savanna_have_distinct_villages_and_portal_variants() {
    use terrain::Biome;
    assert_eq!(village::Kind::of_biome(Biome::Desert),Some(village::Kind::Desert));
    assert_eq!(village::Kind::of_biome(Biome::Savanna),Some(village::Kind::Savanna));
    assert_eq!(village::Kind::of_biome(Biome::SavannaPlateau),None);
    assert_eq!(ruined_portal::Variant::of_biome(Biome::Desert),Some(ruined_portal::Variant::Desert));
}
