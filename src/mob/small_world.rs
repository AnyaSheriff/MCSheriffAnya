// Связь малых мобов с предметами, музыкой и блоками. По minecraft.wiki,
// Allay, Jukebox, Note Block, Sniffer Egg. [проверить] Порядок тактов.
use super::{small, Kind, Mob, Rng, ai::Watcher};
use crate::{blocks, items::Items, shared::Shared, world::World, dimension::Dimension};

pub fn schedule_egg(world: &mut World, (x,y,z): (i32,i32,i32)) {
    // Три стадии: всего 10 минут на мху, 20 на прочем основании.
    let moss = blocks::block_at_state(world.get_block(x,y-1,z)) == Some("moss_block");
    let delay = if moss { 4000 } else { 8000 };
    world.schedule_block(x,y,z,delay,0);
}
pub fn hatch_egg(world: &mut World, (x,y,z): (i32,i32,i32)) -> bool {
    let state = world.get_block(x,y,z);
    if blocks::block_at_state(state) != Some("sniffer_egg") { return false; }
    let Some(o) = blocks::orientation_of(state) else { return true; };
    let hatch = o.value(state,"hatch").and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    if hatch < 2 {
        if let Some(next) = o.with(state,"hatch", &(hatch+1).to_string()) { world.set_block(x,y,z,next); }
        world.note_sound(x,y,z,"minecraft:block.sniffer_egg.crack",1.0,1.0);
        schedule_egg(world,(x,y,z));
    } else {
        world.set_block(x,y,z,0);
        world.note_sound(x,y,z,"minecraft:block.sniffer_egg.hatch",1.0,1.0);
        world.snifflet_births.push((x,y,z));
    }
    true
}

pub fn step(items: &mut Items, world: &mut World, players: &[Watcher], shared: &Shared, _dimension: Dimension, rng: &mut Rng) {
    for (x,y,z) in std::mem::take(&mut world.snifflet_births) {
        let mut baby = Mob::new(shared.next_entity_id(),Kind::Sniffer,(x as f64+0.5,y as f64,z as f64+0.5));
        baby.age = small::SNIFFLET_AGE;
        items.add_mob(baby);
    }
    let notes = std::mem::take(&mut world.allay_notes);
    let mut mobs = std::mem::take(items.mobs_vec());
    for mob in &mut mobs {
        if mob.is_dead() || mob.no_ai || !world.has_chunk(mob.chunk().0,mob.chunk().1) { continue; }
        if mob.brain.is_panicking() { mob.small.dancing=false; continue; }
        if matches!(mob.kind,Kind::Parrot|Kind::Allay) && mob.ticks.is_multiple_of(20) {
            mob.small.dancing = dancing_near(world,mob);
        }
        if mob.kind != Kind::Allay { continue; }
        hear_notes(mob, world, &notes);
        collect(mob, items, players, shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed), &mut || shared.next_entity_id(), rng);
    }
    *items.mobs_vec()=mobs;
    items.flush_mob_events(&mut || shared.next_entity_id(),rng);
}

fn dancing_near(world: &World, mob: &Mob) -> bool {
    let (x,y,z) = mob.cell();
    let radius: i32 = if mob.kind == Kind::Allay { 10 } else { 3 };
    (-radius..=radius).any(|dx| (-radius..=radius).any(|dy| (-radius..=radius).any(|dz|
        dx*dx+dy*dy+dz*dz <= radius*radius && crate::redstone::jukebox_playing(world,(x+dx,y+dy,z+dz)))))
}

// Вики, Allay: «30 seconds», «32 blocks», «64 blocks», «3 seconds».
fn hear_notes(mob: &mut Mob, world: &mut World, notes: &[(i32,i32,i32)]) {
    if mob.small.held.is_none() { return; }
    if let Some((note, left)) = mob.small.pending_note.take() {
        if left > 1 { mob.small.pending_note = Some((note,left-1)); }
        else { mob.small.note = Some(note); mob.small.note_ticks = 600; }
    }
    if mob.small.pending_note.is_some() { return; }
    let at = (mob.x,mob.y+mob.eye(),mob.z);
    let nearest = notes.iter().copied().filter(|note| mob.small.note.is_none_or(|old| old == *note))
        .map(|note| (note,small::distance(at,(note.0 as f64+0.5,note.1 as f64+0.5,note.2 as f64+0.5))))
        .filter(|(note,d)| *d <= 256.0 && hears(world,*note,at))
        .min_by(|a,b| a.1.total_cmp(&b.1));
    if let Some((note,d)) = nearest {
        let ticks = (d.sqrt() as i32).max(1);
        mob.small.pending_note = Some((note,ticks));
        // Чёрный ящик allay-collect: услышав ноту, оригинал рисует частицу вибрации от блока к тихоне.
        world.note_vibration(note, mob.cell(), ticks);
    }
}

// [проверить] Луч через шерсть: пока один луч между центрами, без шести
// смещённых лучей датчика. Сплошные блоки кроме шерсти звук не гасят.
fn hears(world: &World, from: (i32,i32,i32), to: (f64,f64,f64)) -> bool {
    let from = (from.0 as f64+0.5,from.1 as f64+0.5,from.2 as f64+0.5);
    let steps = (small::distance(from,to).sqrt()*8.0).ceil().max(1.0) as i32;
    (0..=steps).all(|i| {
        let t = i as f64/steps as f64;
        let p = ((from.0+(to.0-from.0)*t).floor() as i32,(from.1+(to.1-from.1)*t).floor() as i32,(from.2+(to.2-from.2)*t).floor() as i32);
        !blocks::block_at_state(world.get_block(p.0,p.1,p.2)).is_some_and(|name| name.ends_with("_wool"))
    })
}

fn collect(mob: &mut Mob, items: &mut Items, players: &[Watcher], griefing: bool, next_id: &mut dyn FnMut()->i32, rng: &mut Rng) {
    let at = (mob.x,mob.y,mob.z);
    if mob.small.collecting { mob.small.goal = None; }
    mob.small.collecting = false;
    let owner = players.iter().find(|p| p.id == mob.tame.owner_id && !p.spectator
        && small::distance(at,(p.x,p.y,p.z)) <= 64.0*64.0);
    let destination = mob.small.note.map(|p| (p.0 as f64+0.5,p.1 as f64+1.0,p.2 as f64+0.5))
        .or_else(|| owner.map(|p| (p.x,p.y+1.0,p.z)));
    // Без поручения сохраняем прогулку: её цель обновляет flying AI.
    if let Some(home) = destination { mob.small.goal = Some(home); mob.small.collecting = true; }
    let Some(held) = mob.small.held else { return; };
    if let (Some(cargo),Some(home)) = (mob.small.cargo,destination)
        && small::distance(at,home) < 9.0 && !mob.small.dancing
    {
        let dx=home.0-mob.x; let dy=home.1-mob.y; let dz=home.2-mob.z;
        let length=(dx*dx+dy*dy+dz*dz).sqrt().max(0.1);
        // Чёрный ящик allay-note: бросает по одному предмету за такт, звук один раз
        // (громкость 1, высота k/16 из 0,5…1).
        let one = crate::inventory::Stack { count:1,..cargo };
        items.drop_item(next_id(),one,(mob.x,mob.y+0.3,mob.z),(dx/length*0.2,dy/length*0.2+0.1,dz/length*0.2),10);
        let left = cargo.count-1;
        mob.small.cargo = (left > 0).then_some(crate::inventory::Stack { count:left,..cargo });
        if !mob.small.throwing {
            mob.small.throwing = true;
            mob.events.push(super::Event::Sound { name:"minecraft:entity.allay.item_thrown",volume:1.0,pitch:(8+rng.below(9)) as f32/16.0 });
        }
        if left == 0 { mob.small.pickup_wait=60; mob.small.throwing=false; }
    } else if mob.small.pickup_wait == 0 && griefing && !mob.small.dancing
        && mob.small.cargo.is_none_or(|cargo| cargo.count < cargo.limit())
    {
        // Радиус поиска отсчитывается от самого тихони, не от хозяина.
        let target = items.items().iter().filter(|i| i.block.is_none() && i.stack.item == held.item
            && small::distance(at,(i.x,i.y,i.z)) <= 32.0*32.0
            && mob.small.cargo.is_none_or(|cargo| cargo.stacks_with(&i.stack)))
            .min_by(|a,b| small::distance(at,(a.x,a.y,a.z)).total_cmp(&small::distance(at,(b.x,b.y,b.z))))
            .map(|i| (i.x,i.y,i.z));
        if let Some(target) = target {
            mob.small.goal=Some(target); mob.small.collecting = true;
            items.take_near(at,mob.entity_id,|stack| {
                if stack.item != held.item { return 0; }
                let current = mob.small.cargo.get_or_insert(crate::inventory::Stack { count:0,..stack });
                if !current.stacks_with(&stack) { return 0; }
                let take = stack.count.min(current.limit()-current.count).max(0);
                current.count += take; take
            });
        }
    }
}

/// [проверить] Планировщик попыток использует общий такт, локальная ёмкость 15.
pub fn spawn_bats(items: &mut Items, world: &World, players: &[Watcher], shared: &Shared, rng: &mut Rng) {
    let count=items.mobs().iter().filter(|m| m.kind == Kind::Bat && !m.is_dead()).count();
    if count >= 15 { return; }
    let holiday=super::spawn::bat_holiday();
    for p in players.iter().filter(|p| !p.spectator).take(15-count) {
        let (x,y,z)=(p.x.floor() as i32+rng.below(97) as i32-48,p.y.floor() as i32+rng.below(65) as i32-32,p.z.floor() as i32+rng.below(97) as i32-48);
        if !world.has_chunk(x.div_euclid(16),z.div_euclid(16)) || !(-63..319).contains(&y) || world.open_sky(x,y,z)
            || matches!(world.mob_biome(x,y,z),"deep_dark"|"mushroom_fields")
            || players.iter().any(|p| small::distance((p.x,p.y,p.z),(x as f64,y as f64,z as f64)) < 576.0) { continue; }
        if !matches!(blocks::block_at_state(world.get_block(x,y-1,z)),Some("stone"|"granite"|"diorite"|"andesite"|"tuff"|"deepslate"))
            || !bat_space(world.get_block(x,y,z),world.get_block(x,y+1,z)) { continue; }
        let Some((sky,block))=world.light_cached(x,y,z) else { continue; };
        if !super::spawn::bat_light(sky.max(block),holiday,rng) { continue; }
        let mut bat=Mob::new(shared.next_entity_id(),Kind::Bat,(x as f64+0.5,y as f64,z as f64+0.5));
        bat.persistent=false; items.add_mob(bat);
    }
}

// Вики, Bat, Spawning: «at least 1.5 air blocks». Верхняя полуплита
// или закрытый верхний люк оставляют нижнюю половину свободной.
fn bat_space(feet: i32, above: i32) -> bool {
    let air=|state| matches!(blocks::block_at_state(state),Some("air"|"cave_air"|"void_air"));
    if !air(feet) { return false; }
    if air(above) { return true; }
    let name=blocks::block_at_state(above).unwrap_or("");
    let Some(o)=blocks::orientation_of(above) else { return false; };
    if o.value(above,"waterlogged")==Some("true") { return false; }
    (name.ends_with("_slab") && o.value(above,"type")==Some("top"))
        || (name.ends_with("_trapdoor") && o.value(above,"half")==Some("top") && o.value(above,"open")==Some("false"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eggs_crack_twice_hatch_once_and_broken_eggs_do_not_hatch() {
        let mut world=World::in_memory();
        world.set_block(0,0,0,blocks::state_by_name("moss_block").unwrap());
        world.set_block(0,1,0,blocks::state_by_name("sniffer_egg").unwrap());
        assert!(hatch_egg(&mut world,(0,1,0)));
        assert!(world.snifflet_births.is_empty());
        assert!(hatch_egg(&mut world,(0,1,0))); assert!(world.snifflet_births.is_empty());
        assert!(hatch_egg(&mut world,(0,1,0))); assert_eq!(world.snifflet_births,vec![(0,1,0)]);
        assert!(!hatch_egg(&mut world,(0,1,0))); assert_eq!(world.snifflet_births.len(),1);
    }
    fn allay() -> Mob {
        let mut mob=Mob::new(1,Kind::Allay,(0.5,1.0,0.5));
        mob.small.held=Some(crate::inventory::Stack::new(blocks::item_named("diamond").unwrap(),1));
        mob
    }
    fn pickup(items: &mut Items) {
        items.note_tick_start();
        items.step(&|_,_,_|false,&|_,_,_|false,&|_,_|true);
    }
    #[test]
    fn bat_spawn_needs_one_and_a_half_blocks_of_clearance() {
        assert!(bat_space(0,0));
        assert!(bat_space(blocks::state_by_name("cave_air").unwrap(),0));
        assert!(!bat_space(0,blocks::state_by_name("stone").unwrap()));
        let slab=blocks::state_by_name("stone_slab").unwrap();
        let o=blocks::orientation_of(slab).unwrap();
        assert!(bat_space(0,o.with(slab,"type","top").unwrap()));
        assert!(!bat_space(0,o.with(slab,"type","bottom").unwrap()));
        assert!(!bat_space(blocks::state_by_name("water").unwrap(),0));
    }
    #[test]
    fn allay_collects_matching_items_only_and_respects_stack_limit() {
        let mut mob=allay(); let mut items=Items::new();
        let diamond=mob.small.held.unwrap();
        items.drop_item(2,crate::inventory::Stack { count:40,..diamond },(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        items.drop_item(3,crate::inventory::Stack { count:40,..diamond },(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        items.drop_item(4,crate::inventory::Stack::new(blocks::item_named("dirt").unwrap(),2),(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        pickup(&mut items);
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1));
        assert_eq!(mob.small.cargo.unwrap().count,64);
        assert_eq!(items.items().iter().filter(|i| i.stack.item==diamond.item).map(|i|i.stack.count).sum::<i32>(),16);
        assert!(items.items().iter().any(|i|blocks::item_name(i.stack.item)==Some("dirt") && i.stack.count==2));
        assert_eq!(mob.small.held,Some(diamond));
    }
    #[test]
    fn allay_delivery_throws_cargo_and_waits_three_seconds() {
        let mut mob=allay(); let mut items=Items::new(); let mut rng=Rng::new(1);
        mob.small.note=Some((1,0,0)); mob.small.note_ticks=600;
        mob.small.cargo=Some(crate::inventory::Stack { count:3,..mob.small.held.unwrap() });
        // Бросает по одному предмету за такт (чёрный ящик allay-collect).
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1));
        assert_eq!(mob.small.cargo.unwrap().count,2); assert_eq!(mob.small.pickup_wait,0);
        collect(&mut mob,&mut items,&[],true,&mut ||101,&mut Rng::new(1));
        collect(&mut mob,&mut items,&[],true,&mut ||102,&mut Rng::new(1));
        assert!(mob.small.cargo.is_none()); assert_eq!(mob.small.pickup_wait,60);
        assert_eq!(items.items().len(),3); assert!(items.items().iter().all(|i| i.stack.count==1));
        pickup(&mut items);
        for _ in 0..59 {
            small::tick(&mut mob,&mut rng);
            collect(&mut mob,&mut items,&[],true,&mut ||101,&mut Rng::new(1));
            assert!(mob.small.cargo.is_none());
        }
        small::tick(&mut mob,&mut rng); assert_eq!(mob.small.pickup_wait,0);
    }
    #[test]
    fn allay_matches_item_type_but_keeps_different_components_in_separate_trips() {
        let mut mob=allay(); let mut items=Items::new();
        let plain=mob.small.held.unwrap();
        mob.small.held=Some(plain.with_extras(crate::stackdata::Data { name:Some("sample".into()),..Default::default() }));
        let named=plain.with_extras(crate::stackdata::Data { name:Some("cargo".into()),..Default::default() });
        items.drop_item(2,named,(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        items.drop_item(3,plain,(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        pickup(&mut items);
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1));
        assert_eq!(mob.small.cargo,Some(named));
        assert_eq!(items.items().len(),1); assert_eq!(items.items()[0].stack,plain);
        mob.small.note=Some((1,0,0));
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1));
        assert!(mob.small.cargo.is_none());
        for _ in 0..60 { small::tick(&mut mob,&mut Rng::new(1)); }
        collect(&mut mob,&mut items,&[],true,&mut ||101,&mut Rng::new(1));
        assert_eq!(mob.small.cargo,Some(plain));
    }
    #[test]
    fn allay_griefing_and_dancing_block_pickup() {
        let mut mob=allay(); let mut items=Items::new();
        items.drop_item(2,mob.small.held.unwrap(),(0.5,1.0,0.5),(0.0,0.0,0.0),0); pickup(&mut items);
        collect(&mut mob,&mut items,&[],false,&mut ||100,&mut Rng::new(1)); assert!(mob.small.cargo.is_none());
        mob.small.dancing=true;
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1)); assert!(mob.small.cargo.is_none());
        mob.small.dancing=false;
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1)); assert!(mob.small.cargo.is_some());
    }
    #[test]
    fn allay_note_travels_expires_and_wool_blocks_it() {
        let mut world=World::in_memory(); let mut mob=allay();
        let note=(5,1,0);
        world.set_block(3,1,0,blocks::state_by_name("white_wool").unwrap());
        hear_notes(&mut mob,&mut world,&[note]); assert!(mob.small.pending_note.is_none());
        world.set_block(3,1,0,0);
        hear_notes(&mut mob,&mut world,&[note]); assert!(mob.small.note.is_none());
        let ticks=mob.small.pending_note.unwrap().1;
        for _ in 0..ticks { hear_notes(&mut mob,&mut world,&[]); }
        assert_eq!(mob.small.note,Some(note)); assert_eq!(mob.small.note_ticks,600);
        hear_notes(&mut mob,&mut world,&[(2,1,0)]); assert!(mob.small.pending_note.is_none());
        for _ in 0..600 { small::tick(&mut mob,&mut Rng::new(1)); }
        assert!(mob.small.note.is_none());
    }
    #[test]
    fn allay_without_a_job_keeps_wandering_and_expired_job_clears_goal() {
        let mut mob=allay(); let mut items=Items::new();
        mob.small.goal=Some((2.0,2.0,2.0));
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1));
        assert_eq!(mob.small.goal,Some((2.0,2.0,2.0))); assert!(!mob.small.collecting);
        mob.small.note=Some((10,1,0));
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1)); assert!(mob.small.collecting);
        mob.small.note=None;
        collect(&mut mob,&mut items,&[],true,&mut ||100,&mut Rng::new(1)); assert!(mob.small.goal.is_none());
    }
    #[test]
    fn moss_halves_the_egg_hatching_schedule() {
        for (ground,total) in [("moss_block",12000),("stone",24000)] {
            let mut world=World::in_memory();
            world.set_block(0,0,0,blocks::state_by_name(ground).unwrap());
            world.set_block(0,1,0,blocks::state_by_name("sniffer_egg").unwrap());
            for tick in 1..=total {
                for (pos,kind) in world.advance(100) {
                    if kind == crate::world::TickKind::Block { crate::redstone::ticked(&mut world,pos); }
                }
                if tick < total { assert!(world.snifflet_births.is_empty()); }
            }
            assert_eq!(world.snifflet_births,vec![(0,1,0)]);
            assert_eq!(world.get_block(0,1,0),0);
        }
    }

    #[test]
    fn music_stopping_or_moving_out_of_range_stops_the_dance() {
        let mut world=World::in_memory();
        world.set_block(0,1,0,blocks::state_by_name("jukebox").unwrap());
        world.with_container(0,1,0,|c| c.started=1).unwrap();
        let mut mob=allay();
        assert!(dancing_near(&world,&mob));
        mob.x=10.5; assert!(dancing_near(&world,&mob));
        mob.x=11.5; assert!(!dancing_near(&world,&mob));
        mob.kind=Kind::Parrot; mob.x=3.5; assert!(dancing_near(&world,&mob));
        mob.x=4.5; assert!(!dancing_near(&world,&mob));
        mob.x=0.5; world.with_container(0,1,0,|c| c.started=0);
        assert!(!dancing_near(&world,&mob));
    }

}
