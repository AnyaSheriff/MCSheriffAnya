// Optional audit adapter linked against the release seedmap-core rlib.
// These are our generator's eligible starts, NOT proof of placed structures.
// Never reads game code, archives, data packs, or official placement algorithms.
use seedmap_core::world::{self, terrain::{Style, Terrain}};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg(genaudit_camps)]
#[allow(dead_code,unused_imports)]
mod camp_support {
    pub use seedmap_core::world::{terrain,template};
    pub mod placement { include!(env!("GENAUDIT_CAMP_SOURCE")); }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 { return Err("usage: genaudit-starts PLAN.json".into()); }
    let plan: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let seed = plan["seed"].as_i64().ok_or("missing seed")?;
    let terrain = Terrain::new(seed, Style::Vanilla).with_preview(true);
    let nether = world::nether::Nether::new(seed);
    let end = world::the_end::End::new(seed);
    let kinds = ["village", "pillager_outpost", "desert_pyramid", "jungle_pyramid", "monument", "mansion",
        "ancient_city", "trial_chambers", "stronghold", "mineshaft", "ruined_portal", "shipwreck",
        "buried_treasure", "ocean_ruin", "trail_ruins", "igloo", "swamp_hut", "fortress", "bastion_remnant",
        "end_city", "nether_fossil"];
    let mut output = serde_json::Map::new();
    for dim in ["overworld", "the_nether", "the_end"] {
        let mut found = BTreeSet::new();
        for window in plan["windows"][dim].as_array().ok_or("missing dimension windows")? {
            let b: Vec<i32> = window.as_array().ok_or("window")?.iter().map(|n| n.as_i64().unwrap() as i32).collect();
            #[cfg(genaudit_camps)]
            if dim=="overworld" {
                let spacing=camp_support::placement::SPACING;
                for rx in b[0].div_euclid(spacing)..=b[2].div_euclid(spacing) {
                    for rz in b[1].div_euclid(spacing)..=b[3].div_euclid(spacing) {
                        if let Some(s)=camp_support::placement::site_in(&terrain,rx,rz) {
                            if s.chunk.0>=b[0] && s.chunk.0<=b[2] && s.chunk.1>=b[1] && s.chunk.1<=b[3] {
                                found.insert(("abandoned_camp".to_string(),s.chunk.0,s.chunk.1));
                            }
                        }
                    }
                }
            }
            for (kind,name) in kinds.iter().enumerate() {
                let allowed = match dim { "overworld" => kind<=16, "the_nether" => matches!(kind,10|17|18|20), _ => kind==19 };
                if !allowed { continue; }
                let mut push = |at: (i32,i32)| {
                    if at.0>=b[0] && at.0<=b[2] && at.1>=b[1] && at.1<=b[3] {
                        found.insert((name.to_string(),at.0,at.1));
                    }
                };
                if kind==8 {
                    for &at in world::stronghold::starts_of(&terrain).iter() { push(at); }
                    continue;
                }
                let spacing = match kind {
                    0=>world::village::SPACING, 1=>world::pillager_outpost::SPACING,
                    2|3|15|16=>world::temples::SPACING, 4=>world::monument::SPACING,
                    5=>world::mansion::SPACING, 6=>world::ancient_city::SPACING,
                    7=>world::trial_chambers::SPACING, 9|12=>1, 10=>world::ruined_portal::SPACING,
                    11=>world::shipwreck::SPACING, 13=>world::ocean_ruin::SPACING,
                    14=>world::trail_ruins::SPACING, 17|18=>world::nether::STRUCTURE_SPACING,
                    19=>world::end_city::REGION, 20=>world::nether_fossil::SPACING, _=>unreachable!(),
                };
                for rx in b[0].div_euclid(spacing)..=b[2].div_euclid(spacing) {
                    for rz in b[1].div_euclid(spacing)..=b[3].div_euclid(spacing) {
                        let at = match kind {
                            0=>world::village::start_in(&terrain,rx,rz).map(|(x,z,_)|(x,z)),
                            1=>world::pillager_outpost::start_in(&terrain,rx,rz),
                            2|3|16=>world::temples::start_in(match kind {2=>world::temples::Kind::DesertPyramid,3=>world::temples::Kind::JunglePyramid,_=>world::temples::Kind::SwampHut},&terrain,rx,rz).map(|s|s.chunk),
                            4=>world::monument::start_in(&terrain,rx,rz),
                            5=>world::mansion::start_in(&terrain,rx,rz).map(|s|s.chunk),
                            6=>world::ancient_city::start_in(&terrain,rx,rz),
                            7=>world::trial_chambers::start_in(&terrain,rx,rz),
                            9=>world::mineshaft::start_at(&terrain,rx,rz).map(|_|(rx,rz)),
                            10=>if dim=="the_nether" {world::ruined_portal::start_in_nether(&nether,rx,rz)} else {world::ruined_portal::start_in(&terrain,rx,rz)}.map(|s|s.chunk),
                            11=>world::shipwreck::start_in(&terrain,rx,rz).map(|s|s.chunk),
                            12=>world::shipwreck::treasure_in(&terrain,rx,rz).map(|_|(rx,rz)),
                            13=>world::ocean_ruin::site_in(&terrain,rx,rz).map(|s|s.chunk),
                            14=>world::trail_ruins::site_in(&terrain,rx,rz).map(|s|s.chunk),
                            15=>world::temples::igloo_in(&terrain,rx,rz).map(|s|s.chunk),
                            17=>nether.fortress_start(rx,rz), 18=>nether.bastion_start(rx,rz),
                            19=>end.city_at(rx,rz).map(|(x,z,_)|(x,z)),
                            20=>world::nether_fossil::start_in(&nether,rx,rz).map(|s|s.chunk), _=>unreachable!(),
                        };
                        if let Some(at)=at {push(at);}
                    }
                }
            }
        }
        let mut records=Vec::new();
        for (kind,x,z) in found {
            let mut boxes: Vec<[i32;6]>=Vec::new();
            let mut template_box=|origin:(i32,i32,i32),size:(i32,i32),height:i32| {
                boxes.push([origin.0,origin.1,origin.2,origin.0+size.0-1,origin.1+height-1,origin.2+size.1-1]);
            };
            match kind.as_str() {
                #[cfg(genaudit_camps)]
                "abandoned_camp"=>{
                    if let Some(s)=camp_support::placement::site_in(&terrain,x.div_euclid(camp_support::placement::SPACING),z.div_euclid(camp_support::placement::SPACING)) {
                        boxes=camp_support::placement::audit_boxes(&s);
                    }
                },
                "stronghold"=>boxes=world::stronghold::plan(seed,x,z).iter().map(|p|p.bounds).collect(),
                "fortress"=>boxes=world::nether_fortress::plan(seed,x,z).iter().map(|p|p.bounds).collect(),
                "end_city"=>{
                    if let Some((_,_,y))=end.city_at(x.div_euclid(world::end_city::REGION),z.div_euclid(world::end_city::REGION)) {
                        boxes=world::end_city::plan(seed,x,z,y).iter().map(|p|p.bounds).collect();
                    }
                },
                "shipwreck"=>{
                    if let Some(s)=world::shipwreck::start_in(&terrain,x.div_euclid(world::shipwreck::SPACING),z.div_euclid(world::shipwreck::SPACING)) {
                        template_box(s.origin,s.footprint(),world::shipwreck::library()[s.template].height);
                    }
                },
                "ruined_portal"=>{
                    let s=if dim=="the_nether" {world::ruined_portal::start_in_nether(&nether,x.div_euclid(world::ruined_portal::SPACING),z.div_euclid(world::ruined_portal::SPACING))}
                          else {world::ruined_portal::start_in(&terrain,x.div_euclid(world::ruined_portal::SPACING),z.div_euclid(world::ruined_portal::SPACING))};
                    if let Some(s)=s {template_box(s.origin,s.footprint(),s.height());}
                },
                "ocean_ruin"=>{
                    if let Some(s)=world::ocean_ruin::site_in(&terrain,x.div_euclid(world::ocean_ruin::SPACING),z.div_euclid(world::ocean_ruin::SPACING)) {
                        for p in &s.ruins {template_box(p.origin,p.footprint(),world::ocean_ruin::library()[p.template].height);}
                    }
                },
                "nether_fossil"=>{
                    if let Some(s)=world::nether_fossil::start_in(&nether,x.div_euclid(world::nether_fossil::SPACING),z.div_euclid(world::nether_fossil::SPACING)) {
                        let t=&world::nether_fossil::library()[s.template];
                        template_box(s.origin,t.footprint(s.orient),t.height);
                    }
                },
                "buried_treasure"=>{
                    if let Some(y)=world::shipwreck::treasure_in(&terrain,x,z) {boxes.push([x*16+9,y,z*16+9,x*16+9,y,z*16+9]);}
                },
                _=>{},
            }
            records.push(json!({"type":kind,"chunk":[x,z],"boxes":boxes}));
        }
        output.insert(dim.into(),json!(records));
    }
    println!("{}",json!({"seed":seed,"method":"Eligible starts from our release seedmap-core, preview=true; placement not proven; optional camps use unmodified own pure placement source", "camps_included":cfg!(genaudit_camps),"dimensions":output}));
    Ok(())
}
