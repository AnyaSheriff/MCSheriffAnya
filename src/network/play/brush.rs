// Кисть использует общий учёт удержания кнопки. [проверить] частицы; луч
// проверяется заново, чтобы нельзя было чистить сквозь стену. Взмах — на
// пятом такте каждого десятка от начала (чёрный ящик, brush-sand: щелчок на +10,
// пыль на +15 и +35); сама по себе «использовать предмет» без блока кисть не
// запускает, а лишь заново присылает инвентарь.
use super::*;
fn target(player:&PlayerState,world:&World)->Option<((i32,i32,i32),u8)> {
    let yaw=f64::from(player.yaw).to_radians();let pitch=f64::from(player.pitch).to_radians();
    let (fx,fy,fz)=(-yaw.sin()*pitch.cos(),-pitch.sin(),yaw.cos()*pitch.cos());
    let mut prev=(player.x.floor() as i32,(player.y+1.62).floor() as i32,player.z.floor() as i32);
    for n in 1..=90 {
        let d=f64::from(n)*0.05;
        let at=((player.x+fx*d).floor() as i32,(player.y+1.62+fy*d).floor() as i32,(player.z+fz*d).floor() as i32);
        let state=world.get_block(at.0,at.1,at.2);
        if crate::archaeology::suspicious(state) {return Some((at,face_of(prev,at,(fx,fy,fz))));}
        if crate::mob::solid(state) {return None;}
        prev=at;
    }
    None
}
/// Грань блока `at`, обращённая к клетке `prev` (0 вниз … 5 восток); при ходе
/// луча по диагонали берётся ось, вдоль которой луч идёт быстрее.
fn face_of(prev:(i32,i32,i32),at:(i32,i32,i32),ray:(f64,f64,f64))->u8 {
    let moves=[(prev.1-at.1,ray.1.abs(),[0u8,1]),(prev.2-at.2,ray.2.abs(),[2,3]),(prev.0-at.0,ray.0.abs(),[4,5])];
    moves.iter().filter(|m|m.0!=0).max_by(|a,b|a.1.total_cmp(&b.1)).map_or(1,|m|if m.0<0 {m.2[0]} else {m.2[1]})
}
fn holding(player:&PlayerState,hand:i32)->bool {
    if !matches!(hand,0|1) || player.vit.dead || player.game_mode==GAME_MODE_SPECTATOR {return false;}
    let slot=if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()};
    player.inventory.slot(slot).and_then(|s|blocks::item_name(s.item))==Some("brush")
}
/// Щелчок кистью по блоку: удержание начинается, если там подозрительный блок.
pub(super) fn start(player:&mut PlayerState,shared:&Shared,hand:i32)->bool {
    if !holding(player,hand) {return false;}
    if player.net.using.is_some() {return true;}
    if target(player,&shared.world_in(player.dimension).lock().expect("мир")).is_some() {
        player.net.using=Some(vitals::Using {hand,since:*shared.ticks.borrow(),kind:vitals::UseKind::Brush});
        player.net.last_hand=hand;
    }
    true
}
/// «Использовать предмет» с кистью в воздухе: ничего не начинается; если
/// удержания не было, оригинал присылает весь инвентарь (brush-sand, +110).
pub(super) fn idle_use(player:&PlayerState,hand:i32)->Option<bool> {
    holding(player,hand).then(||player.net.using.is_none())
}
pub(super) fn tick(player:&mut PlayerState,shared:&Shared,now:u64) {
    let Some(vitals::Using {kind:vitals::UseKind::Brush,hand,since})=player.net.using else {return;};
    let slot=if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()};
    let Some(stack)=player.inventory.slot(slot).filter(|s|blocks::item_name(s.item)==Some("brush")) else {player.net.using=None;return;};
    let mut world=shared.world_in(player.dimension).lock().expect("мир");
    let Some((at,face))=target(player,&world) else {player.net.using=None;return;};
    if now.saturating_sub(since)%10!=5 {return;}
    let sand=blocks::block_at_state(world.get_block(at.0,at.1,at.2))==Some("suspicious_sand");
    world.note_sound_except(at,if sand {"minecraft:item.brush.brushing.sand"} else {"minecraft:item.brush.brushing.gravel"},1.0,1.0,player.entity_id);
    if let Some(loot)=crate::archaeology::stroke(&mut world,at,now,face) {
        if player.game_mode!=GAME_MODE_CREATIVE {
            let worn=combat::wear(&stack,1,false,||world.random_unit());
            player.inventory.set(slot,combat::add_damage(stack,worn));player.net.slot_changed(slot);
        }
        player.progress.used(stack.item);
        player.net.using=None;
        drop(world);
        let mut items=shared.items_in(player.dimension).lock().expect("предметы");
        for stack in loot {items.drop_item(shared.next_entity_id(),stack,(at.0 as f64+0.5,at.1 as f64+1.0,at.2 as f64+0.5),(0.0,0.0,0.0),crate::items::MINED_DELAY);}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn face_points_back_at_the_player() {
        assert_eq!(face_of((0,0,1),(0,0,2),(0.0,0.0,1.0)),2);
        assert_eq!(face_of((0,2,0),(0,1,0),(0.0,-1.0,0.0)),1);
        assert_eq!(face_of((3,0,0),(2,0,0),(-1.0,0.0,0.0)),5);
        assert_eq!(face_of((0,0,0),(1,0,1),(0.3,0.0,0.9)),2);
    }
}
