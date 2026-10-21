use super::*;
use crate::gameplay_catalog::{Catalog, Profile, WHITE_SLAB_FIRST_STATE, WHITE_SLAB_LAST_STATE};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let name = format!("mcsa-catalog-save-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }

/// Полный список файлов fixture и SHA256; новые corrupt/level/player файлы
/// тоже изменят результат. Папка содержит только созданные этим тестом данные.
fn inventory(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, result: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() { visit(root, &path, result); }
            else {
                let hash = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, &fs::read(&path).unwrap());
                result.push((path.strip_prefix(root).unwrap().to_path_buf(), hash.as_ref().to_vec()));
            }
        }
    }
    let mut result = Vec::new();
    visit(path, path, &mut result);
    result.sort();
    result
}

#[test]
fn explicit_preview_manifest_stops_startup_before_world_writers() {
    let fixture = Fixture::new();
    let world = fixture.0.join("world");
    fs::create_dir(&world).unwrap();
    let manifest = profile::Manifest {
        schema: profile::MANIFEST_SCHEMA,
        save_format: profile::current_profile().save_format,
        registry_id: Catalog::new(Profile::Preview26_3).registry_id().into_owned(),
        gameplay_version: "26.3".into(),
        snapshot_id: "0123456789abcdef0123456789abcdef".into(),
    };
    fs::write(world.join(profile::MANIFEST_NAME), serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(world.join("fixture-player.bin"), b"fixture unchanged").unwrap();
    let before = inventory(&world);
    assert!(matches!(profile::check_current_world(&world), Err(profile::Error::UnsupportedProfile(version)) if version == "26.3"));
    // Это существующий startup gate, не прямой World::open: private preview
    // разрешён в памяти, но persistent open ещё не реализован.
    let error = match startup::prepare(&world) {
        Err(error) => error,
        Ok(_) => panic!("preview manifest не должен допускать запуск writer"),
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(inventory(&world), before);
    assert!(!world.join(REGIONS).exists());
    assert!(!world.join("playerdata").exists());
}

#[test]
fn base_world_refuses_every_known_preview_state_without_regeneration_or_write() {
    for state in WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE {
        let fixture = Fixture::new();
        let mut world = World::open(&fixture.0, Some(1), WorldKind::Flat).unwrap();
        assert_eq!(world.catalog().profile(), Profile::Java26_1_2);
        let regions = fixture.0.join(REGIONS);
        let mut chunk = Chunk::new();
        chunk.filled_section(0).fill(pack(state));
        let bytes = chunk.to_bytes_with_catalog(Catalog::new(Profile::Preview26_3)).unwrap();
        region::write(&regions, 2, 2, &bytes).unwrap();
        let before = inventory(&regions);
        assert!(!world.ensure(2, 2));
        assert!(!world.has_chunk(2, 2));
        let ready = world.source().take(2, 2);
        world.accept(2, 2, ready);
        assert!(!world.has_chunk(2, 2));
        assert!(world.take_new_chunks().is_empty());
        world.save_all();
        assert_eq!(inventory(&regions), before);
        assert!(!regions.join("corrupt").exists());
        assert_eq!(region::read(&regions, 2, 2).unwrap().unwrap(), bytes);
    }
}

#[test]
fn preview_codec_can_reread_private_disk_fixture_without_claiming_world_open_support() {
    let fixture = Fixture::new();
    let catalog = Catalog::new(Profile::Preview26_3);
    let mut chunk = Chunk::new();
    for (index, state) in (WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE).enumerate() {
        chunk.filled_section(0)[index] = pack(state);
    }
    let bytes = chunk.to_bytes_with_catalog(catalog).unwrap();
    region::write(&fixture.0, -3, 7, &bytes).unwrap();
    let before = inventory(&fixture.0);
    // Освобождаем исходный чанк; читаем только собственный регион-файл.
    drop(chunk);
    let stored = region::read(&fixture.0, -3, 7).unwrap().unwrap();
    let back = Chunk::decode_for_load_with_catalog(&stored, catalog).unwrap();
    for (index, state) in (WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE).enumerate() {
        assert_eq!(back.sections[0].as_ref().unwrap()[index], pack(state));
    }
    assert_eq!(back.to_bytes_with_catalog(catalog).unwrap(), bytes);
    assert!(matches!(Chunk::decode_for_load(&stored), Err(codec::DecodeError::UnsupportedCatalogState { .. })));
    assert_eq!(inventory(&fixture.0), before);
}
