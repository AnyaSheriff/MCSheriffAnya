use super::*;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT:AtomicU64=AtomicU64::new(0);
        let root=std::env::temp_dir().join(format!("mcsa-snapshot-{}-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("world")).unwrap();
        fs::write(root.join("world/session.lock"),b"old-process-marker\n").unwrap();
        // Как у manifest fixtures: локальное утверждение уже допущенных раскладок.
        // Его наличие не выдаётся за проверку старой копии; каждый test создаёт свою.
        let receipt = crate::world::startup::storage_receipt::Receipt::for_backup("fedcba9876543210fedcba9876543210");
        fs::write(root.join("world").join(crate::world::startup::storage_receipt::NAME), serde_json::to_vec(&receipt).unwrap()).unwrap();
        Self(root)
    }
    fn world(&self)->PathBuf { self.0.join("world") }
    fn guard(&self)->StartupGuard { crate::world::startup::prepare(&self.world()).unwrap() }
    fn collection(&self)->PathBuf { self.0.join("backups/world") }
    fn full(&self) {
        for folder in ["dimensions/minecraft/the_nether/regions","dimensions/minecraft/the_end/regions","playerdata","nested/empty","maps","ticks"] {
            fs::create_dir_all(self.world().join(folder)).unwrap();
        }
        for (path,bytes) in [("dimensions/minecraft/the_nether/session.lock",b"nether-lock".as_slice()),
            ("dimensions/minecraft/the_end/session.lock",b"end-lock"),
            ("dimensions/minecraft/the_nether/regions/r.0.0",b"nether-region"),
            ("dimensions/minecraft/the_end/regions/r.0.0",b"end-region"),
            ("playerdata/own.dat",b"fixture-player-v10-cursor-components"),
            (".unknown",b"hidden-plugin-file"),("nested/custom.tmp",b"must-not-skip"),("maps/1.dat",b"map"),("ticks/pending",b"tick")] {
            fs::write(self.world().join(path),bytes).unwrap();
        }
        let invalid=std::ffi::OsString::from_vec(vec![b'n',0xff,b'x']);
        fs::write(self.world().join(invalid),b"nonutf8-name").unwrap();
    }
}
impl Drop for Fixture { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
fn options(id:&str)->Options { Options { memory:MAX_MEMORY,id:Some(id.to_string()) } }
const ID:&str="0123456789abcdef0123456789abcdef";
fn unchanged_bytes(root:&Path)->Vec<(PathBuf,Vec<u8>)> {
    fn walk(root:&Path,path:&Path,out:&mut Vec<(PathBuf,Vec<u8>)>) {
        let mut children:Vec<_>=fs::read_dir(path).unwrap().map(|r|r.unwrap().path()).collect();children.sort();
        for child in children { if child.is_dir() { walk(root,&child,out); } else { out.push((child.strip_prefix(root).unwrap().to_path_buf(),fs::read(&child).unwrap())); } }
    }
    let mut out=Vec::new();walk(root,root,&mut out);out
}

#[test]
fn full_folder_bytes_empty_directories_nonutf8_names_and_independence() {
    let fixture=Fixture::new();fixture.full();
    let original=unchanged_bytes(&fixture.world());
    let guard=fixture.guard();
    let backup=create_with(&guard,options(ID),&mut |_|Ok(())).unwrap();
    assert_eq!(backup.snapshot_id(),ID);
    assert!(backup.directory().join("world/nested/empty").is_dir());
    assert_eq!(unchanged_bytes(&backup.directory().join("world")),original);
    assert_eq!(unchanged_bytes(&fixture.world()),original);
    assert_eq!(backup.file_count(),original.len() as u64);
    assert_eq!(backup.total_bytes(),original.iter().map(|(_,b)|b.len() as u64).sum::<u64>());
    assert!(!fixture.world().join("mcsa-world.json").exists(),"старый мир не маркируется");
    assert!(!same_inode(&fs::metadata(fixture.world().join("playerdata/own.dat")).unwrap(),
        &fs::metadata(backup.directory().join("world/playerdata/own.dat")).unwrap()));
    verify(backup.directory()).unwrap(); // чтение backup не должно делать atime ошибкой.
    drop(guard);
    fs::write(fixture.world().join("playerdata/own.dat"),b"new-source-content").unwrap();
    assert_eq!(fs::read(backup.directory().join("world/playerdata/own.dat")).unwrap(),b"fixture-player-v10-cursor-components");
    verify(backup.directory()).unwrap();
}

#[test]
fn dimensions_need_existing_locks_and_unknown_dimensions_refuse_before_destination() {
    let fixture=Fixture::new();fixture.full();
    let guard=fixture.guard();
    let path=fixture.world().join("dimensions/minecraft/the_nether/session.lock");
    fs::remove_file(&path).unwrap();
    assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());assert!(!path.exists());
    fs::write(&path,b"nether-lock").unwrap();
    fs::create_dir_all(fixture.world().join("dimensions/other/private")).unwrap();
    assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());
}

#[test]
fn every_legacy_dimension_lock_is_exclusive_and_preserves_pid_bytes() {
    let fixture=Fixture::new();fixture.full();let guard=fixture.guard();
    for relative in ["session.lock","dimensions/minecraft/the_nether/session.lock","dimensions/minecraft/the_end/session.lock"] {
        let path=fixture.world().join(relative);let bytes=fs::read(&path).unwrap();
        let held=File::open(&path).unwrap();
        // SAFETY: отдельный открытый fd; удерживается до конца шага.
        assert_eq!(unsafe { libc::flock(held.as_raw_fd(),libc::LOCK_EX|libc::LOCK_NB) },0);
        assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());
        assert_eq!(fs::read(path).unwrap(),bytes);drop(held);
    }
    create(&guard).unwrap();
}

#[test]
fn links_hardlinks_fifo_and_socket_refuse_without_destination() {
    let fixture=Fixture::new();let guard=fixture.guard();
    let outside=fixture.0.join("external-sentinel");fs::write(&outside,b"outside unchanged").unwrap();
    let bad=fixture.world().join("bad");
    symlink(&outside,&bad).unwrap();assert!(create(&guard).is_err());fs::remove_file(&bad).unwrap();
    fs::hard_link(&outside,&bad).unwrap();assert!(create(&guard).is_err());fs::remove_file(&bad).unwrap();
    let c=CString::new(bad.as_os_str().as_bytes()).unwrap();
    // SAFETY: собственный временный fixture, валидное имя.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(),0o600) },0);
    assert!(create(&guard).is_err());fs::remove_file(&bad).unwrap();
    let socket=std::os::unix::net::UnixListener::bind(&bad).unwrap();
    assert!(create(&guard).is_err());drop(socket);fs::remove_file(&bad).unwrap();
    assert!(!fixture.0.join("backups").exists());assert_eq!(fs::read(outside).unwrap(),b"outside unchanged");
}

#[test]
fn unsupported_metadata_and_inventory_budget_refuse_before_destination() {
    let fixture=Fixture::new();let guard=fixture.guard();
    let path=fixture.world().join("restricted");fs::write(&path,b"content").unwrap();
    fs::set_permissions(&path,fs::Permissions::from_mode(0o4600)).unwrap();
    assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());
    fs::set_permissions(&path,fs::Permissions::from_mode(0o600)).unwrap();
    assert!(create_with(&guard,Options { memory:1,id:Some(ID.into()) },&mut |_|Ok(())).is_err());
    assert!(!fixture.0.join("backups").exists());
}

#[test]
fn guard_identity_and_backup_destination_cannot_be_redirected() {
    let fixture=Fixture::new();let guard=fixture.guard();
    fs::rename(fixture.world(),fixture.0.join("old-world")).unwrap();
    fs::create_dir(fixture.world()).unwrap();fs::write(fixture.world().join("session.lock"),b"different world").unwrap();
    assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());
    fs::remove_dir_all(fixture.world()).unwrap();fs::rename(fixture.0.join("old-world"),fixture.world()).unwrap();
    let elsewhere=fixture.0.join("elsewhere");fs::create_dir(&elsewhere).unwrap();symlink(&elsewhere,fixture.0.join("backups")).unwrap();
    assert!(create(&guard).is_err());assert_eq!(fs::read_dir(elsewhere).unwrap().count(),0);
}

#[test]
fn source_change_after_inventory_or_before_publication_never_returns_verified() {
    for point in [Point::InventoryComplete,Point::BeforeFileOpen,Point::BeforeRename] {
        let fixture=Fixture::new();fs::write(fixture.world().join("data"),b"original").unwrap();let guard=fixture.guard();
        let mut fired=false;
        let error=create_with(&guard,options(ID),&mut |at| { if at==point && !fired { fired=true;fs::write(fixture.world().join("data"),b"changed")?; } Ok(()) }).unwrap_err();
        assert!(fired);assert!(!error.publication_uncertain);assert!(!fixture.collection().join(ID).exists());
        if point==Point::InventoryComplete { assert!(!fixture.0.join("backups").exists()); }
    }
}

#[test]
fn injected_write_sync_and_marker_failures_leave_only_partial_and_source_unchanged() {
    for point in [Point::PartialCreated,Point::BeforeWrite,Point::BeforeFileSync,Point::BeforeFileVerify,
        Point::EntriesSynced,Point::MarkerSynced,Point::BeforeRename] {
        let fixture=Fixture::new();fs::write(fixture.world().join("data"),b"original").unwrap();let guard=fixture.guard();
        let error=create_with(&guard,options(ID),&mut |at| {
            if at==point { Err(io::Error::from_raw_os_error(libc::ENOSPC)) } else { Ok(()) }
        }).unwrap_err();
        assert!(!error.publication_uncertain);assert_eq!(fs::read(fixture.world().join("data")).unwrap(),b"original");
        assert!(!fixture.collection().join(ID).exists());
        let partial=error.retained.unwrap();assert!(partial.is_dir());assert!(verify(&partial).is_err());
    }
}

#[test]
fn published_but_unsynced_is_explicit_and_can_be_fully_reverified() {
    let fixture=Fixture::new();let guard=fixture.guard();
    let error=create_with(&guard,options(ID),&mut |at| {
        if at==Point::Published { Err(io::Error::other("injected parent sync failure")) } else { Ok(()) }
    }).unwrap_err();
    assert!(error.publication_uncertain);
    let path=error.retained.unwrap();assert_eq!(path,fixture.collection().join(ID));verify(&path).unwrap();
}

#[test]
fn id_collision_never_overwrites_final_or_partial() {
    let fixture=Fixture::new();let guard=fixture.guard();
    let backup=create_with(&guard,options(ID),&mut |_|Ok(())).unwrap();
    let marker=fs::read(backup.directory().join("snapshot.json")).unwrap();
    assert!(create_with(&guard,options(ID),&mut |_|Ok(())).is_err());
    assert_eq!(fs::read(backup.directory().join("snapshot.json")).unwrap(),marker);
    let other="11111111111111111111111111111111";
    let partial=fixture.collection().join(format!("{other}.partial"));fs::create_dir(&partial).unwrap();fs::write(partial.join("keep"),b"do not replace").unwrap();
    assert!(create_with(&guard,options(other),&mut |_|Ok(())).is_err());assert_eq!(fs::read(partial.join("keep")).unwrap(),b"do not replace");
}

#[test]
fn verification_rejects_corruption_missing_extra_and_symlinked_files() {
    let fixture=Fixture::new();fs::write(fixture.world().join("data"),b"original").unwrap();let guard=fixture.guard();
    let backup=create_with(&guard,options(ID),&mut |_|Ok(())).unwrap();let world=backup.directory().join("world");let data=world.join("data");
    fs::write(&data,b"corrupt!").unwrap();assert!(verify(backup.directory()).is_err());
    fs::remove_file(&data).unwrap();assert!(verify(backup.directory()).is_err());
    symlink(fixture.world().join("data"),&data).unwrap();assert!(verify(backup.directory()).is_err());fs::remove_file(data).unwrap();
    fs::write(world.join("extra"),b"must be detected").unwrap();assert!(verify(backup.directory()).is_err());
}

#[test]
fn manifest_paths_cannot_escape_and_lines_are_bounded() {
    for path_hex in [vec!["2e2e".into()],vec!["2f657463".into()],vec!["00".into()],vec!["".into()],vec!["ff/0".into()]] {
        let record=Record { path_hex,kind:Kind::File,size:0,sha256:Some("0".repeat(64)),mode:0o600,uid:0,gid:0,atime:0,atime_ns:0,mtime:0,mtime_ns:0 };
        assert!(record.path().is_err());
    }
    let bytes=vec![b'x';MAX_LINE+1];assert!(bounded_line(&mut io::Cursor::new(bytes)).is_err());
}

// Отдельные процессы проверяют границу crash, не трогая внешние серверы.
#[test]
fn crash_child() {
    let Some(root)=std::env::var_os("MCSA_SNAPSHOT_FIXTURE_ROOT") else { return; };
    let point=std::env::var("MCSA_SNAPSHOT_CRASH_POINT").unwrap();
    let guard=crate::world::startup::prepare(&PathBuf::from(root).join("world")).unwrap();
    let result=create_with(&guard,options(ID),&mut |at| {
        if format!("{at:?}")==point { std::process::exit(73); } Ok(())
    });
    panic!("crash point not reached: {result:?}");
}
#[test]
fn crash_at_each_durable_boundary_never_promotes_partial() {
    for point in [Point::PartialCreated,Point::EntriesSynced,Point::MarkerSynced,Point::Published,Point::ParentSynced] {
        let fixture=Fixture::new();fs::write(fixture.world().join("data"),b"original").unwrap();
        let status=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","world::snapshot::linux::tests::crash_child","--nocapture"])
            .env("MCSA_SNAPSHOT_FIXTURE_ROOT",&fixture.0).env("MCSA_SNAPSHOT_CRASH_POINT",format!("{point:?}")).status().unwrap();
        assert_eq!(status.code(),Some(73));
        assert_eq!(fs::read(fixture.world().join("data")).unwrap(),b"original");
        let final_path=fixture.collection().join(ID);let partial=fixture.collection().join(format!("{ID}.partial"));
        if matches!(point,Point::Published|Point::ParentSynced) { verify(&final_path).unwrap();assert!(!partial.exists()); }
        else { assert!(!final_path.exists());assert!(partial.exists());assert!(verify(&partial).is_err()); }
        // Crash освободил все advisory locks; новый guard снова доступен.
        let _guard=fixture.guard();
    }
}

#[test]
fn changed_source_to_symlink_and_corrupted_destination_fail_before_publication() {
    for mutate_source in [true,false] {
        let fixture=Fixture::new();fs::write(fixture.world().join("data"),b"original").unwrap();
        let outside=fixture.0.join("outside");fs::write(&outside,b"outside!!").unwrap();let guard=fixture.guard();
        let mut fired=false;
        let point=if mutate_source {Point::BeforeFileOpen} else {Point::BeforeFileVerify};
        let error=create_with(&guard,options(ID),&mut |at| {
            if at==point && !fired {
                fired=true;
                if mutate_source { fs::remove_file(fixture.world().join("data"))?;symlink(&outside,fixture.world().join("data"))?; }
                else { fs::write(fixture.collection().join(format!("{ID}.partial/world/data")),b"corrupt!")?; }
            }
            Ok(())
        }).unwrap_err();
        assert!(fired);assert!(!error.publication_uncertain);assert!(!fixture.collection().join(ID).exists());
        assert_eq!(fs::read(outside).unwrap(),b"outside!!");
    }
}

#[test]
fn final_verification_rejects_manifest_traversal_without_reading_external_paths() {
    let fixture=Fixture::new();let guard=fixture.guard();let backup=create_with(&guard,options(ID),&mut |_|Ok(())).unwrap();
    let list_path=backup.directory().join("files.jsonl");let data=fs::read_to_string(&list_path).unwrap();
    let mut rows:Vec<serde_json::Value>=data.lines().map(|line|serde_json::from_str(line).unwrap()).collect();
    rows[1]["path_hex"]=serde_json::json!(["2e2e","6f757473696465"]);
    let mutated=rows.iter().map(|v|serde_json::to_string(v).unwrap()+"\n").collect::<String>();
    fs::write(&list_path,&mutated).unwrap();
    let marker_path=backup.directory().join("snapshot.json");let mut marker:Marker=serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker.entries_sha256=hex(digest::digest(&digest::SHA256,mutated.as_bytes()).as_ref());
    fs::write(marker_path,serde_json::to_vec(&marker).unwrap()).unwrap();
    assert!(verify(backup.directory()).is_err());
}

#[test]
fn xattrs_depth_and_size_estimation_have_explicit_limits() {
    let fixture=Fixture::new();let guard=fixture.guard();let path=fixture.world().join("attrs");fs::write(&path,b"data").unwrap();
    let file=File::open(&path).unwrap();let name=CString::new("user.mcsa_fixture").unwrap();let value=b"value";
    // SAFETY: собственный файл; действующие буферы имени и значения.
    assert_eq!(unsafe {libc::fsetxattr(file.as_raw_fd(),name.as_ptr(),value.as_ptr().cast(),value.len(),0)},0);
    assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());drop(file);fs::remove_file(path).unwrap();
    let mut path=fixture.world();for _ in 0..=MAX_DEPTH {path=path.join("d");fs::create_dir(&path).unwrap();}
    assert!(create(&guard).is_err());assert!(!fixture.0.join("backups").exists());
    assert!(space_required(u128::MAX,1,4096).is_err());
    assert!(space_required(0,u128::MAX,4096).is_err());
    assert!(space_required(1024,2,4096).unwrap()>=64*1024*1024+1024);
}


#[test]
fn replaced_dimension_lock_is_not_the_held_inode() {
    for point in [Point::LocksAcquired,Point::BeforeRename] {
        for relative in ["session.lock","dimensions/minecraft/the_nether/session.lock","dimensions/minecraft/the_end/session.lock"] {
            let fixture=Fixture::new();fixture.full();let guard=fixture.guard();
            let lock=fixture.world().join(relative);let original=fs::read(&lock).unwrap();
            let moved=fixture.0.join("held-old-lock");let mut fired=false;
            let error=create_with(&guard,options(ID),&mut |at| {
                if at==point { fired=true;fs::rename(&lock,&moved)?;fs::write(&lock,&original)?; }
                Ok(())
            }).unwrap_err();
            assert!(fired);assert!(!error.publication_uncertain);assert!(!fixture.collection().join(ID).exists());
            if point==Point::LocksAcquired {assert!(!fixture.0.join("backups").exists());}
            assert_eq!(fs::read(&lock).unwrap(),original);assert_eq!(fs::read(moved).unwrap(),original);
        }
    }
}

#[test]
fn dimension_appearing_after_lock_acquisition_refuses_before_copy() {
    let fixture=Fixture::new();let guard=fixture.guard();let mut fired=false;
    let error=create_with(&guard,options(ID),&mut |at| {
        if at==Point::LocksAcquired {
            fired=true;let dim=fixture.world().join("dimensions/minecraft/the_nether");
            fs::create_dir_all(&dim)?;fs::write(dim.join("session.lock"),b"new lock")?;
        }
        Ok(())
    }).unwrap_err();
    assert!(fired);assert!(!error.publication_uncertain);assert!(!fixture.0.join("backups").exists());
}

#[test]
fn publication_rejects_replaced_partial_and_final_directory_names() {
    for point in [Point::BeforeRename,Point::Published,Point::ParentSynced] {
        let fixture=Fixture::new();let guard=fixture.guard();let mut fired=false;
        let name=if point==Point::BeforeRename {format!("{ID}.partial")} else {ID.into()};
        let path=fixture.collection().join(name);let moved=fixture.collection().join("verified-but-moved");
        let error=create_with(&guard,options(ID),&mut |at| {
            if at==point {
                fired=true;fs::rename(&path,&moved)?;fs::create_dir(&path)?;fs::write(path.join("replacement"),b"never verified")?;
            }
            Ok(())
        }).unwrap_err();
        assert!(fired);assert_eq!(error.publication_uncertain,point==Point::Published);
        assert_eq!(fs::read(path.join("replacement")).unwrap(),b"never verified");
        assert!(moved.join("snapshot.json").is_file());
        if point==Point::BeforeRename {assert!(!fixture.collection().join(ID).exists());}
        else {assert!(verify(&fixture.collection().join(ID)).is_err());}
    }
}

#[test]
fn verification_rejects_directory_replaced_after_its_bytes_were_checked() {
    let fixture=Fixture::new();let guard=fixture.guard();let backup=create_with(&guard,options(ID),&mut |_|Ok(())).unwrap();
    let path=backup.directory();let moved=fixture.collection().join("verified-but-moved");let mut fired=false;
    assert!(verify_with(path,&mut |point| {
        if point==Point::VerificationComplete {
            fired=true;fs::rename(path,&moved)?;fs::create_dir(path)?;fs::write(path.join("replacement"),b"unverified")?;
        }
        Ok(())
    }).is_err());
    assert!(fired);assert!(moved.join("snapshot.json").is_file());
    assert_eq!(fs::read(path.join("replacement")).unwrap(),b"unverified");
}

#[test]
fn storage_upgrade_failures_retain_partial_or_verified_backup_without_replacing_old_manifest() {
    use crate::world::profile;
    for fail_at in [Point::BeforeWrite, Point::ParentSynced] {
        let fixture = Fixture::new();
        let mut guard = fixture.guard();
        let current = profile::current_profile();
        let old = profile::Manifest { schema: profile::MANIFEST_SCHEMA,
            save_format: profile::SaveFormat { region_layout: 1, playerdata_layout: 10 },
            registry_id: current.registry_id, gameplay_version: current.gameplay_version, snapshot_id: ID.into() };
        let old_bytes = serde_json::to_vec_pretty(&old).unwrap();
        fs::write(fixture.world().join(profile::MANIFEST_NAME), &old_bytes).unwrap();
        guard.check = profile::StartupCheck::StorageUpgradePending { snapshot_id: ID.into() };
        let original = unchanged_bytes(&fixture.world());
        let error = create_with_completion(&guard, options(ID), &mut |point| {
            if point == fail_at { Err(io::Error::other("fixture failure before manifest publication")) } else { Ok(()) }
        }, Some(&old), false, &mut |source, backup, uncertain| {
            publish_storage_manifest(source, &old, backup, uncertain)
        }).unwrap_err();
        assert_eq!(unchanged_bytes(&fixture.world()), original);
        let retained = error.retained.expect("созданная копия остаётся для проверки");
        assert!(retained.exists());
        assert!(!error.publication_uncertain);
        if fail_at == Point::ParentSynced {
            let verified = verify(&retained).unwrap();
            assert_eq!(verified.snapshot_id(), ID);
            assert_eq!(fs::read(retained.join("world").join(profile::MANIFEST_NAME)).unwrap(), old_bytes);
        } else {
            assert!(retained.file_name().unwrap().to_string_lossy().ends_with(".partial"));
            assert!(verify(&retained).is_err());
        }
    }
}

#[test]
fn storage_manifest_publication_keeps_dimension_locks_and_rejects_temp_collision_without_world_mutation() {
    use crate::world::profile;
    let fixture = Fixture::new();
    let mut guard = fixture.guard();
    let current = profile::current_profile();
    let old = profile::Manifest { schema: profile::MANIFEST_SCHEMA,
        save_format: profile::SaveFormat { region_layout: 1, playerdata_layout: 10 },
        registry_id: current.registry_id, gameplay_version: current.gameplay_version, snapshot_id: ID.into() };
    let old_bytes = serde_json::to_vec(&old).unwrap();
    fs::write(fixture.world().join(profile::MANIFEST_NAME), &old_bytes).unwrap();
    guard.check = profile::StartupCheck::StorageUpgradePending { snapshot_id: ID.into() };
    let original = unchanged_bytes(&fixture.world());
    let temp = fixture.0.join(format!(".mcsa-manifest-{ID}.tmp"));
    fs::write(&temp, b"existing-file-preserved").unwrap();
    let error = create_with_completion(&guard, options(ID), &mut |_| Ok(()), Some(&old), false,
        &mut |source, backup, uncertain| {
            let lock = regular(&source.root, b"session.lock")?;
            // SAFETY: новый fd того же fixture: существующий lock ещё удерживается.
            assert_ne!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }, 0);
            assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
            publish_storage_manifest(source, &old, backup, uncertain)
        }).unwrap_err();
    assert_eq!(unchanged_bytes(&fixture.world()), original);
    assert_eq!(fs::read(&temp).unwrap(), b"existing-file-preserved");
    assert!(!error.publication_uncertain);
    verify(&error.retained.unwrap()).unwrap();
}

#[test]
fn unidentified_storage_receipt_collision_after_verified_backup_never_overwrites_receipt() {
    use crate::world::startup::storage_receipt;
    let fixture = Fixture::new();
    let guard = fixture.guard();
    fs::remove_file(fixture.world().join(storage_receipt::NAME)).unwrap();
    let original_lock = fs::read(fixture.world().join("session.lock")).unwrap();
    let collision = b"externally-created-receipt-preserved";
    let error = create_with_completion(&guard, options(ID), &mut |point| {
        if point == Point::ParentSynced {
            fs::write(fixture.world().join(storage_receipt::NAME), collision)?;
        }
        Ok(())
    }, None, true, &mut |source, backup, uncertain| publish_storage_receipt(source, backup, uncertain)).unwrap_err();
    assert_eq!(fs::read(fixture.world().join(storage_receipt::NAME)).unwrap(), collision);
    assert_eq!(fs::read(fixture.world().join("session.lock")).unwrap(), original_lock);
    assert!(!fixture.world().join(crate::world::profile::MANIFEST_NAME).exists());
    assert!(!error.publication_uncertain);
    let backup = error.retained.unwrap(); verify(&backup).unwrap();
    assert!(!backup.join("world").join(storage_receipt::NAME).exists());
    assert!(!fixture.0.join(format!(".mcsa-storage-{ID}.tmp")).exists());
}
