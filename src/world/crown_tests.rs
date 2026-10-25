// Статистические регрессии по независимому чтению region оригинала.
use super::*;
use std::collections::{BTreeMap, HashMap};

#[derive(serde::Deserialize, serde::Serialize)]
struct Sample {
    #[serde(rename = "type")]
    species: String,
    trunk: i32,
    logs: Vec<(i32, i32, i32, Option<String>)>,
    leaves: Vec<(i32, i32, i32, i32)>,
}

fn sample(species: &str, tree: &Tree, seed: i64, x: i32, z: i32) -> Sample {
    let blocks = tree.blocks_in(seed, x, z);
    let logs: BTreeMap<_, _> = blocks.iter().filter_map(|b| if let Part::Log(a) = b.3 { Some(((b.0,b.1,b.2),a)) } else { None }).collect();
    let leaves: BTreeMap<_, _> = blocks.iter().filter(|b| b.3 == Part::Leaves && !logs.contains_key(&(b.0,b.1,b.2))).map(|b| ((b.0,b.1,b.2),Part::Leaves)).collect();
    let geometry: Vec<_> = logs.iter().map(|(&(x,y,z),&a)| (x,y,z,Part::Log(a))).chain(leaves.keys().map(|&(x,y,z)| (x,y,z,Part::Leaves))).collect();
    let distance = tree.leaf_distances_in(seed,x,z,&geometry);
    let mut top = 0;
    while logs.contains_key(&(0,top+1,0)) { top += 1; }
    Sample {
        species: species.to_owned(), trunk: top,
        logs: logs.iter().map(|(&(x,y,z),a)| (x,y-top,z,Some(match a { Axis::X => "x", Axis::Y => "y", Axis::Z => "z" }.to_owned()))).collect(),
        leaves: leaves.keys().map(|&(x,y,z)| (x,y-top,z,*distance.get(&(x,y,z)).unwrap_or(&7))).collect(),
    }
}

fn metrics(s: &Sample) -> BTreeMap<String, f64> {
    let mut m = BTreeMap::new();
    let mut add = |key: String, value: f64| { *m.entry(key).or_default() += value; };
    let low = s.leaves.iter().map(|b| b.1).min().unwrap();
    let high = s.leaves.iter().map(|b| b.1).max().unwrap();
    let radius = s.leaves.iter().map(|b| b.0.abs().max(b.2.abs())).max().unwrap();
    let height = high-low+1;
    for (name,value) in [("trunk",s.trunk),("height",height),("radius",radius)] {
        add(name.into(), f64::from(value));
        for bin in 0..=20 { add(format!("{name}_bin:{bin}"),f64::from(value==bin)); }
    }
    add("leaves".into(),s.leaves.len() as f64);
    add("branches".into(),0.0);
    for axis in ["x","y","z"] { add(format!("branch_axis:{axis}"),0.0); }
    for y in -10..=10 { add(format!("branch_layer:{y}"),0.0); }
    for (x,y,z,axis) in &s.logs {
        if *x!=0 || *z!=0 {
            add("branches".into(),1.0);
            add(format!("branch_axis:{}",axis.as_deref().unwrap_or("y")),1.0);
            if (-10..=10).contains(y) { add(format!("branch_layer:{y}"),1.0); }
        }
    }
    for d in 1..=7 { add(format!("distance:{d}"),s.leaves.iter().filter(|b| b.3==d).count() as f64/s.leaves.len() as f64); }
    for y in -10..=10 {
        let layer: std::collections::HashSet<_> = s.leaves.iter().filter(|b| b.1==y).map(|b| (b.0,b.2)).collect();
        let r=layer.iter().map(|&(x,z)| x.abs().max(z.abs())).max().unwrap_or(0);
        add(format!("layer_count:{y}"),layer.len() as f64);
        add(format!("layer_radius:{y}"),f64::from(r));
        let corners = if r==0 {0} else {[-r,r].into_iter().flat_map(|x| [-r,r].map(|z| (x,z))).filter(|p| layer.contains(p)).count()};
        add(format!("layer_corner:{y}"),corners as f64/4.0);
        let corners = if layer.is_empty() {0} else {
            let xmin=layer.iter().map(|p| p.0).min().unwrap(); let xmax=layer.iter().map(|p| p.0).max().unwrap();
            let zmin=layer.iter().map(|p| p.1).min().unwrap(); let zmax=layer.iter().map(|p| p.1).max().unwrap();
            if xmin==xmax || zmin==zmax {0} else {[xmin,xmax].into_iter().flat_map(|x| [zmin,zmax].map(|z| (x,z))).filter(|p| layer.contains(p)).count()}
        };
        add(format!("box_corner:{y}"),corners as f64/4.0);
        for (x,z) in layer { add(format!("cell:{y}:{x}:{z}"),1.0); }
    }
    m
}

#[derive(serde::Deserialize)]
struct Moment { mean: f64, variance: f64 }
#[derive(serde::Deserialize)]
struct Reference { n: usize, metrics: BTreeMap<String,Moment> }

const CASES: [(&str,Species,bool);8] = [
    ("oak",OAK,false),("fancy_oak",FANCY_OAK,false),("birch",BIRCH,false),
    ("acacia",ACACIA,false),("swamp_oak",SWAMP_OAK,false),("spruce",SPRUCE,false),
    ("natural_poplar",POPLAR,false),("poplar",POPLAR,true),
];

fn generated(name: &str, species: Species, sapling: bool, i: i64, preview: bool) -> Sample {
    let mut random=Random::new(i);
    let trunk=species.tree.trunk+roll(&mut random,species.spread)+roll(&mut random,species.extra);
    let tree=Tree {trunk,..species.tree};
    let tree=if preview {if sapling {tree.in_preview_sapling(i)} else {tree.in_preview(i)}} else {tree};
    sample(name,&tree,1_792_855_987_123_998_292,i as i32*31,i as i32*19)
}

fn compare(name: &str) {
    let reference: BTreeMap<String,Reference> = serde_json::from_str(include_str!("../../testdata/world-profile/crowns-26.3.json")).unwrap();
    let reference=&reference[name];
    let (_,species,sapling)=CASES.into_iter().find(|c| c.0==name).unwrap();
    let mut totals: BTreeMap<String,(f64,f64)> = BTreeMap::new();
    let n=2048;
    for i in 0..n {
        for (key,value) in metrics(&generated(name,species,sapling,i,true)) {
            let entry=totals.entry(key).or_default(); entry.0+=value; entry.1+=value*value;
        }
    }
    // Четыре стандартные ошибки двух выборок; нули и редкие клетки не теряются.
    for key in reference.metrics.keys().chain(totals.keys()).collect::<std::collections::BTreeSet<_>>() {
        let (sum,square)=totals.get(key).copied().unwrap_or_default();
        let mean=sum/n as f64;
        let variance=((square-sum*sum/n as f64)/(n-1) as f64).max(0.0);
        let (expected,mut var)=reference.metrics.get(key).map_or((0.0,0.0),|v| (v.mean,v.variance));
        // Ноль попаданий в конечной выборке не означает нулевую вероятность.
        if key.starts_with("cell:") && (expected==0.0 || expected==1.0) {
            var=var.max(0.5/reference.n as f64);
        }
        let floor=if key.starts_with("distance:") {0.0001} else {0.02};
        let limit=4.0*(var/reference.n as f64+variance/n as f64).sqrt()+floor;
        assert!((mean-expected).abs()<=limit,"{name} {key}: {mean} против {expected}, допуск {limit}");
    }
}

macro_rules! check {
    ($test:ident,$name:literal) => { #[test] fn $test() { compare($name); } };
}
check!(oak_statistics,"oak");
check!(fancy_oak_statistics,"fancy_oak");
check!(birch_statistics,"birch");
check!(acacia_statistics,"acacia");
check!(swamp_oak_statistics,"swamp_oak");
check!(spruce_statistics,"spruce");
check!(natural_poplar_statistics,"natural_poplar");
check!(sapling_poplar_statistics,"poplar");

#[test]
fn old_profile_does_not_select_observed_shapes() {
    for (_,species,_) in CASES { assert!(species.tree.observation.is_none()); }
    for name in ["oak_sapling","birch_sapling","acacia_sapling","spruce_sapling"] {
        for seed in 0..64 { assert!(from_sapling(name,false,seed).unwrap().observation.is_none()); }
    }
    assert!(OAK.tree.in_preview(1).observation.is_some());
}

#[test]
fn observed_leaf_support_survives_rotations_and_reflections() {
    use std::io::Read;
    let mut json=String::new();
    flate2::read::GzDecoder::new(&include_bytes!("../../testdata/world-profile/crowns-26.3-observed-blocks.json.gz")[..]).read_to_string(&mut json).unwrap();
    let raw: HashMap<String,Vec<Sample>>=serde_json::from_str(&json).unwrap();
    for (name,species,sapling) in CASES {

        for seed in 0..128 {
            let tree=if sapling {species.tree.in_preview_sapling(seed)} else {species.tree.in_preview(seed)};
            let actual=sample(name,&tree,0,seed as i32,3);
            assert!(actual.logs.iter().all(|b| b.0.abs()<=TREE_REACH && b.2.abs()<=TREE_REACH));
            assert!(actual.leaves.iter().all(|b| b.0.abs()<=TREE_REACH && b.2.abs()<=TREE_REACH));
            let histogram=|s: &Sample| {let mut h=[0;7]; for b in &s.leaves {h[(b.3-1) as usize]+=1;} h};
            let geometry: Vec<_>=actual.logs.iter().map(|b| (b.0,b.1,b.2,Part::Log(Axis::Y)))
                .chain(actual.leaves.iter().map(|b| (b.0,b.1,b.2,Part::Leaves))).collect();
            let support=super::super::leaf_distances(&geometry);
            for b in &actual.leaves {
                let shortest=*support.get(&(b.0,b.1,b.2)).unwrap_or(&7);
                assert!(shortest<7, "{name}: оторванный лист");
                assert!((shortest..=shortest+1).contains(&b.3), "{name}: состояние не согласовано с опорой");
            }
            assert!(raw[name].iter().any(|s| s.trunk==actual.trunk && s.logs.len()==actual.logs.len() && histogram(s)==histogram(&actual)),"{name} seed={seed}: изменены состояния листвы или ствол");
        }
    }
}

#[test]
fn observed_crowns_vary_along_both_coordinate_axes() {
    let tree=FANCY_OAK.tree.in_preview(42);
    let variants=|along_x: bool| {
        (0..128).map(|i| {
            let (x,z)=if along_x {(i,0)} else {(0,i)};
            let blocks=tree.blocks_in(42,x,z);
            let mut geometry: Vec<_>=blocks.iter().filter(|b| b.3==Part::Leaves).map(|b| (b.0,b.1,b.2)).collect();
            geometry.sort(); geometry
        }).collect::<std::collections::HashSet<_>>().len()
    };
    assert!(variants(true)>=6 && variants(false)>=6, "симметрии не должны зависеть только от младших битов координаты");
}

#[test]
fn observed_shapes_keep_swamp_vines_and_bee_nests() {
    assert!((0..16).any(|seed| SWAMP_OAK.tree.in_preview(seed).blocks_in(seed,0,0).iter()
        .any(|b| matches!(b.3,Part::Block(name) if name.starts_with("vine[")))));
    for species in [OAK,BIRCH] {
        let tree=Tree {bees:true,..species.tree}.in_preview(42);
        let blocks=tree.blocks_in(42,0,0);
        let nest=blocks.iter().find(|b| matches!(b.3,Part::Block(name) if name.starts_with("bee_nest["))).expect("гнездо сохраняется");
        assert!(blocks.iter().any(|b| (b.0,b.1,b.2)==(0,nest.1,0) && b.3==Part::Log(Axis::Y)),"гнездо крепится к стволу");
    }
}

#[test]
#[ignore]
fn export_crown_statistics() {
    let mut output=BTreeMap::new();
    for (label,preview) in [("before",false),("after",true)] {
        let mut groups=BTreeMap::new();
        for (name,species,sapling) in CASES {
            let mut totals: BTreeMap<String,(f64,f64)>=BTreeMap::new();
            let n=2048;
            for i in 0..n {
                for (key,value) in metrics(&generated(name,species,sapling,i,preview)) {
                    let entry=totals.entry(key).or_default(); entry.0+=value; entry.1+=value*value;
                }
            }
            let moments: BTreeMap<_,_>=totals.into_iter().map(|(key,(sum,square))| (key,serde_json::json!({"mean":sum/n as f64,"variance":((square-sum*sum/n as f64)/(n-1) as f64).max(0.0)}))).collect();
            groups.insert(name,serde_json::json!({"n":n,"metrics":moments}));
        }
        output.insert(label,groups);
    }
    let path=std::env::var("MCSA_CROWN_OUTPUT").expect("путь сводки задаётся явно");
    std::fs::write(path,serde_json::to_vec(&output).unwrap()).unwrap();
}
