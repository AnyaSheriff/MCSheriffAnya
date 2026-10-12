// Офлайн-перепись настоящих чанков: без World::open, сервера и сокетов.
// AUDIT_OUT — каталог внутри рабочей копии; AUDIT_LIMIT — пробный предел.
// Старты — те же запросы, что карта; блоки — Chunk::generated, по одному
// чанку в памяти. Сущности тика отдельно: Chunk::mobs их ещё не содержит.
use super::*;
use crate::blocks;
use serde_json::{Value, json};
use std::io::Write;

const SEED: i64 = 100_554_032_945_340;

fn windows() -> Vec<(i32, i32, i32)> {
    (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x * 512, z * 512, if x == 0 && z == 0 { 64 } else { 32 }))).collect()
}

fn description(marker: seedmap::Marker, generator: &Generator) -> Value {
    let (cx, cz) = (marker.x.div_euclid(16), marker.z.div_euclid(16));
    let names = ["village", "pillager_outpost", "desert_pyramid", "jungle_pyramid", "monument", "mansion", "ancient_city", "trial_chambers", "stronghold", "mineshaft", "ruined_portal", "shipwreck", "buried_treasure", "ocean_ruin", "trail_ruins", "igloo", "swamp_hut", "fortress", "bastion_remnant", "end_city"];
    let mut out = json!({"kind":names[marker.kind as usize],"chunk":[cx,cz],"anchor":[marker.x,marker.z]});
    match generator {
        Generator::Normal(t) => {
            let ground = structure_terrain::Ground::new(t);
            out["biome"] = json!(t.column_in(marker.x, marker.z, None).biome.name());
            out["surface"] = json!(ground.height(marker.x, marker.z));
            match marker.kind {
                0 => {
                    let (_, _, kind) = village::start_in(t, cx.div_euclid(village::SPACING), cz.div_euclid(village::SPACING)).unwrap();
                    out["kind"] = json!(format!("village_{}", kind.name()));
                    let pieces = village::plan(t.seed(), kind, cx, cz, &|x,z| ground.supported_height(x,z));
                    out["pieces"] = json!(pieces.len());
                    out["footprints"] = json!(pieces.iter().map(|p| p.bounds).collect::<Vec<_>>());
                }
                2 | 3 | 16 => {
                    let kind = match marker.kind { 2 => temples::Kind::DesertPyramid, 3 => temples::Kind::JunglePyramid, _ => temples::Kind::SwampHut };
                    let s = temples::start_in(kind,t,cx.div_euclid(32),cz.div_euclid(32)).unwrap();
                    out["floor"] = json!(s.y); out["footprint"] = json!(s.area()); out["pieces"] = json!(1);
                }
                4 => { out["floor"] = json!(monument::BOTTOM); }
                5 => {
                    let s = mansion::start_in(t,cx.div_euclid(80),cz.div_euclid(80)).unwrap();
                    out["origin"] = json!(s.origin); out["size_xz"] = json!(s.footprint());
                }
                8 => {
                    let pieces = stronghold::plan(t.seed(),cx,cz);
                    out["pieces"] = json!(pieces.len()); out["boxes"] = json!(pieces.iter().map(|p| p.bounds).collect::<Vec<_>>());
                    out["piece_types"] = json!(pieces.iter().map(|p| p.kind.id()).collect::<Vec<_>>());
                }
                9 => {
                    let (mesa, height) = mineshaft::start_at(t,cx,cz).unwrap();
                    out["kind"] = json!(if mesa { "mineshaft_mesa" } else { "mineshaft" });
                    let plan = mineshaft::plan(t.seed(),cx,cz,mesa,height);
                    out["pieces"] = json!(plan.pieces.len());
                }
                10 => {
                    let s = ruined_portal::start_in(t,cx.div_euclid(40),cz.div_euclid(40)).unwrap();
                    out["kind"] = json!(s.variant.name()); out["origin"] = json!(s.origin);
                    out["template"] = json!(ruined_portal::library()[s.template].name);
                }
                11 => {
                    let s = shipwreck::start_in(t,cx.div_euclid(24),cz.div_euclid(24)).unwrap();
                    out["kind"] = json!(if s.beached { "shipwreck_beached" } else { "shipwreck" });
                    out["origin"] = json!(s.origin); out["template"] = json!(shipwreck::library()[s.template].name);
                }
                12 => { out["floor"] = json!(shipwreck::treasure_in(t,cx,cz).unwrap()); }
                13 => {
                    let s = ocean_ruin::site_in(t,cx.div_euclid(20),cz.div_euclid(20)).unwrap();
                    out["kind"] = json!(if s.warm { "ocean_ruin_warm" } else { "ocean_ruin_cold" });
                    out["pieces"] = json!(s.ruins.len());
                }
                15 => {
                    let s = temples::igloo_in(t,cx.div_euclid(32),cz.div_euclid(32)).unwrap();
                    out["floor"] = json!(s.y); out["shaft"] = json!(s.shaft);
                }
                _ => {}
            }
        }
        Generator::Nether(n) => {
            out["biome"] = json!(n.biome_at(marker.x,marker.z).name());
            let boxes = match marker.kind {
                17 => nether_fortress::plan(n.seed(),cx,cz).iter().map(|p| p.bounds).collect::<Vec<_>>(),
                18 => nether_bastion::plan(n.seed(),cx,cz).iter().map(|p| p.bounds).collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            if !boxes.is_empty() { out["pieces"] = json!(boxes.len()); out["boxes"] = json!(boxes); }
            if marker.kind == 10 {
                let s = ruined_portal::start_in_nether(n,cx.div_euclid(40),cz.div_euclid(40)).unwrap();
                out["kind"] = json!("ruined_portal_nether"); out["origin"] = json!(s.origin);
            }
        }
        Generator::End(e) => {
            out["biome"] = json!(e.biome_at(marker.x,marker.z).name());
            let (_,_,y) = e.city_at(cx.div_euclid(20),cz.div_euclid(20)).unwrap();
            let pieces = end_city::plan(e.seed(),cx,cz,y);
            out["pieces"] = json!(pieces.len()); out["boxes"] = json!(pieces.iter().map(|p| p.bounds).collect::<Vec<_>>());
            out["piece_types"] = json!(pieces.iter().map(|p| p.name()).collect::<Vec<_>>());
        }
        Generator::Flat => unreachable!(),
    }
    out
}

#[test]
#[ignore]
fn offline_structures_audit() {
    let dir = std::path::PathBuf::from(std::env::var("AUDIT_OUT").expect("AUDIT_OUT"));
    std::fs::create_dir_all(&dir).unwrap();
    let limit: usize = std::env::var("AUDIT_LIMIT").ok().map(|s| s.parse().unwrap()).unwrap_or(usize::MAX);
    let selected = std::env::var("AUDIT_DIM").unwrap_or_else(|_| "overworld".into());
    let (dim, generator) = match selected.as_str() {
        "overworld" => (seedmap::Dimension::Overworld, Generator::Normal(Terrain::new(SEED, terrain::Style::Vanilla))),
        "the_nether" => (seedmap::Dimension::Nether, Generator::Nether(nether::Nether::new(SEED))),
        "the_end" => (seedmap::Dimension::End, Generator::End(the_end::End::new(SEED))),
        _ => panic!("AUDIT_DIM"),
    };
    let map = seedmap::Map::new(SEED,dim,false,false);
    let mut starts = Vec::new();
    for (wx,wz,half) in windows() {
        for kind in 0..20 {
            for marker in map.structures(kind,(wx-half)*16,(wz-half)*16,(wx+half)*16,(wz+half)*16).unwrap() {
                let mut record = description(marker,&generator);
                record["window"] = json!([wx/512,wz/512]); starts.push(record);
            }
        }
    }
    std::fs::write(dir.join(format!("{selected}-starts.json")),serde_json::to_vec_pretty(&starts).unwrap()).unwrap();
    let mut output = std::io::BufWriter::new(std::fs::File::create(dir.join(format!("{selected}-chunks.jsonl"))).unwrap());
    // Координаты оболочек для связных компонент, а не число почек вместо
    // числа жеод. Формат: x/y/z i32 LE, затем номер материала u8.
    let mut shells = std::io::BufWriter::new(std::fs::File::create(dir.join(format!("{selected}-shells.bin"))).unwrap());
    let names: Vec<_> = (0..=u16::MAX).map(|s| blocks::block_at_state(i32::from(s)).unwrap_or("air")).collect();
    let mut count = 0;
    let started = std::time::Instant::now();
    for (wx,wz,half) in windows() {
        for cz in wz-half..wz+half {
            for cx in wx-half..wx+half {
                if count == limit { output.flush().unwrap(); shells.flush().unwrap(); return; }
                let chunk = Chunk::generated(&generator,cx,cz);
                let mut histogram = std::collections::BTreeMap::<&str,usize>::new();
                for (si, section) in chunk.sections.iter().enumerate() {
                    let Some(section) = section else { continue };
                    for (i, &state) in section.iter().enumerate() {
                        let name = names[usize::from(state)];
                        if let Some(material) = ["amethyst_block", "budding_amethyst", "calcite", "smooth_basalt"].iter().position(|&n| n == name) {
                            for coordinate in [cx * 16 + i as i32 % 16, MIN_Y + si as i32 * 16 + i as i32 / 256, cz * 16 + i as i32 / 16 % 16] {
                                shells.write_all(&coordinate.to_le_bytes()).unwrap();
                            }
                            shells.write_all(&[material as u8]).unwrap();
                        }
                        if matches!(name,"budding_amethyst"|"bone_block"|"end_gateway"|"spawner"|"chest"|"barrel"|"trial_spawner"|"vault"|"suspicious_sand"|"suspicious_gravel") {
                            *histogram.entry(name).or_default() += 1;
                        }
                    }
                }
                let containers: Vec<_> = chunk.containers.iter().map(|(&(x,y,z),c)| json!({"pos":[cx*16+x,y,cz*16+z],"kind":format!("{:?}",c.kind),"occupied_slots":c.slots.iter().flatten().count()})).collect();
                let spawners: Vec<_> = chunk.spawners.iter().map(|(&(x,y,z),s)| json!({"pos":[cx*16+x,y,cz*16+z],"entity":s.entity,"block":blocks::block_at_state(chunk.block(x,y,z))})).collect();
                let record = json!({"chunk":[cx,cz],"window":[wx/512,wz/512],"blocks":histogram,"containers":containers,"spawners":spawners});
                serde_json::to_writer(&mut output,&record).unwrap(); writeln!(output).unwrap();
                count += 1;
                if count.is_multiple_of(256) { output.flush().unwrap(); eprintln!("{selected}: {count}/49152 chunks, {:.1}s",started.elapsed().as_secs_f64()); }
            }
        }
    }
    output.flush().unwrap();
    shells.flush().unwrap();
}

#[test]
fn audit_windows_are_disjoint_and_have_requested_area() {
    let mut seen = std::collections::HashSet::new();
    for (x,z,half) in windows() { for cx in x-half..x+half { for cz in z-half..z+half { assert!(seen.insert((cx,cz))); } } }
    assert_eq!(seen.len(),49_152);
}

#[test]
fn rejected_structure_block_does_not_create_a_spawner() {
    let mut chunk = Chunk::new();
    let stone = blocks::state_by_name("stone").unwrap();
    let spawner = blocks::state_by_name("spawner").unwrap();
    chunk.put_generated(4,20,5,stone,true);
    chunk.put_generated(4,20,5,spawner,false);
    assert_eq!(chunk.block(4,20,5),stone);
    assert!(chunk.spawners.is_empty());
}

#[test]
fn later_structure_removes_overwritten_spawner_data() {
    let mut chunk = Chunk::new();
    let spawner = blocks::state_by_name("spawner").unwrap();
    chunk.put_generated(4,20,5,spawner,true);
    assert!(chunk.spawners.contains_key(&(4,20,5)));
    chunk.put_generated(4,20,5,AIR,false);
    assert!(chunk.spawners.contains_key(&(4,20,5)),"отвергнутая замена сохраняет спавнер");
    chunk.put_generated(4,20,5,AIR,true);
    assert_eq!(chunk.block(4,20,5),AIR);
    assert!(chunk.spawners.is_empty());
    let decoded = Chunk::from_bytes(&chunk.to_bytes()).unwrap();
    assert!(decoded.spawners.is_empty(),"призрачный спавнер не попадает в сохранение");
}

// Рамки и опоры первого экземпляра каждого вида: блоки самого построителя
// на пустом холсте сопоставляются с итоговым чанкoм. Это диагностическая
// маска, не восстановление исходной плотности оригинала [проверить].
#[test]
#[ignore]
fn offline_structure_support_audit() {
    let dir = std::path::PathBuf::from(std::env::var("AUDIT_OUT").expect("AUDIT_OUT"));
    let generator = Generator::Normal(Terrain::new(SEED,terrain::Style::Vanilla));
    let map = seedmap::Map::new(SEED,seedmap::Dimension::Overworld,false,false);
    let mut results = Vec::new();
    for kind in [0,1,6,14] {
        let marker = windows().into_iter().find_map(|(x,z,h)| map.structures(kind,(x-h)*16,(z-h)*16,(x+h)*16,(z+h)*16).unwrap().first().copied());
        let Some(marker) = marker else { continue };
        let (cx,cz) = (marker.x.div_euclid(16),marker.z.div_euclid(16));
        let radius = match kind { 0 | 6 => 8, 1 => 4, _ => 4 };
        let mut chunks = HashMap::new();
        let mut mask = HashMap::new();
        let mut changes = Vec::new();
        for x in cx-radius..=cx+radius {
            for z in cz-radius..=cz+radius {
                let mut canvas = Chunk::new();
                match kind {
                    0 => village::build(&generator,&mut canvas,x,z),
                    1 => pillager_outpost::build(&generator,&mut canvas,x,z),
                    6 => ancient_city::build(&generator,&mut canvas,x,z),
                    _ => trail_ruins::build(&generator,&mut canvas,x,z),
                }
                if canvas.sections.iter().all(Option::is_none) { continue; }
                let actual = Chunk::generated(&generator,x,z);
                for (si,section) in canvas.sections.iter().enumerate() {
                    let Some(section) = section else { continue };
                    for (i,&packed) in section.iter().enumerate() {
                        let state = unpack(packed);
                        if !blocks::has_box(state) { continue; }
                        let (lx,y,lz) = (i as i32 % 16, MIN_Y+si as i32*16+i as i32/256, i as i32/16%16);
                        let pos = (x*16+lx,y,z*16+lz);
                        if actual.block(lx,y,lz) == state { mask.insert(pos,state); }
                        else if changes.len() < 32 { changes.push(json!({"pos":pos,"expected":blocks::block_at_state(state),"actual":blocks::block_at_state(actual.block(lx,y,lz))})); }
                    }
                }
                chunks.insert((x,z),actual);
            }
        }
        let block = |(x,y,z): (i32,i32,i32)| chunks.get(&(x.div_euclid(16),z.div_euclid(16))).map(|c| c.block(x.rem_euclid(16),y,z.rem_euclid(16)));
        let mut seen = std::collections::HashSet::new();
        let mut unsupported = Vec::new();
        let mut components = 0;
        let mut unknown = 0;
        let mut air_below = 0;
        for &(x,y,z) in mask.keys() {
            if !mask.contains_key(&(x,y-1,z)) && block((x,y-1,z)) == Some(AIR) { air_below += 1; }
        }
        for &start in mask.keys() {
            if !seen.insert(start) { continue; }
            components += 1;
            let mut pending = vec![start];
            let (mut supported,mut boundary,mut size) = (false,false,0);
            while let Some((x,y,z)) = pending.pop() {
                size += 1;
                for p in [(x-1,y,z),(x+1,y,z),(x,y-1,z),(x,y+1,z),(x,y,z-1),(x,y,z+1)] {
                    if mask.contains_key(&p) {
                        if seen.insert(p) { pending.push(p); }
                    } else {
                        match block(p) { Some(s) => supported |= blocks::full_cube(s), None => boundary = true }
                    }
                }
            }
            if !supported { if boundary { unknown += 1; } else { unsupported.push(json!({"example":start,"blocks":size})); } }
        }
        unsupported.sort_by_key(|v| v["example"].to_string());
        results.push(json!({"kind":kind,"anchor":[marker.x,marker.z],"chunks":chunks.len(),"structure_solid_cells":mask.len(),"components":components,"unsupported_components":unsupported,"boundary_unknown_components":unknown,"air_below_exposed_cells":air_below,"changed_examples":changes}));
    }
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("support.json"),serde_json::to_vec_pretty(&results).unwrap()).unwrap();
}
