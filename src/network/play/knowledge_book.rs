use super::*;

/// Wiki, Knowledge Book: открывает recipes и расходуется; обе руки.
/// [проверить] Полная карта vanilla recipe IDs — recipe_book::learn_named.
pub(super) fn use_book(player: &mut PlayerState, hand: i32) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot).filter(|s| blocks::item_name(s.item) == Some("knowledge_book")) else { return false; };
    if player.game_mode == GAME_MODE_SPECTATOR { return true; }
    if player.game_mode != GAME_MODE_CREATIVE {
        player.inventory.set(slot, (stack.count > 1).then(|| stack.with_count(stack.count - 1)));
    }
    if let Some(recipes) = stack.extras().recipes && player.progress.recipes.learn_named(&recipes) { player.progress.used(stack.item); }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn knowledge_book_unlocks_atomically_consumes_correct_hand_and_round_trips() {
        let (_,data) = crate::stackdata::parse_item_spec(r#"knowledge_book[recipes=["minecraft:diamond_sword","minecraft:shield"]]"#).unwrap();
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data.clone()));
        let (_,wire)=crate::stackdata::encode_components(&data);let mut back=crate::stackdata::Data::default();
        crate::stackdata::read_component(66,&wire[1..],&mut back);assert_eq!(back,data);
        for (mode,remaining) in [(GAME_MODE_SURVIVAL,0),(GAME_MODE_CREATIVE,1)] {
            for hand in [0,1] {
                let mut player=PlayerState::new([7;16],"BookAudit".into(),None,None,0x7f,9,(0.0,64.0,0.0));
                player.game_mode=mode;
                let slot=if hand==1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
                player.inventory.set(slot,Some(Stack::new(blocks::item_named("knowledge_book").unwrap(),1).with_extras(data.clone())));
                player.progress.recipes.update();
                assert!(use_book(&mut player,hand));assert_eq!(player.inventory.slot(slot).map_or(0,|s|s.count),remaining);
                assert!(player.progress.recipes.update().is_some());
                assert!(!player.progress.recipes.learn_named(&["diamond_sword".into(),"unknown".into()]));
                assert!(player.progress.recipes.update().is_none());
            }
        }
    }
}
