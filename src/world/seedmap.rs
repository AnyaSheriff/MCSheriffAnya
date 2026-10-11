// Общие запросы карты: только биомы и настоящие старты нашего генератора.
// Маркер означает середину стартового чанка, а не всю рамку постройки.
use super::{ancient_city, end_city, mansion, mineshaft, monument, nether, ocean_ruin,
    pillager_outpost, ruined_portal, shipwreck, spawn_search, stronghold, temples,
    terrain::{self, Biome, Style, Terrain}, the_end, trail_ruins, trial_chambers, village};
use std::sync::OnceLock;

pub const LIMIT: i32 = 29_000_000;
pub const MAX_SPAN: i32 = 32_768;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension { Overworld, Nether, End }

impl Dimension {
    pub fn from_id(id: u8) -> Result<Self, String> {
        match id { 0 => Ok(Self::Overworld), 1 => Ok(Self::Nether), 2 => Ok(Self::End), _ => Err("Unknown dimension".into()) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Marker { pub kind: u8, pub x: i32, pub z: i32 }

#[derive(Clone, Copy, serde::Serialize)]
pub struct Structure {
    pub id: u8,
    pub name: &'static str,
    pub icon: &'static str,
    pub color: &'static str,
    pub dimensions: u8,
}

pub const STRUCTURES: [Structure; 20] = [
    Structure { id: 0, name: "Деревня", icon: "V", color: "#efce70", dimensions: 1 },
    Structure { id: 1, name: "Аванпост", icon: "O", color: "#bb8272", dimensions: 1 },
    Structure { id: 2, name: "Храм пустыни", icon: "D", color: "#ffcc80", dimensions: 1 },
    Structure { id: 3, name: "Храм джунглей", icon: "J", color: "#a4d178", dimensions: 1 },
    Structure { id: 4, name: "Монумент", icon: "M", color: "#60d5c5", dimensions: 1 },
    Structure { id: 5, name: "Особняк", icon: "W", color: "#c2a5ef", dimensions: 1 },
    Structure { id: 6, name: "Древний город", icon: "A", color: "#43bccc", dimensions: 1 },
    Structure { id: 7, name: "Камеры испытаний", icon: "T", color: "#f89959", dimensions: 1 },
    Structure { id: 8, name: "Крепость (Stronghold)", icon: "S", color: "#e2e8bc", dimensions: 1 },
    Structure { id: 9, name: "Шахта", icon: "m", color: "#d8ac80", dimensions: 1 },
    Structure { id: 10, name: "Разрушенный портал", icon: "P", color: "#ec85df", dimensions: 3 },
    Structure { id: 11, name: "Кораблекрушение", icon: "K", color: "#c7aa85", dimensions: 1 },
    Structure { id: 12, name: "Клад", icon: "X", color: "#ffe66d", dimensions: 1 },
    Structure { id: 13, name: "Руины океана", icon: "R", color: "#9ecada", dimensions: 1 },
    Structure { id: 14, name: "Руины тропы", icon: "r", color: "#de9c7a", dimensions: 1 },
    Structure { id: 15, name: "Иглу", icon: "I", color: "#f0f5ff", dimensions: 1 },
    Structure { id: 16, name: "Хижина ведьмы", icon: "H", color: "#b0c26e", dimensions: 1 },
    Structure { id: 17, name: "Крепость Незера", icon: "F", color: "#eaa0a0", dimensions: 2 },
    Structure { id: 18, name: "Бастион", icon: "B", color: "#d3bb72", dimensions: 2 },
    Structure { id: 19, name: "Город Энда", icon: "E", color: "#d9b2f7", dimensions: 4 },
];

pub struct Map {
    pub dimension: Dimension,
    terrain: Terrain,
    nether: OnceLock<nether::Nether>,
    end: OnceLock<the_end::End>,
    strongholds: OnceLock<Vec<(i32, i32)>>,
    autumn: bool,
}

impl Map {
    pub fn new(seed: i64, dimension: Dimension, smooth: bool, autumn: bool) -> Self {
        Self { dimension, terrain: Terrain::new(seed, if smooth { Style::Smooth } else { Style::Vanilla }),
            nether: OnceLock::new(), end: OnceLock::new(), strongholds: OnceLock::new(), autumn }
    }

    fn nether(&self) -> &nether::Nether { self.nether.get_or_init(|| nether::Nether::new(self.terrain.seed())) }
    fn end(&self) -> &the_end::End { self.end.get_or_init(|| the_end::End::new(self.terrain.seed())) }

    pub fn biome(&self, x: i32, z: i32) -> u8 {
        // Сервер записывает биом клетки 4×4 по её середине во всех измерениях.
        let (x, z) = ((x & !3) + 2, (z & !3) + 2);
        let biome = match self.dimension {
            Dimension::Overworld => self.terrain.biome_at(x, z),
            Dimension::Nether => self.nether().biome_at(x, z),
            Dimension::End => self.end().biome_at(x, z),
        };
        if self.dimension == Dimension::Overworld && biome == Biome::Forest && self.terrain.autumn_enabled_at(x, z, self.autumn) {
            terrain::autumn_forest_number() as u8
        } else { biome.number() as u8 }
    }

    pub fn spawn(&self) -> (f64, f64, f64) {
        spawn_search::find(|x, z| self.terrain.column_at(x, z))
    }

    pub fn raster(&self, x: i32, z: i32, width: u32, height: u32, step: u32) -> Result<Vec<u8>, String> {
        if width == 0 || height == 0 || width > 2048 || height > 2048 || step == 0 || step > 1024 {
            return Err("Image dimensions must be 1..2048; step 1..1024".into());
        }
        bounds(x, z, i64::from(x) + i64::from(width) * i64::from(step), i64::from(z) + i64::from(height) * i64::from(step))?;
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in 0..height {
            for col in 0..width {
                let id = self.biome(x + (col * step) as i32, z + (row * step) as i32);
                rgba.extend_from_slice(&color(id));
            }
        }
        Ok(rgba)
    }

    pub fn structures(&self, kind: u8, x0: i32, z0: i32, x1: i32, z1: i32) -> Result<Vec<Marker>, String> {
        bounds(x0, z0, i64::from(x1), i64::from(z1))?;
        let Some(info) = STRUCTURES.get(usize::from(kind)) else { return Err("Unknown structure".into()) };
        let dim = match self.dimension { Dimension::Overworld => 1, Dimension::Nether => 2, Dimension::End => 4 };
        if info.dimensions & dim == 0 { return Ok(Vec::new()); }
        let mut out = Vec::new();
        let mut push = |cx: i32, cz: i32| {
            let offset = if kind == 12 { 9 } else { 8 };
            let (x, z) = (cx * 16 + offset, cz * 16 + offset);
            if x >= x0 && x < x1 && z >= z0 && z < z1 { out.push(Marker { kind, x, z }); }
        };
        let t = &self.terrain;
        if kind == 8 {
            for &(cx, cz) in self.strongholds.get_or_init(|| stronghold::starts_of(t).as_ref().clone()) { push(cx, cz); }
            return Ok(out);
        }
        let spacing = match kind {
            0 => village::SPACING, 1 => pillager_outpost::SPACING,
            2 | 3 | 15 | 16 => temples::SPACING, 4 => monument::SPACING, 5 => mansion::SPACING,
            6 => ancient_city::SPACING, 7 => trial_chambers::SPACING, 9 | 12 => 1,
            10 => ruined_portal::SPACING, 11 => shipwreck::SPACING, 13 => ocean_ruin::SPACING,
            14 => trail_ruins::SPACING, 17 | 18 => nether::STRUCTURE_SPACING, 19 => end_city::REGION,
            _ => unreachable!(),
        };
        for rx in x0.div_euclid(16).div_euclid(spacing)..=(x1 - 1).div_euclid(16).div_euclid(spacing) {
            for rz in z0.div_euclid(16).div_euclid(spacing)..=(z1 - 1).div_euclid(16).div_euclid(spacing) {
                let chunk = match kind {
                    0 => village::start_in(t, rx, rz).map(|(x, z, _)| (x, z)),
                    1 => pillager_outpost::start_in(t, rx, rz),
                    2 | 3 | 16 => temples::start_in(match kind { 2 => temples::Kind::DesertPyramid, 3 => temples::Kind::JunglePyramid, _ => temples::Kind::SwampHut }, t, rx, rz).map(|s| s.chunk),
                    4 => monument::start_in(t, rx, rz), 5 => mansion::start_in(t, rx, rz).map(|s| s.chunk),
                    6 => ancient_city::start_in(t, rx, rz), 7 => trial_chambers::start_in(t, rx, rz),
                    9 => mineshaft::start_at(t, rx, rz).map(|_| (rx, rz)),
                    10 => match self.dimension {
                        Dimension::Nether => ruined_portal::start_in_nether(self.nether(), rx, rz),
                        _ => ruined_portal::start_in(t, rx, rz),
                    }.map(|s| s.chunk),
                    11 => shipwreck::start_in(t, rx, rz).map(|s| s.chunk),
                    12 => shipwreck::treasure_in(t, rx, rz).map(|_| (rx, rz)),
                    13 => ocean_ruin::site_in(t, rx, rz).map(|s| s.chunk),
                    14 => trail_ruins::site_in(t, rx, rz).map(|s| s.chunk),
                    15 => temples::igloo_in(t, rx, rz).map(|s| s.chunk),
                    17 => self.nether().fortress_start(rx, rz), 18 => self.nether().bastion_start(rx, rz),
                    19 => self.end().city_at(rx, rz).map(|(x, z, _)| (x, z)),
                    _ => unreachable!(),
                };
                if let Some((cx, cz)) = chunk { push(cx, cz); }
            }
        }
        Ok(out)
    }
}

fn bounds(x0: i32, z0: i32, x1: i64, z1: i64) -> Result<(), String> {
    if i64::from(x0) < -i64::from(LIMIT) || i64::from(z0) < -i64::from(LIMIT)
        || x1 > i64::from(LIMIT) || z1 > i64::from(LIMIT)
        || x1 <= i64::from(x0) || z1 <= i64::from(z0)
        || x1 - i64::from(x0) > i64::from(MAX_SPAN) || z1 - i64::from(z0) > i64::from(MAX_SPAN) {
        return Err(format!("Region must be inside ±{LIMIT}, with sides 1..{MAX_SPAN} blocks"));
    }
    Ok(())
}

pub fn biome_name(id: u8) -> &'static str {
    if u16::from(id) == terrain::autumn_forest_number() { return terrain::AUTUMN_FOREST; }
    Biome::ALL.iter().chain(Biome::NETHER.iter()).chain(Biome::END.iter())
        .find(|b| b.number() == i32::from(id)).map_or("unknown", |b| b.name())
}

// Собственная палитра: вода синяя, леса зелёные, снег светлый, Незер красный,
// Энд лиловый. Полная таблица автоматически попадает в легенду и README.
pub fn color(id: u8) -> [u8; 4] {
    const RGB: [u32; 65] = [
        0x3765a3, 0x233c78, 0x477a9c, 0x294663, 0x849fae, 0x536b83, 0x398aa5, 0x286079, 0x40b6b2,
        0xa775a8, 0xe8d69a, 0xe2dfc9, 0x999890, 0x4885ca, 0xa6cae4, 0x687447, 0x4b7456,
        0x91b96a, 0xb8c961, 0xe4edf2, 0xb8d7ec, 0x4f8545, 0x7fa354, 0x80a76a, 0x658958,
        0x3d573d, 0x477c70, 0xa3c1be, 0x5d8171, 0x38695b, 0x347039, 0x689f4d, 0x4f993d,
        0xb6ae59, 0xe4c782, 0x91b980, 0xe3adc3, 0xa3984a, 0x9db8ad, 0xd5e0e4, 0xb8c5cc,
        0xc5e4ee, 0xa0a297, 0x82917c, 0x969b8d, 0x60836a, 0xb1a46d, 0xba754f, 0xd29462,
        0x9a804d, 0xb5bba9, 0x50a77d, 0x9a8070, 0x284b53, 0xcf863e,
        0x91483f, 0x8c304b, 0x348d88, 0x79635a, 0x514d56,
        0xd9d7a0, 0x494257, 0x9288a8, 0xb9a8c5, 0x6e647e,
    ];
    let rgb = RGB.get(usize::from(id)).copied().unwrap_or(0xff00ff);
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255]
}

#[cfg(all(test, seedmap_core))]
mod tests {
    use super::*;

    #[test]
    fn validates_regions_and_keeps_negative_cells() {
        let map = Map::new(i64::MIN, Dimension::Overworld, false, false);
        assert!(map.raster(i32::MAX, 0, 16, 16, 4).is_err());
        assert!(map.raster(0, 0, 0, 16, 4).is_err());
        assert!(map.structures(255, 0, 0, 16, 16).is_err());
        assert_eq!(map.biome(-4, -4), map.biome(-1, -1));
        for id in 0..65 { assert_ne!(biome_name(id), "unknown"); }
    }

    #[test]
    fn structure_tiles_partition_without_loss_or_duplicates() {
        let map = Map::new(123, Dimension::Overworld, false, false);
        for kind in [0, 7, 8, 9, 10, 12, 15] {
            let whole = map.structures(kind, -1024, -1024, 1024, 1024).unwrap();
            let mut split = map.structures(kind, -1024, -1024, -1, 1024).unwrap();
            split.extend(map.structures(kind, -1, -1024, 1024, 1024).unwrap());
            let key = |m: &Marker| (m.x, m.z);
            let mut whole = whole;
            whole.sort_by_key(key); split.sort_by_key(key);
            assert_eq!(whole, split, "kind {kind}");
        }
    }
}
