// Прежний вид мира: регионы `.rcw`.
//
// До `.mcs` чанки лежали в файлах `r.X.Z.rcw`: секторы по 4 КиБ, а внутри
// записи блоки шли как «состояние и сколько раз подряд», без сжатия. Этот
// вид больше не пишется. Здесь он только читается — чтобы при запуске
// перенести мир в `.mcs` (`migrate`), ничего не потеряв.
//
// Перенос идёт по одному региону: читаем все чанки старым разбором, пишем
// `r.X.Z.mcs.tmp`, читаем его заново и сверяем каждый чанк с прочитанным из
// старого файла; и только когда всё сошлось, файл встаёт на место, а старый
// удаляется. Что в старом файле было испорчено или не разбирается, не
// пропадает: байты откладываются в `region/corrupt/`, как и при обычной
// загрузке.

use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};

/// Версия записи чанка, с которой началась эта раскладка: биомы объёмные,
/// «номер и сколько подряд».
const LEGACY_VERSION: u8 = 3;

/// Размер сектора старого файла.
const SECTOR: usize = 4096;

/// Сколько секторов занимали таблицы в начале старого файла.
const HEADER_SECTORS: usize = 2;

/// Пометка в старшем бите длины: за длиной идёт контрольная сумма записи.
const CHECKED: u32 = 0x8000_0000;

impl Chunk {
    /// Записывает чанк так, как писал его сервер до `.mcs` (для проверок:
    /// из этого делаются прежние файлы).
    ///
    ///
    /// Записывается не каждый блок по отдельности, а «состояние и сколько его
    /// подряд»: у ровной земли в секции всего несколько таких записей вместо
    /// четырёх тысяч блоков.
    #[cfg(test)]
    pub(super) fn to_legacy_bytes(&self) -> Vec<u8> {
        let mut out = vec![LEGACY_VERSION];

        for section in &self.sections {
            let Some(blocks) = section else {
                out.push(0); // секция целиком из воздуха
                continue;
            };

            out.push(1);

            let runs = runs_of(blocks);

            out.extend_from_slice(&(runs.len() as u16).to_be_bytes());

            for (state, length) in runs {
                out.extend_from_slice(&state.to_be_bytes());
                out.extend_from_slice(&length.to_be_bytes());
            }
        }

        // Биомы идут в конце — так же, «номер и сколько его подряд», в
        // порядке клеток: у чанка без пещерных биомов это одна-две записи.
        let runs = runs_of(&self.biomes);

        out.extend_from_slice(&(runs.len() as u16).to_be_bytes());

        for (biome, length) in runs {
            out.extend_from_slice(&biome.to_be_bytes());
            out.extend_from_slice(&length.to_be_bytes());
        }

        // Хранилища — последними, и только если они есть: запись чанка без
        // них та же, что и прежде. Порядок — по месту, чтобы одинаковый
        // чанк давал одинаковые байты.
        if !self.containers.is_empty() || !self.custom_tnt.is_empty() {
            let mut places: Vec<_> = self.containers.keys().copied().collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for place in places {
                self.containers[&place].write(place, &mut out);
            }
        }

        // Особый динамит — после хранилищ (их число тогда пишется и
        // нулём): сколько, потом место и номер вида.
        if !self.custom_tnt.is_empty() {
            let mut places: Vec<_> = self.custom_tnt.iter().map(|&(place, kind)| (place, kind.0)).collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for ((x, y, z), kind) in places {
                out.push(((x << 4) | z) as u8);
                out.extend_from_slice(&(y as i16).to_be_bytes());
                out.push(kind);
            }
        }

        out
    }

    /// Читает чанк из байтов файла региона `.rcw` — так, как их писал сервер
    /// до `.mcs` (версии записи 1, 2 и 3).
    ///
    ///
    /// Всё прочитанное проверяется: номер состояния должен быть в таблице
    /// блоков, номер биома — в реестре, а записи секции покрывать её ровно.
    /// Испорченная запись даёт None, а не панику под замком мира и не чужие
    /// блоки у клиента.
    pub(super) fn from_legacy(bytes: &[u8]) -> Option<Self> {
        let version = *bytes.first()?;
        let wide = version == CHUNK_VERSION_WIDE;

        if version != LEGACY_VERSION && version != CHUNK_VERSION_FLAT_BIOMES && !wide {
            return None;
        }

        let mut reader = Reader::new(bytes.get(1..)?);
        let mut chunk = Chunk::new();

        for section in 0..SECTIONS as usize {
            if reader.u8().ok()? == 0 {
                continue;
            }

            let runs = reader.u16().ok()?;
            let blocks = chunk.filled_section(section);

            let mut at = 0usize;

            for _ in 0..runs {
                let state = if wide {
                    Packed::try_from(reader.i32().ok()?).ok()?
                } else {
                    reader.u16().ok()?
                };

                if unpack(state) != AIR && crate::blocks::block_at_state(unpack(state)).is_none() {
                    return None;
                }

                let length = reader.u16().ok()? as usize;

                blocks.get_mut(at..at + length)?.fill(state);
                at += length;
            }

            if at != SECTION_BLOCKS {
                return None;
            }
        }

        let biomes = terrain::biome_registry_size();

        if version == LEGACY_VERSION {
            // Биомы объёмные — записи «номер и сколько подряд». Они обязаны
            // покрыть все клетки ровно: иначе запись испорчена.
            let runs = reader.u16().ok()?;
            let mut at = 0usize;

            for _ in 0..runs {
                let biome = reader.u16().ok()?;
                let length = reader.u16().ok()? as usize;

                if biome as usize >= biomes {
                    return None;
                }

                chunk.biomes.get_mut(at..at + length)?.fill(biome);
                at += length;
            }

            if at != chunk.biomes.len() {
                return None;
            }

            // Хранилища, если записаны. Хранилище на месте блока другого
            // вида (файл правили руками) пропускается.
            let mut rest = reader.rest();

            if rest.len() >= 2 {
                let count = u16::from_be_bytes([rest[0], rest[1]]);

                rest = &rest[2..];

                for _ in 0..count {
                    let (place, container) = containers::Container::read(&mut rest, 0, 0)?;

                    if containers::Kind::of(chunk.block(place.0, place.1, place.2)) == Some(container.kind) {
                        chunk.containers.insert(place, container);
                    }
                }
            }

            // Особый динамит, если записан. Вид, которого больше нет в
            // таблице, или не динамит на месте — пропускаются.
            if rest.len() >= 2 {
                let count = u16::from_be_bytes([rest[0], rest[1]]);

                rest = &rest[2..];

                for _ in 0..count {
                    let (record, tail) = rest.split_at_checked(4)?;
                    let place = ((record[0] >> 4) as i32, i16::from_be_bytes([record[1], record[2]]) as i32, (record[0] & 15) as i32);

                    rest = tail;

                    if in_bounds(place.1)
                        && crate::tnt::is_tnt(chunk.block(place.0, place.1, place.2))
                        && let Some(kind) = crate::custom_tnt::by_id(crate::custom_tnt::KindId(record[3]))
                    {
                        chunk.custom_tnt.retain(|(there, _)| *there != place);
                        chunk.custom_tnt.push((place, kind.id));
                    }
                }
            }

            return Some(chunk);
        }

        // Прежние записи: биом один на весь столбец клетки 4×4, по два
        // байта на клетку. Его может и не быть: чанк записан сервером, когда
        // биом был один на весь мир. Тогда остаются нули — это первый биом
        // реестра, и мир от этого не пропадёт.
        for cell in 0..BIOME_LAYER {
            let Ok(biome) = reader.u16() else {
                break;
            };

            if biome as usize >= biomes {
                return None;
            }

            for layer in 0..BIOME_LAYERS {
                chunk.biomes[layer * BIOME_LAYER + cell] = biome;
            }
        }

        Some(chunk)
    }
}

/// Разбивает блоки секции (или биомы чанка) на записи «значение и сколько
/// его подряд».
#[cfg(test)]
fn runs_of(blocks: &[u16]) -> Vec<(u16, u16)> {
    let mut runs: Vec<(u16, u16)> = Vec::new();

    for state in blocks {
        match runs.last_mut() {
            // Подряд одного значения не может быть больше, чем блоков
            // в секции или клеток биома в чанке, так что счётчик не
            // переполнится.
            Some((last, length)) if last == state => *length += 1,
            _ => runs.push((*state, 1)),
        }
    }

    runs
}


/// Разбирает запись чанка старого файла с её места.
fn parse_record(raw: Vec<u8>, sectors: usize) -> region::Stored {
    use region::Stored;

    let Some(length) = raw.get(..4) else {
        return Stored::Damaged { raw, why: "файл региона обрывается на записи чанка" };
    };

    let length = u32::from_be_bytes([length[0], length[1], length[2], length[3]]);
    let checked = length & CHECKED != 0;
    let length = (length & !CHECKED) as usize;
    let start = if checked { 8 } else { 4 };

    if length + start > sectors * SECTOR {
        return Stored::Damaged { raw, why: "в таблице региона указана не та длина чанка" };
    }

    let Some(bytes) = raw.get(start..start + length) else {
        return Stored::Damaged { raw, why: "файл региона обрывается на записи чанка" };
    };

    if checked {
        let sum = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]);
        let mut crc = flate2::Crc::new();

        crc.update(bytes);

        if sum != crc.sum() {
            return Stored::Damaged { raw, why: "не сходится контрольная сумма чанка" };
        }
    }

    Stored::Found(bytes.to_vec())
}

/// Читает старый файл региона целиком: для каждого места таблицы, где
/// что-то есть, — номер места и что там нашлось.
fn read_region(path: &Path) -> io::Result<Vec<(usize, region::Stored)>> {
    let file = fs::read(path)?;
    let mut found = Vec::new();

    for slot in 0..(region::REGION_SIZE * region::REGION_SIZE) as usize {
        let Some(row) = file.get(slot * 4..slot * 4 + 4) else {
            break;
        };

        let offset = u32::from_be_bytes([0, row[0], row[1], row[2]]) as usize;
        let sectors = row[3] as usize;

        if sectors == 0 {
            continue;
        }

        if offset < HEADER_SECTORS {
            found.push((
                slot,
                region::Stored::Damaged { raw: Vec::new(), why: "в таблице региона чанк указан поверх самой таблицы" },
            ));
            continue;
        }

        let from = (offset * SECTOR).min(file.len());
        let to = (from + sectors * SECTOR).min(file.len());

        found.push((slot, parse_record(file[from..to].to_vec(), sectors)));
    }

    Ok(found)
}

/// Неперенесённый старый регион нельзя затереть новой генерацией.
/// Если .mcs уже есть, сохраняется прежнее правило: он имеет приоритет.
/// Ошибка проверки файлов тоже требует отказа, а не генерации.
pub(super) fn pending_region(directory: &Path, chunk_x: i32, chunk_z: i32) -> io::Result<bool> {
    let region_x = chunk_x.div_euclid(region::REGION_SIZE);
    let region_z = chunk_z.div_euclid(region::REGION_SIZE);
    if directory.join(format!("r.{region_x}.{region_z}.mcs")).try_exists()? {
        return Ok(false);
    }
    directory.join(format!("r.{region_x}.{region_z}.rcw")).try_exists()
}

/// Что вышло из переноса одного региона.
struct Moved {
    chunks: usize,
    aside: usize,
    before: u64,
    after: u64,
}

/// Переносит один регион `.rcw` в `.mcs`. Ошибка — старый файл остался как
/// был, а `.mcs` не появился.
fn migrate_region(directory: &Path, old: &Path, region_x: i32, region_z: i32) -> io::Result<Moved> {
    let new = directory.join(format!("r.{}.{}.mcs", region_x, region_z));
    let temporary = directory.join(format!("r.{}.{}.mcs.tmp", region_x, region_z));

    let mut moved = Moved { chunks: 0, aside: 0, before: fs::metadata(old)?.len(), after: 0 };
    let mut kept: Vec<(usize, Vec<u8>, Chunk)> = Vec::new();

    let records = read_region(old)?;
    // Проверяем все целые записи до corrupt-copy и временного .mcs:
    // незнакомая раскладка не означает, что чанк можно выбросить.
    for (slot, stored) in &records {
        if let region::Stored::Found(bytes) = stored
            && let Some(&version) = bytes.first()
            && ![CHUNK_VERSION_WIDE, CHUNK_VERSION_FLAT_BIOMES, LEGACY_VERSION].contains(&version)
        {
            return Err(io::Error::new(io::ErrorKind::Unsupported,
                format!("раскладка старого чанка {version} на месте {slot} не поддержана; регион оставлен без переноса")));
        }
    }

    for (slot, stored) in records {
        let chunk_x = region_x * region::REGION_SIZE + (slot % region::REGION_SIZE as usize) as i32;
        let chunk_z = region_z * region::REGION_SIZE + (slot / region::REGION_SIZE as usize) as i32;

        let (raw, why) = match stored {
            region::Stored::Found(bytes) => match Chunk::from_legacy(&bytes) {
                Some(chunk) => {
                    kept.push((slot, chunk.to_bytes(), chunk));
                    continue;
                }
                None => (bytes, "содержимое чанка не разбирается"),
            },
            region::Stored::Damaged { raw, why } => (raw, why),
            region::Stored::Missing => continue,
        };

        // Испорченное не выбрасывается: если отложить не вышло, старый
        // файл остаётся, и перенос региона откладывается.
        let copy = set_aside(directory, chunk_x, chunk_z, &raw)?;

        log_warn!("Чанк {} {} в {} испорчен ({}): его байты отложены в {}", chunk_x, chunk_z, old.display(), why, copy.display());
        moved.aside += 1;
    }

    let _ = fs::remove_file(&temporary);

    let slots: Vec<(usize, &[u8])> = kept.iter().map(|(slot, bytes, _)| (*slot, bytes.as_slice())).collect();

    region::write_slots(&temporary, &slots)?;

    // Сверка: каждый записанный чанк читается обратно и сходится с тем, что
    // был в старом файле; лишнего в новом нет.
    let mut expected = vec![false; (region::REGION_SIZE * region::REGION_SIZE) as usize];

    for (slot, bytes, chunk) in &kept {
        expected[*slot] = true;

        let same = match region::read_slot(&temporary, *slot)? {
            region::Stored::Found(read) => read == *bytes && Chunk::from_bytes(&read).is_some_and(|back| back.same_content(chunk)),
            _ => false,
        };

        if !same {
            let _ = fs::remove_file(&temporary);

            return Err(io::Error::other(format!("чанк на месте {} после переноса не сошёлся со старым", slot)));
        }
    }

    for (slot, wanted) in expected.iter().enumerate() {
        if !wanted && region::read_slot(&temporary, slot)? != region::Stored::Missing {
            let _ = fs::remove_file(&temporary);

            return Err(io::Error::other(format!("в новом файле на месте {} появился лишний чанк", slot)));
        }
    }

    moved.chunks = kept.len();
    moved.after = fs::metadata(&temporary)?.len();

    // Новый файл встаёт на место и только потом старый уходит.
    fs::rename(&temporary, &new)?;
    region::sync_directory(directory);
    fs::remove_file(old)?;
    region::sync_directory(directory);

    Ok(moved)
}

/// Разбирает имя `r.X.Z.rcw`.
fn parse_name(name: &str) -> Option<(i32, i32)> {
    let inner = name.strip_prefix("r.")?.strip_suffix(".rcw")?;
    let (x, z) = inner.split_once('.')?;

    Some((x.parse().ok()?, z.parse().ok()?))
}

/// Переносит в `.mcs` все регионы `.rcw` папки, у которых `.mcs` ещё нет.
/// Возвращает, сколько регионов перенесено.
pub(super) fn migrate(directory: &Path) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };

    let mut todo = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();

        let Some((x, z)) = parse_name(&name) else {
            continue;
        };

        if directory.join(format!("r.{}.{}.mcs", x, z)).exists() {
            log_warn!(
                "Рядом с {} уже есть регион .mcs — старый файл не тронут; если он больше не нужен, его можно удалить",
                entry.path().display()
            );
            continue;
        }

        todo.push((x, z, entry.path()));
    }

    if todo.is_empty() {
        return 0;
    }

    todo.sort();

    log_info!("Перенос мира в формат .mcs: регионов {}", todo.len());

    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 4).min(todo.len());

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some((x, z, path)) = todo.get(at) else {
                        break;
                    };

                    match migrate_region(directory, path, *x, *z) {
                        Ok(moved) => {
                            done.fetch_add(1, Ordering::Relaxed);

                            log_info!(
                                "Регион {} {}: перенесён в .mcs, чанков {}{}, {:.1} МиБ → {:.1} МиБ",
                                x,
                                z,
                                moved.chunks,
                                if moved.aside > 0 { format!(" (испорченных отложено: {})", moved.aside) } else { String::new() },
                                moved.before as f64 / 1_048_576.0,
                                moved.after as f64 / 1_048_576.0
                            );
                        }
                        Err(e) => log_error!("Регион {} {}: перенести в .mcs не удалось: {} — старый файл .rcw оставлен как был", x, z, e),
                    }
                }
            });
        }
    });

    done.load(Ordering::Relaxed)
}

#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;

    /// Замер формата на настоящих чанках из старых файлов. Запуск:
    /// `MCS_CORPUS=<папка с .rcw> MCS_REGIONS=3 cargo test --release
    /// world_mcs_bench -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn world_mcs_bench() {
        let Ok(corpus) = std::env::var("MCS_CORPUS") else {
            return;
        };
        let regions: usize = std::env::var("MCS_REGIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
        let mut names: Vec<_> = fs::read_dir(&corpus).unwrap().flatten().map(|e| e.path()).collect();

        names.sort();

        let mut old_bytes = Vec::new();
        let mut file_sizes = 0u64;

        for path in names.iter().take(regions) {
            file_sizes += fs::metadata(path).unwrap().len();

            for (_, stored) in read_region(path).unwrap() {
                if let region::Stored::Found(bytes) = stored {
                    old_bytes.push(bytes);
                }
            }
        }

        let count = old_bytes.len();
        println!("чанков: {}, размер .rcw: {:.1} МиБ ({:.1} КиБ на чанк)", count, file_sizes as f64 / 1048576.0, file_sizes as f64 / count as f64 / 1024.0);

        let started = Instant::now();
        let chunks: Vec<Chunk> = old_bytes.iter().map(|bytes| Chunk::from_legacy(bytes).unwrap()).collect();
        println!("старый разбор: {:.1} мкс на чанк", started.elapsed().as_micros() as f64 / count as f64);

        let started = Instant::now();
        let again: Vec<Vec<u8>> = chunks.iter().map(|chunk| chunk.to_legacy_bytes()).collect();
        println!("старая запись: {:.1} мкс на чанк, {:.1} КиБ на чанк", started.elapsed().as_micros() as f64 / count as f64, again.iter().map(Vec::len).sum::<usize>() as f64 / count as f64 / 1024.0);

        let started = Instant::now();
        let payloads: Vec<Vec<u8>> = chunks.iter().map(|chunk| chunk.to_bytes()).collect();
        println!("новая запись без сжатия: {:.1} мкс на чанк, {:.1} КиБ на чанк", started.elapsed().as_micros() as f64 / count as f64, payloads.iter().map(Vec::len).sum::<usize>() as f64 / count as f64 / 1024.0);

        for level in [1, 3, 5, 7, 9, 12, 15, 19] {
            let mut compressor = zstd::bulk::Compressor::new(level).unwrap();
            let started = Instant::now();
            let packed: Vec<Vec<u8>> = payloads.iter().map(|p| compressor.compress(p).unwrap()).collect();
            let took = started.elapsed().as_micros() as f64 / count as f64;
            let total: usize = packed.iter().map(Vec::len).sum();
            // Место с округлением до куска в 256 байт и 16 байт на запись.
            let placed: usize = packed.iter().map(|p| (p.len() + 8).div_ceil(256) * 256).sum();
            println!("zstd {:>2}: {:.1} КиБ на чанк ({:.1} МиБ, с кусками {:.1} МиБ), сжатие {:.0} мкс", level, total as f64 / count as f64 / 1024.0, total as f64 / 1048576.0, placed as f64 / 1048576.0, took);
        }

        // Словарь: учится на других регионах, проверяется на этих.
        if let Ok(size) = std::env::var("MCS_DICT") {
            let size: usize = size.parse().unwrap();
            let mut samples: Vec<Vec<u8>> = Vec::new();

            for path in names.iter().skip(regions + 4).take(3) {
                for (_, stored) in read_region(path).unwrap() {
                    if let region::Stored::Found(bytes) = stored {
                        samples.push(Chunk::from_legacy(&bytes).unwrap().to_bytes());
                    }
                }
            }

            let started = Instant::now();
            let dict = zstd::dict::from_samples(&samples, size).unwrap();
            println!("словарь {} байт, обучение {:.1} с на {} образцах", dict.len(), started.elapsed().as_secs_f64(), samples.len());

            for level in [1, 3, 5, 9] {
                let mut compressor = zstd::bulk::Compressor::with_dictionary(level, &dict).unwrap();
                let started = Instant::now();
                let packed: Vec<Vec<u8>> = payloads.iter().map(|p| compressor.compress(p).unwrap()).collect();
                let took = started.elapsed().as_micros() as f64 / count as f64;
                let total: usize = packed.iter().map(Vec::len).sum();
                let placed: usize = packed.iter().map(|p| (p.len() + 8).div_ceil(256) * 256).sum();
                let mut decompressor = zstd::bulk::Decompressor::with_dictionary(&dict).unwrap();
                let started = Instant::now();
                for p in &packed { decompressor.decompress(p, 1 << 20).unwrap(); }
                println!("со словарём zstd {:>2}: {:.1} КиБ на чанк ({:.1} МиБ, с кусками {:.1} МиБ), сжатие {:.0} мкс, распаковка {:.1} мкс", level, total as f64 / count as f64 / 1024.0, total as f64 / 1048576.0, placed as f64 / 1048576.0, took, started.elapsed().as_micros() as f64 / count as f64);
            }
        }

        let mut compressor = zstd::bulk::Compressor::new(region::LEVEL_FOR_BENCH).unwrap();
        let packed: Vec<Vec<u8>> = payloads.iter().map(|p| compressor.compress(p).unwrap()).collect();
        let started = Instant::now();
        let mut decompressor = zstd::bulk::Decompressor::new().unwrap();
        let mut back = 0usize;

        for p in &packed {
            back += decompressor.decompress(p, 1 << 20).unwrap().len();
        }

        println!("распаковка zstd: {:.1} мкс на чанк ({} байт)", started.elapsed().as_micros() as f64 / count as f64, back);

        let started = Instant::now();

        for p in &payloads {
            Chunk::from_bytes(p).unwrap();
        }

        println!("новый разбор без распаковки: {:.1} мкс на чанк", started.elapsed().as_micros() as f64 / count as f64);

        let started = Instant::now();

        for p in &packed {
            let raw = region::decompress(p).unwrap();
            Chunk::from_bytes(&raw).unwrap();
        }

        println!("новое чтение (распаковка+разбор): {:.1} мкс на чанк", started.elapsed().as_micros() as f64 / count as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mcsheriffanya-migrate-{}-{}", std::process::id(), name));

        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();

        path
    }

    /// Пишет файл региона прежнего вида: записи с суммой, по секторам.
    /// `records` — номер места, байты записи и испортить ли сумму.
    fn write_rcw(path: &Path, records: &[(usize, Vec<u8>, bool)]) {
        let mut file = vec![0u8; SECTOR * HEADER_SECTORS];

        for (slot, bytes, spoil) in records {
            let mut record = ((bytes.len() as u32) | CHECKED).to_be_bytes().to_vec();
            let mut crc = flate2::Crc::new();

            crc.update(bytes);
            record.extend_from_slice(&(crc.sum() ^ u32::from(*spoil)).to_be_bytes());
            record.extend_from_slice(bytes);

            let sectors = record.len().div_ceil(SECTOR);
            let offset = file.len() / SECTOR;

            record.resize(sectors * SECTOR, 0);
            file.extend_from_slice(&record);

            file[slot * 4..slot * 4 + 3].copy_from_slice(&(offset as u32).to_be_bytes()[1..]);
            file[slot * 4 + 3] = sectors as u8;
        }

        fs::write(path, file).unwrap();
    }

    /// Несколько разных чанков: обычный мир, Незер, с хранилищем.
    fn sample_chunks() -> Vec<Chunk> {
        let normal = Generator::Normal(Terrain::new(777, terrain::Style::Vanilla));
        let nether = Generator::Nether(nether::Nether::new(777));
        let mut with_chest = Chunk::flat();

        with_chest.set(2, flat::SURFACE, 3, crate::blocks::state_by_name("chest").unwrap());
        with_chest.containers.get_mut(&(2, flat::SURFACE, 3)).unwrap().slots[4] = Some(crate::inventory::Stack::new(2, 9));

        vec![Chunk::generated(&normal, 1, 1), Chunk::generated(&nether, 2, 0), with_chest, Chunk::new()]
    }

    /// Старый регион переносится: все чанки на месте и те же, `.rcw` ушёл.
    #[test]
    fn a_region_is_migrated_and_checked() {
        let directory = directory("ok");
        let chunks = sample_chunks();
        let slots = [5usize, 6, 1000, 33];
        let records: Vec<_> = chunks.iter().zip(slots).map(|(c, s)| (s, c.to_legacy_bytes(), false)).collect();

        write_rcw(&directory.join("r.-1.2.rcw"), &records);

        assert_eq!(migrate(&directory), 1);

        assert!(!directory.join("r.-1.2.rcw").exists(), "старый файл остался");
        assert!(!directory.join("r.-1.2.mcs.tmp").exists(), "остался временный файл");
        assert!(directory.join("r.-1.2.mcs").exists());

        for (chunk, slot) in chunks.iter().zip(slots) {
            let chunk_x = -32 + (slot % 32) as i32;
            let chunk_z = 64 + (slot / 32) as i32;
            let bytes = region::read(&directory, chunk_x, chunk_z).unwrap().expect("чанк пропал");

            assert!(Chunk::from_bytes(&bytes).unwrap().same_content(chunk), "чанк {} изменился при переносе", slot);
        }

        // Чего не было, того и нет.
        assert_eq!(region::read(&directory, -32, 64).unwrap(), None);

        // Второй запуск ничего не находит.
        assert_eq!(migrate(&directory), 0);
    }

    /// Старые записи первых двух версий переносятся тоже.
    #[test]
    fn oldest_records_are_migrated() {
        let directory = directory("old-versions");
        let stone = crate::blocks::state_by_name("stone").unwrap();
        let mut bytes = vec![CHUNK_VERSION_FLAT_BIOMES];

        for section in 0..SECTIONS {
            if section != 0 {
                bytes.push(0);
                continue;
            }

            bytes.push(1);
            bytes.extend_from_slice(&1u16.to_be_bytes());
            bytes.extend_from_slice(&(stone as u16).to_be_bytes());
            bytes.extend_from_slice(&(SECTION_BLOCKS as u16).to_be_bytes());
        }

        for cell in 0..BIOME_LAYER {
            bytes.extend_from_slice(&(cell as u16 % 3).to_be_bytes());
        }

        write_rcw(&directory.join("r.0.0.rcw"), &[(0, bytes, false)]);

        assert_eq!(migrate(&directory), 1);

        let bytes = region::read(&directory, 0, 0).unwrap().unwrap();
        let chunk = Chunk::from_bytes(&bytes).unwrap();

        assert_eq!(chunk.block(3, MIN_Y + 15, 3), stone);
        assert_eq!(chunk.block(3, MIN_Y + 16, 3), AIR);
    }

    /// Испорченная запись не теряется: она откладывается в `corrupt/`,
    /// а остальное переносится.
    #[test]
    fn damaged_records_are_set_aside() {
        let directory = directory("damaged");
        let good = Chunk::flat();
        let garbage = vec![CHUNK_VERSION_FLAT_BIOMES, 1, 0xFF];

        write_rcw(
            &directory.join("r.0.0.rcw"),
            &[
                (0, good.to_legacy_bytes(), false),
                (1, good.to_legacy_bytes(), true), // сумма не сходится
                (2, garbage.clone(), false),       // сумма верна, содержимое не разбирается
            ],
        );

        assert_eq!(migrate(&directory), 1);
        assert!(!directory.join("r.0.0.rcw").exists());

        let aside: Vec<_> = fs::read_dir(directory.join("corrupt")).expect("нет папки отложенного").flatten().collect();

        assert_eq!(aside.len(), 2, "отложено не всё испорченное");

        let contents: Vec<Vec<u8>> = aside.iter().map(|entry| fs::read(entry.path()).unwrap()).collect();

        assert!(contents.contains(&garbage), "непонятная запись не отложена");
        assert!(contents.iter().any(|bytes| bytes.len() == SECTOR), "запись с плохой суммой отложена не целиком");

        assert!(Chunk::from_bytes(&region::read(&directory, 0, 0).unwrap().unwrap()).unwrap().same_content(&good));
        assert_eq!(region::read(&directory, 1, 0).unwrap(), None);
        assert_eq!(region::read(&directory, 2, 0).unwrap(), None);
    }

    /// Незнакомая версия запрещает перенос всего региона до любых записей,
    /// даже если перед ней попались хороший и повреждённый чанки.
    #[test]
    fn unsupported_legacy_layout_keeps_the_entire_region_untouched() {
        let directory = directory("unsupported-layout");
        let old = directory.join("r.-1.2.rcw");
        for version in [0, CHUNK_VERSION, u8::MAX] {
            write_rcw(&old, &[
                (0, Chunk::flat().to_legacy_bytes(), false),
                (1, vec![LEGACY_VERSION, 1, 255], false),
                (2, vec![version], false),
            ]);
            let before = fs::read(&old).unwrap();
            let error = match migrate_region(&directory, &old, -1, 2) {
                Err(error) => error,
                Ok(_) => panic!("неизвестный регион перенесён"),
            };
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert_eq!(fs::read(&old).unwrap(), before);
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
            assert_eq!(migrate(&directory), 0);
            assert_eq!(fs::read(&old).unwrap(), before);
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        }
        fs::remove_dir_all(directory).unwrap();
    }

    /// Неудачный перенос не должен превращаться в новый .mcs при ensure
    /// или при асинхронной загрузке чанков после World::open.
    #[test]
    fn opening_an_unsupported_legacy_region_cannot_regenerate_it() {
        let path = directory("unsupported-world");
        // Настоящий заголовок мира нужен до добавления старого региона:
        // иначе startup guard остановит пробу раньше чтения .rcw.
        let mut initial = World::open(&path, Some(1), WorldKind::Flat).unwrap();
        initial.save_all();
        drop(initial);
        let regions = path.join(REGIONS);
        fs::create_dir_all(&regions).unwrap();
        let old = regions.join("r.-1.2.rcw");
        write_rcw(&old, &[(0, vec![u8::MAX], false)]);
        let before = fs::read(&old).unwrap();
        let mut world = World::open(&path, Some(1), WorldKind::Flat).unwrap();
        for (x,z) in [(-32,64),(-1,95)] {
            assert!(!world.ensure(x,z));
            let ready = world.source().take(x,z);
            world.accept(x,z,ready);
            assert!(!world.has_chunk(x,z));
            assert!(!world.set_block(x*CHUNK_SIZE,MIN_Y,z*CHUNK_SIZE,1));
        }
        assert!(world.take_new_chunks().is_empty());
        world.save_all();
        assert_eq!(fs::read(&old).unwrap(),before);
        assert_eq!(fs::read_dir(&regions).unwrap().count(),1);
        drop(world);
        fs::remove_dir_all(path).unwrap();
    }

    /// Приоритет уже существующего .mcs сохранён, включая пустые места.
    #[test]
    fn pending_legacy_guard_preserves_existing_mcs_priority() {
        let directory = directory("pending-priority");
        write_rcw(&directory.join("r.-1.2.rcw"), &[(0, vec![u8::MAX], false)]);
        assert!(pending_region(&directory,-32,64).unwrap());
        assert!(pending_region(&directory,-1,95).unwrap());
        assert!(!pending_region(&directory,0,64).unwrap());
        region::write(&directory,-32,64,&Chunk::new().to_bytes()).unwrap();
        assert!(!pending_region(&directory,-1,95).unwrap());
        fs::remove_dir_all(directory).unwrap();
    }

    /// Не вышло отложить испорченное — старый файл остаётся нетронутым, а
    /// нового нет.
    #[test]
    fn a_failed_migration_keeps_the_old_file() {
        let directory = directory("failed");

        write_rcw(&directory.join("r.0.0.rcw"), &[(0, Chunk::flat().to_legacy_bytes(), false), (1, vec![LEGACY_VERSION, 1, 255], false)]);

        // На месте папки отложенного — файл: отложить нечем.
        fs::write(directory.join("corrupt"), "занято").unwrap();

        let before = fs::read(directory.join("r.0.0.rcw")).unwrap();

        assert_eq!(migrate(&directory), 0);
        assert_eq!(fs::read(directory.join("r.0.0.rcw")).unwrap(), before, "старый файл тронут");
        assert!(!directory.join("r.0.0.mcs").exists());
        assert!(!directory.join("r.0.0.mcs.tmp").exists());
    }

    /// Если рядом уже есть `.mcs`, старый файл не трогается и не
    /// переносится поверх.
    #[test]
    fn an_existing_mcs_wins() {
        let directory = directory("existing");
        let chunk = Chunk::flat();

        write_rcw(&directory.join("r.0.0.rcw"), &[(0, chunk.to_legacy_bytes(), false)]);
        region::write(&directory, 5, 5, &[CHUNK_VERSION, 0]).unwrap();

        assert_eq!(migrate(&directory), 0);
        assert!(directory.join("r.0.0.rcw").exists());
        assert_eq!(region::read(&directory, 0, 0).unwrap(), None, ".mcs перезаписан");
    }

    /// Целый мир: старый регион виден игре сразу после открытия, а на диске
    /// остаётся только `.mcs`.
    #[test]
    fn opening_a_world_migrates_it() {
        let path = directory("world");
        let stone = crate::blocks::state_by_name("stone").unwrap();

        // Мир с файлом уровня.
        {
            let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

            world.ensure(0, 0);
            world.save_all();
        }

        let regions = path.join(REGIONS);
        let mut chunk = Chunk::flat();

        chunk.set(4, flat::SURFACE + 5, 6, stone);

        fs::remove_file(regions.join("r.0.0.mcs")).unwrap();
        write_rcw(&regions.join("r.0.0.rcw"), &[(0, chunk.to_legacy_bytes(), false)]);

        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся заново");

        world.ensure(0, 0);

        assert_eq!(world.get_block(4, flat::SURFACE + 5, 6), stone, "правка старого региона потерялась");
        assert!(!regions.join("r.0.0.rcw").exists());
        assert!(regions.join("r.0.0.mcs").exists());

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }
}
