// Регрессии по координатам мира владельца; снимки генерируются только в tmp.
use super::*;

const SEED: i64 = 1_792_932_773_345_722_119;

#[test]
#[ignore]
fn export_terrain_263_regression() {
    let out = std::env::var("TERRAIN_263_OUTPUT").expect("TERRAIN_263_OUTPUT");
    let plan: serde_json::Value = serde_json::from_str(include_str!("../../testdata/world-profile/terrain-26.3-plan.json")).unwrap();
    let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla).with_preview(true));
    for pair in plan["chunks"].as_array().unwrap() {
        let (cx, cz) = (pair[0].as_i64().unwrap() as i32, pair[1].as_i64().unwrap() as i32);
        let chunk = Chunk::generated(&generator, cx, cz);
        region::write(std::path::Path::new(&out), cx, cz, &chunk.to_bytes()).unwrap();
    }
}

#[test]
fn random_section_mask_does_not_survive_chunk_replacement() {
    let mut world = World::in_memory();
    world.ensure(0, 0);
    let revision = world.chunk_revision(0, 0).unwrap();
    world.random_sections.insert((0, 0), (revision, 0));
    let mut chunk = Chunk::new();
    chunk.put_generated(0, 64, 0, crate::blocks::state_by_name("grass_block").unwrap(), true);
    chunk.put_generated(0, 65, 0, crate::blocks::state_by_name("stone").unwrap(), true);
    let chunk = Chunk::from_bytes(&chunk.to_bytes()).expect("загрузка сохранённого чанка");
    world.insert_chunk(0, 0, chunk, true, &[]);
    assert!(!world.random_sections.contains_key(&(0, 0)));
    let start = world.watch_events(0);
    for _ in 0..16 { crate::farming::random_ticks(&mut world, &[(0, 0)], 0, 4096); }
    assert_eq!(crate::blocks::block_at_state(world.get_block(0, 64, 0)), Some("dirt"));
    let (events, _) = world.events_since(start, 0);
    assert!(events.iter().any(|event| matches!(event, WorldEvent::Change(change)
        if (change.x, change.y, change.z) == (0, 64, 0)
            && crate::blocks::block_at_state(change.state) == Some("dirt"))));
}

#[test]
fn reported_cliff_is_continuous_and_independent_of_chunk_order() {
    let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla).with_preview(true));
    let Generator::Normal(terrain) = &generator else { unreachable!() };
    let grid = terrain.relief_grid(1088, -512, 1151, -449);
    let jump = (grid.top(1117, -477) - grid.top(1117, -476)).abs();
    assert!(jump <= 7, "прежний перепад 18, сейчас {jump}");
    let places = [(69, -30), (70, -30), (69, -31), (70, -31)];
    let first: Vec<_> = places.iter().map(|&(cx, cz)| Chunk::generated(&generator, cx, cz)).collect();
    // Другой профиль с тем же семенем не должен отравить кэши высот/чанков.
    let old = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
    for &(cx, cz) in &places { let _ = Chunk::generated(&old, cx, cz); }
    for (i, &(cx, cz)) in places.iter().enumerate().rev() {
        let again = Chunk::generated(&generator, cx, cz);
        assert_eq!(first[i].sections, again.sections, "порядок чанков ({cx},{cz})");
        for x in 0..16 { for z in 0..16 {
            assert_eq!(terrain.column_at(cx * 16 + x, cz * 16 + z).height,
                grid.top(cx * 16 + x, cz * 16 + z));
        } }
    }
}

#[test]
fn village_floors_do_not_bury_grass() {
    let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla).with_preview(true));
    let mut floors = 0;
    for (cx, cz) in [(73, -26), (74, -25), (75, -24), (75, -21)] {
        let chunk = Chunk::generated(&generator, cx, cz);
        for x in 0..16 { for z in 0..16 { for y in 40..160 {
            if matches!(crate::blocks::block_at_state(chunk.block(x, y + 1, z)), Some("dirt_path" | "farmland" | "dirt" | "stone")) {
                floors += 1;
                assert_ne!(crate::blocks::block_at_state(chunk.block(x, y, z)), Some("grass_block"), "под полом ({},{y},{})",cx*16+x,cz*16+z);
            }
        } } }
    }
    assert!(floors > 20, "пустая выборка пола {floors}");
}
