// Запись чанка для файла региона (`.mcs`, см. region.rs).
//
// Содержимое своё, ничего общего с игрой. Чанк — это блоки 24 секций, потом
// биомы тех же 24 секций, потом хранилища и особый динамит. И блоки, и биомы
// лежат одинаково, «контейнером» с одним из четырёх видов:
//
//   0            секция целиком из воздуха (только у блоков)
//   1  v         во всей секции одно значение v (два байта)
//   2  n  P  D   палитра из n значений (два байта на значение), а в D —
//                номера по палитре, каждый в `bits_for(n)` бит подряд
//                (1, 2, 4, 8, потом 9–12), младшими битами вперёд
//   3  D         палитра не окупается — значения прямо, по два байта
//
// Так же устроены ячейки палитры у игры (minecraft.wiki, Chunk format:
// «paletted container») — но раскладка байтов тут другая и своя. Палитра
// идёт в порядке первого появления значения: у слоистой земли номера
// растут вместе с высотой и хорошо жмутся.
//
// Сжимает всю запись чанка уже регион (zstd), поэтому здесь нет ни
// счётчиков повторов, ни заботы о выравнивании: повторы и однообразие
// выбирает сжатие.

use super::*;

use std::cell::RefCell;

/// Секция целиком из воздуха.
const AIR_SECTION: u8 = 0;

/// Одно значение на всю секцию.
const SINGLE: u8 = 1;

/// Палитра и номера по ней.
const PALETTED: u8 = 2;

/// Значения прямо.
const DIRECT: u8 = 3;

thread_local! {
    /// Справочник «значение → место в палитре плюс один» — на 65536
    /// значений; ноль значит «в палитре ещё нет». Заводится один раз на
    /// поток и после каждой секции чистится по палитре.
    static LOOKUP: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
}

/// Сколько бит на номер при палитре из n значений (n больше единицы).
///
/// Ширина округляется вверх до 1, 2, 4 или 8 бит, а дальше — как есть (9–12).
/// Номера на границе байта сжатие берёт заметно лучше, чем «плавающие»
/// по три, пять или семь бит: замер на настоящих чанках дал на четыре
/// процента меньше файла ценой пятой части лишних байт до сжатия.
fn bits_for(n: usize) -> u32 {
    match usize::BITS - (n - 1).leading_zeros() {
        3 => 4,
        5..=7 => 8,
        bits => bits,
    }
}

/// Сколько байт занимают `count` номеров по `bits` бит.
fn packed_len(count: usize, bits: u32) -> usize {
    (count * bits as usize).div_ceil(8)
}

/// Записывает значения контейнером самого короткого вида.
fn write_container(out: &mut Vec<u8>, values: &[u16], lookup: &mut [u16]) {
    let first = values[0];

    if values.iter().all(|value| *value == first) {
        out.push(SINGLE);
        out.extend_from_slice(&first.to_be_bytes());
        return;
    }

    // Палитра в порядке первого появления. Значений в ней не больше, чем
    // самих значений, так что u16 хватает.
    let mut palette: Vec<u16> = Vec::new();

    for value in values {
        if lookup[*value as usize] == 0 {
            palette.push(*value);
            lookup[*value as usize] = palette.len() as u16;
        }
    }

    let bits = bits_for(palette.len());
    let paletted = 1 + 2 + palette.len() * 2 + packed_len(values.len(), bits);

    if paletted > values.len() * 2 {
        for value in &palette {
            lookup[*value as usize] = 0;
        }

        out.push(DIRECT);

        for value in values {
            out.extend_from_slice(&value.to_be_bytes());
        }

        return;
    }

    out.push(PALETTED);
    out.extend_from_slice(&(palette.len() as u16).to_be_bytes());

    for value in &palette {
        out.extend_from_slice(&value.to_be_bytes());
    }

    // Номера, младшими битами вперёд.
    let (mut buffer, mut filled) = (0u64, 0u32);

    for value in values {
        buffer |= u64::from(lookup[*value as usize] - 1) << filled;
        filled += bits;

        while filled >= 8 {
            out.push(buffer as u8);
            buffer >>= 8;
            filled -= 8;
        }
    }

    if filled > 0 {
        out.push(buffer as u8);
    }

    for value in &palette {
        lookup[*value as usize] = 0;
    }
}

/// Читает контейнер в `dest`. `valid` говорит, годится ли значение: номер
/// состояния должен быть в таблице блоков, номер биома — в реестре. None —
/// запись испорчена.
fn read_container(reader: &mut Reader, dest: &mut [u16], valid: &dyn Fn(u16) -> bool) -> Option<()> {
    match reader.u8().ok()? {
        SINGLE => {
            let value = reader.u16().ok()?;

            if !valid(value) {
                return None;
            }

            dest.fill(value);
        }
        PALETTED => {
            let count = reader.u16().ok()? as usize;

            // Палитра из одного значения — это уже вид 1, а больше значений,
            // чем клеток, не бывает.
            if count < 2 || count > dest.len() {
                return None;
            }

            let mut palette = Vec::with_capacity(count);

            for _ in 0..count {
                let value = reader.u16().ok()?;

                if !valid(value) {
                    return None;
                }

                palette.push(value);
            }

            let bits = bits_for(count);
            let data = reader.slice(packed_len(dest.len(), bits)).ok()?;
            let (mut buffer, mut filled, mut next) = (0u64, 0u32, 0usize);
            let mask = (1u64 << bits) - 1;

            for cell in dest.iter_mut() {
                while filled < bits {
                    buffer |= u64::from(*data.get(next)?) << filled;
                    filled += 8;
                    next += 1;
                }

                *cell = *palette.get((buffer & mask) as usize)?;
                buffer >>= bits;
                filled -= bits;
            }
        }
        DIRECT => {
            for cell in dest.iter_mut() {
                let value = reader.u16().ok()?;

                if !valid(value) {
                    return None;
                }

                *cell = value;
            }
        }
        _ => return None,
    }

    Some(())
}

impl Chunk {
    /// Записывает чанк в байты для файла региона (до сжатия).
    pub(super) fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 * 1024);

        out.push(CHUNK_VERSION);

        LOOKUP.with(|lookup| {
            let mut lookup = lookup.borrow_mut();

            if lookup.is_empty() {
                lookup.resize(1 << 16, 0);
            }

            for section in &self.sections {
                match section {
                    // Секция из одного воздуха — то же, что пустая.
                    Some(blocks) if blocks.iter().any(|state| unpack(*state) != AIR) => {
                        write_container(&mut out, blocks, &mut lookup)
                    }
                    _ => out.push(AIR_SECTION),
                }
            }

            for cells in self.biomes.chunks(SECTION_BIOMES) {
                write_container(&mut out, cells, &mut lookup);
            }
        });

        // Хранилища — последними, и только если они есть. Порядок — по
        // месту, чтобы одинаковый чанк давал одинаковые байты. Если дальше
        // идёт что-то ещё, число хранилищ пишется и нулём.
        if !self.containers.is_empty() || !self.custom_tnt.is_empty() || !self.frames.is_empty() || !self.mobs.is_empty() || !self.command_blocks.is_empty() || !self.spawners.is_empty() || !self.signs.is_empty() {
            let mut places: Vec<_> = self.containers.keys().copied().collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for place in places {
                self.containers[&place].write(place, &mut out);
            }
        }

        // Особый динамит — после хранилищ (их число тогда пишется и
        // нулём): сколько, потом место и номер вида. Если дальше рамки —
        // число пишется и нулём.
        if !self.custom_tnt.is_empty() || !self.frames.is_empty() || !self.mobs.is_empty() || !self.command_blocks.is_empty() || !self.spawners.is_empty() || !self.signs.is_empty() {
            let mut places: Vec<_> = self.custom_tnt.iter().map(|&(place, kind)| (place, kind.0)).collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for ((x, y, z), kind) in places {
                out.push(((x << 4) | z) as u8);
                out.extend_from_slice(&(y as i16).to_be_bytes());
                out.push(kind);
            }
        }

        // Рамки для предметов (frame.rs): сколько и записи по порядку места,
        // чтобы одинаковый чанк давал одинаковые байты. Если дальше мобы —
        // число пишется и нулём.
        if !self.frames.is_empty() || !self.mobs.is_empty() || !self.command_blocks.is_empty() || !self.spawners.is_empty() || !self.signs.is_empty() {
            let mut frames: Vec<&crate::frame::Frame> = self.frames.iter().collect();

            frames.sort_unstable_by_key(|frame| (frame.pos, frame.facing));
            out.extend_from_slice(&(frames.len() as u16).to_be_bytes());

            for frame in frames {
                crate::frame::write(frame, &mut out);
            }
        }

        // Мобы (mob/mod.rs): сколько и записи по порядку. Если дальше
        // командные блоки — число пишется и нулём.
        if !self.mobs.is_empty() || !self.command_blocks.is_empty() || !self.spawners.is_empty() || !self.signs.is_empty() {
            out.extend_from_slice(&(self.mobs.len() as u16).to_be_bytes());

            for mob in &self.mobs {
                mob.write(&mut out);
            }
        }

        // Командные блоки (command_block.rs), по порядку места. Если дальше
        // спавнеры — число пишется и нулём.
        if !self.command_blocks.is_empty() || !self.spawners.is_empty() || !self.signs.is_empty() {
            let mut places: Vec<_> = self.command_blocks.keys().copied().collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for place in places {
                self.command_blocks[&place].write(place, &mut out);
            }
        }

        // Спавнеры (spawner.rs), по порядку места. Если дальше таблички —
        // число пишется и нулём.
        if !self.spawners.is_empty() || !self.signs.is_empty() {
            let mut places: Vec<_> = self.spawners.keys().copied().collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for place in places {
                self.spawners[&place].write(place, &mut out);
            }
        }

        // Таблички (sign.rs) — последними, по порядку места.
        if !self.signs.is_empty() {
            let mut places: Vec<_> = self.signs.keys().copied().collect();

            places.sort_unstable();
            out.extend_from_slice(&(places.len() as u16).to_be_bytes());

            for place in places {
                self.signs[&place].write(place, &mut out);
            }
        }

        out
    }

    /// Читает чанк из байтов файла региона (уже распакованных).
    ///
    /// Всё прочитанное проверяется: номер состояния должен быть в таблице
    /// блоков, номер биома — в реестре, палитра и номера — сходиться. Испорченная запись
    /// даёт None, а не панику под замком мира и не чужие блоки у клиента.
    pub(super) fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if *bytes.first()? != CHUNK_VERSION {
            return None;
        }

        let mut reader = Reader::new(bytes.get(1..)?);
        let mut chunk = Chunk::new();

        let state_ok = |state: u16| unpack(state) == AIR || crate::blocks::block_at_state(unpack(state)).is_some();

        for section in 0..SECTIONS as usize {
            // Вид секции подглядываем: воздушная секции не заводится.
            if reader.peek()? == AIR_SECTION {
                reader.u8().ok()?;
                continue;
            }

            let blocks = chunk.filled_section(section);

            read_container(&mut reader, blocks, &state_ok)?;
        }

        let biomes = terrain::biome_registry_size();

        for cells in chunk.biomes.chunks_mut(SECTION_BIOMES) {
            read_container(&mut reader, cells, &|biome| (biome as usize) < biomes)?;
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

        // Рамки, если записаны. Испорченная запись — весь чанк испорчен.
        if rest.len() >= 2 {
            let count = u16::from_be_bytes([rest[0], rest[1]]);

            rest = &rest[2..];

            for _ in 0..count {
                let frame = crate::frame::read(&mut rest, 0, 0, 0)?;

                chunk.frames.push(frame);
            }
        }

        // Мобы, если записаны. Испорченная запись — весь чанк испорчен.
        if rest.len() >= 2 {
            let count = u16::from_be_bytes([rest[0], rest[1]]);

            rest = &rest[2..];

            for _ in 0..count {
                let mob = crate::mob::Mob::read(&mut rest)?;

                chunk.mobs.push(mob);
            }
        }

        // Командные блоки, если записаны. Запись на месте другого блока
        // (файл правили руками) пропускается.
        if rest.len() >= 2 {
            let count = u16::from_be_bytes([rest[0], rest[1]]);

            rest = &rest[2..];

            for _ in 0..count {
                let (place, data) = crate::command_block::Data::read(&mut rest)?;

                if in_bounds(place.1) && crate::command_block::mode_of(chunk.block(place.0, place.1, place.2)).is_some() {
                    chunk.command_blocks.insert(place, data);
                }
            }
        }

        // Спавнеры, если записаны.
        if rest.len() >= 2 {
            let count = u16::from_be_bytes([rest[0], rest[1]]);

            rest = &rest[2..];

            for _ in 0..count {
                let (place, data) = crate::spawner::Data::read(&mut rest)?;

                if in_bounds(place.1) {
                    chunk.spawners.insert(place, data);
                }
            }
        }

        // Таблички, если записаны. Запись на месте другого блока пропускается.
        if rest.len() >= 2 {
            let count = u16::from_be_bytes([rest[0], rest[1]]);

            rest = &rest[2..];

            for _ in 0..count {
                let (place, data) = crate::sign::Data::read(&mut rest)?;

                if in_bounds(place.1) && crate::redstone::is_sign(chunk.block(place.0, place.1, place.2)) {
                    chunk.signs.insert(place, data);
                }
            }
        }

        Some(chunk)
    }

    /// Одинаковы ли чанки по содержимому: блоки (секция из одного воздуха
    /// и отсутствующая секция — одно и то же), биомы, хранилища, особый
    /// динамит. Нужна переносу миров (legacy.rs) для сверки.
    pub(super) fn same_content(&self, other: &Chunk) -> bool {
        let air = [pack(AIR); SECTION_BLOCKS];

        let blocks = self.sections.iter().zip(&other.sections).all(|(a, b)| {
            let a: &[Packed] = a.as_deref().unwrap_or(&air);
            let b: &[Packed] = b.as_deref().unwrap_or(&air);

            a == b
        });

        let mut mine = self.custom_tnt.clone();
        let mut theirs = other.custom_tnt.clone();

        mine.sort_unstable_by_key(|(place, kind)| (*place, kind.0));
        theirs.sort_unstable_by_key(|(place, kind)| (*place, kind.0));

        let mut my_frames = self.frames.clone();
        let mut their_frames = other.frames.clone();

        my_frames.sort_unstable_by_key(|frame| (frame.pos, frame.facing));
        their_frames.sort_unstable_by_key(|frame| (frame.pos, frame.facing));

        blocks && self.biomes == other.biomes && self.containers == other.containers && mine == theirs && my_frames == their_frames && self.command_blocks == other.command_blocks && self.spawners == other.spawners && self.signs == other.signs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Номера состояний, которые есть в таблице блоков.
    fn valid_states(count: usize) -> Vec<u16> {
        (0..30_000i32)
            .filter(|state| crate::blocks::block_at_state(*state).is_some())
            .map(|state| state as u16)
            .take(count)
            .collect()
    }

    /// Чанк записывается и читается обратно тем же, а повторная запись даёт
    /// те же байты.
    fn assert_round_trip(chunk: &Chunk) -> Vec<u8> {
        let bytes = chunk.to_bytes();
        let back = Chunk::from_bytes(&bytes).expect("чанк не прочитался");

        assert!(back.same_content(chunk), "прочитанный чанк отличается");
        assert_eq!(back.to_bytes(), bytes, "повторная запись дала другие байты");

        // И через файл региона, со сжатием. Папка — своя у каждой проверки:
        // проверки идут разом, и общий файл региона они переписывали бы
        // друг у друга (в CI так и падало: «файл обрывается на записи»).
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let directory = std::env::temp_dir().join(format!(
            "mcsheriffanya_codec_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));

        region::write(&directory, 1, 2, &bytes).unwrap();

        let stored = region::read(&directory, 1, 2).unwrap().expect("чанка нет в файле");

        assert_eq!(stored, bytes);

        let _ = std::fs::remove_dir_all(&directory);

        bytes
    }

    /// Рамки идут в конец записи, после хранилищ и особого динамита (их числа
    /// тогда пишутся и нулями), и читаются обратно; чанк без рамок остаётся
    /// таким же коротким, как был.
    #[test]
    fn frames_round_trip_after_the_other_records() {
        let plain = Chunk::flat().to_bytes();
        let mut chunk = Chunk::flat();
        let mut hung = crate::frame::Frame::new(0, (3, 70, 4), crate::frame::NORTH, false);

        hung.item = Some(crate::inventory::Stack::new(1, 1));
        hung.rotation = 3;
        chunk.frames.push(hung.clone());
        chunk.frames.push(crate::frame::Frame::new(0, (9, 71, 4), crate::frame::DOWN, true));

        let bytes = assert_round_trip(&chunk);

        // Число хранилищ и число особого динамита — нули, потом две рамки.
        assert_eq!(&bytes[plain.len()..plain.len() + 4], &[0, 0, 0, 0]);
        assert_eq!(&bytes[plain.len() + 4..plain.len() + 6], &[0, 2]);

        let back = Chunk::from_bytes(&bytes).expect("прочиталось");

        assert!(back.frames.iter().any(|frame| frame.pos == hung.pos && frame.rotation == 3 && frame.item == hung.item));
        assert_eq!(assert_round_trip(&Chunk::flat()), plain);
    }

    /// Мобы идут после рамок (их число тогда пишется и нулём) и читаются
    /// обратно; чанк без мобов остаётся таким же коротким, как был.
    #[test]
    fn mobs_round_trip_after_the_frames() {
        let plain = Chunk::flat().to_bytes();
        let mut chunk = Chunk::flat();
        let mut cow = crate::mob::Mob::new(0, crate::mob::Kind::Cow, (3.5, 70.0, 4.5));

        cow.age = -500;
        cow.vitals.health = 6.0;
        chunk.mobs.push(cow);
        chunk.mobs.push(crate::mob::Mob::new(0, crate::mob::Kind::Chicken, (9.25, 71.0, 4.5)));

        let bytes = assert_round_trip(&chunk);

        // Хранилищ, динамита и рамок нет — три нуля по два байта, потом два моба.
        assert_eq!(&bytes[plain.len()..plain.len() + 8], &[0, 0, 0, 0, 0, 0, 0, 2]);

        let back = Chunk::from_bytes(&bytes).expect("прочиталось");

        assert_eq!(back.mobs.len(), 2);
        assert_eq!(back.mobs[0].kind, crate::mob::Kind::Cow);
        assert_eq!((back.mobs[0].age, back.mobs[0].vitals.health), (-500, 6.0));
        assert_eq!(assert_round_trip(&Chunk::flat()), plain);
    }

    #[test]
    fn overworld_chunks_round_trip() {
        let generator = Generator::Normal(Terrain::new(12345, terrain::Style::Vanilla));

        for (x, z) in [(0, 0), (-3, 5), (40, -17), (7, 7)] {
            assert_round_trip(&Chunk::generated(&generator, x, z));
        }
    }

    #[test]
    fn nether_chunks_round_trip() {
        let generator = Generator::Nether(nether::Nether::new(12345));

        for (x, z) in [(0, 0), (-6, 2), (11, -9)] {
            assert_round_trip(&Chunk::generated(&generator, x, z));
        }
    }

    /// Пустой чанк и секции из одного воздуха занимают по байту, а секция
    /// из одного камня — три.
    #[test]
    fn air_and_single_sections() {
        let empty = Chunk::new();
        let bytes = assert_round_trip(&empty);

        // Версия, 24 воздушные секции и 24 биома по три байта.
        assert_eq!(bytes.len(), 1 + 24 + 24 * 3);
        assert!(bytes[1..25].iter().all(|kind| *kind == AIR_SECTION));

        // Секция, заведённая, но целиком из воздуха, пишется как воздушная.
        let mut chunk = Chunk::new();
        let stone = crate::blocks::state_by_name("stone").unwrap();

        chunk.set(1, 5, 1, stone);
        chunk.set(1, 5, 1, AIR);

        assert!(chunk.sections.iter().any(Option::is_some));
        assert_eq!(assert_round_trip(&chunk), bytes);

        // Секция целиком из камня — одно значение.
        for index in 0..SECTION_BLOCKS {
            chunk.filled_section(3)[index] = pack(stone);
        }

        let bytes = assert_round_trip(&chunk);

        assert_eq!(bytes[1 + 3], SINGLE);
        assert_eq!(&bytes[1 + 4..1 + 6], &(stone as u16).to_be_bytes()[..]);
    }

    /// Палитры разной ширины читаются обратно, а ширина выбирается по числу
    /// значений.
    #[test]
    fn palettes_of_every_width() {
        assert_eq!(
            [2, 3, 4, 5, 8, 9, 16, 17, 256, 257, 1000, 4096].map(bits_for),
            [1, 2, 2, 4, 4, 4, 4, 8, 8, 9, 10, 12]
        );

        let states = valid_states(4096);

        for distinct in [2usize, 3, 5, 8, 9, 17, 100, 256, 257, 1000] {
            let mut chunk = Chunk::new();
            let blocks = chunk.filled_section(4);

            for (index, block) in blocks.iter_mut().enumerate() {
                // Значения идут вперемешку, но каждое встречается.
                *block = states[(index * 7 + index / distinct) % distinct];
            }

            let bytes = assert_round_trip(&chunk);

            assert_eq!(bytes[1 + 4], PALETTED, "{} значений — не палитра", distinct);
        }
    }

    /// Когда палитра не окупается, значения идут прямо; самая большая
    /// палитра — все блоки секции разные.
    #[test]
    fn direct_mode_and_the_largest_palette() {
        let states = valid_states(4096);

        assert_eq!(states.len(), 4096, "в таблице блоков не хватает состояний для проверки");

        for distinct in [1500usize, 2048, 4096] {
            let mut chunk = Chunk::new();
            let blocks = chunk.filled_section(0);

            for (index, block) in blocks.iter_mut().enumerate() {
                *block = states[index % distinct];
            }

            let bytes = assert_round_trip(&chunk);

            assert_eq!(bytes[1], DIRECT, "{} значений — не прямая запись", distinct);
        }

        // Биомы тоже: 64 разных номера в секции — палитра шире байта.
        let mut chunk = Chunk::new();
        let registry = terrain::biome_registry_size() as BiomeId;

        for (cell, biome) in chunk.biomes.iter_mut().enumerate() {
            *biome = (cell as BiomeId * 5) % registry;
        }

        assert_round_trip(&chunk);
    }

    /// Хранилища и особый динамит записываются вместе с блоками.
    #[test]
    fn block_entities_survive() {
        let mut chunk = Chunk::flat();
        let chest = crate::blocks::state_by_name("chest").unwrap();
        let barrel = crate::blocks::state_by_name("barrel").unwrap();
        let (x, y, z) = (3, flat::SURFACE, 9);

        chunk.set(x, y, z, chest);
        chunk.set(12, flat::SURFACE + 2, 4, barrel);

        let container = chunk.containers.get_mut(&(x, y, z)).expect("сундук без хранилища");

        container.slots[0] = Some(crate::inventory::Stack::new(1, 64));
        container.slots[26] = Some(crate::inventory::Stack::new(5, 3));

        let tnt = crate::blocks::state_by_name("tnt").unwrap();

        if let Some(kind) = crate::custom_tnt::by_id(crate::custom_tnt::KindId(1)) {
            chunk.set(8, flat::SURFACE, 8, tnt);
            chunk.custom_tnt.push(((8, flat::SURFACE, 8), kind.id));
        }

        assert_eq!(chunk.containers.len(), 2);

        let bytes = assert_round_trip(&chunk);
        let back = Chunk::from_bytes(&bytes).unwrap();

        assert_eq!(back.containers[&(3, flat::SURFACE, 9)].slots[26], Some(crate::inventory::Stack::new(5, 3)));
        assert_eq!(back.custom_tnt.len(), chunk.custom_tnt.len());
    }

    /// Сжатый настоящий чанк занимает в разы меньше прежней записи.
    #[test]
    fn a_real_chunk_is_much_smaller_than_before() {
        let generator = Generator::Normal(Terrain::new(12345, terrain::Style::Vanilla));
        let chunk = Chunk::generated(&generator, 2, 3);
        let old = chunk.to_legacy_bytes().len();
        let new = region::compress(&chunk.to_bytes()).unwrap().len();

        assert!(new * 3 < old, "было {} байт, стало {}", old, new);
    }
}
