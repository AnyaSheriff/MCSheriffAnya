//! Полный MapChunk775↔777: Wiki Packets oldid3848030, Chunk format3661930.
//! Высота и общий порядок биомов задаются явно, а не угадываются из пакета.
use std::collections::{BTreeMap, BTreeSet};
use crate::{JavaVersion, block_entities, chunk_sections, ids::{self, RegistryKind}, light::Light, registry};

const MAX_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub min_y: i32,
    pub sections: usize,
    /// Обе стороны должны получить биомы в одинаковом порядке.
    pub shared_biome_count: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedVersion(JavaVersion), Invalid, Truncated, TooLarge, Context,
    Sections(chunk_sections::Error), Light(crate::light::Error),
    Entity(block_entities::Error), Nbt(registry::Error), Id(ids::IdTranslationError),
}

pub fn translate(from: JavaVersion, to: JavaVersion, payload: &[u8], context: Context) -> Result<Vec<u8>, Error> {
    for version in [from, to] {
        if !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3) { return Err(Error::UnsupportedVersion(version)); }
    }
    translate_inner(from, to, payload, context)
}

// Только два шага ignored-test export: настоящий native775 builder → bounded776.
// Публичный wrapper выше сохраняет свой прежний список поддержанных версий.
#[cfg(test)]
pub(crate) fn translate_776_fixture(payload: &[u8], context: Context) -> Result<Vec<u8>, Error> {
    translate_inner(JavaVersion::V26_1_2, JavaVersion::V26_2, payload, context)
}

fn translate_inner(from: JavaVersion, to: JavaVersion, payload: &[u8], context: Context) -> Result<Vec<u8>, Error> {
    if payload.len() > MAX_BYTES { return Err(Error::TooLarge); }
    if context.sections == 0 || context.sections > 256 || context.min_y % 16 != 0
        || context.min_y < i32::from(i16::MIN) || context.min_y > i32::from(i16::MAX) - context.sections as i32 * 16 + 1 {
        return Err(Error::Context);
    }
    let height = context.sections as u32 * 16;
    let mut input = Cursor(payload);
    input.take(8)?; // signed X/Z, unchanged in these versions
    let count = input.count(6)?;
    let mut seen_maps = BTreeSet::new();
    let bits = (32 - height.leading_zeros()) as usize;
    for _ in 0..count {
        let kind = input.count(5)?;
        if !seen_maps.insert(kind) { return Err(Error::Invalid); }
        let words = input.count(256)?;
        let per_word = 64 / bits;
        if words != 256usize.div_ceil(per_word) { return Err(Error::Invalid); }
        let bytes = input.take(words * 8)?;
        for (word_index, &word) in bytes.as_chunks::<8>().0.iter().enumerate() {
            let word = u64::from_be_bytes(word);
            for cell in 0..per_word.min(256 - word_index * per_word) {
                if (word >> (cell * bits)) & ((1u64 << bits) - 1) > u64::from(height) { return Err(Error::Invalid); }
            }
        }
    }
    let header_length = payload.len() - input.0.len();
    let size = input.count(MAX_BYTES)?;
    let mut sections = chunk_sections::decode(from, input.take(size)?, context.sections, context.shared_biome_count).map_err(Error::Sections)?;
    let count = input.count(context.sections * 4096)?;
    if (from == JavaVersion::V26_2 || to == JavaVersion::V26_2) && count != 0 {
        return Err(Error::Entity(block_entities::Error::UnsupportedVersion));
    }
    if count > input.0.len() / 5 { return Err(Error::Truncated); }
    let mut entities = BTreeMap::new();
    let mut places = BTreeSet::new();
    for _ in 0..count {
        let xz = input.take(1)?[0];
        let y = i16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?);
        if i32::from(y) < context.min_y || i32::from(y) >= context.min_y + height as i32 || !places.insert((y, xz)) { return Err(Error::Invalid); }
        let kind = input.count(65_535)? as u32;
        let size = registry::nbt_length(input.0).map_err(Error::Nbt)?;
        let data = input.take(size)?;
        if from == JavaVersion::V26_1_2 && kind == 25 {
            let relative_y = (i32::from(y) - context.min_y) as usize;
            let state = sections[relative_y / 16].blocks[(relative_y % 16) * 256 + usize::from(xz & 15) * 16 + usize::from(xz >> 4)];
            if !crate::block_entity_ids::legacy_bed(state) { return Err(Error::Invalid); }
        }
        if let Some(entity) = block_entities::translate(from, to, kind, data).map_err(Error::Entity)? { entities.insert((y, xz), entity); }
    }
    let light = Light::decode(from, input.0).map_err(Error::Light)?;
    for mask in [&light.sky, &light.block, &light.empty_sky, &light.empty_block] {
        for (byte, &value) in mask.iter().enumerate() {
            for bit in 0..8 {
                if value & (1 << bit) != 0 && byte * 8 + bit >= context.sections + 2 { return Err(Error::Invalid); }
            }
        }
    }
    for (section_index, section) in sections.iter_mut().enumerate() {
        for (at, state) in section.blocks.iter_mut().enumerate() {
            *state = ids::translate(RegistryKind::BlockState, from, to, *state).map_err(Error::Id)?;
            if from == JavaVersion::V26_3 && to == JavaVersion::V26_1_2 && crate::block_entity_ids::legacy_bed(*state) {
                let y = (context.min_y + section_index as i32 * 16 + (at / 256) as i32) as i16;
                let xz = (((at & 15) << 4) | ((at >> 4) & 15)) as u8;
                if entities.insert((y, xz), (25, vec![0])).is_some() { return Err(Error::Invalid); }
            }
        }
    }
    let sections = chunk_sections::encode(to, &sections, context.shared_biome_count).map_err(Error::Sections)?;
    let mut out = payload[..header_length].to_vec();
    uint(&mut out, sections.len() as u32); out.extend(sections);
    uint(&mut out, entities.len() as u32);
    for ((y, xz), (kind, data)) in entities {
        out.push(xz); out.extend(y.to_be_bytes()); uint(&mut out, kind); out.extend(data);
    }
    out.extend(light.encode(to).map_err(Error::Light)?);
    if out.len() > MAX_BYTES { return Err(Error::TooLarge); }
    Ok(out)
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], Error> {
        let (part, rest) = self.0.split_at_checked(size).ok_or(Error::Truncated)?; self.0 = rest; Ok(part)
    }
    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf8 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return if value as usize <= limit { Ok(value as usize) } else { Err(Error::TooLarge) }; }
        }
        Err(Error::Invalid)
    }
}
fn uint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; } out.push(value as u8);
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};
    fn context() -> Context { Context { min_y: -64, sections: 24, shared_biome_count: 59 } }
    fn original() -> Vec<u8> {
        let mut out = Vec::new(); out.extend((-1i32).to_be_bytes()); out.extend(9i32.to_be_bytes());
        // Одна heightmap, 37 long с 9-битными значениями для высоты384.
        out.extend([1, 0, 37]); out.resize(out.len() + 37 * 8, 0);
        let mut sections = vec![chunk_sections::Section { blocks_present: 0, fluids_present: 0, blocks: vec![0; 4096], biomes: vec![7; 64] }; 24];
        sections[0].blocks[1] = (0..32768).find(|s| crate::block_entity_ids::legacy_bed(*s)).unwrap();
        sections[0].blocks_present = 1;
        let bytes = chunk_sections::encode(OLD, &sections, 59).unwrap();
        uint(&mut out, bytes.len() as u32); out.extend(bytes);
        out.push(3);
        for (xz, kind, nbt) in [(0u8,27u8,&[0][..]),(16,25,&[0][..]),(32,7,&include_bytes!("../testdata/oak_sign-26.1.2.nbt")[..])] {
            out.push(xz); out.extend((-64i16).to_be_bytes()); out.push(kind); out.extend(nbt);
        }
        out.extend(include_bytes!("../testdata/light-26.1.2-placed.bin"));
        out
    }
    fn entity_count(bytes: &[u8]) -> usize {
        let mut input = Cursor(bytes); input.take(8).unwrap();
        let n = input.count(6).unwrap();
        for _ in 0..n { input.count(5).unwrap(); let count = input.count(256).unwrap(); input.take(count * 8).unwrap(); }
        let size = input.count(MAX_BYTES).unwrap(); input.take(size).unwrap(); input.count(100).unwrap()
    }
    #[test]
    fn full_chunk_preserves_header_light_signs_and_restores_legacy_bed_entities() {
        let old = original(); let new = translate(OLD, NEW, &old, context()).unwrap();
        assert_eq!(&old[..307], &new[..307]);
        assert_eq!(entity_count(&old), 3); assert_eq!(entity_count(&new), 2);
        assert_eq!(translate(NEW, OLD, &new, context()).unwrap(), old);
        // Полный путь wire обязан получить реальный контекст измерения.
        let mut packet = vec![0x2d]; packet.extend(&old);
        let translated = crate::wire::translate_with_context(crate::State::Play, crate::Direction::Clientbound, OLD, NEW, &packet,
            crate::wire::Context { chunk: Some(context()), ..Default::default() }).unwrap();
        assert_eq!(translated[0], 0x2e); assert_eq!(&translated[1..], &new);
        assert_eq!(crate::wire::translate(crate::State::Play, crate::Direction::Clientbound, OLD, NEW, &packet), Err(crate::wire::Error::MissingChunkContext));
    }
    #[test]
    fn dimension_heightmaps_lengths_and_trailing_data_are_checked() {
        let old = original();
        for n in (0..24).chain([old.len() / 2, old.len()-1]) { assert!(translate(OLD, NEW, &old[..n], context()).is_err()); }
        let mut bad = old.clone(); bad.push(0); assert!(translate(OLD, NEW, &bad, context()).is_err());
        let mut bad = old.clone(); bad[10] = 36; assert_eq!(translate(OLD, NEW, &bad, context()), Err(Error::Invalid));
        let mut bad = old.clone(); bad[17] = 255; bad[18] = 255; assert_eq!(translate(OLD, NEW, &bad, context()), Err(Error::Invalid));
        for sections in [0, 257, usize::MAX] { assert_eq!(translate(OLD, NEW, &old, Context { sections, ..context() }), Err(Error::Context)); }
        assert!(translate(OLD, NEW, &old, Context { min_y: -63, ..context() }).is_err());
        assert!(translate(OLD, NEW, &old, Context { shared_biome_count: 7, ..context() }).is_err());
    }
}
