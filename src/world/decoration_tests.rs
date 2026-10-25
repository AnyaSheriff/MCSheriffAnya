// Перепись готовых чанков: выходные данные, без чтения временных миров.
use super::*;
use std::io::Write;

#[test]
#[ignore]
fn export_decoration_chunks() {
    let provenance: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/crowns-26.3-provenance.json")).unwrap();
    let plan = &provenance["ground_decorations_26_3"]["plan"];
    let seed = plan["seed"].as_i64().unwrap();
    let generator = Generator::Normal(terrain::Terrain::new(seed, terrain::Style::Vanilla).with_preview(true));
    let path = std::env::var("DECORATION_OUTPUT").expect("DECORATION_OUTPUT");
    let names: Vec<_> = (0..65536).map(|id| crate::blocks::block_at_state(id).unwrap_or("?")).collect();
    let properties: Vec<_> = (0..65536).map(|id| {
        crate::blocks::orientation_of(id).map(|o| serde_json::json!({"distance": o.value(id,"distance"), "persistent": o.value(id,"persistent")})).unwrap_or(serde_json::Value::Null)
    }).collect();
    let biomes: Vec<_> = (0..128).map(|id| terrain::Biome::by_number(id).map_or("?", |b| b.name())).collect();
    fs::write(format!("{path}.registry.json"), serde_json::to_vec(&serde_json::json!({"blocks": names, "biomes": biomes, "properties": properties})).unwrap()).unwrap();
    let mut out = flate2::write::GzEncoder::new(fs::File::create(path).unwrap(), flate2::Compression::fast());
    for pair in plan["chunks"].as_array().unwrap() {
        let (cx, cz) = (pair[0].as_i64().unwrap() as i32, pair[1].as_i64().unwrap() as i32);
        let chunk = Chunk::generated(&generator, cx, cz);
        let mut sections = Vec::new();
        for sy in 2..20 {
            let mut palette = Vec::new();
            let mut lookup = std::collections::HashMap::new();
            let mut ids = Vec::new();
            let mut props = Vec::new();
            for y in 0..16 { for z in 0..16 { for x in 0..16 {
                let state = chunk.block(x, sy * 16 + y, z);
                let name = crate::blocks::block_at_state(state).unwrap_or("?");
                let index = *lookup.entry(state).or_insert_with(|| { palette.push(name); props.push(properties[state as usize].clone()); palette.len() - 1 });
                ids.push(index);
            } } }
            let mut biomes = Vec::new();
            for y in 0..4 { for z in 0..4 { for x in 0..4 {
                biomes.push(terrain::Biome::by_number(chunk.biome(x * 4, sy * 16 + y * 4, z * 4) as usize).unwrap().name());
            } } }
            sections.push(serde_json::json!({"Y": sy, "palette": palette, "ids": ids, "biomes": biomes, "properties": props}));
        }
        serde_json::to_writer(&mut out, &serde_json::json!({"xPos": cx, "zPos": cz, "sections": sections})).unwrap();
        writeln!(out).unwrap();
    }
    out.finish().unwrap();
}

pub(super) fn leaf_audit(chunks: &std::collections::HashMap<(i32,i32), Chunk>, interior: impl Fn(i32,i32) -> bool) -> (usize,usize) {
    use std::collections::{HashMap,HashSet,VecDeque};
    let mut leaves=HashMap::new();
    let mut logs=HashSet::new();
    for (&(cx,cz),chunk) in chunks {
        for (sy,section) in chunk.sections.iter().enumerate() {
            let Some(section)=section else {continue};
            for (k,&state) in section.iter().enumerate() {
                let state=i32::from(state);
                let name=crate::blocks::block_at_state(state).unwrap_or("");
                let p=(cx*16+(k as i32&15),MIN_Y+sy as i32*16+(k as i32>>8),cz*16+((k as i32>>4)&15));
                if name.ends_with("_leaves") {leaves.insert(p,state);}
                else if name.ends_with("_log") || name.ends_with("_wood") {logs.insert(p);}
            }
        }
    }
    let mut support=HashMap::new();
    let mut queue: VecDeque<_>=logs.into_iter().map(|p|(p,0)).collect();
    while let Some(((x,y,z),d))=queue.pop_front() {
        if d==6 {continue;}
        for (dx,dy,dz) in [(1,0,0),(-1,0,0),(0,1,0),(0,-1,0),(0,0,1),(0,0,-1)] {
            let p=(x+dx,y+dy,z+dz);
            if leaves.contains_key(&p) && !support.contains_key(&p) {
                support.insert(p,d+1);queue.push_back((p,d+1));
            }
        }
    }
    let (mut count,mut mismatches)=(0,0);
    for (&(x,y,z),&state) in &leaves {
        if !interior(x,z) {continue;}
        let o=crate::blocks::orientation_of(state).expect("свойства листа");
        assert_eq!(o.value(state,"persistent"),Some("false"),"генерация не маскирует опору persistent");
        let stored=o.value(state,"distance").unwrap().parse::<i32>().unwrap();
        let actual=support.get(&(x,y,z)).copied().unwrap_or(7);
        assert!(stored<7 && actual<7,"лист распадётся: ({x},{y},{z}), {:?}, stored={stored}, actual={actual}",crate::blocks::block_at_state(state));
        mismatches+=usize::from(stored!=actual);
        count+=1;
    }
    (count,mismatches)
}

/// Независимый обход уже нарисованных чанков; кайма сохраняет опору через швы.
#[test]
fn generated_spawn_and_cherry_leaves_do_not_decay() {
    let generator=Generator::Normal(terrain::Terrain::new(1_792_912_664_922_343_905,terrain::Style::Vanilla).with_preview(true));
    let mut checked=0;
    for (cx,cz) in [(34,-32),(49,-35),(48,-30),(49,-22),(43,-20),(55,-12)] {
        let mut chunks=std::collections::HashMap::new();
        for x in cx-2..=cx+2 {for z in cz-2..=cz+2 {chunks.insert((x,z),Chunk::generated(&generator,x,z));}}
        checked+=leaf_audit(&chunks,|x,z| ((cx-1)*16..(cx+2)*16).contains(&x) && ((cz-1)*16..(cz+2)*16).contains(&z)).0;
    }
    assert!(checked>1000,"пустая выборка {checked}");
}

#[test]
fn reported_stone_mountain_has_no_orphan_litter() {
    let generator=Generator::Normal(terrain::Terrain::new(1_792_912_664_922_343_905,terrain::Style::Vanilla).with_preview(true));
    let mut litter=0;
    for cx in 48..=50 {for cz in -23..=-21 {
        let chunk=Chunk::generated(&generator,cx,cz);
        for x in 0..16 {for z in 0..16 {
            if (cx*16+x-790).abs()>16 || (cz*16+z+352).abs()>16 {continue;}
            for y in 100..180 {litter+=usize::from(crate::blocks::block_at_state(chunk.block(x,y,z))==Some("leaf_litter"));}
        }}
        if (cx,cz)==(49,-22) {assert_eq!(chunk.block(6,141,0),AIR);}
    }}
    assert_eq!(litter,0,"до исправления здесь 266 кучек, 261 на камне");
}

/// Площадки оригинала запрещают стекло, воду и листву, но допускают камень.
#[test]
fn tree_litter_uses_observed_surfaces_and_stays_near_its_tree() {
    let terrain=terrain::Terrain::new(42,terrain::Style::Vanilla).with_preview(true);
    let mut positive=0;
    for surface in ["grass_block","dirt","stone","gravel","sand","glass","water","oak_leaves"] {
        let mut chunk=Chunk::new();
        let floor=crate::blocks::state_by_name(surface).unwrap();
        for x in 0..16 {for z in 0..16 {chunk.put_generated(x,199,z,floor,true);}}
        let tree=trees::from_sapling("oak_sapling",false,1).unwrap().in_preview(42);
        for y in 200..205 {chunk.put_generated(8,y,8,terrain::block_named("oak_log"),true);}
        vegetation::tree_litter(&terrain,&mut chunk,&tree,(8,8),(8,199,8),terrain::Biome::Forest);
        let mut n=0;
        for x in 0..16 {for z in 0..16 {
            if crate::blocks::block_at_state(chunk.block(x,200,z))==Some("leaf_litter") {
                assert!(matches!(surface,"grass_block"|"dirt"|"stone"));
                assert!((x-8)*(x-8)+(z-8)*(z-8)<=32);
                n+=1;
            }
        }}
        if matches!(surface,"grass_block"|"dirt"|"stone") {assert!(n>=16);positive+=n;}
        else {assert_eq!(n,0,"{surface}");}
    }
    assert!(positive>45);
}

#[test]
fn tree_litter_distance_distribution_matches_official_plots() {
    let reference: serde_json::Value=serde_json::from_str(include_str!("../../testdata/world-profile/crowns-26.3-provenance.json")).unwrap();
    let plots=reference["ground_decorations_26_3"]["plots"].as_array().unwrap();
    let mut expected=[0usize;3];
    let mut observed=[0usize;3];
    let mut nreference=0;
    for plot in plots.iter().filter(|p| p["feature"]=="oak_leaf_litter" && ["grass_block","dirt","stone"].iter().any(|s| p["surface"]==*s)) {
        let blocks=plot["blocks"].as_array().unwrap();
        let point=|b: &serde_json::Value| (b[0].as_i64().unwrap(),b[1].as_i64().unwrap(),b[2].as_i64().unwrap());
        let logs: Vec<_>=blocks.iter().filter(|b| b[3].as_str().unwrap().ends_with("_log")).map(point).collect();
        for leaf in blocks.iter().filter(|b| b[3]=="leaf_litter") {
            let p=point(leaf);
            let d=logs.iter().map(|q| (p.0-q.0).pow(2)+(p.1-q.1).pow(2)+(p.2-q.2).pow(2)).min().unwrap();
            for (i,bound) in [4,16,64].into_iter().enumerate() {expected[i]+=usize::from(d<=bound);}
            nreference+=1;
        }
    }
    let mut nactual=0;
    for seed in 0..128 {
        let terrain=terrain::Terrain::new(seed,terrain::Style::Vanilla).with_preview(true);
        let mut chunk=Chunk::new();
        for x in 0..16 {for z in 0..16 {chunk.put_generated(x,199,z,terrain::block_named("grass_block"),true);}}
        let tree=(0..100).map(|s| trees::from_sapling("oak_sapling",false,s).unwrap()).find(|t| t.shape==trees::Shape::Oak).unwrap().in_preview(seed);
        let ground=TreeGround {seed,terrain:Some(&terrain),..TreeGround::new()};
        draw_tree(&mut chunk,&tree,8,8,199,8,8,&ground);
        vegetation::tree_litter(&terrain,&mut chunk,&tree,(8,8),(8,199,8),terrain::Biome::Forest);
        let mut logs=Vec::new();let mut litter=Vec::new();
        for y in 200..220 {for x in 0..16 {for z in 0..16 {
            match crate::blocks::block_at_state(chunk.block(x,y,z)) {
                Some("oak_log")=>logs.push((x,y,z)),Some("leaf_litter")=>litter.push((x,y,z)),_=>(),
            }
        }}}
        for p in litter {
            let d=logs.iter().map(|q| (p.0-q.0).pow(2)+(p.1-q.1).pow(2)+(p.2-q.2).pow(2)).min().unwrap();
            for (i,bound) in [4,16,64].into_iter().enumerate() {observed[i]+=usize::from(d<=bound);}
            nactual+=1;
        }
    }
    assert!(nreference>90 && nactual>3000);
    for i in 0..3 {
        let reference=expected[i] as f64/nreference as f64;
        let actual=observed[i] as f64/nactual as f64;
        assert!((reference-actual).abs()<0.07,"CDF {i}: оригинал {reference}, наш {actual}");
    }
    assert_eq!(observed[2],nactual);
}
