// Мешок в руке: «использовать» держит предмет, и пока кнопка зажата, из мешка вылетает по одной
// стопке — первая сразу, вторая через 12 тактов, дальше каждые 2 (чёрный ящик bundle-use-drop: стопки на
// +0, +12, +14, +16; Metadata 8=1 держится до отпускания даже у пустого мешка). Вики, Bundle: «one group of items
// is emptied at a time». [проверить] длинная очередь.
use super::*;
/// Первая пауза 12 тактов, дальше по 2 (чёрный ящик bundle-use-drop: +0, +12, +14, +16).
const FIRST_PAUSE: u64 = 12;
const PAUSE: u64 = 2;
fn slot_of(player:&PlayerState,hand:i32)->i32 { if hand==1 {inventory::OFFHAND} else {player.inventory.selected_slot()} }
/// Начало использования: флаг «использует предмет» и первая стопка. false — в руке не мешок.
pub(super) fn start(shared:&Shared,player:&mut PlayerState,hand:i32,now:u64)->bool {
    if !matches!(hand,0|1) || player.vit.dead || player.game_mode==GAME_MODE_SPECTATOR || player.net.using.is_some() {return false;}
    let slot=slot_of(player,hand);
    if !player.inventory.slot(slot).is_some_and(|s|crate::bundle::is_bundle(s.item)) {return false;}
    player.net.using=Some(vitals::Using{hand,since:now,kind:vitals::UseKind::Bundle});
    player.net.last_hand=hand;
    drop_one(shared,player,slot);
    true
}
fn drop_one(shared:&Shared,player:&mut PlayerState,slot:i32) {
    let Some(b)=player.inventory.slot(slot).filter(|s|crate::bundle::is_bundle(s.item)) else {return;};
    let (rest,item)=crate::bundle::extract(b);
    let Some(item)=item else {return;};
    player.inventory.set(slot,Some(rest)); player.net.slot_changed(slot);
    let mut world=shared.world_in(player.dimension).lock().expect("мир");
    // Бросок игрока с разбросом: 0,3 вдоль взгляда, из точки на уровне глаз без сдвига вперёд (там же).
    let (yaw,pitch)=(f64::from(player.yaw).to_radians(),f64::from(player.pitch).to_radians());
    let spread=f64::from(world.random_unit())*std::f64::consts::TAU;
    let power=0.02*f64::from(world.random_unit());
    let velocity=(-yaw.sin()*pitch.cos()*0.3+spread.cos()*power,-pitch.sin()*0.3+0.1+f64::from(world.random_unit()-world.random_unit())*0.1*0.2,yaw.cos()*pitch.cos()*0.3+spread.sin()*power);
    let pitch_sound=0.8+world.random_unit()*0.4;
    world.note_sound_player_at((player.x,player.y+0.5,player.z),"minecraft:item.bundle.drop_contents",0.8,pitch_sound);
    drop(world);
    shared.items_in(player.dimension).lock().expect("предметы").drop_from_player(shared.next_entity_id(),item,(player.x,player.y+1.32,player.z),velocity,player.uuid);
}
pub(super) fn tick(player:&mut PlayerState,shared:&Shared,now:u64) {
    let Some(vitals::Using{kind:vitals::UseKind::Bundle,hand,since})=player.net.using else {return;};
    let slot=slot_of(player,hand);
    if !player.inventory.slot(slot).is_some_and(|s|crate::bundle::is_bundle(s.item)) {player.net.using=None;return;}
    let elapsed=now.saturating_sub(since);
    if elapsed==FIRST_PAUSE || (elapsed>FIRST_PAUSE && (elapsed-FIRST_PAUSE).is_multiple_of(PAUSE)) {drop_one(shared,player,slot);}
}
