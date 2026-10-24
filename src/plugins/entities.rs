// Сущности принадлежат миру; гость получает UUID и проверенные копии предметов.
use super::api::{PluginCtx, parse_uuid, dimension_from_wit, dimension_to_wit, mcsa::api::{entities, inventory, types}};
use crate::{dimension::Dimension, players::Order};

fn valid_location(at: &types::Location) -> bool {
    at.x.is_finite() && at.z.is_finite() && at.x.abs() <= 30_000_000.0 && at.z.abs() <= 30_000_000.0 && at.y.is_finite() && (-2048.0..=2048.0).contains(&at.y)
}
fn vector(value: (f64, f64, f64)) -> entities::Vector {entities::Vector {x: value.0, y: value.1, z: value.2}}
fn snapshot(uuid: [u8;16], kind: &str, dimension: Dimension, at: (f64,f64,f64), size: (f64,f64)) -> entities::Snapshot {
    entities::Snapshot {uuid:crate::network::login::format_uuid(&uuid),kind:kind.into(),at:types::Location {dimension:dimension_to_wit(dimension),x:at.0,y:at.1,z:at.2,yaw:None,pitch:None},width:size.0,height:size.1,health:0.0,living:false,hostile:false,animal:false,shooter:String::new(),embedded:false}
}
fn potion(effect: &crate::survival::Effect) -> super::api::mcsa::api::player::PotionEffect {
    super::api::mcsa::api::player::PotionEffect {kind:effect.kind.name().into(),duration:effect.ticks_left,amplifier:effect.amplifier,ambient:effect.ambient,particles:effect.visible,icon:effect.show_icon}
}
impl PluginCtx {
    fn edit_mob<T>(&mut self, uuid: [u8;16], mut edit: impl FnMut(&mut crate::mob::Mob)->Result<T,String>) -> Result<T,String> {
        if let Some((_,mob))=self.pending_mobs.get_mut(&uuid) {return edit(mob);}
        for dimension in Dimension::ALL {let mut items=self.shared.items_in(dimension).lock().expect("сущности");if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&!m.is_dead()) {return edit(mob);}}
        Err("Живая сущность больше не существует".into())
    }

    fn entity_snapshots(&self, dimension: Dimension) -> Result<Vec<entities::Snapshot>,String> {
        let mut result=Vec::new();let mut ids=std::collections::HashMap::new();
        {let players=self.shared.players.lock().expect("игроки");for player in players.members().iter().filter(|p|p.dimension==dimension) {
            ids.insert(player.entity_id,crate::network::login::format_uuid(&player.uuid));
            let mut state=snapshot(player.uuid,"player",dimension,(player.position.x,player.position.y,player.position.z),(0.6,if player.sneaking {1.5} else {1.8}));state.living=true;result.push(state);
        }}
        {let items=self.shared.items_in(dimension).lock().expect("сущности");
            for mob in items.mobs() {
                ids.insert(mob.entity_id,crate::network::login::format_uuid(&mob.uuid));
                let mut state=snapshot(mob.uuid,mob.kind.name(),dimension,(mob.x,mob.y,mob.z),mob.size());state.living= !matches!(mob.kind,crate::mob::Kind::EndCrystal|crate::mob::Kind::LeashKnot|crate::mob::Kind::ShulkerBullet|crate::mob::Kind::EvokerFangs);state.health=f64::from(mob.vitals.health);state.hostile=mob.kind.hostile();
                state.animal=matches!(mob.kind,crate::mob::Kind::Cow|crate::mob::Kind::Pig|crate::mob::Kind::Sheep|crate::mob::Kind::Chicken|crate::mob::Kind::Horse|crate::mob::Kind::Donkey|crate::mob::Kind::Mule|crate::mob::Kind::Wolf|crate::mob::Kind::Cat|crate::mob::Kind::Rabbit|crate::mob::Kind::Fox|crate::mob::Kind::Bee|crate::mob::Kind::Turtle|crate::mob::Kind::Axolotl|crate::mob::Kind::Frog|crate::mob::Kind::Sniffer|crate::mob::Kind::Armadillo|crate::mob::Kind::Strider|crate::mob::Kind::Panda|crate::mob::Kind::PolarBear|crate::mob::Kind::Ocelot|crate::mob::Kind::Mooshroom|crate::mob::Kind::Llama|crate::mob::Kind::TraderLlama|crate::mob::Kind::Camel|crate::mob::Kind::Goat|crate::mob::Kind::Parrot);
                state.at.yaw=Some(mob.yaw);state.at.pitch=Some(mob.pitch);result.push(state);
            }
            for cart in items.carts() {let mut state=snapshot(cart.uuid,cart.kind.name(),dimension,(cart.x,cart.y,cart.z),(if cart.kind.is_boat() {1.375}else {0.98},if cart.kind.is_boat() {0.5625}else {0.7}));state.at.yaw=Some(cart.yaw);result.push(state);}
            for frame in items.frames().iter().filter(|f|f.painting.is_none()) {result.push(snapshot(frame.uuid,if frame.glow {"glow_item_frame"} else {"item_frame"},dimension,(f64::from(frame.pos.0)+0.5,f64::from(frame.pos.1)+0.125,f64::from(frame.pos.2)+0.5),(0.75,0.75)));}
            for item in items.items().iter().filter(|i|i.fuse.is_none()&&i.block.is_none()) {result.push(snapshot(item.uuid,"item",dimension,(item.x,item.y,item.z),(0.25,0.25)));}
        }
        {let arrows=self.shared.arrows.lock().expect("стрелы");for arrow in arrows.list.iter().filter(|a|a.dimension==dimension) {
            let mut state=snapshot(arrow.uuid,if arrow.trident.is_some() {"trident"} else if arrow.spectral() {"spectral_arrow"} else {"arrow"},dimension,(arrow.x,arrow.y,arrow.z),(0.5,0.5));state.shooter=ids.get(&arrow.owner).cloned().unwrap_or_default();state.embedded=arrow.in_ground;result.push(state);
        }}
        {let thrown=self.shared.thrown.lock().expect("снаряды");for shot in thrown.flying.iter().chain(thrown.landed.iter()).filter(|p|p.dimension==dimension) {
            use crate::thrown::Kind;
            let name=match shot.kind {Kind::Splash=>"splash_potion",Kind::Lingering=>"lingering_potion",Kind::DragonFireball=>"dragon_fireball",Kind::SmallFireball=>"small_fireball",Kind::GhastFireball=>"fireball",Kind::Snowball=>"snowball",Kind::Egg=>"egg",Kind::EnderPearl=>"ender_pearl",Kind::ExperienceBottle=>"experience_bottle",Kind::WitherSkull=>"wither_skull",Kind::LlamaSpit=>"llama_spit",Kind::WindCharge=>"wind_charge",Kind::BreezeWindCharge=>"breeze_wind_charge"};
            let mut state=snapshot(shot.uuid,name,dimension,(shot.x,shot.y,shot.z),(0.25,0.25));state.shooter=ids.get(&shot.owner).cloned().unwrap_or_default();result.push(state);
        }}
        {let thrown=self.shared.thrown.lock().expect("снаряды");for cloud in thrown.clouds.iter().chain(thrown.faded.iter()).filter(|c|c.dimension==dimension) {let mut state=snapshot(cloud.uuid,"area_effect_cloud",dimension,(cloud.x,cloud.y,cloud.z),(f64::from(cloud.radius)*2.0,0.5));state.shooter=ids.get(&cloud.owner).cloned().unwrap_or_default();result.push(state);}for rocket in thrown.rockets.iter().filter(|r|r.dimension==dimension) {let mut state=snapshot(rocket.uuid,"firework_rocket",dimension,(rocket.x,rocket.y,rocket.z),(0.25,0.25));state.shooter=ids.get(&rocket.owner).cloned().unwrap_or_default();result.push(state);}}
        if result.len()>8192 {return Err("Слишком много сущностей для одного запроса (8192)".into());}Ok(result)
    }
}
impl entities::Host for PluginCtx {
    fn create_mob(&mut self, at: types::Location, kind: String) -> Result<String,String> {
        if !valid_location(&at)||at.yaw.is_some_and(|v|!v.is_finite())||at.pitch.is_some_and(|v|!v.is_finite()) {return Err("Недопустимая позиция сущности".into());}
        if kind!="vex" {return Err("Создание этого вида через Bukkit пока не поддерживается".into());}
        if self.pending_mobs.len()>=64 {return Err("Слишком много не добавленных в мир сущностей (64)".into());}
        let dimension=dimension_from_wit(at.dimension);
        if !self.shared.world_in(dimension).lock().expect("мир").has_chunk((at.x.floor() as i32).div_euclid(16),(at.z.floor() as i32).div_euclid(16)) {return Err("Чанк не загружен".into());}
        let mut mob=crate::mob::Mob::new(self.shared.next_entity_id(),crate::mob::Kind::Vex,(at.x,at.y,at.z));mob.yaw=at.yaw.unwrap_or(0.0);mob.pitch=at.pitch.unwrap_or(0.0);
        let result=crate::network::login::format_uuid(&mob.uuid);self.pending_mobs.insert(mob.uuid,(dimension,mob));Ok(result)
    }
    fn finish_mob(&mut self, uuid: String, commit: bool) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let (dimension,mob)=self.pending_mobs.remove(&uuid).ok_or("Сущность не принадлежит текущему созданию")?;
        if commit {self.shared.items_in(dimension).lock().expect("сущности").add_mob(mob);}Ok(())
    }
    fn mob_control(&mut self, uuid: String, field: u8, value: i32) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if field>2 || field<2&&!(0..=1).contains(&value) {return Err("Неверное поле сущности или логическое значение".into());}
        self.edit_mob(uuid,|mob| {if field>0&&mob.kind!=crate::mob::Kind::Vex {return Err("Ограниченный срок жизни поддерживается только у вредин".into());}
            match field {0=>mob.persistent=value!=0,1=>mob.illager.plugin_limited_life=Some(value!=0),_=>mob.illager.life=value}Ok(())})
    }
    fn get_target(&mut self, uuid: String) -> Result<Option<String>,String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let target=self.edit_mob(uuid,|mob|Ok(mob.plugin_targets()[0]))?;let Some(target)=target else {return Ok(None);};
        if let Some(player)=self.shared.players.lock().expect("игроки").members().iter().find(|p|p.entity_id==target) {return Ok(Some(crate::network::login::format_uuid(&player.uuid)));}
        for dimension in Dimension::ALL {if let Some(mob)=self.shared.items_in(dimension).lock().expect("сущности").mobs().iter().find(|m|m.entity_id==target) {return Ok(Some(crate::network::login::format_uuid(&mob.uuid)));}}Ok(None)
    }
    fn set_target(&mut self, uuid: String, target: Option<String>) -> Result<(),String> {
        let mob_dimension=entities::Host::describe(self,uuid.clone())?.at.dimension;
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let target=target.map(|uuid| {let uuid=parse_uuid(&uuid).ok_or("UUID цели не разобрать")?;self.shared.players.lock().expect("игроки").members().iter().find(|p|p.uuid==uuid).map(|p|(p.entity_id,p.dimension)).ok_or("Принудительная цель моба пока должна быть игроком")}).transpose()?;
        if target.is_some_and(|(_,dimension)|dimension_to_wit(dimension)!=mob_dimension) {return Err("Цель должна находиться в мире моба".into());}
        self.edit_mob(uuid,|mob|{mob.set_plugin_target(0,target.map(|p|p.0));Ok(())})
    }

    fn horse(&mut self, uuid: String) -> Result<entities::HorseState,String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {
            let state={let items=self.shared.items_in(dimension).lock().expect("сущности");items.mobs().iter().find(|m|m.uuid==uuid&&(crate::mob::horse::equine(m.kind)||crate::mob::land::rideable(m.kind))).map(|m|if crate::mob::horse::equine(m.kind) {(m.horse.tamed,m.horse.chest,m.horse.inventory())}else {(m.land.tamed,m.land.chest,crate::mob::land::inventory(m))})};
            if let Some((tamed,chest,stacks))=state {return Ok(entities::HorseState {tamed,chest,items:self.inventory_items(&stacks)});}
        }Err("Инвентарь этого вида ездового животного пока не поддерживается".into())
    }
    fn set_horse_item(&mut self, uuid: String, slot: u32, stack: Option<inventory::Item>) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let stack=self.inventory_stack(stack)?;
        for dimension in Dimension::ALL {let mut items=self.shared.items_in(dimension).lock().expect("сущности");
            if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&crate::mob::land::rideable(m.kind)) {let mut slots=crate::mob::land::inventory(mob);let place=slots.get_mut(slot as usize).ok_or("Ячейка вне инвентаря животного")?;*place=stack;if !crate::mob::land::valid_inventory(mob,&slots) {return Err("Предмет не подходит для этой ячейки животного".into());}crate::mob::land::store_inventory(mob,&slots);return Ok(());}
            if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&crate::mob::horse::equine(m.kind)) {
            if slot>=if mob.horse.chest {17} else {2} {return Err("Ячейка вне инвентаря лошади".into());}
            if stack.is_some_and(|s|!crate::mob::horse::may_place(mob.kind,slot as usize,s.item)||slot<2&&s.count!=1) {return Err("Предмет не подходит для этой ячейки лошади".into());}
            let old=mob.horse.slots;mob.horse.slots[slot as usize]=stack;crate::mob::horse::equipment_changed(mob,old);return Ok(());
        }}Err("Лошадь отсутствует".into())
    }
    fn tame_horse(&mut self, uuid: String, tamed: bool) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {let mut items=self.shared.items_in(dimension).lock().expect("сущности");if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&crate::mob::horse::equine(m.kind)) {mob.horse.tamed=tamed;if !tamed {mob.tame.owner=None;mob.tame.owner_id=0;}return Ok(());}}
        Err("Лошадь отсутствует".into())
    }
    fn set_horse_owner(&mut self, uuid: String, owner: Option<String>) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let owner=owner.map(|value|parse_uuid(&value).ok_or("UUID хозяина не разобрать")).transpose()?;
        let owner_id=owner.and_then(|uuid|self.shared.players.lock().expect("игроки").members().iter().find(|p|p.uuid==uuid).map(|p|p.entity_id)).unwrap_or(0);
        for dimension in Dimension::ALL {let mut items=self.shared.items_in(dimension).lock().expect("сущности");if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&crate::mob::horse::equine(m.kind)) {mob.tame.owner=owner;mob.tame.owner_id=owner_id;if owner.is_some() {mob.horse.tamed=true;mob.persistent=true;}return Ok(());}}
        Err("Лошадь отсутствует".into())
    }

    fn launch(&mut self, uuid: String, kind: String, velocity: entities::Vector) -> Result<String,String> {
        if ![velocity.x,velocity.y,velocity.z].iter().all(|v|v.is_finite()&&v.abs()<=64.0) {return Err("Скорость снаряда: конечные компоненты в пределах ±64".into());}
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let (owner,dimension,at)={let players=self.shared.players.lock().expect("игроки");let player=players.members().iter().find(|p|p.uuid==uuid).ok_or("Стрелок отсутствует")?;let eye=players.ui.info.get(&uuid).ok_or("Стрелок Java отсутствует")?.plugin_state.eye_height;(player.entity_id,player.dimension,(player.position.x,player.position.y+eye,player.position.z))};
        let id=self.shared.next_entity_id();let motion=(velocity.x,velocity.y,velocity.z);
        use crate::thrown::{Flying,Kind};
        let shot=match kind.as_str() {
            "fireball"=>Flying::mob_fireball(id,dimension,Kind::GhastFireball,at,motion,owner,1),
            "small_fireball"=>Flying::mob_fireball(id,dimension,Kind::SmallFireball,at,motion,owner,0),
            "wind_charge"=>Flying::wind_charge(id,dimension,at,motion,owner,false),
            "splash_potion"=>Flying::potion(id,dimension,Kind::Splash,crate::inventory::Stack::new(crate::blocks::item_named("splash_potion").ok_or("Неизвестный предмет зелья")?,1),at,motion,owner),
            _=>return Err("Этот вид снаряда пока не поддерживает запуск через Bukkit".into()),
        };
        let mut shot=shot;shot.in_owner=true;shot.vx=velocity.x;shot.vy=velocity.y;shot.vz=velocity.z;shot.start=(at,motion);
        let result=crate::network::login::format_uuid(&shot.uuid);
        let mut world=self.shared.world_in(dimension).lock().expect("мир");world.note_projectile_launch(shot.clone());
        self.shared.thrown.lock().expect("снаряды").flying.push(shot);Ok(result)
    }
    fn set_explosive(&mut self, uuid: String, yield_: Option<f32>, incendiary: Option<bool>) -> Result<(),String> {
        if yield_.is_some_and(|v|!v.is_finite()||!(0.0..=16.0).contains(&v)) {return Err("Сила взрыва должна быть конечной, от 0 до 16".into());}
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let mut thrown=self.shared.thrown.lock().expect("снаряды");
        let shot=thrown.flying.iter_mut().find(|p|p.uuid==uuid&&matches!(p.kind,crate::thrown::Kind::GhastFireball|crate::thrown::Kind::SmallFireball)).ok_or("Огненный шар отсутствует или не поддерживает эту настройку")?;
        if let Some(value)=yield_ {if shot.kind==crate::thrown::Kind::SmallFireball&&value!=0.0 {return Err("Малый огненный шар не взрывается".into());}shot.plugin_yield=Some(value);}
        if let Some(value)=incendiary {shot.plugin_incendiary=Some(value);}Ok(())
    }

    fn describe(&mut self, uuid: String) -> Result<entities::Snapshot,String> {
        let id=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if let Some((dimension,mob))=self.pending_mobs.get(&id) {let mut state=snapshot(id,mob.kind.name(),*dimension,(mob.x,mob.y,mob.z),mob.size());state.living=true;state.hostile=mob.kind.hostile();state.health=f64::from(mob.vitals.health);state.at.yaw=Some(mob.yaw);state.at.pitch=Some(mob.pitch);return Ok(state);}
        let uuid=crate::network::login::format_uuid(&id);
        for dimension in Dimension::ALL {if let Some(state)=self.entity_snapshots(dimension)?.into_iter().find(|state|state.uuid==uuid) {return Ok(state);}}
        Err("Сущность больше не существует".into())
    }
    fn in_world(&mut self, dimension: types::Dimension) -> Result<Vec<entities::Snapshot>,String> {self.entity_snapshots(dimension_from_wit(dimension))}
    fn cloud_potion(&mut self, uuid: String) -> Result<Option<String>,String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let thrown=self.shared.thrown.lock().expect("снаряды");let cloud=thrown.clouds.iter().chain(thrown.faded.iter()).find(|c|c.uuid==uuid).ok_or("Облако больше не существует")?;
        Ok(cloud.base_potion.and_then(|id|crate::survival::POTIONS.get(usize::from(id))).map(|name|(*name).into()))
    }
    fn effects(&mut self, uuid: String) -> Result<Vec<super::api::mcsa::api::player::PotionEffect>,String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {let items=self.shared.items_in(dimension).lock().expect("сущности");if let Some(mob)=items.mobs().iter().find(|m|m.uuid==uuid) {return Ok(mob.vitals.effects.iter().map(potion).collect());}}
        let thrown=self.shared.thrown.lock().expect("снаряды");if let Some(cloud)=thrown.clouds.iter().chain(thrown.faded.iter()).find(|p|p.uuid==uuid) {return Ok(cloud.custom_effects.iter().map(potion).collect());}
        if let Some(shot)=thrown.flying.iter().chain(thrown.landed.iter()).find(|p|p.uuid==uuid) {return Ok(shot.effects().iter().map(potion).collect());}
        Err("Живая сущность или зелье больше не существует".into())
    }
    fn equipment(&mut self, uuid: String, slot: u8) -> Result<Option<inventory::Item>,String> {
        if slot>5 {return Err("Неизвестная ячейка экипировки".into());}let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {
            let stack={let items=self.shared.items_in(dimension).lock().expect("сущности");items.mobs().iter().find(|m|m.uuid==uuid).map(|m|crate::mob::equipment::slot(m,slot))};
            if let Some(stack)=stack {return Ok(self.inventory_items(&[stack]).pop().flatten());}
        }Err("Живая сущность больше не существует".into())
    }
    fn set_health(&mut self, uuid: String, health: f64) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {let mut items=self.shared.items_in(dimension).lock().expect("сущности");if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&!m.is_dead()) {
            if !health.is_finite()||health<0.0||health>f64::from(mob.vitals.max_health) {return Err("Здоровье должно быть от нуля до максимума сущности".into());}
            mob.vitals.health=health as f32;if health==0.0 {mob.vitals.dead=true;}return Ok(());
        }}Err("Живая сущность больше не существует".into())
    }
    fn teleport(&mut self, uuid: String, at: types::Location) -> Result<bool,String> {
        if !valid_location(&at)||at.yaw.is_some_and(|v|!v.is_finite())||at.pitch.is_some_and(|v|!v.is_finite()) {return Err("Недопустимая позиция сущности".into());}
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let dimension=dimension_from_wit(at.dimension);
        let mut items=self.shared.items_in(dimension).lock().expect("сущности");
        if let Some(mob)=items.mobs_mut().iter_mut().find(|m|m.uuid==uuid&&!m.is_dead()) {mob.x=at.x;mob.y=at.y;mob.z=at.z;if let Some(yaw)=at.yaw {mob.yaw=yaw;}
            if let Some(pitch)=at.pitch {mob.pitch=pitch;}mob.teleports=mob.teleports.wrapping_add(1);return Ok(true);}
        Err("Перенос этой сущности или перенос между мирами пока не поддерживается".into())
    }
    fn set_arrow_critical(&mut self, uuid: String, critical: bool) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let mut arrows=self.shared.arrows.lock().expect("стрелы");
        let arrow=arrows.list.iter_mut().find(|a|a.uuid==uuid).ok_or("Стрела больше не существует")?;arrow.crit=critical;Ok(())
    }
    fn set_potion_item(&mut self, uuid: String, stack: inventory::Item) -> Result<(),String> {
        let uuid=parse_uuid(&uuid).ok_or("UUID не разобрать")?;let stack=self.inventory_stack(Some(stack))?.ok_or("Пустое зелье")?;
        if !matches!(crate::blocks::item_name(stack.item),Some("splash_potion"|"lingering_potion")) {return Err("Требуется взрывное или туманное зелье".into());}
        let mut thrown=self.shared.thrown.lock().expect("снаряды");let shot=thrown.flying.iter_mut().find(|p|p.uuid==uuid&&matches!(p.kind,crate::thrown::Kind::Splash|crate::thrown::Kind::Lingering)).ok_or("Зелье больше не существует")?;shot.stack=Some(stack.with_count(1));Ok(())
    }

    fn valid(&mut self, uuid: String) -> bool {
        let Some(uuid) = parse_uuid(&uuid) else {return false;};
        if self.shared.players.lock().expect("игроки").members().iter().any(|p| p.uuid == uuid) {return true;}
        for dimension in Dimension::ALL {
            let items = self.shared.items_in(dimension).lock().expect("сущности");
            if items.carts().iter().any(|c|c.uuid==uuid) || items.frames().iter().any(|f|f.uuid==uuid) || items.items().iter().any(|i| i.uuid == uuid) || items.mobs().iter().any(|m| m.uuid == uuid && !m.is_dead()) {return true;}
        }
        if self.shared.arrows.lock().expect("стрелы").list.iter().any(|a|a.uuid==uuid) {return true;}
        let thrown=self.shared.thrown.lock().expect("снаряды");thrown.flying.iter().any(|p|p.uuid==uuid)||thrown.clouds.iter().any(|p|p.uuid==uuid)||thrown.rockets.iter().any(|p|p.uuid==uuid)
    }
    fn velocity(&mut self, uuid: String) -> Result<entities::Vector, String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if let Some(info) = self.shared.players.lock().expect("игроки").ui.info.get(&uuid) {return Ok(vector(info.plugin_state.velocity));}
        for dimension in Dimension::ALL {
            let items = self.shared.items_in(dimension).lock().expect("сущности");
            if items.frames().iter().any(|f|f.uuid==uuid) {return Ok(vector((0.0,0.0,0.0)));}
            if let Some(item) = items.items().iter().find(|i| i.uuid == uuid) {return Ok(vector((item.vx, item.vy, item.vz)));}
            if let Some(mob) = items.mobs().iter().find(|m| m.uuid == uuid) {return Ok(vector((mob.vx, mob.vy, mob.vz)));}
        }
        if let Some(a)=self.shared.arrows.lock().expect("стрелы").list.iter().find(|a|a.uuid==uuid) {return Ok(vector((a.vx,a.vy,a.vz)));}
        {let thrown=self.shared.thrown.lock().expect("снаряды");if thrown.clouds.iter().any(|c|c.uuid==uuid) {return Ok(vector((0.0,0.0,0.0)));}
        if let Some(rocket)=thrown.rockets.iter().find(|r|r.uuid==uuid) {return Ok(vector((rocket.vx,rocket.vy,rocket.vz)));}}
        if let Some(p) = self.shared.thrown.lock().expect("снаряды").flying.iter().find(|p| p.uuid == uuid) {return Ok(vector((p.vx, p.vy, p.vz)));}
        Err("Сущность больше не существует".into())
    }
    fn set_velocity(&mut self, uuid: String, value: entities::Vector) -> Result<(), String> {
        if ![value.x, value.y, value.z].iter().all(|n| n.is_finite() && n.abs() <= 64.0) {return Err("Скорость: конечные компоненты в пределах ±64 блоков/такт".into());}
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        {let mut players = self.shared.players.lock().expect("игроки");
            if let Some(info) = players.ui.info.get_mut(&uuid) {info.plugin_state.velocity=(value.x,value.y,value.z);players.order(&uuid, Order::PluginVelocity {value:(value.x,value.y,value.z)});return Ok(());}}
        for dimension in Dimension::ALL {
            let mut items = self.shared.items_in(dimension).lock().expect("сущности");
            if let Some(item) = items.items_mut().iter_mut().find(|i| i.uuid == uuid) {(item.vx,item.vy,item.vz)=(value.x,value.y,value.z);return Ok(());}
            if let Some(mob) = items.mobs_mut().iter_mut().find(|m| m.uuid == uuid) {(mob.vx,mob.vy,mob.vz)=(value.x,value.y,value.z);mob.impulses=mob.impulses.wrapping_add(1);return Ok(());}
        }
        if let Some(a)=self.shared.arrows.lock().expect("стрелы").list.iter_mut().find(|a|a.uuid==uuid) {(a.vx,a.vy,a.vz)=(value.x,value.y,value.z);return Ok(());}
        if let Some(rocket)=self.shared.thrown.lock().expect("снаряды").rockets.iter_mut().find(|r|r.uuid==uuid) {(rocket.vx,rocket.vy,rocket.vz)=(value.x,value.y,value.z);return Ok(());}
        if let Some(p) = self.shared.thrown.lock().expect("снаряды").flying.iter_mut().find(|p| p.uuid == uuid) {(p.vx,p.vy,p.vz)=(value.x,value.y,value.z);return Ok(());}
        Err("Сущность больше не существует".into())
    }
    fn remove(&mut self, uuid: String) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if self.shared.players.lock().expect("игроки").members().iter().any(|p| p.uuid == uuid) {return Err("Игрока нельзя удалить как сущность; используйте kickPlayer".into());}
        for dimension in Dimension::ALL {
            let mut items = self.shared.items_in(dimension).lock().expect("сущности");
            items.remove_items(|i| i.uuid == uuid);items.remove_mobs(|m| m.uuid == uuid);items.remove_frames(|f|f.uuid==uuid);
        }
        self.shared.arrows.lock().expect("стрелы").list.retain(|a|a.uuid!=uuid);
        let mut thrown=self.shared.thrown.lock().expect("снаряды");thrown.flying.retain(|p|p.uuid!=uuid);thrown.clouds.retain(|p|p.uuid!=uuid);thrown.rockets.retain(|p|p.uuid!=uuid);Ok(())
    }
    fn drop_item(&mut self, at: types::Location, stack: inventory::Item, natural: bool) -> Result<String, String> {
        if !valid_location(&at) {return Err("Недопустимая позиция предмета".into());}
        let stack = self.inventory_stack(Some(stack))?.ok_or("Пустой предмет")?;
        let dimension = dimension_from_wit(at.dimension);
        let (x,y,z) = if natural {
            let mut world = self.shared.world_in(dimension).lock().expect("мир");
            (at.x+0.25+f64::from(world.random_unit())*0.5, at.y+0.25+f64::from(world.random_unit())*0.5, at.z+0.25+f64::from(world.random_unit())*0.5)
        } else {(at.x,at.y,at.z)};
        let mut items = self.shared.items_in(dimension).lock().expect("предметы");
        let id = self.shared.next_entity_id();items.drop_item(id,stack,(x,y,z),(0.0,0.2,0.0),10);
        Ok(crate::network::login::format_uuid(&items.items().iter().find(|i| i.entity_id == id).expect("созданный предмет").uuid))
    }
    fn item_stack(&mut self, uuid: String) -> Result<inventory::Item, String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {
            let stack = self.shared.items_in(dimension).lock().expect("предметы").items().iter().find(|i| i.uuid == uuid && i.fuse.is_none() && i.block.is_none()).map(|i| i.stack);
            if let Some(stack) = stack {return self.inventory_items(&[Some(stack)]).pop().flatten().ok_or("Неизвестный предмет".into());}
        }
        Err("Предмет больше не существует".into())
    }
    fn set_item_stack(&mut self, uuid: String, stack: inventory::Item) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let stack = self.inventory_stack(Some(stack))?.ok_or("Пустой предмет")?;
        for dimension in Dimension::ALL {if self.shared.items_in(dimension).lock().expect("предметы").plugin_set_item(uuid,stack) {return Ok(());}}
        Err("Предмет больше не существует".into())
    }
    fn item_location(&mut self, uuid: String) -> Result<types::Location, String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        for dimension in Dimension::ALL {
            let items=self.shared.items_in(dimension).lock().expect("предметы");
            if let Some(i)=items.items().iter().find(|i| i.uuid == uuid) {return Ok(types::Location {dimension:dimension_to_wit(dimension),x:i.x,y:i.y,z:i.z,yaw:None,pitch:None});}
        }
        Err("Предмет больше не существует".into())
    }
    fn item_teleport(&mut self, uuid: String, at: types::Location) -> Result<bool, String> {
        if !valid_location(&at) {return Err("Недопустимая позиция предмета".into());}
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let mut items=self.shared.items_in(dimension_from_wit(at.dimension)).lock().expect("предметы");
        if let Some(i)=items.items_mut().iter_mut().find(|i| i.uuid == uuid) {(i.x,i.y,i.z)=(at.x,at.y,at.z);return Ok(true);}
        Err("Перенос предметов между измерениями пока не поддерживается".into())
    }
}
