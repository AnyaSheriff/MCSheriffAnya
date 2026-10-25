// Офлайн-сетка по собственной генерации; JSON-план задаёт окна и шаг.
use seedmap_core::world::terrain::{Terrain, Style};
use std::{fs, io::{BufWriter, Write}};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "biome_profile_263 PLAN OUTPUT");
    let plan: serde_json::Value = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let terrain = Terrain::new(plan["seed"].as_i64().unwrap(), Style::Vanilla).with_preview(true);
    let mut out = BufWriter::new(fs::File::create(&args[2]).unwrap());
    writeln!(out, "x,z,biome,height,continent,temperature").unwrap();
    for rect in plan["rects"].as_array().unwrap() {
        let n: Vec<i32> = rect.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
        let [x0,z0,width,depth,step] = n[..] else { panic!("окно x,z,width,depth,step") };
        assert!(width>0 && depth>0 && step>0);
        let grid = plan["heights"].as_bool().unwrap_or(false).then(|| terrain.relief_grid(x0,z0,x0+width-1,z0+depth-1));
        for z in (z0..z0+depth).step_by(step as usize) { for x in (x0..x0+width).step_by(step as usize) {
            let biome = terrain.biome_at(x,z);
            let height = grid.as_ref().map_or(-999, |g| terrain.column_in(x,z,Some(g)).height);
            let c = terrain.climate_at(x,z);
            writeln!(out,"{x},{z},{},{height},{},{}",biome.name(),c.continent,c.temperature).unwrap();
        } }
    }
}
