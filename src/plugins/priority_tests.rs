// Actual translated WASM; старые 12 baseline не меняются.
#[test]
#[ignore = "requires actual translated staged fixtures"]
fn translated_bukkit_staged_priority_contract() {
    use std::io::Write as _;
    let spec: serde_json::Value = serde_json::from_str(include_str!("../../tools/plugin-translator/fixtures/priority-expected.json")).unwrap();
    let cohorts = spec["cohorts"].as_array().unwrap();
    assert_eq!(cohorts.len(), 12);
    let mut labels = std::collections::BTreeSet::new();
    for cohort in cohorts {
        for reference in ["public_contract", "public_contract"] {
            for label in cohort[reference]["trace"].as_array().unwrap() {
                labels.insert(label.as_str().unwrap().to_string());
            }
        }
    }
    let capture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/capture");
    std::fs::create_dir_all(&capture_dir).unwrap();
    let mut recorded = Vec::new();
    let mut public_mismatches = 0;
    let mut verdict_mismatches = 0;
    for order in [&["a", "b"][..], &["b", "a"][..], &["a"][..], &["b"][..]] {
        let root = server_root(&format!("staged-priority-{}", order.join("-")));
        for letter in order {
            let name = format!("java-priority-{letter}");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-priority-staged/plugins").join(&name);
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
            assert_eq!(row["trace"], reference["public_contract"]["trace"], "неожиданная staged последовательность: {name}");
            assert_eq!(row["verdict"], reference["public_contract"]["verdict"]);
            assert!(public_match, "staged contract mismatch {name}");
        }
        host.unload_all();
        assert!(host.is_empty());
    }
    assert_eq!((recorded.len(), public_mismatches, verdict_mismatches), (12, 0, 0));
    let output = serde_json::json!({"baseline_kind":"staged-native-block-break-only","wasm_rust_runtime_executed":true,"cohorts":recorded,"public_contract_mismatches":public_mismatches,"public_verdict_mismatches":verdict_mismatches});
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tmp/java-priority-staged/staged-capture-{}.json", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    writeln!(file, "{}", serde_json::to_string_pretty(&output).unwrap()).unwrap();
    println!("PRIORITY_CAPTURE {}", path.display());
}
