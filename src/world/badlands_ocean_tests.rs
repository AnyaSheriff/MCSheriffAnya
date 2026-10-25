// Профиль срезов берётся из testdata; данные генерации пишутся только по явному пути.
use super::*;

#[test]
#[ignore]
fn export_badlands_263_chunks() {
    let plan: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/badlands-ocean-26.3-plan.json")).unwrap();
    let seed = plan["seed"].as_i64().unwrap();
    let generator = Generator::Normal(Terrain::new(seed, Style::Vanilla).with_preview(true));
    let out = std::env::var("BADLANDS_OUTPUT").expect("BADLANDS_OUTPUT");
    for window in plan["ours_badlands_windows"].as_array().unwrap() {
        let v: Vec<i32> = window.as_array().unwrap().iter().map(|n| n.as_i64().unwrap() as i32).collect();
        let [x, z, width, depth] = v[..] else { panic!("окно x,z,width,depth") };
        for cx in x..x + width { for cz in z..z + depth {
            let chunk = Chunk::generated(&generator, cx, cz);
            region::write(std::path::Path::new(&out), cx, cz, &chunk.to_bytes()).unwrap();
        } }
    }
}

/// Проверяется готовый чанк, а не только прибавка к карте высот: столбы
/// должны попасть в секции и состоять из полос терракоты до основания.
#[test]
fn eroded_badlands_spires_reach_the_chunk_as_solid_terracotta() {
    let generator = Generator::Normal(Terrain::new(1_792_932_773_345_722_119, Style::Vanilla).with_preview(true));
    let Generator::Normal(terrain) = &generator else { unreachable!() };
    // После смены масштаба материков берём новое пятно того же семени.
    let places = [(47, 219), (48, 219)];
    let chunks: Vec<_> = places.iter().map(|&(cx, cz)| Chunk::generated(&generator, cx, cz)).collect();
    let wide = terrain.relief_grid(751, 3503, 784, 3520);
    let mut columns = 0;
    let mut tallest = 0;
    for (i, &(cx, cz)) in places.iter().enumerate() {
        let small = terrain.relief_grid(cx * 16 - 1, cz * 16 - 1, cx * 16 + 16, cz * 16 + 16);
        for x in 0..16 { for z in 0..16 {
            let (wx, wz) = (cx * 16 + x, cz * 16 + z);
            let column = terrain.column_in(wx, wz, Some(&small));
            let other = terrain.column_in(wx, wz, Some(&wide));
            assert_eq!((column.height, column.hoodoo_base), (other.height, other.hoodoo_base));
            let Some(base) = column.hoodoo_base else { continue };
            columns += 1;
            assert_ne!(chunks[i].block(x, base, z), AIR, "опора ({wx},{base},{wz})");
            tallest = tallest.max(column.height - base);
            for y in base + 1..=column.height {
                let name = crate::blocks::block_at_state(chunks[i].block(x, y, z)).unwrap_or("?");
                assert!(name.ends_with("terracotta"), "({wx},{y},{wz}): {name}");
            }
        } }
    }
    assert!(columns >= 20, "столбцов со столбами: {columns}");
    assert!(tallest >= 28, "максимальная добавка: {tallest}");
    // Генерация соседей в обратном порядке не меняет сохранённые секции.
    for (i, &(cx, cz)) in places.iter().enumerate().rev() {
        assert_eq!(chunks[i].sections, Chunk::generated(&generator, cx, cz).sections);
    }
    let west = &chunks[0]; let east = &chunks[1];
    let mut colors = std::collections::BTreeSet::new();
    for y in 80..100 {
        let a = crate::blocks::block_at_state(west.block(15, y, 8)).unwrap();
        let b = crate::blocks::block_at_state(east.block(0, y, 8)).unwrap();
        assert_eq!(a, b, "полоса через границу чанка на {y}");
        colors.insert(a);
    }
    assert!(colors.len() >= 3, "полосы: {colors:?}");
}
