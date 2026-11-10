// Свои новые реальные WASM; старые BlockBreak и place baseline не меняются.
fn place_copy(host_root: &Path, source_set: &str, name: &str) {
    let source=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp").join(source_set).join("plugins").join(name);
    let target=host_root.join("plugins").join(name);std::fs::create_dir_all(target.join("data")).unwrap();
    for file in ["plugin.wasm","plugin.toml"] {std::fs::copy(source.join(file),target.join(file)).unwrap();}
}
fn place_messages(shared: &Shared)->Vec<Vec<u8>> {
    shared.players.lock().unwrap().take_orders(&[1;16]).into_iter().filter_map(|o|match o {crate::players::Order::Message{component}=>Some(component),_=>None}).collect()
}
#[test]
#[ignore = "actual new Place WASM plus immutable old Place baseline"]
fn translated_bukkit_place_public_and_whole_legacy_fallback() {
    let spec:serde_json::Value=serde_json::from_str(include_str!("../../tools/plugin-translator/fixtures/place-staged-expected.json")).unwrap();
    let mut rows=Vec::new();
    for legacy in [None,Some("a"),Some("b")] {
        for case in spec["cohorts"].as_array().unwrap() {
            let letters:Vec<_>=case["load_order"].as_array().unwrap().iter().map(|s|s.as_str().unwrap()).collect();
            if legacy.is_some() && letters.len()!=2 {continue;}
            let mode=case["mode"].as_str().unwrap();
            // Старые fixture знают только исходные три режима; не выдаём их за новые.
            if legacy.is_some() && !matches!(mode,"order"|"cancel"|"uncancel") {continue;}
            let root=server_root(&format!("place-public-{}-{}",legacy.unwrap_or("new"),case["name"].as_str().unwrap()));
            let names:Vec<_>=letters.iter().map(|letter|format!("java-place-{}-{letter}",if legacy==Some(*letter) {"priority"} else {"staged"})).collect();
            for (letter,name) in letters.iter().zip(&names) {place_copy(&root,if legacy==Some(*letter){"place-old-baseline"}else{"place-staged"},name);}
            let shared=shared();java_info(&shared);let mut host=Host::new(Arc::clone(&shared),root).unwrap();
            for name in &names {host.load(name).unwrap();}
            assert_eq!(host.block_place_mode().contains("общим приоритетам"),legacy.is_none());
            for letter in &letters {
                let command=format!("place{}{letter}",if legacy==Some(*letter){"priority"}else{"staged"});
                let sender=wit::Sender{kind:wit::SenderKind::Player,uuid:Some(crate::network::login::format_uuid(&[1;16])),name:"Первый".into()};
                assert!(host.run_command(sender,&command,&[mode.into()]).unwrap().0);
            }
            place_messages(&shared);
            let verdict=host.block_place(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:64,z:3},"minecraft:stone");
            let reference=&case[if legacy.is_none(){"public_contract"}else{"source_predicted_legacy"}];
            assert_eq!(verdict==Verdict::Allow,reference["verdict"]=="allow");assert_eq!(host.loaded.len(),letters.len());
            let expected:Vec<_>=reference["trace"].as_array().unwrap().iter().map(|s|{
                let s=s.as_str().unwrap();let letter=s[..1].to_ascii_lowercase();
                let label=if legacy==Some(letter.as_str()){s.rsplit_once(" b").unwrap().0}else{s};
                crate::network::nbt::text_component(&format!("PLACE|{label}|{}|world|3|64|3|STONE|same=1",crate::network::login::format_uuid(&[1;16])))
            }).collect();
            let raw=place_messages(&shared);assert_eq!(raw,expected,"case={case} legacy={legacy:?}");
            let boundaries=host.priority_test_trace().to_vec();
            let mut cross_place_raw=Vec::new();
            if legacy.is_none() {
                assert_eq!(boundaries.len(),letters.len()*8);
                for row in boundaries.iter().filter(|r|r["stage"]=="finish") {
                    assert_eq!(row["cancelled"],reference["final"]["cancelled"]);assert_eq!(row["can_build"],reference["final"]["can_build"]);
                }
                // Эти компоненты имеют оба namespace: два Break не вызывают Place listeners.
                assert_eq!(host.block_break(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:64,z:3},"minecraft:stone"),Verdict::Allow);
                assert!(place_messages(&shared).is_empty());
                assert_eq!(host.block_place(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:64,z:3},"minecraft:stone"),verdict);
                let cross=place_messages(&shared);assert_eq!(cross,expected);cross_place_raw=cross;
                assert_eq!(host.block_break(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:64,z:3},"minecraft:stone"),Verdict::Allow);
                assert!(place_messages(&shared).is_empty());
            } else {assert!(boundaries.is_empty());}
            rows.push(serde_json::json!({"case":case["name"],"legacy":legacy,"mode":host.block_place_mode(),"boundaries":boundaries,"cross_boundaries":host.priority_test_trace(),"cross_place_raw_nbt":cross_place_raw.iter().map(|s|s.iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>(),"raw_nbt":raw.iter().map(|s|s.iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>() }));
            host.unload_all();assert!(host.is_empty());
        }
    }
    let output=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/place-staged/host-contract.json");assert!(!output.exists());std::fs::write(output,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore = "actual new Place WASM; raw frame failures are not runtime traps"]
fn translated_bukkit_place_frames_two_states_order_replay_capacity() {
    use super::priority::PlaceState;
    let root=server_root("place-frame-negatives");place_copy(&root,"place-staged","java-place-staged-a");
    let shared=shared();java_info(&shared);let mut host=Host::new(shared,root).unwrap();host.load("java-place-staged-a").unwrap();
    let who=view();let state=PlaceState{cancelled:false,can_build:false};
    for operation in [1,2] {assert!(host.place_priority_probe(operation,1,0,state,&who).unwrap_err().contains("Missing"));}
    assert!(host.place_priority_probe(0,0,0,state,&who).unwrap_err().contains("Missing"));
    host.place_priority_probe(0,1,0,state,&who).unwrap();
    assert!(host.place_priority_probe(0,1,0,state,&who).unwrap_err().contains("Duplicate"));
    assert!(host.place_priority_probe(1,1,6,state,&who).unwrap_err().contains("Priority"));
    assert!(host.place_priority_probe(1,1,1,state,&who).unwrap_err().contains("Order"));
    assert!(host.place_priority_probe(2,1,0,state,&who).unwrap_err().contains("Order"));
    assert_eq!(host.place_priority_probe(1,1,0,state,&who).unwrap(),state);
    assert!(host.place_priority_probe(1,1,0,state,&who).unwrap_err().contains("Order"));
    for priority in 1..6 {
        // Each import overwrites BOTH state fields, including false build with uncancelled.
        let incoming=PlaceState{cancelled:priority%2==0,can_build:priority%3==0};
        assert_eq!(host.place_priority_probe(1,1,priority,incoming,&who).unwrap(),incoming);
    }
    host.place_priority_probe(2,1,0,state,&who).unwrap();
    assert!(host.place_priority_probe(2,1,0,state,&who).unwrap_err().contains("Missing"));
    assert!(host.place_priority_probe(0,1,0,state,&who).unwrap_err().contains("Order"));
    for id in 2..34 {host.place_priority_probe(0,id,0,state,&who).unwrap();}
    assert!(host.place_priority_probe(0,34,0,state,&who).unwrap_err().contains("Capacity"));
    host.unload_all();host.load("java-place-staged-a").unwrap();
    host.place_priority_probe(0,u64::MAX,0,state,&who).unwrap();
    for priority in 0..6 {assert_eq!(host.place_priority_probe(1,u64::MAX,priority,state,&who).unwrap(),state);}
    host.place_priority_probe(2,u64::MAX,0,state,&who).unwrap();
    assert!(host.place_priority_probe(0,u64::MAX,0,state,&who).unwrap_err().contains("Order"));
    assert!(host.place_priority_probe(0,1,0,state,&who).unwrap_err().contains("Order"));
    host.unload_all();assert!(host.is_empty());
}

#[test]
#[ignore = "actual own new Place fault/empty components; cumulative budgets"]
fn translated_bukkit_place_faults_retain_both_flags_and_finish_survivors() {
    use std::time::Duration;
    let mut captures=Vec::new();
    for (fault,material) in [("begin","stone"),("phase","dirt"),("finish","granite"),("fuel","andesite"),("guest","diorite"),("native","calcite")] {
        for mode in ["order","cancel","buildfalse","uncancelonly"] {
            let root=server_root(&format!("place-fault-{fault}-{mode}"));
            for suffix in ["a","fault","empty"] {place_copy(&root,"place-staged",&format!("java-place-staged-{suffix}"));}
            let shared=shared();java_info(&shared);let mut host=Host::new(Arc::clone(&shared),root.clone()).unwrap();
            for suffix in ["a","fault","empty"] {host.load(&format!("java-place-staged-{suffix}")).unwrap();}
            let sender=wit::Sender{kind:wit::SenderKind::Player,uuid:Some(crate::network::login::format_uuid(&[1;16])),name:"Первый".into()};
            assert!(host.run_command(sender,"placestageda",&[mode.into()]).unwrap().0);place_messages(&shared);
            if fault=="fuel" {host.priority_test_fuel(50_000_000);}
            if fault=="guest" {host.priority_test_fuel(u64::MAX);host.priority_test_budget(100,Duration::from_millis(1000),Duration::ZERO);}
            if fault=="native" {host.priority_test_budget(40,Duration::from_millis(1000),Duration::from_millis(250));}
            let cancelled=matches!(mode,"cancel"|"uncancelonly");let can_build=matches!(mode,"order"|"cancel");
            let verdict=host.block_place(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:64,z:3},&format!("minecraft:{material}"));
            assert_eq!(verdict==Verdict::Deny,cancelled||!can_build);assert_eq!(host.loaded.len(),2);
            assert!(!host.is_loaded("java-place-staged-fault"));
            let rows=host.priority_test_trace().to_vec();
            let failed:Vec<_>=rows.iter().filter(|r|r["plugin"]=="java-place-staged-fault"&&r["ok"]==false).collect();assert_eq!(failed.len(),1);
            assert_eq!(failed[0]["stage"],if fault=="begin"{"begin"}else if fault=="finish"{"finish"}else{"phase"});
            if matches!(fault,"fuel"|"guest"|"native") {
                let successful=rows.iter().filter(|r|r["plugin"]=="java-place-staged-fault"&&r["stage"]=="phase"&&r["ok"]==true).count();assert!((1..6).contains(&successful),"budget must exhaust between phases: {rows:?}");
            }
            for suffix in ["a","empty"] {
                assert!(rows.iter().any(|r|r["plugin"]==format!("java-place-staged-{suffix}")&&r["stage"]=="finish"&&r["ok"]==true&&r["cancelled"]==cancelled&&r["can_build"]==can_build));
            }
            if fault=="fuel" {assert_eq!(failed[0]["fuel"],0);}
            if fault=="native" {assert_eq!(failed[0]["native_epochs"],0);assert_eq!(failed[0]["native_ns"],0);assert!(root.join("plugins/java-place-staged-fault/data/native-budget-proof.txt").is_file());}
            captures.push(serde_json::json!({"fault":fault,"mode":mode,"cancelled":cancelled,"can_build":can_build,"boundaries":rows,"raw_nbt":place_messages(&shared).iter().map(|s|s.iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>() }));
            host.load("java-place-staged-fault").unwrap();assert_eq!(host.block_place(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:64,z:3},"minecraft:deepslate"),verdict);assert_eq!(host.loaded.len(),3);
            host.unload_all();assert!(host.is_empty());
        }
    }
    let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/place-staged/fault-capture.json");assert!(!path.exists());std::fs::write(path,serde_json::to_vec_pretty(&captures).unwrap()).unwrap();
}

#[test]
#[ignore = "actual new malformed Place components; namespace present"]
fn translated_bukkit_place_partial_and_wrong_type_are_rejected() {
    for kind in ["missing","type"] {
        let name=format!("java-place-staged-invalid-{kind}");let root=server_root(&format!("place-invalid-{kind}"));place_copy(&root,"place-staged",&name);
        let shared=shared();java_info(&shared);let mut host=Host::new(shared,root).unwrap();let error=host.load(&name).unwrap_err();
        assert!(host.is_empty());assert!(error.contains("optional block-place"),"unexpected rejection: {error}");
        assert!(host.command_level("placestageda").is_none());
        println!("PLACE_INVALID_EXPORT_REJECTED kind={kind} reason={error}");
    }
}
