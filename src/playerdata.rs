// Файлы игроков: где кто стоял и куда смотрел.
//
// Положение игрока — такая же часть мира, как блоки. Если сервер упадёт или
// его выключат, игрок должен вернуться туда, где стоял, а не в точку
// появления. Поэтому файл игрока пишется на диск сразу, как только игрок
// заметно сдвинулся, — тем же способом, каким сохраняется каждый
// поставленный блок.
//
// Файлов столько, сколько игроков заходило на сервер: у каждого своё имя —
// опознаватель игрока. Имя для этого не годится (его можно сменить), а
// опознаватель остаётся тем же. Так же устроено и в оригинале, где данные
// игроков лежат в директории мира, в поддиректории playerdata.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::inventory::Inventory;
use crate::{log_info, log_warn};

/// Подпись файла игрока: по ней сервер понимает, что файл его.
const MAGIC: &[u8; 4] = b"MCPD";

/// Версия формата файла. Меняется, если меняется раскладка записи, чтобы
/// старый файл не был прочитан неправильно.
///
/// Версия 1 — только место, поворот и режим игры. Версия 2 — то же самое
/// и инвентарь следом. Файлы первой версии читаются по-прежнему: игрок
/// встанет там, где вышел, просто с пустым инвентарём.
const FORMAT_VERSION: u32 = 2;

/// Прежняя версия формата, которую сервер ещё умеет читать.
const OLD_FORMAT_VERSION: u32 = 1;

/// Директория с файлами игроков внутри директории мира. Как в оригинале.
pub const DIRECTORY: &str = "world/playerdata";

/// Длина постоянной части файла: подпись, версия, три координаты, два угла
/// и режим игры. Дальше идёт инвентарь, а он разной длины.
const SIZE: usize = 4 + 4 + 8 * 3 + 4 * 2 + 4;

/// Что сервер помнит об игроке между заходами.
#[derive(Clone, PartialEq, Debug)]
pub struct PlayerData {
    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Поворот вокруг вертикали: куда игрок смотрит.
    pub yaw: f32,

    /// Наклон: вверх или вниз.
    pub pitch: f32,

    /// Режим игры — каким он был при выходе, таким и останется при заходе.
    pub game_mode: i32,

    /// Инвентарь — с чем игрок вышел, с тем и зайдёт.
    pub inventory: Inventory,
}

/// Путь к файлу игрока внутри директории.
///
/// Имя — опознаватель игрока, записанный шестнадцатеричными парами. Годится
/// любое имя файла, но такое удобно: его видно в директории и по нему сразу
/// понятно, чей это файл.
pub fn path_for(directory: &Path, uuid: &[u8; 16]) -> PathBuf {
    let mut name = String::with_capacity(32);

    for byte in uuid {
        name.push_str(&format!("{:02x}", byte));
    }

    directory.join(format!("{}.mcrw", name))
}

/// Читает данные игрока.
///
/// None означает, что файла нет или он не читается: тогда игрок заходит как
/// в первый раз. Ронять сервер из-за испорченного файла незачем — терять
/// при этом нечего, кроме положения одного игрока.
pub fn load(directory: &Path, uuid: &[u8; 16]) -> Option<PlayerData> {
    let path = path_for(directory, uuid);

    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        // Файла нет — игрок на сервере впервые.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            log_warn!("Не удалось прочитать данные игрока ({}): {}", path.display(), e);
            return None;
        }
    };

    match parse(&bytes) {
        Some(data) => {
            log_info!(
                "Положение игрока прочитано из {}: {:.2} {:.2} {:.2}",
                path.display(),
                data.x,
                data.y,
                data.z
            );
            Some(data)
        }
        None => {
            log_warn!(
                "Файл игрока {} не читается, игрок зайдёт как в первый раз",
                path.display()
            );
            None
        }
    }
}

/// Разбирает содержимое файла. None — файл не наш, другой версии или обрезан.
fn parse(bytes: &[u8]) -> Option<PlayerData> {
    if bytes.len() < SIZE || &bytes[..4] != MAGIC {
        return None;
    }

    let version = u32::from_be_bytes(take(bytes, 4)?);

    // Файл первой версии — без инвентаря, и длина у него постоянная.
    let inventory = match version {
        OLD_FORMAT_VERSION if bytes.len() == SIZE => Inventory::new(),
        FORMAT_VERSION => Inventory::from_bytes(bytes.get(SIZE..)?)?,
        _ => return None,
    };

    Some(PlayerData {
        x: f64::from_be_bytes(take(bytes, 8)?),
        y: f64::from_be_bytes(take(bytes, 16)?),
        z: f64::from_be_bytes(take(bytes, 24)?),
        yaw: f32::from_be_bytes(take(bytes, 32)?),
        pitch: f32::from_be_bytes(take(bytes, 36)?),
        game_mode: i32::from_be_bytes(take(bytes, 40)?),
        inventory,
    })
}

/// Забирает следующие N байт, начиная со смещения.
fn take<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..offset + N)?.try_into().ok()
}

/// Записывает данные игрока.
///
/// Пишем во временный файл и переименовываем — так же, как сохраняется мир:
/// если сервер упадёт посреди записи, на диске останется прежний целый файл,
/// а не половина нового.
pub fn save(directory: &Path, uuid: &[u8; 16], data: &PlayerData) -> io::Result<()> {
    let path = path_for(directory, uuid);

    let mut out = Vec::with_capacity(SIZE);

    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&data.x.to_be_bytes());
    out.extend_from_slice(&data.y.to_be_bytes());
    out.extend_from_slice(&data.z.to_be_bytes());
    out.extend_from_slice(&data.yaw.to_be_bytes());
    out.extend_from_slice(&data.pitch.to_be_bytes());
    out.extend_from_slice(&data.game_mode.to_be_bytes());
    out.extend_from_slice(&data.inventory.to_bytes());

    let temporary = path.with_extension("tmp");
    fs::write(&temporary, &out)?;
    fs::rename(&temporary, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Отдельная директория на каждый тест: тесты идут вразнобой, и в общей
    /// директории они бы мешали друг другу.
    fn test_directory(name: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("mc_playerdata_{}", name));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("создать директорию для теста");

        directory
    }

    fn sample() -> PlayerData {
        PlayerData {
            inventory: Inventory::new(),
            x: 12.5,
            y: 65.0,
            z: -3.25,
            yaw: 90.0,
            pitch: -12.0,
            game_mode: 1,
        }
    }

    /// Записанное положение читается обратно ровно таким же: иначе игрок
    /// после перезапуска оказывался бы не там, где стоял.
    #[test]
    fn saved_position_is_read_back() {
        let directory = test_directory("round_trip");

        save(&directory, &[1; 16], &sample()).expect("записать");
        assert_eq!(load(&directory, &[1; 16]), Some(sample()));
    }

    /// Файла нет — игрок заходит впервые, и это не ошибка.
    #[test]
    fn a_missing_file_means_a_new_player() {
        let directory = test_directory("missing");

        assert_eq!(load(&directory, &[2; 16]), None);
    }

    /// У каждого игрока свой файл: чужое положение ему не достаётся.
    #[test]
    fn players_do_not_share_files() {
        let directory = test_directory("separate");

        save(&directory, &[3; 16], &sample()).expect("записать первого");

        let other = PlayerData {
            x: -100.0,
            ..sample()
        };
        save(&directory, &[4; 16], &other).expect("записать второго");

        assert_eq!(load(&directory, &[3; 16]), Some(sample()));
        assert_eq!(load(&directory, &[4; 16]), Some(other));
        assert_eq!(load(&directory, &[5; 16]), None);
    }

    /// Испорченный файл не читается — игрок просто зайдёт как в первый раз.
    /// Важно, что сервер при этом не падает.
    #[test]
    fn a_broken_file_is_not_trusted() {
        let directory = test_directory("broken");

        let path = path_for(&directory, &[6; 16]);

        fs::write(&path, b"this is not a player file at all").expect("записать мусор");
        assert_eq!(load(&directory, &[6; 16]), None);

        // Наш файл, но обрезанный: читать его нельзя.
        let mut truncated = Vec::new();
        truncated.extend_from_slice(MAGIC);
        truncated.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        truncated.extend_from_slice(&1.0f64.to_be_bytes());

        fs::write(&path, &truncated).expect("записать обрезанный файл");
        assert_eq!(load(&directory, &[6; 16]), None);

        // Наш файл, но другой версии формата.
        let mut other_version = Vec::new();
        other_version.extend_from_slice(MAGIC);
        other_version.extend_from_slice(&(FORMAT_VERSION + 1).to_be_bytes());
        other_version.resize(SIZE, 0);

        fs::write(&path, &other_version).expect("записать файл другой версии");
        assert_eq!(load(&directory, &[6; 16]), None);
    }

    /// Имя файла — опознаватель игрока: два разных игрока не могут попасть
    /// в один файл.
    #[test]
    fn the_file_is_named_after_the_player() {
        let directory = Path::new("world/playerdata");

        let mut uuid = [0u8; 16];
        uuid[0] = 0xab;
        uuid[15] = 0x0f;

        let path = path_for(directory, &uuid);

        assert_eq!(
            path,
            Path::new("world/playerdata/ab00000000000000000000000000000f.mcrw")
        );

        assert_ne!(path, path_for(directory, &[0u8; 16]));
    }

    /// Инвентарь записывается вместе с положением и читается обратно таким же:
    /// иначе игрок терял бы вещи при каждом выходе.
    #[test]
    fn the_inventory_is_saved_with_the_player() {
        use crate::inventory::Stack;

        let directory = test_directory("inventory");

        let mut inventory = Inventory::new();
        inventory.set(crate::inventory::FIRST_HOTBAR, Some(Stack::new(1, 12)));
        inventory.set(crate::inventory::OFFHAND, Some(Stack::new(5, 1)));
        inventory.select(3);

        let data = PlayerData { inventory, ..sample() };

        save(&directory, &[7; 16], &data).expect("записать");
        assert_eq!(load(&directory, &[7; 16]), Some(data));
    }

    /// Файл прежней версии, без инвентаря, читается по-прежнему: игрок встанет
    /// там, где вышел, просто с пустыми руками.
    #[test]
    fn an_old_file_is_still_read() {
        let directory = test_directory("old_version");
        let path = path_for(&directory, &[8; 16]);

        let data = sample();

        let mut old = Vec::new();
        old.extend_from_slice(MAGIC);
        old.extend_from_slice(&OLD_FORMAT_VERSION.to_be_bytes());
        old.extend_from_slice(&data.x.to_be_bytes());
        old.extend_from_slice(&data.y.to_be_bytes());
        old.extend_from_slice(&data.z.to_be_bytes());
        old.extend_from_slice(&data.yaw.to_be_bytes());
        old.extend_from_slice(&data.pitch.to_be_bytes());
        old.extend_from_slice(&data.game_mode.to_be_bytes());

        fs::write(&path, &old).expect("записать старый файл");

        assert_eq!(load(&directory, &[8; 16]), Some(data));
    }
}
