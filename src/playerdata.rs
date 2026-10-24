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
///
/// Версия 3 — плюс измерение последним байтом. Версия 4 — плюс здоровье,
/// еда, насыщение, изнурение, воздух, огонь и высота падения: постоянный
/// хвост из `VITALS_SIZE` байт после измерения. Файлы версий 1–3 читаются:
/// игрок зайдёт сытым и здоровым.
///
/// Версия 6 — плюс семя зачарования перед точкой возрождения
/// (`XP_SEED_SIZE`). Файлы ранних версий читаются с семенем 0: оно
/// заведётся заново, как у оригинала («when joining a world with … seed of
/// 0, it will be regenerated», вики, Enchanting mechanics).
///
/// Версия 7 — плюс эндер-сундук игрока в самом конце: записи ячеек и после
/// них четыре байта длины записи (`write_ender`). Файлы версий 1–6 читаются
/// с пустым эндер-сундуком.
/// Версия 8 — после эндер-сундука сущности попугаев на плечах.
// Версия 9 — общий полученный опыт отдельно от очков шкалы (anvil-rename-repair).
// Версия 10 — способности (вики, Player.dat_format: «abilities»).
const FORMAT_VERSION: u32 = 10;
const TOTAL_XP_VERSION: u32 = 9;
const ABILITIES_SIZE: usize = 13;
const SHOULDERS_VERSION: u32 = 8;
const ENDER_VERSION: u32 = 7;

/// Версия с семенем зачарования, но без эндер-сундука.
const XP_SEED_VERSION: u32 = 6;

/// Версия 5 — плюс личная точка возрождения (кровать, якорь, `spawnpoint`):
/// постоянный хвост `bed::RespawnPoint::SIZE` после здоровья.
const RESPAWN_VERSION: u32 = 5;

/// Версия со здоровьем, но без точки возрождения.
const VITALS_VERSION: u32 = 4;

/// Семя зачарования: четыре байта после здоровья.
const XP_SEED_SIZE: usize = 4;

/// Версия с измерением, но без здоровья.
const DIMENSION_VERSION: u32 = 3;

/// Длина записи о здоровье: здоровье, еда, насыщение, изнурение, воздух,
/// огонь, высота падения, очки опыта — по четыре байта.
const VITALS_SIZE: usize = 8 * 4;

/// Версия с инвентарём, но без измерения: такой игрок — в обычном мире.
const INVENTORY_VERSION: u32 = 2;

/// Прежняя версия формата, которую сервер ещё умеет читать.
const OLD_FORMAT_VERSION: u32 = 1;

/// Директория с файлами игроков внутри директории мира. Как в оригинале.
pub const DIRECTORY: &str = "world/playerdata";

/// Длина постоянной части файла: подпись, версия, три координаты, два угла
/// и режим игры. Дальше идёт инвентарь, а он разной длины.
const SIZE: usize = 4 + 4 + 8 * 3 + 4 * 2 + 4;

/// Здоровье игрока, каким оно ложится в файл.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SavedVitals {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub air: i32,
    /// Сколько тактов ещё горит.
    pub fire: i32,
    pub fall: f32,
    /// Всего очков опыта.
    pub experience: i32,
    pub experience_total: i32,
    /// Семя зачарования (`XpSeed`).
    pub xp_seed: i32,
}

impl Default for SavedVitals {
    /// Новорождённый: полное здоровье и еда, насыщение 5, воздуха 300.
    fn default() -> Self {
        Self { health: 20.0, food: 20, saturation: 5.0, exhaustion: 0.0, air: 300, fire: 0, fall: 0.0, experience: 0, experience_total: 0, xp_seed: 0 }
    }
}

impl SavedVitals {
    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.health.to_be_bytes());
        out.extend_from_slice(&self.food.to_be_bytes());
        out.extend_from_slice(&self.saturation.to_be_bytes());
        out.extend_from_slice(&self.exhaustion.to_be_bytes());
        out.extend_from_slice(&self.air.to_be_bytes());
        out.extend_from_slice(&self.fire.to_be_bytes());
        out.extend_from_slice(&self.fall.to_be_bytes());
        out.extend_from_slice(&self.experience.to_be_bytes());
    }

    /// None — запись не той длины или числа не годятся (не число, здоровье
    /// вне 0…1024, еда вне 0…20): такому файлу не верим.
    fn read(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != VITALS_SIZE {
            return None;
        }

        let vitals = Self {
            health: f32::from_be_bytes(take(bytes, 0)?),
            food: i32::from_be_bytes(take(bytes, 4)?),
            saturation: f32::from_be_bytes(take(bytes, 8)?),
            exhaustion: f32::from_be_bytes(take(bytes, 12)?),
            air: i32::from_be_bytes(take(bytes, 16)?),
            fire: i32::from_be_bytes(take(bytes, 20)?),
            fall: f32::from_be_bytes(take(bytes, 24)?),
            experience: i32::from_be_bytes(take(bytes, 28)?),
            experience_total: i32::from_be_bytes(take(bytes, 28)?),
            xp_seed: 0,
        };

        let sane = vitals.health.is_finite()
            && (0.0..=1024.0).contains(&vitals.health)
            && (0..=20).contains(&vitals.food)
            && vitals.saturation.is_finite()
            && vitals.saturation >= 0.0
            && vitals.exhaustion.is_finite()
            && vitals.exhaustion >= 0.0
            && vitals.fall.is_finite()
            && vitals.fall >= 0.0
            && (-20..=300).contains(&vitals.air)
            && (0..=100_000).contains(&vitals.fire)
            && vitals.experience >= 0;

        sane.then_some(vitals)
    }
}

/// Способности из minecraft.wiki/w/Player.dat_format: «abilities», «flying»,
/// «mayfly», «instabuild», «invulnerable», «mayBuild», «flySpeed», «walkSpeed».
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Abilities {
    pub flying: bool,
    pub mayfly: bool,
    pub instabuild: bool,
    pub invulnerable: bool,
    pub may_build: bool,
    pub fly_speed: f32,
    pub walk_speed: f32,
}

impl Default for Abilities {
    fn default() -> Self {
        Self { flying: false, mayfly: false, instabuild: false, invulnerable: false,
            may_build: true, fly_speed: 0.05, walk_speed: 0.1 }
    }
}

impl Abilities {
    pub fn for_mode(mode: i32) -> Self {
        let mut abilities = Self::default();
        abilities.apply_mode(mode);
        abilities
    }

    /// Вики, Creative: «flight»; Spectator: «always flying».
    pub fn apply_mode(&mut self, mode: i32) {
        self.mayfly = matches!(mode, 1 | 3);
        self.instabuild = mode == 1;
        self.invulnerable = matches!(mode, 1 | 3);
        self.may_build = matches!(mode, 0 | 1);
        self.flying = self.mayfly && (self.flying || mode == 3);
    }

    pub fn client_flying(&mut self, flags: u8) {
        if self.mayfly { self.flying = flags & 2 != 0; }
    }

    pub fn flags(self) -> u8 {
        u8::from(self.invulnerable) | (u8::from(self.flying) << 1)
            | (u8::from(self.mayfly) << 2) | (u8::from(self.instabuild) << 3)
    }

    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&[u8::from(self.flying), u8::from(self.mayfly),
            u8::from(self.instabuild), u8::from(self.invulnerable), u8::from(self.may_build)]);
        out.extend_from_slice(&self.fly_speed.to_be_bytes());
        out.extend_from_slice(&self.walk_speed.to_be_bytes());
    }

    fn read(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != ABILITIES_SIZE || bytes[..5].iter().any(|b| *b > 1) { return None; }
        let value = Self { flying: bytes[0] != 0, mayfly: bytes[1] != 0,
            instabuild: bytes[2] != 0, invulnerable: bytes[3] != 0, may_build: bytes[4] != 0,
            fly_speed: f32::from_be_bytes(take(bytes, 5)?), walk_speed: f32::from_be_bytes(take(bytes, 9)?) };
        (value.fly_speed.is_finite() && value.walk_speed.is_finite()).then_some(value)
    }
}

/// Что сервер помнит об игроке между заходами.
#[derive(Clone, PartialEq, Debug)]
pub struct PlayerData {
    pub abilities: Abilities,
    /// Сущности попугаев на плечах: независимые записи Mob.
    pub shoulders: [Option<Vec<u8>>; 2],
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

    /// В каком измерении игрок вышел. Записан последним байтом файла.
    pub dimension: crate::dimension::Dimension,

    /// Здоровье, еда, воздух, огонь: хвост файла версии 4.
    pub vitals: SavedVitals,

    /// Личная точка возрождения: хвост файла версии 5.
    pub respawn: Option<crate::bed::RespawnPoint>,

    /// Эндер-сундук игрока, 27 ячеек: хвост файла версии 7.
    pub ender: Vec<Option<crate::inventory::Stack>>,
}

/// Ячеек в эндер-сундуке.
pub const ENDER_SLOTS: usize = 27;

/// Пустой эндер-сундук.
pub fn empty_ender() -> Vec<Option<crate::inventory::Stack>> {
    vec![None; ENDER_SLOTS]
}

/// Записывает эндер-сундук: число занятых ячеек, у каждой номер, предмет,
/// число и довески; в конце — длина записи, чтобы читать файл с хвоста.
fn write_ender(slots: &[Option<crate::inventory::Stack>], out: &mut Vec<u8>) {
    let start = out.len();
    let filled: Vec<_> = slots.iter().enumerate().filter_map(|(slot, stack)| stack.map(|stack| (slot, stack))).collect();

    out.push(filled.len() as u8);

    for (slot, stack) in filled {
        out.push(slot as u8);
        out.extend_from_slice(&stack.stored_item().to_be_bytes());
        out.extend_from_slice(&stack.count.to_be_bytes());
        stack.write_extras(out);
    }

    let length = (out.len() - start) as u32;

    out.extend_from_slice(&length.to_be_bytes());
}

/// Читает эндер-сундук из записи без хвоста длины. None — запись испорчена.
fn read_ender(bytes: &[u8]) -> Option<Vec<Option<crate::inventory::Stack>>> {
    use crate::inventory::Stack;

    let mut slots = empty_ender();
    let mut at = 1;

    for _ in 0..*bytes.first()? {
        let mut take = |count: usize| -> Option<&[u8]> {
            let part = bytes.get(at..at + count)?;

            at += count;

            Some(part)
        };
        let slot = usize::from(take(1)?[0]);
        let item = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let count = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let extras = Stack::read_extras(item, &mut take)?;
        let mut stack = Stack::from_stored(item, count);

        if let Some(extras) = extras {
            stack = stack.with_extras(extras);
        }

        if count > 0 && crate::blocks::stack_size(stack.item) > 0 {
            *slots.get_mut(slot)? = Some(stack.with_count(count.min(crate::blocks::stack_size(stack.item))));
        }
    }

    Some(slots)
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

    // Файл первой версии — без инвентаря, и длина у него постоянная;
    // второй — без измерения; в третьей измерение — последний байт.
    let mut ender = empty_ender();
    let mut shoulders = [None,None];
    let mut abilities = Abilities::for_mode(i32::from_be_bytes(take(bytes, 40)?));
    let (inventory, dimension, vitals, respawn) = match version {
        OLD_FORMAT_VERSION if bytes.len() == SIZE => (Inventory::new(), Default::default(), Default::default(), None),
        INVENTORY_VERSION => (Inventory::from_bytes(bytes.get(SIZE..)?)?, Default::default(), Default::default(), None),
        DIMENSION_VERSION => {
            let (last, inventory) = bytes.get(SIZE..)?.split_last()?;
            let dimension = *crate::dimension::Dimension::ALL.get(*last as usize)?;

            (Inventory::from_bytes(inventory)?, dimension, Default::default(), None)
        }
        VITALS_VERSION | RESPAWN_VERSION | XP_SEED_VERSION | ENDER_VERSION | SHOULDERS_VERSION | TOTAL_XP_VERSION | FORMAT_VERSION => {
            let mut tail = bytes.get(SIZE..)?;
            let mut respawn = None;
            let mut seed = 0;

            if version >= FORMAT_VERSION {
                let (rest, stored) = tail.split_at(tail.len().checked_sub(ABILITIES_SIZE)?);
                abilities = Abilities::read(stored)?;
                tail = rest;
            }
            let total = if version >= TOTAL_XP_VERSION {
                let (rest, bytes) = tail.split_at(tail.len().checked_sub(4)?);
                let total = i32::from_be_bytes(bytes.try_into().ok()?);
                if total < 0 { return None; }
                tail = rest;
                Some(total)
            } else { None };
            if version >= SHOULDERS_VERSION {
                let (rest,length) = tail.split_at(tail.len().checked_sub(4)?);
                let length = u32::from_be_bytes(length.try_into().ok()?) as usize;
                let (rest,stored) = rest.split_at(rest.len().checked_sub(length)?);
                shoulders = serde_json::from_slice(stored).ok()?;
                tail = rest;
            }
            // Эндер-сундук — самый конец файла: длина записи и сама запись.
            if version >= ENDER_VERSION {
                let (rest, length) = tail.split_at(tail.len().checked_sub(4)?);
                let length = u32::from_be_bytes(length.try_into().ok()?) as usize;
                let (rest, stored) = rest.split_at(rest.len().checked_sub(length)?);

                ender = read_ender(stored)?;
                tail = rest;
            }

            if version >= RESPAWN_VERSION {
                let (rest, point) = tail.split_at(tail.len().checked_sub(crate::bed::RespawnPoint::SIZE)?);

                respawn = crate::bed::RespawnPoint::read(point)?;
                tail = rest;
            }

            if version >= XP_SEED_VERSION {
                let (rest, bytes) = tail.split_at(tail.len().checked_sub(XP_SEED_SIZE)?);

                seed = i32::from_be_bytes(bytes.try_into().ok()?);
                tail = rest;
            }

            let (rest, vitals) = tail.split_at(tail.len().checked_sub(VITALS_SIZE)?);
            let (last, inventory) = rest.split_last()?;
            let dimension = *crate::dimension::Dimension::ALL.get(*last as usize)?;
            let mut vitals = SavedVitals { xp_seed: seed, ..SavedVitals::read(vitals)? };
            if let Some(total) = total { vitals.experience_total = total; }

            (Inventory::from_bytes(inventory)?, dimension, vitals, respawn)
        }
        _ => return None,
    };

    Some(PlayerData {
        abilities,
        shoulders,
        x: f64::from_be_bytes(take(bytes, 8)?),
        y: f64::from_be_bytes(take(bytes, 16)?),
        z: f64::from_be_bytes(take(bytes, 24)?),
        yaw: f32::from_be_bytes(take(bytes, 32)?),
        pitch: f32::from_be_bytes(take(bytes, 36)?),
        game_mode: i32::from_be_bytes(take(bytes, 40)?),
        inventory,
        dimension,
        vitals,
        respawn,
        ender,
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
    let out = encode(data)?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, &out)?;
    fs::rename(&temporary, &path)
}

fn encode(data: &PlayerData) -> io::Result<Vec<u8>> {
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
    out.push(data.dimension.index() as u8);
    data.vitals.write(&mut out);
    out.extend_from_slice(&data.vitals.xp_seed.to_be_bytes());
    crate::bed::RespawnPoint::write(data.respawn.as_ref(), &mut out);
    write_ender(&data.ender, &mut out);
    let shoulders = serde_json::to_vec(&data.shoulders)?;
    out.extend_from_slice(&shoulders);
    out.extend_from_slice(&(shoulders.len() as u32).to_be_bytes());
    out.extend_from_slice(&data.vitals.experience_total.to_be_bytes());
    data.abilities.write(&mut out);

    Ok(out)
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
            abilities: Abilities::for_mode(1),
            shoulders: [None,None],
            inventory: Inventory::new(),
            x: 12.5,
            y: 65.0,
            z: -3.25,
            yaw: 90.0,
            pitch: -12.0,
            game_mode: 1,
            dimension: Default::default(),
            vitals: Default::default(),
            respawn: None,
            ender: empty_ender(),
        }
    }

    #[test]
    fn earned_experience_survives_level_spending_and_version_eight_is_readable() {
        // anvil-rename-repair: опыт шкалы и общий опыт теперь независимы.
        let mut data = sample();
        data.vitals.experience = 1288;
        data.vitals.experience_total = 0;
        let mut bytes = encode(&data).unwrap();
        assert_eq!(parse(&bytes), Some(data.clone()));
        bytes.truncate(bytes.len() - ABILITIES_SIZE - 4);
        bytes[4..8].copy_from_slice(&SHOULDERS_VERSION.to_be_bytes());
        data.vitals.experience_total = data.vitals.experience;
        assert_eq!(parse(&bytes), Some(data));
    }

    #[test]
    fn abilities_round_trip_all_seven_fields_for_every_mode() {
        for mode in 0..=3 {
            let mut data = sample();
            data.game_mode = mode;
            data.abilities = Abilities { flying: true, mayfly: true, instabuild: true,
                invulnerable: true, may_build: false, fly_speed: 0.123, walk_speed: 0.234 };
            assert_eq!(parse(&encode(&data).unwrap()), Some(data.clone()));
            data.abilities = Abilities { flying: false, mayfly: false, instabuild: false,
                invulnerable: false, may_build: true, fly_speed: 0.02, walk_speed: 0.03 };
            assert_eq!(parse(&encode(&data).unwrap()), Some(data));
        }
    }

    #[test]
    fn version_nine_restores_mode_abilities_without_flight_and_preserves_total_xp() {
        for mode in 0..=3 {
            let mut data = sample();
            data.game_mode = mode;
            data.vitals.experience_total = 42;
            let mut bytes = encode(&data).unwrap();
            bytes.truncate(bytes.len() - ABILITIES_SIZE);
            bytes[4..8].copy_from_slice(&TOTAL_XP_VERSION.to_be_bytes());
            data.abilities = Abilities::for_mode(mode);
            assert_eq!(parse(&bytes), Some(data));
        }
    }

    #[test]
    fn corrupt_or_truncated_abilities_are_rejected() {
        let bytes = encode(&sample()).unwrap();
        for missing in 1..=ABILITIES_SIZE {
            assert!(parse(&bytes[..bytes.len() - missing]).is_none());
        }
        let start = bytes.len() - ABILITIES_SIZE;
        for flag in 0..5 {
            let mut bad = bytes.clone();
            bad[start + flag] = 2;
            assert!(parse(&bad).is_none());
        }
        for offset in [5, 9] {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut bad = bytes.clone();
                bad[start + offset..start + offset + 4].copy_from_slice(&value.to_be_bytes());
                assert!(parse(&bad).is_none());
            }
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

    /// Эндер-сундук переживает файл: ячейки с довесками читаются такими же,
    /// а файл прежней версии читается с пустым сундуком.
    #[test]
    fn the_ender_chest_is_saved_with_the_player() {
        let directory = test_directory("ender");
        let stone = crate::blocks::item_named("stone").expect("камень");
        let mut data = sample();

        data.ender[0] = Some(crate::inventory::Stack::new(stone, 12));
        data.ender[26] = Some(crate::inventory::Stack::new(stone, 64));
        save(&directory, &[8; 16], &data).expect("записать");
        assert_eq!(load(&directory, &[8; 16]), Some(data));

        // Прежняя версия (6): тот же файл без хвоста эндер-сундука.
        let mut old = fs::read(path_for(&directory, &[8; 16])).expect("прочитать");
        old.truncate(old.len() - ABILITIES_SIZE - 4); // общий опыт версии 9
        let tail = u32::from_be_bytes(old[old.len() - 4..].try_into().expect("длина")) as usize;

        old.truncate(old.len() - 4 - tail);
        let tail = u32::from_be_bytes(old[old.len()-4..].try_into().expect("длина сундука")) as usize;
        old.truncate(old.len()-4-tail);
        old[4..8].copy_from_slice(&XP_SEED_VERSION.to_be_bytes());
        fs::write(path_for(&directory, &[9; 16]), &old).expect("записать");
        assert_eq!(load(&directory, &[9; 16]).map(|loaded| loaded.ender), Some(empty_ender()));
    }

    /// Версии 7 и 8 разбираются вместе с переменным хвостом инвентаря.
    #[test]
    fn shoulders_round_trip_and_version_seven_still_loads() {
        let uuid=[9;16];
        let mut data=sample(); let mut parrot=crate::mob::Mob::new(1,crate::mob::Kind::Parrot,(0.0,1.0,0.0));
        parrot.small.variant=4; parrot.tame.owner=Some(uuid);
        let mut stored=Vec::new(); parrot.write(&mut stored); data.shoulders[0]=Some(stored);
        let mut old=encode(&data).unwrap(); assert_eq!(parse(&old),Some(data.clone()));
        old.truncate(old.len() - ABILITIES_SIZE - 4);
        let length=u32::from_be_bytes(old[old.len()-4..].try_into().unwrap()) as usize;
        old.truncate(old.len()-4-length); old[4..8].copy_from_slice(&ENDER_VERSION.to_be_bytes());
        data.shoulders=[None,None];
        assert_eq!(parse(&old),Some(data));
    }

    /// Старые раскладки строим напрямую: новый писатель не должен менять
    /// проверку совместимости при добавлении очередного хвоста файла.
    #[test]
    fn all_legacy_versions_preserve_their_saved_fields() {
        let mut data = sample();
        let stone = crate::blocks::item_named("stone").expect("камень");

        data.inventory.set(crate::inventory::FIRST_HOTBAR, Some(crate::inventory::Stack::new(stone, 12)));
        data.inventory.select(3);
        data.dimension = crate::dimension::Dimension::Nether;
        data.vitals = SavedVitals { health: 7.5, food: 9, saturation: 0.0, exhaustion: 2.25, air: 120, fire: 60, fall: 4.5, experience: 1234, experience_total: 1234, xp_seed: -77 };
        data.respawn = Some(crate::bed::RespawnPoint { dimension: crate::dimension::Dimension::Nether, pos: (3, 70, -9), yaw: -90.0, forced: false });
        data.ender[26] = Some(crate::inventory::Stack::new(stone, 64));

        for version in OLD_FORMAT_VERSION..=ENDER_VERSION {
            let mut bytes = Vec::new();
            let mut expected = sample();

            bytes.extend_from_slice(MAGIC);
            bytes.extend_from_slice(&version.to_be_bytes());
            bytes.extend_from_slice(&data.x.to_be_bytes());
            bytes.extend_from_slice(&data.y.to_be_bytes());
            bytes.extend_from_slice(&data.z.to_be_bytes());
            bytes.extend_from_slice(&data.yaw.to_be_bytes());
            bytes.extend_from_slice(&data.pitch.to_be_bytes());
            bytes.extend_from_slice(&data.game_mode.to_be_bytes());

            if version >= INVENTORY_VERSION {
                bytes.extend_from_slice(&data.inventory.to_bytes());
                expected.inventory = data.inventory.clone();
            }
            if version >= DIMENSION_VERSION {
                bytes.push(data.dimension.index() as u8);
                expected.dimension = data.dimension;
            }
            if version >= VITALS_VERSION {
                data.vitals.write(&mut bytes);
                expected.vitals = SavedVitals { xp_seed: 0, ..data.vitals };
            }
            if version >= XP_SEED_VERSION {
                bytes.extend_from_slice(&data.vitals.xp_seed.to_be_bytes());
                expected.vitals.xp_seed = data.vitals.xp_seed;
            }
            if version >= RESPAWN_VERSION {
                crate::bed::RespawnPoint::write(data.respawn.as_ref(), &mut bytes);
                expected.respawn = data.respawn;
            }
            if version >= ENDER_VERSION {
                write_ender(&data.ender, &mut bytes);
                expected.ender = data.ender.clone();
            }

            assert_eq!(parse(&bytes), Some(expected), "версия {version}");
        }
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
    fn custom_tnt_is_saved_with_the_player() {
        let directory = test_directory("custom-tnt");
        let uuid = [0x5a; 16];
        let mut data = sample();
        let cluster = crate::custom_tnt::by_name("cluster").expect("cluster");
        let stack = crate::custom_tnt::stack(cluster, 7).expect("динамит");

        data.inventory.set(crate::inventory::FIRST_HOTBAR + 2, Some(stack));
        save(&directory, &uuid, &data).expect("записать");

        let read = load(&directory, &uuid).expect("прочитать");
        assert_eq!(read.inventory.slot(crate::inventory::FIRST_HOTBAR + 2), Some(stack));

        let _ = fs::remove_dir_all(&directory);
    }

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

    /// Здоровье, еда, воздух, огонь и падение переживают перезапуск.
    #[test]
    fn vitals_are_saved_with_the_player() {
        let directory = test_directory("vitals");
        let data = PlayerData {
            vitals: SavedVitals { health: 7.5, food: 9, saturation: 0.0, exhaustion: 2.25, air: 120, fire: 60, fall: 4.5, experience: 1234, experience_total: 1234, xp_seed: -77 },
            ..sample()
        };

        save(&directory, &[9; 16], &data).expect("записать");
        assert_eq!(load(&directory, &[9; 16]), Some(data));
    }

    /// Точка возрождения (кровать в Незере — якорь) переживает перезапуск.
    #[test]
    fn the_respawn_point_is_saved_with_the_player() {
        let directory = test_directory("respawn");
        let data = PlayerData {
            respawn: Some(crate::bed::RespawnPoint { dimension: crate::dimension::Dimension::Nether, pos: (3, 70, -9), yaw: -90.0, forced: false }),
            ..sample()
        };

        save(&directory, &[11; 16], &data).expect("записать");
        assert_eq!(load(&directory, &[11; 16]), Some(data));
    }

    /// Файл четвёртой версии (без точки возрождения) читается: точки нет.
    #[test]
    fn a_version_four_file_has_no_respawn_point() {
        let directory = test_directory("v4");
        let data = sample();

        save(&directory, &[12; 16], &data).expect("записать");

        let path = path_for(&directory, &[12; 16]);
        let mut bytes = fs::read(&path).expect("прочитать");

        bytes.truncate(bytes.len() - ABILITIES_SIZE - 4); // общий опыт версии 9
        // Сначала убираем попугаев и длину их записи, добавленные в версии 8.
        let shoulders = u32::from_be_bytes(bytes[bytes.len() - 4..].try_into().expect("длина попугаев")) as usize;
        bytes.truncate(bytes.len() - 4 - shoulders);

        // Четвёртая версия — без семени зачарования и точки возрождения:
        // семя читается нулём и заведётся заново.
        // Пустой эндер-сундук — число ячеек и длина записи: пять байт.
        bytes.truncate(bytes.len() - 5 - crate::bed::RespawnPoint::SIZE - XP_SEED_SIZE);
        bytes[4..8].copy_from_slice(&VITALS_VERSION.to_be_bytes());
        fs::write(&path, &bytes).expect("записать");

        let mut expected = data;

        expected.vitals.xp_seed = 0;
        expected.respawn = None;
        assert_eq!(load(&directory, &[12; 16]), Some(expected));
    }

    /// Файл третьей версии (без здоровья) читается: игрок сыт и здоров.
    #[test]
    fn a_version_three_file_gives_a_full_player() {
        let directory = test_directory("v3");
        let data = sample();
        let mut old = Vec::new();

        old.extend_from_slice(MAGIC);
        old.extend_from_slice(&DIMENSION_VERSION.to_be_bytes());
        old.extend_from_slice(&data.x.to_be_bytes());
        old.extend_from_slice(&data.y.to_be_bytes());
        old.extend_from_slice(&data.z.to_be_bytes());
        old.extend_from_slice(&data.yaw.to_be_bytes());
        old.extend_from_slice(&data.pitch.to_be_bytes());
        old.extend_from_slice(&data.game_mode.to_be_bytes());
        old.extend_from_slice(&data.inventory.to_bytes());
        old.push(data.dimension.index() as u8);

        fs::write(path_for(&directory, &[10; 16]), &old).expect("записать");

        let read = load(&directory, &[10; 16]).expect("прочитать");

        assert_eq!(read.vitals, SavedVitals::default());
        assert_eq!(read, data);
    }

    /// Здоровье из невозможных чисел файлу не верится.
    #[test]
    fn nonsense_vitals_are_rejected() {
        let mut bytes = Vec::new();

        SavedVitals { health: f32::NAN, ..Default::default() }.write(&mut bytes);
        assert_eq!(SavedVitals::read(&bytes), None);

        bytes.clear();
        SavedVitals { food: 99, ..Default::default() }.write(&mut bytes);
        assert_eq!(SavedVitals::read(&bytes), None);

        bytes.clear();
        SavedVitals::default().write(&mut bytes);
        assert_eq!(SavedVitals::read(&bytes), Some(SavedVitals::default()));
        assert_eq!(SavedVitals::read(&bytes[1..]), None);
    }
}
