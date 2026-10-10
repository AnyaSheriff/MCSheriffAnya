// Удочка и слежение за поплавком Java. [Bedrock: потом] поплавок и поклёвка.
// Номера и поля пакетов — minecraft-data 26.1 protocol.json/entities.json.
use super::*;

#[derive(Clone,Copy)]
pub(super) struct Shown { grid: (i64,i64,i64), hooked: Option<i32>, biting: bool, velocity: (f64,f64,f64), rot: (i8,i8) }
fn grid(at:(f64,f64,f64))->(i64,i64,i64) {((at.0*4096.0).floor() as i64,(at.1*4096.0).floor() as i64,(at.2*4096.0).floor() as i64)}

/// `None` — в руке не удочка; `Some(true)` — удочка износилась, клиенту
/// нужен весь инвентарь (чёрный ящик, fishing-catch: пакет «Container Content»).
pub(super) fn use_rod(shared:&Shared,player:&mut PlayerState,hand:i32)->Option<bool> {
    if !matches!(hand, 0 | 1) { return None; }
    let slot=if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()};
    let stack=player.inventory.slot(slot).filter(|s|blocks::item_name(s.item)==Some("fishing_rod"))?;
    if player.game_mode==GAME_MODE_SPECTATOR||player.vit.dead{return None;}
    let mut resync=false;
    let old={let mut list=shared.fishing.lock().expect("поплавки");list.iter().position(|b|b.owner==player.entity_id).map(|i|list.remove(i))};
    let mut world=shared.world_in(player.dimension).lock().expect("мир");
    let owner=(player.x,player.y,player.z);
    player.progress.used(stack.item);
    let pitch=0.4/(world.random_unit()*0.4+0.8);
    if let Some(b)=old {
        let mut wear=0;
        world.note_sound_neutral_at(owner,"minecraft:entity.fishing_bobber.retrieve",1.0,pitch);
        if b.dimension==player.dimension {
            let mut items=shared.items_in(player.dimension).lock().expect("предметы");
            if let Some(id)=b.hooked {
                let v=crate::fishing::pull(b.at,owner,false);
                if let Some(m)=items.mobs_mut().iter_mut().find(|m|m.entity_id==id) {m.vx+=v.0;m.vy+=v.1;m.vz+=v.2;wear=2;}
                else if let Some(i)=items.items_mut().iter_mut().find(|i|i.entity_id==id) {i.vx+=v.0;i.vy+=v.1;i.vz+=v.2;i.impulse=true;wear=2;}
                else {
                    let mut players=shared.players.lock().expect("игроки");
                    if let Some(uuid)=players.members().iter().find(|m|m.entity_id==id).map(|m|m.uuid) {
                        let mut body=encode_varint(0x65);body.extend(encode_varint(id));types::push_velocity(&mut body,v.0,v.1,v.2);
                        players.order(&uuid,Order::Packet(body));wear=2;
                    }
                }
            } else if b.bite>0 {
                let jungle=world.surface_biome(b.at.0.floor() as i32,b.at.2.floor() as i32).contains("jungle");
                let catch=crate::fishing::loot(&mut world,b.luck,b.open,jungle);
                // Чёрный ящик, fishing-catch: предмет появляется на месте
                // поплавка, тяга считается от него; одним шариком опыта (вся
                // сумма) — на 0,5 выше и 0,5 вперёд (по z) от игрока, с толчком
                // как у моба.
                let item_at=b.at;
                items.drop_item(shared.next_entity_id(),catch,item_at,crate::fishing::pull(item_at,owner,true),10);
                let points=1+(world.random_bits()%6) as i32;
                let push=((f64::from(world.random_unit())*0.2-0.1)*2.0,f64::from(world.random_unit())*0.4,(f64::from(world.random_unit())*0.2-0.1)*2.0);
                items.add_orb(shared.next_entity_id(),points,(player.x,player.y+0.5,player.z+0.5),push);
                if let Some(name @ ("cod"|"salmon"|"tropical_fish"|"pufferfish"))=blocks::item_name(catch.item) {
                    player.progress.custom("fish_caught",1);player.progress.adv.grant("husbandry/fishy_business",name);
                }
                wear=1;
            }
            if b.grounded {wear=5;}
        }
        world.note_thrown_tracker(false);
        drop(world);
        if player.game_mode!=GAME_MODE_CREATIVE && wear>0 {
            pvp::wear_slot(shared,player,slot,stack,wear,false);
            player.net.slots_dirty.retain(|s|*s!=slot);
            resync=true;
        }
    } else {
        // Чёрный ящик, fishing-cast: появляется на 0,3 вперёд по курсу (не по
        // взгляду) на уровне глаз; скорость — вектор (−sin ψ, −tg θ, cos ψ) длины
        // L, умноженный на 0,6/L + 0,5 (модуль 1,1517 при θ = 25° против 1,1508
        // в записи), с небольшим гауссовым шумом по y и z.
        let (sin_y,cos_y)=(f64::from(player.yaw).to_radians().sin(),f64::from(player.yaw).to_radians().cos());
        let eye=player.y+if player.sneaking {1.27} else {1.62};
        let at=(player.x-sin_y*0.3,eye,player.z+cos_y*0.3);
        let raw=(-sin_y,(-f64::from(player.pitch).to_radians().tan()).clamp(-5.0,5.0),cos_y);
        let length=(raw.0*raw.0+raw.1*raw.1+raw.2*raw.2).sqrt();
        let scale=0.6/length+0.5;
        let mut gauss=|| (0..12).map(|_|f64::from(world.random_unit())).sum::<f64>()-6.0;
        let velocity=(raw.0*scale,raw.1*scale*(1.0+gauss()*0.0045),raw.2*scale*(1.0+gauss()*0.0045));
        let data=stack.extras();
        let luck = f64::from(data.level("luck_of_the_sea")) + player.attrs.value(crate::attributes::LUCK);
        let bobber=crate::fishing::Bobber::new(shared.next_entity_id(),player.entity_id,player.dimension,at,velocity,data.level("lure"),luck);
        world.note_sound_neutral_at(owner,"minecraft:entity.fishing_bobber.throw",0.5,pitch);
        shared.fishing.lock().expect("поплавки").push(bobber);
    }
    Some(resync)
}

pub(super) async fn send_ticks(stream:&mut Outgoing<'_>,shared:&Shared,player:&mut PlayerState)->io::Result<()> {
    let holds=player.inventory.held().into_iter().chain(player.inventory.slot(inventory::OFFHAND)).any(|s|blocks::item_name(s.item)==Some("fishing_rod"));
    let list={
        let mut list=shared.fishing.lock().expect("поплавки");
        list.retain(|b|b.owner!=player.entity_id || (holds && !player.vit.dead && b.dimension==player.dimension));
        list.iter().filter(|b|b.dimension==player.dimension && (b.at.0-player.x).abs()<128.0 && (b.at.2-player.z).abs()<128.0).cloned().collect::<Vec<_>>()
    };
    let gone:Vec<_>=player.seen_fishing.keys().filter(|id|!list.iter().any(|b|b.id==**id)).copied().collect();
    for id in gone {send_remove_entity(stream,id).await?;player.seen_fishing.remove(&id);}
    for b in list {
        let previous=player.seen_fishing.get(&b.id).copied();
        let rotation=|v:(f64,f64,f64)| {
            let flat=(v.0*v.0+v.2*v.2).sqrt();
            let byte=|degrees:f64| (degrees*256.0/360.0).floor() as i8;
            (byte(v.0.atan2(v.2).to_degrees()-1e-6),byte(v.1.atan2(flat).to_degrees()))
        };
        let (yaw,pitch)=rotation(b.prev_velocity);
        let mut next=Shown{grid:grid(b.prev_at),hooked:b.prev_hooked,biting:b.prev_bite,velocity:b.prev_velocity,rot:(yaw,pitch)};
        match previous {
            None => {
                let mut body = encode_varint(ADD_ENTITY);
                body.extend(encode_varint(b.id));
                body.extend(crate::items::uuid_for(b.id));
                body.extend(encode_varint(156));
                push_f64(&mut body, b.prev_at.0);
                push_f64(&mut body, b.prev_at.1);
                push_f64(&mut body, b.prev_at.2);
                types::push_velocity(&mut body, b.prev_velocity.0, b.prev_velocity.1, b.prev_velocity.2);
                body.extend([pitch as u8, yaw as u8, 0]);
                body.extend(encode_varint(b.owner));
                write_body(stream, body).await?;
                // Как у оригинала: сразу пакет движения с нулевым сдвигом.
                let mut body = encode_varint(0x35);
                body.extend(encode_varint(b.id));
                for _ in 0..3 { push_i16(&mut body, 0); }
                body.push(0);
                write_body(stream, body).await?;
            }
            Some(old) if b.age.is_multiple_of(5) || b.prev_force => {
                if b.prev_velocity!=old.velocity {
                    let mut body = encode_varint(0x65);
                    body.extend(encode_varint(b.id));
                    types::push_velocity(&mut body, b.prev_velocity.0, b.prev_velocity.1, b.prev_velocity.2);
                    write_body(stream, body).await?;
                }
                let delta = [next.grid.0 - old.grid.0, next.grid.1 - old.grid.1, next.grid.2 - old.grid.2];
                let moved = delta != [0, 0, 0];
                let turned = next.rot != old.rot;
                if delta.iter().all(|d| i16::try_from(*d).is_ok()) {
                    if moved || turned {
                        let mut body = encode_varint(match (moved, turned) { (true, true) => ENTITY_POSITION_AND_ROTATION, (true, false) => ENTITY_POSITION, _ => ENTITY_ROTATION });
                        body.extend(encode_varint(b.id));
                        if moved { for d in delta { push_i16(&mut body, d as i16); } }
                        if turned { body.push(yaw as u8); body.push(pitch as u8); }
                        body.push(u8::from(b.grounded));
                        write_body(stream, body).await?;
                    }
                } else {
                    let position = Position { x: b.prev_at.0, y: b.prev_at.1, z: b.prev_at.2, yaw: 0.0, pitch: 0.0, on_ground: b.grounded };
                    send_teleport_with_speed(stream, b.id, position, b.prev_velocity).await?;
                }
                if !moved && !turned { next = old; }
            }
            Some(_) => {}
        }
        if previous.map_or(next.hooked.is_some()||next.biting,|p|p.hooked!=next.hooked||p.biting!=next.biting) {
            let mut body=encode_varint(SET_ENTITY_DATA);body.extend(encode_varint(b.id));
            // Только изменившееся (чёрный ящик, fishing-catch: при поклёвке одно «9=true»).
            if previous.is_none_or(|p|p.hooked!=next.hooked) {body.push(8);body.extend(encode_varint(1));body.extend(encode_varint(next.hooked.map_or(0,|id|id+1)));}
            if previous.is_none_or(|p|p.biting!=next.biting) {body.push(9);body.extend(encode_varint(8));body.push(u8::from(next.biting));}
            body.push(PROPERTIES_END);write_body(stream,body).await?;
        }
        if let Some(old)=previous&&!b.age.is_multiple_of(5)&&!b.prev_force {
            next = Shown { grid: old.grid, velocity: old.velocity, rot: old.rot, ..next };
        }
        player.seen_fishing.insert(b.id,next);
    }
    Ok(())
}
