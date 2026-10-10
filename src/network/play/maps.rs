// Карты Java: заполнение пустой и Map Data (26.1 protocol.json: 0x33).
// [Bedrock: потом] собственный пакет данных карты.
use super::*;
use crate::log_warn;

/// `None` — не пустая карта; `Some(())` — заполнена (клиенту нужен весь инвентарь).
pub(super) fn fill(shared:&Shared,player:&mut PlayerState,hand:i32)->Option<()> {
    if !matches!(hand, 0 | 1) { return None; }
    let slot=if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()};
    let stack=player.inventory.slot(slot).filter(|s|blocks::item_name(s.item)==Some("map"))?;
    if player.game_mode==GAME_MODE_SPECTATOR||player.vit.dead{return None;}
    let mut maps=shared.maps.lock().expect("карты");
    let id=maps.create(player.dimension,player.x.floor() as i32,player.z.floor() as i32,0);
    if let Err(e)=maps.save(){log_warn!("Карта не записана: {}",e);return Some(());}
    drop(maps);
    let data=crate::stackdata::Data{map_id:Some(id),..Default::default()};
    let filled=Stack::new(blocks::item_named("filled_map").unwrap(),1).with_extras(data);
    if player.game_mode!=GAME_MODE_CREATIVE&&stack.count==1 {player.inventory.set(slot,Some(filled));}
    else {
        if player.game_mode!=GAME_MODE_CREATIVE {player.inventory.set(slot,inventory::remainder(stack,stack.count-1));}
        shared.players.lock().expect("игроки").order(&player.uuid,Order::Give{stack:filled});
    }
    // Чёрный ящик, map-fill: звук привязан к игроку (Entity Sound), инвентарь — целиком.
    player.progress.used(stack.item);
    shared.world_in(player.dimension).lock().expect("мир").note_entity_sound(player.entity_id,"minecraft:ui.cartography_table.take_result",1.0,1.0);
    Some(())
}

pub(super) async fn send_ticks(stream:&mut Outgoing<'_>,shared:&Shared,player:&mut PlayerState)->io::Result<()> {
    let tick = *shared.ticks.borrow();
    // Чёрный ящик, map-fill: новая карта уходит сразу (в такт взятия), без
    // столбца 0; он доходит ровно через 15 тактов отдельным кусочком 1×128.
    let fresh:Vec<i32>=player.inventory.held().into_iter().chain(player.inventory.slot(inventory::OFFHAND)).filter_map(|s|s.extras().map_id)
        .filter(|id|!player.map_first.contains_key(id)).collect();
    let column_due=player.map_first.values().any(|first|*first+15==tick);
    if !tick.is_multiple_of(5)&&fresh.is_empty()&&!column_due{return Ok(());}
    for id in &fresh {player.map_first.insert(*id,tick);}
    let mut held:HashSet<i32>=player.inventory.held().into_iter().chain(player.inventory.slot(inventory::OFFHAND)).filter_map(|s|s.extras().map_id).collect();
    let frames=shared.items_in(player.dimension).lock().expect("рамки").frames().iter()
        .filter(|f|(f.pos.0 as f64-player.x).abs()<64.0&&(f.pos.2 as f64-player.z).abs()<64.0)
        .filter_map(|f|f.item.and_then(|s|s.extras().map_id).map(|id|(id,f.pos,f.facing))).collect::<Vec<_>>();
    let carried=held.clone();held.extend(frames.iter().map(|f|f.0));
    player.seen_maps.retain(|id, _| held.contains(id));
    player.seen_map_icons.retain(|id, _| held.contains(id));
    player.map_first.retain(|id, _| held.contains(id));
    let packets={
        let world=shared.world_in(player.dimension).lock().expect("мир");
        let mut maps=shared.maps.lock().expect("карты");
        let mut packets=Vec::new();
        for id in held {
            if carried.contains(&id)&&maps.maps.get(&id).is_some_and(|m|m.dimension==player.dimension.index() as u8) {
                let first=player.map_first.get(&id).copied().unwrap_or(0);
                maps.update(id,&world,(player.x,player.z),usize::MAX,tick<first+15);
            }
            let Some(map)=maps.maps.get(&id) else{continue;};
            let mut body=encode_varint(0x33);body.extend(encode_varint(id));body.push(map.scale);body.push(u8::from(map.locked));
            let mut icons=Vec::new();
            if let Some((x,z,kind))=map.destination {
                let (x,z,d)=crate::maps::marker(map,x as f64,z as f64,180.0);icons.push((kind,x,z,d));
            }
            if carried.contains(&id)&&map.dimension==player.dimension.index() as u8
                && let Some(icon)=crate::maps::player_marker(map,player.x,player.z,player.yaw) {icons.push(icon);}
            for (_,p,facing) in frames.iter().filter(|f|f.0==id) {
                let yaw=match facing {2=>180.0,3=>0.0,4=>90.0,_=>270.0};
                let (x,z,d)=crate::maps::marker(map,p.0 as f64,p.2 as f64,yaw);icons.push((1,x,z,d));
            }
            // Значки шлются, только когда изменились (чёрный ящик, map-fill: при
            // обновлении столбца «icons=undefined»).
            let mut sent_anything=false;
            if player.seen_map_icons.get(&id)==Some(&icons) { body.push(0); }
            else {
                sent_anything=true;
                body.push(1);
                body.extend(encode_varint(icons.len() as i32));
                for (kind,x,z,dir) in &icons {body.extend(encode_varint(*kind));body.extend([*x as u8,*z as u8,*dir,0]);}
                player.seen_map_icons.insert(id,icons);
            }
            let previous = player.seen_maps.get(&id);
            if let Some((x, z, width, height, pixels)) = crate::maps::patch(previous.map(Vec::as_slice), &map.colors) {
                body.extend([width, height, x, z]);
                body.extend(encode_varint(pixels.len() as i32));
                body.extend(pixels);
                player.seen_maps.insert(id, map.colors.clone());
                sent_anything=true;
            } else { body.push(0); }
            if sent_anything { packets.push(body); }
        }
        packets
    };
    for packet in packets {write_body(stream,packet).await?;}
    Ok(())
}
