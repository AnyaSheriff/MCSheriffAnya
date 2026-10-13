// Офлайн-перепись без чанков, сервера и внешних данных. Биомы — каждые
// 4 блока, высоты настоящей сетки плотности — каждые 32 блока.
// CENSUS_LAYOUT=reference повторяет девять окон biome_neighbours;
// square (по умолчанию) — квадрат 4096×4096 для каждого семени;
// wide — 16384×16384, чтобы захватить много материков, а не только спавн.
// CENSUS_SEEDS — семена через запятую. Вывод совместим с biome_compare.py.

use super::*;
use std::collections::BTreeMap;

const SEEDS: [i64; 4] = [100_554_032_945_340, 42, 1, 1_234_567_890];

fn deep(biome: Biome) -> bool {
    matches!(biome, Biome::DeepOcean | Biome::DeepColdOcean | Biome::DeepFrozenOcean | Biome::DeepLukewarmOcean)
}

fn quantiles(values: &mut [i32]) -> [i32; 5] {
    values.sort_unstable();
    [5, 25, 50, 75, 95].map(|p| values[(values.len() - 1) * p / 100])
}

// Пятна по четырём соседям; окна считаются отдельно, чтобы разрыв в
// выборке не соединял далёкие моря. Краевые пятна усечены окном.
fn patches(grid: &[Biome], side: usize, joined: bool, out: &mut BTreeMap<&'static str, Vec<usize>>) {
    let mut seen = vec![false; grid.len()];
    for start in 0..grid.len() {
        if seen[start] || (joined && !grid[start].is_ocean()) {
            continue;
        }
        let name = if joined { "all_oceans" } else { grid[start].name() };
        let mut stack = vec![start];
        let mut size = 0;
        seen[start] = true;
        while let Some(at) = stack.pop() {
            size += 1;
            let (x, z) = (at % side, at / side);
            let near = [
                (x > 0).then(|| at - 1), (x + 1 < side).then_some(at + 1),
                (z > 0).then(|| at - side), (z + 1 < side).then_some(at + side),
            ];
            for next in near.into_iter().flatten() {
                let same = if joined { grid[next].is_ocean() } else { grid[next] == grid[start] };
                if same && !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        out.entry(name).or_default().push(size);
    }
}

/// `cargo test --release offline_ocean_census -- --ignored --nocapture`.
#[test]
#[ignore]
fn offline_ocean_census() {
    let seeds = std::env::var("CENSUS_SEEDS").map(|s| s.split(',').map(|v| v.parse::<i64>().unwrap()).collect::<Vec<_>>()).unwrap_or_else(|_| SEEDS.to_vec());
    let layout = std::env::var("CENSUS_LAYOUT").unwrap_or_else(|_| "square".into());
    let windows: Vec<(i32, i32, i32)> = if layout == "reference" {
        (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x * 8192, z * 8192, if x == 0 && z == 0 { 1024 } else { 512 }))).collect()
    } else {
        assert!(matches!(layout.as_str(), "square" | "wide"), "неизвестная раскладка {layout}");
        vec![(0, 0, if layout == "wide" { 8192 } else { 2048 })]
    };
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut heights = BTreeMap::<&str, Vec<i32>>::new();
    let mut grouped_heights = BTreeMap::<&str, Vec<i32>>::new();
    let mut sizes = BTreeMap::new();
    let mut climate_bins = BTreeMap::<(i32, i32), usize>::new();
    let (mut total, mut oceans, mut deeps) = (0, 0, 0);
    println!("meta\tlayout\t{layout}\tstep\t4\theight_step\t32\tseeds\t{seeds:?}");
    for seed in seeds {
        let terrain = Terrain::new(seed, Style::Vanilla);
        let mut seed_counts = BTreeMap::<&str, usize>::new();
        let (mut seed_total, mut seed_ocean, mut seed_deep) = (0, 0, 0);
        for &(cx, cz, half) in &windows {
            let side = (half / 2) as usize;
            let mut grid = Vec::with_capacity(side * side);
            for iz in 0..side {
                for ix in 0..side {
                    let (x, z) = (cx - half + ix as i32 * 4 + 2, cz - half + iz as i32 * 4 + 2);
                    let climate = terrain.climate_at(x, z);
                    let biome = terrain.cell_biome(&climate, x, z);
                    grid.push(biome);
                    *counts.entry(biome.name()).or_default() += 1;
                    *seed_counts.entry(biome.name()).or_default() += 1;
                    seed_total += 1;
                    seed_ocean += usize::from(biome.is_ocean());
                    seed_deep += usize::from(deep(biome));
                    if ix % 8 == 0 && iz % 8 == 0 {
                        *climate_bins.entry(((climate.continent * 100.0).floor() as i32, (climate.temperature * 100.0).floor() as i32)).or_default() += 1;
                        let height = terrain.column_at(x, z).height;
                        heights.entry(biome.name()).or_default().push(height);
                        let group = if deep(biome) { "deep" } else if biome.is_ocean() { "shallow" } else { "land" };
                        grouped_heights.entry(group).or_default().push(height);
                    }
                }
            }
            patches(&grid, side, false, &mut sizes);
            patches(&grid, side, true, &mut sizes);
        }
        println!("seed\t{seed}\t{seed_total}\t{:.6}\t{:.6}", seed_ocean as f64 / seed_total as f64, seed_deep as f64 / seed_total as f64);
        for (name, count) in seed_counts { println!("seed_share\t{seed}\t{name}\t{:.6}", count as f64 / seed_total as f64); }
        total += seed_total;
        oceans += seed_ocean;
        deeps += seed_deep;
        let (mut centres, mut starts) = (0, 0);
        for rx in -16..16 {
            for rz in -16..16 {
                let (cx, cz) = crate::world::monument::attempt(seed, rx, rz);
                centres += usize::from(deep(terrain.column_in(cx * 16, cz * 16, None).biome));
                starts += usize::from(crate::world::monument::start_in(&terrain, rx, rz).is_some());
            }
        }
        println!("monument\t{seed}\t1024\t{centres}\t{starts}\t{:.6}", starts as f64 / 1024.0);
    }
    println!("total\t{total}");
    if std::env::var_os("CENSUS_CLIMATE").is_some() {
        for ((continent, temperature), count) in climate_bins {
            println!("climate_bin\t{continent}\t{temperature}\t{count}");
        }
    }
    for (name, count) in counts {
        println!("share\t{name}\t{:.6}", count as f64 / total as f64);
        if !name.contains("ocean") {
            println!("land_share\t{name}\t{:.6}", count as f64 / (total - oceans) as f64);
        }
    }
    println!("summary\tocean\t{:.6}\tdeep\t{:.6}\tland\t{:.6}", oceans as f64 / total as f64, deeps as f64 / total as f64, (total - oceans) as f64 / total as f64);
    for (kind, table) in [("height", &mut heights), ("height_group", &mut grouped_heights)] {
        for (name, values) in table {
            let [a, b, c, d, e] = quantiles(values);
            println!("{kind}\t{name}\t{a}\t{b}\t{c}\t{d}\t{e}\t{}", values.len());
        }
    }
    let mut all: Vec<_> = sizes.iter().filter(|(name,_)| **name != "all_oceans").flat_map(|(_,v)| v.iter().copied()).collect();
    all.sort_unstable();
    println!("patch_summary\tall_surface\t{}\t{:.3}\t{}\t{}", all.len(), total as f64/all.len() as f64, all[all.len()/2], all[(all.len()-1)*9/10]);
    for (name, values) in &mut sizes {
        values.sort_unstable();
        let small = values.iter().filter(|&&v| v <= 3).count();
        println!("patch\t{name}\t{}\t{}\t{}\t{}\t{:.3}", values.len(), values[values.len() / 2], values[(values.len() - 1) * 9 / 10], values[values.len() - 1], small as f64 / values.len() as f64);
        // Размер моря, в котором лежит случайная морская клетка: такие
        // квантили не забиваются крошечными краевыми пятнами.
        let sum: usize = values.iter().sum();
        let weighted = [50, 90].map(|p| {
            let mut acc = 0;
            *values.iter().find(|&&v| { acc += v; acc * 100 >= sum * p }).unwrap()
        });
        println!("patch_area\t{name}\t{}\t{}", weighted[0] * 16, weighted[1] * 16);
    }
}

#[test]
fn census_joins_oceans_but_not_diagonal_cells() {
    let grid = [Biome::Ocean, Biome::DeepOcean, Biome::Plains, Biome::Plains, Biome::Plains, Biome::Ocean, Biome::Plains, Biome::Plains, Biome::Ocean];
    let mut sizes = BTreeMap::new();
    patches(&grid, 3, true, &mut sizes);
    assert_eq!(sizes["all_oceans"], vec![2, 2]);
    sizes.clear();
    patches(&grid, 3, false, &mut sizes);
    assert_eq!(sizes["deep_ocean"], vec![1]);
    assert_eq!(sizes["ocean"], vec![1, 2]);
}

/// Независимые семена, сетка 64 блока на 16384²: ловит возвращение редкого
/// моря и исчезновение гор при простом сдвиге континентальности. Числа —
/// перепись vanilla-big и наша перепись до правки, а не отдельные координаты.
#[test]
fn ocean_distribution_and_land_relief() {
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut land_heights = Vec::new();
    let (mut total, mut oceans, mut deeps) = (0, 0, 0);
    for seed in [7, 314_159, 987_654_321, -20_261_010] {
        let terrain = Terrain::new(seed, Style::Vanilla);
        for iz in 0..256 {
            for ix in 0..256 {
                let (x, z) = (ix * 64 - 8192 + 2, iz * 64 - 8192 + 2);
                let biome = terrain.column_in(x, z, None).biome;
                *counts.entry(biome.name()).or_default() += 1;
                total += 1;
                oceans += usize::from(biome.is_ocean());
                deeps += usize::from(deep(biome));
                if ix % 4 == 0 && iz % 4 == 0 && !biome.is_ocean() {
                    land_heights.push(terrain.column_at(x, z).height);
                }
            }
        }
    }
    let share = |n| n as f64 / total as f64;
    println!("holdout: ocean {:.4}, deep {:.4}", share(oceans), share(deeps));
    assert!((share(oceans) - 0.4047).abs() < 0.035, "доля моря {}", share(oceans));
    assert!((share(deeps) - 0.1041).abs() < 0.025, "доля глубокого моря {}", share(deeps));
    for (name, measured) in [
        ("ocean", 0.0354), ("deep_ocean", 0.0094),
        ("cold_ocean", 0.0354), ("deep_cold_ocean", 0.0361),
        ("frozen_ocean", 0.0540), ("deep_frozen_ocean", 0.0424),
        ("lukewarm_ocean", 0.0843), ("deep_lukewarm_ocean", 0.0162),
        ("warm_ocean", 0.0915),
    ] {
        let ours = share(*counts.get(name).unwrap_or(&0));
        assert!((ours - measured).abs() < 0.025, "{name}: {ours}, оригинал {measured}");
    }
    // Доли суши по полной переписи, а не сохранение прежней ошибки:
    // например, болота занимают 0,52% мира, то есть около 0,87% суши.
    for (name, before) in [("forest", 0.1241), ("plains", 0.1247), ("swamp", 0.0087), ("meadow", 0.0130)] {
        let ours = *counts.get(name).unwrap_or(&0) as f64 / (total - oceans) as f64;
        assert!((ours - before).abs() < 0.03, "доля {name} в суше: {ours}, прежде {before}");
    }
    let heights = quantiles(&mut land_heights);
    println!("holdout land heights: {heights:?}");
    for ((ours, before), tolerance) in heights.into_iter().zip([59, 64, 69, 81, 135]).zip([3, 3, 3, 5, 8]) {
        assert!((ours - before).abs() <= tolerance, "высота суши {ours}, прежде {before}");
    }
}

// Подбор только по данным нашего шума и измеренной таблице долей.
#[test]
#[ignore]
fn calibrate_biome_distribution() {
    use super::distribution::{INITIAL, mapped};
    let path = std::env::var("BIOME_REFERENCE").expect("BIOME_REFERENCE");
    let reference = std::fs::read_to_string(path).unwrap();
    let mut target = [0.0_f64; 65];
    for line in reference.lines() {
        let parts: Vec<_> = line.split('\t').collect();
        if parts[0] == "share" && let Some(b) = Biome::ALL.iter().find(|b| b.name() == parts[1]) {
            target[b.number() as usize] = parts[2].parse().unwrap();
        }
    }
    let mut samples = Vec::new();
    for seed in SEEDS {
        let t = Terrain::new(seed, Style::Vanilla);
        for cx in [-8192, 0, 8192] { for cz in [-8192, 0, 8192] {
            let half = if cx == 0 && cz == 0 { 1024 } else { 512 };
            for x in (cx-half..cx+half).step_by(16) { for z in (cz-half..cz+half).step_by(16) {
                samples.push(t.raw_climate_at(x+2,z+2));
            }}
        }}
    }
    let score = |p: &[f64; 45]| {
        let mut counts = [0_usize; 65];
        for c in &samples { counts[biome_of(&mapped(c,p)).number() as usize] += 1; }
        let mut loss = 0.0;
        for (i, &want) in target.iter().enumerate() {
            let got = counts[i] as f64 / samples.len() as f64;
            if want >= 0.005 {
                let err = ((got + 0.0001) / (want + 0.0001)).ln();
                loss += err * err + 8.0 * (err.abs() - 0.22).max(0.0).powi(2);
            } else { loss += 0.03 * ((got - want) / 0.005).powi(2); }
        }
        (loss, counts)
    };
    let mut p = std::env::var("BIOME_PARAMS").ok().map(|s| serde_json::from_str::<Vec<f64>>(&s).unwrap().try_into().unwrap()).unwrap_or(INITIAL);
    let mut best = score(&p).0;
    for round in 0..45 {
        let step = (0.035 * 0.91_f64.powi(round)).max(0.002);
        for i in 0..45 {
            if i == 32 || i == 33 { continue; }
            let old = p[i];
            let mut chosen = old;
            for sign in [-1.0, 1.0] {
                p[i] = old + step * sign;
                let groups = [0..4,4..8,8..12,12..16,16..20,20..24,24..28,28..32,32..37,37..43];
                let valid = groups.iter().all(|g| p[g.clone()].windows(2).all(|v| v[1]-v[0] > 0.012)) && p[i].abs() < 1.0 && p[44] > 0.1;
                if valid {
                    let s = score(&p).0;
                    if s < best { best = s; chosen = p[i]; }
                }
            }
            p[i] = chosen;
        }
        println!("round {round}: {best} {p:?}");
    }
    let (_, counts) = score(&p);
    for b in Biome::ALL { let i=b.number() as usize; println!("fit\t{}\t{}\t{}", b.name(), target[i], counts[i] as f64/samples.len() as f64); }
    std::fs::write(std::env::var("BIOME_PARAMS_OUT").unwrap(),format!("{p:?}")).unwrap();
}
