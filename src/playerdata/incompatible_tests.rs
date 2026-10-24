use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let name=format!("mcsa-incompatible-player-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let path=std::env::temp_dir().join(name);fs::create_dir(&path).unwrap();Self(path)
    }
}
impl Drop for Fixture {fn drop(&mut self){fs::remove_dir_all(&self.0).unwrap();}}
fn empty_player()->PlayerData {
    PlayerData {abilities: Abilities::for_mode(0),shoulders:[None,None],inventory:Inventory::new(),x:1.5,y:65.,z:-3.5,yaw:90.,pitch:0.,game_mode:0,
        dimension:Default::default(),vitals:Default::default(),respawn:None,ender:empty_ender()}
}

#[test]
fn future_player_header_returns_typed_error_and_preserves_the_file() {
    let fixture=Fixture::new();let uuid=[39;16];let path=path_for(&fixture.0,&uuid);
    for version in [0,FORMAT_VERSION+1,u32::MAX] {
        let mut bytes=encode(&empty_player()).unwrap();bytes[4..8].copy_from_slice(&version.to_be_bytes());
        for bytes in [bytes.clone(),bytes[..8].to_vec()] {
            fs::write(&path,&bytes).unwrap();
            assert_eq!(load_checked(&fixture.0,&uuid),Err(LoadError::UnsupportedLayout(version)));
            assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(fs::read_dir(&fixture.0).unwrap().count(),1);
        }
    }
}

#[test]
fn missing_current_and_known_damaged_player_keep_their_previous_meanings() {
    let fixture=Fixture::new();let uuid=[40;16];let path=path_for(&fixture.0,&uuid);
    assert_eq!(load_checked(&fixture.0,&uuid),Ok(None));assert!(!path.exists());
    let mut expected=empty_player();let stone=crate::blocks::item_named("stone").unwrap();
    expected.inventory.set(36,Some(crate::inventory::Stack::new(stone,23)));
    expected.inventory.set_cursor(Some(crate::inventory::Stack::new(stone,7)));
    expected.ender[0]=Some(crate::inventory::Stack::new(stone,11));
    fs::write(&path,encode(&expected).unwrap()).unwrap();assert_eq!(load_checked(&fixture.0,&uuid),Ok(Some(expected)));
    let mut truncated=MAGIC.to_vec();truncated.extend(FORMAT_VERSION.to_be_bytes());
    fs::write(&path,&truncated).unwrap();assert_eq!(load_checked(&fixture.0,&uuid),Ok(None));
    assert_eq!(fs::read(path).unwrap(),truncated);
}
