// Мир на диске: регионы `.mcs`.
//
// Хранить весь мир одним файлом нельзя: он переписывается целиком, и чем
// больше мир, тем дороже каждая запись. Поэтому мир разложен по регионам —
// так же, как в игре (minecraft.wiki, страница Region file format): один
// регион это квадрат 32×32 чанка, и лежит он отдельным файлом.
//
// От вики взята только мысль: таблица в начале файла говорит, где чей чанк,
// и чтобы переписать один чанк, достаточно тронуть его место и одну строчку
// таблицы, а не весь файл. Остальное своё, и цель одна — чтобы мир занимал
// как можно меньше места:
//
//   * кусок выделения не 4 КиБ, а 256 байт: чанк после сжатия весит
//     несколько килобайт, и с крупными секторами четверть файла была бы
//     пустотой;
//   * каждый чанк сжат zstd (см. `compress`), а внутри него блоки лежат
//     палитрой с битовыми индексами (см. `Chunk::to_bytes` в mod.rs);
//   * хвост файла, освободившийся после переписывания, обрезается.
//
// Раскладка файла (числа — big-endian):
//
//   0   3   магия «MCS»
//   3   1   версия раскладки (1)
//   4   1   сжатие: 1 — zstd
//   5   3   зарезервировано, нули
//   8   8192 таблица: 1024 строки по 8 байт — (сдвиг в кусках по 256 байт,
//           длина записи в байтах); нули — чанка нет
//   ... записи, каждая начинается с границы куска:
//           4  CRC-32 сжатых байтов
//           4  длина сжатых байтов
//           N  сжатые байты (zstd), внутри — запись чанка
//
// Данные не пропадают: запись, которая не сошлась по длине, сумме или не
// распаковалась, не выбрасывается, а отдаётся целиком как `Stored::Damaged`,
// и вызывающий откладывает её в сторону.

use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use windows_at::FileExt;

/// Чтение и запись по смещению для Windows: те же `read_exact_at` и
/// `write_all_at`, что у unix, поверх `seek_read`/`seek_write`. Курсор файла
/// они двигают, но регион везде ходит по смещениям, а не по курсору.
#[cfg(windows)]
mod windows_at {
    use std::fs::File;
    use std::io;
    use std::os::windows::fs::FileExt as _;

    pub trait FileExt {
        fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
        fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()>;
    }

    impl FileExt for File {
        fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
            while !buf.is_empty() {
                match self.seek_read(buf, offset) {
                    Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                    Ok(n) => {
                        buf = &mut buf[n..];
                        offset += n as u64;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }

        fn write_all_at(&self, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
            while !buf.is_empty() {
                match self.seek_write(buf, offset) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(n) => {
                        buf = &buf[n..];
                        offset += n as u64;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }
    }
}
use std::path::{Path, PathBuf};

/// Сторона региона в чанках.
pub const REGION_SIZE: i32 = 32;

/// Чанков в регионе.
const SLOTS: usize = (REGION_SIZE * REGION_SIZE) as usize;

/// Размер куска, которыми выделяется место. Меньше — меньше пустот, но
/// длиннее записи в таблице; 256 байт дают потерю в среднем 128 байт на
/// чанк, то есть около двух процентов от чанка.
const UNIT: usize = 256;

/// Магия и версия раскладки в начале файла.
const MAGIC: [u8; 3] = *b"MCS";
const LAYOUT: u8 = 1;

/// Сжатие: 1 — zstd.
const CODEC_ZSTD: u8 = 1;

/// Где в файле начинается таблица и сколько она занимает.
const TABLE_AT: usize = 8;
const HEADER_LEN: usize = TABLE_AT + SLOTS * 8;

/// Сколько кусков занимает начало файла: за ним идут записи.
const HEADER_UNITS: usize = HEADER_LEN.div_ceil(UNIT);

/// Длина, сумма и сразу за ними — сжатые байты.
const PRELUDE: usize = 8;

/// Больше этого запись чанка (после распаковки) не бывает: чанк в десятки
/// мегабайт — это порча, а не чанк.
const MAX_PAYLOAD: usize = 64 << 20;

/// Уровень zstd. Выбран замером (см. `world_mcs_bench`): до третьего уровня
/// размер падает заметно, дальше — на доли процента, а время растёт.
const LEVEL: i32 = 3;

#[cfg(test)]
pub(super) const LEVEL_FOR_BENCH: i32 = LEVEL;

/// Файл региона, в котором лежит этот чанк.
pub fn path_for(directory: &Path, chunk_x: i32, chunk_z: i32) -> PathBuf {
    let (region_x, region_z) = region_of(chunk_x, chunk_z);

    directory.join(format!("r.{}.{}.mcs", region_x, region_z))
}

/// В каком регионе лежит чанк.
pub fn region_of(chunk_x: i32, chunk_z: i32) -> (i32, i32) {
    (
        chunk_x.div_euclid(REGION_SIZE),
        chunk_z.div_euclid(REGION_SIZE),
    )
}

/// Место чанка в таблице региона.
pub fn slot_of(chunk_x: i32, chunk_z: i32) -> usize {
    let x = chunk_x.rem_euclid(REGION_SIZE) as usize;
    let z = chunk_z.rem_euclid(REGION_SIZE) as usize;

    x + z * REGION_SIZE as usize
}

/// Что нашлось в файле региона на месте чанка.
#[derive(Debug, PartialEq)]
pub enum Stored {
    /// Чанка в файле нет.
    Missing,

    /// Чанк на месте, и запись цела. Внутри — уже распакованная запись.
    Found(Vec<u8>),

    /// Запись есть, но испорчена: не сходится длина или сумма, не
    /// распаковывается, файл обрывается. `raw` — всё, что удалось прочитать
    /// с её места, чтобы отложить в сторону, а не потерять.
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
/// ответил, файл не нашей версии. Такой чанк нельзя считать ни пропавшим,
/// ни испорченным — его надо попробовать прочитать позже.
pub fn read_stored(directory: &Path, chunk_x: i32, chunk_z: i32) -> io::Result<Stored> {
    // Ещё не записанное лежит у потока записи — оно новее того, что на диске.
    if let Some(bytes) = super::writer::waiting(directory, chunk_x, chunk_z) {
        return Ok(Stored::Found(bytes.to_vec()));
    }

    read_slot(&path_for(directory, chunk_x, chunk_z), slot_of(chunk_x, chunk_z))
}

/// Читает одно место таблицы из файла региона.
pub fn read_slot(path: &Path, slot: usize) -> io::Result<Stored> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Stored::Missing),
        Err(e) => return Err(e),
    };

    let mut head = [0u8; TABLE_AT];

    if let Err(e) = file.read_exact_at(&mut head, 0) {
        // Файл короче начала — значит, в нём ещё ничего нет.
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(Stored::Missing);
        }

        return Err(e);
    }

    check_head(&head)?;

    let mut row = [0u8; 8];

    if let Err(e) = file.read_exact_at(&mut row, (TABLE_AT + slot * 8) as u64) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(Stored::Missing);
        }

        return Err(e);
    }

    let (offset, length) = entry(&row, 0);

    if length == 0 {
        return Ok(Stored::Missing);
    }

    if offset < HEADER_UNITS {
        return Ok(Stored::Damaged {
            raw: Vec::new(),
            why: "в таблице региона чанк указан поверх самой таблицы",
        });
    }

    // Всё место записи: сколько его есть в файле.
    file.seek(SeekFrom::Start((offset * UNIT) as u64))?;

    let mut raw = Vec::with_capacity(length.min(1 << 20));
    Read::by_ref(&mut file).take(length as u64).read_to_end(&mut raw)?;

    Ok(parse_record(raw, length))
}

/// Начало файла должно быть нашим и нашей версии: чужой файл не читаем и,
/// главное, не пишем — он мог бы пропасть.
fn check_head(head: &[u8]) -> io::Result<()> {
    if head[..3] != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "это не файл региона MCSheriffAnya (.mcs)"));
    }

    if head[3] != LAYOUT || head[4] != CODEC_ZSTD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("файл региона версии {}/{}, а сервер понимает {}/{}", head[3], head[4], LAYOUT, CODEC_ZSTD),
        ));
    }

    Ok(())
}

/// Разбирает запись чанка с её места в файле: длина, сумма, распаковка.
fn parse_record(raw: Vec<u8>, length: usize) -> Stored {
    if raw.len() < length || length < PRELUDE {
        return Stored::Damaged { raw, why: "файл региона обрывается на записи чанка" };
    }

    let sum = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let body = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;

    // Длина в самой записи и в таблице обязаны совпасть.
    if body + PRELUDE != length {
        return Stored::Damaged { raw, why: "в таблице региона указана не та длина чанка" };
    }

    if sum != checksum(&raw[PRELUDE..]) {
        return Stored::Damaged { raw, why: "не сходится контрольная сумма чанка" };
    }

    match decompress(&raw[PRELUDE..]) {
        Ok(bytes) => Stored::Found(bytes),
        Err(_) => Stored::Damaged { raw, why: "запись чанка не распаковывается" },
    }
}

/// Контрольная сумма записи — обычная CRC-32.
fn checksum(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();

    crc.update(bytes);
    crc.sum()
}

thread_local! {
    /// Сжиматель и распаковщик живут на потоке: их состояние дорого
    /// заводить на каждый чанк.
    static COMPRESSOR: RefCell<Option<zstd::bulk::Compressor<'static>>> = const { RefCell::new(None) };
    static DECOMPRESSOR: RefCell<Option<zstd::bulk::Decompressor<'static>>> = const { RefCell::new(None) };
}

/// Сжимает запись чанка.
pub(super) fn compress(payload: &[u8]) -> io::Result<Vec<u8>> {
    COMPRESSOR.with(|slot| {
        let mut slot = slot.borrow_mut();

        if slot.is_none() {
            *slot = Some(zstd::bulk::Compressor::new(LEVEL)?);
        }

        slot.as_mut().expect("сжиматель заведён выше").compress(payload)
    })
}

/// Распаковывает запись чанка. Размер берётся из заголовка кадра, но не
/// больше предела: порченый заголовок не должен просить гигабайты.
pub(super) fn decompress(body: &[u8]) -> io::Result<Vec<u8>> {
    let size = match zstd::zstd_safe::get_frame_content_size(body) {
        Ok(Some(size)) if size as usize <= MAX_PAYLOAD => size as usize,
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "размер записи не указан или слишком велик")),
    };

    DECOMPRESSOR.with(|slot| {
        let mut slot = slot.borrow_mut();

        if slot.is_none() {
            *slot = Some(zstd::bulk::Decompressor::new()?);
        }

        slot.as_mut().expect("распаковщик заведён выше").decompress(body, size)
    })
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
/// свободные куски (или в конец файла), сбрасывается на диск, и только
/// после этого таблица начинает указывать на неё. Оборвись запись на
/// середине — в таблице останется прежний, целый чанк. Освободившееся место
/// займут следующие записи.
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

    let slots: Vec<(usize, &[u8])> = chunks.iter().map(|&(x, z, bytes)| (slot_of(x, z), bytes)).collect();

    write_slots(&path_for(directory, first_x, first_z), &slots)
}

/// Записывает записи чанков в файл региона по номерам мест. Пустой список
/// тоже заводит файл: у него есть таблица, и она пуста.
pub fn write_slots(path: &Path, chunks: &[(usize, &[u8])]) -> io::Result<()> {
    // Сжатие — до того, как тронут файл, и не под его замком.
    let mut records = Vec::with_capacity(chunks.len());

    for &(slot, bytes) in chunks {
        if bytes.len() > MAX_PAYLOAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "чанк слишком велик"));
        }

        let body = compress(bytes)?;
        let mut record = Vec::with_capacity(PRELUDE + body.len());

        record.extend_from_slice(&checksum(&body).to_be_bytes());
        record.extend_from_slice(&(body.len() as u32).to_be_bytes());
        record.extend_from_slice(&body);

        records.push((slot, record));
    }

    let created = !path.exists();

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;

    let mut header = vec![0u8; HEADER_LEN];
    let length = file.metadata()?.len() as usize;

    if length < HEADER_LEN {
        // Новый файл начинается с пустой таблицы. Файл короче таблицы — это
        // либо только что созданный, либо оборванный на создании: чанков
        // в нём быть не могло.
        header[..3].copy_from_slice(&MAGIC);
        header[3] = LAYOUT;
        header[4] = CODEC_ZSTD;

        file.set_len(HEADER_LEN.next_multiple_of(UNIT) as u64)?;
        file.write_all_at(&header, 0)?;
    } else {
        file.read_exact_at(&mut header, 0)?;
        check_head(&header)?;
    }

    let length = file.metadata()?.len() as usize;

    // Какие куски заняты. Прежние места переписываемых чанков тоже заняты —
    // до самого конца пачки: на диске таблица ещё указывает на них.
    let mut used = vec![false; length.div_ceil(UNIT).max(HEADER_UNITS)];

    used[..HEADER_UNITS].fill(true);

    for slot in 0..SLOTS {
        let (offset, bytes) = entry(&header[TABLE_AT..], slot);

        if bytes == 0 || offset < HEADER_UNITS {
            continue;
        }

        mark(&mut used, offset, bytes.div_ceil(UNIT));
    }

    let mut placed = Vec::with_capacity(records.len());

    for (slot, mut record) in records {
        let length = record.len();
        let units = length.div_ceil(UNIT);

        record.resize(units * UNIT, 0);

        let offset = free_run(&used, units);

        mark(&mut used, offset, units);

        file.write_all_at(&record, (offset * UNIT) as u64)?;

        placed.push((slot, offset, length));
    }

    // Сперва данные — на диск, и только потом таблица.
    file.sync_data()?;

    for (slot, offset, length) in placed {
        set_entry(&mut header[TABLE_AT..], slot, offset, length);
    }

    file.write_all_at(&header, 0)?;

    // Прежние места переписанных чанков теперь свободны; если они были
    // в хвосте, файл сжимается. Таблица уже указывает на новое, так что
    // обрезать можно всё, что за последней записью.
    let mut end = HEADER_UNITS;

    for slot in 0..SLOTS {
        let (offset, bytes) = entry(&header[TABLE_AT..], slot);

        if bytes != 0 && offset >= HEADER_UNITS {
            end = end.max(offset + bytes.div_ceil(UNIT));
        }
    }

    if (end * UNIT) < file.metadata()?.len() as usize {
        file.set_len((end * UNIT) as u64)?;
    }

    file.sync_data()?;

    // Новый файл должен остаться в каталоге и после сбоя.
    if created && let Some(directory) = path.parent() {
        sync_directory(directory);
    }

    Ok(())
}

/// Отмечает куски занятыми, при нужде удлиняя список.
fn mark(used: &mut Vec<bool>, offset: usize, units: usize) {
    if used.len() < offset + units {
        used.resize(offset + units, false);
    }

    used[offset..offset + units].fill(true);
}

/// Первое место, где подряд свободно столько кусков; нет такого — конец
/// файла.
fn free_run(used: &[bool], units: usize) -> usize {
    let mut run = 0;

    for (at, taken) in used.iter().enumerate().skip(HEADER_UNITS) {
        if *taken {
            run = 0;
            continue;
        }

        run += 1;

        if run == units {
            return at + 1 - units;
        }
    }

    // Хвост файла может быть свободен не целиком: новая запись начинается
    // сразу за последним занятым куском.
    used.len() - run
}

/// Сбрасывает на диск сам каталог: без этого новое имя файла может
/// пропасть при сбое питания, хотя содержимое уже записано.
pub fn sync_directory(directory: &Path) {
    if let Ok(directory) = File::open(directory) {
        let _ = directory.sync_all();
    }
}

/// Где лежит чанк (в кусках) и сколько байт занимает его запись.
fn entry(table: &[u8], slot: usize) -> (usize, usize) {
    let at = slot * 8;

    let offset = u32::from_be_bytes([table[at], table[at + 1], table[at + 2], table[at + 3]]) as usize;
    let length = u32::from_be_bytes([table[at + 4], table[at + 5], table[at + 6], table[at + 7]]) as usize;

    (offset, length)
}

/// Записывает в таблицу, где теперь лежит чанк.
fn set_entry(table: &mut [u8], slot: usize, offset: usize, length: usize) {
    let at = slot * 8;

    table[at..at + 4].copy_from_slice(&(offset as u32).to_be_bytes());
    table[at + 4..at + 8].copy_from_slice(&(length as u32).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn directory(name: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("mcsheriffanya_region_{}_{}", std::process::id(), name));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("создать директорию для проверки");

        directory
    }

    /// Строка таблицы чанка: (где, сколько байт).
    fn row(directory: &Path, chunk_x: i32, chunk_z: i32) -> (usize, usize) {
        let mut bytes = vec![0u8; HEADER_LEN];

        File::open(path_for(directory, chunk_x, chunk_z)).unwrap().read_exact(&mut bytes).unwrap();

        entry(&bytes[TABLE_AT..], slot_of(chunk_x, chunk_z))
    }

    /// Байты, которые не сжимаются: чтобы запись занимала место всерьёз.
    fn noise(length: usize, seed: u32) -> Vec<u8> {
        let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);

        (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect()
    }

    /// Записанный чанк читается обратно ровно таким же.
    #[test]
    fn a_chunk_is_read_back() {
        let directory = directory("round_trip");

        write(&directory, 3, 5, "содержимое".as_bytes()).expect("записать");

        assert_eq!(read(&directory, 3, 5).expect("прочитать"), Some("содержимое".as_bytes().to_vec()));
    }

    /// Файл начинается с магии и версии, а имя у него `.mcs`.
    #[test]
    fn the_file_starts_with_a_magic_and_a_version() {
        let directory = directory("magic");

        write(&directory, 0, 0, &[1]).expect("записать");

        let path = path_for(&directory, 0, 0);
        let bytes = fs::read(&path).unwrap();

        assert!(path.to_string_lossy().ends_with("r.0.0.mcs"));
        assert_eq!(&bytes[..5], &[b'M', b'C', b'S', LAYOUT, CODEC_ZSTD]);
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
        let big = noise(UNIT * 30, 1);

        write(&directory, 0, 0, &[1; 10]).expect("записать маленький");
        write(&directory, 1, 0, &[2; 10]).expect("записать соседний");
        write(&directory, 0, 0, &big).expect("записать выросший");

        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some(big));
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
    fn a_rewrite_goes_to_free_units() {
        let directory = directory("cow");

        // Несжимаемое, чтобы запись занимала несколько кусков.
        let (a, b, c) = (noise(600, 1), noise(600, 2), noise(600, 3));

        write(&directory, 0, 0, &a).expect("записать");

        let (first, _) = row(&directory, 0, 0);

        write(&directory, 0, 0, &b).expect("переписать");

        let (second, _) = row(&directory, 0, 0);

        assert_ne!(first, second, "чанк переписан на месте");
        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some(b));

        // Третья запись занимает освободившееся первое место.
        write(&directory, 0, 0, &c).expect("переписать ещё раз");

        let (third, length) = row(&directory, 0, 0);

        assert_eq!(third, first, "освободившееся место не занято");
        assert_eq!(read(&directory, 0, 0).expect("прочитать"), Some(c));

        // Файл кончается на последней записи: хвост не копится.
        let file = fs::metadata(path_for(&directory, 0, 0)).unwrap().len() as usize;

        assert_eq!(file, (third + length.div_ceil(UNIT)) * UNIT, "файл разросся");
    }

    /// Пачка чанков одного региона пишется разом и читается обратно.
    #[test]
    fn a_batch_is_written_at_once() {
        let directory = directory("batch");
        let big = noise(UNIT * 8, 4);
        let batch: Vec<(i32, i32, &[u8])> = vec![(0, 0, &[1u8; 10][..]), (5, 3, &big[..]), (31, 31, &[9u8; 1][..])];

        write_many(&directory, &batch).expect("записать пачку");

        assert_eq!(read(&directory, 0, 0).unwrap(), Some(vec![1; 10]));
        assert_eq!(read(&directory, 5, 3).unwrap(), Some(big.clone()));
        assert_eq!(read(&directory, 31, 31).unwrap(), Some(vec![9; 1]));

        // Чанки разных регионов в одну пачку не берутся.
        let mixed: Vec<(i32, i32, &[u8])> = vec![(0, 0, &[1u8][..]), (32, 0, &[1u8][..])];
        assert!(write_many(&directory, &mixed).is_err());
    }

    /// Сжатый чанк занимает меньше, чем он весит, а место выделяется
    /// мелкими кусками: ровная запись в четыре килобайта — считанные куски.
    #[test]
    fn a_compressible_chunk_takes_few_units() {
        let directory = directory("small");

        write(&directory, 0, 0, &[7; 4096]).expect("записать");

        let (_, length) = row(&directory, 0, 0);

        assert!(length < 64, "запись заняла {} байт", length);
        assert_eq!(fs::metadata(path_for(&directory, 0, 0)).unwrap().len() as usize, HEADER_UNITS * UNIT + UNIT);
    }

    /// Порча одного байта данных распознаётся по контрольной сумме, а байты
    /// записи отдаются целиком — чтобы их можно было сберечь.
    #[test]
    fn a_checksum_mismatch_is_recognised() {
        let directory = directory("damaged");

        write(&directory, 2, 2, &noise(300, 5)).expect("записать");

        let (offset, length) = row(&directory, 2, 2);
        let path = path_for(&directory, 2, 2);
        let file = OpenOptions::new().read(true).write(true).open(&path).unwrap();

        // Портим один байт внутри данных.
        let mut byte = [0u8; 1];
        file.read_exact_at(&mut byte, (offset * UNIT + PRELUDE + 20) as u64).unwrap();
        file.write_all_at(&[byte[0] ^ 0xFF], (offset * UNIT + PRELUDE + 20) as u64).unwrap();
        drop(file);

        match read_stored(&directory, 2, 2).expect("прочитать") {
            Stored::Damaged { raw, why } => {
                assert_eq!(raw.len(), length, "отдана не вся запись");
                assert!(why.contains("сумма"), "{}", why);
            }
            other => panic!("порча не замечена: {:?}", other),
        }

        assert_eq!(read(&directory, 2, 2).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    /// Запись с верной суммой, но не zstd, не пропадает, а тоже считается
    /// испорченной.
    #[test]
    fn a_body_that_is_not_zstd_is_damaged() {
        let body = b"not zstd at all".to_vec();
        let mut raw = checksum(&body).to_be_bytes().to_vec();

        raw.extend_from_slice(&(body.len() as u32).to_be_bytes());
        raw.extend_from_slice(&body);

        let length = raw.len();

        assert!(matches!(parse_record(raw, length), Stored::Damaged { why: "запись чанка не распаковывается", .. }));
    }

    /// Неверная длина, обрыв файла и таблица, указывающая на саму себя.
    #[test]
    fn broken_lengths_are_damaged() {
        let directory = directory("lengths");

        write(&directory, 0, 0, &noise(2000, 6)).expect("записать");

        let path = path_for(&directory, 0, 0);
        let (offset, length) = row(&directory, 0, 0);

        // Файл обрывается посреди записи.
        let file = OpenOptions::new().write(true).open(&path).unwrap();

        file.set_len((offset * UNIT + length / 2) as u64).unwrap();
        drop(file);

        match read_stored(&directory, 0, 0).unwrap() {
            Stored::Damaged { raw, .. } => assert_eq!(raw.len(), length / 2),
            other => panic!("{:?}", other),
        }

        // Строка таблицы указывает на начало файла.
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        let mut table = vec![0u8; 8];

        set_entry(&mut table, 0, 1, 100);
        file.write_all_at(&table, TABLE_AT as u64).unwrap();
        drop(file);

        assert!(matches!(read_stored(&directory, 0, 0).unwrap(), Stored::Damaged { .. }));

        // Длина в записи не та, что в таблице.
        let raw = vec![0u8; 40];

        assert!(matches!(parse_record(raw, 32), Stored::Damaged { why: "в таблице региона указана не та длина чанка", .. }));
    }

    /// Чужой файл или файл другой версии не читается и не переписывается.
    #[test]
    fn a_foreign_file_is_left_alone() {
        let directory = directory("foreign");
        let path = path_for(&directory, 0, 0);
        let foreign = vec![0x55u8; HEADER_LEN + 100];

        fs::write(&path, &foreign).unwrap();

        assert_eq!(read_stored(&directory, 0, 0).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(write(&directory, 0, 0, &[1]).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).unwrap(), foreign, "чужой файл тронут");

        // Наша магия, но версия из будущего.
        let mut future = vec![0u8; HEADER_LEN];

        future[..3].copy_from_slice(&MAGIC);
        future[3] = LAYOUT + 1;
        future[4] = CODEC_ZSTD;
        fs::write(&path, &future).unwrap();

        assert!(read_stored(&directory, 0, 0).is_err());
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
