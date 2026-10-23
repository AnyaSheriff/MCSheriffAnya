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
fn missing_and_empty_worlds_are_not_created_or_marked() {
    let fixture = Fixture::new();
    let world = fixture.world();
    {
        let guard = prepare(&world).unwrap();
        assert_eq!(guard.check, StartupCheck::LegacyUnidentified);
        assert_eq!(guard.has_exclusive_lock(), cfg!(unix));
        assert!(!world.exists());
    }
    fs::create_dir(&world).unwrap();
    let guard = prepare(&world).unwrap();
    assert_eq!(guard.check, StartupCheck::LegacyUnidentified);
    assert_eq!(fs::read_dir(&world).unwrap().count(), 0);
    assert!(!fixture.0.join("backups").exists());
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
    old_layout.save_format.playerdata_layout = 9;
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

#[cfg(target_os = "linux")]
fn old_storage_world(fixture: &Fixture) -> (PathBuf, Vec<u8>) {
    let world = fixture.world();
    fs::create_dir(&world).unwrap();
    fs::write(world.join("session.lock"), b"fixture-lock-pid").unwrap();
    fs::create_dir(world.join("playerdata")).unwrap();
    fs::write(world.join("playerdata/fixture.dat"), b"old-player-bytes-preserved").unwrap();
    let mut manifest = current_manifest();
    manifest.save_format.playerdata_layout = 10;
    let mut bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    bytes.push(b'\n');
    fs::write(world.join(profile::MANIFEST_NAME), &bytes).unwrap();
    (world, bytes)
}

#[cfg(target_os = "linux")]
#[test]
fn known_storage_ten_upgrades_after_verified_backup_and_restart_does_not_repeat_it() {
    let fixture = Fixture::new();
    let (world, previous) = old_storage_world(&fixture);
    assert!(matches!(profile::check_current_world(&world), Err(profile::Error::TransitionRequired)));
    let guard = prepare(&world).unwrap();
    let StartupCheck::ManifestMatches { snapshot_id } = &guard.check else { panic!("незавершённый перенос не выходит из prepare"); };
    assert_ne!(snapshot_id, &current_manifest().snapshot_id);
    let backup = fixture.0.join("backups/world").join(snapshot_id);
    let verified = super::super::snapshot::verify_backup(&backup).unwrap();
    assert_eq!(verified.snapshot_id(), snapshot_id);
    assert_eq!(fs::read(backup.join("world").join(profile::MANIFEST_NAME)).unwrap(), previous);
    assert_eq!(fs::read(backup.join("world/session.lock")).unwrap(), b"fixture-lock-pid");
    assert_eq!(fs::read(backup.join("world/playerdata/fixture.dat")).unwrap(), b"old-player-bytes-preserved");
    assert_eq!(fs::read(world.join("playerdata/fixture.dat")).unwrap(), b"old-player-bytes-preserved");
    assert_eq!(fs::read(world.join("session.lock")).unwrap(), b"fixture-lock-pid");
    let marker: serde_json::Value = serde_json::from_slice(&fs::read(backup.join("snapshot.json")).unwrap()).unwrap();
    assert_eq!(marker["source_profile"]["save_format"]["playerdata_layout"], 10);
    let updated = profile::read_manifest(&world).unwrap().unwrap();
    assert_eq!(updated.save_format, current_manifest().save_format);
    assert_eq!(&updated.snapshot_id, snapshot_id);
    assert_eq!(updated.registry_id, current_manifest().registry_id);
    assert_eq!(updated.gameplay_version, current_manifest().gameplay_version);
    let published = fs::read(world.join(profile::MANIFEST_NAME)).unwrap();
    let check = guard.check.clone();
    drop(guard);
    assert_eq!(prepare(&world).unwrap().check, check);
    assert_eq!(fs::read_dir(fixture.0.join("backups/world")).unwrap().count(), 1);
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), published);
}

#[cfg(target_os = "linux")]
#[test]
fn storage_upgrade_backup_failure_and_occupied_locks_preserve_original_world() {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let (world, previous) = old_storage_world(&fixture);
    // Даже старый core, не знающий внешнего замка, исключается session.lock.
    let held = File::open(world.join("session.lock")).unwrap();
    // SAFETY: живой fd нашего fixture удерживается до drop.
    assert_eq!(unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }, 0);
    assert!(prepare(&world).is_err());
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), previous);
    drop(held);
    let outside = fixture.0.join("outside");
    fs::write(&outside, b"preserve-outside").unwrap();
    symlink(&outside, world.join("unsupported-link")).unwrap();
    assert!(prepare(&world).is_err());
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), previous);
    assert_eq!(fs::read(world.join("session.lock")).unwrap(), b"fixture-lock-pid");
    assert_eq!(fs::read(world.join("playerdata/fixture.dat")).unwrap(), b"old-player-bytes-preserved");
    assert_eq!(fs::read(&outside).unwrap(), b"preserve-outside");
    fs::remove_file(world.join("unsupported-link")).unwrap();
    // Внешний замок проверяется ещё до планирования и создания снимка.
    let external = lock_world(&world).unwrap();
    assert_eq!(prepare(&world).err().unwrap().kind(), io::ErrorKind::WouldBlock);
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), previous);
    drop(external);
}

#[cfg(target_os = "linux")]
#[test]
fn storage_upgrade_never_relabels_mismatched_manifest_before_or_during_backup() {
    let fixture = Fixture::new();
    let (world, previous) = old_storage_world(&fixture);
    let mut wrong = profile::decode(&previous).unwrap();
    wrong.registry_id = "different-registry".into();
    let wrong_bytes = serde_json::to_vec(&wrong).unwrap();
    fs::write(world.join(profile::MANIFEST_NAME), &wrong_bytes).unwrap();
    assert!(prepare(&world).is_err());
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), wrong_bytes);
    fs::write(world.join(profile::MANIFEST_NAME), &previous).unwrap();
    let result = prepare_with_upgrade(&world, |guard, expected| {
        // Детерминированная подмена между read-only plan и fd-relative snapshot.
        fs::write(world.join(profile::MANIFEST_NAME), &wrong_bytes)?;
        super::super::snapshot::create_storage_upgrade(guard, expected).map(|_| ()).map_err(io::Error::other)
    });
    assert!(result.is_err());
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), wrong_bytes);
    assert_eq!(fs::read(world.join("playerdata/fixture.dat")).unwrap(), b"old-player-bytes-preserved");
}


#[cfg(target_os = "linux")]
#[test]
fn unidentified_nonempty_world_gets_verified_copy_and_storage_receipt_without_gameplay_label() {
    let fixture = Fixture::new();
    let world = fixture.world();
    fs::create_dir(&world).unwrap();
    fs::write(world.join("session.lock"), b"unknown-old-lock").unwrap();
    fs::create_dir(world.join("playerdata")).unwrap();
    fs::write(world.join("playerdata/old.dat"), b"opaque-layout10-player").unwrap();
    fs::write(world.join("old-region"), b"opaque-layout4-chunk").unwrap();
    let guard = prepare(&world).unwrap();
    assert_eq!(guard.check, StartupCheck::LegacyUnidentified);
    assert!(!world.join(profile::MANIFEST_NAME).exists());
    let receipt = storage_receipt::read(&world).unwrap().unwrap();
    let backup = fixture.0.join("backups/world").join(&receipt.snapshot_id);
    let verified = super::super::snapshot::verify_backup(&backup).unwrap();
    assert_eq!(verified.snapshot_id(), receipt.snapshot_id);
    assert_eq!(fs::read(backup.join("world/session.lock")).unwrap(), b"unknown-old-lock");
    assert_eq!(fs::read(backup.join("world/playerdata/old.dat")).unwrap(), b"opaque-layout10-player");
    assert_eq!(fs::read(backup.join("world/old-region")).unwrap(), b"opaque-layout4-chunk");
    assert!(!backup.join("world").join(storage_receipt::NAME).exists());
    assert!(!backup.join("world").join(profile::MANIFEST_NAME).exists());
    let marker: serde_json::Value = serde_json::from_slice(&fs::read(backup.join("snapshot.json")).unwrap()).unwrap();
    assert!(marker["source_profile"].is_null());
    let bytes = fs::read(world.join(storage_receipt::NAME)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["region_layout"], 1); assert_eq!(value["chunk_layout"], 5); assert_eq!(value["playerdata_layout"], 11);
    assert!(value.get("gameplay_version").is_none()); assert!(value.get("registry_id").is_none());
    drop(guard);
    // Настоящий gameplay мог поменять файлы: повторный допуск не требует равенства старой копии.
    fs::write(world.join("playerdata/old.dat"), b"new-layout11-state").unwrap();
    assert_eq!(prepare(&world).unwrap().check, StartupCheck::LegacyUnidentified);
    assert_eq!(fs::read_dir(fixture.0.join("backups/world")).unwrap().count(), 1);
    assert_eq!(fs::read(world.join(storage_receipt::NAME)).unwrap(), bytes);
    assert_eq!(fs::read(world.join("playerdata/old.dat")).unwrap(), b"new-layout11-state");
}

#[cfg(target_os = "linux")]
#[test]
fn unidentified_backup_failure_never_publishes_receipt_or_changes_existing_files() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let world = fixture.world();
    fs::create_dir(&world).unwrap(); fs::write(world.join("session.lock"), b"old-lock").unwrap();
    fs::write(world.join("old-region"), b"unchanged").unwrap();
    let target = fixture.0.join("outside"); fs::write(&target, b"outside").unwrap();
    symlink(&target, world.join("unsupported")).unwrap();
    assert!(prepare(&world).is_err());
    assert!(!world.join(storage_receipt::NAME).exists());
    assert!(!world.join(profile::MANIFEST_NAME).exists());
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(fs::read(world.join("old-region")).unwrap(), b"unchanged");
    assert_eq!(fs::read(world.join("session.lock")).unwrap(), b"old-lock");
    assert_eq!(fs::read(&target).unwrap(), b"outside");
}

#[test]
fn malformed_future_or_ambiguous_storage_receipts_refuse_without_writing_world() {
    let fixture = Fixture::new();
    let world = fixture.world(); fs::create_dir(&world).unwrap();
    fs::write(world.join("session.lock"), b"old-lock").unwrap();
    fs::write(world.join("old-region"), b"unchanged").unwrap();
    let receipt = storage_receipt::Receipt::for_backup("0123456789abcdef0123456789abcdef");
    let valid = serde_json::to_value(receipt).unwrap();
    let duplicate = serde_json::to_string(&valid).unwrap().replacen('{', "{\"schema\":1,", 1);
    let mut invalid = vec![b"{".to_vec(), duplicate.into_bytes()];
    for key in ["schema", "region_layout", "chunk_layout", "playerdata_layout"] {
        let mut value = valid.clone(); value[key] = serde_json::json!(999);
        invalid.push(serde_json::to_vec(&value).unwrap());
    }
    for bad_id in ["", "../world", "0123456789ABCDEF0123456789ABCDEF"] {
        let mut value = valid.clone(); value["snapshot_id"] = serde_json::json!(bad_id);
        invalid.push(serde_json::to_vec(&value).unwrap());
    }
    let mut unknown = valid.clone(); unknown["gameplay_version"] = serde_json::json!("26.3");
    invalid.push(serde_json::to_vec(&unknown).unwrap());
    for bytes in invalid {
        fs::write(world.join(storage_receipt::NAME), &bytes).unwrap();
        assert!(prepare(&world).is_err());
        assert_eq!(fs::read(world.join(storage_receipt::NAME)).unwrap(), bytes);
        assert_eq!(fs::read(world.join("old-region")).unwrap(), b"unchanged");
        assert!(!world.join(profile::MANIFEST_NAME).exists());
        assert!(!fixture.0.join("backups").exists());
    }
}


#[test]
fn existing_shape_valid_receipt_is_a_local_claim_without_repeat_backup_verification() {
    let fixture = Fixture::new(); let world = fixture.world(); fs::create_dir(&world).unwrap();
    fs::write(world.join("session.lock"), b"old-lock").unwrap();
    let receipt = storage_receipt::Receipt::for_backup("0123456789abcdef0123456789abcdef");
    fs::write(world.join(storage_receipt::NAME), serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert!(!fixture.0.join("backups").exists());
    assert_eq!(prepare(&world).unwrap().check, StartupCheck::LegacyUnidentified);
    assert!(!fixture.0.join("backups").exists());
    assert!(!world.join(profile::MANIFEST_NAME).exists());
}

#[test]
fn profile_263_fresh_restart_and_unsafe_transitions() {
    let fixture = Fixture::new();
    let world = fixture.world();
    let version = mcsa_protocol::JavaVersion::V26_3;
    let guard = prepare_for_version(&world, version).unwrap();
    let manifest = profile::read_manifest(&world).unwrap().unwrap();
    assert_eq!(manifest.gameplay_version, "26.3");
    assert_eq!(manifest.registry_id, crate::gameplay_catalog::Catalog::for_version(version).registry_id());
    let bytes = fs::read(world.join(profile::MANIFEST_NAME)).unwrap();
    assert!(prepare_for_version(&world, version).is_err());
    drop(guard);
    fs::write(world.join("marker"), b"player and world bytes").unwrap();
    drop(prepare_for_version(&world, version).unwrap());
    assert!(prepare_for_version(&world, mcsa_protocol::JavaVersion::V26_1_2).is_err());
    assert_eq!(fs::read(world.join(profile::MANIFEST_NAME)).unwrap(), bytes);
    assert_eq!(fs::read(world.join("marker")).unwrap(), b"player and world bytes");
    fs::remove_file(world.join(profile::MANIFEST_NAME)).unwrap();
    assert!(prepare_for_version(&world, version).is_err());
    assert!(!world.join(profile::MANIFEST_NAME).exists());
}

#[cfg(unix)]
#[test]
fn guard_drop_unlocks_even_with_an_inherited_descriptor() {
    let fixture = Fixture::new();
    let guard = prepare_for_version(&fixture.world(), mcsa_protocol::JavaVersion::V26_3).unwrap();
    // dup воспроизводит удержание open description дочерним процессом до exec.
    let inherited = guard.lock.as_ref().unwrap().try_clone().unwrap();
    assert!(prepare_for_version(&fixture.world(), mcsa_protocol::JavaVersion::V26_3).is_err());
    drop(guard);
    let reopened = prepare_for_version(&fixture.world(), mcsa_protocol::JavaVersion::V26_3).unwrap();
    drop(inherited);
    assert!(prepare_for_version(&fixture.world(), mcsa_protocol::JavaVersion::V26_3).is_err());
    drop(reopened);
}
