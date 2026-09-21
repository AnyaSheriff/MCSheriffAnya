// Мир на диске: регионы.
//
// Хранить весь мир одним файлом нельзя: он переписывается целиком, и чем
// больше мир, тем дороже каждая запись. Поэтому мир разложен по регионам —
// так же, как в игре (minecraft.wiki, страница Region file format): один
// регион это квадрат 32×32 чанка, и лежит он отдельным файлом.
//
// Устройство файла тоже взято оттуда, потому что оно удачное: файл нарезан
// на куски по 4 КиБ («секторы»), а в самом начале лежит таблица — где чей
// чанк и сколько секторов занимает. Чтобы переписать один чанк, достаточно
// тронуть его секторы и одну строчку таблицы, а не весь файл.
//
// Содержимое самого чанка — своё, ничего общего с игрой: секции по порядку,
// а в каждой блоки записаны «состояние и сколько раз подряд». У ровной земли
// это несколько чисел на секцию вместо четырёх тысяч.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Сторона региона в чанках.
pub const REGION_SIZE: i32 = 32;

/// Размер сектора.
const SECTOR: usize = 4096;

/// Сколько секторов занимают таблицы в начале файла: где чей чанк и когда
/// он записан.
const HEADER_SECTORS: usize = 2;

/// Файл региона, в котором лежит этот чанк.
pub fn path_for(directory: &Path, chunk_x: i32, chunk_z: i32) -> PathBuf {
    let (region_x, region_z) = region_of(chunk_x, chunk_z);

    directory.join(format!("r.{}.{}.rcw", region_x, region_z))
}

/// В каком регионе лежит чанк.
pub fn region_of(chunk_x: i32, chunk_z: i32) -> (i32, i32) {
    (
        chunk_x.div_euclid(REGION_SIZE),
        chunk_z.div_euclid(REGION_SIZE),
    )
}

/// Место чанка в таблице региона.
fn slot_of(chunk_x: i32, chunk_z: i32) -> usize {
    let x = chunk_x.rem_euclid(REGION_SIZE) as usize;
    let z = chunk_z.rem_euclid(REGION_SIZE) as usize;

    x + z * REGION_SIZE as usize
}

/// Читает содержимое чанка. None — такого чанка в файле нет.
pub fn read(directory: &Path, chunk_x: i32, chunk_z: i32) -> io::Result<Option<Vec<u8>>> {
    let path = path_for(directory, chunk_x, chunk_z);

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    let mut header = vec![0u8; SECTOR];

    if file.read_exact(&mut header).is_err() {
        // Файл короче таблицы — значит, в нём ещё ничего нет.
        return Ok(None);
    }

    let (offset, sectors) = entry(&header, slot_of(chunk_x, chunk_z));

    if sectors == 0 {
        return Ok(None);
    }

    file.seek(SeekFrom::Start((offset * SECTOR) as u64))?;

    let mut length = [0u8; 4];
    file.read_exact(&mut length)?;

    let length = u32::from_be_bytes(length) as usize;

    if length > sectors * SECTOR {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "в таблице региона указана не та длина чанка",
        ));
    }

    let mut bytes = vec![0u8; length];
    file.read_exact(&mut bytes)?;

    Ok(Some(bytes))
}

/// Записывает содержимое чанка.
///
/// Если чанк вырос и в прежние секторы не помещается, он переносится в конец
/// файла. Освободившиеся секторы остаются пустыми: разбирать эти дыры
/// незачем, файл всё равно растёт медленно.
pub fn write(directory: &Path, chunk_x: i32, chunk_z: i32, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(directory)?;

    let path = path_for(directory, chunk_x, chunk_z);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;

    let mut header = vec![0u8; SECTOR * HEADER_SECTORS];

    // Новый файл начинается с пустых таблиц.
    if file.metadata()?.len() < (SECTOR * HEADER_SECTORS) as u64 {
        file.set_len((SECTOR * HEADER_SECTORS) as u64)?;
    } else {
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut header)?;
    }

    let slot = slot_of(chunk_x, chunk_z);
    let (old_offset, old_sectors) = entry(&header, slot);

    // Длина впереди, дальше сами данные, и всё это дополняется до целых
    // секторов: так каждый чанк начинается со своего сектора.
    let mut payload = Vec::with_capacity(bytes.len() + 4);
    payload.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    payload.extend_from_slice(bytes);

    let sectors = payload.len().div_ceil(SECTOR);
    payload.resize(sectors * SECTOR, 0);

    if sectors > u8::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "чанк не помещается в отведённые ему секторы",
        ));
    }

    // На прежнее место чанк ложится, только если помещается; иначе — в конец.
    let offset = if sectors <= old_sectors && old_offset >= HEADER_SECTORS {
        old_offset
    } else {
        let end = file.metadata()?.len() as usize;

        end.div_ceil(SECTOR).max(HEADER_SECTORS)
    };

    file.seek(SeekFrom::Start((offset * SECTOR) as u64))?;
    file.write_all(&payload)?;

    set_entry(&mut header, slot, offset, sectors);
    set_written(&mut header, slot);

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;

    file.sync_data()
}

/// Где лежит чанк и сколько секторов занимает.
fn entry(header: &[u8], slot: usize) -> (usize, usize) {
    let at = slot * 4;

    let offset = u32::from_be_bytes([0, header[at], header[at + 1], header[at + 2]]) as usize;
    let sectors = header[at + 3] as usize;

    (offset, sectors)
}

/// Записывает в таблицу, где теперь лежит чанк.
fn set_entry(header: &mut [u8], slot: usize, offset: usize, sectors: usize) {
    let at = slot * 4;
    let offset = (offset as u32).to_be_bytes();

    header[at] = offset[1];
    header[at + 1] = offset[2];
    header[at + 2] = offset[3];
    header[at + 3] = sectors as u8;
}

/// Записывает во вторую таблицу время записи чанка.
///
/// Серверу оно пока не нужно, но пусть будет: по нему видно, когда чанк
/// трогали в последний раз.
fn set_written(header: &mut [u8], slot: usize) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|passed| passed.as_secs() as u32)
        .unwrap_or(0);

    let at = SECTOR + slot * 4;

    header[at..at + 4].copy_from_slice(&now.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn directory(name: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("rustcraft_region_{}", name));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("создать директорию для проверки");

        directory
    }

    /// Записанный чанк читается обратно ровно таким же.
    #[test]
    fn a_chunk_is_read_back() {
        let directory = directory("round_trip");

        write(&directory, 3, 5, "содержимое".as_bytes()).expect("записать");

        assert_eq!(read(&directory, 3, 5).expect("прочитать"), Some("содержимое".as_bytes().to_vec()));
    }

    /// Чанков в регионе много, и они не мешают друг другу.
    #[test]
    fn chunks_do_not_overwrite_each_other() {
        let directory = directory("many");

        for number in 0..8u8 {
            write(&directory, number as i32, 1, &[number; 100]).expect("записать");
        }

        for number in 0..8u8 {
            assert_eq!(
                read(&directory, number as i32, 1).expect("прочитать"),
                Some(vec![number; 100])
            );
        }
    }

    /// Выросший чанк переезжает и остаётся целым, а соседний не портится.
    #[test]
    fn a_grown_chunk_moves_and_stays_whole() {
        let directory = directory("grow");

        write(&directory, 0, 0, &[1; 10]).expect("записать маленький");
        write(&directory, 1, 0, &[2; 10]).expect("записать соседний");
        write(&directory, 0, 0, &[3; SECTOR * 3]).expect("записать выросший");

        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some(vec![3; SECTOR * 3]));
        assert_eq!(read(&directory, 1, 0).expect("прочитать соседний"), Some(vec![2; 10]));
    }

    /// Чанки разных регионов лежат в разных файлах, а отрицательные
    /// координаты не путаются с положительными.
    #[test]
    fn regions_are_separate_files() {
        let directory = directory("split");

        assert_eq!(region_of(0, 0), (0, 0));
        assert_eq!(region_of(31, 31), (0, 0));
        assert_eq!(region_of(32, 0), (1, 0));
        assert_eq!(region_of(-1, -1), (-1, -1));

        assert_ne!(path_for(&directory, 0, 0), path_for(&directory, 32, 0));
        assert_ne!(slot_of(0, 0), slot_of(-1, -1));

        write(&directory, 0, 0, "здесь".as_bytes()).expect("записать");
        write(&directory, -1, -1, "там".as_bytes()).expect("записать в соседний регион");

        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some("здесь".as_bytes().to_vec()));
        assert_eq!(read(&directory, -1, -1).expect("прочитать"), Some("там".as_bytes().to_vec()));
    }

    /// Чанка, которого не записывали, в файле нет — и это не ошибка.
    #[test]
    fn a_missing_chunk_is_not_an_error() {
        let directory = directory("missing");

        assert_eq!(read(&directory, 7, 7).expect("прочитать"), None);

        write(&directory, 0, 0, "есть".as_bytes()).expect("записать");

        assert_eq!(read(&directory, 1, 1).expect("прочитать"), None);
    }
}
