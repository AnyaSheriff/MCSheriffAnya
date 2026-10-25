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
fn only_missing_player_is_new_and_damaged_player_is_refused() {
    let fixture=Fixture::new();let uuid=[40;16];let path=path_for(&fixture.0,&uuid);
    assert_eq!(load_checked(&fixture.0,&uuid),Ok(None));assert!(!path.exists());
    let mut expected=empty_player();let stone=crate::blocks::item_named("stone").unwrap();
    expected.inventory.set(36,Some(crate::inventory::Stack::new(stone,23)));
    expected.inventory.set_cursor(Some(crate::inventory::Stack::new(stone,7)));
    expected.ender[0]=Some(crate::inventory::Stack::new(stone,11));
    fs::write(&path,encode(&expected).unwrap()).unwrap();assert_eq!(load_checked(&fixture.0,&uuid),Ok(Some(expected)));
    let mut truncated=MAGIC.to_vec();truncated.extend(FORMAT_VERSION.to_be_bytes());
    fs::write(&path,&truncated).unwrap();assert_eq!(load_checked(&fixture.0,&uuid),Err(LoadError::InvalidData));
    assert_eq!(fs::read(path).unwrap(),truncated);
}

// Фиксированные bytes выпущенных версий, независимо от encode этой ветки.
pub(super) const RELEASED_V8: &[u8] = &[
    0x4d, 0x43, 0x50, 0x44, 0x00, 0x00, 0x00, 0x08, 0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x40, 0x50, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x42, 0xb4, 0x00, 0x00, 0xc1, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x02, 0x24, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x25, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x05,
    0x00, 0x41, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x13, 0x40, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x01, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 0x00, 0x01, 0xe2, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x07, 0x00, 0x00, 0x00, 0x0a, 0x5b, 0x6e, 0x75, 0x6c, 0x6c, 0x2c, 0x6e, 0x75, 0x6c, 0x6c,
    0x5d, 0x00, 0x00, 0x00, 0x0b,
];
pub(super) const RELEASED_V9: &[u8] = &[
    0x4d, 0x43, 0x50, 0x44, 0x00, 0x00, 0x00, 0x09, 0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x40, 0x50, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x42, 0xb4, 0x00, 0x00, 0xc1, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x02, 0x24, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x25, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x05,
    0x00, 0x41, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x13, 0x40, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x01, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 0x00, 0x01, 0xe2, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x07, 0x00, 0x00, 0x00, 0x0a, 0x5b, 0x6e, 0x75, 0x6c, 0x6c, 0x2c, 0x6e, 0x75, 0x6c, 0x6c,
    0x5d, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x2a,
];

// Происхождение: git show <tag>:src/playerdata.rs, encode/Inventory/bed.
// v0.2.0-alpha (2d2d054313f3dc623e6ff0de71d53990b90953a9): layout8; source SHA256 a92ef6fed345705fb0f772c39d203ad8079605606b3677e19f705dfe2ff3461d.
// v0.2.1-alpha (f45016fea155e8cbc1a602d2da0c03b900c4a22e): layout8; source SHA256 a92ef6fed345705fb0f772c39d203ad8079605606b3677e19f705dfe2ff3461d.
// v0.2.2-alpha (f1f64d1af109877715e1d9a186d8e82c3325911f): layout8; source SHA256 a92ef6fed345705fb0f772c39d203ad8079605606b3677e19f705dfe2ff3461d.
// v0.2.3-alpha (238e3c939c5087bcffa1a6907c48bd1fa88b0c41): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// v0.2.4-alpha (08a6633d6b2304a261d3c0195203656fb1145fcd): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// v0.2.5-alpha (6b46c85125c0dabdc0bccdd25759cffc4dea6147): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// v0.2.6-alpha (dc3fef7a8f30e439ed9c862dd58f08967754ccc4): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// v0.2.7-alpha (199902b2e4890ceaac55f2026f5db10b26b684d0): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// v0.2.8-alpha (e2b3cb9150e6dd0359739703a5ce28e04fdd2896): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// v0.2.9-alpha (462d309ece55674a0111257457f4e62cba64951f): layout9; source SHA256 26f88e69d06d0236e6ac534de3fbd16fd7ef79d64a39f0ce63de6c4769e17da1.
// RELEASED_V8 SHA256 20a82cff4ef309956f971bf80333e9399c9ceadca15761a4fbdc761c0496343f.
// RELEASED_V9 SHA256 8f731ca8c0e2ee762f2d92e7273cfe5dd83ade9693df7d6114587eacedcf6273.

#[test]
fn invalid_saved_coordinates_refuse_without_recreating_or_overwriting() {
    let fixture = Fixture::new();
    let uuid = [41; 16];
    let path = path_for(&fixture.0, &uuid);
    for (at, invalid) in [(8, f64::NAN.to_be_bytes().to_vec()), (16, f64::INFINITY.to_be_bytes().to_vec()),
        (32, f32::NAN.to_be_bytes().to_vec()), (40, (-1i32).to_be_bytes().to_vec())] {
        let mut bytes = encode(&empty_player()).unwrap();
        bytes[at..at + invalid.len()].copy_from_slice(&invalid);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(load_checked(&fixture.0, &uuid), Err(LoadError::InvalidData));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}
