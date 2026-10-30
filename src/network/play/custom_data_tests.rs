//! Полный путь creative → игрок/файл/хранилище → предмет на земле → пакет.
use super::*;
use crate::stackdata::{self, CustomData, Data};
use mcsa_protocol::{JavaVersion, items};

const SPAWN: (f64, f64, f64) = (0.5, 64.0, 0.5);

fn custom() -> CustomData {
    // Все 12 типов, включая long вне точности f64 и MUTF-8 NUL + 😀.
    let mut bytes = vec![10];
    for (kind, name, body) in [
        (1,"byte",vec![255]), (2,"short",(-123i16).to_be_bytes().to_vec()),
        (3,"int",i32::MIN.to_be_bytes().to_vec()), (4,"long",9007199254740993i64.to_be_bytes().to_vec()),
        (5,"float",0x7fc00001u32.to_be_bytes().to_vec()), (6,"double",(-0.0f64).to_be_bytes().to_vec()),
        (7,"bytes",vec![0,0,0,2,0,255]), (8,"text",vec![0,8,0xc0,0x80,0xed,0xa0,0xbd,0xed,0xb8,0x80]),
        (9,"list",vec![10,0,0,0,1,3,0,1,b'x',0,0,0,5,0]), (10,"compound",vec![8,0,1,b'n',0,1,b'v',0]),
        (11,"ints",[1i32.to_be_bytes().as_slice(),i32::MIN.to_be_bytes().as_slice()].concat()),
        (12,"longs",[1i32.to_be_bytes().as_slice(),i64::MIN.to_be_bytes().as_slice()].concat()),
    ] {
        bytes.push(kind); bytes.extend_from_slice(&(name.len() as u16).to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());bytes.extend(body);
    }
    bytes.push(0);CustomData::from_bytes(&bytes).unwrap()
}
fn player() -> PlayerState {
    let mut p = PlayerState::new([0xca;16],"CustomData".into(),None,None,0x7f,17,SPAWN);
    p.game_mode = GAME_MODE_CREATIVE;p
}
fn received_payload(stack: Stack, slot: i16, version: JavaVersion) -> Vec<u8> {
    let mut native = Vec::new();push_slot(&mut native,Some(stack));
    let (canonical,n) = items::decode_canonical(&native,false,items::Context::default()).unwrap();assert_eq!(n,native.len());
    let wire = items::encode(version,canonical.as_ref(),true,items::Context::default()).unwrap();
    let (canonical,n) = items::decode(version,&wire,true,items::Context::default()).unwrap();assert_eq!(n,wire.len());
    let native = items::encode_canonical(canonical.as_ref(),true,items::Context::default()).unwrap();
    [slot.to_be_bytes().as_slice(),native.as_slice()].concat()
}
fn raw_payload(bytes: &[u8]) -> Vec<u8> {
    let mut payload = 36i16.to_be_bytes().to_vec();
    for v in [1,blocks::item_named("stone").unwrap(),1,0,0,bytes.len() as i32] {payload.extend(encode_varint(v));}
    payload.extend_from_slice(bytes);payload
}
fn saved(p: &PlayerState) -> PlayerData {
    PlayerData {abilities:playerdata::Abilities::for_mode(p.game_mode),shoulders:[None,None],x:p.x,y:p.y,z:p.z,
        yaw:p.yaw,pitch:p.pitch,game_mode:p.game_mode,inventory:p.inventory.clone(),dimension:p.dimension,
        vitals:Default::default(),respawn:None,ender:playerdata::empty_ender()}
}

#[tokio::test]
async fn creative_custom_data_survives_native_profiles_player_file_container_and_drop_packet() {
    for (profile,version) in [(crate::gameplay_catalog::Profile::Java26_1_2,JavaVersion::V26_1_2),
        (crate::gameplay_catalog::Profile::Preview26_3,JavaVersion::V26_3)] {
        let catalog = crate::gameplay_catalog::Catalog::new(profile);
        let expected = Stack::new(blocks::item_named("stone").unwrap(),7).with_extras(Data {custom_data:Some(custom()),..Default::default()});
        let mut p = player();
        let payload = received_payload(expected,36,version);
        assert!(read_creative_slot_with_catalog(&payload,&mut p,catalog).is_none());
        assert_eq!(p.inventory.slot(36),Some(expected));
        p.inventory.set_cursor(Some(expected.with_count(3)));
        let dir = std::env::temp_dir().join(format!("mcsa-custom-data-{}-{}",std::process::id(),version.protocol()));
        let data = saved(&p);playerdata::save_for_catalog(&dir,&p.uuid,&data,catalog).unwrap();
        let restored = playerdata::load_checked_for_catalog(&dir,&p.uuid,catalog).unwrap().unwrap();
        assert_eq!(restored,data);std::fs::remove_dir_all(&dir).unwrap();
        let mut chest = containers::Container::new(containers::Kind::Chest);chest.slots[4]=restored.inventory.slot(36);
        let mut bytes = Vec::new();chest.write((1,64,2),&mut bytes);
        let mut rest = bytes.as_slice();let (pos,back)=containers::Container::read(&mut rest,0,0).unwrap();
        assert!(rest.is_empty());assert_eq!(pos,(1,64,2));assert_eq!(back.slots,chest.slots);
        // Creative slot −1 возвращает именно ту стопку, которая попадёт в Items.
        let payload = received_payload(back.slots[4].unwrap(),-1,version);
        let thrown = read_creative_slot_with_catalog(&payload,&mut p,catalog).unwrap();
        let mut drops = crate::items::Items::new();drops.drop_from_player(81,thrown,SPAWN,(0.0,0.0,0.0),p.uuid);
        let dropped = &drops.items()[0];assert_eq!(dropped.stack,expected);
        let mut out = Outgoing::capture();send_item_properties(&mut out,81,dropped.stack,false).await.unwrap();
        let (size,n) = decode_varint(out.queued()).unwrap();let body=&out.queued()[n..];assert_eq!(body.len(),size as usize);
        let mut cursor = Cursor::new(body);assert_eq!(cursor.varint(),Some(SET_ENTITY_DATA));assert_eq!(cursor.varint(),Some(81));
        assert_eq!(cursor.u8(),Some(ITEM_PROPERTY));assert_eq!(cursor.varint(),Some(PROPERTY_SLOT));
        let (actual,n)=items::decode_canonical(&body[cursor.offset..],false,items::Context::default()).unwrap();
        assert_eq!(body[cursor.offset+n..],[PROPERTIES_END]);
        let actual=actual.unwrap();let compounds:Vec<_>=actual.added.iter().filter(|c|c.kind==0).collect();
        assert_eq!(compounds.len(),1);assert_eq!(compounds[0].data,custom().as_bytes());
        let wire=items::encode(version,Some(&actual),false,items::Context::default()).unwrap();
        let (actual,n)=items::decode(version,&wire,false,items::Context::default()).unwrap();assert_eq!(n,wire.len());
        assert_eq!(actual.unwrap().added.iter().find(|c|c.kind==0).unwrap().data,custom().as_bytes());
    }
}

#[test]
fn creative_custom_data_rejects_malformed_before_replacing_existing_slot() {
    let mut deeply = vec![10];for _ in 0..34 {deeply.extend([10,0,1,b'x']);}deeply.extend(vec![0;35]);
    let mut oversized = vec![10,7,0,1,b'x'];oversized.extend(65510i32.to_be_bytes());oversized.extend(vec![0;65510]);oversized.push(0);
    assert!(CustomData::from_bytes(&oversized).is_some(),"ограничение общей записи, а не повреждённый NBT");
    let invalid = [vec![0],vec![10,0,0],vec![10,1,0,1,b'a',1,1,0,1,b'a',2,0],
        vec![10,9,0,1,b'l',0,0,0,0,1,0],vec![10,8,0,1,b's',0,1,0,0],deeply,vec![10;65536],oversized];
    for profile in [crate::gameplay_catalog::Profile::Java26_1_2,crate::gameplay_catalog::Profile::Preview26_3] {
        let mut p=player();p.inventory.set(36,Some(Stack::new(blocks::item_named("diamond").unwrap(),3)));
        for bytes in &invalid {
            let before=p.inventory.clone();read_creative_slot_with_catalog(&raw_payload(bytes),&mut p,crate::gameplay_catalog::Catalog::new(profile));
            assert_eq!(p.inventory,before);
        }
        let valid=raw_payload(custom().as_bytes());
        for n in 0..valid.len() {
            let before=p.inventory.clone();read_creative_slot_with_catalog(&valid[..n],&mut p,crate::gameplay_catalog::Catalog::new(profile));
            assert_eq!(p.inventory,before,"truncated {n}");
        }
        let mut extra=valid.clone();extra.push(0);let before=p.inventory.clone();
        read_creative_slot_with_catalog(&extra,&mut p,crate::gameplay_catalog::Catalog::new(profile));assert_eq!(p.inventory,before);
    }
}

#[test]
fn custom_data_nested_items_and_tnt_extras_survive_creative_and_storage() {
    let ordinary=Stack::new(blocks::item_named("stone").unwrap(),2).with_extras(Data {custom_data:Some(custom()),..Default::default()});
    let kind=crate::custom_tnt::by_name("nuke").unwrap();
    let mut compound=nbt::Compound::new().string(crate::custom_tnt::MARKER,kind.name).int("aaa",17).encode_network();
    // После сортировки aaa (TAG_Int) стоит перед служебной строкой TNT;
    // поиск метки должен пропустить числовое поле, сохранив его.
    let custom=CustomData::from_bytes(&compound).unwrap();compound=custom.as_bytes().to_vec();
    let tnt=stackdata::stack_with_components(blocks::item_named("tnt").unwrap(),2,Data {custom_data:Some(custom),damage:3,name:Some("Особое имя".into()),..Default::default()});
    assert_eq!(tnt.tnt,Some(kind.id));
    let content=|s:Stack,slot|stackdata::Content {slot,item:s.stored_item(),count:s.count,data:s.data};
    let shulker=Stack::new(blocks::item_named("shulker_box").unwrap(),1).with_extras(Data {contents:vec![content(ordinary,0),content(tnt,26)],..Default::default()});
    let bundle=Stack::new(blocks::item_named("bundle").unwrap(),1).with_extras(Data {bundle:vec![content(ordinary,0),content(tnt,0)],..Default::default()});
    let arrow=Stack::new(blocks::item_named("arrow").unwrap(),1).with_extras(ordinary.extras());
    let crossbow=Stack::new(blocks::item_named("crossbow").unwrap(),1).with_extras(Data {charged:1,charged_arrow:Some(stackdata::Content {slot:0,item:arrow.item,count:arrow.count,data:arrow.data}),..Default::default()});
    for version in [JavaVersion::V26_1_2,JavaVersion::V26_3] {
        for expected in [shulker,bundle,crossbow,tnt] {
            let payload=received_payload(expected,36,version);let mut p=player();
            let catalog=crate::gameplay_catalog::Catalog::new(if version==JavaVersion::V26_3 {crate::gameplay_catalog::Profile::Preview26_3}else{crate::gameplay_catalog::Profile::Java26_1_2});
            read_creative_slot_with_catalog(&payload,&mut p,catalog);assert_eq!(p.inventory.slot(36),Some(expected));
            assert_eq!(Inventory::from_bytes_for_catalog(&p.inventory.to_bytes(),catalog).unwrap(),p.inventory);
        }
    }
    let mut bytes=Vec::new();push_slot(&mut bytes,Some(tnt));
    let (actual,_)=items::decode_canonical(&bytes,false,items::Context::default()).unwrap();let actual=actual.unwrap();
    assert_eq!(actual.added.iter().filter(|c|c.kind==0).count(),1);
    assert_eq!(actual.added.iter().find(|c|c.kind==0).unwrap().data,compound);
    assert_eq!(actual.added.iter().find(|c|c.kind==3).unwrap().data,vec![3]);
    let inconsistent=Stack {tnt:Some(kind.id),..Stack::new(tnt.item,1).with_extras(Data {
        custom_data:CustomData::from_bytes(&nbt::Compound::new().string(crate::custom_tnt::MARKER,"unknown").encode_network()),..Default::default()
    })};
    assert!(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2).validate_stack(&inconsistent).is_err());
    let empty=Stack::new(ordinary.item,2).with_extras(Data {custom_data:CustomData::from_bytes(&[10,0]),..Default::default()});
    assert!(!empty.stacks_with(&Stack::new(ordinary.item,2)));
    assert_eq!(stackdata::from_bytes(&stackdata::to_bytes(&empty.extras())),Some(empty.extras()));
}

#[test]
fn custom_data_commands_fail_explicitly_even_inside_items() {
    for spec in ["stone[custom_data={x:1b}]","stone[minecraft:custom_data={}]",
        "shulker_box[container=[{slot:0,item:{id:stone,components:{custom_data:{x:1L}}}}]]",
        "bundle[bundle_contents=[{id:stone,components:{custom_data:{x:1s}}}]]",
        "crossbow[charged_projectiles=[{id:arrow,components:{custom_data:{x:1}}}]]"] {
        assert!(stackdata::parse_item_spec(spec).is_err(),"{spec}");
    }
    assert!(crate::commands::item_from_text("{Item:{id:stone,components:{custom_data:{x:1}}}}").is_none());
}

#[test]
fn duplicate_custom_data_components_are_rejected_before_inventory_or_nested_mutation() {
    let custom=custom();
    for removed in [false,true] {
        let mut payload=36i16.to_be_bytes().to_vec();
        for v in [1,blocks::item_named("stone").unwrap(),if removed {1}else{2},i32::from(removed)] {payload.extend(encode_varint(v));}
        payload.extend(encode_varint(0));payload.extend(encode_varint(custom.as_bytes().len() as i32));payload.extend(custom.as_bytes());
        if !removed {payload.extend(encode_varint(0));payload.extend(encode_varint(custom.as_bytes().len() as i32));payload.extend(custom.as_bytes());}
        else {payload.extend(encode_varint(0));}
        for profile in [crate::gameplay_catalog::Profile::Java26_1_2,crate::gameplay_catalog::Profile::Preview26_3] {
            let mut p=player();p.inventory.set(36,Some(Stack::new(blocks::item_named("diamond").unwrap(),3)));
            let before=p.inventory.clone();read_creative_slot_with_catalog(&payload,&mut p,crate::gameplay_catalog::Catalog::new(profile));
            assert_eq!(p.inventory,before);
        }
        for kind in [50,75,49] {
            let item=blocks::item_named(if kind==49 {"arrow"}else{"stone"}).unwrap();
            let mut body=encode_varint(1);
            if kind==75 {body.push(1);}
            for v in [item,1,if removed {1}else{2},i32::from(removed)] {body.extend(encode_varint(v));}
            body.extend(encode_varint(0));body.extend(custom.as_bytes());
            body.extend(encode_varint(0));if !removed {body.extend(custom.as_bytes());}
            let mut data=Data {damage:17,custom_data:Some(custom.clone()),bundle:vec![stackdata::Content {
                slot:0,item:blocks::item_named("dirt").unwrap(),count:1,data:None}],..Default::default()};
            let before=data.clone();
            let accepted=match kind {
                50=>stackdata::read_bundle_component(&body,&mut data),
                75=>stackdata::read_container_component(&body,&mut data),
                _=>stackdata::read_charged_component(&body,&mut data),
            };
            assert!(!accepted,"kind={kind}, removed={removed}");assert_eq!(data,before);
        }
    }
}

#[test]
fn bundle_component_with_trailing_or_truncated_bytes_is_atomic() {
    let original=Data {bundle:vec![stackdata::Content {slot:0,item:blocks::item_named("dirt").unwrap(),count:1,data:None}],..Default::default()};
    let mut body=encode_varint(1);
    for value in [blocks::item_named("stone").unwrap(),2,1,0,0] {body.extend(encode_varint(value));}
    body.extend(custom().as_bytes());
    let mut parsed=original.clone();assert!(stackdata::read_bundle_component(&body,&mut parsed));assert_ne!(parsed,original);
    for end in 0..body.len() {
        let mut parsed=original.clone();assert!(!stackdata::read_bundle_component(&body[..end],&mut parsed));assert_eq!(parsed,original);
    }
    body.push(0);let mut parsed=original.clone();assert!(!stackdata::read_bundle_component(&body,&mut parsed));assert_eq!(parsed,original);
}
