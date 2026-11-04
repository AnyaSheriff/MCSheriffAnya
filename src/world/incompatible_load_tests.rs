use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let name=format!("mcsa-incompatible-chunk-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let path=std::env::temp_dir().join(name);fs::create_dir(&path).unwrap();Self(path)
    }
}
impl Drop for Fixture {fn drop(&mut self){fs::remove_dir_all(&self.0).unwrap();}}
fn region_files(path:&Path)->Vec<(PathBuf,Vec<u8>)> {
    let mut files:Vec<_>=fs::read_dir(path).unwrap().map(|entry|entry.unwrap().path()).collect();files.sort();
    files.into_iter().map(|file|{assert!(file.is_file());let bytes=fs::read(&file).unwrap();(file,bytes)}).collect()
}

#[test]
fn incompatible_chunks_are_never_regenerated_or_written_as_corrupt() {
    for unknown_state in [false,true] {
        let fixture=Fixture::new();
        let mut world=World::open(&fixture.0,Some(1),WorldKind::Flat).unwrap();
        let regions=fixture.0.join(REGIONS);let mut bytes=Chunk::new().to_bytes();
        if unknown_state {bytes.splice(1..2,[1,255,255]);} else {bytes[0]=CHUNK_VERSION+1;}
        region::write(&regions,2,2,&bytes).unwrap();let before=region_files(&regions);
        assert!(!world.ensure(2,2));assert!(!world.has_chunk(2,2));
        let ready=world.source().take(2,2);world.accept(2,2,ready);
        assert!(!world.has_chunk(2,2));assert!(world.take_new_chunks().is_empty());
        world.save_all();
        assert_eq!(region_files(&regions),before);assert!(!regions.join("corrupt").exists());
    }
}

#[test]
fn known_corrupt_chunk_keeps_existing_recovery_policy() {
    let fixture=Fixture::new();let mut world=World::open(&fixture.0,Some(1),WorldKind::Flat).unwrap();
    let regions=fixture.0.join(REGIONS);let bytes=vec![CHUNK_VERSION,1,255];
    region::write(&regions,2,2,&bytes).unwrap();assert!(world.ensure(2,2));
    let copies:Vec<_>=fs::read_dir(regions.join("corrupt")).unwrap().collect();
    assert_eq!(copies.len(),1);assert_eq!(fs::read(copies[0].as_ref().unwrap().path()).unwrap(),bytes);
}
