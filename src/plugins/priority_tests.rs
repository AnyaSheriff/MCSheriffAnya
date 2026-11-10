// Actual translated WASM; старые 12 baseline не меняются.
#[test]
#[ignore = "requires actual translated staged fixtures"]
fn translated_bukkit_staged_priority_contract() { capture_priority_contract("java-priority-staged",true); }

#[test]
#[ignore = "requires exact old/staged mixed components"]
fn translated_bukkit_staged_mixed_a_uses_full_legacy_fallback() { capture_priority_contract("java-priority-mixed-a",false); }

#[test]
#[ignore = "requires exact old/staged mixed components"]
fn translated_bukkit_staged_mixed_b_uses_full_legacy_fallback() { capture_priority_contract("java-priority-mixed-b",false); }

fn capture_priority_contract(fixture_folder: &str, public: bool) {
    use std::io::Write as _;
    let spec: serde_json::Value = serde_json::from_str(include_str!("../../tools/plugin-translator/fixtures/priority-expected.json")).unwrap();
    let cohorts = spec["cohorts"].as_array().unwrap();
    assert_eq!(cohorts.len(), 12);
    let mut labels = std::collections::BTreeSet::new();
    for cohort in cohorts {
        for reference in ["source_predicted_legacy", "public_contract"] {
            for label in cohort[reference]["trace"].as_array().unwrap() {
                labels.insert(label.as_str().unwrap().to_string());
            }
        }
    }
    let capture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp").join(fixture_folder).join("capture");
    std::fs::create_dir_all(&capture_dir).unwrap();
    let mut recorded = Vec::new();
    let mut public_mismatches = 0;
    let mut verdict_mismatches = 0;
    for order in [&["a", "b"][..], &["b", "a"][..], &["a"][..], &["b"][..]] {
        let root = server_root(&format!("staged-priority-{}", order.join("-")));
        for letter in order {
            let name = format!("java-priority-{letter}");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp").join(fixture_folder).join("plugins").join(&name);
            let folder = root.join("plugins").join(&name);
            std::fs::create_dir_all(folder.join("data")).unwrap();
            for file in ["plugin.toml", "plugin.wasm"] {
                std::fs::copy(source.join(file), folder.join(file)).unwrap();
            }
        }
        let shared = shared();
        java_info(&shared);
        let mut host = Host::new(Arc::clone(&shared), root).unwrap();
        for letter in order { host.load(&format!("java-priority-{letter}")).unwrap(); }
        let sender = wit::Sender { kind: wit::SenderKind::Player, uuid: Some(crate::network::login::format_uuid(&[1;16])), name: "Первый".into() };
        for mode in ["order", "cancel", "uncancel"] {
            let name = format!("{}-{mode}", order.join("-"));
            let reference = cohorts.iter().find(|cohort| cohort["name"].as_str() == Some(name.as_str())).unwrap();
            for letter in order {
                let result = host.run_command(sender.clone(), &format!("priority{letter}"), &[mode.into()]).unwrap();
                let messages: Vec<_> = shared.players.lock().unwrap().take_orders(&[1;16]).into_iter().filter_map(|order| match order {
                    crate::players::Order::Message { component } => Some(component),
                    _ => None,
                }).collect();
                assert!(result.0, "fixture mode command failed: {name}/{letter}");
                assert_eq!(messages, [crate::network::nbt::text_component(&format!("mode {} {mode}", letter.to_ascii_uppercase()))]);
            }
            let verdict = match host.block_break(&view(), &wit::BlockPos { dimension: wit::Dimension::Overworld, x: 3, y: 70, z: 0 }, "minecraft:stone") {
                Verdict::Allow => "allow",
                Verdict::Deny => "deny",
            };
            assert_eq!(host.loaded.len(), order.len());
            let messages: Vec<_> = shared.players.lock().unwrap().take_orders(&[1;16]).into_iter().filter_map(|order| match order {
                crate::players::Order::Message { component } => Some(component),
                _ => None,
            }).collect();
            let raw: Vec<String> = messages.iter().map(|bytes| bytes.iter().map(|byte| format!("{byte:02x}")).collect()).collect();
            let trace: Vec<String> = messages.iter().zip(&raw).map(|(component, hex)| {
                labels.iter().find(|label| component == &crate::network::nbt::text_component(label.as_str())).cloned().unwrap_or_else(|| format!("<unknown NBT {hex}>"))
            }).collect();
            let public_match = serde_json::json!(&trace) == reference["public_contract"]["trace"] && verdict == reference["public_contract"]["verdict"].as_str().unwrap();
            if !public_match { public_mismatches += 1; }
            if verdict != reference["public_contract"]["verdict"].as_str().unwrap() { verdict_mismatches += 1; }
            let row = serde_json::json!({"name":&name,"load_order":order,"mode":mode,"trace":&trace,"verdict":verdict,"raw_message_nbt_hex":&raw,"public_contract_match":public_match});
            println!("PRIORITY_STAGED_CONTRACT {row}");
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(capture_dir.join(format!("{}-{}.json", name, std::process::id()))).unwrap();
            writeln!(file, "{}", serde_json::to_string_pretty(&row).unwrap()).unwrap();
            recorded.push(row.clone());
            assert_eq!(row["trace"], reference[if public {"public_contract"} else {"source_predicted_legacy"}]["trace"], "неожиданная staged последовательность: {name}");
            assert_eq!(row["verdict"], reference[if public {"public_contract"} else {"source_predicted_legacy"}]["verdict"]);
            assert_eq!(public_match,if public {true} else {reference["expected_contract_match"].as_bool().unwrap()}, "contract mismatch {name}");
        }
        host.unload_all();
        assert!(host.is_empty());
    }
    assert_eq!((recorded.len(), public_mismatches, verdict_mismatches), if public {(12,0,0)} else {(12,6,2)});
    let output = serde_json::json!({"baseline_kind":"staged-native-block-break-only","wasm_rust_runtime_executed":true,"cohorts":recorded,"public_contract_mismatches":public_mismatches,"public_verdict_mismatches":verdict_mismatches});
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tmp/{fixture_folder}/staged-capture-{}.json", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    writeln!(file, "{}", serde_json::to_string_pretty(&output).unwrap()).unwrap();
    println!("PRIORITY_CAPTURE {}", path.display());
}

#[test]
#[ignore = "requires actual translated staged fixtures"]
fn translated_bukkit_staged_frames_reject_invalid_order_replay_and_capacity() {
    let root=server_root("staged-frame-negatives");let source=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/plugins/java-priority-a");
    let folder=root.join("plugins/java-priority-a");std::fs::create_dir_all(&folder).unwrap();
    for file in ["plugin.toml","plugin.wasm"] {std::fs::copy(source.join(file),folder.join(file)).unwrap();}
    let shared=shared();java_info(&shared);let mut host=Host::new(shared,root).unwrap();host.load("java-priority-a").unwrap();
    let who=view();
    for operation in [1,2] {assert!(host.priority_probe(operation,1,0,false,&who).unwrap_err().contains("Missing"));}
    assert!(host.priority_probe(0,0,0,false,&who).unwrap_err().contains("Missing"));
    host.priority_probe(0,1,0,false,&who).unwrap();
    assert!(host.priority_probe(0,1,0,false,&who).unwrap_err().contains("Duplicate"));
    assert!(host.priority_probe(1,1,6,false,&who).unwrap_err().contains("Priority"));
    assert!(host.priority_probe(1,1,1,false,&who).unwrap_err().contains("Order"));
    assert!(host.priority_probe(2,1,0,false,&who).unwrap_err().contains("Order"));
    host.priority_probe(1,1,0,false,&who).unwrap();
    assert!(host.priority_probe(1,1,0,false,&who).unwrap_err().contains("Order"));
    for priority in 1..6 {host.priority_probe(1,1,priority,false,&who).unwrap();}
    host.priority_probe(2,1,0,true,&who).unwrap();
    assert!(host.priority_probe(0,1,0,false,&who).unwrap_err().contains("Order"));
    assert!(host.priority_probe(2,1,0,false,&who).unwrap_err().contains("Missing"));
    // До вызова handlers map остаётся bounded.
    for id in 2..34 {host.priority_probe(0,id,0,false,&who).unwrap();}
    assert!(host.priority_probe(0,34,0,false,&who).unwrap_err().contains("Capacity"));
    host.unload_all();assert!(host.is_empty());
    println!("PRIORITY_FRAME_NEGATIVES_PASS missing duplicate zero invalid-priority out-of-order early-finish phase-replay id-replay finish-replay capacity unload");
}

#[test]
#[ignore = "requires actual translated staged safety fixtures"]
fn translated_bukkit_staged_monitor_epoch_nested_trap_and_empty() {
    let spec:serde_json::Value=serde_json::from_str(include_str!("../../tools/plugin-translator/fixtures/priority-safety-expected.json")).unwrap();
    let mut rows=Vec::new();
    for case in spec["cases"].as_array().unwrap() {
        let mode=case["mode"].as_str().unwrap();let root=server_root(&format!("priority-safety-{mode}"));
        let names=if mode=="empty" {vec!["observer","empty"]} else {vec!["observer","mutator"]};
        for suffix in names.iter().copied().chain(std::iter::once("failed")) {
            let name=format!("java-priority-{suffix}");let source=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/plugins").join(&name);let folder=root.join("plugins").join(&name);std::fs::create_dir_all(&folder).unwrap();
            for file in ["plugin.wasm","plugin.toml"] {std::fs::copy(source.join(file),folder.join(file)).unwrap();}
        }
        let shared=shared();java_info(&shared);let mut host=Host::new(Arc::clone(&shared),root).unwrap();
        assert!(host.load("java-priority-failed").is_err());assert!(host.is_empty());
        for name in &names {host.load(&format!("java-priority-{name}")).unwrap();}
        assert!(host.block_break_mode().contains("общим приоритетам"));
        for command in ["observer","mutator"] {
            if command=="mutator"&&mode=="empty" {continue;}
            let sender=wit::Sender{kind:wit::SenderKind::Player,uuid:Some(crate::network::login::format_uuid(&[1;16])),name:"Первый".into()};
            assert!(host.run_command(sender,command,&[mode.into()]).unwrap().0);
        }
        shared.players.lock().unwrap().take_orders(&[1;16]);
        let verdict=if host.block_break(&view(),&wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:70,z:0},"minecraft:stone")==Verdict::Deny {"deny"} else {"allow"};
        assert_eq!(verdict,case["verdict"].as_str().unwrap());assert_eq!(host.loaded.len(),case["survivors"].as_u64().unwrap() as usize);
        let messages:Vec<_>=shared.players.lock().unwrap().take_orders(&[1;16]).into_iter().filter_map(|order|match order {crate::players::Order::Message{component}=>Some(component),_=>None}).collect();
        assert_eq!(messages,[crate::network::nbt::text_component(case["message"].as_str().unwrap())]);
        let raw:Vec<String>=messages.iter().map(|m|m.iter().map(|b|format!("{b:02x}")).collect()).collect();
        let row=serde_json::json!({"mode":mode,"verdict":verdict,"survivors":host.loaded.len(),"raw_message_nbt_hex":raw});println!("PRIORITY_SAFETY {row}");rows.push(row);
        if mode=="trap"||mode=="fuel" {assert!(host.command_level("mutator").is_none());}
        host.unload_all();assert!(host.is_empty());
    }
    let file=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/safety-capture.json");assert!(!file.exists());std::fs::write(file,serde_json::to_string_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore = "requires actual translated staged fixture"]
fn translated_bukkit_staged_empty_fuel_quarantines_and_reload_is_fresh() {
    let root=server_root("staged-budget-quarantine");let source=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/plugins/java-priority-a");let folder=root.join("plugins/java-priority-a");std::fs::create_dir_all(&folder).unwrap();
    for file in ["plugin.toml","plugin.wasm"] {std::fs::copy(source.join(file),folder.join(file)).unwrap();}
    let shared=shared();java_info(&shared);let mut host=Host::new(shared,root).unwrap();host.load("java-priority-a").unwrap();host.priority_test_fuel(1);
    let pos=wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:70,z:0};
    // begin не подтверждён: прежнее false; store с незавершённым frame удаляется.
    assert_eq!(host.block_break(&view(),&pos,"minecraft:stone"),Verdict::Allow);assert!(host.is_empty());assert!(host.command_level("prioritya").is_none());
    host.load("java-priority-a").unwrap();assert_eq!(host.block_break(&view(),&pos,"minecraft:stone"),Verdict::Allow);assert_eq!(host.loaded.len(),1);host.unload_all();
    println!("PRIORITY_BUDGET_QUARANTINE_PASS fresh-reload command-cleanup no-refill");
}

#[test]
#[ignore = "requires actual own translated fault component; bounded by CI timeout"]
fn translated_bukkit_staged_faults_and_cumulative_native_budgets() {
    use std::time::Duration;
    let spec:serde_json::Value=serde_json::from_str(include_str!("../../tools/plugin-translator/fixtures/priority-budget-expected.json")).unwrap();let mut captures=Vec::new();
    for fault in spec["faults"].as_array().unwrap() {
        let fault=fault.as_str().unwrap();let root=server_root(&format!("priority-fault-{fault}"));
        for suffix in ["a","fault","empty"] {
            let name=format!("java-priority-{suffix}");let source=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/plugins").join(&name);let folder=root.join("plugins").join(&name);std::fs::create_dir_all(&folder).unwrap();for file in ["plugin.wasm","plugin.toml"]{std::fs::copy(source.join(file),folder.join(file)).unwrap();}
        }
        let shared=shared();java_info(&shared);let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("java-priority-a").unwrap();host.load("java-priority-fault").unwrap();host.load("java-priority-empty").unwrap();
        let sender=wit::Sender{kind:wit::SenderKind::Player,uuid:Some(crate::network::login::format_uuid(&[1;16])),name:"Первый".into()};assert!(host.run_command(sender,"prioritya",&["cancel".into()]).unwrap().0);shared.players.lock().unwrap().take_orders(&[1;16]);
        if fault=="fuel" {host.priority_test_fuel(50_000_000);}
        if fault=="guest" {host.priority_test_fuel(u64::MAX);host.priority_test_budget(100,Duration::from_millis(1000),Duration::ZERO);}
        if fault=="native" {host.priority_test_budget(40,Duration::from_millis(400),Duration::from_millis(250));}
        let pos=wit::BlockPos{dimension:wit::Dimension::Overworld,x:3,y:70,z:0};
        assert_eq!(host.block_break(&view(),&pos,&format!("test:{fault}")),Verdict::Deny,"fault={fault}");
        assert_eq!(host.loaded.len(),2,"fault={fault}");assert!(host.is_loaded("java-priority-empty"));assert!(host.is_loaded("java-priority-a"));assert!(!host.is_loaded("java-priority-fault"));
        let rows=host.priority_test_trace().to_vec();
        let failed:Vec<_>=rows.iter().filter(|r|r["plugin"]=="java-priority-fault"&&r["ok"]==false).collect();assert_eq!(failed.len(),1,"fault={fault} rows={rows:?}");
        assert_eq!(failed[0]["stage"],if fault=="begin" {"begin"} else if fault=="finish" {"finish"} else {"phase"});
        let successful_phases=rows.iter().filter(|r|r["plugin"]=="java-priority-fault"&&r["stage"]=="phase"&&r["ok"]==true).count();
        if matches!(fault,"fuel"|"guest"|"native") {assert!(successful_phases>=1&&successful_phases<6,"must exhaust BETWEEN phases: {fault} rows={rows:?}");}
        if fault=="native" {assert_eq!(failed[0]["native_epochs"],0);assert_eq!(failed[0]["native_ns"],0);}
        if fault=="fuel" {assert_eq!(failed[0]["fuel"],0);}
        for healthy in ["java-priority-a","java-priority-empty"] {assert!(rows.iter().any(|r|r["plugin"]==healthy&&r["stage"]=="finish"&&r["ok"]==true&&r["cancelled"]==true));}
        let messages:Vec<_>=shared.players.lock().unwrap().take_orders(&[1;16]).into_iter().filter_map(|order|match order {crate::players::Order::Message{component}=>Some(component),_=>None}).collect();assert_eq!(messages,vec![crate::network::nbt::text_component("trace A0 c0>1"),crate::network::nbt::text_component("trace A5 c1")]);
        let row=serde_json::json!({"fault":fault,"boundaries":rows,"survivors":2,"verdict":"deny","raw_message_nbt_hex":messages.iter().map(|m|m.iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>()});println!("PRIORITY_CUMULATIVE_FAULT {row}");captures.push(row);
        host.load("java-priority-fault").unwrap();assert_eq!(host.block_break(&view(),&pos,"minecraft:stone"),Verdict::Deny);assert_eq!(host.loaded.len(),3);host.unload_all();assert!(host.is_empty());
    }
    let file=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/fault-capture.json");assert!(!file.exists());std::fs::write(file,serde_json::to_string_pretty(&captures).unwrap()).unwrap();
}

#[test]
#[ignore = "requires actual validated malformed optional components"]
fn translated_bukkit_staged_partial_and_wrong_type_are_not_legacy() {
    for kind in ["missing","type"] {
        let root=server_root(&format!("priority-invalid-{kind}"));let name=format!("java-priority-invalid-{kind}");let source=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/plugins").join(&name);let folder=root.join("plugins").join(&name);std::fs::create_dir_all(&folder).unwrap();for file in ["plugin.wasm","plugin.toml"]{std::fs::copy(source.join(file),folder.join(file)).unwrap();}
        let shared=shared();java_info(&shared);let mut host=Host::new(shared,root).unwrap();let error=host.load(&name).unwrap_err();
        assert!(host.is_empty());assert!(error.contains("optional block-break")||error.contains("type mismatch")||error.contains("typecheck"),"unexpected rejection {kind}: {error}");
        println!("PRIORITY_INVALID_EXPORT_REJECTED kind={kind} reason={error}");
    }
}
