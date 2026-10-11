// Wiki, Debug Stick: attack выбирает свойство, use меняет значение;
// sneak меняет направление. Только Creative + op 2. [проверить] Wire parity.
use super::*;

fn cycle(state: i32, data: &mut crate::stackdata::Data, select: bool, reverse: bool) -> Option<(i32, String)> {
    let block = blocks::orientation_of(state)?;
    let key = format!("minecraft:{}", block.name);
    let selected = data.debug_properties.as_ref().and_then(|v| v.iter().find(|(k,_)| *k == key)).map(|(_,v)| v.as_str());
    let index = selected.and_then(|v| block.properties.iter().position(|p| p.name == v));
    let next = |current: usize, size: usize| if reverse { (current + size - 1) % size } else { (current + 1) % size };
    if block.properties.is_empty() { return None; }
    let index = if select { index.map(|i| next(i, block.properties.len())).unwrap_or(if reverse { block.properties.len()-1 } else { 0 }) } else { index.unwrap_or(0) };
    let property = &block.properties[index];
    // Выбор запоминает только удар; щелчок без выбора меняет первое свойство
    // и в предмет ничего не пишет (чёрный ящик, tails-debug-stick: на +0
    // изменение без обновления слота, запись «facing» — после удара).
    if select {
        let selected = data.debug_properties.get_or_insert_with(Vec::new);
        selected.retain(|(k,_)| *k != key); selected.push((key, property.name.to_string())); selected.sort();
    }
    let value = block.value(state, property.name)?;
    let changed = if select { state } else {
        let index = property.values.iter().position(|v| *v == value)?;
        block.with(state, property.name, property.values[next(index, property.values.len())])?
    };
    Some((changed, format!("{}: {}", property.name, block.value(changed, property.name)?)))
}

pub(super) fn interact(shared: &Shared, player: &mut PlayerState, at: (i32,i32,i32), face: i32, select: bool, sequence: i32) -> Option<Reaction> {
    let stack = player.inventory.held().filter(|s| blocks::item_name(s.item) == Some("debug_stick"))?;
    if !crate::command_block::may_edit(shared, &player.uuid, player.game_mode) {
        return (!select).then(|| Reaction::accepted(sequence));
    }
    let mut world = shared.world_in(player.dimension).lock().expect("мир");
    let mut data = stack.extras();
    let mut answer = Reaction::accepted(sequence);
    if let Some((state, text)) = cycle(world.get_block(at.0,at.1,at.2), &mut data, select, player.sneaking) {
        if !select { world.set_debug_state(at, state); }
        if select {
            let slot = player.inventory.selected_slot();
            player.inventory.set(slot, Some(stack.with_extras(data))); answer.slots.push(slot);
        } else {
            // Щелчок: блок, клетка за гранью и блок ещё раз (чёрный ящик,
            // tails-debug-stick: «блок, воздух над ним, блок»).
            let (bx, by, bz) = offset_towards(at.0, at.1, at.2, face);
            world.tell_directly(at.0, at.1, at.2);
            world.tell_directly(bx, by, bz);
        }
        let mut body = encode_varint(SYSTEM_CHAT); push_text_component(&mut body, &text); body.push(1); answer.packets.push(body);
    }
    if select { answer.confirm.push(at); }
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debug_stick_cycles_and_reverses_properties_without_losing_selection() {
        let state = blocks::state_from_text("oak_stairs[facing=north,half=bottom]").unwrap();
        let mut data = crate::stackdata::Data::default();
        let (same, _) = cycle(state, &mut data, true, false).unwrap(); assert_eq!(same,state);
        let (next, _) = cycle(state, &mut data, false, false).unwrap(); assert_ne!(next,state);
        assert_eq!(cycle(next, &mut data, false, true).unwrap().0,state);
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data.clone()));
        let (_,bytes)=crate::stackdata::encode_components(&data);let mut decoded=crate::stackdata::Data::default();
        crate::stackdata::read_component(57,&bytes[1..],&mut decoded);assert_eq!(decoded,data);
        let before=data.clone();cycle(state,&mut data,true,false).unwrap();assert_ne!(data,before);
        cycle(state,&mut data,true,true).unwrap();assert_eq!(data,before);
        assert!(cycle(blocks::state_by_name("stone").unwrap(),&mut data,false,false).is_none());
    }
}

#[cfg(test)]
mod authorization_tests {
    use super::*;
    #[test]
    fn debug_stick_requires_creative_operator_and_keeps_block_type() {
        let shared=Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),crate::ops::Ops::open(std::path::Path::new("tmp/itemaudit4/no-ops-debug.json")),crate::skins::Settings::default());
        let mut p=PlayerState::new([11;16],"DebugAudit".into(),None,None,0x7f,11,(0.0,64.0,0.0));
        p.inventory.set(p.inventory.selected_slot(),Some(Stack::new(blocks::item_named("debug_stick").unwrap(),1)));
        let at=(0,64,0);let state=blocks::state_from_text("oak_log[axis=y]").unwrap();
        shared.world_in(p.dimension).lock().unwrap().set_block(at.0,at.1,at.2,state);
        shared.ops.lock().unwrap().take_away(Some(&p.uuid), &p.name);
        p.game_mode=GAME_MODE_CREATIVE;
        interact(&shared,&mut p,at,1,false,0);
        assert_eq!(shared.world_in(p.dimension).lock().unwrap().get_block(at.0,at.1,at.2),state);
        shared.ops.lock().unwrap().give(&p.uuid,&p.name,2);
        p.game_mode=GAME_MODE_SURVIVAL;interact(&shared,&mut p,at,1,false,0);
        assert_eq!(shared.world_in(p.dimension).lock().unwrap().get_block(at.0,at.1,at.2),state);
        p.game_mode=GAME_MODE_CREATIVE;interact(&shared,&mut p,at,1,false,0);
        let changed=shared.world_in(p.dimension).lock().unwrap().get_block(at.0,at.1,at.2);
        assert_ne!(changed,state);assert_eq!(blocks::block_at_state(changed),Some("oak_log"));
        shared.ops.lock().unwrap().take_away(Some(&p.uuid), &p.name);
    }
}
