use super::*;

fn with_first_section(section: &[u8]) -> Vec<u8> {
    let mut bytes=Chunk::new().to_bytes();
    bytes.splice(1..2, section.iter().copied());
    bytes
}

#[test]
fn unknown_catalog_state_is_distinct_in_every_storage_mode() {
    let state=u16::MAX;
    assert!(crate::blocks::block_at_state(i32::from(state)).is_none());
    let single=vec![SINGLE,255,255];
    let mut paletted=vec![PALETTED,0,2,0,1,255,255];
    paletted.extend(vec![0;SECTION_BLOCKS/8]);
    let mut direct=vec![DIRECT];
    direct.extend(state.to_be_bytes());direct.extend(vec![0;2*(SECTION_BLOCKS-1)]);
    for section in [single,paletted,direct] {
        let bytes=with_first_section(&section);
        assert!(matches!(Chunk::decode_for_load(&bytes),Err(DecodeError::UnknownBlockState(value)) if value==state));
        assert!(Chunk::from_bytes(&bytes).is_none());
    }
}

#[test]
fn unsupported_layout_and_known_malformed_bytes_are_not_conflated() {
    let mut future=Chunk::new().to_bytes();future[0]=CHUNK_VERSION+1;
    assert!(matches!(Chunk::decode_for_load(&future),Err(DecodeError::UnsupportedLayout(value)) if value==CHUNK_VERSION+1));
    for bytes in [Vec::new(),vec![CHUNK_VERSION,SINGLE,255],vec![CHUNK_VERSION,255]] {
        assert!(matches!(Chunk::decode_for_load(&bytes),Err(DecodeError::Damaged)));
    }
    // Неверный индекс известной палитры остаётся повреждением, не каталогом.
    let mut palette=vec![PALETTED,0,3,0,0,0,1,0,2];
    palette.extend(vec![255;SECTION_BLOCKS/4]);
    assert!(matches!(Chunk::decode_for_load(&with_first_section(&palette)),Err(DecodeError::Damaged)));
}

#[test]
fn current_native_chunk_still_roundtrips_through_runtime_decoder() {
    let source=Chunk::flat();let bytes=source.to_bytes();
    let decoded=Chunk::decode_for_load(&bytes).unwrap_or_else(|error|panic!("{error}"));
    assert!(source.same_content(&decoded));assert_eq!(decoded.to_bytes(),bytes);
}
