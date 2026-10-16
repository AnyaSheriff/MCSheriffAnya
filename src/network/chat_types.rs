// minecraft.wiki, Java Edition 1.19: «synchronized to clients when they join».
// Поля и порядок — minecraft-data pc/26.1/loginPacket.json, protocol 775.
// Inline ChatTypesHolder допустим по protocol.json, но без chat_type робот
// падает при разборе чата; объявляем реестр и посылаем ссылку на say_command.

use super::nbt::{Compound, text_component};
use super::varint::encode_varint;

pub const NAMES: &[&str] = &[
    "chat", "emote_command", "msg_command_incoming", "msg_command_outgoing",
    "say_command", "team_msg_command_incoming", "team_msg_command_outgoing",
];
const SAY_ID: usize = 4;

pub fn entry(name: &str) -> Option<Compound> {
    let (key, parameters): (&str, &[&str]) = match name {
        "chat" => ("chat.type.text", &["sender", "content"]),
        "emote_command" => ("chat.type.emote", &["sender", "content"]),
        "msg_command_incoming" => ("commands.message.display.incoming", &["sender", "content"]),
        "msg_command_outgoing" => ("commands.message.display.outgoing", &["target", "content"]),
        "say_command" => ("chat.type.announcement", &["sender", "content"]),
        "team_msg_command_incoming" => ("chat.type.team.text", &["target", "sender", "content"]),
        "team_msg_command_outgoing" => ("chat.type.team.sent", &["target", "sender", "content"]),
        _ => return None,
    };
    let style = if matches!(name, "msg_command_incoming" | "msg_command_outgoing") {
        Compound::new().string("color", "gray").boolean("italic", true)
    } else {
        Compound::new()
    };
    let narration = if name == "emote_command" { key } else { "chat.type.text.narrate" };
    Some(Compound::new()
        .compound("chat", Compound::new().string("translation_key", key).strings("parameters", parameters).compound("style", style))
        .compound("narration", Compound::new().string("translation_key", narration)
            .strings("parameters", &["sender", "content"]).compound("style", Compound::new())))
}

pub fn say_body(name: &str, text: &str) -> Vec<u8> {
    let mut body = text_component(text);
    // Holder: 0 означает inline, номер записи передаётся как id + 1.
    body.extend_from_slice(&encode_varint(SAY_ID as i32 + 1));
    body.extend_from_slice(&text_component(name));
    body.push(0); // Optional target отсутствует; перед name признака нет.
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    // Независимый разбор байтов: не сверяем пакет с тем же кодировщиком.
    struct Reader<'a> { bytes: &'a [u8], at: usize }
    impl<'a> Reader<'a> {
        fn new(bytes: &'a [u8]) -> Self { Self { bytes, at: 0 } }
        fn take(&mut self, count: usize) -> &'a [u8] {
            let value = &self.bytes[self.at..self.at + count];
            self.at += count;
            value
        }
        fn byte(&mut self) -> u8 { self.take(1)[0] }
        fn varint(&mut self) -> i32 {
            let (value, used) = super::super::packet::decode_varint(&self.bytes[self.at..]).unwrap();
            self.at += used;
            value
        }
        fn string(&mut self) -> String {
            super::super::packet::decode_string(self.bytes, &mut self.at).unwrap()
        }
        fn nbt_string(&mut self) -> String {
            let len = u16::from_be_bytes(self.take(2).try_into().unwrap()) as usize;
            String::from_utf8(self.take(len).to_vec()).unwrap()
        }
        fn nbt(&mut self) -> Value {
            let kind = self.byte();
            self.payload(kind)
        }
        fn payload(&mut self, kind: u8) -> Value {
            match kind {
                1 => json!(self.byte()),
                8 => json!(self.nbt_string()),
                9 => {
                    let kind = self.byte();
                    let count = i32::from_be_bytes(self.take(4).try_into().unwrap());
                    assert!(count >= 0);
                    Value::Array((0..count).map(|_| self.payload(kind)).collect())
                }
                10 => {
                    let mut fields = serde_json::Map::new();
                    loop {
                        let kind = self.byte();
                        if kind == 0 { break; }
                        let name = self.nbt_string();
                        assert!(fields.insert(name, self.payload(kind)).is_none());
                    }
                    Value::Object(fields)
                }
                _ => panic!("неожиданный NBT {kind}"),
            }
        }
        fn end(&self) { assert_eq!(self.at, self.bytes.len()); }
    }

    #[test]
    fn configuration_and_say_decode_field_by_field() {
        let (registry, names) = super::super::configuration::REQUIRED_REGISTRIES.iter()
            .find(|(name, _)| *name == "minecraft:chat_type").unwrap();
        let bytes = super::super::configuration::registry_body(registry, names);
        let mut r = Reader::new(&bytes);
        assert_eq!(r.varint(), 0x07);
        assert_eq!(r.string(), "minecraft:chat_type");
        assert_eq!(r.varint(), 7);
        let mut entries = Vec::new();
        for name in NAMES {
            assert_eq!(r.string(), format!("minecraft:{name}"));
            assert_eq!(r.byte(), 1, "данные обязательны без Known Packs");
            let value = r.nbt();
            for decoration in ["chat", "narration"] {
                assert!(value[decoration]["translation_key"].is_string());
                assert!(value[decoration]["parameters"].is_array());
                assert!(value[decoration]["style"].is_object());
            }
            entries.push(value);
        }
        r.end();
        assert_eq!(NAMES[SAY_ID], "say_command");
        assert_eq!(entries[SAY_ID], json!({
            "chat": {"translation_key":"chat.type.announcement", "parameters":["sender","content"], "style":{}},
            "narration": {"translation_key":"chat.type.text.narrate", "parameters":["sender","content"], "style":{}}
        }));
        for (name, message) in [("@", "tick"), ("Блок", "цепочка"), ("", "")] {
            let bytes = say_body(name, message);
            let mut r = Reader::new(&bytes);
            assert_eq!(r.nbt(), json!(message));
            let holder = r.varint();
            assert_eq!(holder, 5);
            assert_eq!(entries[(holder - 1) as usize]["chat"]["translation_key"], "chat.type.announcement");
            assert_eq!(r.nbt(), json!(name));
            assert_eq!(r.byte(), 0);
            r.end();
        }
    }
}
