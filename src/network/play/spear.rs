// Рывок копья «Порыв» (Lunge), Java Player Action 7.
// Собственные чёрные ящики 26.1.2: enchant-lunge-probe/conditions/
// cost-motion/cooldown. Клиент отдельно присылает атаку сущности: сам
// status7 не ищет жертв и не наносит дополнительный урон.

use super::*;

/// Наземный, ледяной и воздушный замеры дают один горизонтальный импульс:
/// после сетевого квантования 0.45797473, 0.91601050, 1.37398523.
/// Наклон 60° уменьшает его вдвое, вертикальный взгляд обнуляет.
/// [проверить] Сложение с горизонтальной скоростью бегущего игрока.
const IMPULSE_PER_LEVEL: f64 = 0.458;

fn impulse(player: &PlayerState, level: i32) -> (f64, f64, f64) {
    let yaw = f64::from(player.yaw).to_radians();
    let pitch = f64::from(player.pitch).to_radians();
    let boost = IMPULSE_PER_LEVEL * f64::from(level) * pitch.cos();
    let old = player.net.last_delta;
    let vertical = if player.on_ground { 0.0 } else { old.1 };
    (old.0 - yaw.sin() * boost,
        (vertical - player.attrs.value(crate::attributes::GRAVITY)) * 0.98,
        old.2 + yaw.cos() * boost)
}

/// cost-motion при difficulty normal: I/II/III снимают 1/2/3 насыщенности,
/// остаток exhaustion не меняется: четыре единицы за уровень.
fn exhaustion(level: i32) -> f32 { 4.0 * level as f32 }

pub(super) fn jab(shared: &Shared, player: &mut PlayerState) -> bool {
    if player.vit.dead || player.game_mode == GAME_MODE_SPECTATOR || player.ride.vehicle.is_some()
        || player.net.in_water || player.net.in_lava { return false; }
    let Some(stack) = player.inventory.held() else { return false; };
    if !blocks::item_name(stack.item).is_some_and(|name| name.ends_with("_spear")) { return false; }
    let level = stack.extras().level("lunge");
    if level <= 0 { return false; }
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    // food-threshold (normal, LungeI): еда7 разрешает, еда6 запрещает.
    // food-drain (LungeIII) также прекращает рывки при еде4.
    if !creative && player.vit.food <= 6 { return false; }
    let now = *shared.ticks.borrow();
    vitals::update_equipment(player, now);
    let since = u32::try_from(now.saturating_sub(player.last_attack)).unwrap_or(u32::MAX);
    // threshold: железное копьё I/III отвергает 10–13 тактов и принимает 14;
    // cooldown: при attack_speed base10 достаточно 3 тактов.
    // [проверить] Промежуточные скорости вне этих двух замеров.
    if combat::charge(since, player.attrs.value(crate::attributes::ATTACK_SPEED)) < 0.75 { return false; }

    player.last_attack = now;
    player.net.velocity = Some(impulse(player, level));
    player.net.velocity_due = false;
    let sound = match level {
        1 => "minecraft:item.spear.lunge_1",
        2 => "minecraft:item.spear.lunge_2",
        _ => "minecraft:item.spear.lunge_3",
    };
    shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x, player.y, player.z), sound, 1.0, 1.0);
    if !creative {
        player.vit.add_exhaustion(exhaustion(level));
        let slot = player.inventory.selected_slot();
        pvp::wear_slot(shared, player, slot, stack, 1, false);
    }
    player.progress.used(stack.item);
    publish_player(shared, player);
    true
}


// Вики Spear, Charge attack: «relative to the target», «offhand».
// NoAI-корова на официальном777 получила6 урона на +17 и +27, износ1/2.
// Повторный контакт — через10тактов; [проверить] толчки внутри неуязвимости.
// Скорость — блоки/с; Strength/Weakness не входят в кинетический урон.
#[derive(Clone, Copy, Debug)]
struct Tier { multiplier: f64, delay: u64, engaged: u64, tired: u64, duration: u64, dismount_speed: f64 }
fn tier(item: i32) -> Option<Tier> {
    let (multiplier, delay, engaged, tired, duration, dismount_speed) = match blocks::item_name(item)? {
        "wooden_spear" => (0.7, 15, 100, 100, 300, 14.0),
        "golden_spear" => (0.7, 14, 70, 100, 275, 13.0),
        "stone_spear" => (0.82, 14, 90, 90, 275, 13.0),
        "copper_spear" => (0.82, 13, 80, 85, 250, 12.0),
        "iron_spear" => (0.95, 12, 50, 85, 225, 11.0),
        "diamond_spear" => (1.075, 10, 60, 70, 200, 10.0),
        "netherite_spear" => (1.2, 8, 50, 60, 175, 9.0),
        _ => return None,
    };
    Some(Tier { multiplier, delay, engaged, tired, duration, dismount_speed })
}

#[derive(Default)]
pub(super) struct Charge {
    active: Option<(u64, i32, i32)>,
    at: (f64, f64, f64),
    tick: u64,
    hit: std::collections::HashMap<i32, u64>,
    players: std::collections::HashMap<i32, (f64, f64, f64)>,
    sound: Option<u64>,
}

pub(super) fn start(player: &mut PlayerState, hand: i32, now: u64, item: i32) {
    player.net.spear_charge = Charge { active: Some((now, hand, item)), at: (player.x, player.y, player.z), tick: now, ..Default::default() };
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Contact { damage: f32, knockback: bool, dismount: bool }
fn contact(tier: Tier, elapsed: u64, speed: f64, relative: f64) -> Option<Contact> {
    let age = elapsed.checked_sub(tier.delay)?;
    if age >= tier.duration { return None; }
    let damage = if relative >= 4.6 { (1.0 + tier.multiplier * relative).floor() as f32 } else { 0.0 };
    let knockback = age < tier.engaged + tier.tired && speed >= 5.1;
    let dismount = age < tier.engaged && speed >= tier.dismount_speed;
    (damage > 0.0 || knockback || dismount).then_some(Contact { damage, knockback, dismount })
}

fn ray_box(eye: (f64, f64, f64), dir: (f64, f64, f64), feet: (f64, f64, f64), size: (f64, f64), range: f64) -> Option<f64> {
    let origin = [eye.0, eye.1, eye.2];
    let direction = [dir.0, dir.1, dir.2];
    let low = [feet.0-size.0*0.5-0.125, feet.1-0.125, feet.2-size.0*0.5-0.125];
    let high = [feet.0+size.0*0.5+0.125, feet.1+size.1+0.125, feet.2+size.0*0.5+0.125];
    let (mut near, mut far) = (2.0f64, range);
    for i in 0..3 {
        if direction[i].abs() < 1e-9 {
            if origin[i] < low[i] || origin[i] > high[i] { return None; }
        } else {
            let a = (low[i]-origin[i])/direction[i];
            let b = (high[i]-origin[i])/direction[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if far < near { return None; }
        }
    }
    // Вики: «minimum attack range» ограничивает сам луч: пересечение
    // ящика с дальней частью луча допустимо, даже если ближняя грань ближе2.
    (near <= range).then_some(near)
}

pub(super) fn tick(shared: &Shared, player: &mut PlayerState) {
    let now = *shared.ticks.borrow();
    let Some(using) = player.net.using.filter(|u| u.kind == vitals::UseKind::Spear) else {
        player.net.spear_charge = Charge::default();
        return;
    };
    let slot = if using.hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else { player.net.using = None; return; };
    let Some(tier) = tier(stack.item) else { player.net.using = None; return; };
    if player.vit.dead || player.game_mode == GAME_MODE_SPECTATOR
        || player.net.spear_charge.active != Some((using.since, using.hand, stack.item)) {
        player.net.using = None;
        player.net.spear_charge = Charge::default();
        return;
    }
    let mut state = std::mem::take(&mut player.net.spear_charge);
    if now <= state.tick { player.net.spear_charge = state; return; }
    let at = (player.x, player.y, player.z);
    let scale = 20.0 / (now-state.tick) as f64;
    let velocity = ((at.0-state.at.0)*scale, (at.1-state.at.1)*scale, (at.2-state.at.2)*scale);
    state.at = at;
    state.tick = now;
    let yaw = f64::from(player.yaw).to_radians();
    let pitch = f64::from(player.pitch).to_radians();
    let dir = (-yaw.sin()*pitch.cos(), -pitch.sin(), yaw.cos()*pitch.cos());
    let dot = |v: (f64, f64, f64)| v.0*dir.0+v.1*dir.1+v.2*dir.2;
    let speed = dot(velocity);
    let elapsed = now.saturating_sub(using.since);
    let eye = (at.0, at.1 + if player.sneaking { 1.27 } else { 1.62 }, at.2);
    let range = if player.game_mode == GAME_MODE_CREATIVE { 6.5 } else { 4.5 };
    let end = (eye.0+dir.0*range, eye.1+dir.1*range, eye.2+dir.2*range);
    let wall = crate::wind::first_hit(&shared.world_in(player.dimension).lock().expect("мир"), eye, end).map_or(range, |hit| hit.0*range);
    let data = if using.hand == 0 { stack.extras() } else { Default::default() };
    let mut contacts = Vec::new();
    {
        let mut items = shared.items_in(player.dimension).lock().expect("мобы");
        for mob in items.mobs_mut() {
            if mob.is_dead() || state.hit.get(&mob.entity_id).is_some_and(|before| now.saturating_sub(*before)<10) { continue; }
            if ray_box(eye, dir, (mob.x,mob.y,mob.z), mob.size(), wall).is_none() { continue; }
            // NoAI сохраняет импульс в metadata, но не двигает тело:
            // повторный замер777 даёт те же6урона после импульса0,4.
            let target_speed=if mob.no_ai {0.0}else{dot((mob.vx*20.0,mob.vy*20.0,mob.vz*20.0))};
            let relative = speed - target_speed;
            let Some(hit) = contact(tier, elapsed, speed, relative) else { continue; };
            let (undead, arthropod) = combat::mob_traits(mob.kind.name());
            let damage = hit.damage + if hit.damage > 0.0 { combat::enchant_bonus(&data,undead,arthropod) } else { 0.0 };
            let push = hit.knockback.then_some(crate::mob::Push { dir: crate::mob::look_direction(player.yaw), strength: crate::mob::BASE_KNOCKBACK, cause: player.entity_id });
            let health=mob.vitals.health;
            let result = mob.hurt_credited_burning(damage, crate::survival::Source::Spear, push, player.uuid, data.level("looting"), data.level("fire_aspect")*80);
            let landed = result.is_some();
            if let Some(result) = result {
                let count=(result.taken.min(health)/2.0) as i32;
                if count>0 {mob.events.push(crate::mob::Event::Particle {kind:crate::mob::Particle::DamageIndicator,
                    at:(mob.x,mob.y+mob.size().1*0.5,mob.z),offset:(0.1,0.0,0.1),speed:0.2,count});}
            }
            if !landed && hit.knockback { mob.knockback(dir.0,dir.2,crate::mob::BASE_KNOCKBACK); }
            if landed && hit.knockback && data.level("knockback")>0 {mob.knockback(dir.0,dir.2,f64::from(data.level("knockback"))*0.5);}
            contacts.push((mob.entity_id,hit.dismount,landed));
        }
    }
    let members = shared.players.lock().expect("игроки").members().to_vec();
    for victim in members {
        if victim.entity_id == player.entity_id || victim.dimension != player.dimension { continue; }
        let feet = (victim.position.x,victim.position.y,victim.position.z);
        let previous = state.players.insert(victim.entity_id,feet).unwrap_or(feet);
        if state.hit.get(&victim.entity_id).is_some_and(|before| now.saturating_sub(*before)<10) || victim.game_mode == GAME_MODE_CREATIVE || victim.game_mode == GAME_MODE_SPECTATOR { continue; }
        if ray_box(eye,dir,feet,(0.6,if victim.sneaking {1.5}else{1.8}),wall).is_none() { continue; }
        if !shared.scoreboard.lock().expect("табло").may_hurt(&player.name,&victim.name) { continue; }
        let relative = speed-dot(((feet.0-previous.0)*scale,(feet.1-previous.1)*scale,(feet.2-previous.2)*scale));
        let Some(hit) = contact(tier,elapsed,speed,relative) else { continue; };
        let damage = hit.damage + if hit.damage > 0.0 { combat::enchant_bonus(&data,false,false) } else { 0.0 };
        let gear=shared.players.lock().expect("игроки").gear(&victim.uuid);
        let outcome=combat::outcome(&gear.view,damage,crate::survival::Source::Spear,now,(feet.0,feet.2,victim.position.yaw),(at.0,at.2));
        if let combat::Outcome::Taken(taken)=outcome {
            let count=(taken.min(gear.health)*0.5) as i32;
            let mut world=shared.world_in(player.dimension).lock().expect("мир");
            world.note_sound_player_except(feet,"minecraft:entity.player.hurt",1.0,1.0,victim.entity_id);
            if count>0 {world.note_particle(crate::mob::Particle::DamageIndicator.id(),None,(feet.0,feet.1+0.9,feet.2),(0.1,0.0,0.1),0.2,count);}
        }
        shared.world_in(player.dimension).lock().expect("мир").note_player_strike(combat::PlayerStrike { projectile: None,
            kinetic: Some(combat::Kinetic { knockback:hit.knockback, dismount:hit.dismount }),
            victim:victim.entity_id,attacker:player.entity_id,attacker_uuid:player.uuid,attacker_name:player.name.clone(),
            from:at,yaw:player.yaw,damage,source:crate::survival::Source::Spear,extra_knockback:if hit.knockback {data.level("knockback") as f32}else{0.0},weapon:Some(stack),
            disables_shield_ticks:0,breach_scale:1.0,fire_ticks:data.level("fire_aspect")*80,ignores_shield:false,thorns_slots:Vec::new(),effects:Vec::new() });
        contacts.push((victim.entity_id,false,matches!(outcome,combat::Outcome::Taken(_))));
    }
    for (target,dismount,landed) in contacts {
        state.hit.insert(target,now);
        if dismount { shared.riding.lock().expect("седоки").dismount(target); }
        if landed {
            if state.sound.is_none_or(|before| now.saturating_sub(before)>=10) {
                let sound = if blocks::item_name(stack.item)==Some("wooden_spear") {"minecraft:item.spear_wood.hit"} else {"minecraft:item.spear.hit"};
                shared.world_in(player.dimension).lock().expect("мир").note_sound_player_except(at,sound,1.0,1.0,player.entity_id);
                state.sound=Some(now);
            }
            if player.game_mode != GAME_MODE_CREATIVE && let Some(current) = player.inventory.slot(slot) {
                pvp::wear_slot(shared,player,slot,current,1,false);
            }
            player.progress.used_weapon(stack.item);
            player.vit.add_exhaustion(0.1);
        }
    }
    player.net.spear_charge=state;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(level: i32) -> (Shared, PlayerState) {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(Path::new("tmp/lunge/ops.json")), crate::skins::Settings::default());
        shared.ticks.send_replace(100);
        let mut player = PlayerState::new([72; 16], "LungeTest".into(), None, None, 0, 72, (0.5, 64.0, 0.5));
        player.game_mode = GAME_MODE_SURVIVAL;
        player.on_ground = true;
        player.yaw = 180.0;
        player.pitch = 0.0;
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("lunge").unwrap(), level);
        player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("iron_spear").unwrap(), 1).with_extras(data)));
        (shared, player)
    }

    #[test]
    fn charge_tiers_delays_stages_and_speed_thresholds() {
        for name in ["wooden_spear","golden_spear","stone_spear","copper_spear","iron_spear","diamond_spear","netherite_spear"] {
            let tier = tier(blocks::item_named(name).unwrap()).unwrap();
            assert!(contact(tier,tier.delay-1,20.0,20.0).is_none());
            assert!(contact(tier,tier.delay,4.59,4.59).is_none());
            let hit = contact(tier,tier.delay,4.6,4.6).unwrap();
            assert!(hit.damage > 0.0 && !hit.knockback && !hit.dismount);
            assert!(contact(tier,tier.delay,5.1,0.0).unwrap().knockback);
            let hit = contact(tier,tier.delay,tier.dismount_speed,20.0).unwrap();
            assert!(hit.dismount && hit.knockback);
            let hit = contact(tier,tier.delay+tier.engaged,20.0,20.0).unwrap();
            assert!(!hit.dismount && hit.knockback);
            let hit = contact(tier,tier.delay+tier.engaged+tier.tired,20.0,20.0).unwrap();
            assert!(!hit.dismount && !hit.knockback && hit.damage>0.0);
            assert!(contact(tier,tier.delay+tier.duration,20.0,20.0).is_none());
            assert!(contact(tier,tier.delay,0.0,6.0).unwrap().damage>0.0);
        }
    }

    fn charge_setup(hand: i32) -> (Shared, PlayerState) {
        let (shared,mut player) = setup(0);
        player.yaw = 0.0;
        player.pitch = 10.0;
        if hand==1 { let stack=player.inventory.held();player.inventory.set(inventory::OFFHAND,stack);player.inventory.set(inventory::FIRST_HOTBAR,None); }
        assert!(vitals::start_using(&shared,&mut player,hand,100));
        let mob=crate::mob::Mob::new(73,crate::mob::Kind::Cow,(0.5,64.0,4.0));
        shared.items_in(player.dimension).lock().unwrap().add_mob(mob);
        (shared,player)
    }

    #[test]
    fn holding_use_hits_without_attack_packet_and_does_not_consume_jab_charge() {
        for hand in [0,1] {
            let (shared,mut player)=charge_setup(hand);
            player.last_attack=100;
            for elapsed in 1..=12 {
                shared.ticks.send_replace(100+elapsed);
                player.z+=0.27;
                // Цель всё время перед копьём: проверяем задержку, а не путь робота.
                shared.items_in(player.dimension).lock().unwrap().mobs_mut()[0].z=player.z+3.0;
                tick(&shared,&mut player);
                if elapsed<12 { assert_eq!(shared.items_in(player.dimension).lock().unwrap().mobs()[0].vitals.health,10.0); }
            }
            let health=shared.items_in(player.dimension).lock().unwrap().mobs()[0].vitals.health;
            assert!(health<10.0);
            assert_eq!(player.last_attack,100);
            let slot=if hand==1 {inventory::OFFHAND}else{inventory::FIRST_HOTBAR};
            assert_eq!(player.inventory.slot(slot).unwrap().extras().damage,1);
            shared.ticks.send_replace(113);player.z+=0.27;tick(&shared,&mut player);
            assert_eq!(shared.items_in(player.dimension).lock().unwrap().mobs()[0].vitals.health,health);
            assert_eq!(player.inventory.slot(slot).unwrap().extras().damage,1);
        }
    }

    #[test]
    fn charge_uses_target_velocity_and_stops_at_walls_release_or_changed_weapon() {
        for case in 0..4 {
            let (shared,mut player)=charge_setup(0);
            if case==0 { shared.world_in(player.dimension).lock().unwrap().set_block(0,65,2,blocks::state_by_name("stone").unwrap()); }
            if case==1 { player.net.using=None; }
            if case==2 { player.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("stick").unwrap(),1))); }
            shared.ticks.send_replace(112);
            shared.items_in(player.dimension).lock().unwrap().mobs_mut()[0].vz = -0.3;
            tick(&shared,&mut player);
            let health=shared.items_in(player.dimension).lock().unwrap().mobs()[0].vitals.health;
            assert_eq!(health<10.0,case==3);
        }
    }

    #[tokio::test]
    async fn kinetic_player_hit_applies_damage_without_jab_and_stage_controls_push_and_mount() {
        for (knockback,dismount,damage) in [(true,true,6.0),(false,false,6.0),(true,false,0.0)] {
            let (shared,mut player)=setup(0);
            player.entity_id=72;
            shared.riding.lock().unwrap().mount(72,90,1);
            let strike=combat::PlayerStrike {kinetic:Some(combat::Kinetic {knockback,dismount}),projectile:None,
                victim:72,attacker:73,attacker_uuid:[73;16],attacker_name:"Charger".into(),from:(0.5,64.0,-3.0),yaw:0.0,
                damage,source:crate::survival::Source::Spear,extra_knockback:0.0,weapon:None,disables_shield_ticks:0,
                breach_scale:1.0,fire_ticks:0,ignores_shield:false,thorns_slots:Vec::new(),effects:Vec::new()};
            let mut out=Outgoing::capture();
            pvp::take_strike(&mut out,&shared,&mut player,&strike).await.unwrap();
            assert_eq!(player.vit.health,20.0-damage);
            let mut bytes=out.queued();
            let mut sent=false;
            while !bytes.is_empty() {if read_packet(&mut bytes).await.unwrap().id==0x65 {sent=true;}}
            assert_eq!(player.net.velocity.is_some() || sent,knockback);
            assert_eq!(shared.riding.lock().unwrap().vehicle_of(72).is_none(),dismount);
        }
    }

    #[test]
    fn moving_charge_finds_players_and_publishes_spear_damage() {
        let (shared,mut player)=charge_setup(0);
        let mut victim=PlayerState::new([74;16],"Target".into(),None,None,0,74,(0.5,64.0,4.0));
        victim.entity_id=74;victim.game_mode=GAME_MODE_SURVIVAL;
        shared.players.lock().unwrap().join(victim.member(),74).unwrap();
        publish_player(&shared,&victim);
        let reader=shared.world_in(player.dimension).lock().unwrap().watch_events(72);
        for elapsed in 1..=12 {
            shared.ticks.send_replace(100+elapsed);
            if elapsed==12 { player.z+=0.27; }
            publish_player(&shared,&victim);
            tick(&shared,&mut player);
        }
        let (events,_)=shared.world_in(player.dimension).lock().unwrap().events_since(reader,72);
        let hits:Vec<_>=events.iter().filter_map(|event|if let crate::world::WorldEvent::PlayerStrike(strike)=event {Some(strike)}else{None}).collect();
        assert_eq!(hits.len(),1);
        assert_eq!(hits[0].source,crate::survival::Source::Spear);
        assert_eq!(hits[0].damage,6.0);
        assert!(hits[0].kinetic.unwrap().knockback);
    }

    #[test]
    fn charge_ray_rejects_dead_zone_side_and_behind_targets() {
        let eye=(0.0,1.62,0.0);let dir=(0.0,0.0,1.0);
        assert!(ray_box(eye,dir,(0.0,0.0,3.0),(0.6,1.8),4.5).is_some());
        assert_eq!(ray_box(eye,dir,(0.0,0.0,2.1),(0.6,1.8),4.5),Some(2.0));
        for feet in [(0.0,0.0,1.0),(0.0,0.0,-3.0),(2.0,0.0,3.0),(0.0,0.0,5.0)] {
            assert!(ray_box(eye,dir,feet,(0.6,1.8),4.5).is_none());
        }
    }

    #[test]
    fn measured_levels_boost_wear_and_exhaustion() {
        for (level, horizontal, food_cost) in [(1, -0.458, 4.0), (2, -0.916, 8.0), (3, -1.374, 12.0)] {
            let (shared, mut player) = setup(level);
            player.vit.exhaustion = 1.0;
            player.net.velocity_due = true;
            assert!(jab(&shared, &mut player));
            let velocity = player.net.velocity.unwrap();
            assert!(velocity.0.abs() < 1e-12);
            assert!((velocity.1 + 0.0784).abs() < 1e-9);
            assert!((velocity.2 - horizontal).abs() < 1e-9);
            assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
            assert_eq!(player.vit.exhaustion, 1.0 + food_cost);
            assert_eq!(player.last_attack, 100);
            assert!(!player.net.velocity_due);
        }
    }

    #[test]
    fn measured_pitch_projection_and_airborne_gravity() {
        for (pitch, horizontal) in [(60.0, -0.687), (90.0, 0.0)] {
            let (shared, mut player) = setup(3);
            player.pitch = pitch;
            assert!(jab(&shared, &mut player));
            let velocity = player.net.velocity.unwrap();
            assert!((velocity.2 - horizontal).abs() < 1e-9);
            assert!((velocity.1 + 0.0784).abs() < 1e-9);
        }
        let (shared, mut player) = setup(3);
        player.on_ground = false;
        player.net.last_delta = (0.0, -0.0784, 0.0);
        assert!(jab(&shared, &mut player));
        let velocity = player.net.velocity.unwrap();
        assert!((velocity.1 + 0.155232).abs() < 1e-9);
        assert!((velocity.2 + 1.374).abs() < 1e-9);
    }

    #[test]
    fn measured_cooldown_rejections_do_not_spend_or_restart_it() {
        for level in [1, 3] {
            let (shared, mut player) = setup(level);
            assert!(jab(&shared, &mut player));
            let exhaustion = player.vit.exhaustion;
            for elapsed in 1..14 {
                shared.ticks.send_replace(100 + elapsed);
                assert!(!jab(&shared, &mut player));
                assert_eq!(player.last_attack, 100);
                assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
                assert_eq!(player.vit.exhaustion, exhaustion);
            }
            shared.ticks.send_replace(114);
            assert!(jab(&shared, &mut player));
            assert_eq!(player.inventory.held().unwrap().extras().damage, 2);
        }
        let (shared, mut player) = setup(1);
        player.attrs.set_base(crate::attributes::ATTACK_SPEED, 10.0);
        assert!(jab(&shared, &mut player));
        shared.ticks.send_replace(103);
        assert!(jab(&shared, &mut player));
    }

    #[test]
    fn measured_food_gate_rejects_without_consuming_the_action() {
        let (shared, mut player) = setup(3);
        player.vit.food = 6;
        let before = player.vit.exhaustion;
        assert!(!jab(&shared, &mut player));
        assert_eq!(player.last_attack, 0);
        assert_eq!(player.vit.exhaustion, before);
        assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
        player.vit.food = 7;
        assert!(jab(&shared, &mut player));
    }

    #[test]
    fn creative_and_unbreakable_keep_the_spear() {
        for creative in [false, true] {
            let (shared, mut player) = setup(3);
            let stack = player.inventory.held().unwrap();
            let mut data = stack.extras();
            data.unbreakable = !creative;
            player.inventory.set(inventory::FIRST_HOTBAR, Some(stack.with_extras(data)));
            if creative { player.game_mode = GAME_MODE_CREATIVE; player.vit.food = 0; }
            let before = player.vit.exhaustion;
            assert!(jab(&shared, &mut player));
            assert_eq!(player.inventory.held().unwrap().extras().damage, 0);
            assert_eq!(player.vit.exhaustion, before + if creative { 0.0 } else { 12.0 });
        }
    }

    #[test]
    fn rejects_wrong_item_offhand_mount_and_water_without_side_effects() {
        for case in 0..7 {
            let (shared, mut player) = setup(3);
            match case {
                0 => { let data = player.inventory.held().unwrap().extras(); player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("stick").unwrap(), 1).with_extras(data))); }
                1 => { let stack = player.inventory.held(); player.inventory.set(inventory::OFFHAND, stack); player.inventory.set(inventory::FIRST_HOTBAR, None); }
                2 => player.ride.vehicle = Some(81),
                3 => player.net.in_water = true,
                4 => player.game_mode = GAME_MODE_SPECTATOR,
                5 => player.vit.dead = true,
                _ => player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("iron_spear").unwrap(), 1))),
            }
            let stack = player.inventory.held();
            let exhaustion = player.vit.exhaustion;
            assert!(!jab(&shared, &mut player));
            assert_eq!(player.inventory.held(), stack);
            assert_eq!(player.vit.exhaustion, exhaustion);
            assert!(player.net.velocity.is_none());
        }
    }

    #[test]
    fn player_action_seven_works_far_from_origin_and_does_not_attack_mobs() {
        let (shared, mut player) = setup(3);
        shared.items_in(player.dimension).lock().unwrap().add_mob(crate::mob::Mob::new(73, crate::mob::Kind::Cow, (0.5, 64.0, -1.5)));
        // Status7, BlockPos(0,0,0), face0, sequence9. Нулевая позиция не
        // должна отвергаться проверкой дальности обычного копания.
        let payload = [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];
        let reaction = break_block(&payload, &mut player, &shared).unwrap();
        assert_eq!(reaction.acknowledge, Some(9));
        assert!(player.net.velocity.is_some());
        assert_eq!(player.inventory.held().unwrap().extras().damage, 1);
        assert_eq!(shared.items_in(player.dimension).lock().unwrap().mobs()[0].vitals.health, 10.0);
    }
}
