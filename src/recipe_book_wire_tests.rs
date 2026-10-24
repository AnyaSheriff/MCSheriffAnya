//! Независимый читатель проверяет готовые сетевые пакеты, а не обратный translator.
use super::*;
use mcsa_protocol::{wire, JavaVersion, State, Direction};
use std::io::Write;
use std::process::{Command, Stdio};

fn wire_packet(bytes: &[u8], catalog: crate::gameplay_catalog::Catalog) -> Vec<u8> {
    wire::translate_with_context(State::Play, Direction::Clientbound,
        JavaVersion::V26_1_2, JavaVersion::V26_3, bytes,
        wire::Context { items: mcsa_protocol::items::Context {
            content: catalog.content_codec(), ..Default::default()
        }, ..Default::default() }).unwrap().into_owned()
}

fn check_packets(cases: &[serde_json::Value]) {
    let mut child = Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tools/protocol-bridge/recipe_book_decoder.py"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

fn case(bytes: &[u8], count: usize) -> serde_json::Value {
    assert_eq!(bytes[0], 0x4b);
    let hex: String = bytes[1..].iter().map(|byte| format!("{byte:02x}")).collect();
    serde_json::json!({"payload": hex, "entries": count, "idset_ingredients": true})
}

#[test]
fn every_777_recipe_decodes_in_initial_singleton_and_batch_packets() {
    let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
    let all = entries_for_catalog(catalog);
    assert_eq!(all.len(), 1677);
    let mut cases = Vec::new();
    for (id, entry) in all.iter().enumerate() {
        for announce in [false, true] {
            let mut body = vec![ADD as u8, 1];
            body.extend(entry_body_with_ingredients(id, entry, announce, true));
            body.push(u8::from(!announce));
            cases.push(case(&wire_packet(&body, catalog), 1));
        }
    }
    let mut book = Book::for_catalog(catalog);
    book.give_all();
    for bytes in book.update() { cases.push(case(&wire_packet(&bytes, catalog), all.len())); }
    let keys: Vec<_> = all.iter().map(|entry| entry.key.clone()).collect();
    book.take(&[]).unwrap();
    book.known.extend(keys);
    book.batch = true;
    for bytes in book.update() { cases.push(case(&wire_packet(&bytes, catalog), all.len())); }
    check_packets(&cases);
}

#[test]
fn observed_official_777_all_recipes_decode() {
    let fixture = include_bytes!("../tools/protocol-bridge/testdata/recipe-book-777/official-all.bin");
    let mut bytes = vec![0x4b]; bytes.extend(fixture);
    let mut official = case(&bytes, 1720); official["idset_ingredients"] = false.into();
    check_packets(&[official]);
}

#[test]
fn native_2612_recipe_book_bytes_keep_the_previous_builder() {
    let mut book = Book::default(); book.give_all();
    let bytes = book.update().pop().unwrap();
    let mut previous = encode_varint(ADD);
    previous.extend(encode_varint(entries().len() as i32));
    for (id, entry) in entries().iter().enumerate() { previous.extend(entry_body(id, entry, false)); }
    previous.push(1);
    assert_eq!(bytes, previous);
    use sha2::{Digest, Sha256};
    // Отпечаток прежнего builder на08d0a1b: все записи775 и флаг replace.
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)),
        "f0880db0d158a459d428bb7ae0831750a37dc993259eb47f70e038fd8586c7f2");
}
