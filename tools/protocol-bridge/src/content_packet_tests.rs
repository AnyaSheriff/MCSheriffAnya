use crate::{content::{ContentCodec,IdSpace},items,chunk,chunk_sections,wire,ids::{self,RegistryKind},JavaVersion,State,Direction};
use JavaVersion::{V26_1_2 as OLD,V26_3 as NEW};
fn content()->ContentCodec { ContentCodec::white_slab_preview() }
fn context()->wire::Context { wire::Context { items:items::Context {content:content(),..Default::default()},..Default::default() } }
fn id(kind:RegistryKind,from:JavaVersion,to:JavaVersion,name:&str)->u32 {
    (0..256).find(|&id|crate::packet_layouts::packet_info(kind,from,to,id).is_some_and(|info|info.name==name)).unwrap() as u32
}
fn packet(kind:RegistryKind,from:JavaVersion,to:JavaVersion,name:&str,data:&[u8])->Vec<u8> {
    let mut out=Vec::new();wire::uint(&mut out,id(kind,from,to,name));out.extend(data);out
}
fn translate(direction:Direction,from:JavaVersion,to:JavaVersion,body:&[u8])->Vec<u8> {
    wire::translate_with_context(State::Play,direction,from,to,body,context()).unwrap().into_owned()
}
#[test]
fn block_updates_use_explicit_canonical_states_and_blocks() {
    for (name,kind,header,first,last,wire_first) in [
        ("block_change",RegistryKind::BlockState,8,29873,29878,18307),
        ("block_action",RegistryKind::Block,10,1168,1168,800)] {
        for state in first..=last {
            let mut a=vec![0;header];wire::uint(&mut a,state);
            let mut b=vec![0;header];wire::uint(&mut b,wire_first+state-first);
            let source=packet(RegistryKind::PlayClientbound,OLD,NEW,name,&a);
            let target=packet(RegistryKind::PlayClientbound,NEW,OLD,name,&b);
            assert_eq!(translate(Direction::Clientbound,OLD,NEW,&source),target);
            assert_eq!(translate(Direction::Clientbound,NEW,OLD,&target),source);
            assert!(wire::translate(State::Play,Direction::Clientbound,OLD,NEW,&source).is_err());
            assert_eq!(content().map(kind,IdSpace::Canonical,IdSpace::Wire(NEW),state).unwrap(),wire_first+state-first);
        }
    }
    let mut a=vec![0;8];let mut b=a.clone();a.push(6);b.push(6);
    for offset in 0..6 {wire::uint(&mut a,((29873+offset)<<12)|offset);wire::uint(&mut b,((18307+offset)<<12)|offset);}
    let a=packet(RegistryKind::PlayClientbound,OLD,NEW,"multi_block_change",&a);
    let b=packet(RegistryKind::PlayClientbound,NEW,OLD,"multi_block_change",&b);
    assert_eq!(translate(Direction::Clientbound,OLD,NEW,&a),b);
    assert_eq!(translate(Direction::Clientbound,NEW,OLD,&b),a);
}
#[test]
fn slot_packets_and_creative_input_use_canonical_payload_not_wire775_alias() {
    let stack=items::Stack {item:1506,count:3,added:vec![],removed:vec![]};
    let ctx=context().items;
    let mut canonical=vec![0];canonical.extend(items::encode_canonical(Some(&stack),false,ctx).unwrap());
    let mut foreign=vec![0];foreign.extend(items::encode(NEW,Some(&stack),false,ctx).unwrap());
    let a=packet(RegistryKind::PlayClientbound,OLD,NEW,"set_player_inventory",&canonical);
    let b=packet(RegistryKind::PlayClientbound,NEW,OLD,"set_player_inventory",&foreign);
    assert_eq!(translate(Direction::Clientbound,OLD,NEW,&a),b);
    assert_eq!(translate(Direction::Clientbound,NEW,OLD,&b),a);
    let mut canonical=vec![0,36];canonical.extend(items::encode_canonical(Some(&stack),true,ctx).unwrap());
    let mut foreign=vec![0,36];foreign.extend(items::encode(NEW,Some(&stack),true,ctx).unwrap());
    let a=packet(RegistryKind::PlayServerbound,OLD,NEW,"set_creative_slot",&canonical);
    let b=packet(RegistryKind::PlayServerbound,NEW,OLD,"set_creative_slot",&foreign);
    assert_eq!(translate(Direction::Serverbound,NEW,OLD,&b),a);
    assert!(wire::translate(State::Play,Direction::Serverbound,NEW,OLD,&b).is_err());
    for version in [OLD,JavaVersion::V26_2,JavaVersion::V1_12_2] {
        assert!(wire::translate_with_context(State::Play,Direction::Clientbound,OLD,version,&a,context()).is_err());
    }
}
#[test]
fn chunk_preview_translates_single_indirect_direct_without_changing_default_decoder() {
    let ctx=chunk::Context {min_y:-64,sections:24,shared_biome_count:59};
    // Fixture не содержит block entities: bed states сюда не входят.
    // Reverse777→775 правильно добавляет legacy bed entity; пустой список
    // при bed block делал прежний synthetic fixture несогласованным.
    let base:Vec<u32>=(0..29873).filter(|&id|!crate::block_entity_ids::legacy_bed(id)
        && ids::translate(RegistryKind::BlockState,OLD,NEW,id).is_ok()).take(4096).collect();
    assert_eq!(base.len(),4096);
    for mode in 0..3 {
        let mut sections=vec![chunk_sections::Section {blocks_present:0,fluids_present:0,blocks:vec![0;4096],biomes:vec![7;64]};24];
        if mode==0 {sections[0].blocks.fill(29876);}
        if mode==1 {for (at,state) in (29873..=29878).enumerate(){sections[0].blocks[at]=state;}}
        if mode==2 {sections[0].blocks.copy_from_slice(&base);for (at,state) in (29873..=29878).enumerate(){sections[0].blocks[at]=state;}}
        sections[0].blocks_present=sections[0].blocks.iter().filter(|&&id|id!=0).count() as u16;
        let encoded=chunk_sections::encode_with_content(OLD,&sections,59,IdSpace::Canonical,content()).unwrap();
        assert_eq!(encoded[4],if mode==0{0}else if mode==1{4}else{15});
        assert!(chunk_sections::decode(OLD,&encoded,24,59).is_err());
        let mut source=vec![0;8];source.push(0);wire::uint(&mut source,encoded.len() as u32);source.extend(encoded);source.push(0);
        source.extend(include_bytes!("../testdata/light-26.1.2-placed.bin"));
        let target=chunk::translate_with_content(OLD,NEW,&source,ctx,content()).unwrap();
        let mut input=wire::Cursor::new(&target);input.take(9).unwrap();let size=input.uint().unwrap() as usize;
        let mapped=chunk_sections::decode(NEW,input.take(size).unwrap(),24,59).unwrap();
        for (a,b) in sections.iter().zip(mapped.iter()) {for (&a,&b) in a.blocks.iter().zip(&b.blocks){assert_eq!(content().map(RegistryKind::BlockState,IdSpace::Canonical,IdSpace::Wire(NEW),a).unwrap(),b);}}
        let restored=chunk::translate_with_content(NEW,OLD,&target,ctx,content()).unwrap();
        let mut input=wire::Cursor::new(&restored);input.take(9).unwrap();let size=input.uint().unwrap() as usize;
        let canonical=chunk_sections::decode_with_content(OLD,input.take(size).unwrap(),24,59,IdSpace::Canonical,content()).unwrap();
        for (section,(expected,actual)) in sections.iter().zip(&canonical).enumerate() {
            assert_eq!(actual.blocks_present,expected.blocks_present,"mode {mode}, section {section}: block count");
            assert_eq!(actual.fluids_present,expected.fluids_present,"mode {mode}, section {section}: fluid count");
            for (cell,(&expected,&actual)) in expected.blocks.iter().zip(&actual.blocks).enumerate() {
                assert_eq!(actual,expected,"mode {mode}, section {section}, cell {cell}: canonical state changed");
            }
            for (cell,(&expected,&actual)) in expected.biomes.iter().zip(&actual.biomes).enumerate() {
                assert_eq!(actual,expected,"mode {mode}, section {section}, biome cell {cell}");
            }
        }
        assert_eq!(input.uint().unwrap(),0,"no block entities in this fixture");
        let first_difference=restored.iter().zip(&source).position(|(a,b)|a!=b);
        assert!(restored.len()==source.len() && first_difference.is_none(),
            "mode {mode}: first byte difference {first_difference:?}; restored/source lengths {}/{}",
            restored.len(),source.len());
        assert!(chunk::translate(OLD,NEW,&source,ctx).is_err());
    }
}
