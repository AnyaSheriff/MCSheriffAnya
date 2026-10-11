use std::{fs::File, io::BufWriter, path::PathBuf, time::Instant};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use seedmap_core::world::seedmap::{Map, Dimension, Marker, STRUCTURES};

const NAMES: [&str; 20] = ["Village", "Outpost", "Desert temple", "Jungle temple", "Monument", "Mansion",
    "Ancient city", "Trial chambers", "Stronghold", "Mineshaft", "Ruined portal", "Shipwreck",
    "Buried treasure", "Ocean ruins", "Trail ruins", "Igloo", "Swamp hut", "Fortress", "Bastion", "End city"];

struct Options { seed: i64, x: i32, z: i32, radius: i32, step: u32, dimension: Dimension, smooth: bool, autumn: bool, out: PathBuf }

fn options() -> Result<Option<Options>, String> {
    let mut args = std::env::args().skip(1);
    let (mut seed, mut x, mut z, mut radius, mut step, mut dimension, mut smooth, mut autumn, mut out) =
        (None, 0, 0, 2048, 4, Dimension::Overworld, false, false, PathBuf::from("map.png"));
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("seedmap --seed <i64> [--x 0 --z 0 --radius 2048 --step 4 --out map.png]\n  --dimension overworld|nether|end   --smooth   --autumn\nRadius is in blocks; PNG pixels sample every --step blocks. A legend and spawn marker are included.\nA companion .json file records settings, palette and all structure coordinates.");
            return Ok(None);
        }
        if arg == "--smooth" { smooth = true; continue; }
        if arg == "--autumn" { autumn = true; continue; }
        let value = args.next().ok_or_else(|| format!("Missing value for {arg}"))?;
        let bad = || format!("Invalid value for {arg}: {value}");
        match arg.as_str() {
            "--seed" => seed = Some(value.parse().map_err(|_| bad())?),
            "--x" => x = value.parse().map_err(|_| bad())?, "--z" => z = value.parse().map_err(|_| bad())?,
            "--radius" => radius = value.parse().map_err(|_| bad())?, "--step" => step = value.parse().map_err(|_| bad())?,
            "--dimension" => dimension = match value.as_str() { "overworld" => Dimension::Overworld, "nether" => Dimension::Nether, "end" => Dimension::End, _ => return Err(bad()) },
            "--out" => out = value.into(), _ => return Err(format!("Unknown option: {arg}")),
        }
    }
    let seed = seed.ok_or("--seed is required")?;
    if !(1..=16_384).contains(&radius) || !(1..=1024).contains(&step) || !(radius as u32 * 2).is_multiple_of(step) {
        return Err("Radius must be 1..16384, step 1..1024, and 2*radius divisible by step".into());
    }
    if out.extension().is_none_or(|ext| !ext.eq_ignore_ascii_case("png")) { return Err("--out must end in .png".into()); }
    Ok(Some(Options { seed, x, z, radius, step, dimension, smooth, autumn, out }))
}

struct Canvas { width: u32, height: u32, pixels: Vec<u8> }
impl Canvas {
    fn put(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            let at = (y as usize * self.width as usize + x as usize) * 4;
            self.pixels[at..at + 4].copy_from_slice(&color);
        }
    }
    fn text(&mut self, x: i32, y: i32, text: &str, color: [u8; 4]) {
        for (col, ch) in text.chars().enumerate() {
            if let Some(glyph) = BASIC_FONTS.get(ch) {
                for (row, bits) in glyph.iter().enumerate() {
                    for bit in 0..8 { if bits & (1 << bit) != 0 { self.put(x + col as i32 * 8 + bit, y + row as i32, color); } }
                }
            }
        }
    }
    fn badge(&mut self, x: i32, y: i32, icon: &str, color: [u8; 4]) {
        for dy in -7..=7 { for dx in -7..=7 { self.put(x + dx, y + dy, if dx.abs() == 7 || dy.abs() == 7 { [20, 25, 35, 255] } else { color }); } }
        self.text(x - 4, y - 4, icon, [20, 25, 35, 255]);
    }
}
fn hex(text: &str) -> [u8; 4] {
    let rgb = u32::from_str_radix(&text[1..], 16).expect("palette hex");
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255]
}

fn run(o: Options) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let x0 = o.x.checked_sub(o.radius).ok_or("X overflow")?;
    let z0 = o.z.checked_sub(o.radius).ok_or("Z overflow")?;
    let x1 = o.x.checked_add(o.radius).ok_or("X overflow")?;
    let z1 = o.z.checked_add(o.radius).ok_or("Z overflow")?;
    let size = (o.radius * 2) as u32 / o.step;
    let map = Map::new(o.seed, o.dimension, o.smooth, o.autumn);
    let raster = map.raster(x0, z0, size, size, o.step)?;
    let biome_time = start.elapsed();
    let biome_hash = raster.iter().fold(2_166_136_261u32, |hash, byte| (hash ^ u32::from(*byte)).wrapping_mul(16_777_619));
    let mut markers: Vec<Marker> = Vec::new();
    for info in STRUCTURES { markers.extend(map.structures(info.id, x0, z0, x1, z1)?); }
    let spawn = (o.dimension == Dimension::Overworld).then(|| map.spawn());
    let generation_time = start.elapsed();
    let (width, height) = (size + 320, (size + 40).max(600));
    let mut canvas = Canvas { width, height, pixels: [22, 29, 40, 255].repeat(width as usize * height as usize) };
    for row in 0..size as usize {
        let at = ((row + 32) * width as usize) * 4;
        canvas.pixels[at..at + size as usize * 4].copy_from_slice(&raster[row * size as usize * 4..(row + 1) * size as usize * 4]);
    }
    let white = [224, 234, 241, 255];
    canvas.text(10, 12, &format!("MCSheriffAnya / seed {} / {:?}", o.seed, o.dimension), white);
    for m in &markers {
        let info = &STRUCTURES[m.kind as usize];
        canvas.badge((m.x - x0) / o.step as i32, 32 + (m.z - z0) / o.step as i32, info.icon, hex(info.color));
    }
    if let Some((x, _, z)) = spawn.filter(|(x, _, z)| *x >= x0 as f64 && *x < x1 as f64 && *z >= z0 as f64 && *z < z1 as f64) {
        canvas.badge((x as i32 - x0) / o.step as i32, 32 + (z as i32 - z0) / o.step as i32, "+", [255, 255, 255, 255]);
    }
    let left = size as i32 + 16;
    for (line, text) in [format!("X [{x0}, {x1})"), format!("Z [{z0}, {z1})"), format!("1 pixel = {} blocks", o.step),
        format!("Terrain: {}", if o.smooth { "smooth" } else { "normal" }), format!("Autumn forests: {}", o.autumn), "North (-Z) is up".into()].iter().enumerate() {
        canvas.text(left, 36 + line as i32 * 16, text, white);
    }
    let dim = match o.dimension { Dimension::Overworld => 1, Dimension::Nether => 2, Dimension::End => 4 };
    let mut line = 0;
    for info in STRUCTURES.iter().filter(|s| s.dimensions & dim != 0) {
        let y = 154 + line * 22;
        canvas.badge(left + 6, y, info.icon, hex(info.color));
        canvas.text(left + 23, y - 4, NAMES[info.id as usize], white);
        line += 1;
    }
    if let Some((x, y, z)) = spawn {
        canvas.text(left, 154 + line * 22, &format!("+ Spawn {},{},{}", x.floor(), y, z.floor()), white);
    }
    canvas.text(left, height as i32 - 28, "Markers: start chunk centers", white);
    canvas.text(left, height as i32 - 14, "Treasure: chest X/Z", white);
    let file = File::create(&o.out)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba); encoder.set_depth(png::BitDepth::Eight);
    encoder.add_text_chunk("Seed".into(), o.seed.to_string())?;
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&canvas.pixels)?; writer.finish()?;
    let palette: Vec<_> = (0..65).map(|id| serde_json::json!({"id": id, "name": seedmap_core::world::seedmap::biome_name(id), "rgba": seedmap_core::world::seedmap::color(id)})).collect();
    let json = serde_json::json!({"seed": o.seed.to_string(), "dimension": format!("{:?}", o.dimension), "smooth": o.smooth,
        "autumn": o.autumn, "biome_rgba_fnv1a": format!("{biome_hash:08x}"), "biome_size": size, "bounds": [x0,z0,x1,z1], "step": o.step, "spawn": spawn, "structures": markers, "types": STRUCTURES, "palette": palette});
    serde_json::to_writer_pretty(BufWriter::new(File::create(o.out.with_extension("json"))?), &json)?;
    println!("{}: {}x{} blocks, {}x{} biome pixels, {} markers; biomes {:.3}s, generation {:.3}s, total {:.3}s",
        o.out.display(), o.radius * 2, o.radius * 2, size, size, markers.len(), biome_time.as_secs_f64(), generation_time.as_secs_f64(), start.elapsed().as_secs_f64());
    Ok(())
}

fn main() {
    let result = options().map_err(|e| -> Box<dyn std::error::Error> { e.into() }).and_then(|o| match o { Some(o) => run(o), None => Ok(()) });
    if let Err(e) = result { eprintln!("seedmap: {e}"); std::process::exit(2); }
}
