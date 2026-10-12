// Независимый от near()/радиуса поиска контроль: полный принятый план
// рисуется напрямую в каждый пересечённый чанк. Отличие от итогового
// мира может быть законным наложением, а не обрезкой [проверить].
use super::*;

#[test]
#[ignore]
fn offline_structure_border_audit() {
    let dir = std::path::PathBuf::from(std::env::var("AUDIT_OUT").expect("AUDIT_OUT"));
    let mut results = Vec::new();
    for (dim, generator, kinds) in [
        (seedmap::Dimension::Overworld,Generator::Normal(Terrain::new(SEED,terrain::Style::Vanilla)),vec![0]),
        (seedmap::Dimension::Nether,Generator::Nether(nether::Nether::new(SEED)),vec![17,18]),
        (seedmap::Dimension::End,Generator::End(the_end::End::new(SEED)),vec![19]),
    ] {
        let map = seedmap::Map::new(SEED,dim,false,false);
        for (wx,wz,h) in windows() {
            for &kind in &kinds {
                for marker in map.structures(kind,(wx-h)*16,(wz-h)*16,(wx+h)*16,(wz+h)*16).unwrap() {
                    let desc = description(marker,&generator);
                    let mut coords = std::collections::BTreeSet::new();
                    for p in desc["piece_records"].as_array().unwrap() {
                        let b: [i32;6] = serde_json::from_value(p["box"].clone()).unwrap();
                        for cx in b[0].div_euclid(16)..=b[3].div_euclid(16) {
                            for cz in b[2].div_euclid(16)..=b[5].div_euclid(16) { coords.insert((cx,cz)); }
                        }
                    }
                    let (sx,sz) = (marker.x.div_euclid(16),marker.z.div_euclid(16));
                    let (mut checked,mut changed,mut seam_changed,mut seam_checked) = (0usize,0usize,0usize,0usize);
                    let mut examples = Vec::new();
                    for &(cx,cz) in &coords {
                        let mut mask = HashMap::<(i32,i32,i32),i32>::new();
                        match &generator {
                            Generator::Normal(t) => {
                                let ground = structure_terrain::Ground::new(t);
                                let vk = village::start_in(t,sx.div_euclid(village::SPACING),sz.div_euclid(village::SPACING)).unwrap().2;
                                let pieces = village::plan(t.seed(),vk,sx,sz,&|x,z| ground.supported_height(x,z));
                                let beards = structure_terrain::near(t,cx,cz);
                                village::paint(vk,&pieces,[cx*16,cz*16,cx*16+15,cz*16+15],&|x,z| structure_terrain::street_height(&ground,x,z,&beards),&mut |village::Put::Block(x,y,z,state)| {
                                    if (cx*16..cx*16+16).contains(&x) && (cz*16..cz*16+16).contains(&z) { mask.insert((x,y,z),state); }
                                });
                            }
                            Generator::Nether(n) => {
                                let mut states = vec![-1;128*16*16];
                                let mut canvas = nether_fortress::Canvas { blocks: &mut states,x0:cx*16,z0:cz*16,passable:&|_| true,air:AIR,supports:false };
                                if kind == 17 { nether_fortress::paint(&nether_fortress::plan(n.seed(),sx,sz),&mut canvas); }
                                else { nether_bastion::paint(n.seed(),&nether_bastion::plan(n.seed(),sx,sz),&mut canvas); }
                                for (i,&state) in states.iter().enumerate() {
                                    if state >= 0 { mask.insert((cx*16+i as i32%16,i as i32/256,cz*16+i as i32/16%16),state); }
                                }
                            }
                            Generator::End(e) => {
                                let y = e.city_at(sx.div_euclid(20),sz.div_euclid(20)).unwrap().2;
                                end_city::paint(&end_city::plan(e.seed(),sx,sz,y),cx*16,cz*16,AIR,&mut |x,y,z,state| { mask.insert((x,y,z),state); });
                            }
                            _ => unreachable!(),
                        }
                        let actual = Chunk::generated(&generator,cx,cz);
                        let mut cells: Vec<_> = mask.into_iter().collect();
                        cells.sort_by_key(|&(p,_)| p);
                        for ((x,y,z),state) in cells {
                            if !blocks::has_box(state) { continue; }
                            checked += 1;
                            let seam = matches!(x.rem_euclid(16),0|15) || matches!(z.rem_euclid(16),0|15);
                            seam_checked += usize::from(seam);
                            let got = actual.block(x.rem_euclid(16),y,z.rem_euclid(16));
                            if got != state {
                                changed += 1; seam_changed += usize::from(seam);
                                if examples.len()<12 { examples.push(json!({"pos":[x,y,z],"expected":blocks::block_at_state(state),"actual":blocks::block_at_state(got)})); }
                            }
                        }
                    }
                    results.push(json!({"kind":desc["kind"],"chunk":[sx,sz],"chunks":coords.len(),"solid_cells":checked,"changed":changed,"seam_cells":seam_checked,"seam_changed":seam_changed,"examples":examples}));
                }
            }
        }
    }
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("border.json"),serde_json::to_vec_pretty(&results).unwrap()).unwrap();
}
