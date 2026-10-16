// Ответы старых команд в формате клиента 26.1.2.
// minecraft.wiki, Java Edition 1.21.5: «either the block state changed or
// the block entity data changed» — setblock сохраняет прежний подсчёт,
// здесь меняется только текст (tmp/bb/cmdblock-impulse.md:6, chain.md:33).

use crate::commands::Answer;
use crate::commands_admin::{Tag, player_name, translate};
use crate::inventory::Stack;
use crate::players::Order;
use crate::shared::Shared;

fn s(text: impl Into<String>) -> Tag {
    Tag::Str(text.into())
}

fn obj(fields: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(fields.into_iter().map(|(key, value)| (key.to_string(), value)).collect())
}

pub(crate) fn setblock(changed: bool, x: i32, y: i32, z: i32) -> Answer {
    use crate::commands_world::{Arg, fail, tr};

    if changed {
        let mut answer = tr("commands.setblock.success", &[Arg::Int(x.into()), Arg::Int(y.into()), Arg::Int(z.into())]);
        answer.reply = format!("Changed the block at {}, {}, {}", x, y, z);
        answer
    } else {
        let mut answer = fail("commands.setblock.failed", &[]);
        answer.reply = "Could not set the block".to_string();
        answer
    }
}

/// tmp/bb/cmd-attribute.md:30: количество, название с show_item и имя игрока.
/// [проверить] редкость, особые названия и компоненты предмета при наведении
/// требуют отдельных записей; здесь подтверждён обычный незачарованный меч.
pub(crate) fn give(stack: Stack, targets: &[([u8; 16], String)]) -> Answer {
    let name = crate::blocks::item_name(stack.item).unwrap_or("air");
    let kind = if crate::blocks::state_by_name(name).is_some() { "block" } else { "item" };
    let label = obj(vec![
        ("text", s("")),
        ("extra", Tag::List(vec![translate(&format!("{}.minecraft.{}", kind, name), vec![])])),
    ]);
    let mut hover = vec![("action", s("show_item")), ("id", s(format!("minecraft:{}", name)))];
    if stack.count != 1 {
        hover.push(("count", Tag::Int(stack.count)));
    }
    let item = obj(vec![
        ("translate", s("chat.square_brackets")),
        ("with", Tag::List(vec![label])),
        ("color", s("white")),
        ("hover_event", obj(hover)),
    ]);
    let (suffix, who) = match targets {
        [(uuid, name)] => ("single", player_name(name, uuid)),
        _ => ("multiple", Tag::Int(targets.len() as i32)),
    };
    let component = translate(&format!("commands.give.success.{}", suffix), vec![Tag::Int(stack.count), item, who]);
    let mut answer = Answer::saying(format!("Gave {} [{}] to {}", stack.count, name, targets.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>().join(", ")));
    answer.component = Some(component.encode());
    answer
}

pub(crate) fn block_say(shared: &Shared, name: &str, text: &str) {
    let body = crate::network::chat_types::say_body(name, text);
    let plain = format!("[{}] {}", name, text);
    crate::log_info!("{}", plain);
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
    let targets: Vec<_> = players.members().iter().map(|member| member.uuid).collect();
    for uuid in targets {
        players.order(&uuid, Order::PacketText { id: 0x21, body: body.clone(), plain: plain.clone() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Поля из tmp/bb/cmd-attribute.md:30, включая обёртку числа в with.
    #[test]
    fn give_sword_matches_the_recorded_component() {
        use crate::network::nbt::Compound;
        let uuid = [0; 16];
        let stack = Stack::new(crate::blocks::item_named("diamond_sword").unwrap(), 1);
        let answer = give(stack, &[(uuid, "blackbox".to_string())]);
        let item = Compound::new()
            .compounds("with", vec![Compound::new()
                .compounds("extra", vec![Compound::new().string("translate", "item.minecraft.diamond_sword")])
                .string("text", "")])
            .string("color", "white")
            .compound("hover_event", Compound::new().string("action", "show_item").string("id", "minecraft:diamond_sword"))
            .string("translate", "chat.square_brackets");
        let player = Compound::new()
            .compound("click_event", Compound::new().string("action", "suggest_command").string("command", "/tell blackbox "))
            .string("insertion", "blackbox").string("text", "blackbox")
            .compound("hover_event", Compound::new().string("name", "blackbox").string("action", "show_entity")
                .string("id", "minecraft:player").int_array("uuid", &[0; 4]));
        let expected = Compound::new().compounds("with", vec![Compound::new().int("", 1), item, player])
            .string("translate", "commands.give.success.single").encode_network();
        assert_eq!(answer.component, Some(expected));
    }

}
