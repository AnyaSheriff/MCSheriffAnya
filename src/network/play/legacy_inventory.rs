//! Старые подтверждения окружают общий обработчик; вещи считает только ядро.
use super::*;
use mcsa_protocol::{items, legacy_inventory::{Action, Confirmation}, legacy_items, JavaVersion};

fn expected_stack(stack: Option<Stack>) -> Result<Option<items::Stack>, mcsa_protocol::wire::Error> {
    let mut bytes = Vec::new(); push_slot(&mut bytes, stack);
    let context = legacy_items::Context { enchantments: super::super::packet::legacy_enchantments() };
    let (stack, _) = items::decode(JavaVersion::V26_1_2, &bytes, false, items::Context {
        shared_enchantment_count: Some(context.enchantments.len() as u32), ..Default::default()
    }).map_err(mcsa_protocol::wire::Error::Item)?;
    // Сравниваем именно представление клиента (в том числе старый Damage).
    let bytes = legacy_items::encode(stack.as_ref(), context).map_err(mcsa_protocol::wire::Error::LegacyItem)?;
    Ok(legacy_items::decode(&bytes, context).map_err(mcsa_protocol::wire::Error::LegacyItem)?.0)
}

pub(super) fn handle(action: &Action, packet: &PacketInfo, player: &mut PlayerState, shared: &Shared) -> Reaction {
    match action {
        Action::Acknowledge(ack) => {
            // Наблюдение official-inventory: даже accepted=false разблокирует,
            // но чужие window/action IDs — нет. Обрабатываем в игровой очереди,
            // иначе reader мог бы принять ACK до выполнения предыдущего щелчка.
            if player.legacy_transaction == Some((ack.window, ack.transaction)) {
                player.legacy_transaction = None;
            }
            Reaction::default()
        }
        Action::Click(click) => {
            if player.legacy_transaction.is_some() || player.window.is_some() || player.vit.dead {
                return Reaction::default();
            }
            crafting::refresh(shared, player);
            let before = expected_stack(if click.mode == 0 { player.inventory.slot(i32::from(click.slot)) } else { None });
            let mut reaction = handle_client_packet_inner(packet, player, shared);
            // Для mode0 официальный340 возвращает стопку ДО действия; даже
            // ложное предсказание не отменяет авторитетное действие сервера.
            // У остальных наблюдённых режимов возвращается пустая стопка.
            let accepted = before.is_ok_and(|before| mcsa_protocol::legacy_inventory::same_stack(&before, &click.expected));
            reaction.legacy_confirmation = Some(Confirmation { window: click.window, transaction: click.transaction, accepted });
            if !accepted {
                player.legacy_transaction = Some((click.window, click.transaction));
                reaction.inventory = true;
            }
            player.idle = Instant::now();
            reaction
        }
    }
}
