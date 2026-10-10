// Попугай на плече: хранится у игрока, из списка мобов убирается.
// Вики, Parrot: «shoulder»; minecraft-data 26.1: shoulder_parrot_left/right.
// [проверить] В 26.1 свойства — номера вариантов, прежнего типа NBT уже нет.
use super::*;
use crate::mob::{Kind, Mob};

fn variants(shoulders: &[Option<Vec<u8>>;2]) -> [i32;2] {
    std::array::from_fn(|i| shoulders[i].as_ref().and_then(|b| Mob::read(&mut b.as_slice()))
        .map_or(-1, |m| i32::from(m.small.variant)))
}

pub(super) async fn shoulders(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let now = shared.world_in(player.dimension).lock().expect("мир").tick();
    let wet = {
        let world = shared.world_in(player.dimension).lock().expect("мир");
        crate::mob::is_water(world.get_block(player.x.floor() as i32,player.y.floor() as i32,player.z.floor() as i32))
    };
    {
        let mut items = shared.items_in(player.dimension).lock().expect("предметы");
        update_shoulders(&mut items, player, now, wet, &mut || shared.next_entity_id());
    }
    let own=variants(&player.shoulders);
    let visible = {
        let mut players=shared.players.lock().expect("игроки");
        players.set_shoulders(player.uuid,own);
        let mut visible=players.members().iter().filter(|m| m.dimension == player.dimension)
            .map(|m| (m.entity_id,players.shoulders(&m.uuid))).collect::<Vec<_>>();
        visible.push((player.entity_id,own)); visible
    };
    player.shoulder_shown.retain(|id,_| visible.iter().any(|(visible_id,_)| id == visible_id));
    for (id,variants) in visible {
        let before = player.shoulder_shown.get(&id).copied().unwrap_or([-1,-1]);
        if before == variants { player.shoulder_shown.insert(id,variants); continue; }
        // Чёрный ящик parrot-shoulder: тип 19 (optional_unsigned_int) — 0 пусто, иначе вариант+1;
        // уходит только то плечо, что изменилось (посадка `19=4`, сход `19=0`).
        let mut body=encode_varint(SET_ENTITY_DATA); body.extend(encode_varint(id));
        for (i,variant) in variants.iter().enumerate().filter(|(i,v)| before[*i] != **v) {
            body.push(19+i as u8); body.extend(encode_varint(19)); body.extend(encode_varint(variant+1));
        }
        body.push(0xff); write_body(stream,body).await?;
        player.shoulder_shown.insert(id,variants);
    }
    Ok(())
}

fn update_shoulders(items: &mut items::Items, player: &mut PlayerState, now: u64, wet: bool, next_id: &mut dyn FnMut()->i32) {
    // [проверить] Посадка после секунды жизни; слезает при падении, воде,
    // полёте, сне или уроне. Простое подпрыгивание не снимает попугая.
    let hurt = player.vit.health < player.shoulder_health;
    // Чёрный ящик parrot-shoulder: оригинал сбрасывает на такт раньше нашего шага проверки (падение 0,45).
    let release = player.vit.fall > 0.4 || wet || player.sleeping.is_some() || hurt
        || player.glide.active || player.creative_flying || player.ride.vehicle.is_some()
        || player.game_mode == GAME_MODE_SPECTATOR || player.vit.health <= 0.0;
    player.shoulder_health=player.vit.health;
        if release && now > player.shoulder_since + 20 {
            for stored in &mut player.shoulders {
                if let Some(bytes) = stored.take() && let Some(mut mob) = Mob::read(&mut bytes.as_slice()) {
                    mob.entity_id=next_id();
                    // В прерванной версии UUID ещё не сохранялся.
                    if mob.small.saved_uuid.is_none() { mob.uuid=crate::mob::uuid_for(mob.entity_id); }
                    mob.x=player.x; mob.y=player.y; mob.z=player.z;
                    mob.tame.owner=Some(player.uuid); mob.tame.owner_id=player.entity_id;
                    mob.tame.ordered_sit=false; mob.tame.sitting=false;
                    // Вики, Parrot: после схода снова на плечо не сядет 100 тактов.
                    mob.ticks=0;
                    items.add_mob(mob);
                }
            }
        } else if !(wet || player.sleeping.is_some() || player.glide.active || player.creative_flying || player.ride.vehicle.is_some()
            || player.game_mode == GAME_MODE_SPECTATOR || player.vit.health <= 0.0 || hurt)
            && player.shoulders.iter().any(Option::is_none) {
            // Чёрный ящик parrot-shoulder: попугай садится и на падающего игрока.

            let free=player.shoulders.iter().filter(|s| s.is_none()).count();
            let mut picked=0;
            let parrots=items.remove_mobs(|m| {
                let yes=picked < free && m.kind == Kind::Parrot && !m.is_dead() && !m.no_ai && m.ticks > 100
                    && m.tame.owner == Some(player.uuid) && !m.tame.ordered_sit && m.leash.is_none()
                    && (m.x-player.x).abs()<0.6 && (m.z-player.z).abs()<0.6 && (m.y-player.y).abs()<1.5;
                if yes { picked+=1; } yes
            });
            for mob in parrots {
                if let Some(slot)=player.shoulders.iter_mut().find(|s| s.is_none()) {
                    let mut bytes=Vec::new(); mob.write(&mut bytes); *slot=Some(bytes); player.shoulder_since=now;
                }
            }
        }

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shoulders_hold_two_owned_parrots_and_release_them_in_water() {
        let mut player=PlayerState::new([7;16],"test".into(),None,None,0x7f,1,(0.5,1.0,0.5));
        player.on_ground=true; player.entity_id=42;
        let mut items=items::Items::new();
        for id in 1..=3 {
            let mut parrot=Mob::new(id,Kind::Parrot,(0.5,1.0,0.5));
            parrot.tame.owner=Some(player.uuid); parrot.small.variant=id as u8; parrot.ticks=101;
            items.add_mob(parrot);
        }
        let original_uuids=items.mobs().iter().map(|m|m.uuid).collect::<Vec<_>>();
        update_shoulders(&mut items,&mut player,30,false,&mut ||100);
        assert_eq!(items.mobs().len(),1); assert_eq!(variants(&player.shoulders),[1,2]);
        update_shoulders(&mut items,&mut player,60,true,&mut ||100);
        assert_eq!(variants(&player.shoulders),[-1,-1]); assert_eq!(items.mobs().len(),3);
        assert!(items.mobs().iter().all(|m|m.tame.owner==Some(player.uuid)));
        assert!(original_uuids.iter().all(|uuid|items.mobs().iter().any(|m|m.uuid==*uuid)));
        update_shoulders(&mut items,&mut player,61,true,&mut ||101);
        assert_eq!(items.mobs().len(),3);
    }

    #[test]
    fn a_sitting_or_foreign_parrot_cannot_mount() {
        let mut player=PlayerState::new([7;16],"test".into(),None,None,0x7f,1,(0.5,1.0,0.5));
        player.on_ground=true;
        let mut items=items::Items::new();
        let mut parrot=Mob::new(1,Kind::Parrot,(0.5,1.0,0.5));
        parrot.tame.owner=Some(player.uuid); parrot.ticks=101; parrot.tame.ordered_sit=true;
        items.add_mob(parrot);
        update_shoulders(&mut items,&mut player,30,false,&mut ||100);
        assert_eq!(variants(&player.shoulders),[-1,-1]);
        items.mobs_mut()[0].tame.ordered_sit=false; items.mobs_mut()[0].tame.owner=Some([8;16]);
        update_shoulders(&mut items,&mut player,31,false,&mut ||100);
        assert_eq!(variants(&player.shoulders),[-1,-1]);
    }

    #[test]
    fn creative_flight_releases_a_shoulder_and_prevents_mounting() {
        let mut player=PlayerState::new([7;16],"test".into(),None,None,0x7f,1,(0.5,1.0,0.5));
        player.on_ground=true;
        let mut items=items::Items::new();
        let mut parrot=Mob::new(1,Kind::Parrot,(0.5,1.0,0.5));
        parrot.tame.owner=Some(player.uuid); parrot.ticks=101;
        items.add_mob(parrot);
        update_shoulders(&mut items,&mut player,30,false,&mut ||100);
        player.creative_flying=true;
        update_shoulders(&mut items,&mut player,60,false,&mut ||100);
        assert_eq!(variants(&player.shoulders),[-1,-1]); assert_eq!(items.mobs().len(),1);
        items.mobs_mut()[0].ticks=101;
        update_shoulders(&mut items,&mut player,61,false,&mut ||101);
        assert_eq!(variants(&player.shoulders),[-1,-1]);
    }
}
