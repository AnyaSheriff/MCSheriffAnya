// Взаимодействия наземных животных, поверх общих инвентарей и седоков.
// [проверить] Порядок пакетов: сценарии пока не записаны.
use super::*;
use crate::mob::{land, Event, Kind};

pub(super) fn use_on(items: &mut items::Items, index: usize, world: &World, player: &mut PlayerState, shared: &Shared, reaction: &mut Reaction) -> bool {
    let mob = &mut items.mobs_mut()[index];
    if !land::animal(mob.kind) { return false; }
    let reach: f64=if player.game_mode==GAME_MODE_CREATIVE { 6.0 } else { 4.5 };
    if (mob.x-player.x).powi(2)+(mob.y-player.y).powi(2)+(mob.z-player.z).powi(2)>reach*reach { return true; }
    let held = player.inventory.held().and_then(|s| blocks::item_name(s.item));
    let mut rng = crate::mob::Rng::new(shared.next_entity_id() as u64 ^ mob.ticks);
    let mut spend = false;
    let mut output = None;
    let mut wear = false;
    let mut convert = false;
    let mut drops: Option<(i32, (f64, f64, f64))> = None;
    let mut extra=crate::stackdata::Data::default();
    if land::llama(mob.kind) && mob.land.tamed && player.sneaking && !mob.is_baby() {
        reaction.open_cart=Some(mob.entity_id); return true;
    }
    if held == Some("bucket") && !mob.is_baby() && matches!(mob.kind, Kind::Goat | Kind::Mooshroom) {
        output = blocks::item_named("milk_bucket"); spend = true;
        mob.events.push(Event::Sound { name: if mob.kind == Kind::Goat { "minecraft:entity.goat.milk" } else { "minecraft:entity.cow.milk" }, volume: 1.0, pitch: 1.0 });
    } else if mob.kind == Kind::Mooshroom && !mob.is_baby() && held == Some("bowl") {
        if let Some(effect)=land::stew(mob.land.flower) { extra.stew.push(effect); mob.land.flower=0; output=blocks::item_named("suspicious_stew"); }
        else { output = blocks::item_named("mushroom_stew"); } spend = true;
    } else if mob.kind==Kind::Mooshroom && mob.land.brown && !mob.is_baby() && held.and_then(land::flower).is_some() {
        if mob.land.flower==0 { mob.land.flower=held.and_then(land::flower).unwrap(); spend=true; }
        else { mob.events.push(Event::Status(6)); }
    } else if mob.kind == Kind::Mooshroom && !mob.is_baby() && held == Some("shears") {
        let mushroom = if mob.land.brown { "brown_mushroom" } else { "red_mushroom" };
        // Чёрный ящик, mooshroom-shear: звук у сущности (категория «игроки»), затем пять
        // отдельных грибов над спиной (y+1,4), как шерсть овцы; потом корова.
        if let Some(id) = blocks::item_named(mushroom) { drops = Some((id, (mob.x, mob.y + 1.4, mob.z))); }
        mob.events.push(Event::EntitySound { name: "minecraft:entity.mooshroom.shear", category: SOUND_PLAYERS, volume: 1.0, pitch: 1.0 });
        // Новая сущность коровы, сохраняя здоровье, возраст и имя.
        // Номер меняем после рассылки звука: он привязан к грибной корове.
        convert = true; wear = true;
    } else if mob.kind == Kind::Ocelot && !mob.land.trusting && held.is_some_and(|s| matches!(s,"cod"|"salmon")) {
        if (mob.x-player.x).hypot(mob.z-player.z) > 3.0 { return true; }
        spend = true;
        if rng.below(3) == 0 { mob.land.trusting = true; mob.persistent_tag = true; mob.events.push(Event::Status(41)); }
        else { mob.events.push(Event::Status(40)); }
    } else if land::llama(mob.kind) && held == Some("chest") && mob.land.tamed && !mob.land.chest && !mob.is_baby() {
        mob.land.chest = true; mob.land.cargo.resize(mob.land.slots(),None); spend = true;
        mob.events.push(Event::Sound { name: "minecraft:entity.llama.chest", volume: 1.0, pitch: 1.0 });
    } else if land::llama(mob.kind) && mob.land.tamed && held.is_some_and(|s| s.ends_with("_carpet")) {
        let color = held.unwrap().trim_end_matches("_carpet");
        let colors = ["white","orange","magenta","light_blue","yellow","lime","pink","gray","light_gray","cyan","purple","blue","brown","green","red","black"];
        if let Some(c) = colors.iter().position(|s| *s == color) {
            if let Some(old) = mob.land.decor && let Some(id) = blocks::item_named(&format!("{}_carpet",colors[usize::from(old)])) { mob.events.push(Event::Loot(vec![Stack::new(id,1)])); }
            mob.land.decor = Some(c as u8); spend = true; mob.attr_changes = mob.attr_changes.wrapping_add(1);
        }
    } else if mob.kind == Kind::Camel && held == Some("saddle") && !mob.land.saddle && !mob.is_baby() {
        mob.land.saddle = true; spend = true; mob.attr_changes = mob.attr_changes.wrapping_add(1);
    } else if land::llama(mob.kind) && held.is_some_and(|s| matches!(s,"wheat"|"hay_block")) {
        let hay = held == Some("hay_block");
        if mob.is_baby() && !mob.age_locked { mob.age = (mob.age + if hay { 1800 } else { 200 }).min(0); spend = true; }
        if mob.vitals.health < mob.vitals.max_health { mob.vitals.health = (mob.vitals.health + if hay { 10.0 } else { 2.0 }).min(mob.vitals.max_health); spend = true; }
        if !mob.land.tamed && mob.land.temper < 30 { mob.land.temper = (mob.land.temper + if hay { 6 } else { 3 }).min(30); spend = true; }
        if hay && mob.land.tamed && mob.age == 0 && mob.love == 0 { mob.love = crate::mob::LOVE_TICKS; mob.events.push(Event::Hearts); spend = true; }
    } else if mob.kind == Kind::Panda && held.is_some_and(|s| matches!(s,"bamboo"|"cake")) {
        let nearby = land::bamboo_near(&MobWorld(world),mob);
        if mob.is_baby() && !mob.age_locked { mob.age = (mob.age + (-mob.age / 10)).min(0); }
        else if held == Some("bamboo") && mob.age == 0 && nearby { mob.love = crate::mob::LOVE_TICKS; mob.events.push(Event::Hearts); }
        else { mob.land.eating = 32; mob.land.held=player.inventory.held().map(|s| s.with_count(1)); mob.attr_changes=mob.attr_changes.wrapping_add(1); }
        spend = true;
    } else if mob.kind==Kind::Camel && held==Some("cactus") {
        if mob.vitals.health<mob.vitals.max_health { mob.vitals.health=(mob.vitals.health+2.0).min(mob.vitals.max_health); spend=true; }
        if mob.is_baby() && !mob.age_locked { mob.age=(mob.age+(-mob.age/10)).min(0); spend=true; }
        else if mob.age==0 && mob.love==0 { mob.love=crate::mob::LOVE_TICKS; mob.events.push(Event::Hearts); spend=true; }
    } else if mob.kind==Kind::Camel && held==Some("shears") && mob.land.saddle {
        mob.land.saddle=false; mob.attr_changes=mob.attr_changes.wrapping_add(1); wear=true;
        if let Some(id)=blocks::item_named("saddle") { mob.events.push(Event::Loot(vec![Stack::new(id,1)])); }
    } else if land::rideable(mob.kind) && !mob.is_baby() && !player.sneaking && player.ride.vehicle.is_none() && (held.is_none() || mob.land.tamed || mob.kind == Kind::Camel) {
        let capacity = if mob.kind == Kind::Camel { 2 } else { 1 };
        if shared.riding.lock().expect("седоки").mount(player.entity_id,mob.entity_id,capacity) {
            mob.land.rider = shared.riding.lock().expect("седоки").driver(mob.entity_id); mob.land.mount_ticks = 0;
        }
    } else { return false; }
    if spend && player.game_mode != GAME_MODE_CREATIVE && let Some(slot) = player.inventory.spend_held() { reaction.slots.push(slot); }
    if let Some(item) = output {
        let result=Stack::new(item,1).with_extras(extra);
        if player.game_mode!=GAME_MODE_CREATIVE && player.inventory.held().is_none() {
            let slot=player.inventory.selected_slot(); player.inventory.set(slot,Some(result)); reaction.slots.push(slot);
        } else if player.game_mode!=GAME_MODE_CREATIVE || !player.inventory.slots().iter().flatten().any(|s| s.stacks_with(&result)) {
            let (slots,left)=player.inventory.put(result); reaction.slots.extend(slots);
            if let Some(stack)=left { items.drop_item(shared.next_entity_id(),stack,(player.x,player.y+0.5,player.z),(0.0,0.0,0.0),items::MINED_DELAY); }
        }
    }
    if wear && player.game_mode != GAME_MODE_CREATIVE {
        let slot = player.inventory.selected_slot();
        if let Some(stack) = player.inventory.slot(slot) {
            let damage=crate::combat::wear(&stack,1,false,|| rng.unit() as f32);
            player.inventory.set(slot,crate::combat::add_damage(stack,damage)); reaction.slots.push(slot);
        }
    }
    items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);
    if let Some((id, at)) = drops {
        for _ in 0..5 {
            let velocity = (rng.unit() * 0.2 - 0.1, 0.2, rng.unit() * 0.2 - 0.1);
            items.drop_item(shared.next_entity_id(), Stack::new(id, 1), at, velocity, items::MINED_DELAY);
        }
    }
    if convert {
        let mob = &mut items.mobs_mut()[index];
        mob.kind = Kind::Cow; mob.entity_id = shared.next_entity_id(); mob.uuid = crate::mob::uuid_for(mob.entity_id);
        mob.land = land::State::new(Kind::Cow); mob.ticks = 0;
    }
    true
}
struct MobWorld<'a>(&'a World);
impl crate::mob::View for MobWorld<'_> {
    fn catalog(&self) -> crate::gameplay_catalog::Catalog { self.0.catalog() } fn block(&self,x:i32,y:i32,z:i32)->i32 { self.0.get_block(x,y,z) } fn loaded(&self,x:i32,z:i32)->bool { self.0.has_chunk(x,z) } }

// Окно ламы: две ячейки снаряжения, затем 3·Strength грузовых ячеек.
// [проверить] 26.1 horse-screen после переноса седла в отдельный слот.
pub(super) fn slots(mob: &crate::mob::Mob) -> Vec<Option<Stack>> {
    let decor=mob.land.decor.and_then(|c| blocks::item_named(&crate::mob::WOOL[usize::from(c)].replace("_wool","_carpet"))).map(|id| Stack::new(id,1));
    let mut slots=vec![None,decor];
    slots.extend((0..mob.land.slots()).map(|i| mob.land.cargo.get(i).copied().flatten())); slots
}
pub(super) fn valid(mob:&crate::mob::Mob, slots:&[Option<Stack>])->bool {
    slots.len()==mob.land.slots()+2 && slots[0].is_none() && slots[1].is_none_or(|s| s.count==1 && blocks::item_name(s.item).is_some_and(|n| n.ends_with("_carpet")))
}
pub(super) fn store(mob:&mut crate::mob::Mob, slots:&[Option<Stack>]) {
    mob.land.decor=slots[1].and_then(|s| blocks::item_name(s.item)).and_then(|name| crate::mob::WOOL.iter().position(|w| w.replace("_wool","_carpet")==name)).map(|i| i as u8);
    mob.land.cargo=slots[2..].to_vec(); mob.attr_changes=mob.attr_changes.wrapping_add(1);
}

// Shift: груз идёт в грузовые ячейки, а ковёр — в единственную ячейку декора.
pub(super) fn quick_move(slots:&mut [Option<Stack>], inventory:&mut Inventory, slot:i32)->inventory::Click {
    let n=slots.len();
    let mut window=window::Window { container:slots,inventory,kind:None,horse:None };
    let Ok(slot)=usize::try_from(slot) else { return inventory::Click::default(); };
    if slot>=window.len() { return inventory::Click::default(); }
    if slot<n { window.move_some(slot,&(n..n+window::PLAYER_SLOTS).rev().collect::<Vec<_>>()); }
    else if let Some(stack)=window.get(slot) {
        if blocks::item_name(stack.item).is_some_and(|s| s.ends_with("_carpet")) && window.get(1).is_none() {
            window.set(1,Some(stack.with_count(1))); window.set(slot,inventory::remainder(stack,stack.count-1));
        } else { window.move_some(slot,&(2..n).collect::<Vec<_>>()); }
    }
    inventory::Click { understood:true,thrown:None }
}
