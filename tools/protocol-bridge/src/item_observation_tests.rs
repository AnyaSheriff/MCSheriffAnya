// Собственные сетевые записи; команды/SHA1 в testdata/item-observations.json.
const SAMPLES: &[(&str, &str, &str, &str)] = &[
    ("stone", "000103000000", "03010000", "03010000"),
    ("granite", "000104000100", "04020000", "04020000"),
    ("red_wool", "002305000e00", "05e3010000", "0588020000"),
    ("sword", "01140100070a0000090004656e63680a000000010200036c766c0005020002696400100001000b556e627265616b61626c650103000a526570616972436f73740000000200", "01a907040003070413020d012105", "019a08040003070413020d012105"),
    ("book", "01930100000a000009001253746f726564456e6368616e746d656e74730a000000010200036c766c0003020002696400220000", "01dd0901002a012803", "01f30a01002d012803"),
    ("name", "01530100000a00000a0007646973706c61790900044c6f7265080000000200084c696e65206f6e6500084c696e652074776f0800044e616d65000a4d4353412070726f62650000", "01850802000608000a4d4353412070726f62650b020800084c696e65206f6e650800084c696e652074776f", "01f90802000608000a4d4353412070726f62650b020800084c696e65206f6e650800084c696e652074776f"),
    ("leather", "012b0100090a00000a0007646973706c6179030005636f6c6f7200ff00000000", "01bc07020003092c00ff0000", "01ad08020003092f00ff0000"),
    ("map", "016601001700", "01d10801002e17", "01d60901003017"),
    ("white_banner", "01a902000f00", "02f3090000", "02890b0000"),
    ("empty", "ffff", "00", "00"),
];
const ENCHANTMENTS: &[&str] = &["aqua_affinity", "bane_of_arthropods", "binding_curse", "blast_protection", "breach", "channeling", "density", "depth_strider", "efficiency", "feather_falling", "fire_aspect", "fire_protection", "flame", "fortune", "frost_walker", "impaling", "infinity", "knockback", "looting", "loyalty", "luck_of_the_sea", "lunge", "lure", "mending", "multishot", "piercing", "power", "projectile_protection", "protection", "punch", "quick_charge", "respiration", "riptide", "sharpness", "silk_touch", "smite", "soul_speed", "sweeping_edge", "swift_sneak", "thorns", "unbreaking", "vanishing_curse", "wind_burst"];
use crate::{items::{self, Component, Context, Stack}, legacy_items, JavaVersion::{V26_1_2 as OLD, V26_3 as NEW}, wire, Direction, State};
fn bytes(hex: &str) -> Vec<u8> { hex.as_bytes().as_chunks::<2>().0.iter().map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect() }
fn context() -> Context { Context {shared_enchantment_count:Some(ENCHANTMENTS.len() as u32), ..Default::default() } }
fn legacy() -> legacy_items::Context<'static> { legacy_items::Context {enchantments:ENCHANTMENTS} }
fn canonical(mut stack: Option<Stack>) -> Option<Stack> {
    if let Some(s) = &mut stack {s.added.sort_by_key(|c|c.kind);s.removed.sort();} stack
}
#[test]
fn independent_official_slots_agree_in_all_three_versions() {
    for &(name, old, native, modern) in SAMPLES {
        let (old,native,modern)=(bytes(old),bytes(native),bytes(modern));
        let (stack,n)=legacy_items::decode(&old,legacy()).unwrap(); assert_eq!(n,old.len(),"{name}");
        for (version, observed) in [(OLD,&native),(NEW,&modern)] {
            let (expected,n)=items::decode(version,observed,false,context()).unwrap(); assert_eq!(n,observed.len(),"{name}");
            assert_eq!(canonical(stack.clone()),canonical(expected.clone()),"{name}");
            let encoded=items::encode(version,stack.as_ref(),false,context()).unwrap();
            assert_eq!(canonical(items::decode(version,&encoded,false,context()).unwrap().0),canonical(expected.clone()),"{name}");
            let reversed=legacy_items::encode(expected.as_ref(),legacy()).unwrap();
            assert_eq!(canonical(legacy_items::decode(&reversed,legacy()).unwrap().0),canonical(stack.clone()),"{name}");
        }
        // Только ID меняются; порядок компонентов в новых протоколах сохраняется.
        let stack=items::decode(OLD,&native,false,context()).unwrap().0;
        assert_eq!(items::encode(NEW,stack.as_ref(),false,context()).unwrap(),modern,"{name}");
        let stack=items::decode(NEW,&modern,false,context()).unwrap().0;
        assert_eq!(items::encode(OLD,stack.as_ref(),false,context()).unwrap(),native,"{name}");
    }
}
#[test]
fn every_exact_legacy_variant_preserves_item_identity_and_damage() {
    for &(old,meta,item,durable,_) in crate::legacy_item_ids::ITEMS {
        let damage=if durable {7} else if old==358 {23} else {meta};
        let mut packet=old.to_be_bytes().to_vec();packet.push(1);packet.extend(damage.to_be_bytes());packet.push(0);
        let (stack,n)=legacy_items::decode(&packet,legacy()).unwrap();assert_eq!(n,packet.len());
        assert_eq!(stack.as_ref().unwrap().item,item);
        assert_eq!(legacy_items::encode(stack.as_ref(),legacy()).unwrap(),packet);
    }
}
#[test]
fn packets_translate_inventory_cursor_equipment_and_untrusted_creative_lengths() {
    let ctx=wire::Context {items:context(),..Default::default()};
    let change=|body:&[u8],from,to,direction|wire::translate_with_context(State::Play,direction,from,to,body,ctx).unwrap().into_owned();
    let mut native=vec![0x12,0,1,10];let mut modern=native.clone();
    for &(_,_,a,b) in SAMPLES {native.extend(bytes(a));modern.extend(bytes(b));}
    native.push(0);modern.push(0);
    assert_eq!(change(&native,OLD,NEW,Direction::Clientbound),modern);
    assert_eq!(change(&modern,NEW,OLD,Direction::Clientbound),native);
    let (_,_,a,b)=SAMPLES[4];
    for (old_id,new_id,header) in [(0x14,0x14,&[0,2,0,36][..]),(0x60,0x62,&[]),(0x6c,0x6e,&[0]),(0x66,0x68,&[1,0])] {
        let mut old=vec![old_id];old.extend(header);old.extend(bytes(a));
        let mut new=vec![new_id];new.extend(header);new.extend(bytes(b));
        assert_eq!(change(&old,OLD,NEW,Direction::Clientbound),new);
        assert_eq!(change(&new,NEW,OLD,Direction::Clientbound),old);
    }
    let stack=items::decode(OLD,&bytes(a),false,context()).unwrap().0;
    let mut old=vec![0x38,0,36];old.extend(items::encode(OLD,stack.as_ref(),true,context()).unwrap());
    let mut new=vec![0x39,0,36];new.extend(items::encode(NEW,stack.as_ref(),true,context()).unwrap());
    assert_eq!(change(&old,OLD,NEW,Direction::Serverbound),new);
    assert_eq!(change(&new,NEW,OLD,Direction::Serverbound),old);
    for n in 0..new.len() {assert!(wire::translate_with_context(State::Play,Direction::Serverbound,NEW,OLD,&new[..n],ctx).is_err());}
}
#[test]
fn truncation_duplicates_bad_lengths_and_unknown_data_are_not_ignored() {
    for &(_,old,a,b) in SAMPLES {
        let old=bytes(old);for n in 0..old.len() {assert!(legacy_items::decode(&old[..n],legacy()).is_err());}
        for (version,raw) in [(OLD,a),(NEW,b)] {
            let raw=bytes(raw);for n in 0..raw.len() {assert!(items::decode(version,&raw[..n],false,context()).is_err());}
        }
    }
    // Два Damage / компонент одновременно добавлен и удалён / неизвестный тип.
    for raw in [&[1,1,2,0,3,1,3,2][..], &[1,1,1,1,3,1,3], &[1,1,1,0,127]] {assert!(items::decode(OLD,raw,false,context()).is_err());}
    assert!(items::decode(OLD,&[1,1,1,0,3,2,1],true,context()).is_err()); // объявлено2байта, осталось1
    assert!(items::decode(OLD,&[1,1,1,0,3,2,1,0],true,context()).is_err()); // лишний байт компонента
    assert!(items::decode(OLD,&[128,1,1,0,0],false,context()).is_err()); // oversized count
    let mut unknown=vec![0,1,1,0,0,10,0,0];unknown.extend([1,0,1,b'x',1,0]);
    assert!(matches!(legacy_items::decode(&unknown,legacy()),Err(legacy_items::Error::UnsupportedNbt(_))));
    let mut sword=items::decode(OLD,&bytes(SAMPLES[3].2),false,context()).unwrap().0.unwrap();
    sword.added.push(Component {kind:0,data:vec![10,0]});
    assert_eq!(legacy_items::encode(Some(&sword),legacy()),Err(legacy_items::Error::UnsupportedComponent(0)));
    sword.added.pop();sword.removed.push(4);assert!(legacy_items::encode(Some(&sword),legacy()).is_err());
}
#[test]
fn unknown_old_content_and_new_only_enchantments_fail_without_replacements() {
    assert!(legacy_items::decode(&[1,127,1,0,0,0],legacy()).is_err()); // spawn egg needs EntityTag
    assert!(legacy_items::decode(&[0,35,1,0,16,0],legacy()).is_err()); // wool variant16
    let mut map=items::decode(OLD,&bytes(SAMPLES[7].2),false,context()).unwrap().0.unwrap();
    map.added.clear();assert!(legacy_items::encode(Some(&map),legacy()).is_err());
    let mut sword=items::decode(OLD,&bytes(SAMPLES[3].2),false,context()).unwrap().0.unwrap();
    sword.added.iter_mut().find(|c|c.kind==13).unwrap().data=vec![1,4,1]; // breach отсутствует1.12.2
    assert_eq!(legacy_items::encode(Some(&sword),legacy()),Err(legacy_items::Error::UnknownEnchantment(4)));
    assert_eq!(items::decode(OLD,&bytes(SAMPLES[3].2),false,Context::default()),Err(items::Error::MissingEnchantmentContext));
    assert!(items::decode(crate::JavaVersion::V26_2,&[0],true,context()).is_err()); // Creative776 ещё закрыт.
}
#[test]
fn malformed_text_and_out_of_range_color_do_not_reach_clients() {
    for data in [vec![10,0],vec![10,1,0,4,b'b',b'o',b'l',b'd',1,0]] {
        let stack=Stack {item:1,count:1,added:vec![Component {kind:6,data}],removed:vec![]};
        assert!(items::encode(NEW,Some(&stack),false,context()).is_err());
    }
    let stack=Stack {item:1,count:1,added:vec![Component {kind:44,data:vec![255;4]}],removed:vec![]};
    assert!(items::encode(NEW,Some(&stack),false,context()).is_err());
    // Java modified-UTF8 для NUL/суррогатных пар пока не реализован: ошибка,
    // а не отправка обычного4-байтногоUTF8 в старый readUTF.
    for value in ["\0", "🙂", "§cRed"] {
        let mut raw=vec![8];raw.extend((value.len() as u16).to_be_bytes());raw.extend(value.as_bytes());
        let stack=Stack {item:1,count:1,added:vec![Component {kind:6,data:raw}],removed:vec![]};
        assert!(legacy_items::encode(Some(&stack),legacy()).is_err());
    }
    let mut map=items::decode(OLD,&bytes(SAMPLES[7].2),false,context()).unwrap().0.unwrap();
    map.added[0].data=vec![128,128,2]; //32768 не помещается в signedDamage1.12.2
    assert!(legacy_items::encode(Some(&map),legacy()).is_err());
}
#[test]
#[ignore = "экспорт наших преобразованных слотов для проверки официальным1.12.2"]
fn export_legacy_item_probe() {
    let mut records=Vec::new();
    for &(name,_,native,_) in SAMPLES {
        let stack=items::decode(OLD,&bytes(native),false,context()).unwrap().0;
        let encoded=legacy_items::encode(stack.as_ref(),legacy()).unwrap();
        let hex:String=encoded.iter().map(|b|format!("{b:02x}")).collect();
        records.push(format!("{{\"name\":\"{name}\",\"slot_hex\":\"{hex}\"}}"));
    }
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("legacy-item-replay.json"),format!("[{}]\n",records.join(",\n"))).unwrap();
}
