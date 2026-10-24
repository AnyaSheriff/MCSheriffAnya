use super::*;
use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};

fn bytes(hex: &str) -> Vec<u8> { hex.as_bytes().as_chunks::<2>().0.iter().map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect() }
fn records() -> serde_json::Value { serde_json::from_str(include_str!("../testdata/entity-observations.json")).unwrap() }
fn context() -> wire::Context {
    // Проба согласовала Known Packs официального сервера. Kebab имеет index24
    // в обоих наборах; это разрешение относится только к данной записи.
    wire::Context { metadata_registry_counts: &[("minecraft:painting_variant",51)], ..Default::default() }
}
fn cb(body: &[u8], from: JavaVersion, to: JavaVersion, context: wire::Context) -> Result<Vec<u8>,wire::Error> {
    wire::translate_with_context(State::Play,Direction::Clientbound,from,to,body,context).map(Cow::into_owned)
}
fn normalize_uuid(mut packet: Vec<u8>) -> Vec<u8> {
    let mut input = Cursor::new(&packet); input.uint().unwrap(); input.uint().unwrap();
    let pos = packet.len()-input.remaining(); packet[pos..pos+16].fill(0); packet
}

#[test]
fn removed_bed_updates_are_validated_then_omitted_only_for_777() {
    let mut body=vec![6];body.extend([0;8]);body.extend([25,0]);
    let mut tracker=Tracker::default();
    assert!(tracker.translate_packets(OLD,NEW,&body,context()).unwrap().is_empty());
    assert_eq!(tracker.translate_packets(OLD,OLD,&body,context()).unwrap()[0],body);
    body.pop();
    assert!(tracker.translate_packets(OLD,NEW,&body,context()).is_err());
}

#[test]
fn official_spawns_and_metadata_match_in_both_directions() {
    let data = records();
    assert_eq!(data["old"]["entities"].as_array().unwrap().len(),11);
    assert_eq!(data["new"]["entities"].as_array().unwrap().len(),11);
    for (old,new) in data["old"]["entities"].as_array().unwrap().iter().zip(data["new"]["entities"].as_array().unwrap()) {
        assert_eq!(old["type"],new["type"]);
        for (from,to,source,target) in [(OLD,NEW,old,new),(NEW,OLD,new,old)] {
            let mut tracker = Tracker::default();
            for name in ["spawn_entity","entity_metadata"] {
                let packet = |entry: &serde_json::Value| bytes(entry["packets"].as_array().unwrap().iter().find(|p| p["name"]==name).unwrap()["body"].as_str().unwrap());
                let source = packet(source); let target = packet(target);
                let actual = tracker.translate(from,to,&source,context()).unwrap().into_owned();
                if name == "spawn_entity" { assert_eq!(normalize_uuid(actual),normalize_uuid(target),"{}",old["type"]); }
                else { assert_eq!(actual,target,"{}",old["type"]); }
            }
        }
    }
}

#[test]
fn official_respawns_preserve_previous_mode_and_negative_sea_level() {
    let data = records();
    for (old,new) in data["old"]["respawns"].as_array().unwrap().iter().zip(data["new"]["respawns"].as_array().unwrap()) {
        let old=bytes(old["body"].as_str().unwrap()); let new=bytes(new["body"].as_str().unwrap());
        assert_eq!(cb(&old,OLD,NEW,context()).unwrap(),new);
        assert_eq!(cb(&new,NEW,OLD,context()).unwrap(),old);
        for (from,to,source) in [(OLD,NEW,old),(NEW,OLD,new)] {
            for len in 0..source.len() { assert!(cb(&source[..len],from,to,context()).is_err()); }
            let mut bad=source.clone(); bad.push(0); assert!(cb(&bad,from,to,context()).is_err());
            *bad.last_mut().unwrap()=4; bad.remove(bad.len()-2); assert!(cb(&bad,from,to,context()).is_err());
        }
    }
}

#[test]
fn metadata_requires_spawn_and_failed_packets_do_not_change_tracking() {
    let data=records(); let entity=&data["old"]["entities"][2];
    let spawn=bytes(entity["packets"][0]["body"].as_str().unwrap());
    let meta=bytes(entity["packets"][1]["body"].as_str().unwrap());
    let mut tracker=Tracker::default();
    assert!(matches!(tracker.translate(OLD,NEW,&meta,context()),Err(wire::Error::Entity(Error::UnknownEntity(_)))));
    for len in 0..spawn.len() {
        assert!(tracker.translate(OLD,NEW,&spawn[..len],context()).is_err());
        assert!(tracker.entities.is_empty());
    }
    tracker.translate(OLD,NEW,&spawn,context()).unwrap();
    for len in 0..meta.len() { assert!(tracker.translate(OLD,NEW,&meta[..len],context()).is_err()); }
    assert_eq!(tracker.entities.len(),1);
    assert!(tracker.translate(OLD,NEW,&meta,context()).is_ok());
    let mut removed=Vec::new();
    let (id,_)=(0..141).find_map(|id| crate::generated_ids::packet_info(RegistryKind::PlayClientbound,false,id).filter(|(n,_)| *n=="entity_destroy").map(|info|(id,info))).unwrap();
    uint(&mut removed,id as u32); removed.extend([1,entity["id"].as_u64().unwrap() as u8]);
    let mut invalid=removed.clone(); invalid.push(0);
    assert!(tracker.translate(OLD,NEW,&invalid,context()).is_err()); assert_eq!(tracker.entities.len(),1);
    tracker.translate(OLD,NEW,&removed,context()).unwrap(); assert!(tracker.entities.is_empty());
    assert!(tracker.translate(OLD,NEW,&meta,context()).is_err());
}

#[test]
fn dimension_change_clears_entities_but_same_dimension_respawn_keeps_them() {
    let data=records(); let mut tracker=Tracker::default();
    let login=include_bytes!("../testdata/play-login-26.1.2.bin");
    tracker.translate(OLD,NEW,login,wire::Context {online_mode:Some(false),..context()}).unwrap();
    let entity=&data["old"]["entities"][0]; let spawn=bytes(entity["packets"][0]["body"].as_str().unwrap());
    tracker.translate(OLD,NEW,&spawn,context()).unwrap(); assert_eq!(tracker.entities.len(),2);
    let nether=bytes(data["old"]["respawns"][0]["body"].as_str().unwrap());
    tracker.translate(OLD,NEW,&nether,context()).unwrap(); assert_eq!(tracker.entities.len(),1);
    tracker.translate(OLD,NEW,&spawn,context()).unwrap();
    tracker.translate(OLD,NEW,&nether,context()).unwrap(); assert_eq!(tracker.entities.len(),2);
}

#[test]
fn duplicate_fields_unsupported_serializers_and_new_age_fields_are_not_dropped() {
    let slime=entity_layouts::LAYOUTS.iter().position(|v| v.0=="slime").unwrap() as u32;
    let context=wire::Context {entity_kind:Some(slime),..context()};
    assert_eq!(cb(&[0x63,4,16,1,4,255],OLD,NEW,context).unwrap(),[0x65,4,18,1,4,255]);
    for bad in [vec![0x63,4,16,1,4,16,1,2,255],vec![0x63,4,9,44,255],vec![0x63,4,254,0,0,255]] {
        assert!(cb(&bad,OLD,NEW,context).is_err());
    }
    assert!(matches!(cb(&[0x65,4,16,8,1,255],NEW,OLD,context),Err(wire::Error::Entity(Error::MissingField("baby")))));
    assert!(matches!(cb(&[0x65,4,17,8,0,255],NEW,OLD,context),Err(wire::Error::Entity(Error::MissingField("age_locked")))));
    // Пустой metadata без контекста тоже не означает известную сущность.
    assert_eq!(cb(&[0x63,4,255],OLD,NEW,wire::Context::default()),Err(wire::Error::MissingEntityContext));
}

#[test]
fn dynamic_registry_ids_need_explicit_shared_context() {
    let painting=entity_layouts::LAYOUTS.iter().position(|v| v.0=="painting").unwrap() as u32;
    let body=[0x63,9,9,34,25,255];
    assert!(cb(&body,OLD,NEW,wire::Context {entity_kind:Some(painting),..Default::default()}).is_err());
    assert!(cb(&body,OLD,NEW,wire::Context {entity_kind:Some(painting),..context()}).is_ok());
    for holder in [0,52,127] {
        let mut bad=body;bad[4]=holder;
        assert!(cb(&bad,OLD,NEW,wire::Context {entity_kind:Some(painting),..context()}).is_err());
    }
}

#[test]
fn optional_block_state_uses_zero_for_absence_and_direct_ids_otherwise() {
    let enderman=entity_layouts::LAYOUTS.iter().position(|v| v.0=="enderman").unwrap() as u32;
    let context=wire::Context {entity_kind:Some(enderman),..context()};
    // Перед bamboo_planks777вставлен новый блок: ошибка «+1» здесь видна.
    for (old,new) in [(0,0),(27,28)] {
        assert_eq!(cb(&[0x63,1,16,15,old,255],OLD,NEW,context).unwrap(),[0x65,1,16,15,new,255]);
        assert_eq!(cb(&[0x65,1,16,15,new,255],NEW,OLD,context).unwrap(),[0x63,1,16,15,old,255]);
    }
}

#[test]
fn poplar_boats_spawn_with_wire_kinds_only_with_the_preview_content() {
    let data=records(); let entity=&data["old"]["entities"][2];
    let spawn=bytes(entity["packets"][0]["body"].as_str().unwrap());
    let mut input=Cursor::new(&spawn); input.uint().unwrap(); input.uint().unwrap(); input.take(16).unwrap();
    let at=spawn.len()-input.remaining();
    let old_kind=input.uint().unwrap(); assert!(old_kind<128,"вид занимает один байт");
    let preview=wire::Context { items: items::Context { content: crate::content::ContentCodec::white_slab_preview(), ..Default::default() }, ..context() };
    for (internal,wire_kind) in [(157u8,106u8),(158,107)] {
        let mut patched=spawn[..at].to_vec(); uint(&mut patched,u32::from(internal)); patched.extend_from_slice(&spawn[at+1..]);
        let mut tracker=Tracker::default();
        let out=tracker.translate(OLD,NEW,&patched,preview).unwrap();
        let mut output=Cursor::new(&out); output.uint().unwrap(); output.uint().unwrap(); output.take(16).unwrap();
        assert_eq!(output.uint().unwrap(),u32::from(wire_kind));
        assert_eq!(tracker.entities.len(),1);
        // Без предварительного профиля такого вида нет.
        assert!(Tracker::default().translate(OLD,NEW,&patched,context()).is_err());
    }
}

#[test]
fn sulfur_cube_layout_and_ids_follow_the_observed_777_indices() {
    // Прогон официального 26.3: baby 16, size 18 (= Size + 1), from_bucket 20; внутренние индексы куба равны индексам 777.
    let cube=entity_layouts::LAYOUTS.iter().find(|v| v.0=="sulfur_cube").unwrap();
    assert_eq!(cube.1,cube.2);
    assert_eq!((cube.2[16],cube.2[17],cube.2[18],cube.2[19],cube.2[20]),("baby","age_locked","size","max_fuse","from_bucket"));
    assert_eq!(crate::content::entity_ids_of("sulfur_cube_spawn_egg"),Some((160,133)));
    assert_eq!(crate::content::ContentCodec::white_slab_preview().entity_layout_base(160),Some("sulfur_cube"));
    // Большой куб: здоровье 8,0 и размер 2 - как в записи официального сервера (09 03 41000000 12 01 02).
    let kind=entity_layouts::LAYOUTS.iter().position(|v| v.0=="sulfur_cube").unwrap() as u32;
    let context=wire::Context {entity_kind:Some(kind),..context()};
    let packet=[0x63,2,9,3,0x41,0,0,0,18,1,2,255];
    assert_eq!(cb(&packet,OLD,NEW,context).unwrap()[1..],packet[1..]);
}
