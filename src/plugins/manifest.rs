// Описание плагина: файл `plugin.toml` рядом с `.wasm`.
//
//     name = "hello"
//     version = "0.1.0"
//     api = "0.1"
//     author = "Аня"
//     description = "Приветствие и команда /hello"
//     main = "hello.wasm"          # необязательно: по умолчанию <name>.wasm
//     dependencies = ["other"]     # необязательно: что должно быть загружено раньше

use std::path::Path;

use serde::Deserialize;

/// Версия API, которую знает сервер (`mcsa:api@0.1.0`). Плагин с другой
/// версией `major.minor` не грузится: до 1.0 любая смена минорной версии
/// может ломать совместимость.
pub const API_VERSION: &str = "0.1";

/// Самое длинное имя плагина.
const MAX_NAME: usize = 32;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub api: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// Имя `.wasm` в папке плагина; нет — `<name>.wasm`.
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

impl Manifest {
    /// Разбирает и проверяет текст `plugin.toml`.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let manifest: Manifest = toml::from_str(text).map_err(|error| format!("plugin.toml: {}", error.message()))?;

        manifest.check()?;
        Ok(manifest)
    }

    /// Читает `plugin.toml` из папки плагина.
    pub fn read(folder: &Path) -> Result<Manifest, String> {
        let text = std::fs::read_to_string(folder.join("plugin.toml")).map_err(|error| format!("plugin.toml: {error}"))?;

        Self::parse(&text)
    }

    fn check(&self) -> Result<(), String> {
        if !valid_name(&self.name) {
            return Err(format!("имя плагина «{}» не годится: нужны a-z, 0-9, «-» и «_», до {MAX_NAME} знаков", self.name));
        }

        if !api_compatible(&self.api) {
            return Err(format!("плагин написан под API {}, а сервер знает {}", self.api, API_VERSION));
        }

        if let Some(main) = &self.main
            && (main.contains(['/', '\\']) || main.starts_with('.') || !main.ends_with(".wasm"))
        {
            return Err(format!("main «{main}»: нужно имя файла .wasm в папке плагина"));
        }

        if let Some(own) = self.dependencies.iter().find(|name| **name == self.name) {
            return Err(format!("плагин зависит сам от себя: {own}"));
        }

        if let Some(bad) = self.dependencies.iter().find(|name| !valid_name(name)) {
            return Err(format!("зависимость «{bad}»: неверное имя"));
        }

        Ok(())
    }

    /// Имя файла `.wasm`.
    pub fn wasm_file(&self) -> String {
        self.main.clone().unwrap_or_else(|| format!("{}.wasm", self.name))
    }
}

/// Имя плагина: строчные латинские буквы, цифры, «-» и «_».
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Подходит ли запрошенная версия API: совпадают major и minor.
pub fn api_compatible(wanted: &str) -> bool {
    let key = |text: &str| -> Option<(u32, u32)> {
        let mut parts = text.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().map_or(Some(0), |minor| minor.parse().ok())?;

        Some((major, minor))
    };

    key(wanted).is_some() && key(wanted) == key(API_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
        name = "hello"
        version = "0.1.0"
        api = "0.1"
        author = "Аня"
        description = "Приветствие"
        dependencies = ["base"]
    "#;

    #[test]
    fn full_manifest_parses() {
        let manifest = Manifest::parse(GOOD).unwrap();

        assert_eq!(manifest.name, "hello");
        assert_eq!(manifest.dependencies, ["base"]);
        assert_eq!(manifest.wasm_file(), "hello.wasm");
    }

    #[test]
    fn optional_fields_may_be_missing() {
        let manifest = Manifest::parse("name = \"a\"\nversion = \"1\"\napi = \"0.1.3\"").unwrap();

        assert!(manifest.author.is_empty() && manifest.dependencies.is_empty());
    }

    #[test]
    fn bad_names_and_apis_are_refused() {
        assert!(Manifest::parse(&GOOD.replace("hello", "Hello World")).is_err());
        assert!(Manifest::parse(&GOOD.replace("hello", "../x")).is_err());
        assert!(Manifest::parse(&GOOD.replace("\"0.1\"", "\"0.2\"")).is_err());
        assert!(Manifest::parse(&GOOD.replace("\"0.1\"", "\"abc\"")).is_err());
        assert!(Manifest::parse("name = \"x\"").is_err());
    }

    #[test]
    fn main_must_stay_inside_the_folder() {
        assert!(Manifest::parse(&format!("{GOOD}\nmain = \"../evil.wasm\"")).is_err());
        assert!(Manifest::parse(&format!("{GOOD}\nmain = \"x.so\"")).is_err());
        assert_eq!(Manifest::parse(&format!("{GOOD}\nmain = \"core.wasm\"")).unwrap().wasm_file(), "core.wasm");
    }

    #[test]
    fn self_dependency_is_refused() {
        assert!(Manifest::parse(&GOOD.replace("[\"base\"]", "[\"hello\"]")).is_err());
    }
}
