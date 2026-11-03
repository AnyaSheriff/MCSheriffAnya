//! Первый запуск без вопросов и чтения стандартного ввода.
use std::{fs, io, path::Path};

/// Подготовленные настройки записываются только после проверки мира.
pub struct Prepared {
    pub first_start: bool,
    pub settings: super::mcsheriffanya::Settings,
    path: std::path::PathBuf,
    text: Option<String>,
}
impl Prepared {
    pub fn save(self) -> io::Result<()> {
        fs::create_dir_all(self.path.parent().expect("папка настроек"))?;
        if let Some(text) = self.text {
            fs::write(&self.path, text)?;
        }
        Ok(())
    }
}

/// Определяет новую установку до создания мира и сохранения настроек.
/// Непустой мир и прежние TOML-настройки никогда не включают отчёты молча.
pub fn prepare(root: &Path) -> io::Result<Prepared> {
    let path = root.join(super::mcsheriffanya::FILE);
    let old_world = match fs::read_dir(root.join("world")) {
        Ok(mut entries) => entries.next().transpose()?.is_some(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    let existing = old_world || super::mcsheriffanya::OLD_FILES.iter().any(|name| root.join(name).exists());
    let mut text = match fs::read_to_string(&path) {
        Ok(text) => text,
        // Старые TOML читаем без удаления и сохраняем прежний opt-out.
        Err(error) if error.kind() == io::ErrorKind::NotFound && existing => {
            let mut settings = super::mcsheriffanya::Settings::default();
            for name in super::mcsheriffanya::OLD_FILES {
                let old = root.join(name);
                if old.exists() {
                    let text = fs::read_to_string(old)?;
                    settings = super::mcsheriffanya::parse_old_text(&text)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
                    break;
                }
            }
            settings.validate()?;
            // Оригинальный TOML сохраняем как резервную копию; новый файл
            // получит ровно те настройки, которые проверены для runtime.
            let text = Some(settings.to_file());
            return Ok(Prepared { first_start: false, settings, path, text });
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let original = text.clone();
    let values = super::server_properties::parse_properties(&text);
    if values.get("java-version").is_some_and(|version| version != "26.3" && version != "choose") {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "config/mcsa.properties: поддерживается только java-version=26.3. Для старого мира сначала сохраните резервную копию, выполните mcsheriffanya --upgrade-world-26.3 world, затем укажите java-version=26.3. Мир автоматически не изменяется."));
    }
    if values.get("send-error-reports").is_some_and(|value| !matches!(value.as_str(), "true" | "false" | "choose")) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "config/mcsa.properties: send-error-reports должен быть true или false"));
    }
    let mut changes = Vec::new();
    if values.get("java-version").is_none_or(|value| value == "choose") {
        changes.push(("java-version", "26.3"));
    }
    if values.get("send-error-reports").is_none_or(|value| value == "choose") {
        changes.push(("send-error-reports", if existing { "false" } else { "true" }));
    }
    // Меняем только служебное choose, остальные строки и комментарии сохраняем.
    for (key, value) in &changes {
        if values.contains_key(*key) {
            text = text.split_inclusive('\n').map(|line| {
                if line.trim().split_once('=').is_some_and(|(name, _)| name.trim() == *key) {
                    format!("{key}={value}{}", if line.ends_with('\n') { "\n" } else { "" })
                } else { line.to_owned() }
            }).collect();
        } else {
            if !text.is_empty() && !text.ends_with('\n') { text.push('\n'); }
            text.push_str(&format!("{key}={value}\n"));
        }
    }
    let settings = super::mcsheriffanya::Settings::parse(&text, &path);
    settings.validate()?;
    let values = super::server_properties::parse_properties(&text);
    for (key, section) in settings.sections() {
        if !values.contains_key(key) {
            if !text.is_empty() && !text.ends_with("\n\n") {
                text.push_str(if text.ends_with('\n') { "\n" } else { "\n\n" });
            }
            text.push_str(&section);
        }
    }
    let changed = text != original;
    Ok(Prepared { first_start: !existing, settings, path, text: changed.then_some(text) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepare(root: &Path) -> io::Result<bool> {
        let prepared = super::prepare(root)?;
        let first = prepared.first_start;
        prepared.save()?;
        Ok(first)
    }
    fn root(name: &str) -> std::path::PathBuf {
        let path = std::path::PathBuf::from(format!("tmp/first-start-{name}-{}", std::process::id()));
        if path.exists() { fs::remove_dir_all(&path).unwrap(); }
        fs::create_dir_all(path.join("config")).unwrap();
        path
    }
    #[test]
    fn headless_start_defaults_to_263_without_input_or_world_creation() {
        let root = root("headless");
        assert!(prepare(&root).unwrap());
        let text = fs::read_to_string(root.join(super::super::mcsheriffanya::FILE)).unwrap();
        assert!(text.contains("java-version=26.3\n"));
        assert!(text.contains("send-error-reports=true\n"));
        assert!(!root.join("world").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn panel_empty_world_and_partial_config_are_new_installation() {
        let root = root("panel");
        fs::create_dir(root.join("world")).unwrap();
        let path = root.join(super::super::mcsheriffanya::FILE);
        fs::write(&path, "# панель\nbedrock-port=0\n").unwrap();
        assert!(prepare(&root).unwrap());
        assert!(fs::read_to_string(path).unwrap().starts_with("# панель\nbedrock-port=0\njava-version=26.3\nsend-error-reports=true\n"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn old_choose_normalizes_and_explicit_false_survives() {
        let root = root("choose");
        let path = root.join(super::super::mcsheriffanya::FILE);
        fs::write(&path, "# сохранить\njava-version=choose\nsend-error-reports=choose\nauth-enabled=false\n").unwrap();
        assert!(prepare(&root).unwrap());
        assert!(fs::read_to_string(&path).unwrap().starts_with("# сохранить\njava-version=26.3\nsend-error-reports=true\nauth-enabled=false\n"));
        fs::write(&path, "java-version=26.3\nsend-error-reports=false\n").unwrap();
        prepare(&root).unwrap();
        assert!(fs::read_to_string(path).unwrap().contains("send-error-reports=false"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_world_missing_report_key_remains_disabled_and_untouched() {
        let root = root("old-world");
        fs::create_dir(root.join("world")).unwrap();
        fs::write(root.join("world/level.dat"), b"old-world").unwrap();
        let path = root.join(super::super::mcsheriffanya::FILE);
        fs::write(&path, "java-version=26.3\n").unwrap();
        assert!(!prepare(&root).unwrap());
        assert!(fs::read_to_string(path).unwrap().contains("send-error-reports=false"));
        assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"old-world");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_primary_and_malformed_values_error_without_any_rewrite() {
        let root = root("invalid");
        let path = root.join(super::super::mcsheriffanya::FILE);
        for text in ["java-version=26.1.2\n", "java-version=oops\n", "java-version=\n", "java-version=26.3\nsend-error-reports=maybe\n"] {
            fs::write(&path, text).unwrap();
            assert_eq!(prepare(&root).unwrap_err().kind(), io::ErrorKind::InvalidInput);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
            assert!(!root.join("world").exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn old_world_rejection_does_not_save_config_or_change_world() {
        let root = root("guard");
        fs::create_dir(root.join("world")).unwrap();
        fs::write(root.join("world/level.dat"), b"old-world").unwrap();
        let path = root.join(super::super::mcsheriffanya::FILE);
        fs::write(&path, "auth-enabled=false\n").unwrap();
        let prepared = super::prepare(&root).unwrap();
        assert!(!prepared.settings.send_error_reports);
        assert!(crate::world::startup::prepare_for_version(&root.join("world"), prepared.settings.java_version.unwrap()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "auth-enabled=false\n");
        assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"old-world");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_toml_is_read_without_removal_and_missing_reports_stay_off() {
        let root = root("toml");
        let path = root.join(super::super::mcsheriffanya::OLD_FILES[0]);
        for report in ["", "send_error_reports = false\n"] {
            let text = format!("java_version = \"26.3\"\nauth_enabled = false\n{report}");
            fs::write(&path, &text).unwrap();
            let prepared = super::prepare(&root).unwrap();
            assert!(!prepared.first_start);
            assert!(!prepared.settings.send_error_reports);
            assert_eq!(prepared.settings.offline_license_mode, super::super::mcsheriffanya::OfflineLicenseMode::Disabled);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
            assert!(!root.join(super::super::mcsheriffanya::FILE).exists());
        }
        let prepared = super::prepare(&root).unwrap();
        let expected = prepared.settings.clone();
        prepared.save().unwrap();
        let config = root.join(super::super::mcsheriffanya::FILE);
        assert_eq!(super::super::mcsheriffanya::Settings::parse(&fs::read_to_string(&config).unwrap(), &config), expected);
        assert!(path.exists(), "исходный TOML остаётся резервной копией");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_preparation_does_not_write_before_world_guard() {
        let root = root("deferred");
        let prepared = super::prepare(&root).unwrap();
        assert!(prepared.first_start);
        assert!(prepared.settings.send_error_reports);
        assert_eq!(prepared.settings.java_version, Some(mcsa_protocol::JavaVersion::V26_3));
        assert!(!root.join(super::super::mcsheriffanya::FILE).exists());
        assert!(!root.join("world").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_toml_2612_is_rejected_without_migration_or_deletion() {
        let root = root("toml-old");
        let path = root.join(super::super::mcsheriffanya::OLD_FILES[0]);
        let text = "java_version = \"26.1.2\"\nsend_error_reports = false\n";
        fs::write(&path, text).unwrap();
        let error = super::prepare(&root).err().expect("старый основной профиль запрещён");
        assert!(error.to_string().contains("--upgrade-world-26.3"));
        assert_eq!(fs::read_to_string(path).unwrap(), text);
        assert!(!root.join(super::super::mcsheriffanya::FILE).exists());
        fs::remove_dir_all(root).unwrap();
    }

}
