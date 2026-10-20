//! Совпадающие состояния340↔775/777. Неполные свойства не дополняются
//! догадками, неизвестный блок не подменяется воздухом/камнем.
use crate::{JavaVersion, ids::{self, RegistryKind}, legacy_block_ids::{BLOCKS, NATIVE_STATES, SNOWY, SNOW}};

const MISSING: u16 = u16::MAX;
const fn forward() -> [u16; 8192] {
    let mut table = [MISSING; 8192];
    let mut i = 0;
    while i < BLOCKS.len() { table[BLOCKS[i].0 as usize] = BLOCKS[i].1 as u16; i += 1; }
    table
}
const fn reverse() -> [u16; NATIVE_STATES] {
    let mut table = [MISSING; NATIVE_STATES];
    let mut i = 0;
    while i < BLOCKS.len() { table[BLOCKS[i].1 as usize] = BLOCKS[i].0; i += 1; }
    table
}
static TO_NATIVE: [u16; 8192] = forward();
static FROM_NATIVE: [u16; NATIVE_STATES] = reverse();

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedLegacy(u16), UnsupportedModern(u32), Id(ids::IdTranslationError), InvalidView }

fn supported_payload(version: JavaVersion) -> Result<(), Error> {
    if matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3) { Ok(()) }
    else { Err(Error::Id(ids::IdTranslationError::UnsupportedVersion(version))) }
}

pub fn to_modern(state: u16, version: JavaVersion) -> Result<u32, Error> {
    supported_payload(version)?;
    let value = TO_NATIVE.get(usize::from(state)).copied().filter(|v| *v != MISSING).ok_or(Error::UnsupportedLegacy(state))?;
    ids::translate(RegistryKind::BlockState, JavaVersion::V26_1_2, version, u32::from(value)).map_err(Error::Id)
}

pub fn to_legacy(state: u32, version: JavaVersion) -> Result<u16, Error> {
    supported_payload(version)?;
    let value = ids::translate(RegistryKind::BlockState, version, JavaVersion::V26_1_2, state).map_err(Error::Id)?;
    FROM_NATIVE.get(value as usize).copied().filter(|v| *v != MISSING).ok_or(Error::UnsupportedModern(state))
}

/// Ссылки на снимок мира. Распаковка в промежуточный JSON и копия всего мира
/// не нужны; вызывающий удерживает только снимок выбранных секций.
pub struct SourceSection<'a> {
    pub blocks: Option<&'a [u16]>,
    pub block_light: &'a [u8],
    pub sky_light: Option<&'a [u8]>,
}

pub struct View<'a> {
    pub version: JavaVersion,
    pub x: i32,
    pub z: i32,
    pub min_y: i32,
    pub window: crate::legacy_chunk::HeightWindow,
    pub sections: &'a [SourceSection<'a>],
    /// Явные старые двумерные биомы. Проекция трёхмерного реестра — отдельная
    /// политика вызывающего, её нельзя угадать по числовому ID современного.
    pub biomes: [u8; 256],
    /// Уже переведённые именованные NBT с координатами в выбранном окне.
    pub block_entities: Vec<Vec<u8>>,
}

/// Строит полное представление выбранного окна. Отказ атомарный: частично
/// переведённый чанк не возвращается и исходный мир не изменяется.
pub fn project(view: View<'_>) -> Result<crate::legacy_chunk::Chunk, Error> {
    use crate::legacy_chunk::{Chunk, Section};
    supported_payload(view.version)?;
    if view.min_y % 16 != 0 || view.sections.is_empty() || view.sections.len() > 256 { return Err(Error::InvalidView); }
    let start = view.window.origin().checked_sub(view.min_y).filter(|v| *v >= 0).ok_or(Error::InvalidView)? as usize / 16;
    let source = view.sections.get(start..start + 16).ok_or(Error::InvalidView)?;
    let sky = source[0].sky_light.is_some();
    let mut sections = Vec::with_capacity(16);
    for section in source {
        if section.block_light.len() != 2048 || section.sky_light.is_some() != sky
            || section.sky_light.is_some_and(|v| v.len() != 2048)
            || section.blocks.is_some_and(|v| v.len() != 4096) { return Err(Error::InvalidView); }
    }
    for (y, section) in source.iter().enumerate() {
        let blocks = match section.blocks {
            Some(states) => states.iter().enumerate().map(|(index, id)| {
                let native = ids::translate(RegistryKind::BlockState, view.version, JavaVersion::V26_1_2, u32::from(*id)).map_err(Error::Id)?;
                if let Some(value) = FROM_NATIVE.get(native as usize).filter(|v| **v != MISSING) { return Ok(*value); }
                if let Some(&(_, old, snowy)) = SNOWY.iter().find(|row| row.0 == native) {
                    // Верхний сосед за границей окна клиенту недоступен.
                    // Не теряем snowy молча на границе секции/окна.
                    let above = if index < 3840 { states[index + 256] }
                        else { source.get(y + 1).and_then(|s| s.blocks).map_or(0, |s| s[index - 3840]) };
                    let above = ids::translate(RegistryKind::BlockState, view.version, JavaVersion::V26_1_2, u32::from(above)).map_err(Error::Id)?;
                    if snowy != SNOW.contains(&above) { return Err(Error::InvalidView); }
                    Ok(old)
                } else {
                    Err(Error::UnsupportedModern(u32::from(*id)))
                }
            }).collect::<Result<Vec<_>, _>>()?,
            None => vec![to_legacy(0, view.version)?; 4096],
        };
        sections.push((y as u8, Section { blocks, block_light: section.block_light.to_vec(), sky_light: section.sky_light.map(<[u8]>::to_vec) }));
    }
    let chunk = Chunk { x: view.x, z: view.z, sections, biomes: Some(view.biomes), block_entities: view.block_entities };
    crate::legacy_chunk::validate_block_entities(&chunk.block_entities).map_err(|_| Error::InvalidView)?;
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_json_states_round_trip_without_ambiguous_or_neighbour_dependent_fallbacks() {
        assert_eq!(BLOCKS.len(), 896);
        for &(old, native) in BLOCKS {
            assert_eq!(to_modern(old, JavaVersion::V26_1_2), Ok(native));
            assert_eq!(to_legacy(native, JavaVersion::V26_1_2), Ok(old));
            if let Ok(new) = to_modern(old, JavaVersion::V26_3) { assert_eq!(to_legacy(new, JavaVersion::V26_3), Ok(old)); }
        }
        for old in [2*16, 53*16, 64*16, 140*16, 144*16, 175*16, 176*16, 155*16+6, 155*16+10, 8192, u16::MAX] {
            assert!(to_modern(old, JavaVersion::V26_1_2).is_err());
        }
        assert!(to_modern(0, JavaVersion::V26_2).is_err());
        assert_eq!(to_modern(16, JavaVersion::V26_1_2), Ok(1));
        assert_eq!(to_legacy(0, JavaVersion::V26_1_2), Ok(0));
    }

    #[test]
    fn snowy_presentation_requires_visible_neighbour_including_section_and_window_edges() {
        let light = [0; 2048];
        let mut blocks = vec![vec![0u16; 4096]; 16];
        let build = |blocks: &[Vec<u16>]| {
            let source: Vec<_> = blocks.iter().map(|b| SourceSection { blocks: Some(b), block_light: &light, sky_light: None }).collect();
            project(View { version: JavaVersion::V26_1_2, x: 0, z: 0, min_y: -64,
                window: crate::legacy_chunk::HeightWindow::new(-64).unwrap(), sections: &source, biomes: [1; 256], block_entities: vec![] })
        };
        for &(state, old, snowy) in SNOWY {
            for index in [0, 3840] {
                blocks[0][index] = state as u16;
                let (section, above) = if index == 3840 { (1, 0) } else { (0, 256) };
                blocks[section][above] = if snowy { SNOW[0] as u16 } else { 0 };
                assert_eq!(build(&blocks).unwrap().sections[0].1.blocks[index], old);
                blocks[section][above] = if snowy { 0 } else { SNOW[0] as u16 };
                assert_eq!(build(&blocks), Err(Error::InvalidView));
                blocks[0][index] = 0; blocks[section][above] = 0;
            }
            blocks[15][3840] = state as u16;
            assert_eq!(build(&blocks).is_ok(), !snowy);
            blocks[15][3840] = 0;
            assert!(to_legacy(state, JavaVersion::V26_1_2).is_err());
        }
    }

    #[test]
    fn view_rejects_missing_sections_bad_light_and_unmapped_states_atomically() {
        let block = [0; 4096]; let light = [0; 2048];
        let mut source: Vec<_> = (0..24).map(|_| SourceSection { blocks: Some(&block), block_light: &light, sky_light: None }).collect();
        let build = |source: &[SourceSection<'_>], origin| project(View { version: JavaVersion::V26_1_2, x: -1, z: -2, min_y: -64,
            window: crate::legacy_chunk::HeightWindow::new(origin).unwrap(), sections: source, biomes: [1; 256], block_entities: vec![] });
        assert!(build(&source, -64).is_ok()); assert!(build(&source, 64).is_ok());
        assert!(build(&source, 80).is_err()); assert!(build(&source, -80).is_err());
        source[0].block_light = &light[..2047]; assert!(build(&source, -64).is_err());
        source[0].block_light = &light; source[0].sky_light = Some(&light); assert!(build(&source, -64).is_err());
        source[0].sky_light = None;
        let unsupported = [u16::MAX; 4096]; source[0].blocks = Some(&unsupported);
        assert!(build(&source, -64).is_err()); assert!(build(&source, 0).is_ok());
    }
}
