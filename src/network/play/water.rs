// Ведра и кормление водных мобов: общий склад сущностей и обычные стопки.
// Вики, Bucket of aquatic mob, Axolotl, Dolphin, Tadpole. [проверить] Пакеты.
use super::*;
use crate::mob::{Kind, water};

pub(super) fn use_on_mob(items: &mut items::Items, index: usize, player: &mut PlayerState, world: &mut World, reaction: &mut Reaction) -> bool {
    let mob=&items.mobs()[index];
    let held=player.inventory.held().and_then(|stack| blocks::item_name(stack.item));
    if held==Some("water_bucket") && let Some(item)=water::bucket(mob.kind).and_then(blocks::item_named) {
        let stack=Stack::new(item,1).with_extras(water::bucket::capture(mob));
        let id=mob.entity_id;
        let at=(mob.x,mob.y,mob.z);
        let sound=match mob.kind { Kind::Axolotl=>"minecraft:item.bucket.fill_axolotl",Kind::Tadpole=>"minecraft:item.bucket.fill_tadpole",_=>"minecraft:item.bucket.fill_fish" };
        items.remove_mobs(|mob| mob.entity_id==id);
        let slot=inventory::FIRST_HOTBAR+player.inventory.selected();
        if player.game_mode==GAME_MODE_CREATIVE {
            let (changed,left)=player.inventory.put(stack);
            reaction.slots.extend(changed);
            // Вода в творческом остаётся; полное ведро выдаётся отдельно.
            // [проверить] При полном инвентаре оно не помещается.
            let _=left;
        } else {
            player.inventory.set(slot,Some(stack)); reaction.slots.push(slot);
        }
        // Чёрный ящик fish-bucket: сам игрок свой звук не слышит (всем, кроме него).
        world.actor=Some(player.entity_id);
        world.note_sound_by_hand(at.0.floor() as i32,at.1.floor() as i32,at.2.floor() as i32,sound,1.0,1.0);
        world.actor=None;
        return true;
    }
    if mob.kind==Kind::Dolphin && matches!(held,Some("cod" | "salmon")) {
        let mob=&mut items.mobs_mut()[index];
        if mob.is_baby() { mob.age=(mob.age+((-mob.age) as f32*0.1) as i32).min(0); }
        else { mob.water.got_fish=true; }
        // Кораблей/руин в генераторе нет: выдуманной цели сокровища не даём.
        if player.game_mode!=GAME_MODE_CREATIVE && let Some(slot)=player.inventory.spend_held() { reaction.slots.push(slot); }
        return true;
    }
    if mob.kind==Kind::Tadpole && held==Some("slime_ball") {
        let mob=&mut items.mobs_mut()[index];
        let left=(water::GROW_TADPOLE-mob.water.growth).max(0);
        mob.water.growth+=(left as f32*0.1) as i32;
        mob.events.push(crate::mob::Event::Hearts);
        if player.game_mode!=GAME_MODE_CREATIVE && let Some(slot)=player.inventory.spend_held() { reaction.slots.push(slot); }
        return true;
    }
    if matches!(mob.kind,Kind::Turtle | Kind::Frog) && mob.water.has_egg && held.is_some_and(|food|mob.kind.likes(food)) { return true; }
    false
}
