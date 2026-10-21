// Ульи и гнёзда: minecraft.wiki/w/Beehive, /Bee, /Campfire.
// «up to 3 bees», «2400 ticks» с нектаром, «600 ticks» без него.
// [проверить] сроки и порядок событий ещё не сверены с чёрным ящиком.
use crate::{blocks, inventory::Stack, mob::{Kind, Mob}, world::World};
use serde::{Deserialize, Serialize};

pub mod component;

type Cell = (i32,i32,i32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Occupant {
    pub entity: Vec<u8>,
    pub ticks: i32,
    pub minimum: i32,
    pub nectar: bool,
    pub uuid: [u8;16],
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data { pub bees: Vec<Occupant> }

pub fn is_hive(state: i32) -> bool { matches!(blocks::block_at_state(state),Some("beehive"|"bee_nest")) }
pub fn honey(state: i32) -> i32 { crate::mob::wildlife::number(state,"honey_level") }
pub fn with_honey(state: i32, value: i32) -> Option<i32> { crate::mob::wildlife::with(state,"honey_level",&value.clamp(0,5).to_string()) }

pub fn entrance(at: Cell, state: i32) -> (f64,f64,f64) {
    let face=blocks::orientation_of(state).and_then(|b|b.value(state,"facing")).unwrap_or("north");
    let (dx,dz)=match face {"south"=>(0.0,1.0),"east"=>(1.0,0.0),"west"=>(-1.0,0.0),_=>(0.0,-1.0)};
    (f64::from(at.0)+0.5+dx*0.9,f64::from(at.1)+0.2,f64::from(at.2)+0.5+dz*0.9)
}

/// Вики, Campfire: дым до пяти блоков, ковёр пропускает дым.
pub fn smoked(world: &World, (x,y,z): Cell) -> bool {
    for dy in 1..=5 {
        let state=world.get_block(x,y-dy,z);
        let name=blocks::block_at_state(state).unwrap_or("");
        if matches!(name,"campfire"|"soul_campfire") {
            return blocks::orientation_of(state).and_then(|b|b.value(state,"lit"))==Some("true");
        }
        if crate::mob::solid(state) && !name.ends_with("_carpet") && name!="carpet" { return false; }
    }
    false
}

impl Data {
    pub fn enter(&mut self, bee: &Mob) -> bool {
        if self.bees.len()>=3 || bee.kind!=Kind::Bee || bee.is_dead() || bee.leash.is_some() || bee.wild.stung { return false; }
        if bee.validate_item_storage().is_err() { return false; }
        let mut entity=Vec::new(); bee.write(&mut entity);
        self.bees.push(Occupant { entity,ticks:0,minimum:if bee.wild.nectar {2400}else{600},nectar:bee.wild.nectar,uuid:bee.uuid });
        true
    }
    pub fn generated(seed: u64) -> Self {
        let mut data=Self::default();
        // UUID случайный даже у двух гнёзд с одинаковым местным положением.
        static NEXT: std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
        let nonce=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let moment=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|n|n.as_nanos() as u64);
        let mut rng=crate::mob::Rng::new(seed^moment^nonce);
        for _ in 0..3 {
            let mut bee=Mob::new(0,Kind::Bee,(0.0,0.0,0.0));
            bee.uuid[..8].copy_from_slice(&rng.next_u64().to_be_bytes());
            bee.uuid[8..].copy_from_slice(&rng.next_u64().to_be_bytes());
            data.enter(&bee);
        }
        data
    }
    pub fn bytes(&self) -> Vec<u8> { serde_json::to_vec(self).expect("данные улья") }
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let _scope = crate::stackdata::storage_depth::enter()?;
        let data: Self=serde_json::from_slice(bytes).ok()?;
        if data.bees.len()>3 || data.bees.iter().any(|b|decode_occupant(b).is_none()) { return None; }
        Some(data)
    }
}

/// В записи жильца должен находиться ровно один моб нужного вида.
/// Остаток нельзя игнорировать: там может быть вторая сущность или неверный хвост.
pub(crate) fn decode_occupant(occupant: &Occupant) -> Option<Mob> {
    let mut bytes = occupant.entity.as_slice();
    let mob = Mob::read(&mut bytes)?;
    (bytes.is_empty() && mob.kind == Kind::Bee).then_some(mob)
}

/// Работа ульев идёт и тогда, когда снаружи не осталось ни одной пчелы.
pub fn tick(world: &mut World, items: &mut crate::items::Items, next_id: &mut dyn FnMut()->i32) {
    let entries: Vec<_>=items.mobs().iter().filter(|m|m.kind==Kind::Bee && m.wild.enter && !m.is_dead()).filter_map(|m|m.wild.hive.map(|at|(m.entity_id,at,m.clone()))).collect();
    for (id,at,bee) in entries {
        if !is_hive(world.get_block(at.0,at.1,at.2)) { continue; }
        let entered=world.with_hive(at.0,at.1,at.2,|h|h.enter(&bee)).unwrap_or(false);
        if entered {
            items.remove_mobs(|m|m.entity_id==id);
            world.note_sound(at.0,at.1,at.2,"minecraft:block.beehive.enter",1.0,1.0);
        } else if let Some(bee)=items.mobs_mut().iter_mut().find(|m|m.entity_id==id) {
            bee.wild.hive=None; bee.wild.cannot_enter=200;
        }
    }
    let day=world.is_nether() || world.is_end() || world.clock().rem_euclid(24000)<12000;
    for at in world.hive_places() {
        let state=world.get_block(at.0,at.1,at.2);
        let exit=entrance(at,state);
        let blocked=crate::mob::solid(world.get_block(exit.0.floor() as i32,exit.1.floor() as i32,exit.2.floor() as i32));
        let release=day && (world.is_nether() || world.is_end() || !world.weather.is_raining()) && !blocked;
        // Вики, Beehive: жужжание «block.beehive.work», пока внутри кто-то есть, —
        // 0,5 % на такт (чёрный ящик, hive-exit: на +19) [проверить: вероятность].
        if world.hive(at.0,at.1,at.2).is_some_and(|data|!data.bees.is_empty()) && world.random_unit()<0.005 {
            world.note_sound_block_at((f64::from(at.0)+0.5,f64::from(at.1),f64::from(at.2)+0.5),"minecraft:block.beehive.work",1.0,1.0);
        }
        let occupants=world.with_hive(at.0,at.1,at.2,|data| {
            for bee in &mut data.bees { bee.ticks=bee.ticks.saturating_add(1); }
            let mut out=Vec::new();
            if release { data.bees.retain(|bee| { if bee.ticks>bee.minimum { out.push(bee.clone());false } else {true} }); }
            out
        }).unwrap_or_default();
        let mut level=honey(state);
        for occupant in occupants {
            if let Some(mut bee)=released(&occupant,at,state,next_id()) {
                if occupant.nectar {
                    level=(level+1+i32::from(world.random_unit()<0.01)).min(5); bee.wild.nectar=false; bee.wild.crops=0;
                    // Нектар снят при выходе, но клиент узнаёт об этом тактом позже: 18=0 (чёрный ящик, hive-exit).
                    bee.wild.nectar_ghost=true;
                }
                bee.wild.cooldown=200;
                // Рождённая в такте блоков, пчела видна слежению со следующего такта (hive-exit).
                bee.late=true; bee.wild.fresh=true;
                items.add_mob_late(bee);
                world.note_sound(at.0,at.1,at.2,"minecraft:block.beehive.exit",1.0,1.0);
            }
        }
        if level!=honey(state) && let Some(new)=with_honey(state,level) { world.set_block(at.0,at.1,at.2,new); }
    }
}
fn released(occupant: &Occupant, at: Cell, state: i32, id: i32) -> Option<Mob> {
    let mut bee=decode_occupant(occupant)?;
    bee.entity_id=id; bee.uuid=occupant.uuid;
    (bee.x,bee.y,bee.z)=entrance(at,state);
    bee.wild.hive=Some(at); bee.wild.enter=false;
    // Вышедшая из улья летит: 5=true сразу при появлении (чёрный ящик, bee-hive-honey).
    bee.wild.flying=true;
    // Вики, Bee: рост и пауза разведения внутри улья заморожены.
    Some(bee)
}

pub fn disturb(world: &mut World, items: &mut crate::items::Items, at: Cell, player: i32, next_id: &mut dyn FnMut()->i32) {
    let state=world.get_block(at.0,at.1,at.2);
    let safe=smoked(world,at);
    let bees=world.with_hive(at.0,at.1,at.2,|data|std::mem::take(&mut data.bees)).unwrap_or_default();
    for occupant in bees {
        if let Some(mut bee)=released(&occupant,at,state,next_id()) {
            bee.wild.cannot_enter=400;
            if !safe { crate::mob::wildlife::on_hurt(&mut bee,player); }
            bee.wild.flying=true;
            items.add_mob_late(bee);
            world.note_sound(at.0,at.1,at.2,"minecraft:block.beehive.exit",1.0,1.0);
        }
    }
    if !safe {
        for bee in items.mobs_mut().iter_mut().filter(|m|m.kind==Kind::Bee && !m.is_dead()) {
            if (bee.x-f64::from(at.0)).abs()<=8.0 && (bee.y-f64::from(at.1)).abs()<=6.0 && (bee.z-f64::from(at.2)).abs()<=8.0 { crate::mob::wildlife::on_hurt(bee,player); }
        }
    }
}

/// Полный улей: бутылка мёда либо три соты, уровень снова ноль.
pub fn harvest(world: &mut World, at: Cell, item: &str) -> Option<Stack> {
    let state=world.get_block(at.0,at.1,at.2);
    if !is_hive(state) || honey(state)!=5 { return None; }
    // Звук — у вызывающего: бутылка играет всем, кроме щёлкнувшего, ножницы —
    // из точки игрока всем (чёрный ящик, bee-hive-honey).
    let (name,count)=match item {
        "glass_bottle"=>("honey_bottle",1),
        "shears"=>("honeycomb",3),
        _=>return None,
    };
    let stack=Stack::new(blocks::item_named(name)?,count);
    world.set_block(at.0,at.1,at.2,with_honey(state,0)?);
    Some(stack)
}

pub fn silk_stack(world: &World, at: Cell) -> Option<Stack> {
    let state=world.get_block(at.0,at.1,at.2);
    let name=blocks::block_at_state(state)?;
    if !is_hive(state) { return None; }
    let data=world.hive(at.0,at.1,at.2).cloned().unwrap_or_default();
    Some(Stack::new(blocks::item_named(name)?,1).with_extras(crate::stackdata::Data { bees:data.bytes(),honey:Some(honey(state) as u8),..Default::default() }))
}

pub fn place_contents(world: &mut World, at: Cell, stack: &Stack) {
    let state=world.get_block(at.0,at.1,at.2);
    if !is_hive(state) { return; }
    let extras=stack.extras();
    if let Some(data)=Data::from_bytes(&extras.bees) { world.with_hive(at.0,at.1,at.2,|h|*h=data); }
    if let Some(level)=extras.honey && let Some(state)=with_honey(state,i32::from(level)) { world.set_block(at.0,at.1,at.2,state); }
}

/// Данные `setblock ... {bees:[...]}` — Entity format / Beehive.
pub fn from_snbt(text: &str) -> Option<Data> {
    from_value(&crate::snbt::parse(text)?)
}

pub fn from_value(value: &crate::snbt::Value) -> Option<Data> {
    let list=value.get("bees").or_else(||value.get("Bees"));
    let list=match list {None=>return Some(Data::default()),Some(crate::snbt::Value::List(list))=>list,_=>return None};
    if list.len()>3 { return None; }
    let mut hive=Data::default();
    for entry in list {
        let entity=entry.get("entity_data").or_else(||entry.get("EntityData"))?;
        let mut bee=Mob::new(0,Kind::Bee,(0.0,0.0,0.0));
        if let Some(id)=entity.get("id").and_then(|v|v.as_str()) && id!="minecraft:bee" && id!="bee" { return None; }
        if let Some(n)=entity.get("Age").and_then(|v|v.as_f64()) { bee.age=n as i32; }
        if let Some(n)=entity.get("Health").and_then(|v|v.as_f64()) { bee.vitals.health=(n as f32).clamp(0.5,10.0); }
        if let Some(name)=entity.get("CustomName").and_then(|v|v.as_str()) { bee.name=Some(name.into()); }
        if let Some(n)=entity.get("HasNectar").and_then(|v|v.as_f64()) { bee.wild.nectar=n!=0.0; }
        if let Some(n)=entity.get("InLove").and_then(|v|v.as_f64()) {bee.love=(n as i32).max(0);}
        if let Some(n)=entity.get("NoAI").and_then(|v|v.as_f64()) {bee.no_ai=n!=0.0;}
        if let Some(n)=entity.get("Silent").and_then(|v|v.as_f64()) {bee.silent=n!=0.0;}
        if let Some(n)=entity.get("CannotEnterHiveTicks").and_then(|v|v.as_f64()) {bee.wild.cannot_enter=(n as i32).max(0);}
        static NEXT: std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
        let nonce=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let moment=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|n|n.as_nanos() as u64);
        let mut rng=crate::mob::Rng::new(moment^nonce);
        let mut uuid=[0;16];
        uuid[..8].copy_from_slice(&rng.next_u64().to_be_bytes());
        uuid[8..].copy_from_slice(&rng.next_u64().to_be_bytes());
        if let Some(crate::snbt::Value::List(parts))=entity.get("UUID") && parts.len()==4 {
            for (j,p) in parts.iter().enumerate() {uuid[j*4..j*4+4].copy_from_slice(&(p.as_f64()? as i32).to_be_bytes());}
        }
        bee.uuid=uuid;
        hive.enter(&bee);
        let stored=hive.bees.last_mut()?;
        stored.ticks=entry.get("ticks_in_hive").or_else(||entry.get("TicksInHive")).and_then(|v|v.as_f64()).unwrap_or(0.0).max(0.0) as i32;
        stored.minimum=entry.get("min_ticks_in_hive").or_else(||entry.get("MinOccupationTicks")).and_then(|v|v.as_f64()).unwrap_or(f64::from(stored.minimum)).max(0.0) as i32;
        stored.uuid=uuid;
    }
    Some(hive)
}

/// Компонент `bees` 77: список anonymousNbt, VarInt, VarInt
/// (minecraft-data 26.1 protocol.json). [Bedrock: потом] довески ульев.
pub fn encode_bees(data: &Data, out: &mut Vec<u8>) {
    use crate::network::{nbt::Compound, varint::encode_varint};
    out.extend_from_slice(&encode_varint(data.bees.len() as i32));
    for occupant in &data.bees {
        let bee=Mob::read(&mut occupant.entity.as_slice()).expect("проверенная запись пчелы");
        let mut tag=Compound::new().string("id","minecraft:bee")
            .int("Age",bee.age).int("InLove",bee.love).float("Health",bee.vitals.health)
            .boolean("HasNectar",bee.wild.nectar).boolean("HasStung",bee.wild.stung)
            .int("CannotEnterHiveTicks",bee.wild.cannot_enter)
            .boolean("NoAI",bee.no_ai).boolean("Silent",bee.silent);
        let uuid: Vec<i32>=occupant.uuid.as_chunks::<4>().0.iter().map(|part|i32::from_be_bytes(*part)).collect();
        tag=tag.int_array("UUID",&uuid);
        if let Some(name)=bee.name { tag=tag.string("CustomName",&name); }
        out.extend_from_slice(&tag.encode_network());
        out.extend_from_slice(&encode_varint(occupant.ticks));
        out.extend_from_slice(&encode_varint(occupant.minimum));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(s:&str)->i32 {blocks::state_from_text(s).unwrap()}
    fn world()->World {let mut w=World::in_memory();w.set_block(0,4,0,state("beehive[facing=south,honey_level=5]"));w}
    #[test]
    fn native_component_and_command_components_preserve_bees() {
        let mut data=Data::generated(38);
        data.bees[0].ticks=800;data.bees[0].minimum=2400;
        let mut bytes=Vec::new();encode_bees(&data,&mut bytes);
        let back=component::decode(&bytes).unwrap();
        assert_eq!(back.bees.len(),3);
        for (a,b) in data.bees.iter().zip(&back.bees) {
            assert_eq!((a.uuid,a.ticks,a.minimum,a.nectar),(b.uuid,b.ticks,b.minimum,b.nectar));
            assert_eq!(Mob::read(&mut a.entity.as_slice()).unwrap().age,Mob::read(&mut b.entity.as_slice()).unwrap().age);
        }
        assert!(component::decode(&bytes[..bytes.len()-1]).is_none());
        assert!(component::decode(&[4]).is_none());
        let (_,extras)=crate::stackdata::parse_item_spec("beehive[bees=[{entity_data:{id:bee,Age:-123,HasNectar:1b},ticks_in_hive:20,min_ticks_in_hive:2400}],block_state={honey_level:5}]").unwrap();
        assert_eq!(extras.honey,Some(5));
        let hive=Data::from_bytes(&extras.bees).unwrap();
        assert_eq!(hive.bees[0].ticks,20);assert!(hive.bees[0].nectar);
        let mut received=crate::stackdata::Data::default();
        crate::stackdata::read_component(77,&bytes,&mut received);
        assert_eq!(Data::from_bytes(&received.bees).unwrap().bees.len(),3);
        crate::stackdata::read_component(76,&[1,11,b'h',b'o',b'n',b'e',b'y',b'_',b'l',b'e',b'v',b'e',b'l',1,b'5'],&mut received);
        assert_eq!(received.honey,Some(5));
    }
    #[test]
    fn harvesting_resets_honey_and_preserves_facing() {
        let mut w=world();
        let bottle=harvest(&mut w,(0,4,0),"glass_bottle").unwrap();
        assert_eq!(blocks::item_name(bottle.item),Some("honey_bottle"));
        assert_eq!(w.get_block(0,4,0),state("beehive[facing=south,honey_level=0]"));
        assert!(harvest(&mut w,(0,4,0),"shears").is_none());
        w.set_block(0,4,0,state("bee_nest[honey_level=5]"));
        let comb=harvest(&mut w,(0,4,0),"shears").unwrap();assert_eq!(comb.count,3);
    }
    #[test]
    fn smoke_requires_lit_campfire_and_clear_path() {
        let mut w=world();w.set_block(0,0,0,state("campfire[lit=true]"));
        assert!(smoked(&w,(0,4,0)));
        w.set_block(0,2,0,state("white_carpet"));assert!(smoked(&w,(0,4,0)));
        w.set_block(0,2,0,state("stone"));assert!(!smoked(&w,(0,4,0)));
        w.set_block(0,2,0,0);w.set_block(0,0,0,state("campfire[lit=false]"));assert!(!smoked(&w,(0,4,0)));
    }
    #[test]
    fn only_three_bees_and_no_leashed_or_spent_bees_enter() {
        let mut hive=Data::default();let mut bee=Mob::new(1,Kind::Bee,(0.0,0.0,0.0));
        bee.wild.stung=true;assert!(!hive.enter(&bee));bee.wild.stung=false;
        bee.leash=Some(crate::mob::leash::Holder::Player { id:3,uuid:[1;16] });assert!(!hive.enter(&bee));bee.leash=None;
        for _ in 0..3 {assert!(hive.enter(&bee));}assert!(!hive.enter(&bee));
        assert_eq!(hive.bees[0].minimum,600);
    }
    #[test]
    fn silk_touch_roundtrip_preserves_occupants_honey_and_name() {
        let mut w=world();let mut bee=Mob::new(2,Kind::Bee,(0.0,0.0,0.0));
        bee.name=Some("Honey".into());bee.age= -123;bee.wild.nectar=true;
        w.with_hive(0,4,0,|h|assert!(h.enter(&bee)));
        let stack=silk_stack(&w,(0,4,0)).unwrap();
        let data=stack.extras();let bytes=crate::stackdata::to_bytes(&data);
        assert_eq!(crate::stackdata::from_bytes(&bytes),Some(data));
        let mut hive_bytes=Vec::new();
        w.with_hive(0,4,0,|h|hive_bytes=h.bytes());
        w.set_block(0,4,0,0);w.set_block(2,4,0,state("beehive"));place_contents(&mut w,(2,4,0),&stack);
        assert_eq!(honey(w.get_block(2,4,0)),5);
        let hive=w.hive(2,4,0).unwrap();assert_eq!(hive.bytes(),hive_bytes);
        let read=Mob::read(&mut hive.bees[0].entity.as_slice()).unwrap();assert_eq!(read.name.as_deref(),Some("Honey"));assert_eq!(read.age,-123);
    }
    #[test]
    fn honey_is_made_on_exit_and_blocked_exit_keeps_occupants() {
        let mut w=world();w.set_time_of_day(1000);
        w.set_block(0,4,0,state("beehive[facing=south,honey_level=4]"));
        let mut bee=Mob::new(2,Kind::Bee,(0.0,4.0,0.0));bee.wild.nectar=true;
        w.with_hive(0,4,0,|h|{h.enter(&bee);h.bees[0].ticks=2400;});
        w.set_block(0,4,1,state("stone"));let mut items=crate::items::Items::new();let mut id=10;
        tick(&mut w,&mut items,&mut ||{id+=1;id});assert_eq!(w.hive(0,4,0).unwrap().bees.len(),1);assert_eq!(honey(w.get_block(0,4,0)),4);
        w.set_block(0,4,1,0);tick(&mut w,&mut items,&mut ||{id+=1;id});
        assert!(w.hive(0,4,0).unwrap().bees.is_empty());assert_eq!(honey(w.get_block(0,4,0)),5);assert_eq!(items.mobs().len(),1);assert!(!items.mobs()[0].wild.nectar);
    }
    #[test]
    fn released_bee_waits_a_tick_and_clears_nectar_flag_later() {
        let mut w=world();w.set_time_of_day(1000);
        let mut bee=Mob::new(2,Kind::Bee,(0.0,4.0,0.0));bee.wild.nectar=true;
        w.with_hive(0,4,0,|h|{h.enter(&bee);h.bees[0].ticks=2400;});
        let mut items=crate::items::Items::new();let mut id=10;
        tick(&mut w,&mut items,&mut ||{id+=1;id});
        let out=&items.mobs()[0];
        assert!(out.late&&out.wild.fresh&&out.wild.flying&&out.wild.nectar_ghost);
        assert!(!out.wild.nectar);
        // Строго «больше», а не «не меньше»: на 2400 пчела ещё внутри.
        let mut w=world();w.set_time_of_day(1000);
        w.with_hive(0,4,0,|h|{h.enter(&bee);h.bees[0].ticks=2398;});
        let mut items=crate::items::Items::new();
        tick(&mut w,&mut items,&mut ||{id+=1;id});
        assert_eq!(w.hive(0,4,0).unwrap().bees.len(),1);
    }
    #[test]
    fn unprotected_harvest_releases_and_angers_swarm() {
        let mut w=world();w.with_hive(0,4,0,|h|*h=Data::generated(1));
        let mut items=crate::items::Items::new();let mut id=10;
        disturb(&mut w,&mut items,(0,4,0),99,&mut ||{id+=1;id});
        assert_eq!(items.mobs().len(),3);assert!(items.mobs().iter().all(|m|m.wild.attacker==Some(99)));
    }
    #[test]
    fn malformed_or_oversized_hives_are_rejected() {
        assert!(Data::from_bytes(b"{}").is_none());
        let mut h=Data::generated(1);h.bees.push(h.bees[0].clone());assert!(Data::from_bytes(&h.bytes()).is_none());
    }
}

#[cfg(test)]
mod storage_entry_tests {
    use super::*;

    #[test]
    fn unrepresentable_equipment_keeps_real_bee_outside_hive() {
        let mut world = World::in_memory();
        world.set_block(0,64,0,blocks::state_by_name("beehive").unwrap());
        let mut bee = Mob::new(1,Kind::Bee,(0.5,64.0,0.5));
        let stack = Stack::new(blocks::item_named("stone").unwrap(),1).with_extras(crate::stackdata::Data {
            name: Some("я".repeat(40_000)), ..Default::default()
        });
        bee.equipment = Some([Some(stack),None,None,None,None,None]);
        assert!(bee.validate_item_storage().is_err());
        let mut hive = Data::default();
        assert!(!hive.enter(&bee)); assert!(hive.bees.is_empty());
        bee.wild.hive = Some((0,64,0)); bee.wild.enter = true;
        let mut items = crate::items::Items::new(); items.add_mob(bee);
        tick(&mut world,&mut items,&mut || 2);
        assert_eq!(items.mobs().len(),1);
        assert_eq!(items.mobs()[0].entity_id,1);
        assert_eq!(items.mobs()[0].equipment.unwrap()[0],Some(stack));
        assert!(world.hive(0,64,0).unwrap().bees.is_empty());
    }
}
