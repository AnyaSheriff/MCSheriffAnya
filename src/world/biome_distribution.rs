// Распределение входов таблицы биомов; шумы рельефа и пещер не меняются.
// Вики, World generation: «six-dimensional (6D) space».
// Калибровка по сохранённым квадратам vanilla-big, не по коду игры.
use super::{Climate, spline};

#[cfg(test)]
pub const INITIAL: [f64; 45] = [
    -0.45, -0.15, 0.2, 0.55,
    -0.45, -0.15, 0.2, 0.55,
    -0.45, -0.15, 0.2, 0.55,
    -0.35, -0.1, 0.1, 0.3,
    -0.35, -0.1, 0.1, 0.3,
    -0.35, -0.1, 0.1, 0.3,
    -0.35, -0.1, 0.1, 0.3,
    -0.35, -0.1, 0.1, 0.3,
    -0.455, -0.19, -0.11, 0.03, 0.3,
    -0.78, -0.375, -0.2225, 0.05, 0.45, 0.55,
    0.0, 1.0,
];

fn remap(value: f64, from: &[f64], to: &[f64]) -> f64 {
    let mut points = [(-1.2, -1.2); 8];
    for (i, (&x, &y)) in from.iter().zip(to).enumerate() { points[i + 1] = (x, y); }
    points[from.len() + 1] = (1.2, 1.2);
    spline(&points[..from.len() + 2], value)
}

pub fn mapped(c: &Climate, p: &[f64; 45]) -> Climate {
    let continent = remap(c.continent, &p[32..37], &[-0.455, -0.19, -0.11, 0.03, 0.3]);
    let band = if continent < -0.455 { 8 } else if continent < -0.19 { 4 } else { 0 };
    let temperature = remap(c.temperature, &p[band..band+4], &[-0.45, -0.15, 0.2, 0.55]);
    let t = super::level(temperature, &[-0.45, -0.15, 0.2, 0.55]) as usize;
    Climate {
        continent, temperature,
        humidity: remap(c.humidity, &p[12+t*4..16+t*4], &[-0.35, -0.1, 0.1, 0.3]),
        erosion: remap(c.erosion, &p[37..43], &[-0.78, -0.375, -0.2225, 0.05, 0.45, 0.55]),
        weirdness: ((c.weirdness + p[43]) * p[44]).clamp(-1.0, 1.0),
    }
}

// Пороги исходного климата: T суши/мелкого/глубокого моря (3×4),
// H для пяти поясов T (5×4), C (5), E (6), сдвиг и масштаб W (2).
// Это НЕ параметры оригинала. Монотонные переносы подогнаны по площадям
// девяти окон на четырёх семенах; таблица biome_of и сами шумы прежние.
// Границы моря/глубокого моря оставлены прежними, чтобы береговая линия
// не зависела от семян калибровки. Ширина русла и высоты — прежние;
// попадание расширенного биома реки на сухой берег [проверить].
// Полная перепись: tools/measure/biomes_{vanilla_big,ours_after}.tsv.
pub const CALIBRATED: [f64; 45] = [
    -0.667443, -0.420703, -0.039099, 0.111172,
    -0.508540, -0.300118, -0.004384, 0.606237,
    -0.485397, -0.038018, 0.128706, 0.333101,
    -0.311252, -0.197735, 0.653554, 0.713548,
    0.011172, 0.023383, 0.324175, 0.646139,
    -0.155126, 0.081518, 0.436610, 0.758993,
    -0.350000, 0.163969, 0.181976, 0.418812,
    -0.350000, 0.013440, 0.265302, 0.278042,
    -0.455000, -0.190000, 0.030919, 0.334743, 0.380213,
    -0.980000, -0.938077, -0.706846, -0.023760, 0.600727, 0.999526,
    -0.026647, 0.713323,
];

// 26.3 использует исходный климат именованных октав (climate_noise.rs).
// Прежние переносы порогов остаются только у профиля 26.1.2.

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Biome, Terrain, Style};

    #[test]
    fn calibration_is_monotone_and_finite() {
        for range in [0..4,4..8,8..12,12..16,16..20,20..24,24..28,28..32,32..37,37..43] {
            assert!(CALIBRATED[range].windows(2).all(|p| p[0]<p[1]));
        }
        for i in -120..=120 {
            let n=f64::from(i)/100.0;
            let c=Climate{continent:n,temperature:n,humidity:n,erosion:n,weirdness:n};
            let out=mapped(&c,&CALIBRATED);
            assert!([out.continent,out.temperature,out.humidity,out.erosion,out.weirdness].iter().all(|v|v.is_finite()));
        }
    }

    /// Два тёплых квадрата не должны удалять холодные моря за их пределами.
    #[test]
    fn preview_keeps_all_ocean_temperature_bands_on_holdout_seeds() {
        let mut counts = [0usize; 5];
        for seed in [7,314_159,987_654_321,-20_261_010] {
            let terrain = Terrain::new(seed,Style::Vanilla).with_preview(true);
            for z in (-8192..8192).step_by(64) { for x in (-8192..8192).step_by(64) {
                let band = match terrain.biome_at(x+2,z+2) {
                    Biome::FrozenOcean | Biome::DeepFrozenOcean => Some(0),
                    Biome::ColdOcean | Biome::DeepColdOcean => Some(1),
                    Biome::Ocean | Biome::DeepOcean => Some(2),
                    Biome::LukewarmOcean | Biome::DeepLukewarmOcean => Some(3),
                    Biome::WarmOcean => Some(4),
                    _ => None,
                };
                if let Some(band) = band { counts[band] += 1; }
            } }
        }
        assert!(counts.iter().all(|&count|count>100),"исчез температурный пояс: {counts:?}");
    }

    #[test]
    fn reference_common_shares_on_four_seeds() {
        let mut counts=[0usize;65]; let mut total=0;
        for seed in [100_554_032_945_340,42,1,1_234_567_890] {
            let terrain=Terrain::new(seed,Style::Vanilla);
            for cx in [-8192,0,8192] { for cz in [-8192,0,8192] {
                let half=if cx==0 && cz==0 {1024} else {512};
                for x in (cx-half..cx+half).step_by(16) { for z in (cz-half..cz+half).step_by(16) {
                    counts[terrain.biome_at(x+2,z+2).number() as usize]+=1;
                    total+=1;
                }}
            }}
        }
        for row in include_str!("../../tools/measure/biomes_vanilla_big.tsv").lines() {
            let parts:Vec<_>=row.split('\t').collect();
            if parts[0]!="share" {continue;}
            let share: f64=parts[2].parse().unwrap();
            if share<0.005 {continue;}
            let biome=Biome::ALL.iter().find(|b|b.name()==parts[1]).unwrap();
            let actual=counts[biome.number() as usize] as f64/total as f64;
            assert!((actual/share-1.0).abs()<=0.3,"{}: reference {share}, measured {actual}",parts[1]);
        }
    }

    #[test]
    fn biome_mapping_preserves_geometry_climate_and_smooth_worlds() {
        for style in [Style::Vanilla,Style::Smooth] {
            let terrain=Terrain::new(42,style);
            for (x,z) in [(-400,612),(0,0),(8192,-8192),(513,123)] {
                let c=terrain.climate_at(x,z);
                let height=terrain.smooth_height(&c,x,z);
                assert_eq!(terrain.column_in(x,z,None).height,height);
                let before=[c.continent,c.erosion,c.weirdness,c.temperature,c.humidity];
                let raw=terrain.raw_climate_at(x,z);
                assert_eq!(before,[raw.continent,raw.erosion,raw.weirdness,raw.temperature,raw.humidity]);
                let _=terrain.biome_at(x,z);
                let after=terrain.climate_at(x,z);
                assert_eq!(before,[after.continent,after.erosion,after.weirdness,after.temperature,after.humidity]);
                if style==Style::Smooth {
                    let input=terrain.biome_input(&c,x,z);
                    assert_eq!(before,[input.continent,input.erosion,input.weirdness,input.temperature,input.humidity]);
                    let raw=terrain.raw_climate_at(x,z);
                    assert_eq!(before,[raw.continent,raw.erosion,raw.weirdness,raw.temperature,raw.humidity]);
                }
                let bx=x&!3; let bz=z&!3;
                assert_eq!(terrain.biome_at(bx,bz),terrain.biome_at(bx+3,bz+3));
            }
        }
    }
}
