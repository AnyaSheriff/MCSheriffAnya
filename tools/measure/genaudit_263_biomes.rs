// Отбор подземных биомов по текущему генератору без полной генерации чанков.
// После сборки seedmap-core этот файл компилируется rustc с --extern для
// seedmap_core и serde_json из одного release/deps. PLAN задаёт окна чанков.
// Все данные оригинала берутся отдельно из сохранённой переписи.
use seedmap_core::world::{cave_biomes, terrain::{Terrain, Style}};
use std::{collections::{BTreeMap, BTreeSet}, fs};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "genaudit_263_biomes PLAN OUTPUT");
    let plan: serde_json::Value = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let seed = plan["seed"].as_i64().unwrap();
    let terrain = Terrain::new(seed, Style::Vanilla).with_preview(true);
    let mut biomes = BTreeMap::<String, usize>::new();
    let mut sulfur = Vec::new();
    let mut seen = BTreeSet::new();
    for window in plan["windows"]["overworld"].as_array().unwrap() {
        let b: Vec<_> = window.as_array().unwrap().iter().map(|n| n.as_i64().unwrap() as i32).collect();
        for cx in b[0]..=b[2] { for cz in b[1]..=b[3] {
            assert!(seen.insert((cx, cz)), "повтор чанка в плане");
            let (x0, z0) = (cx * 16, cz * 16);
            // Та же кайма сетки, что у Chunk::underground_generated.
            let grid = terrain.relief_grid(x0 - 1, z0 - 1, x0 + 16, z0 + 16);
            let mut sulfur_cells = 0;
            for dx in (0..16).step_by(4) { for dz in (0..16).step_by(4) {
                let (x, z) = (x0 + dx + 2, z0 + dz + 2);
                let column = terrain.column_in(x, z, Some(&grid));
                assert!(column.climate_263, "селектор требует именованных октав 26.3");
                for y in (-62..320).step_by(4) {
                    let name = if terrain.sulfur_cell(&column, x, y, z) {
                        sulfur_cells += 1;
                        "sulfur_caves"
                    } else {
                        cave_biomes::biome_at(&column, y).map_or("surface", |b| b.name())
                    };
                    *biomes.entry(name.into()).or_default() += 1;
                }
            } }
            if sulfur_cells > 0 { sulfur.push(serde_json::json!({"chunk": [cx, cz], "cells": sulfur_cells})); }
        } }
        eprintln!("биомы {seed}: {b:?}");
    }
    let result = serde_json::json!({"seed": seed, "chunks": seen.len(), "biomes_3d": biomes, "sulfur_chunks": sulfur});
    fs::write(&args[2], serde_json::to_vec_pretty(&result).unwrap()).unwrap();
}
