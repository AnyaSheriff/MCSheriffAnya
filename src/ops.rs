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
use crate::{log_debug, log_info, log_warn};

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
                    // Испорченный файл не затираем первой же командой op:
                    // откладываем в сторону, чтобы его можно было починить.
                    let aside = path.with_extension("json.broken");
                    let _ = fs::rename(path, &aside);
                    log_warn!(
                        "Права: файл {} испорчен ({}) — отложен в {}, пока прав ни у кого нет",
                        FILE,
                        error,
                        aside.display()
                    );
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

    /// UUID защищённого имени; только проверенный владелец может завести
    /// такой аккаунт из игры. Права по имени этот метод не выдаёт.
    pub fn uuid_for_name(&self, name: &str) -> Option<[u8; 16]> {
        let op = self.list.iter().find(|op| op.name.eq_ignore_ascii_case(name))?;
        let hex = op.uuid.replace('-', "");
        if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) { return Some([0; 16]); }
        let mut uuid = [0; 16];
        for (i, byte) in uuid.iter_mut().enumerate() { *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?; }
        Some(uuid)
    }

    /// Выдаёт права. Возвращает false, если они уже были.
    pub fn give(&mut self, uuid: &[u8; 16], name: &str, level: i32) -> bool {
        let uuid = format_uuid(uuid);

        if let Some(op) = self.list.iter_mut().find(|op| op.uuid == uuid) {
            if op.level == level && op.name == name {
                return false;
            }

            let changed = op.level != level;
            op.level = level;
            op.name = name.to_string();

            // Сменился только ник — запомнить его, но права не «новые».
            if !changed {
                self.save();
                return false;
            }
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

    /// Снимает права по опознавателю (если игрок в сети) или по нику из
    /// файла. Возвращает false, если их и не было.
    pub fn take_away(&mut self, uuid: Option<&[u8; 16]>, name: &str) -> bool {
        let before = self.list.len();

        // Ник сравнивается без учёта регистра, и не только латинского:
        // в игре ники бывают какие угодно.
        let wanted = name.to_lowercase();
        let uuid = uuid.map(format_uuid);

        self.list.retain(|op| op.name.to_lowercase() != wanted && Some(&op.uuid) != uuid.as_ref());

        if self.list.len() == before {
            return false;
        }

        self.save();
        true
    }

    /// Переносит права на новый опознаватель того же ника. Раньше сервер
    /// верил опознавателю, который присылает клиент, и в файле могли остаться
    /// записи с ним; теперь опознаватель считается из ника, и старую запись
    /// с тем же ником надо переписать, иначе права пропадут. Возвращает true,
    /// если что-то перенесено.
    pub fn adopt(&mut self, uuid: &[u8; 16], name: &str) -> bool {
        let uuid = format_uuid(uuid);

        if self.list.iter().any(|op| op.uuid == uuid) {
            return false;
        }

        let Some(op) = self
            .list
            .iter_mut()
            .find(|op| op.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };

        log_info!("Права: {} перенесены на опознаватель {} (был {})", name, uuid, op.uuid);
        op.uuid = uuid;
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

        // Через временный файл: сбой посреди записи не оставит файл пустым.
        let temporary = self.path.with_extension("json.tmp");

        if let Err(error) = fs::write(&temporary, text).and_then(|_| fs::rename(&temporary, &self.path)) {
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
        path.push(format!("mcsheriffanya-ops-{}-{}.json", std::process::id(), what));
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
        assert!(ops.take_away(None, "игрок"), "снятие по нику не сработало");
        assert_eq!(ops.level_of(&uuid), 0);
        assert!(!ops.take_away(None, "Игрок"), "сняли то, чего нет");

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
