//! Расширение по закреплённым схемам, не новая запись official776.
use crate::{Direction, JavaVersion, State, entities::Tracker, items, wire::{self, Context}};
use JavaVersion::{V26_1_2 as OLD, V26_2 as MID};

fn hex(value: &serde_json::Value) -> Vec<u8> {
    value.as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn cb(from: JavaVersion, to: JavaVersion, body: &[u8], context: Context) -> Result<Vec<u8>, wire::Error> {
    wire::translate_inner(State::Play, Direction::Clientbound, from, to, body, context, true).map(|b| b.into_owned())
}

#[test]
fn typed_776_complex_slots_preserve_54_existing_cases_and_untrusted_lengths() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
    let cases = data["old"]["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 54);
    for case in cases {
        let raw = hex(&case["slot_hex"]);
        let (stack, size) = items::decode(OLD, &raw, false, items::Context::default()).unwrap();
        assert_eq!(size, raw.len());
        for untrusted in [false, true] {
            let encoded = items::encode(MID, stack.as_ref(), untrusted, items::Context::default()).unwrap();
            assert_eq!(items::decode(MID, &encoded, untrusted, items::Context::default()).unwrap(), (stack.clone(), encoded.len()), "{}", case["name"]);
            for end in 0..encoded.len() {
                assert!(items::decode(MID, &encoded[..end], untrusted, items::Context::default()).is_err(), "{} prefix{end}", case["name"]);
            }
        }
        assert_eq!(items::encode(OLD, stack.as_ref(), false, items::Context::default()).unwrap(), raw);
    }
    // Damage7: длина относится к компоненту, а не ко всему Slot.
    let correct = [1, 1, 1, 0, 3, 1, 7];
    assert_eq!(items::decode(MID, &correct, true, items::Context::default()).unwrap().1, correct.len());
    for raw in [&[1, 1, 1, 0, 3, 0][..], &[1, 1, 1, 0, 3, 2, 7], &[1, 1, 1, 0, 3, 2, 7, 0]] {
        assert!(items::decode(MID, raw, true, items::Context::default()).is_err());
    }
}

#[test]
fn typed_776_enchanted_named_equipment_preserves_all_ten_existing_samples() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/item-observations.json")).unwrap();
    let cases = data.as_array().unwrap().iter().find(|r| r["protocol"] == 775).unwrap()["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    let context = items::Context { shared_enchantment_count: Some(43), ..Default::default() };
    for case in cases {
        let (stack, _) = items::decode(OLD, &hex(&case["slot_hex"]), false, context).unwrap();
        for untrusted in [false, true] {
            let encoded = items::encode(MID, stack.as_ref(), untrusted, context).unwrap();
            assert_eq!(items::decode(MID, &encoded, untrusted, context).unwrap(), (stack.clone(), encoded.len()));
        }
    }
}

#[test]
fn typed_776_world_particles_keep_old_envelope_and_all_23_existing_effects() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/particle-observations.json")).unwrap();
    let cases = data["old"]["particles"].as_array().unwrap();
    assert_eq!(cases.len(), 23);
    for case in cases {
        let source = hex(&case["body"]);
        let encoded = cb(OLD, MID, &source, Context::default()).unwrap();
        assert_eq!(cb(MID, OLD, &encoded, Context::default()).unwrap(), source, "{}", case["case"]);
        for end in 0..encoded.len() { assert!(cb(MID, OLD, &encoded[..end], Context::default()).is_err()); }
        let mut extra = encoded; extra.push(0);
        assert!(cb(MID, OLD, &extra, Context::default()).is_err());
    }
}

#[test]
fn pinned_776_layouts_and_eleven_recorded_entity_payloads_preserve_fields() {
    assert_eq!(crate::generated_payload_776::LAYOUTS.len(), 158);
    for &(kind, _) in crate::generated_payload_776::LAYOUTS {
        // Общий shared_flags; проверка индексов всех раскладок, не поведение моба.
        let body = [0x63, 2, 0, 0, 3, 255];
        let context = Context { entity_kind: Some(kind), ..Default::default() };
        assert_eq!(cb(OLD, MID, &body, context).unwrap(), body);
        assert_eq!(cb(MID, OLD, &body, context).unwrap(), body);
    }
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/entity-observations.json")).unwrap();
    let cases = data["old"]["entities"].as_array().unwrap();
    assert_eq!(cases.len(), 11);
    let context = Context { metadata_registry_counts: &[("minecraft:painting_variant", 51), ("minecraft:wolf_variant", 9)], ..Default::default() };
    for case in cases {
        let mut tracker = Tracker::default();
        let kind = crate::entity_layouts::LAYOUTS.iter().position(|r| Some(r.0) == case["type"].as_str()).unwrap() as u32;
        for packet in case["packets"].as_array().unwrap() {
            if !matches!(packet["name"].as_str(), Some("spawn_entity" | "entity_metadata")) { continue; }
            let source = hex(&packet["body"]);
            let encoded = crate::probe_776::clientbound(&mut tracker, &source, context).unwrap();
            let context = Context { entity_kind: Some(kind), ..context };
            assert_eq!(cb(MID, OLD, &encoded, context).unwrap(), source, "{}", case["type"]);
        }
    }
}

#[test]
fn schema_776_particle_metadata_preserves_nonempty_lists() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/particle-observations.json")).unwrap();
    for entry in data["old"]["metadata"].as_array().unwrap() {
        let kind = crate::entity_layouts::LAYOUTS.iter().position(|r| Some(r.0) == entry["type"].as_str()).unwrap() as u32;
        let context = Context { entity_kind: Some(kind), ..Default::default() };
        for packet in entry["packets"].as_array().unwrap().iter().filter(|p| p["name"] == "entity_metadata") {
            let source = hex(&packet["body"]);
            let middle = cb(OLD, MID, &source, context).unwrap();
            assert_eq!(cb(MID, OLD, &middle, context).unwrap(), source);
        }
    }
}

#[test]
fn primary_777_extensions_keep_sulfur_cube_and_project_cushion_metadata_for_776() {
    let context = Context { items: items::Context {
        content: crate::content::ContentCodec::white_slab_preview(), ..Default::default()
    }, ..Default::default() };
    for canonical in [157, 158, 159, 160] {
        let mut source = vec![1, 2]; source.extend([0; 16]);
        wire::uint(&mut source, canonical);
        for coordinate in [0.5f64, 65.0, 0.5] { source.extend(coordinate.to_be_bytes()); }
        source.extend([0, 0, 0, 0, 0]);
        let mut tracker = Tracker::default();
        let spawn = crate::probe_776::clientbound(&mut tracker, &source, context).unwrap();
        let mut input = wire::Cursor::new(&spawn);
        assert_eq!(input.uint().unwrap(), 1); assert_eq!(input.uint().unwrap(), 2);
        input.take(16).unwrap();
        let expected = context.items.content.packet_id(crate::ids::RegistryKind::Entity, OLD, MID, canonical).unwrap();
        assert_eq!(input.uint().unwrap(), expected);
        let source: &[u8] = match canonical {
            159 => &[0x63, 2, 8, 1, 14, 255],
            160 => &[0x63, 2, 18, 1, 2, 255],
            _ => &[0x63, 2, 0, 0, 1, 255],
        };
        let metadata = crate::probe_776::clientbound(&mut tracker, source, context).unwrap();
        assert_eq!(metadata.as_ref(), if canonical == 159 { &[0x63, 2, 255] } else { source });
    }
}

#[test]
fn profile_776_complete_partial_properties_and_skin_patch_are_bounded() {
    let mut complete = vec![1]; complete.extend([1;16]); complete.extend([1,b'P',1,1,b'k',1,b'v',1,1,b's']);
    complete.extend([1,1,b'b',1,1,b'c',1,1,b'e',1,1]);
    let mut partial = vec![0,1,1,b'P',1]; partial.extend([0;16]); partial.extend([0;5]);
    for data in [vec![0,0,0,0,0,0,0,0], complete, partial] {
        let mut cursor = wire::Cursor::new(&data);
        crate::profile::read(&mut cursor).unwrap(); assert!(cursor.rest.is_empty());
        for end in 0..data.len() { assert!(crate::profile::read(&mut wire::Cursor::new(&data[..end])).is_err()); }
    }
    for data in [&[2][..], &[0,2], &[0,0,0,0x81,2], &[0,0,0,0,0,0,0,1,2]] {
        assert!(crate::profile::read(&mut wire::Cursor::new(data)).is_err());
    }
    let kind = crate::entity_layouts::LAYOUTS.iter().position(|row| row.0 == "mannequin").unwrap() as u32;
    let field = crate::entity_layouts::LAYOUTS[kind as usize].1.iter().position(|v| *v == "profile").unwrap() as u8;
    let mut body = vec![0x63,7,field,41]; body.extend([0,0,0,0,0,0,0,0]);body.push(255);
    let context = Context { entity_kind: Some(kind), ..Default::default() };
    assert_eq!(cb(OLD,MID,&body,context).unwrap(),body);
    assert_eq!(cb(MID,OLD,&body,context).unwrap(),body);
    for end in 0..body.len() { assert!(cb(OLD,MID,&body[..end],context).is_err()); }
}

#[test]
fn ten_776_gui_branches_validate_full_payload_without_trusting_client_content() {
    let cases: &[(&str, &[u8])] = &[
        ("set_beacon_effect", &[1,1,0]), ("name_item", &[1,b'A']),
        ("enchant_item", &[1,2]), ("select_trade", &[1]),
        ("set_slot_state", &[2,1,1]), ("update_sign", &[0,0,0,0,0,0,0,0,1,1,b'A',0,0,0]),
        ("select_bundle_item", &[1,255,255,255,255,15]), ("edit_book", &[0,1,1,b'P',1,1,b'T']),
        ("pick_item_from_block", &[0,0,0,0,0,0,0,0,1]), ("pick_item_from_entity", &[7,1]),
    ];
    for &(name, payload) in cases {
        let id = (0..256).find(|&id| crate::packet_layouts::packet_info(crate::ids::RegistryKind::PlayServerbound,MID,OLD,id).is_some_and(|v|v.name==name)).unwrap();
        let mut body = vec![id as u8]; body.extend(payload);
        let converted = crate::probe_776::serverbound(&body,Context::default()).unwrap();
        assert_eq!(&converted[1..],payload,"{name}");
        for end in 0..body.len() { assert!(crate::probe_776::serverbound(&body[..end],Context::default()).is_err(),"{name} {end}"); }
        let mut extra=body;extra.push(0);assert!(crate::probe_776::serverbound(&extra,Context::default()).is_err(),"{name}");
    }
    for (name,payload) in [("set_beacon_effect",vec![2]),("enchant_item",vec![1,3]),("set_slot_state",vec![0,0,2]),
        ("edit_book",vec![0,101]),("pick_item_from_entity",vec![1,2]),("select_bundle_item",vec![0,254,255,255,255,15])] {
        assert!(crate::actions::gui(name,&mut wire::Cursor::new(&payload),&mut Vec::new()).is_err(),"{name}");
    }
}

#[test]
fn six_additional_776_components_keep_nested_profile_and_untrusted_lengths() {
    let cases = [(5,vec![1,0,0x3f,0x80,0,0]),(7,vec![0x3f,0x40,0,0]),(10,vec![1,b'x']),
        (17,vec![1,0x3f,0x80,0,0,1,1,1,1,b'x',1,255,255,255,255]),(34,vec![]),(70,vec![0,0,0,0,0,0,0,0])];
    for (kind,data) in cases {
        let stack=items::Stack { item:1,count:1,added:vec![items::Component {kind,data}],removed:vec![] };
        for version in [OLD,MID] {
            for untrusted in [false,true] {
                let encoded=items::encode(version,Some(&stack),untrusted,items::Context::default()).unwrap();
                assert_eq!(items::decode(version,&encoded,untrusted,items::Context::default()).unwrap(),(Some(stack.clone()),encoded.len()));
                for end in 0..encoded.len() { assert!(items::decode(version,&encoded[..end],untrusted,items::Context::default()).is_err(),"{kind}/{end}"); }
            }
        }
    }
}

#[test]
fn fresh_777_authored_block_entity_reference_uses_the_776_post_bed_mapping() {
    let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/step1-774-776/official777-blockentities-reference/result.json")).unwrap();
    assert_eq!(data["status"],"pass");
    assert_eq!(data["entities"].as_array().unwrap().len(),43);
    for row in data["entities"].as_array().unwrap() {
        let kind=row["kind"].as_u64().unwrap() as u32;
        let raw=hex(&row["nbt_hex"]);
        assert_eq!(crate::block_entities::translate(JavaVersion::V26_3,MID,kind,&raw).unwrap(),Some((kind,raw.clone())),"{}",row["block"]);
        for end in 0..raw.len() { assert!(crate::block_entities::translate(JavaVersion::V26_3,MID,kind,&raw[..end]).is_err()); }
        let mut extra=raw;extra.push(0);assert!(crate::block_entities::translate(JavaVersion::V26_3,MID,kind,&extra).is_err());
    }
}

fn nbt_string(value: &str) -> Vec<u8> {
    let mut out=(value.len() as u16).to_be_bytes().to_vec();out.extend(value.as_bytes());out
}
fn nbt_fields(fields: &[(u8,&str,Vec<u8>)]) -> Vec<u8> {
    let mut out=Vec::new();
    for (kind,key,value) in fields {out.push(*kind);out.extend(nbt_string(key));out.extend(value);}
    out.push(0);out
}
#[test]
fn rich_776_text_preserves_nested_styles_translation_and_text_events_with_bounds() {
    let mut child=vec![10,0,0,0,1];child.extend(nbt_fields(&[(8,"text",nbt_string("B")),(1,"bold",vec![1])]));
    let translated=nbt_fields(&[(8,"translate",nbt_string("chat.type.text")),(8,"fallback",nbt_string("%s"))]);
    let hover=nbt_fields(&[(8,"action",nbt_string("show_text")),(10,"value",translated)]);
    let click=nbt_fields(&[(8,"action",nbt_string("copy_to_clipboard")),(8,"value",nbt_string("MCSA"))]);
    let mut text=vec![10];text.extend(nbt_fields(&[(8,"text",nbt_string("A")),(9,"extra",child),(10,"hover_event",hover),(10,"click_event",click)]));
    items::plain_text(&text).unwrap();
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:6,data:text.clone()}],removed:vec![]};
    for untrusted in [false,true] {
        let encoded=items::encode(MID,Some(&stack),untrusted,items::Context::default()).unwrap();
        assert_eq!(items::decode(MID,&encoded,untrusted,items::Context::default()).unwrap().0,Some(stack.clone()));
    }
    for end in 0..text.len(){assert!(items::plain_text(&text[..end]).is_err());}
    let mut tail=text.clone();tail.push(0);assert!(items::plain_text(&tail).is_err());
    let mut nested=nbt_fields(&[(8,"text",nbt_string("A"))]);
    for _ in 0..34 {
        let mut list=vec![10,0,0,0,1];list.extend(nested);
        nested=nbt_fields(&[(8,"text",nbt_string("A")),(9,"extra",list)]);
    }
    let mut too_deep=vec![10];too_deep.extend(nested);assert!(items::plain_text(&too_deep).is_err());
    let hover=nbt_fields(&[(8,"action",nbt_string("show_item")),(8,"id",nbt_string("minecraft:stone"))]);
    let mut unremapped=vec![10];unremapped.extend(nbt_fields(&[(8,"text",nbt_string("A")),(10,"hover_event",hover)]));
    assert!(items::plain_text(&unremapped).is_err());
}

#[test]
#[ignore="экспорт переведённых payload776 для непрозрачного клиента в частном реле"]
fn export_live_776_payloads() { export_live_payloads(MID); }

#[test]
#[ignore="экспорт payload774 для непрозрачного клиента"]
fn export_live_774_payloads() { export_live_payloads(JavaVersion::V1_21_11); }

#[test]
#[ignore="экспорт payload773 для непрозрачного клиента"]
fn export_live_773_payloads() {export_live_payloads(JavaVersion::V1_21_9);}

#[test]
#[ignore="экспорт payload772 для непрозрачного клиента"]
fn export_live_772_payloads() {export_live_payloads(JavaVersion::V1_21_8);}

fn export_live_payloads(target: JavaVersion) {
    let adapter = if target==JavaVersion::V1_21_8 {crate::client_772::clientbound} else if target==JavaVersion::V1_21_9 {crate::client_773::clientbound} else if target==JavaVersion::V1_21_11 {crate::client_774::clientbound} else {crate::probe_776::clientbound};
    let packet_id=|old:u32|crate::packet_layouts::packet_info(crate::ids::RegistryKind::PlayClientbound,OLD,target,old as usize).unwrap().target_id.unwrap() as u8;
    let mut packets=Vec::new();
    let mut add=|label:String,body:Vec<u8>| {
        packets.push(serde_json::json!({"label":label,"hex":body.iter().map(|b|format!("{b:02x}")).collect::<String>()}));
    };
    let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
    for row in data["old"]["cases"].as_array().unwrap() {
        let (stack,_)=items::decode(OLD,&hex(&row["slot_hex"]),false,items::Context::default()).unwrap();
        let mut body=vec![packet_id(0x6c),0];body.extend(items::encode(target,stack.as_ref(),false,items::Context::default()).unwrap());
        add(format!("complex-item:{}",row["name"]),body);
    }
    for (kind,data) in [(5,vec![1,0,0x3f,0x80,0,0]),(7,vec![0x3f,0x40,0,0]),(10,vec![1,b'x']),
        (17,vec![1,0x3f,0x80,0,0,1,1,1,1,b'x',1,255,255,255,255]),(34,vec![]),(70,vec![0,0,0,0,0,0,0,0])] {
        let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind,data}],removed:vec![]};
        let mut body=vec![packet_id(0x6c),0];body.extend(items::encode(target,Some(&stack),false,items::Context::default()).unwrap());add(format!("component:{kind}"),body);
    }
    let mut list=vec![10,0,0,0,1];list.extend(nbt_fields(&[(8,"text",nbt_string("B")),(1,"bold",vec![1])]));
    let hover=nbt_fields(&[(8,"action",nbt_string("show_text")),(8,"value",nbt_string("MCSA tooltip"))]);
    let mut rich=vec![10];rich.extend(nbt_fields(&[(8,"text",nbt_string("A")),(9,"extra",list),(10,"hover_event",hover)]));
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:6,data:rich}],removed:vec![]};
    let mut body=vec![packet_id(0x6c),0];body.extend(items::encode(target,Some(&stack),false,items::Context::default()).unwrap());add("rich-name".into(),body);
    let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/particle-observations.json")).unwrap();
    for row in data["old"]["particles"].as_array().unwrap() {add(format!("particle:{}",row["case"]),cb(OLD,target,&hex(&row["body"]),Context::default()).unwrap());}
    let mut tracker=Tracker::default();
    let kind=crate::entity_layouts::LAYOUTS.iter().position(|row|row.0=="mannequin").unwrap() as u32;
    let mut spawn=vec![1];wire::uint(&mut spawn,6000);spawn.extend([6;16]);wire::uint(&mut spawn,kind);
    for value in [3.5f64,64.0,3.5] {spawn.extend(value.to_be_bytes());}spawn.extend([0;5]);
    add("mannequin-spawn".into(),adapter(&mut tracker,&spawn,Context::default()).unwrap().into_owned());
    let field=crate::entity_layouts::LAYOUTS[kind as usize].1.iter().position(|v|*v=="profile").unwrap() as u8;
    let mut metadata=vec![0x63];wire::uint(&mut metadata,6000);metadata.extend([field,41,0,0,0,0,0,0,0,0,255]);
    add("serializer41".into(),adapter(&mut tracker,&metadata,Context::default()).unwrap().into_owned());
    let mut destroy=vec![packet_id(0x4d),1];wire::uint(&mut destroy,6000);add("mannequin-remove".into(),destroy);
    let data:serde_json::Value=serde_json::from_str(include_str!("../testdata/step1-774-776/official777-blockentities-reference/result.json")).unwrap();
    let packet=(0..256).find_map(|id|crate::packet_layouts::packet_info(crate::ids::RegistryKind::PlayClientbound,OLD,target,id).filter(|r|r.name=="tile_entity_data").map(|_|id as u32)).unwrap();
    for row in data["entities"].as_array().unwrap() {
        let kind=row["kind"].as_u64().unwrap() as u32;
        let (kind,nbt)=crate::block_entities::translate(JavaVersion::V26_3,OLD,kind,&hex(&row["nbt_hex"])).unwrap().unwrap();
        let position=row["position"].as_array().unwrap();let x=position[0].as_i64().unwrap();let y=position[1].as_i64().unwrap()-1;let z=position[2].as_i64().unwrap();
        let mut body=Vec::new();wire::uint(&mut body,packet);body.extend((((x as u64&0x3ffffff)<<38)|((z as u64&0x3ffffff)<<12)|(y as u64&4095)).to_be_bytes());
        wire::uint(&mut body,kind);body.extend(nbt);add(format!("block-entity:{}",row["block"]),cb(OLD,target,&body,Context::default()).unwrap());
    }
    for (block,old_kind,nbt) in [
        ("oak_shelf",40,item_block_nbt("minecraft:stone")),
        ("campfire",33,item_block_nbt("minecraft:chicken")),
        ("vault",45,{let mut nbt=vec![10];nbt.extend(nbt_fields(&[(10,"shared_data",nbt_fields(&[(10,"display_item",displayed_item("minecraft:diamond",false))]))]));nbt}),
        ("spawner",9,{let mut nbt=vec![10];nbt.extend(nbt_fields(&[(10,"SpawnData",nbt_fields(&[(10,"entity",nbt_fields(&[(8,"id",nbt_string("minecraft:pig"))]))]))]));nbt}),
        ("trial_spawner",44,{let mut nbt=vec![10];nbt.extend(nbt_fields(&[(10,"spawn_data",nbt_fields(&[(10,"entity",nbt_fields(&[(8,"id",nbt_string("minecraft:pig"))]))]))]));nbt}),
    ] {
        let row=data["entities"].as_array().unwrap().iter().find(|r|r["block"]==block).unwrap();
        let position=row["position"].as_array().unwrap();let x=position[0].as_i64().unwrap();let y=position[1].as_i64().unwrap()-1;let z=position[2].as_i64().unwrap();
        let mut body=Vec::new();wire::uint(&mut body,packet);body.extend((((x as u64&0x3ffffff)<<38)|((z as u64&0x3ffffff)<<12)|(y as u64&4095)).to_be_bytes());
        wire::uint(&mut body,old_kind);body.extend(nbt);add(format!("populated-block-entity:{block}"),cb(OLD,target,&body,Context::default()).unwrap());
    }
    for (kind,data) in remaining_primary_components() {
        let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind,data}],removed:vec![]};
        let mut body=vec![packet_id(0x6c),0];body.extend(items::encode(target,Some(&stack),false,items::Context::default()).unwrap());
        add(format!("primary-component:{kind}"),body);
    }
    let mut statue=Vec::new();wire::uint(&mut statue,packet);statue.extend((((-16i64 as u64&0x3ffffff)<<38)|((-13i64 as u64&0x3ffffff)<<12)|64).to_be_bytes());
    statue.extend([48,10,0]);let context=Context{block_entity_view:(target==JavaVersion::V1_21_8).then(||crate::generated_payload_772::REMOVED_BE_VIEWS.iter().find(|r|r.0==48).unwrap().1),..Default::default()};
    add("block-entity:copper_golem_statue".into(),cb(OLD,target,&statue,context).unwrap());
    let entities:serde_json::Value=serde_json::from_str(include_str!("../testdata/entity-observations.json")).unwrap();
    let context=Context{metadata_registry_counts:&[("minecraft:painting_variant",51),("minecraft:wolf_variant",9)],..Default::default()};
    let mut tracker=Tracker::default();
    for (index,row) in entities["old"]["entities"].as_array().unwrap().iter().enumerate() {
        let entity=6100+index as u32;
        for (number,packet) in row["packets"].as_array().unwrap().iter().enumerate() {
            let raw=hex(&packet["body"]);let mut input=wire::Cursor::new(&raw);let id=input.uint().unwrap();input.uint().unwrap();
            let mut source=Vec::new();wire::uint(&mut source,id);wire::uint(&mut source,entity);source.extend(input.rest);
            add(format!("entity:{}:{number}",row["type"]),adapter(&mut tracker,&source,context).unwrap().into_owned());
        }
    }
    let mut destroy=vec![packet_id(0x4d),11];for entity in 6100..6111 {wire::uint(&mut destroy,entity);}add("entity-samples-remove".into(),destroy);
    let context=Context{items:items::Context{content:crate::content::ContentCodec::white_slab_preview(),..Default::default()},..Default::default()};
    let mut tracker=Tracker::default();
    for (index,canonical) in [157,158,159,160].into_iter().enumerate() {
        let entity=6200+index as u32;let mut source=vec![1];wire::uint(&mut source,entity);source.extend([40+index as u8;16]);wire::uint(&mut source,canonical);
        for coordinate in [3.5f64,64.0,5.5+index as f64] {source.extend(coordinate.to_be_bytes());}source.extend([0;5]);
        add(format!("preview-entity:{canonical}:spawn"),adapter(&mut tracker,&source,context).unwrap().into_owned());
        let mut source=vec![0x63];wire::uint(&mut source,entity);
        source.extend(match canonical{159=>&[8,1,14,255][..],160=>&[18,1,2,255],_=>&[0,0,1,255]});
        add(format!("preview-entity:{canonical}:metadata"),adapter(&mut tracker,&source,context).unwrap().into_owned());
    }
    let mut destroy=vec![packet_id(0x4d),4];for entity in 6200..6204 {wire::uint(&mut destroy,entity);}add("preview-entities-remove".into(),destroy);
    let filename=format!("tmp/{}-payload-live.json",target.protocol());let path=std::path::Path::new(&filename);std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"protocol":target.protocol(),"packets":packets})).unwrap()).unwrap();
}

#[test]
fn actual_776_update_requires_compound_while_chunk_empty_nbt_remains_optional() {
    let mut body=vec![6];body.extend([0;8]);body.extend([1,0]);
    let mut expected=body[..body.len()-1].to_vec();expected.extend([10,0]);
    assert_eq!(cb(OLD,MID,&body,Context::default()).unwrap(),expected);
    assert_eq!(crate::block_entities::translate(OLD,MID,1,&[0]).unwrap(),Some((1,vec![0])));
    for end in 0..body.len(){assert!(cb(OLD,MID,&body[..end],Context::default()).is_err());}
}

fn displayed_item(name:&str,slot:bool)->Vec<u8>{
    let mut fields=vec![(3,"count",1i32.to_be_bytes().to_vec()),(8,"id",nbt_string(name))];
    if slot{fields.push((1,"Slot",vec![0]));}nbt_fields(&fields)
}
fn item_block_nbt(name:&str)->Vec<u8>{
    let mut list=vec![10,0,0,0,1];list.extend(displayed_item(name,true));
    let mut out=vec![10];out.extend(nbt_fields(&[(9,"Items",list)]));out
}
#[test]
fn filled_776_block_entities_remap_named_items_without_mutating_canonical_data() {
    let raw=item_block_nbt("minecraft:white_concrete_slab");let before=raw.clone();
    let content=crate::content::ContentCodec::white_slab_preview();
    let mapped=crate::block_entities::translate_with_content(OLD,MID,40,&raw,content).unwrap().unwrap();
    assert_eq!(mapped,(39,item_block_nbt("minecraft:quartz_slab")));assert_eq!(raw,before);
    assert!(crate::block_entities::translate(OLD,MID,40,&raw).is_err());
    for kind in [33,40] {
        let raw=item_block_nbt("minecraft:stone");
        assert_eq!(crate::block_entities::translate(OLD,MID,kind,&raw).unwrap().unwrap().1,raw);
        for end in 0..raw.len(){assert!(crate::block_entities::translate(OLD,MID,kind,&raw[..end]).is_err());}
        assert!(crate::block_entities::translate(OLD,MID,kind,&item_block_nbt("minecraft:unverified_item")).is_err());
    }
    let mut vault=vec![10];vault.extend(nbt_fields(&[(10,"shared_data",nbt_fields(&[(10,"display_item",displayed_item("minecraft:diamond",false))]))]));
    assert_eq!(crate::block_entities::translate(OLD,MID,45,&vault).unwrap().unwrap().1,vault);
    let mut bad=vec![10];bad.extend(nbt_fields(&[(9,"Items",vec![10,0,0,1,0]) ]));
    assert!(crate::block_entities::translate(OLD,MID,40,&bad).is_err());
}

fn remaining_primary_components()->Vec<(u32,Vec<u8>)>{
    let mut debug=vec![10];debug.extend(nbt_fields(&[(8,"minecraft:oak_log",nbt_string("axis"))]));
    let mut bucket=vec![10];bucket.extend(nbt_fields(&[(3,"Age",0i32.to_be_bytes().to_vec()),(5,"Health",10f32.to_be_bytes().to_vec()),(1,"NoAI",vec![0])]));
    let mut recipes=vec![9,8,0,0,0,1];recipes.extend(nbt_string("minecraft:oak_planks"));
    let mut boom=vec![2,1];boom.extend(0x00ff0000i32.to_be_bytes());boom.extend([0,1,1]);
    let mut fireworks=vec![1,1];fireworks.extend(&boom);
    let mut props=vec![1];wire::uint(&mut props,11);props.extend(b"honey_level");props.extend([1,b'5']);
    let mut bees=vec![1,10];bees.extend(nbt_fields(&[(8,"id",nbt_string("minecraft:bee")),(3,"Age",0i32.to_be_bytes().to_vec())]));
    bees.push(20);wire::uint(&mut bees,2400);
    vec![(53,vec![1,22,7]),(56,vec![1,1]),(57,debug),(59,bucket),(61,vec![1,1]),(66,recipes),
        (68,boom),(69,fireworks),(72,vec![1,1,15]),(74,vec![4,1,1,1,1]),(76,props),(77,bees),
        (86,vec![1]),(88,vec![2]),(89,vec![3]),(90,vec![4]),(104,vec![1])]
}
#[test]
fn remaining_primary_776_component_shapes_preserve_ids_lengths_and_reject_truncation() {
    for (kind,data) in remaining_primary_components() {
        let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind,data}],removed:vec![]};
        for untrusted in [false,true] {
            let encoded=items::encode(MID,Some(&stack),untrusted,items::Context::default()).unwrap();
            assert_eq!(items::decode(MID,&encoded,untrusted,items::Context::default()).unwrap(),(Some(stack.clone()),encoded.len()),"{kind}");
            for end in 0..encoded.len(){assert!(items::decode(MID,&encoded[..end],untrusted,items::Context::default()).is_err(),"{kind}/{end}");}
        }
    }
}

#[test]
fn live_776_instrument_uses_direct_holder_and_canonical_writer_keeps_legacy_bool() {
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:61,data:vec![1,1]}],removed:vec![]};
    let kind=crate::ids::translate(crate::ids::RegistryKind::Component,OLD,MID,61).unwrap();
    let mut expected=vec![1,1,1,0];wire::uint(&mut expected,kind);expected.push(1);
    assert_eq!(items::encode(MID,Some(&stack),false,items::Context::default()).unwrap(),expected);
    assert_eq!(items::decode(MID,&expected,false,items::Context::default()).unwrap(),(Some(stack.clone()),expected.len()));
    let mut bounded=vec![1,1,1,0];wire::uint(&mut bounded,kind);bounded.extend([2,1,1]);
    assert!(items::decode(MID,&bounded,true,items::Context::default()).is_err());
    let failed:serde_json::Value=serde_json::from_str(include_str!("../testdata/step1-774-776/payload-actions-instrument775/result.json")).unwrap();
    assert_eq!(failed["status"],"failed");
    let rows=failed["payloads"].as_array().unwrap();
    assert_eq!(rows.iter().filter(|row|row["post_pong"]==true).count(),139);
    assert_eq!(rows.last().unwrap()["label"],"primary-component:61");
}

#[test]
fn bee_component_776_remaps_entity_prefix_and_rejects_other_occupants() {
    let data=remaining_primary_components().into_iter().find(|row|row.0==77).unwrap().1;
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:77,data:data.clone()}],removed:vec![]};
    let encoded=items::encode(MID,Some(&stack),false,items::Context::default()).unwrap();
    let bee=crate::entity_layouts::LAYOUTS.iter().position(|row|row.0=="bee").unwrap() as u32;
    let bee=crate::ids::translate(crate::ids::RegistryKind::Entity,OLD,MID,bee).unwrap();
    let kind=crate::ids::translate(crate::ids::RegistryKind::Component,OLD,MID,77).unwrap();
    let mut expected=vec![1,1,1,0];wire::uint(&mut expected,kind);expected.push(1);wire::uint(&mut expected,bee);expected.extend(&data[1..]);
    assert_eq!(encoded,expected);
    let mut bad=expected;bad[6]=255;assert!(items::decode(MID,&bad,false,items::Context::default()).is_err());
    assert_eq!(crate::block_entities::translate(OLD,MID,48,&[10,0]).unwrap(),Some((47,vec![10,0])));
    assert!(crate::block_entities::translate(OLD,MID,48,&[10,1,0,1,b'x',1,0]).is_err());
}

#[test]
fn actual_774_bees_have_a_typed_entity_prefix_without_changing_canonical_nbt() {
    let data=remaining_primary_components().into_iter().find(|r|r.0==77).unwrap().1;
    let stack=items::Stack{item:1,count:1,added:vec![items::Component{kind:77,data:data.clone()}],removed:vec![]};
    let old=JavaVersion::V1_21_11;
    let encoded=items::encode(old,Some(&stack),false,Default::default()).unwrap();
    let mut cursor=wire::Cursor::new(&encoded);for _ in 0..5{cursor.uint().unwrap();}
    assert_eq!(cursor.uint().unwrap(),1);
    let bee=crate::entity_layouts::LAYOUTS.iter().position(|r|r.0=="bee").unwrap() as u32;
    assert_eq!(cursor.uint().unwrap(),crate::ids::translate(crate::ids::RegistryKind::Entity,OLD,old,bee).unwrap());
    assert_eq!(cursor.rest[0],10);
    assert_eq!(items::decode(old,&encoded,false,Default::default()).unwrap(),(Some(stack),encoded.len()));
    assert_eq!(data[0],1);
}
