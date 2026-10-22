//! Старое представление того же снимка World и того же рассчитанного света.
use super::*;
use mcsa_protocol::{legacy_blocks::{self, SourceSection, View}, legacy_chunk::{Chunk, HeightWindow}};

pub(super) fn snapshot(world: &Mutex<World>, x: i32, z: i32, light: &ChunkLight, window: HeightWindow, sky: bool) -> io::Result<Chunk> {
    let (sections, biomes) = {
        let world = world.lock().expect("мир захвачен другим потоком");
        let sections = world.chunk_packet(x,z,|sections,_| sections.to_vec())
            .unwrap_or_else(|| vec![None; world::SECTIONS as usize]);
        let mut biomes = [0;256];
        for dz in 0..16 { for dx in 0..16 {
            let wx = x * 16 + dx; let wz = z * 16 + dz;
            // Старый биом двумерный: цвет столбца определяется поверхностью,
            // не выбором правил для клиента и не заменой биома в самом мире.
            let y = world.column_top(wx,wz).unwrap_or(63).clamp(world::MIN_Y,world::MIN_Y + world::SECTIONS * 16 - 1);
            let name = world.mob_biome(wx,y,wz);
            biomes[(dz*16+dx) as usize] = mcsa_protocol::legacy_biomes::id(name)
                .ok_or_else(|| io::Error::other(format!("Нет представления биома {name} для1.12.2")))?;
        } }
        (sections,biomes)
    };
    // NBT не пересылается как будто его формат совпал. Пока такой чанк
    // отклоняется, а не становится сундуком/табличкой с потерянными данными.
    let first = window.origin().checked_sub(world::MIN_Y).filter(|v| *v >= 0)
        .ok_or_else(|| io::Error::other("Окно1.12.2 вне мира"))? as usize / 16;
    let selected = sections.get(first..first+16).ok_or_else(|| io::Error::other("Окно1.12.2 вне мира"))?;
    let mut seen = [0u64; 1024];
    for blocks in selected.iter().flatten() { for &state in blocks.iter() {
        let index = usize::from(state) / 64; let bit = 1u64 << (state % 64);
        if seen[index] & bit != 0 { continue; } seen[index] |= bit;
        if state != 0 && (containers::Kind::of(i32::from(state)).is_some() || redstone::block_entity_type(i32::from(state)).is_some()) {
            return Err(io::Error::other("Перевод данных блок-сущности в1.12.2 ещё не поддерживается"));
        }
    } }
    if light.block.len() != sections.len() || light.sky.len() != sections.len() {
        return Err(io::Error::other("Размер света не соответствует снимку мира"));
    }
    let block: Vec<_> = light.block.iter().map(|v| v.nibbles()).collect();
    let sky_arrays: Vec<_> = light.sky.iter().map(|v| v.nibbles()).collect();
    let source: Vec<_> = sections.iter().enumerate().map(|(i,states)| SourceSection {
        blocks: states.as_deref(), block_light: &block[i], sky_light: sky.then_some(sky_arrays[i].as_slice()),
    }).collect();
    legacy_blocks::project(View { version: mcsa_protocol::wire::INTERNAL, x,z,min_y:world::MIN_Y,window,
        sections:&source,biomes,block_entities:vec![] }).map_err(|error| io::Error::other(format!("Чанк1.12.2: {error:?}")))
}
