//! Проверяет настоящий порядок startup: отказ до World::open и слушателей.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
}

#[test]
fn incomplete_first_start_does_not_create_a_world_lock() {
    let root = Fixture(std::env::temp_dir().join(format!("mcsa-startup-flags-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
    fs::create_dir(&root.0).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcsheriffanya"))
        .current_dir(&root.0).stdin(Stdio::null()).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Запуск остановлен до выбора"));
    assert!(!root.0.join("world").exists());
    assert!(!root.0.join(".mcsa-world.lock").exists());
}

#[test]
fn unavailable_config_version_keeps_its_existing_error_for_unidentified_worlds() {
    let root = Fixture(std::env::temp_dir().join(format!("mcsa-startup-config-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
    fs::create_dir_all(root.0.join("world")).unwrap();
    fs::create_dir(root.0.join("config")).unwrap();
    fs::write(root.0.join("world/legacy-marker"), b"preserve").unwrap();
    fs::write(root.0.join("config/mcsa.properties"),
        "java-version=26.2\nsend-error-reports=false\nauth-enabled=false\noffline-license-mode=disabled\nbedrock-port=0\n").unwrap();
    fs::write(root.0.join("config/server.properties"), "server-port=0\nonline-mode=false\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcsheriffanya"))
        .current_dir(&root.0).stdin(Stdio::null()).output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("эта сборка поддерживает java-version=26.1.2"), "{error}");
    assert!(!root.0.join("world/mcsa-world.json").exists());
    assert!(!root.0.join("world/mcsa-storage.json").exists());
    assert!(!root.0.join("backups").exists());
    assert!(!root.0.join(".mcsa-world.lock").exists());
    assert_eq!(fs::read(root.0.join("world/legacy-marker")).unwrap(), b"preserve");
}

fn tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let mut children: Vec<_> = fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()).collect();
        children.sort();
        for child in children {
            let relative = child.strip_prefix(root).unwrap().to_path_buf();
            if child.is_dir() {
                entries.push((relative, None));
                visit(root, &child, entries);
            } else {
                entries.push((relative, Some(fs::read(child).unwrap())));
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}

#[test]
fn unsupported_manifests_stop_before_writing_world_or_starting_network() {
    let root = Fixture(std::env::temp_dir().join(format!("mcsa-startup-binary-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
    fs::create_dir_all(root.0.join("world/regions")).unwrap();
    fs::create_dir(root.0.join("config")).unwrap();
    fs::write(root.0.join("config/mcsa.properties"),
        "java-version=26.1.2\nsend-error-reports=false\nauth-enabled=false\noffline-license-mode=disabled\nbedrock-port=0\n").unwrap();
    // Даже при регрессии проверки нельзя занять игровой порт настоящего сервера.
    fs::write(root.0.join("config/server.properties"), "server-port=0\nonline-mode=false\n").unwrap();
    fs::write(root.0.join("world/blocks.mcrw"), b"preserve-single-file").unwrap();
    fs::write(root.0.join("world/regions/r.0.0.rcw"), b"preserve-old-region").unwrap();
    let base = serde_json::json!({
        "schema": 1,
        "save_format": { "region_layout": 1, "playerdata_layout": 10 },
        "registry_id": "intentionally-different-registry",
        "gameplay_version": "26.1.2",
        "snapshot_id": "0123456789abcdef0123456789abcdef"
    });
    let mut unsupported = base.clone();
    unsupported["gameplay_version"] = "unsupported-profile".into();
    let mut future = base.clone();
    future["schema"] = 2.into();
    for manifest in [
        serde_json::to_vec(&base).unwrap(),
        serde_json::to_vec(&unsupported).unwrap(),
        serde_json::to_vec(&future).unwrap(),
        b"{".to_vec(),
    ] {
        fs::write(root.0.join("world/mcsa-world.json"), manifest).unwrap();
        let before = tree(&root.0.join("world"));
        // Файлы вместо pipes: даже регрессия с потоком логов не заполнит pipe.
        let stdout = fs::File::create(root.0.join("stdout.log")).unwrap();
        let stderr = fs::File::create(root.0.join("stderr.log")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_mcsheriffanya"))
            .current_dir(&root.0).stdin(Stdio::null()).stdout(stdout).stderr(stderr).spawn().unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if started.elapsed() > Duration::from_secs(10) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("неподходящий manifest не остановил запуск за 10 секунд");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(!status.success());
        assert_eq!(tree(&root.0.join("world")), before);
        let output = fs::read_to_string(root.0.join("stdout.log")).unwrap();
        let error = fs::read_to_string(root.0.join("stderr.log")).unwrap();
        assert!(error.contains("запуск мира остановлен"), "{output}\n{error}");
        assert!(!output.contains("Сервер запущен"));
        assert!(!root.0.join("logs").exists());
    }
}
