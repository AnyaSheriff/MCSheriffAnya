//! Декларации аргументов закреплены сетевой записью оригинала, а не строковым
//! разбором обработчиков: клиенту нужны границы чисел, селекторы и redirect.
use super::{encode_string, encode_varint};
use serde::Deserialize;
use std::collections::BTreeSet;


pub(super) const IMPLEMENTED: &[&str] = &[
    "advancement", "attribute", "ban", "ban-ip", "banlist", "bossbar", "clear", "clone", "damage", "data", "datapack",
    "defaultgamemode", "deop", "difficulty", "effect", "enchant", "execute", "experience", "fill", "fillbiome", "forceload",
    "function", "gamemode", "gamerule", "give", "help", "item", "kick", "kill", "list", "locate", "loot", "me", "msg", "op",
    "pardon", "pardon-ip", "particle", "place", "playsound", "random", "recipe", "reload", "return", "ride", "rotate",
    "save-all", "save-off", "save-on", "say", "schedule", "scoreboard", "seed", "setblock", "setidletimeout", "setworldspawn",
    "spawnpoint", "spectate", "spreadplayers", "stop", "stopsound", "summon", "tag", "team", "teammsg", "teleport", "tell",
    "tellraw", "tick", "time", "title", "tm", "tp", "transfer", "trigger", "version", "w", "weather", "whitelist", "worldborder", "xp",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(super) struct Node {
    pub flags: u8,
    pub children: Vec<usize>,
    #[serde(default)]
    pub redirect: Option<usize>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub parser: Option<i32>,
    #[serde(default)]
    pub properties: String,
    #[serde(default)]
    pub suggestion: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
    #[serde(default = "default_protocol")]
    pub protocol: i32,
}

fn default_protocol() -> i32 { 777 }

impl Tree {
    /// Замыкание только опубликованных ветвей; redirect на корень возвращает
    /// уже отфильтрованный корень и не открывает административные команды.
    pub fn retain(&mut self, allowed: impl Fn(&str) -> bool) {
        self.nodes[self.root].children = self.nodes[self.root].children.iter().copied()
            .filter(|&index| allowed(&self.nodes[index].name)).collect();
        let mut seen = BTreeSet::new();
        let mut pending = vec![self.root];
        while let Some(index) = pending.pop() {
            if seen.insert(index) {
                pending.extend(&self.nodes[index].children);
                pending.extend(self.nodes[index].redirect);
            }
        }
        let mut mapping = vec![0; self.nodes.len()];
        for (new, &old) in seen.iter().enumerate() { mapping[old] = new; }
        self.nodes = seen.into_iter().map(|old| {
            let mut node = self.nodes[old].clone();
            node.children.iter_mut().for_each(|index| *index = mapping[*index]);
            node.redirect = node.redirect.map(|index| mapping[index]);
            node
        }).collect();
        self.root = mapping[self.root];
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = encode_varint(super::COMMANDS);
        out.extend(encode_varint(self.nodes.len() as i32));
        for node in &self.nodes {
            out.push(node.flags);
            out.extend(encode_varint(node.children.len() as i32));
            for &child in &node.children { out.extend(encode_varint(child as i32)); }
            if let Some(redirect) = node.redirect { out.extend(encode_varint(redirect as i32)); }
            if node.flags & 3 != 0 { out.extend(encode_string(&node.name)); }
            if let Some(parser) = node.parser {
                out.extend(encode_varint(parser));
                out.extend(bytes(&node.properties));
                if let Some(provider) = &node.suggestion { out.extend(encode_string(provider)); }
            }
        }
        out.extend(encode_varint(self.root as i32));
        out
    }
}

pub(super) fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes().as_chunks::<2>().0.iter().map(|pair| {
        let digit = |b: u8| match b { b'0'..=b'9' => b - b'0', b'a'..=b'f' => b - b'a' + 10, _ => panic!("неверный hex в записи Commands") };
        digit(pair[0]) * 16 + digit(pair[1])
    }).collect()
}



pub(super) fn packet(level: i32, protocol: i32, may_tnt: bool) -> Vec<u8> {
    let mut tree = for_level(level, protocol);

    // Собственные команды не подменяют ни одной ветви оригинала.
    let at = tree.nodes.len();
    tree.nodes[tree.root].children.push(at);
    tree.nodes.push(Node { flags: 1, children: vec![at + 1], redirect: None, name: "changepassword".into(), parser: None, properties: String::new(), suggestion: None });
    tree.nodes.push(Node { flags: 6, children: vec![], redirect: None, name: "старый_и_новый_пароль".into(), parser: Some(5), properties: "02".into(), suggestion: None });
    if may_tnt {
        let at = tree.nodes.len();
        tree.nodes[tree.root].children.push(at);
        for (flags, name, children, parser, properties, suggestion) in [
            (1, "tnt", vec![at + 1, at + 5], None, "", None),
            (1, "give", vec![at + 2], None, "", None),
            (2, "targets", vec![at + 3], Some(6), "02", None),
            (22, "kind", vec![at + 4], Some(5), "00", Some("minecraft:ask_server")),
            (6, "count", vec![], Some(3), "0100000001", None),
            (5, "list", vec![], None, "", None),
        ] {
            tree.nodes.push(Node { flags, name: name.into(), children, redirect: None, parser, properties: properties.into(), suggestion: suggestion.map(str::to_owned) });
        }
    }
    tree.encode()
}

pub(super) fn op4(protocol: i32) -> &'static Tree {
    static NATIVE: std::sync::OnceLock<Tree> = std::sync::OnceLock::new();
    static PREVIEW: std::sync::OnceLock<Tree> = std::sync::OnceLock::new();
    if protocol >= 777 {
        PREVIEW.get_or_init(|| serde_json::from_str(include_str!("command_tree_op4_777.json")).expect("запись Commands op4/777"))
    } else {
        NATIVE.get_or_init(|| serde_json::from_str(include_str!("command_tree_op4_775.json")).expect("запись Commands op4/775"))
    }
}

pub(super) fn for_level(level: i32, protocol: i32) -> Tree {
    if level < 2 {
        let mut tree = nonop().clone();
        tree.protocol = protocol;
        return tree;
    }
    let mut tree = op4(protocol).clone();
    tree.retain(|name| crate::commands::level_needed(name) <= level);
    tree
}

pub(super) fn nonop() -> &'static Tree {
    static TREE: std::sync::OnceLock<Tree> = std::sync::OnceLock::new();
    TREE.get_or_init(|| serde_json::from_str(include_str!("command_tree_nonop.json")).expect("запись Commands non-op"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonop() -> Tree {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/commands-777/official.json")).unwrap();
        serde_json::from_value(fixture["roles"]["nonop"]["tree"].clone()).unwrap()
    }

    #[test]
    fn nonop_recorded_packet_roundtrip() {
        let packet = nonop().encode();
        assert_eq!(&packet[1..], include_bytes!("../../../tools/protocol-bridge/testdata/commands-777/nonop.bin"));
    }

    #[test]
    fn op4_nodes_match_recorded_projection_for_both_clients() {
        for (protocol, bytes) in [
            (777, include_bytes!("../../../tools/protocol-bridge/testdata/commands-777/op4-projected.bin").as_slice()),
            (775, include_bytes!("../../../tools/protocol-bridge/testdata/commands-777/native-775/op4-projected.bin").as_slice()),
        ] {
            assert_eq!(&op4(protocol).encode()[1..], bytes, "протокол {protocol}");
            assert!(op4(protocol).nodes[op4(protocol).root].children.iter().all(|&i| IMPLEMENTED.contains(&op4(protocol).nodes[i].name.as_str())));
        }
    }

    #[test]
    fn permission_levels_do_not_expose_higher_level_roots() {
        let names = |level| {
            let tree = for_level(level, 777);
            tree.nodes[tree.root].children.iter().map(|&i| tree.nodes[i].name.clone()).collect::<Vec<_>>()
        };
        assert_eq!(names(0), names(1));
        assert!(names(2).contains(&"give".into()));
        assert!(!names(2).contains(&"op".into()));
        assert!(names(3).contains(&"op".into()));
        assert!(!names(3).contains(&"stop".into()));
        assert!(names(4).contains(&"stop".into()));
    }

    #[test]
    fn redirects_retain_their_target_and_no_other_roots() {
        let mut tree = nonop();
        tree.retain(|name| name == "tell");
        assert_eq!(tree.nodes[tree.root].children.len(), 1);
        let alias = &tree.nodes[tree.nodes[tree.root].children[0]];
        let target = &tree.nodes[alias.redirect.unwrap()];
        assert_eq!(target.name, "msg");
        assert_eq!(target.children.len(), 1);
        assert!(tree.nodes.iter().all(|node| node.name != "help"));
    }
}
