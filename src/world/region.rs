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

/// Пометка в старшем бите длины: за длиной идёт контрольная сумма записи.
///
/// Прежние записи суммы не имеют, и старший бит у них всегда пуст: чанк не
/// бывает больше 255 секторов, до двух гигабайт ему далеко. Поэтому старые
/// файлы читаются как раньше, а новые записи проверяются.
const CHECKED: u32 = 0x8000_0000;

/// Что нашлось в файле региона на месте чанка.
#[derive(Debug, PartialEq)]
pub enum Stored {
    /// Чанка в файле нет.
    Missing,

    /// Чанк на месте, и запись цела.
    Found(Vec<u8>),

    /// Запись есть, но испорчена: не сходится длина или сумма, файл
    /// обрывается. `raw` — всё, что удалось прочитать с её места, чтобы
    /// отложить в сторону, а не потерять.
    Damaged { raw: Vec<u8>, why: &'static str },
}

/// Читает содержимое чанка. None — такого чанка в файле нет.
///
/// Испорченная запись — ошибка `InvalidData`. Кому важно отличить порчу от
/// сбоя чтения и сберечь испорченные байты, зовёт `read_stored`.
#[cfg(test)]
pub fn read(directory: &Path, chunk_x: i32, chunk_z: i32) -> io::Result<Option<Vec<u8>>> {
    match read_stored(directory, chunk_x, chunk_z)? {
        Stored::Missing => Ok(None),
        Stored::Found(bytes) => Ok(Some(bytes)),
        Stored::Damaged { why, .. } => Err(io::Error::new(io::ErrorKind::InvalidData, why)),
    }
}

/// Читает запись чанка и говорит, цела ли она.
///
/// Ошибка (`Err`) — только сбой самого чтения: файл не открылся, диск не
/// ответил. Такой чанк нельзя считать ни пропавшим, ни испорченным — его
/// надо попробовать прочитать позже.
pub fn read_stored(directory: &Path, chunk_x: i32, chunk_z: i32) -> io::Result<Stored> {
    let path = path_for(directory, chunk_x, chunk_z);

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(e) => return Err(e),
    };

    let mut header = vec![0u8; SECTOR];

    if let Err(e) = file.read_exact(&mut header) {
        // Файл короче таблицы — значит, в нём ещё ничего нет.
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(Stored::Missing);
        }

        return Err(e);
    }

    let (offset, sectors) = entry(&header, slot_of(chunk_x, chunk_z));

    if sectors == 0 {
        return Ok(Stored::Missing);
    }

    if offset < HEADER_SECTORS {
        return Ok(Stored::Damaged {
            raw: Vec::new(),
            why: "в таблице региона чанк указан поверх самой таблицы",
        });
    }

    // Всё место записи целиком: сколько его есть в файле.
    file.seek(SeekFrom::Start((offset * SECTOR) as u64))?;

    let mut raw = Vec::with_capacity(sectors * SECTOR);
    Read::by_ref(&mut file).take((sectors * SECTOR) as u64).read_to_end(&mut raw)?;

    Ok(parse_record(raw, sectors))
}

/// Разбирает запись чанка с её места в файле.
fn parse_record(raw: Vec<u8>, sectors: usize) -> Stored {
    let Some(length) = raw.get(..4) else {
        return Stored::Damaged { raw, why: "файл региона обрывается на записи чанка" };
    };

    let length = u32::from_be_bytes([length[0], length[1], length[2], length[3]]);
    let checked = length & CHECKED != 0;
    let length = (length & !CHECKED) as usize;
    let start = if checked { 8 } else { 4 };

    // Длина, сумма и сами данные обязаны уместиться в отведённые секторы.
    if length + start > sectors * SECTOR {
        return Stored::Damaged { raw, why: "в таблице региона указана не та длина чанка" };
    }

    let Some(bytes) = raw.get(start..start + length) else {
        return Stored::Damaged { raw, why: "файл региона обрывается на записи чанка" };
    };

    if checked {
        let sum = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]);

        if sum != checksum(bytes) {
            return Stored::Damaged { raw, why: "не сходится контрольная сумма чанка" };
        }
    }

    Stored::Found(bytes.to_vec())
}

/// Контрольная сумма записи — обычная CRC-32.
fn checksum(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();

    crc.update(bytes);
    crc.sum()
}

/// Записывает содержимое одного чанка.
#[cfg(test)]
pub fn write(directory: &Path, chunk_x: i32, chunk_z: i32, bytes: &[u8]) -> io::Result<()> {
    write_many(directory, &[(chunk_x, chunk_z, bytes)])
}

/// Записывает сразу несколько чанков одного региона: файл открывается один
/// раз, и на диск он сбрасывается дважды на всю пачку, а не на каждый чанк.
///
/// Чанк никогда не пишется поверх своей прежней записи: новая ложится в
/// свободные секторы (или в конец файла), сбрасывается на диск, и только
/// после этого таблица начинает указывать на неё. Оборвись запись на
/// середине — в таблице останется прежний, целый чанк. Освободившиеся
/// секторы займут следующие записи.
pub fn write_many(directory: &Path, chunks: &[(i32, i32, &[u8])]) -> io::Result<()> {
    let Some(&(first_x, first_z, _)) = chunks.first() else {
        return Ok(());
    };

    let region = region_of(first_x, first_z);

    if chunks.iter().any(|(x, z, _)| region_of(*x, *z) != region) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "в одну пачку попали чанки разных регионов",
        ));
    }

    fs::create_dir_all(directory)?;

    let path = path_for(directory, first_x, first_z);
    let created = !path.exists();

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;

    let mut header = vec![0u8; SECTOR * HEADER_SECTORS];
    let length = file.metadata()?.len() as usize;

    // Новый файл начинается с пустых таблиц.
    if length < SECTOR * HEADER_SECTORS {
        file.set_len((SECTOR * HEADER_SECTORS) as u64)?;
    } else {
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut header)?;
    }

    // Какие секторы заняты. Прежние места переписываемых чанков тоже
    // заняты — до самого конца пачки: на диске таблица ещё указывает на них.
    let mut used = vec![false; length.div_ceil(SECTOR).max(HEADER_SECTORS)];

    used[..HEADER_SECTORS].fill(true);

    for slot in 0..(REGION_SIZE * REGION_SIZE) as usize {
        let (offset, sectors) = entry(&header, slot);

        if sectors == 0 || offset < HEADER_SECTORS {
            continue;
        }

        mark(&mut used, offset, sectors);
    }

    let mut placed = Vec::with_capacity(chunks.len());

    for &(chunk_x, chunk_z, bytes) in chunks {
        // Длина с пометкой, сумма, дальше сами данные, и всё это дополняется
        // до целых секторов: так каждый чанк начинается со своего сектора.
        let mut payload = Vec::with_capacity(bytes.len() + 8);
        payload.extend_from_slice(&(bytes.len() as u32 | CHECKED).to_be_bytes());
        payload.extend_from_slice(&checksum(bytes).to_be_bytes());
        payload.extend_from_slice(bytes);

        let sectors = payload.len().div_ceil(SECTOR);
        payload.resize(sectors * SECTOR, 0);

        if sectors > u8::MAX as usize || bytes.len() as u32 & CHECKED != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "чанк не помещается в отведённые ему секторы",
            ));
        }

        let offset = free_run(&used, sectors);

        mark(&mut used, offset, sectors);

        file.seek(SeekFrom::Start((offset * SECTOR) as u64))?;
        file.write_all(&payload)?;

        placed.push((slot_of(chunk_x, chunk_z), offset, sectors));
    }

    // Сперва данные — на диск, и только потом таблица.
    file.sync_data()?;

    for (slot, offset, sectors) in placed {
        set_entry(&mut header, slot, offset, sectors);
        set_written(&mut header, slot);
    }

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;
    file.sync_data()?;

    // Новый файл должен остаться в каталоге и после сбоя.
    if created {
        sync_directory(directory);
    }

    Ok(())
}

/// Отмечает секторы занятыми, при нужде удлиняя список.
fn mark(used: &mut Vec<bool>, offset: usize, sectors: usize) {
    if used.len() < offset + sectors {
        used.resize(offset + sectors, false);
    }

    used[offset..offset + sectors].fill(true);
}

/// Первое место, где подряд свободно столько секторов; нет такого — конец
/// файла.
fn free_run(used: &[bool], sectors: usize) -> usize {
    let mut run = 0;

    for (at, taken) in used.iter().enumerate().skip(HEADER_SECTORS) {
        if *taken {
            run = 0;
            continue;
        }

        run += 1;

        if run == sectors {
            return at + 1 - sectors;
        }
    }

    // Хвост файла может быть свободен не целиком: новая запись начинается
    // сразу за последним занятым сектором.
    used.len() - run
}

/// Сбрасывает на диск сам каталог: без этого новое имя файла может
/// пропасть при сбое питания, хотя содержимое уже записано.
pub fn sync_directory(directory: &Path) {
    if let Ok(directory) = File::open(directory) {
        let _ = directory.sync_all();
    }
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

    /// Переписанный чанк ложится на новое место, а не поверх прежнего:
    /// оборвись запись — таблица указывала бы на целый старый чанк. Ушедшее
    /// место потом занимает следующая запись, и файл не растёт без конца.
    #[test]
    fn a_rewrite_goes_to_free_sectors() {
        let directory = directory("cow");

        write(&directory, 0, 0, &[1; 100]).expect("записать");

        let header = |directory: &Path| {
            let mut bytes = vec![0u8; SECTOR];
            File::open(path_for(directory, 0, 0)).unwrap().read_exact(&mut bytes).unwrap();
            bytes
        };
        let (first, _) = entry(&header(&directory), slot_of(0, 0));

        write(&directory, 0, 0, &[2; 100]).expect("переписать");

        let (second, _) = entry(&header(&directory), slot_of(0, 0));

        assert_ne!(first, second, "чанк переписан на месте");
        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some(vec![2; 100]));

        // Третья запись занимает освободившееся первое место.
        write(&directory, 0, 0, &[3; 100]).expect("переписать ещё раз");

        let (third, _) = entry(&header(&directory), slot_of(0, 0));

        assert_eq!(third, first, "освободившееся место не занято");
        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some(vec![3; 100]));

        let length = fs::metadata(path_for(&directory, 0, 0)).unwrap().len() as usize;
        assert_eq!(length, (HEADER_SECTORS + 2) * SECTOR, "файл разросся");
    }

    /// Пачка чанков одного региона пишется разом и читается обратно.
    #[test]
    fn a_batch_is_written_at_once() {
        let directory = directory("batch");
        let big = vec![7u8; SECTOR * 2];
        let batch: Vec<(i32, i32, &[u8])> = vec![(0, 0, &[1u8; 10][..]), (5, 3, &big[..]), (31, 31, &[9u8; 1][..])];

        write_many(&directory, &batch).expect("записать пачку");

        assert_eq!(read(&directory, 0, 0).unwrap(), Some(vec![1; 10]));
        assert_eq!(read(&directory, 5, 3).unwrap(), Some(big.clone()));
        assert_eq!(read(&directory, 31, 31).unwrap(), Some(vec![9; 1]));

        // Чанки разных регионов в одну пачку не берутся.
        let mixed: Vec<(i32, i32, &[u8])> = vec![(0, 0, &[1u8][..]), (32, 0, &[1u8][..])];
        assert!(write_many(&directory, &mixed).is_err());
    }

    /// Испорченная запись распознаётся по контрольной сумме, а её байты
    /// отдаются целиком — чтобы их можно было сберечь.
    #[test]
    fn a_damaged_record_is_recognised() {
        let directory = directory("damaged");

        write(&directory, 2, 2, &[5; 300]).expect("записать");

        let mut header = vec![0u8; SECTOR];
        let path = path_for(&directory, 2, 2);
        let mut file = OpenOptions::new().read(true).write(true).open(&path).unwrap();

        file.read_exact(&mut header).unwrap();

        let (offset, _) = entry(&header, slot_of(2, 2));

        // Портим один байт внутри данных.
        file.seek(SeekFrom::Start((offset * SECTOR + 100) as u64)).unwrap();
        file.write_all(&[6]).unwrap();
        drop(file);

        match read_stored(&directory, 2, 2).expect("прочитать") {
            Stored::Damaged { raw, .. } => assert_eq!(raw.len(), SECTOR),
            other => panic!("порча не замечена: {:?}", other),
        }

        assert_eq!(read(&directory, 2, 2).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    /// Записи прежнего вида — без суммы — читаются как раньше, и длина,
    /// впритык занимающая свои секторы, не считается ошибкой.
    #[test]
    fn old_records_without_a_checksum_are_read() {
        let body = vec![4u8; SECTOR - 4];
        let mut raw = (body.len() as u32).to_be_bytes().to_vec();

        raw.extend_from_slice(&body);

        assert_eq!(parse_record(raw.clone(), 1), Stored::Found(body.clone()));

        // На байт длиннее — уже не помещается.
        let mut long = ((body.len() + 1) as u32).to_be_bytes().to_vec();
        long.extend_from_slice(&body);
        long.push(4);

        assert!(matches!(parse_record(long, 1), Stored::Damaged { .. }));

        // Файл обрывается посреди записи.
        let mut cut = raw;
        cut.truncate(100);

        assert!(matches!(parse_record(cut, 1), Stored::Damaged { .. }));
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
