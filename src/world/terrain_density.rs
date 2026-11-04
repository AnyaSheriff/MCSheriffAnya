// Независимая модель роутера, а не параметры из архивов игры.
// https://minecraft.wiki/w/World_generation: «height bias» и «base height».
// https://minecraft.wiki/w/Density_function: «x/2 - x*x*x/24».
// Узлы сплайнов и масштабы шумов — подгонка по регионам переписи [проверить].
use super::{Climate, ReliefGrid, CELL_HEIGHT, CELL_WIDTH};
use crate::world::noise::Noise;

pub(super) struct Router {
    lower: Noise,
    upper: Noise,
    selector: Noise,
    jagged: Noise,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Shape {
    pub offset: f64,
    pub factor: f64,
    pub jaggedness: f64,
    pub vertical_scale: f64,
}

impl Router {
    pub fn new(seed: i64) -> Self {
        Self {
            lower: Noise::new(seed ^ 0x0041_364C_4F57_4552),
            upper: Noise::new(seed ^ 0x0041_3655_5050_4552),
            selector: Noise::new(seed ^ 0x0041_3653_454C_4543),
            jagged: Noise::new(seed ^ 0x0041_364A_4147_4745),
        }
    }

    // Температура и влажность сюда не входят: вики, World generation,
    // «used only in biome generation». Знак W остаётся отдельной координатой.
    pub fn shape(&self, c: &Climate) -> Shape {
        let pv = c.peaks_valleys();
        let inland = curve(&[(-0.22, 0.0), (-0.11, 0.25), (0.03, 0.65), (0.3, 1.0)], c.continent);
        let mountain = curve(&[(-1.0, 1.0), (-0.78, 0.95), (-0.375, 0.55), (-0.22, 0.25), (0.05, 0.0)], c.erosion);
        let ridge = curve(&[(-1.0, 0.0), (-0.4, 0.15), (-0.2, 0.3), (0.2, 0.65), (0.7, 0.95), (1.0, 1.0)], pv);
        let negative_w = curve(&[(-0.1, 1.0), (0.1, 0.0)], c.weirdness);
        let shattered = curve(&[(0.4, 0.0), (0.45, 1.0), (0.55, 1.0), (0.62, 0.0)], c.erosion);
        let sea = curve(&[(-1.2, 78.0), (-1.05, 38.0), (-0.455, 41.0), (-0.25, 47.0),
            (-0.19, 58.0), (-0.11, 63.0), (0.03, 64.0), (0.3, 67.0), (1.0, 70.0)], c.continent);
        let plateau = curve(&[(0.03, 0.0), (0.3, 1.0)], c.continent)
            * curve(&[(-0.4, 1.0), (0.05, 0.6), (0.45, 0.0)], c.erosion)
            * curve(&[(-0.85, 0.0), (-0.2, 1.0), (0.55, 1.0), (0.85, 0.0)], pv);
        // Узлы offset подогнаны по p50/p95/p99 двух региональных выборок,
        // без сравнения высот в одинаковых координатах. Экстраполяция [проверить].
        let coast_peak = curve(&[(0.03,1.0),(0.15,0.0),(0.3,0.0)], c.continent);
        let middle_peak = curve(&[(0.03,0.0),(0.15,1.0),(0.3,0.0)], c.continent);
        let inland_peak = curve(&[(0.03,0.0),(0.15,0.0),(0.3,1.0)], c.continent);
        let terrace = curve(&[(-0.375,0.0),(-0.1,1.0),(0.05,1.0),(0.45,0.0)], c.erosion)
            * curve(&[(-0.2,0.0),(0.2,0.5),(0.7,1.0)], pv)
            * curve(&[(0.03,0.0),(0.3,1.0)], c.continent);
        let saddle = curve(&[(-0.375,1.0),(-0.2225,1.0),(0.05,0.0)], c.erosion)
            * curve(&[(-0.85,0.0),(-0.2,1.0),(0.2,0.0)], pv);
        let lowland = curve(&[(-0.22,0.0),(0.05,1.0),(0.45,1.0),(0.55,0.0)], c.erosion);
        let far_peaks = curve(&[(0.45,0.0),(0.7,60.0),(1.0,100.0)], c.continent)
            * mountain * ridge;
        let height = sea + inland * (-12.894 * mountain
            + mountain * ridge * (68.0 * coast_peak + 140.0 * middle_peak
                + 134.0 * inland_peak + 9.0 * negative_w)
            - 9.333 * plateau + 3.0 * shattered + 18.0 * shattered * ridge
            + 45.383 * terrace + 10.42 * saddle - 15.436 * lowland + 11.008 + far_peaks);
        let valley = curve(&[(-1.0, 1.0), (-0.85, 0.9), (-0.65, 0.0)], pv);
        let height = height + (52.0 - height).min(0.0) * valley * inland;
        let (factor, broken) = terrain_factor(c);
        Shape {
            offset: (height - 64.0) / 128.0,
            factor,
            jaggedness: inland * mountain * ridge * negative_w * 0.35,
            vertical_scale: 64.0 + 960.0 * broken.sqrt(),
        }
    }

    fn jagged_noise(&self, x: i32, z: i32) -> f64 {
        let noise = self.jagged.octaves(f64::from(x) / 32.0 + 0.37, f64::from(z) / 32.0 + 0.61, 3);
        if noise < 0.0 { noise * 0.5 } else { noise }
    }

    pub fn density(&self, s: Shape, x: i32, y: i32, z: i32) -> f64 {
        // World generation: глубина меняется на «1/128» за блок.
        // Изрезанность — двумерная; положительная глубина усиливается до
        // добавления base_3d_noise, а не после него, иначе остаются полки.
        let depth = s.offset - (f64::from(y) - 64.0) / 128.0;
        let sloped = (depth + s.jaggedness * self.jagged_noise(x, z)) * s.factor;
        let sloped = if sloped > 0.0 { sloped * 4.0 } else { sloped };
        let (x, y3, z) = (f64::from(x), f64::from(y), f64::from(z));
        let alpha = (0.5 + self.selector.octaves3(x / 160.0 + 0.37, y3 / 96.0, z / 160.0, 2)).clamp(0.0, 1.0);
        let lo = self.lower.octaves3(x / 32.0 + 0.37, y3 / s.vertical_scale + 0.61, z / 32.0, 3);
        let hi = self.upper.octaves3(x / 32.0 + 0.61, y3 / s.vertical_scale + 0.37, z / 32.0, 3);
        let density = sloped + (lo + (hi - lo) * alpha) * 1.8;
        let v = density.clamp(-1.0, 1.0);
        let squeezed = v / 2.0 - v * v * v / 24.0;
        let top = ((y3 - 240.0) / 64.0).clamp(0.0, 1.0);
        let bottom = ((-40.0 - y3) / 24.0).clamp(0.0, 1.0);
        let slid = squeezed + (-0.458_333_333_333_333_3 - squeezed) * top;
        // Единицы сетки остаются блоковыми для собственной подгонки построек.
        (slid + (0.458_333_333_333_333_3 - slid) * bottom) * 64.0
    }

    pub fn grid(&self, x0: i32, z0: i32, across_x: i32, across_z: i32,
        climate: impl Fn(i32, i32) -> Climate) -> ReliefGrid {
        // Полная полоса: верх нельзя отрезать оценкой плоской поверхности.
        let y0 = -64;
        let layers = 384 / CELL_HEIGHT + 1;
        let mut nodes = Vec::with_capacity((across_x * across_z * layers) as usize);
        for cx in 0..across_x { for cz in 0..across_z {
            let (x, z) = (x0 + cx * CELL_WIDTH, z0 + cz * CELL_WIDTH);
            let shape = self.shape(&climate(x, z));
            for layer in 0..layers { nodes.push(self.density(shape, x, y0 + layer * CELL_HEIGHT, z)); }
        } }
        ReliefGrid { x0, z0, across_z, y0, layers, nodes }
    }

    // 26.3, Noise router: «chunk_surface_level». Это поиск верха по роутеру
    // в данном столбце; удалённое initial_density_without_jaggedness не нужно.
    // Это собственная оценка, не восстановленная функция оригинала. Отступ
    // вниз для водоносных слоёв и правил поверхности пока [проверить].
    pub fn chunk_surface_level(&self, c: &Climate, x: i32, z: i32) -> i32 {
        let shape = self.shape(c);
        (-64..320).rev().find(|&y| self.density(shape, x, y, z) > 0.0).unwrap_or(-64)
    }
}

// Один factor используется и в роутере, и в защите поверхности пещер.
// Обратный factor задаёт размах в блоках; его интерполяция сохраняет
// редкие береговые хвосты вместо почти постоянного морского градиента.
fn terrain_factor(c: &Climate) -> (f64, f64) {
    let inland = curve(&[(-0.22,0.0),(-0.11,0.25),(0.03,0.65),(0.3,1.0)], c.continent);
    let ordinary = curve(&[(-1.0,5.0),(-0.375,5.0),(0.05,6.0),(0.4,6.0),
        (0.45,1.3),(0.55,1.3),(0.62,6.0),(1.0,7.0)], c.erosion);
    let broken = curve(&[(0.43,0.0),(0.46,1.0),(0.54,1.0),(0.57,0.0)], c.erosion)
        * curve(&[(-0.2,0.0),(-0.17,1.0),(0.03,1.0),(0.3,0.0)], c.continent)
        * curve(&[(0.1,0.0),(0.35,1.0)], c.weirdness.abs());
    let coast_factor = curve(&[(-0.19,0.2),(-0.15,0.25),(-0.1,0.4),(0.03,0.5),(0.3,1.3)], c.continent)
        * (1.0 + curve(&[(-0.1,1.0),(0.1,0.0)], c.weirdness));
    let regular_factor = 7.0 + (ordinary - 7.0) * inland;
    (1.0 / ((1.0 - broken) / regular_factor + broken / coast_factor), broken)
}

// Связь пещер с положительной плотностью, а не изменение их шумов.
// Диапазон защиты подогнан по готовым полосам jagged_peaks Y128/Y144;
// глубокие полости и старый профиль сохраняются [проверить].
pub(super) fn cave_surface_protection(c: &Climate, y: i32, depth: i32) -> f64 {
    if y < 96 { return 0.0; }
    let pv = c.peaks_valleys();
    let mountain = curve(&[(-0.375,1.0),(-0.2225,0.0)], c.erosion)
        * curve(&[(0.2,0.0),(0.7,1.0)], pv);
    let (factor, broken) = terrain_factor(c);
    if mountain == 0.0 && broken == 0.0 { return 0.0; }
    let positive_bias = f64::from(depth) * factor * 4.0 / 128.0;
    let altitude = curve(&[(96.0,0.0),(128.0,0.5),(144.0,1.0)], f64::from(y));
    (mountain * 0.4).max(broken * 0.18) * altitude
        * ((10.0 - positive_bias) / 8.0).clamp(0.0,1.0)
}

// Монотонный кубический сплайн Эрмита: гармоническая производная не
// создаёт выбросов между измеренными узлами. Это наш выбор [проверить].
fn curve(points: &[(f64, f64)], x: f64) -> f64 {
    if x <= points[0].0 { return points[0].1; }
    let tangent = |i: usize| {
        let slope = |a: usize, b: usize| (points[b].1 - points[a].1) / (points[b].0 - points[a].0);
        if i == 0 { return slope(0, 1); }
        if i == points.len() - 1 { return slope(i - 1, i); }
        let (a, b) = (slope(i - 1, i), slope(i, i + 1));
        if a * b <= 0.0 { 0.0 } else { 2.0 * a * b / (a + b) }
    };
    for i in 0..points.len() - 1 {
        let (a, b) = (points[i], points[i + 1]);
        if x <= b.0 {
            let width = b.0 - a.0;
            let t = (x - a.0) / width;
            return (2.0*t*t*t - 3.0*t*t + 1.0)*a.1 + (t*t*t - 2.0*t*t + t)*width*tangent(i)
                + (-2.0*t*t*t + 3.0*t*t)*b.1 + (t*t*t - t*t)*width*tangent(i + 1);
        }
    }
    points.last().unwrap().1
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;
    use super::super::{Biome, Terrain, Style};

    fn candidate(seed: i64) -> Terrain {
        let mut t = Terrain::new(seed, Style::Vanilla).with_preview(true);
        t.density_router = Some(Box::new(Router::new(seed)));
        t
    }

    #[test]
    fn shape_uses_weirdness_sign_without_using_temperature_or_humidity() {
        let r = Router::new(42);
        let c = Climate { continent: 0.4, erosion: -0.5, weirdness: -0.65,
            temperature: -0.8, humidity: -0.7 };
        let a = r.shape(&c);
        let b = r.shape(&Climate { temperature: 0.8, humidity: 0.7, ..c });
        assert_eq!([a.offset, a.factor, a.jaggedness, a.vertical_scale], [b.offset, b.factor, b.jaggedness, b.vertical_scale]);
        let opposite = Climate { weirdness: -c.weirdness, ..c };
        assert_eq!(c.peaks_valleys(), opposite.peaks_valleys());
        assert!(a.jaggedness > r.shape(&opposite).jaggedness);
    }

    /// Шов проверяется по плотности внутри клетки, включая отрицательные
    /// координаты, а не только по одинаковым целым узлам решётки.
    #[test]
    fn independent_router_windows_join_without_a_seam() {
        let t = candidate(1792932773345722119);
        let a = t.relief_grid(-17, -17, 16, 16);
        let b = t.relief_grid(-1, -1, 32, 32);
        for x in -1..16 { for z in -1..16 { for y in (-64..320).step_by(3) {
            assert_eq!(a.density(x, y, z), b.density(x, y, z), "{x}/{y}/{z}");
        } } }
        for x in -1..16 { for z in -1..16 {
            assert!(a.solid(x, -64, z));
            assert!(!a.solid(x, 319, z));
        } }
    }

    /// Высота сама по себе не снимает лесную почву: проверяем настоящий
    /// чанк выше старого порога Y112, сохраняя оголение крутых склонов.
    #[test]
    fn generated_high_forest_keeps_its_soil() {
        let t = candidate(1792932773345722119);
        let (cx, cz) = (-110, -86);
        let grid = t.relief_grid(cx*16-1, cz*16-1, cx*16+16, cz*16+16);
        let generator = crate::world::Generator::Normal(t);
        let crate::world::Generator::Normal(t) = &generator else { unreachable!() };
        let chunk = crate::world::Chunk::generated(&generator, cx, cz);
        let (mut eligible, mut dirt) = (0, 0);
        for x in 0..16 { for z in 0..16 {
            let (wx, wz) = (cx*16+x, cz*16+z);
            let column = t.column_in(wx, wz, Some(&grid));
            if column.height <= 120 || t.weather_biome(&column, wx, wz) != Biome::Forest { continue; }
            let steep = grid.top(wx, wz+1)-grid.top(wx, wz-1)>=4
                || grid.top(wx-1, wz)-grid.top(wx+1, wz)>=4;
            if steep { continue; }
            eligible += 1;
            dirt += usize::from(crate::blocks::block_at_state(chunk.block(x, column.height-2, z)) == Some("dirt"));
        } }
        assert!(eligible >= 16, "недостаточно высоких лесных столбцов: {eligible}");
        assert!(dirt*100 >= eligible*90, "почва высокого леса {dirt}/{eligible}");
    }

    /// Проверка готовых блоков: нельзя объявить исправлением пустой
    /// знаменатель после удаления всех высоких столбцов.
    #[test]
    fn generated_router_mountains_have_material_below_the_upper_surface() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/batch-a6/support.json")).unwrap();
        let t = candidate(fixture["seed"].as_i64().unwrap());
        let baseline = Terrain::new(fixture["seed"].as_i64().unwrap(), Style::Vanilla).with_preview(true);
        let generator = crate::world::Generator::Normal(t);
        let crate::world::Generator::Normal(t) = &generator else { unreachable!() };
        let (mut air, mut total, mut columns) = (0, 0, 0);
        let band = fixture["band_y"].as_i64().unwrap() as i32;
        let natural = |name: &str| matches!(name, "stone" | "dirt" | "grass_block" | "gravel"
            | "snow_block" | "powder_snow" | "ice" | "packed_ice" | "calcite" | "andesite"
            | "diorite" | "granite" | "deepslate" | "tuff" | "bedrock") || name.ends_with("_ore");
        for pair in fixture["chunks"].as_array().unwrap() {
            let (cx, cz) = (pair[0].as_i64().unwrap() as i32, pair[1].as_i64().unwrap() as i32);
            let chunk = crate::world::Chunk::generated(&generator, cx, cz);
            for x in 0..16 { for z in 0..16 {
                let top = (-64..320).rev().find(|&y| crate::blocks::block_at_state(
                    chunk.block(x, y, z)).is_some_and(natural)).unwrap();
                let biome = baseline.biome_at(cx*16+x, cz*16+z);
                assert_eq!(t.biome_at(cx*16+x, cz*16+z), biome);
                assert_eq!(chunk.biome(x, 240, z), biome.number() as u16);
                if biome != Biome::JaggedPeaks { continue; }
                columns += 1;
                for y in band..band+16 {
                    if y >= top - 5 { continue; }
                    total += 1;
                    air += usize::from(crate::blocks::block_at_state(chunk.block(x, y, z))
                        .is_some_and(|name| matches!(name, "air" | "cave_air" | "void_air")));
                }
            } }
        }
        let percent = air as f64 * 100.0 / total.max(1) as f64;
        println!("A6: {columns} столбцов jagged_peaks, воздух Y{band} {air}/{total} = {percent}%");
        assert!(columns >= fixture["minimum_columns"].as_u64().unwrap() as usize);
        assert!(total >= fixture["minimum_denominator"].as_u64().unwrap() as usize);
        assert!(percent <= fixture["maximum_air_percent"].as_f64().unwrap());
    }
}
