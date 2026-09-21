// Права на сервере.
//
// Кто и что может — записано в файле `ops.json` рядом с миром, как у
// обычного сервера. Внутри список: ник, опознаватель игрока и уровень прав.
// Уровень по умолчанию берётся из настройки `op-permission-level`.
//
// Человек за консолью сервера — всегда с полными правами: он и так может
// остановить сервер.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::network::login::format_uuid;
use crate::{log_debug, log_warn};

/// Имя файла — как у оригинального сервера.
pub const FILE: &str = "ops.json";

/// Наибольший уровень прав.
pub const HIGHEST_LEVEL: i32 = 4;

/// Запись о том, у кого есть права.
#[derive(Serialize, Deserialize, Clone)]
pub struct Op {
    /// Опознаватель игрока в привычном виде 8-4-4-4-12.
    pub uuid: String,

    /// Ник — чтобы файл можно было читать и править глазами.
    pub name: String,

    /// Уровень прав: чем больше, тем больше можно.
    pub level: i32,
}

/// Список тех, у кого есть права.
pub struct Ops {
    path: PathBuf,
    list: Vec<Op>,
}

impl Ops {
    /// Читает файл прав. Нет файла — пустой список, это не ошибка:
    /// у нового сервера прав нет ни у кого.
    pub fn open(path: &Path) -> Self {
        let list = match fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Vec<Op>>(&text) {
                Ok(list) => {
                    log_debug!("Права: прочитано записей — {}", list.len());
                    list
                }
                Err(error) => {
                    log_warn!("Права: файл {} испорчен ({}) — считаем пустым", FILE, error);
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };

        Self {
            path: path.to_path_buf(),
            list,
        }
    }

    /// Уровень прав игрока. Нет в списке — ноль.
    pub fn level_of(&self, uuid: &[u8; 16]) -> i32 {
        let uuid = format_uuid(uuid);

        self.list
            .iter()
            .find(|op| op.uuid == uuid)
            .map(|op| op.level)
            .unwrap_or(0)
    }

    /// Выдаёт права. Возвращает false, если они уже были.
    pub fn give(&mut self, uuid: &[u8; 16], name: &str, level: i32) -> bool {
        let uuid = format_uuid(uuid);

        if let Some(op) = self.list.iter_mut().find(|op| op.uuid == uuid) {
            if op.level == level {
                return false;
            }

            op.level = level;
            op.name = name.to_string();
        } else {
            self.list.push(Op {
                uuid,
                name: name.to_string(),
                level,
            });
        }

        self.save();
        true
    }

    /// Снимает права по нику. Возвращает false, если их и не было.
    pub fn take_away(&mut self, name: &str) -> bool {
        let before = self.list.len();

        // Ник сравнивается без учёта регистра, и не только латинского:
        // в игре ники бывают какие угодно.
        let wanted = name.to_lowercase();

        self.list.retain(|op| op.name.to_lowercase() != wanted);

        if self.list.len() == before {
            return false;
        }

        self.save();
        true
    }

    /// Все, у кого есть права.
    pub fn names(&self) -> Vec<&str> {
        self.list.iter().map(|op| op.name.as_str()).collect()
    }

    /// Записывает список на диск. Ошибку записи прячем в лог: она не должна
    /// ронять сервер, но и молчать о ней нельзя.
    fn save(&self) {
        let text = match serde_json::to_string_pretty(&self.list) {
            Ok(text) => text,
            Err(error) => {
                log_warn!("Права: не удалось собрать {} ({})", FILE, error);
                return;
            }
        };

        if let Err(error) = fs::write(&self.path, text) {
            log_warn!("Права: не удалось записать {} ({})", FILE, error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Свой файл на каждую проверку: проверки идут разом, и общий файл
    /// они делили бы друг с другом.
    fn temporary(what: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-ops-{}-{}.json", std::process::id(), what));
        let _ = fs::remove_file(&path);
        path
    }

    /// Права выдаются, снимаются и переживают перезапуск: список читается
    /// из того же файла заново.
    #[test]
    fn rights_are_kept_in_the_file() {
        let path = temporary("kept");
        let uuid = [7u8; 16];

        let mut ops = Ops::open(&path);
        assert_eq!(ops.level_of(&uuid), 0, "права взялись из ниоткуда");

        assert!(ops.give(&uuid, "Игрок", HIGHEST_LEVEL));
        assert_eq!(ops.level_of(&uuid), HIGHEST_LEVEL);

        // Перезапуск: читаем тот же файл заново.
        let again = Ops::open(&path);
        assert_eq!(again.level_of(&uuid), HIGHEST_LEVEL, "права не сохранились");

        let mut ops = Ops::open(&path);
        assert!(ops.take_away("игрок"), "снятие по нику не сработало");
        assert_eq!(ops.level_of(&uuid), 0);
        assert!(!ops.take_away("Игрок"), "сняли то, чего нет");

        let _ = fs::remove_file(&path);
    }

    /// Испорченный файл не роняет сервер: список просто пустой.
    #[test]
    fn a_broken_file_is_taken_as_empty() {
        let path = temporary("broken");
        fs::write(&path, "не json вовсе").expect("временный файл пишется");

        let ops = Ops::open(&path);
        assert_eq!(ops.level_of(&[1u8; 16]), 0);
        assert!(ops.names().is_empty());

        let _ = fs::remove_file(&path);
    }
}
