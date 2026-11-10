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
