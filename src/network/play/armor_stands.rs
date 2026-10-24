// Wiki, Armor Stand: взаимодействие с частью стойки; minecraft-data 26.1,
// Use Entity: hand, location (lpVec3), sneaking. [проверить] границы слотов.
use super::*;

pub(super) fn location(cursor:&mut Cursor<'_>)->Option<[f64;3]> {
    let first=cursor.u8()?;
    if first==0 {return Some([0.0;3]);}
    let packed=u64::from(first)|(u64::from(cursor.u8()?)<<8)|(u64::from(cursor.u32()?)<<16);
    let mut scale=(packed&3) as u32;
    if packed&4!=0 {let high=cursor.varint()?;if !(0..=(i32::MAX>>2)).contains(&high){return None;}scale|=(high as u32)<<2;}
    Some(std::array::from_fn(|i|((((packed>>(3+15*i))&32767) as f64/32766.0)*2.0-1.0)*f64::from(scale)))
}

pub(super) fn packet(payload:&[u8],player:&mut PlayerState,shared:&Shared)->Option<Reaction> {
    let mut cursor=Cursor::new(payload);
    let (target,hand)=(cursor.varint()?,cursor.varint()?);
    if !(0..=1).contains(&hand){return None;}
    let mut items=shared.items_in(player.dimension).lock().expect("предметы");
    let mob=items.mobs_mut().iter_mut().find(|m|m.entity_id==target&&m.kind==crate::mob::Kind::ArmorStand)?;
    let mut reaction=Reaction::default();
    if player.game_mode==GAME_MODE_SPECTATOR || player.game_mode==GAME_MODE_ADVENTURE || player.vit.dead || mob.is_dead()
        || (mob.x-player.x).powi(2)+(mob.y-player.y).powi(2)+(mob.z-player.z).powi(2)>36.0 {return Some(reaction);}
    let Some(at)=location(&mut cursor) else {return Some(reaction);};
    if cursor.u8().is_none(){return Some(reaction);}
    let slot=if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()};
    let mut held=player.inventory.slot(slot);
    if crate::mob::armor_stand::equip_at(mob,&mut held,player.game_mode==GAME_MODE_CREATIVE,Some(at[1]),hand as usize) {
        player.inventory.set(slot,held);reaction.slots.push(slot);
        // Взмах руки уходит и самому игроку (чёрный ящик armorstand-equip-break).
        player.swing_pending=true;
    }
    Some(reaction)
}

pub(super) fn plugin_target(payload: &[u8], player: &PlayerState, shared: &Shared) -> Option<(String,u32,Option<u32>)> {
    let mut cursor=Cursor::new(payload);let (target,hand)=(cursor.varint()?,cursor.varint()?);if !(0..=1).contains(&hand) {return None;}
    let items=shared.items_in(player.dimension).lock().expect("сущности");
    let mob=items.mobs().iter().find(|m|m.entity_id==target&&!m.is_dead())?;
    if (mob.x-player.x).powi(2)+(mob.y-player.y).powi(2)+(mob.z-player.z).powi(2)>36.0 {return None;}
    let uuid=crate::network::login::format_uuid(&mob.uuid);let mut changed=None;
    if mob.kind==crate::mob::Kind::ArmorStand {
        let at=location(&mut cursor)?;cursor.u8()?;
        let mut probe=mob.clone();let mut held=player.inventory.slot(if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()});
        if crate::mob::armor_stand::equip_at(&mut probe,&mut held,player.game_mode==GAME_MODE_CREATIVE,Some(at[1]),hand as usize) {
            changed=(0..6).find(|i|probe.stand.equipment[*i]!=mob.stand.equipment[*i]).map(|i|i as u32);
        }
    }
    Some((uuid,hand as u32,changed))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn hit_location_round_trips_the_protocol_vector() {
        for at in [[0.0,0.0,0.0],[-0.25,0.3,0.25],[0.0,1.8,0.0],[7.0,-4.0,2.0]] {
            let mut bytes=Vec::new();types::push_velocity(&mut bytes,at[0],at[1],at[2]);
            let decoded=location(&mut Cursor::new(&bytes)).unwrap();
            for i in 0..3 {assert!((at[i]-decoded[i]).abs()<0.001);}
        }
    }
}
