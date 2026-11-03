// Регрессии хвостов Java 26.1.2: реальные обработчики и пакетный буфер,
// без сокетов. Внешняя сверка сценариев с оригиналом [проверить].
use super::*;
use crate::survival;

fn shared() -> Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/tails-no-ops.json")),crate::skins::Settings::default())
}
fn player(shared: &Shared, id: u8) -> PlayerState {
    let mut p=PlayerState::new([id;16],format!("Tails{id}"),None,None,0x7f,u64::from(id),(0.5,64.0,0.5));
    p.game_mode=GAME_MODE_SURVIVAL;p.awaiting_teleport=false;
    shared.players.lock().unwrap().join(p.member(),p.entity_id).unwrap();
    p
}
fn stack(name: &str) -> Stack { Stack::new(blocks::item_named(name).unwrap(),1) }
async fn packet_ids(out: &Outgoing<'_>) -> Vec<i32> {
    let mut bytes=out.queued();let mut ids=Vec::new();
    while !bytes.is_empty() {ids.push(read_packet(&mut bytes).await.unwrap().id);}
    ids
}
fn boundary(shared: &Shared) {
    let _publication=shared.publication.lock().unwrap();
    shared.world_in(Dimension::Overworld).lock().unwrap().publish_events();
    shared.players.lock().unwrap().publish_tick();
}

#[test]
fn crossbow_sound_and_loaded_equipment_wait_for_the_same_boundary() {
    let shared=shared();let mut p=player(&shared,1);
    p.inventory.set(p.inventory.selected_slot(),Some(stack("crossbow")));
    p.inventory.set(inventory::OFFHAND,Some(stack("arrow")));
    publish_player(&shared,&p);
    assert!(vitals::start_using(&shared,&mut p,0,100));
    let (a,b)={let mut w=shared.world_in(p.dimension).lock().unwrap();(w.watch_events(41),w.watch_events(42))};
    boundary(&shared);
    let unloaded=shared.players.lock().unwrap().published()[0].0.held;
    vitals::tick_crossbow(&shared,&mut p,125);
    assert_ne!(p.inventory.held(),unloaded);
    assert_eq!(shared.players.lock().unwrap().published()[0].0.held,unloaded);
    let mut w=shared.world_in(p.dimension).lock().unwrap();
    let first=w.published_events_since(a,41);let second=w.published_events_since(b,42);
    assert_eq!(first.0.len(),second.0.len());assert!(first.0.is_empty());drop(w);
    boundary(&shared);
    assert_eq!(shared.players.lock().unwrap().published()[0].0.held,p.inventory.held());
    let mut w=shared.world_in(p.dimension).lock().unwrap();
    let events=w.published_events_since(first.1,41).0;
    assert!(events.iter().any(|e|matches!(e,WorldEvent::Sound(sound) if sound.name=="minecraft:item.crossbow.loading_end")));
}

#[tokio::test]
async fn arrow_hitting_in_spawn_tick_sends_spawn_then_remove_once() {
    let shared=shared();let mut observer=player(&shared,1);observer.x=4.0;
    {
    let mut w=shared.world_in(observer.dimension).lock().unwrap();
    w.set_block_silently(0,63,0,blocks::state_by_name("stone").unwrap());
    let mut cow=crate::mob::Mob::new(30,crate::mob::Kind::Cow,(0.5,64.0,1.5));cow.no_ai=true;
    shared.items_in(observer.dimension).lock().unwrap().add_mob(cow);
    let mut arrow=crate::arrow::Arrow::from_mob(40,observer.dimension,(0.5,65.0,0.5),(0.0,0.0,3.0),0,"");
    arrow.fresh=false;shared.arrows.lock().unwrap().spawn(arrow);
    crate::arrow::step(&shared,observer.dimension,&mut w);
    assert!(shared.arrows.lock().unwrap().list.is_empty());
    }
    let mut out=Outgoing::capture();bow::send_arrow_ticks(&mut out,&shared,&mut observer,None).await.unwrap();
    let ids=packet_ids(&out).await;
    assert_eq!(ids,vec![ADD_ENTITY,REMOVE_ENTITIES]);
    bow::send_arrow_ticks(&mut out,&shared,&mut observer,None).await.unwrap();
    assert_eq!(packet_ids(&out).await,ids);
}

#[test]
fn summoned_no_gravity_arrow_keeps_height_but_still_has_drag() {
    let shared=shared();
    shared.world_in(Dimension::Overworld).lock().unwrap().set_block_silently(0,63,0,blocks::state_by_name("stone").unwrap());
    crate::commands::run("summon arrow 0.5 65 0.5 {NoGravity:1b,Motion:[0.1d,0d,0d]}",&crate::commands::Source::Console,&shared);
    for _ in 0..5 {crate::arrow::step(&shared,Dimension::Overworld,&mut shared.world_in(Dimension::Overworld).lock().unwrap());}
    let arrows=shared.arrows.lock().unwrap();let a=&arrows.list[0];
    assert!(a.no_gravity);assert_eq!((a.y,a.vy),(65.0,0.0));assert!((a.vx-0.1*0.99f64.powi(5)).abs()<1e-9);
}

#[test]
fn use_item_waterlogs_from_either_hand_and_lava_does_not_replace_slab() {
    for hand in [0,1] {
        let shared=shared();let mut p=player(&shared,1);
        let slot=if hand==1 {inventory::OFFHAND} else {p.inventory.selected_slot()};
        p.inventory.set(slot,Some(stack("water_bucket")));
        let slab=blocks::state_from_text("oak_slab[type=bottom,waterlogged=false]").unwrap();
        shared.world_in(p.dimension).lock().unwrap().set_block_silently(0,65,2,slab);
        let mut payload=encode_varint(hand);payload.extend(encode_varint(1));payload.extend(0f32.to_be_bytes());payload.extend(0f32.to_be_bytes());
        use_item(&payload,&mut p,&shared).unwrap();
        let state=shared.world_in(p.dimension).lock().unwrap().get_block(0,65,2);
        assert_eq!(blocks::orientation_of(state).unwrap().value(state,"waterlogged"),Some("true"));
        assert_eq!(p.inventory.slot(slot),Some(stack("bucket")));
        p.inventory.set(slot,Some(stack("lava_bucket")));
        use_item(&payload,&mut p,&shared).unwrap();
        assert_eq!(shared.world_in(p.dimension).lock().unwrap().get_block(0,65,2),state);
    }
}

#[tokio::test]
async fn cart_window_reads_published_slots_while_clicks_read_live_slots() {
    let shared=shared();let mut p=player(&shared,1);
    let mut cart=crate::minecart::Minecart::new(40,crate::minecart::Kind::Chest,(0.5,64.0,1.5));
    let before=cart.slots.clone();cart.slots[0]=Some(stack("dirt"));
    shared.items_in(p.dimension).lock().unwrap().add_cart(cart);
    shared.world_in(p.dimension).lock().unwrap().publish_contents([(40,before.clone())].into());
    p.window=Some(OpenWindow {id:1,halves:Vec::new(),cart:Some(40),shown:before,lectern:false,crafter:false,kind:None,props:Vec::new(),merchant:None,station:None,plugin_chest: None, ender: false});
    let mut out=Outgoing::capture();send_window_changes(&mut out,&shared,&mut p).await.unwrap();assert!(out.queued().is_empty());
    let live=cart_slots(&shared,p.dimension,40).unwrap();assert_eq!(live[0],Some(stack("dirt")));
    shared.world_in(p.dimension).lock().unwrap().publish_contents([(40,live)].into());
    send_window_changes(&mut out,&shared,&mut p).await.unwrap();assert_eq!(packet_ids(&out).await,vec![SET_CONTAINER_SLOT]);
}

#[tokio::test]
async fn periodic_effect_damage_is_announced_once_and_not_repeated_next_tick() {
    // Досрочной рассылки метаданных и скорости нет: фазы такта не тронуты
    // (чёрный ящик, effect-regen совпал после отката досрочного сброса).
    let shared=shared();let mut p=player(&shared,1);p.on_ground=true;
    shared.natural_regeneration.store(false,std::sync::atomic::Ordering::Relaxed);
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(0,63,0,blocks::state_by_name("stone").unwrap());
    p.vit.give_effect(survival::EffectKind::Poison,0,25);
    let mut out=Outgoing::capture();vitals::tick(&mut out,&shared,&mut p).await.unwrap();
    assert_eq!(packet_ids(&out).await.iter().filter(|id|**id==0x19).count(),1);
    let mut next=Outgoing::capture();vitals::tick(&mut next,&shared,&mut p).await.unwrap();
    assert!(!packet_ids(&next).await.contains(&0x19));
}

#[test]
fn bottle_experience_covers_three_to_eleven_with_mean_seven() {
    let outcomes:Vec<_>=(0..5).flat_map(|a|(0..5).map(move |b|crate::thrown::bottle_experience(a,b))).collect();
    assert_eq!(outcomes.iter().min(),Some(&3));assert_eq!(outcomes.iter().max(),Some(&11));
    assert_eq!(outcomes.iter().sum::<i32>(),25*7);
}

#[test]
fn respiration_adds_fifteen_seconds_per_level_in_expectation() {
    // Вики, Respiration: «15 seconds per enchantment level». Стратифицированные
    // броски проверяют вероятность, не обещают точный срок случайному игроку.
    for level in 0..=3 {
        let mut vit=survival::Vitals::new();
        for _ in 0..300 {for index in 0..=level {
            vit.air_tick_with_bonus(true,false,level as f64,(index as f32+0.5)/(level+1) as f32);
        }}
        assert_eq!(vit.air,0);
    }
}

#[tokio::test]
async fn mob_effect_packets_are_dirty_on_apply_and_clear_only() {
    let shared=shared();let mut p=player(&shared,1);
    let mut cow=crate::mob::Mob::new(30,crate::mob::Kind::Cow,(0.5,64.0,2.5));cow.no_ai=true;
    {let mut items=shared.items_in(p.dimension).lock().unwrap();items.add_mob(cow);items.snapshot_mobs();}
    let mut intro=Outgoing::capture();mobs::send_mob_ticks(&mut intro,&shared,&mut p).await.unwrap();
    for change in [1,0,-1] {
        {let mut items=shared.items_in(p.dimension).lock().unwrap();
        if change==1 {items.mobs_mut()[0].vitals.give_effect(survival::EffectKind::Slowness,0,100);}
        if change == -1 {items.mobs_mut()[0].vitals.effects.clear();}
        items.snapshot_mobs();}
        let mut out=Outgoing::capture();mobs::send_mob_ticks(&mut out,&shared,&mut p).await.unwrap();
        assert_eq!(packet_ids(&out).await.iter().filter(|id|**id==0x83).count(),usize::from(change!=0));
    }
}

#[tokio::test]
async fn raid_setup_converts_bad_omen_and_requests_wave_one() {
    let shared=shared();let mut p=player(&shared,1);p.on_ground=true;
    shared.raids.store(true,std::sync::atomic::Ordering::Relaxed);
    shared.difficulty.store(2,std::sync::atomic::Ordering::Relaxed);
    let mut villager=crate::mob::Mob::new(30,crate::mob::Kind::Villager,(2.5,64.0,-2.5));
    villager.villager.job_site=Some((3,64,-3));
    shared.items_in(p.dimension).lock().unwrap().add_mob(villager);
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(0,63,0,blocks::state_by_name("stone").unwrap());
    p.vit.give_effect(survival::EffectKind::BadOmen,0,6000);
    for tick in 0..=601 {
        shared.ticks.send_replace(tick);
        let mut out=Outgoing::capture();vitals::tick(&mut out,&shared,&mut p).await.unwrap();
        if tick==0 {assert!(p.vit.has_effect(survival::EffectKind::RaidOmen));}
    }
    let world=shared.world_in(p.dimension).lock().unwrap();
    assert_eq!(world.raids.requests.len(),1);
    assert_eq!(world.raids.requests[0].player,p.entity_id);
}
