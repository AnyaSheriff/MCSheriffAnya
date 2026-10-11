// Сердце, высохший гаст и сортировка меди (minecraft.wiki).
// [проверить] порядок обновлений и анимаций: сценарии приготовлены, не запускались.
use super::{newmobs, Event, Kind, Mob, Rng};
use crate::{blocks, containers, items::Items, world::World};
type Pos = (i32,i32,i32);

fn prop(state: i32, key: &str) -> Option<&'static str> {
    let name = blocks::block_at_state(state)?;
    blocks::orientation(name)?.value(state,key)
}
fn change(world: &mut World, p: Pos, key: &str, value: &str) {
    let old = world.get_block(p.0,p.1,p.2);
    if let Some(state) = blocks::block_at_state(old).and_then(blocks::orientation).and_then(|b| b.with(old,key,value)) {
        world.set_block(p.0,p.1,p.2,state);
    }
}

pub fn heart_supported(world: &World, p: Pos) -> bool {
    let axis = prop(world.get_block(p.0,p.1,p.2),"axis").unwrap_or("y");
    let d = match axis { "x" => (1,0,0), "z" => (0,0,1), _ => (0,1,0) };
    [-1,1].iter().all(|s| {
        let state = world.get_block(p.0+d.0*s,p.1+d.1*s,p.2+d.2*s);
        matches!(blocks::block_at_state(state),Some("pale_oak_log" | "pale_oak_wood" | "stripped_pale_oak_log" | "stripped_pale_oak_wood")) && prop(state,"axis") == Some(axis)
    })
}

pub fn blocks_tick(world: &mut World, items: &mut Items, players: &[super::ai::Watcher], spawning: bool, rng: &mut Rng, next_id: &mut dyn FnMut()->i32) {
    let places: Vec<_> = world.redstone.mob_blocks.iter().copied().collect();
    let now = world.tick();
    let night = (12600..23401).contains(&world.time_of_day()) && !world.is_nether() && !world.is_end();
    for p in places {
        if !world.has_chunk(p.0.div_euclid(16),p.2.div_euclid(16)) { continue; }
        let state = world.get_block(p.0,p.1,p.2);
        match blocks::block_at_state(state) {
            Some("dried_ghast") => {
                let level: u8 = prop(state,"hydration").and_then(|s|s.parse().ok()).unwrap_or(0);
                let wet = prop(state,"waterlogged") == Some("true");
                // Вики: случайный такт запускает задержку 5000 тактов.
                // Общая очередь мира сохраняется на диске.
                if (wet || level > 0) && rng.below(4096) < crate::farming::random_tick_speed() {
                    world.schedule_block_once(p.0,p.1,p.2,5000);
                }
            }
            Some("creaking_heart") => {
                let linked = items.mobs().iter().any(|m| m.newmobs.heart == Some(p) && !m.is_dead() && !m.h.despawn);
                let supported = linked || heart_supported(world,p);
                let status = if !supported { "uprooted" } else if night { "awake" } else { "dormant" };
                if prop(state,"creaking_heart_state") != Some(status) { change(world,p,"creaking_heart_state",status); }
                if !night {
                    for m in items.mobs_mut().iter_mut().filter(|m|m.newmobs.heart == Some(p)) { m.h.despawn = true; }
                } else if supported && spawning && !linked && now.is_multiple_of(20) && players.iter().any(|w| w.targetable && (w.x-f64::from(p.0)).powi(2)+(w.y-f64::from(p.1)).powi(2)+(w.z-f64::from(p.2)).powi(2) < 32.0*32.0) {
                    for _ in 0..5 {
                        let at = (p.0+rng.below(33) as i32-16,p.1+rng.below(17) as i32-8,p.2+rng.below(33) as i32-16);
                        let at = super::ai::settle(&crate::tick::MobView(world),at,p.1);
                        if super::path::standable(&crate::tick::MobView(world),at) && (0..3).all(|dy| !super::solid(world.get_block(at.0,at.1+dy,at.2))) {
                            let mut mob = Mob::new(next_id(),Kind::Creaking,(f64::from(at.0)+0.5,f64::from(at.1),f64::from(at.2)+0.5));
                            if !super::spawn::fits(&crate::tick::MobView(world),&mob) {continue;}
                            mob.newmobs.heart=Some(p); items.add_mob(mob); break;
                        }
                    }
                }
            }
            _ => { world.redstone.mob_blocks.remove(&p); }
        }
    }
    for mob in items.mobs_mut().iter_mut().filter(|m| m.kind == Kind::Creaking && !m.is_dead()) {
        if let Some(p) = mob.newmobs.heart {
            if (mob.x-f64::from(p.0)-0.5).powi(2)+(mob.y-f64::from(p.1)-0.5).powi(2)+(mob.z-f64::from(p.2)-0.5).powi(2) > 32.0*32.0 { mob.h.despawn=true; }
            if mob.newmobs.resin_hit && mob.newmobs.resin_wait == 0 && night {
                resin(world,p,rng);
                let from=(mob.x,mob.y+mob.eye(),mob.z);
                let to=(f64::from(p.0)+0.5,f64::from(p.1)+0.5,f64::from(p.2)+0.5);
                world.note_trail(from,to,0x5f5f5f,20);
                world.note_trail(to,from,0xffa000,20);
                mob.newmobs.resin_wait=100;
            }
            mob.newmobs.resin_hit=false;
        }
    }
}

fn resin(world: &mut World, heart: Pos, rng: &mut Rng) {
    // Вики: 1–3 сгустка на соседних бледных брёвнах, пауза 5 секунд.
    // [проверить] распределение по связным брёвнам и частицы trail.
    let faces = [((1,0,0),"west"),((-1,0,0),"east"),((0,1,0),"down"),((0,-1,0),"up"),((0,0,1),"north"),((0,0,-1),"south")];
    let mut left=1+rng.below(3);
    for _ in 0..64 {
        let p=(heart.0+rng.below(5) as i32-2,heart.1+rng.below(5) as i32-2,heart.2+rng.below(5) as i32-2);
        if !matches!(blocks::block_at_state(world.get_block(p.0,p.1,p.2)),Some("pale_oak_log"|"pale_oak_wood"|"stripped_pale_oak_log"|"stripped_pale_oak_wood")) { continue; }
        let (d,face)=faces[rng.below(6) as usize]; let at=(p.0+d.0,p.1+d.1,p.2+d.2);
        let old=world.get_block(at.0,at.1,at.2);
        let base=match blocks::block_at_state(old) { Some("air")=>blocks::state_by_name("resin_clump").unwrap(),Some("resin_clump")=>old,_=>continue };
        if prop(base,face)==Some("true") {continue;}
        if let Some(state)=blocks::orientation("resin_clump").and_then(|b|b.with(base,face,"true")) {
            world.set_block(at.0,at.1,at.2,state);left-=1;if left==0 {break;}
        }
    }
}

/// Перед общим тактом: цель сортировщика и перенос предметов через хранилища мира.
pub fn copper_tick(world: &mut World, items: &mut Items, rng: &mut Rng) {
    let places=world.containers_in_order();
    let now=world.tick() as i64;
    for mob in items.mobs_mut().iter_mut().filter(|m|m.kind==Kind::CopperGolem) {
        if mob.is_dead() || mob.h.despawn {
            if let Some(p)=mob.newmobs.chest.take() && mob.newmobs.chest_wait>0 {containers::close(world,&containers::halves(world,p));}
            continue;
        }
        if !world.has_chunk(mob.chunk().0,mob.chunk().1) {continue;}
        if !mob.newmobs.waxed {
            if mob.newmobs.oxidize_at==0 { mob.newmobs.oxidize_at=now+504000+i64::from(rng.below(48001)); }
            if mob.newmobs.oxidation<3 && now>=mob.newmobs.oxidize_at {mob.newmobs.oxidation+=1;mob.newmobs.oxidize_at=0;}
            if mob.newmobs.oxidation==3 && rng.unit()<0.0058 {
                let p=mob.cell();
                if statue_space(world,mob) {
                    let base=blocks::state_by_name("oxidized_copper_golem_statue").unwrap();
                    let poses=["standing","sitting","running","star"];
                    let facing=["south","west","north","east"][((mob.yaw/90.0).round() as i32).rem_euclid(4) as usize];
                    let b=blocks::orientation("oxidized_copper_golem_statue").unwrap();
                    let state=b.with(b.with(base,"facing",facing).unwrap(),"copper_golem_pose",poses[rng.below(4) as usize]).unwrap();
                    if let Some(chest)=mob.newmobs.chest.take() && mob.newmobs.chest_wait>0 {containers::close(world,&containers::halves(world,chest));}
                    world.set_block(p.0,p.1,p.2,state);
                    if let Some(stack)=mob.newmobs.cargo.take(){mob.events.push(Event::Loot(vec![stack]));}
                    world.note_sound(p.0,p.1,p.2,"minecraft:entity.copper_golem_become_statue",1.0,1.0);
                    mob.h.despawn=true;continue;
                }
            }
        }
        if mob.no_ai {continue;}
        if mob.newmobs.chest_wait<0 {mob.newmobs.chest_wait+=1;continue;}
        if mob.newmobs.chest.is_none() {
            let source=mob.newmobs.cargo.is_none();
            mob.newmobs.chest=places.iter().copied().filter(|p| !mob.newmobs.visited.contains(p) && (p.0-mob.cell().0).abs()<=32 && (p.1-mob.cell().1).abs()<=8 && (p.2-mob.cell().2).abs()<=32)
                .filter(|p|world.container(p.0,p.1,p.2).is_some_and(|c| if source {c.kind==containers::Kind::CopperChest} else {matches!(c.kind,containers::Kind::Chest|containers::Kind::TrappedChest)}))
                .min_by_key(|p|(p.0-mob.cell().0).pow(2)+(p.1-mob.cell().1).pow(2)+(p.2-mob.cell().2).pow(2));
        }
        let Some(p)=mob.newmobs.chest else {mob.newmobs.visited.clear();mob.newmobs.chest_wait = -140;continue;};
        let halves=containers::halves(world,p);
        let close=(mob.x-f64::from(p.0)-0.5).hypot(mob.z-f64::from(p.2)-0.5)<1.8 && (p.1-mob.cell().1).abs()<=1;
        if !close {
            let goal=super::ai::settle(&crate::tick::MobView(world),(p.0-1,p.1,p.2),mob.cell().1);
            newmobs::walk_to(mob,&crate::tick::MobView(world),goal);
            if !mob.brain.walking() {finish_chest(mob,p,false);}
            continue;
        }
        mob.brain.stop();
        if !containers::can_open(world,&halves) {
            if mob.newmobs.chest_wait>0 {containers::close(world,&halves);}
            finish_chest(mob,p,false);continue;
        }
        if mob.newmobs.chest_wait==0 {let first=containers::open_begin(world,&halves);containers::open_finish(world,&halves,first);}
        mob.newmobs.chest_wait+=1;
        // Через 8 тактов после открытия — звук: «не взял» у источника, «бросил» у сундука (copper-golem-sort).
        if mob.newmobs.chest_wait==9 {mob.events.push(Event::Sound{name:if mob.newmobs.cargo.is_some() {"minecraft:entity.copper_golem.item_drop"} else {"minecraft:entity.copper_golem.no_item_get"},volume:1.0,pitch:1.0});}
        if mob.newmobs.chest_wait<60 {continue;}
        let was_carrying=mob.newmobs.cargo.is_some();
        if let Some(stack)=mob.newmobs.cargo {
            let accepts=halves.iter().filter_map(|p|world.container(p.0,p.1,p.2)).all(|c|c.slots.iter().all(Option::is_none)) || halves.iter().filter_map(|p|world.container(p.0,p.1,p.2)).any(|c|c.slots.iter().flatten().any(|s|s.item==stack.item));
            if accepts {mob.newmobs.cargo=containers::insert_stack(world,&halves,stack,(0,0,0));}
        } else {
            for p in &halves {
                let taken=world.with_container(p.0,p.1,p.2,|c| {
                    let slot=c.slots.iter().position(Option::is_some)?;
                    let stack=c.slots[slot]?;let count=stack.count.min(16);
                    c.slots[slot]=crate::inventory::remainder(stack,stack.count-count);Some(stack.with_count(count))
                }).flatten();
                if taken.is_some(){mob.newmobs.cargo=taken;break;}
            }
        }
        containers::close(world,&halves);
        finish_chest(mob,p,was_carrying!=mob.newmobs.cargo.is_some());
    }
}

fn statue_space(world: &World, mob: &Mob) -> bool {
    let p=mob.cell();
    if !mob.on_ground || (mob.y-f64::from(p.1)).abs()>0.001 || !(crate::world::MIN_Y..crate::world::MIN_Y+crate::world::WORLD_HEIGHT).contains(&p.1) {return false;}
    let radius=mob.size().0/2.0;
    for x in (mob.x-radius).floor() as i32..=(mob.x+radius).floor() as i32 {
        for z in (mob.z-radius).floor() as i32..=(mob.z+radius).floor() as i32 {
            if !matches!(blocks::block_at_state(world.get_block(x,p.1,z)),Some("air"|"cave_air"|"void_air")) {return false;}
        }
    }
    true
}

fn finish_chest(mob: &mut Mob, p: Pos, transferred: bool) {
    mob.newmobs.chest=None;mob.newmobs.chest_wait=0;
    if transferred {mob.newmobs.visited.clear();} else {mob.newmobs.visited.push(p);}
    if mob.newmobs.visited.len()>=10 {mob.newmobs.visited.clear();mob.newmobs.chest_wait = -140;}
}

/// Очередной этап высыхания или увлажнения, из общей очереди блочных тактов.
pub fn hydration_tick(world: &mut World, p: Pos) {
    let state=world.get_block(p.0,p.1,p.2);
    if blocks::block_at_state(state)!=Some("dried_ghast") { return; }
    let level:u8=prop(state,"hydration").and_then(|s|s.parse().ok()).unwrap_or(0);
    let wet=prop(state,"waterlogged")==Some("true");
    if wet && level==3 {
        world.set_block(p.0,p.1,p.2,blocks::state_by_name("water").unwrap());
        let mut mob=Mob::new(0,Kind::HappyGhast,(f64::from(p.0)+0.5,f64::from(p.1)+0.5,f64::from(p.2)+0.5));
        mob.age=super::BABY_AGE;
        world.queue_water_hatch(mob);
    } else {
        let level=if wet {level+1} else {level.saturating_sub(1)};
        change(world,p,"hydration",&level.to_string());
    }
}

/// Воск, соскабливание и оживление статуи (вики, Copper Golem Statue).
pub fn statue_use(world: &mut World, p: Pos, item: &str) -> bool {
    let old=world.get_block(p.0,p.1,p.2);
    let Some(name)=blocks::block_at_state(old).filter(|n|n.ends_with("copper_golem_statue")) else {return false;};
    let waxed=name.starts_with("waxed_");
    let plain=name.strip_prefix("waxed_").unwrap_or(name);
    let (next,sound)=if item=="honeycomb" && !waxed {
        (format!("waxed_{plain}"),"minecraft:item.honeycomb.wax_on")
    } else if item.ends_with("_axe") && waxed {
        (plain.to_string(),"minecraft:item.axe.wax_off")
    } else if item.ends_with("_axe") {
        let name=match plain {
            "oxidized_copper_golem_statue"=>"weathered_copper_golem_statue",
            "weathered_copper_golem_statue"=>"exposed_copper_golem_statue",
            "exposed_copper_golem_statue"=>"copper_golem_statue",
            _=>{
                world.set_block(p.0,p.1,p.2,crate::world::AIR);
                let mob=Mob::new(0,Kind::CopperGolem,(f64::from(p.0)+0.5,f64::from(p.1),f64::from(p.2)+0.5));
                world.queue_water_hatch(mob);
                return true;
            }
        };
        (name.to_string(),"minecraft:item.axe.scrape")
    } else {return false;};
    if let (Some(before),Some(after))=(blocks::orientation_of(old),blocks::orientation(&next)) {
        let props:Vec<_>=before.properties.iter().filter_map(|p|before.value(old,p.name).map(|v|(p.name,v))).collect();
        if let Some(state)=after.state(&props) {world.set_block(p.0,p.1,p.2,state);}
    }
    world.note_sound(p.0,p.1,p.2,sound,1.0,1.0);
    true
}

/// Вики, Creaking Heart: Silk Touch сохраняет сердце; иначе 1–3 смолы,
/// Fortune добавляет к верхней границе. Естественное сердце даёт 20–24 опыта.
pub fn heart_drop(state: i32, silk: bool, fortune: i32, roll: f32) -> Option<crate::inventory::Stack> {
    if blocks::block_at_state(state)!=Some("creaking_heart") {return None;}
    let (name,count)=if silk {("creaking_heart",1)} else {("resin_clump",1+(roll*(3+fortune.max(0)) as f32) as i32)};
    blocks::item_named(name).map(|id|crate::inventory::Stack::new(id,count))
}
