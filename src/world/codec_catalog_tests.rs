use super::*;
use crate::gameplay_catalog::{Catalog, Profile, WHITE_SLAB_FIRST_STATE, WHITE_SLAB_LAST_STATE};

fn preview() -> Catalog { Catalog::new(Profile::Preview26_3) }
fn native() -> Catalog { Catalog::new(Profile::Java26_1_2) }

fn assert_preview_roundtrip(chunk: &Chunk, mode: u8, state: u16) {
    let bytes = chunk.to_bytes_with_catalog(preview()).expect("каталог разрешает запись");
    assert_eq!(bytes[1], mode);
    let original = bytes.clone();
    let decoded = Chunk::decode_for_load_with_catalog(&bytes, preview()).expect("preview читается");
    assert!(chunk.same_content(&decoded));
    assert_eq!(decoded.to_bytes_with_catalog(preview()).unwrap(), bytes);
    for result in [Chunk::decode_for_load(&bytes), Chunk::decode_for_load_with_catalog(&bytes, native())] {
        assert!(matches!(result, Err(DecodeError::UnsupportedCatalogState { profile: Profile::Java26_1_2, state: value }) if value == state));
    }
    assert_eq!(bytes, original, "отказ не меняет исходные байты");
    assert_eq!(chunk.to_bytes_with_catalog(native()).unwrap_err(),
        DecodeError::UnsupportedCatalogState { profile: Profile::Java26_1_2, state });
    assert!(chunk.same_content(&decoded), "отказ записи не меняет чанк");
}

#[test]
fn every_slab_state_roundtrips_in_single_paletted_and_direct_modes() {
    let base: Vec<u16> = (0..WHITE_SLAB_FIRST_STATE).filter(|&state|
        crate::blocks::block_at_state(state).is_some()).take(SECTION_BLOCKS).map(pack).collect();
    assert_eq!(base.len(), SECTION_BLOCKS);
    assert!(u16::try_from(WHITE_SLAB_LAST_STATE).is_ok());
    for state in WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE {
        let state = pack(state);
        let mut single = Chunk::new();
        single.filled_section(0).fill(state);
        assert_preview_roundtrip(&single, SINGLE, state);
        let mut paletted = Chunk::new();
        paletted.filled_section(0)[0] = state;
        assert_preview_roundtrip(&paletted, PALETTED, state);
        let mut direct = Chunk::new();
        direct.filled_section(0).copy_from_slice(&base);
        direct.filled_section(0)[0] = state;
        assert_preview_roundtrip(&direct, DIRECT, state);
    }
}

#[test]
fn native_catalog_bytes_stay_identical_and_preview_does_not_accept_unknown_ids() {
    let chunk = Chunk::flat();
    let bytes = chunk.to_bytes();
    assert_eq!(chunk.to_bytes_with_catalog(native()).unwrap(), bytes);
    assert_eq!(chunk.to_bytes_with_catalog(preview()).unwrap(), bytes);
    for catalog in [native(), preview()] {
        let back = Chunk::decode_for_load_with_catalog(&bytes, catalog).unwrap();
        assert!(chunk.same_content(&back));
        let mut unknown = Chunk::new();
        unknown.filled_section(0).fill(u16::MAX);
        assert_eq!(unknown.validate_catalog(catalog), Err(DecodeError::UnknownBlockState(u16::MAX)));
        assert_eq!(unknown.to_bytes_with_catalog(catalog).unwrap_err(), DecodeError::UnknownBlockState(u16::MAX));
        assert!(matches!(Chunk::decode_for_load_with_catalog(&unknown.to_bytes(), catalog),
            Err(DecodeError::UnknownBlockState(u16::MAX))));
        assert!(matches!(Chunk::decode_for_load_with_catalog(&[CHUNK_VERSION, SINGLE, 0], catalog),
            Err(DecodeError::Damaged)));
    }
}

#[test]
fn unused_disabled_palette_entry_is_incompatible_too() {
    let state = pack(WHITE_SLAB_FIRST_STATE);
    let mut bytes = Chunk::new().to_bytes();
    let mut section = vec![PALETTED, 0, 2, 0, 0];
    section.extend(state.to_be_bytes());
    section.extend(vec![0; SECTION_BLOCKS / 8]);
    bytes.splice(1..2, section);
    assert!(matches!(Chunk::decode_for_load(&bytes),
        Err(DecodeError::UnsupportedCatalogState { state: value, .. }) if value == state));
    let decoded = Chunk::decode_for_load_with_catalog(&bytes, preview()).unwrap();
    assert!(decoded.same_content(&Chunk::new()));
}

#[test]
fn chunk_container_preview_stacks_preserve_components_and_deny_default() {
    use crate::{containers::{Container, Kind}, inventory::Stack, gameplay_catalog::WHITE_SLAB_ITEM};
    let place = (1, 65, 1);
    let mut chunk = Chunk::new();
    chunk.set(place.0, place.1, place.2, crate::blocks::state_by_name("chest").unwrap());
    let stack = Stack::new(WHITE_SLAB_ITEM, 37).with_extras(crate::stackdata::Data { name: Some("Плита в сундуке".into()), ..Default::default() });
    let mut container = Container::new(Kind::Chest);
    container.slots[0] = Some(stack);
    chunk.containers.insert(place, container);
    let bytes = chunk.to_bytes_with_catalog(preview()).unwrap();
    let back = Chunk::decode_for_load_with_catalog(&bytes, preview()).unwrap();
    assert_eq!(back.containers[&place].slots[0], Some(stack));
    assert_eq!(back.to_bytes_with_catalog(preview()).unwrap(), bytes);
    assert!(matches!(chunk.to_bytes_with_catalog(native()), Err(DecodeError::InvalidInventory { .. })));
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes, native()), Err(DecodeError::InvalidInventory { .. })));
    let mut overstack = chunk;
    overstack.containers.get_mut(&place).unwrap().slots[0] = Some(stack.with_count(65));
    assert!(matches!(overstack.validate_catalog(preview()), Err(DecodeError::InvalidInventory { .. })));
    // Старый читатель ограничивал 65→64 до проверки. Сырые bytes обязаны
    // вернуть typed error, чтобы загрузчик не перегенерировал чанк.
    let malformed = overstack.to_bytes();
    assert!(matches!(Chunk::decode_for_load_with_catalog(&malformed, preview()), Err(DecodeError::InvalidInventory { .. })));
}

#[test]
fn chunk_frames_pots_and_nested_contents_obey_catalog() {
    use crate::{inventory::Stack, gameplay_catalog::WHITE_SLAB_ITEM};
    let slab = Stack::new(WHITE_SLAB_ITEM, 1);
    let mut frame_chunk = Chunk::new();
    let mut frame = crate::frame::Frame::new(0, (1, 65, 1), crate::frame::NORTH, false);
    frame.item = Some(slab);
    frame_chunk.frames.push(frame);
    let bytes = frame_chunk.to_bytes_with_catalog(preview()).unwrap();
    assert!(Chunk::decode_for_load_with_catalog(&bytes, preview()).is_ok());
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes, native()), Err(DecodeError::InvalidInventory { .. })));
    let mut pot_chunk = Chunk::new();
    pot_chunk.pots.insert((1, 65, 1), [WHITE_SLAB_ITEM; 4]);
    assert!(pot_chunk.validate_catalog(preview()).is_ok());
    assert!(matches!(pot_chunk.validate_catalog(native()), Err(DecodeError::InvalidInventory { .. })));
    let mut nested = crate::containers::Container::new(crate::containers::Kind::Chest);
    nested.slots[0] = Some(Stack::new(crate::blocks::item_named("shulker_box").unwrap(), 1).with_extras(crate::stackdata::Data {
        contents: vec![crate::stackdata::Content { slot: 0, item: WHITE_SLAB_ITEM, count: 3, data: None }], ..Default::default()
    }));
    let mut chunk = Chunk::new();
    chunk.containers.insert((1, 65, 1), nested);
    assert!(chunk.validate_catalog(preview()).is_ok());
    assert!(matches!(chunk.validate_catalog(native()), Err(DecodeError::InvalidInventory { .. })));
}
