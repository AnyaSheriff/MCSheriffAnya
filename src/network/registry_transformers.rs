// Реестр26.3: Wiki Block transformer definition oldid3845159,
// Block state provider oldid3828513; формы NBT проверяет живой клиент.
// Правила соответствуют нашим block_use.rs/copper.rs/farming.rs.
// Это описание клиенту, не замена серверных проверок взаимодействия.
use super::{Compound, sound};
use crate::{blocks, copper};

fn matching(names: &[&str]) -> Compound {
    let names: Vec<String> = names.iter().map(|n| format!("minecraft:{n}")).collect();
    Compound::new().string("type", "minecraft:matching_blocks")
        .strings("blocks", &names.iter().map(String::as_str).collect::<Vec<_>>())
}

fn transform(names: &[&str], target: &str, above_air: bool, copy: bool, noise: &str) -> Compound {
    let predicate = if above_air {
        Compound::new().string("type", "minecraft:all_of").compounds("predicates", vec![
            matching(names), matching(&["air"]).ints("offset", &[0, 1, 0]),
        ])
    } else { matching(names) };
    let state = Compound::new().string("id", &format!("minecraft:{target}"));
    let next = if copy {
        Compound::new().string("type", "minecraft:copy_properties").compound("source", state)
    } else { Compound::new().string("type", "minecraft:simple").compound("state", state) };
    Compound::new().compound("block_state_provider", Compound::new()
        .string("type", "minecraft:rule_based")
        .compounds("rules", vec![Compound::new().compound("if_true", predicate).compound("then", next)]))
        .compound("sound", sound(noise)).int("item_damage_per_use", 1)
}

fn rules(name: &str) -> Option<Vec<Compound>> {
    Some(match name {
        "shovel" => vec![transform(
            &["grass_block", "dirt", "coarse_dirt", "rooted_dirt", "podzol", "mycelium"],
            "dirt_path", true, false, "minecraft:item.shovel.flatten",
        ).strings("disallowed_faces", &["down"])],
        "hoe" => vec![
            transform(&["grass_block", "dirt", "dirt_path"], "farmland", true, false, "minecraft:item.hoe.till")
                .strings("disallowed_faces", &["down"]),
            transform(&["coarse_dirt"], "dirt", true, false, "minecraft:item.hoe.till")
                .strings("disallowed_faces", &["down"]),
            transform(&["rooted_dirt"], "dirt", false, false, "minecraft:item.hoe.till")
                .string("loot", "minecraft:till/rooted_dirt").string("drop_strategy", "clicked_face"),
        ],
        "axe" => {
            let mut rules = Vec::new();
            for &(name, state) in crate::blocks_table::DEFAULT_STATES {
                if let Some((next, action)) = copper::interaction(state, "iron_axe") {
                    let target = blocks::block_at_state(next).expect("состояние из нашей таблицы");
                    let (noise, particle) = match action {
                        copper::Use::Unwax => ("minecraft:item.axe.wax_off", "wax_off"),
                        copper::Use::Scrape => ("minecraft:item.axe.scrape", "scrape"),
                        copper::Use::Wax => unreachable!("топор не добавляет воск"),
                    };
                    let mut rule = transform(&[name], target, false, true, noise).string("particle", particle);
                    if name.ends_with("copper_door") || name.ends_with("copper_chest") {
                        rule = rule.boolean("update_from_neighbors", false);
                    }
                    if name.ends_with("copper_chest") { rule = rule.string("transform_type", "copper_chest"); }
                    rules.push(rule);
                } else {
                    let target = format!("stripped_{name}");
                    if blocks::state_by_name(&target).is_some() {
                        rules.push(transform(&[name], &target, false, true, "minecraft:item.axe.strip"));
                    }
                }
            }
            rules
        }
        _ => return None,
    })
}

/// В отличие от остальных наших реестров, здесь корень NBT — список.
pub fn block_transformer(name: &str) -> Option<Vec<u8>> {
    let rules = rules(name)?;
    let mut bytes = vec![9, 10]; // TAG_List с элементами TAG_Compound.
    bytes.extend((rules.len() as i32).to_be_bytes());
    for rule in rules { bytes.extend(&rule.encode_network()[1..]); }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tool_rules_are_nonempty_and_keep_properties_and_guards() {
        let axe = block_transformer("axe").unwrap();
        let hoe = block_transformer("hoe").unwrap();
        let shovel = block_transformer("shovel").unwrap();
        assert!(i32::from_be_bytes(axe[2..6].try_into().unwrap()) > 100);
        assert_eq!(i32::from_be_bytes(hoe[2..6].try_into().unwrap()), 3);
        assert_eq!(i32::from_be_bytes(shovel[2..6].try_into().unwrap()), 1);
        for field in ["minecraft:copy_properties", "minecraft:stripped_oak_log", "wax_off", "scrape", "minecraft:copper_chest"] {
            assert!(axe.windows(field.len()).any(|w| w == field.as_bytes()), "{field}");
        }
        for data in [&hoe, &shovel] {
            for field in ["minecraft:all_of", "disallowed_faces", "offset", "item_damage_per_use"] {
                assert!(data.windows(field.len()).any(|w| w == field.as_bytes()), "{field}");
            }
        }
        assert!(block_transformer("pickaxe").is_none());
    }
}
