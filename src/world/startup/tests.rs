use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("mcsa-startup-{}-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn world(&self) -> PathBuf { self.0.join("world") }
}
impl Drop for Fixture {
    fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
}

fn current_manifest() -> profile::Manifest {
    let current = profile::current_profile();
    profile::Manifest {
        schema: profile::MANIFEST_SCHEMA,
        save_format: current.save_format,
        registry_id: current.registry_id,
        gameplay_version: current.gameplay_version,
        snapshot_id: "0123456789abcdef0123456789abcdef".into(),
    }
}

#[test]
fn missing_and_unidentified_worlds_are_not_created_or_marked() {
    let fixture = Fixture::new();
    let world = fixture.world();
    {
        let guard = prepare(&world).unwrap();
        assert_eq!(guard.check, StartupCheck::LegacyUnidentified);
        assert_eq!(guard.has_exclusive_lock(), cfg!(unix));
        assert!(!world.exists());
    }
    fs::create_dir(&world).unwrap();
    fs::write(world.join("old-world-marker"), b"unchanged").unwrap();
    let guard = prepare(&world).unwrap();
    assert_eq!(guard.check, StartupCheck::LegacyUnidentified);
    assert_eq!(fs::read_dir(&world).unwrap().count(), 1);
    assert_eq!(fs::read(world.join("old-world-marker")).unwrap(), b"unchanged");
}

#[test]
fn bad_manifests_refuse_without_migration_and_release_the_guard() {
    let fixture = Fixture::new();
    let world = fixture.world();
    fs::create_dir(&world).unwrap();
    fs::write(world.join("blocks.mcrw"), b"legacy-single-file").unwrap();
    fs::create_dir(world.join("regions")).unwrap();
    fs::write(world.join("regions/r.0.0.rcw"), b"legacy-region").unwrap();
    let path = world.join(profile::MANIFEST_NAME);
    let current = current_manifest();
    let mut wrong_registry = current.clone();
    wrong_registry.registry_id = "other-registry".into();
    let mut wrong_profile = current.clone();
    wrong_profile.gameplay_version = "unsupported-profile".into();
    let mut future = current.clone();
    future.schema += 1;
    let mut old_layout = current.clone();
    old_layout.save_format.playerdata_layout -= 1;
    for bytes in [
        serde_json::to_vec(&wrong_registry).unwrap(),
        serde_json::to_vec(&wrong_profile).unwrap(),
        serde_json::to_vec(&future).unwrap(),
        serde_json::to_vec(&old_layout).unwrap(),
        b"{".to_vec(),
    ] {
        fs::write(&path, &bytes).unwrap();
        let error = prepare(&world).err().expect("неподходящий мир не открывается");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(world.join("blocks.mcrw")).unwrap(), b"legacy-single-file");
        assert_eq!(fs::read(world.join("regions/r.0.0.rcw")).unwrap(), b"legacy-region");
        assert_eq!(fs::read_dir(&world).unwrap().count(), 3);
        assert_eq!(fs::read_dir(world.join("regions")).unwrap().count(), 1);
        // Ошибка проверки должна отпустить уже взятый внешний замок.
        fs::write(&path, serde_json::to_vec(&current).unwrap()).unwrap();
        let guard = prepare(&world).unwrap();
        assert_eq!(guard.check, StartupCheck::ManifestMatches { snapshot_id: current.snapshot_id.clone() });
    }
}

#[test]
fn a_file_cannot_be_used_as_the_world_directory() {
    let fixture = Fixture::new();
    fs::write(fixture.world(), b"preserve").unwrap();
    assert!(prepare(&fixture.world()).is_err());
    assert_eq!(fs::read(fixture.world()).unwrap(), b"preserve");
}

#[cfg(unix)]
#[test]
fn links_and_special_lock_files_fail_without_touching_their_targets() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let world = fixture.world();
    let target = fixture.0.join("target");
    fs::create_dir(&target).unwrap();
    symlink(&target, &world).unwrap();
    assert!(prepare(&world).is_err());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    fs::remove_file(&world).unwrap();
    let lock = fixture.0.join(".mcsa-world.lock");
    let target_file = target.join("preserve");
    fs::write(&target_file, b"preserve").unwrap();
    symlink(&target_file, &lock).unwrap();
    assert!(prepare(&world).is_err());
    assert_eq!(fs::read(&target_file).unwrap(), b"preserve");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(prepare(&world).is_err());
    fs::remove_dir(&lock).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(lock.as_os_str().as_bytes()).unwrap();
    // SAFETY: name — живой путь с завершающим NUL к собственному fixture.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(prepare(&world).is_err());
    assert!(!world.exists());
}

#[cfg(unix)]
fn child(root: &Path, expected: &str) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "world::startup::tests::child_probe", "--nocapture"])
        .env("MCSA_STARTUP_TEST_ROOT", root)
        .env("MCSA_STARTUP_TEST_EXPECTED", expected)
        .output().unwrap();
    assert!(output.status.success(), "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

#[cfg(unix)]
#[test]
fn child_probe() {
    let Some(root) = std::env::var_os("MCSA_STARTUP_TEST_ROOT") else { return; };
    let expected = std::env::var("MCSA_STARTUP_TEST_EXPECTED").unwrap();
    let result = prepare(&PathBuf::from(root).join("world"));
    match expected.as_str() {
        "busy" => assert_eq!(result.err().expect("замок должен быть занят").kind(), io::ErrorKind::WouldBlock),
        "free" => assert!(result.is_ok()),
        _ => panic!("неизвестное ожидание дочерней проверки"),
    }
}

#[cfg(unix)]
#[test]
fn independent_process_is_excluded_until_guard_drops() {
    use std::os::unix::fs::MetadataExt;
    let fixture = Fixture::new();
    let guard = prepare(&fixture.world()).unwrap();
    let lock = fixture.0.join(".mcsa-world.lock");
    let inode = fs::metadata(&lock).unwrap().ino();
    child(&fixture.0, "busy");
    assert!(!fixture.world().exists());
    drop(guard);
    assert_eq!(fs::metadata(&lock).unwrap().ino(), inode);
    child(&fixture.0, "free");
    assert_eq!(fs::metadata(&lock).unwrap().ino(), inode);
    assert!(!fixture.world().exists());
}
