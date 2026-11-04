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
    let names = ["village", "pillager_outpost", "desert_pyramid", "jungle_pyramid", "monument", "mansion", "ancient_city", "trial_chambers", "stronghold", "mineshaft", "ruined_portal", "shipwreck", "buried_treasure", "ocean_ruin", "trail_ruins", "igloo", "swamp_hut", "fortress", "bastion_remnant", "end_city", "nether_fossil"];
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
                    out["piece_records"] = json!(village::audit_pieces(kind, &pieces));
                    out["footprints"] = json!(pieces.iter().map(|p| p.bounds).collect::<Vec<_>>());
                }
                1 => { out["piece_records"] = json!(pillager_outpost::audit_pieces(t,cx,cz)); }
                6 => {
                    let column = t.column_at(marker.x,marker.z);
                    out["surface_biome"] = out["biome"].clone();
                    out["biome"] = json!(cave_biomes::biome_at(&column,-51).unwrap_or(column.biome).name());
                    out["biome_y"] = json!(-51);
                    out["piece_records"] = json!([ancient_city::audit_box(t.seed(),cx,cz)]);
                }
                7 => { out["piece_records"] = json!([trial_chambers::audit_box(t.seed(),cx,cz)]); }
                14 => {
                    let site = trail_ruins::site_in(t,cx.div_euclid(trail_ruins::SPACING),cz.div_euclid(trail_ruins::SPACING)).unwrap();
                    out["piece_records"] = json!(trail_ruins::audit_pieces(&site));
                }
                2 | 3 | 16 => {
                    let kind = match marker.kind { 2 => temples::Kind::DesertPyramid, 3 => temples::Kind::JunglePyramid, _ => temples::Kind::SwampHut };
                    let s = temples::start_in(kind,t,cx.div_euclid(32),cz.div_euclid(32)).unwrap();
                    out["floor"] = json!(s.y); out["footprint"] = json!(s.area()); out["pieces"] = json!(1);
                    out["piece_records"] = json!([temples::audit_piece(&s)]);
                }
                4 => { out["floor"] = json!(monument::BOTTOM); out["piece_records"] = json!([monument::audit_box(t.seed(),cx,cz)]); }
                5 => {
                    let s = mansion::start_in(t,cx.div_euclid(80),cz.div_euclid(80)).unwrap();
                    out["origin"] = json!(s.origin); out["size_xz"] = json!(s.footprint());
                }
                8 => {
                    let pieces = stronghold::plan(t.seed(),cx,cz);
                    out["pieces"] = json!(pieces.len()); out["boxes"] = json!(pieces.iter().map(|p| p.bounds).collect::<Vec<_>>());
                    out["piece_types"] = json!(pieces.iter().map(|p| p.kind.id()).collect::<Vec<_>>());
                    out["piece_records"] = json!(pieces.iter().map(|p| json!({"name":p.kind.id(),"box":p.bounds})).collect::<Vec<_>>());
                }
                9 => {
                    let (mesa, height) = mineshaft::start_at(t,cx,cz).unwrap();
                    out["kind"] = json!(if mesa { "mineshaft_mesa" } else { "mineshaft" });
                    let plan = mineshaft::plan(t.seed(),cx,cz,mesa,height);
                    out["pieces"] = json!(plan.pieces.len());
                    out["piece_records"] = json!(plan.pieces.iter().map(|p| json!({"name":format!("{:?}",p.kind),"rotation":p.facing,"box":p.bb})).collect::<Vec<_>>());
                }
                10 => {
                    let s = ruined_portal::start_in(t,cx.div_euclid(40),cz.div_euclid(40)).unwrap();
                    out["kind"] = json!(s.variant.name()); out["origin"] = json!(s.origin);
                    out["template"] = json!(ruined_portal::library()[s.template].name);
                    out["piece_records"] = json!([template_record(&ruined_portal::library()[s.template],s.orient,s.origin)]);
                }
                11 => {
                    let s = shipwreck::start_in(t,cx.div_euclid(24),cz.div_euclid(24)).unwrap();
                    out["kind"] = json!(if s.beached { "shipwreck_beached" } else { "shipwreck" });
                    out["origin"] = json!(s.origin); out["template"] = json!(shipwreck::library()[s.template].name);
                    out["piece_records"] = json!([template_record(&shipwreck::library()[s.template],s.orient,s.origin)]);
                }
                12 => { let y = shipwreck::treasure_in(t,cx,cz).unwrap(); out["floor"] = json!(y); out["piece_records"] = json!([{"name":"buried_treasure","box":[cx*16+9,y,cz*16+9,cx*16+9,y,cz*16+9]}]); }
                13 => {
                    let s = ocean_ruin::site_in(t,cx.div_euclid(20),cz.div_euclid(20)).unwrap();
                    out["kind"] = json!(if s.warm { "ocean_ruin_warm" } else { "ocean_ruin_cold" });
                    out["pieces"] = json!(s.ruins.len());
                    out["piece_records"] = json!(s.ruins.iter().map(|p| template_record(&ocean_ruin::library()[p.template],p.orient,p.origin)).collect::<Vec<_>>());
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
            out["piece_records"] = match marker.kind {
                17 => json!(nether_fortress::plan(n.seed(),cx,cz).iter().map(|p| json!({"name":p.kind.id(),"rotation":p.facing,"box":p.bounds})).collect::<Vec<_>>()),
                18 => json!(nether_bastion::audit_pieces(&nether_bastion::plan(n.seed(),cx,cz))),
                _ => json!([]),
            };
            let boxes = match marker.kind {
                17 => nether_fortress::plan(n.seed(),cx,cz).iter().map(|p| p.bounds).collect::<Vec<_>>(),
                18 => nether_bastion::plan(n.seed(),cx,cz).iter().map(|p| p.bounds).collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            if !boxes.is_empty() { out["pieces"] = json!(boxes.len()); out["boxes"] = json!(boxes); }
            if marker.kind == 20 {
                let s = nether_fossil::start_in(n,cx.div_euclid(2),cz.div_euclid(2)).unwrap();
                out["anchor_biome"] = out["biome"].clone();
                out["origin"] = json!(s.origin);
                out["biome"] = json!(n.biome_at(s.origin.0,s.origin.2).name());
                out["piece_records"] = json!([template_record(&nether_fossil::library()[s.template],s.orient,s.origin)]);
            }
            if marker.kind == 10 {
                let s = ruined_portal::start_in_nether(n,cx.div_euclid(40),cz.div_euclid(40)).unwrap();
                out["kind"] = json!("ruined_portal_nether"); out["origin"] = json!(s.origin);
                out["piece_records"] = json!([template_record(&ruined_portal::library()[s.template],s.orient,s.origin)]);
            }
        }
        Generator::End(e) => {
            out["biome"] = json!(e.biome_at(marker.x,marker.z).name());
            let (_,_,y) = e.city_at(cx.div_euclid(20),cz.div_euclid(20)).unwrap();
            let pieces = end_city::plan(e.seed(),cx,cz,y);
            out["pieces"] = json!(pieces.len()); out["boxes"] = json!(pieces.iter().map(|p| p.bounds).collect::<Vec<_>>());
            out["piece_types"] = json!(pieces.iter().map(|p| p.name()).collect::<Vec<_>>());
            out["piece_records"] = json!(end_city::audit_pieces(&pieces));
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
    let halo: i32 = std::env::var("AUDIT_HALO").ok().map(|s| s.parse().unwrap()).unwrap_or(0);
    let mut starts = Vec::new();
    for (wx,wz,half) in windows() {
        let half = half + halo;
        for kind in 0..seedmap::STRUCTURES.len() as u8 {
            for marker in map.structures(kind,(wx-half)*16,(wz-half)*16,(wx+half)*16,(wz+half)*16).unwrap() {
                let mut record = description(marker,&generator);
                record["window"] = if cx_in_core(marker.x.div_euclid(16), marker.z.div_euclid(16), wx, wz, half-halo) { json!(format!("{},{}",wx/512,wz/512)) } else { Value::Null }; starts.push(record);
            }
        }
    }
    std::fs::write(dir.join(format!("{selected}-starts.json")),serde_json::to_vec_pretty(&starts).unwrap()).unwrap();
    let mut output = std::io::BufWriter::new(std::fs::File::create(dir.join(format!("{selected}-chunks.jsonl"))).unwrap());
    // Координаты оболочек для связных компонент, а не число почек вместо
    // числа жеод. Формат: x/y/z i32 LE, затем номер материала u8.
    let mut shells = std::io::BufWriter::new(std::fs::File::create(dir.join(format!("{selected}-shells.bin"))).unwrap());
    let names: Vec<_> = (0..=u16::MAX).map(|s| blocks::block_at_state(i32::from(s)).unwrap_or("air")).collect();
    let probes: std::collections::BTreeMap<String, Vec<[i32;3]>> = std::env::var("AUDIT_PROBES").ok().map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()).unwrap_or_default();
    let selected_chunks: Option<std::collections::HashSet<[i32;2]>> = std::env::var("AUDIT_CHUNKS").ok().map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap());
    let mut count = 0;
    let started = std::time::Instant::now();
    for (wx,wz,half) in windows() {
        let half = half + halo;
        for cz in wz-half..wz+half {
            for cx in wx-half..wx+half {
                if selected_chunks.as_ref().is_some_and(|cs| !cs.contains(&[cx,cz])) { continue; }
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
                let mut probe_tops = std::collections::HashMap::new();
                let samples: Vec<_> = probes.get(&format!("{cx},{cz}")).into_iter().flatten().map(|&[x,y,z]| {
                    let top = *probe_tops.entry((x,z)).or_insert_with(|| (MIN_Y..MIN_Y+WORLD_HEIGHT).rev().find(|&h| blocks::has_box(chunk.block(x.rem_euclid(16),h,z.rem_euclid(16)))));
                    json!({"pos":[x,y,z],"at":blocks::block_at_state(chunk.block(x.rem_euclid(16),y,z.rem_euclid(16))),"below":blocks::block_at_state(chunk.block(x.rem_euclid(16),y-1,z.rem_euclid(16))),"surface":top})
                }).collect();
                let missing_containers: Vec<_> = chunk.sections.iter().enumerate().flat_map(|(si,section)| section.iter().flat_map(move |s| s.iter().enumerate().map(move |(i,&state)| (si,i,state)))).filter_map(|(si,i,state)| {
                    let (x,y,z) = (i as i32%16,MIN_Y+si as i32*16+i as i32/256,i as i32/16%16);
                    (matches!(names[usize::from(state)],"chest"|"barrel") && !chunk.containers.contains_key(&(x,y,z))).then_some([cx*16+x,y,cz*16+z])
                }).collect();
                let frames: Vec<_> = chunk.frames.iter().map(|f| json!({"id":"item_frame","pos":[cx*16+f.pos.0,f.pos.1,cz*16+f.pos.2],"facing":f.facing,"item":f.item.and_then(|s| blocks::item_name(s.item))})).collect();
                let saved_mobs: Vec<_> = chunk.mobs.iter().map(|m| json!({"kind":format!("{:?}",m.kind),"pos":[m.x,m.y,m.z],"age":m.age,"type":m.villager.kind,"profession":m.villager.profession,"variant":m.tame.variant})).collect();
                let record = json!({"saved_mobs":saved_mobs,"frames":frames,"spawn_markers":spawn_markers(&generator,cx,cz),"samples":samples,"missing_containers":missing_containers,"chunk":[cx,cz],"window":[wx/512,wz/512],"blocks":histogram,"containers":containers,"spawners":spawners});
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

fn cx_in_core(cx: i32, cz: i32, wx: i32, wz: i32, half: i32) -> bool {
    (wx-half..wx+half).contains(&cx) && (wz-half..wz+half).contains(&cz)
}

#[test]
fn measured_bastion_spawner_keeps_magma_cube_on_reload() {
    let generator = Generator::Nether(nether::Nether::new(SEED));
    // После исправления размещения Незера старые координаты больше не бастион.
    let Generator::Nether(n) = &generator else { unreachable!() };
    let (cx,cz,pos) = (-4..=4).flat_map(|x| (-4..=4).map(move |z| (x,z)))
        .filter_map(|(x,z)| n.bastion_start(x,z))
        .flat_map(|(x,z)| (x-4..=x+4).flat_map(move |cx| (z-4..=z+4).map(move |cz| (cx,cz))))
        .find_map(|(cx,cz)| n.bastion_spawners(cx,cz).into_iter().next().map(|pos| (cx,cz,pos)))
        .expect("сокровищница бастиона со спаунером");
    let chunk = Chunk::generated(&generator,cx,cz);
    assert_eq!(blocks::block_at_state(chunk.block(pos.0,pos.1,pos.2)),Some("spawner"));
    assert_eq!(chunk.spawners[&pos].entity,"minecraft:magma_cube");
    let decoded = Chunk::from_bytes(&chunk.to_bytes()).unwrap();
    assert_eq!(decoded.spawners[&pos].entity,"minecraft:magma_cube");
    assert!(n.bastion_spawners(cx,cz).contains(&pos));
}

fn template_record(t: &template::Template, orient: template::Orient, (x,y,z): (i32,i32,i32)) -> Value {
    let (w,d) = t.footprint(orient);
    json!({"name":t.name,"rotation":orient.turns,"mirror":orient.mirror,"box":[x,y,z,x+w-1,y+t.height-1,z+d-1]})
}

// Плановые маркеры свежего чанка — до фильтра столкновений в tick.rs.
// Не выдаём их за реально сохранённых мобов после симуляции.
fn spawn_markers(generator: &Generator, cx: i32, cz: i32) -> Vec<Value> {
    let mut out = Vec::new();
    let mut add = |kind: &str, pos: (f64,f64,f64)| out.push(json!({"id":kind,"pos":pos}));
    match generator {
        Generator::Normal(t) => {
            for p in monument::elders(t,cx,cz) { add("elder_guardian",p); }
            for p in ocean_ruin::drowned(t,cx,cz) { add("drowned",p); }
            for p in temples::hut_dwellers(t,cx,cz) { add("witch",p); add("cat_all_black",p); }
            for p in pillager_outpost::golems(t,cx,cz) { add("iron_golem",p); }
            for p in pillager_outpost::allays(t,cx,cz) { add("allay",p); }
            for (p,k) in mansion::dwellers(t,cx,cz) { add(k,p); }
            for (p,zombie) in temples::igloo_dwellers(t,cx,cz) { add(if zombie {"zombie_villager"} else {"villager"},p); }
            for ((x,y,z),_) in mineshaft::carts(t,cx,cz) { add("chest_minecart",(f64::from(x),f64::from(y),f64::from(z))); }
        }
        Generator::End(e) => for (x,y,z) in e.city_sentries(cx,cz) { add("shulker",(f64::from(x),f64::from(y),f64::from(z))); },
        _ => {}
    }
    out
}

#[path = "structures_audit_border.rs"]
mod border;

#[test]
fn generated_end_ship_frame_survives_reload_without_duplication() {
    let g = Generator::End(the_end::End::new(SEED));
    let c = Chunk::generated(&g,-97,-165);
    assert_eq!(c.frames.len(),1);
    let f = &c.frames[0];
    assert_eq!(f.pos,(7,109,8));
    assert_eq!(f.facing,crate::frame::SOUTH);
    assert_eq!(blocks::item_name(f.item.unwrap().item),Some("elytra"));
    let back = Chunk::from_bytes(&c.to_bytes()).unwrap();
    assert_eq!(back.frames,c.frames);
}

/// Доля площади по одинаковой с эталоном сетке 4×4, плюс биомы попыток.
#[test]
#[ignore]
fn offline_structure_exposure() {
    let dir = std::path::PathBuf::from(std::env::var("AUDIT_OUT").expect("AUDIT_OUT"));
    std::fs::create_dir_all(&dir).unwrap();
    let t = Terrain::new(SEED,terrain::Style::Vanilla);
    let mut rows = serde_json::Map::new();
    let mut caves = serde_json::Map::new();
    for (wx,wz,h) in windows() {
        let mut surface = std::collections::BTreeMap::<String,u64>::new();
        let mut cave = std::collections::BTreeMap::<String,u64>::new();
        for cx in wx-h..wx+h { for cz in wz-h..wz+h {
            for dx in [2,6,10,14] { for dz in [2,6,10,14] {
                let c=t.column_at(cx*16+dx,cz*16+dz);
                *surface.entry(cave_biomes::biome_at(&c,64).unwrap_or(c.biome).name().into()).or_default()+=16;
                *cave.entry(cave_biomes::biome_at(&c,-51).unwrap_or(c.biome).name().into()).or_default()+=16;
            }}
        }}
        rows.insert(format!("{},{}",wx/512,wz/512),json!(surface));
        caves.insert(format!("{},{}",wx/512,wz/512),json!(cave));
    }
    std::fs::write(dir.join("ours-exposure.json"),serde_json::to_vec_pretty(&json!({"y64":rows,"y_minus51":caves})).unwrap()).unwrap();
}

#[test]
fn ancient_city_audit_reports_its_underground_biome() {
    let generator=Generator::Normal(Terrain::new(SEED,terrain::Style::Vanilla));
    let Generator::Normal(t)=&generator else { unreachable!() };
    let mut found=0;
    for rx in -24..24 { for rz in -24..24 {
        if let Some((cx,cz))=ancient_city::start_in(t,rx,rz) {
            let record=description(seedmap::Marker{kind:6,x:cx*16+8,z:cz*16+8},&generator);
            assert_eq!(record["biome"],"deep_dark");
            assert_eq!(record["biome_y"],-51);
            found+=1;
        }
    }}
    assert!(found>0);
}
