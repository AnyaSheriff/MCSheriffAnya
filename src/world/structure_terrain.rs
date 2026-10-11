// Локальная подгонка рельефа структур; исходные шумы мира не меняются.
// Вики, Structure definition: beard_thin — «generating terrain under the
// structure, while removing terrain inside the structure».
// https://minecraft.wiki/w/Structure_definition#JSON_format
// Вики не задаёт формулу и радиус спада [проверить]: ниже — наше
// приближение, плавный кубический спад за рамкой на 12 блоков.

use std::cell::RefCell;
use std::collections::HashMap;

use super::terrain::{Column, ReliefGrid, Style, Terrain};

pub const FALLOFF: i32 = 12;

/// Та же сетка, что у Chunk::generated, без загруженных соседних чанков.
pub struct Ground<'a> {
    terrain: &'a Terrain,
    grids: RefCell<HashMap<(i32, i32), ReliefGrid>>,
    columns: RefCell<HashMap<(i32, i32), Column>>,
}

impl<'a> Ground<'a> {
    pub fn new(terrain: &'a Terrain) -> Self {
        Self { terrain, grids: RefCell::new(HashMap::new()), columns: RefCell::new(HashMap::new()) }
    }

    fn with_grid<T>(&self, x: i32, z: i32, get: impl FnOnce(Option<&ReliefGrid>) -> T) -> T {
        if self.terrain.style() == Style::Smooth { return get(None); }
        let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
        let mut grids = self.grids.borrow_mut();
        // Каноническая рамка исходного чанка, включая его кайму. Даже
        // продолжение плотности вне сетки не зависит от размера запроса.
        let grid = grids.entry((cx, cz)).or_insert_with(|| self.terrain.relief_grid(cx * 16 - 1, cz * 16 - 1, cx * 16 + 16, cz * 16 + 16));
        get(Some(grid))
    }

    fn column(&self, x: i32, z: i32) -> Column {
        if let Some(column) = self.columns.borrow().get(&(x, z)) { return *column; }
        let column = self.with_grid(x, z, |grid| self.terrain.column_in(x, z, grid));
        self.columns.borrow_mut().insert((x, z), column);
        column
    }

    pub fn height(&self, x: i32, z: i32) -> i32 {
        self.column(x, z).height
    }

    /// Высота земли с опорой под верхним блоком. Собственный объёмный
    /// шум иногда даёт отдельную однослойную корочку над воздухом;
    /// дорога не должна заменять её и оставаться без опоры [проверить].
    pub fn supported_height(&self, x: i32, z: i32) -> i32 {
        street_height(self, x, z, &[])
    }

    fn density(&self, x: i32, y: i32, z: i32, original: i32) -> f64 {
        self.with_grid(x, z, |grid| grid.map_or(f64::from(original - y) + 0.5, |grid| grid.density(x, y, z)))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Beard {
    pub bounds: [i32; 4],
    /// Низ и верх рамки образца, включительно.
    pub floor: i32,
    pub ceiling: i32,
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Beard {
    fn weight(self, x: i32, z: i32) -> f64 {
        let dx = (self.bounds[0] - x).max(x - self.bounds[2]).max(0);
        let dz = (self.bounds[1] - z).max(z - self.bounds[3]).max(0);
        smooth(1.0 - f64::from(dx * dx + dz * dz).sqrt() / f64::from(FALLOFF))
    }

    fn density(self, source: f64, original: i32, y: i32, weight: f64) -> f64 {
        // Ниже земли и над крышей влияние затухает; глубокие пещеры
        // остаются исходными. В рамке положительная плотность под полом,
        // отрицательная внутри: добавляем склон и вырезаем склон горы.
        let low = original.min(self.floor - 1);
        let high = original.max(self.ceiling);
        let distance = (low - y).max(y - high).max(0);
        let weight = weight * smooth(1.0 - f64::from(distance) / f64::from(FALLOFF));
        source + weight * (f64::from(self.floor - y) - 0.5 - source)
    }
}

/// Перекрытия [проверить] решаются независимо от порядка: сильнейшее влияние, при
/// равенстве — верхний пол. Воздух самих комнат затем кладёт образец.
fn influence(x: i32, z: i32, beards: &[Beard]) -> Option<(Beard, f64)> {
    beards.iter().copied().map(|beard| (beard, beard.weight(x, z)))
        .filter(|(_, weight)| *weight > 0.0)
        .max_by(|(a, aw), (b, bw)| aw.total_cmp(bw).then(a.floor.cmp(&b.floor)).then(a.ceiling.cmp(&b.ceiling)))
}

/// Верх после подгонки — из той же плотности, которой заполнен чанк.
pub fn height(ground: &Ground<'_>, x: i32, z: i32, beards: &[Beard]) -> i32 {
    let original = ground.height(x, z);
    let Some((beard, weight)) = influence(x, z, beards) else { return original };
    (original.min(beard.floor) - FALLOFF..=original.max(beard.ceiling) + FALLOFF).rev()
        .find(|&y| beard.density(ground.density(x, y, z, original), original, y, weight) > 0.0)
        .unwrap_or(original)
}

/// Проекция улицы выбирает верхний непрерывный слой земли, оставляя
/// исходные висячие одиночные блоки рельефа неизменными.
pub fn street_height(ground: &Ground<'_>, x: i32, z: i32, beards: &[Beard]) -> i32 {
    let original = ground.height(x, z);
    let top = height(ground, x, z, beards);
    let influence = influence(x, z, beards);
    let solid = |y| {
        let density = ground.density(x, y, z, original);
        influence.map_or(density, |(beard, weight)| beard.density(density, original, y, weight)) > 0.0
    };
    (super::MIN_Y + 1..=top).rev().find(|&y| solid(y) && solid(y - 1)).unwrap_or(top)
}

#[cfg(not(seedmap_core))]
pub(super) fn near(terrain: &Terrain, cx: i32, cz: i32) -> Vec<Beard> {
    let mut beards = super::village::beards_near(terrain, cx, cz);
    beards.extend(super::pillager_outpost::beards_near(terrain, cx, cz));
    beards
}

#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, cx: i32, cz: i32) {
    let super::Generator::Normal(terrain) = generator else { return };
    let beards = near(terrain, cx, cz);
    if beards.is_empty() { return; }
    let ground = Ground::new(terrain);
    adapt(terrain, chunk, cx, cz, &ground, &beards);
}

#[cfg(not(seedmap_core))]
fn adapt(terrain: &Terrain, chunk: &mut super::Chunk, cx: i32, cz: i32, ground: &Ground<'_>, beards: &[Beard]) {
    for x in cx * 16..cx * 16 + 16 {
        for z in cz * 16..cz * 16 + 16 {
            let Some((beard, weight)) = influence(x, z, beards) else { continue };
            let old = ground.height(x, z);
            let top = height(ground, x, z, beards);
            let mut column = ground.column(x, z);
            column.height = top;
            for y in old.min(beard.floor) - FALLOFF..=old.max(beard.ceiling) + FALLOFF {
                let density = ground.density(x, y, z, old);
                let solid = beard.density(density, old, y, weight) > 0.0;
                let inside = weight == 1.0 && (beard.floor..=beard.ceiling).contains(&y);
                let state = if solid && (density <= 0.0 || (top - 3..=top).contains(&y)) {
                    // Покров — по биому, включая песок в пустыне.
                    let name = terrain.surface(&column, x, y, z, top - y).unwrap_or("stone");
                    crate::blocks::state_from_text(name).unwrap_or(super::AIR)
                } else if !solid && (density > 0.0 || inside || y == old + 1 && old != top) {
                    super::AIR
                } else {
                    continue;
                };
                chunk.put_generated(x - cx * 16, y, z - cz * 16, state, true);
            }
        }
    }
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    #[test]
    fn roads_skip_unsupported_single_block_terrain_skin() {
        let terrain = Terrain::new(42, Style::Vanilla);
        let ground = Ground::new(&terrain);
        assert_eq!(ground.height(-1052, -795), 80);
        assert_eq!(ground.supported_height(-1052, -795), 69);
        assert!(terrain.relief_grid(-1052, -795, -1052, -795).solid(-1052, 80, -795));
    }

    #[test]
    fn adaptation_makes_slopes_and_leaves_distant_blocks_unchanged() {
        let terrain = Terrain::new(1_791_716_133_710_533_940, Style::Vanilla);
        let ground = Ground::new(&terrain);
        let base = ground.height(-176, 340);
        let beard = Beard { bounds: [-176, 340, -172, 344], floor: base + 8, ceiling: base + 14 };
        let generator = super::super::Generator::Normal(Terrain::new(terrain.seed(), Style::Vanilla));
        let mut raised_outside = 0;
        for cx in -12..=-10 {
            for cz in 20..=22 {
                let mut chunk = super::super::Chunk::generated(&generator, cx, cz);
                let before = chunk.clone();
                adapt(&terrain, &mut chunk, cx, cz, &ground, &[beard]);
                for x in cx * 16..cx * 16 + 16 {
                    for z in cz * 16..cz * 16 + 16 {
                        let top = height(&ground, x, z, &[beard]);
                        if beard.weight(x, z) == 0.0 {
                            for y in super::super::MIN_Y..super::super::MIN_Y + super::super::WORLD_HEIGHT {
                                assert_eq!(before.block(x - cx * 16, y, z - cz * 16), chunk.block(x - cx * 16, y, z - cz * 16));
                            }
                        } else if x < beard.bounds[0] && top > ground.height(x, z) && top < beard.floor - 1 {
                            raised_outside += 1;
                            assert_ne!(chunk.block(x - cx * 16, top, z - cz * 16), super::super::AIR);
                        }
                    }
                }
            }
        }
        assert!(raised_outside > 5, "земля должна подниматься и за рамкой дома");
        let cut = Beard { floor: base - 4, ..beard };
        assert_eq!(height(&ground, -174, 342, &[cut]), base - 5);
    }

    #[test]
    fn beard_adds_below_carves_inside_and_fades_outside() {
        let beard = Beard { bounds: [0, 0, 8, 8], floor: 75, ceiling: 82 };
        assert!(beard.density(-10.0, 64, 74, 1.0) > 0.0);
        assert!(beard.density(10.0, 85, 75, 1.0) < 0.0);
        assert_eq!(beard.weight(-FALLOFF, 4), 0.0);
        let slopes: Vec<_> = (-FALLOFF..=0).map(|x| beard.weight(x, 4)).collect();
        assert!(slopes.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(slopes.iter().any(|&weight| weight > 0.0 && weight < 1.0));
        assert_eq!(beard.density(20.0, 64, 64 - FALLOFF, 1.0), 20.0);
        let lower = Beard { floor: 70, ..beard };
        assert_eq!(influence(0, 0, &[beard, lower]).unwrap().0.floor, influence(0, 0, &[lower, beard]).unwrap().0.floor);
    }
}
