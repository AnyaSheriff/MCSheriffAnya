// Сущности принадлежат миру; гость получает UUID и проверенные копии предметов.
use super::api::{PluginCtx, parse_uuid, dimension_from_wit, dimension_to_wit, mcsa::api::{entities, inventory, types}};
use crate::{dimension::Dimension, players::Order};

fn valid_location(at: &types::Location) -> bool {
    at.x.is_finite() && at.z.is_finite() && at.x.abs() <= 30_000_000.0 && at.z.abs() <= 30_000_000.0 && at.y.is_finite() && (-2048.0..=2048.0).contains(&at.y)
}
fn vector(value: (f64, f64, f64)) -> entities::Vector {entities::Vector {x: value.0, y: value.1, z: value.2}}
impl entities::Host for PluginCtx {
    fn valid(&mut self, uuid: String) -> bool {
        let Some(uuid) = parse_uuid(&uuid) else {return false;};
        if self.shared.players.lock().expect("игроки").members().iter().any(|p| p.uuid == uuid) {return true;}
        for dimension in Dimension::ALL {
            let items = self.shared.items_in(dimension).lock().expect("сущности");
            if items.items().iter().any(|i| i.uuid == uuid) || items.mobs().iter().any(|m| m.uuid == uuid && !m.is_dead()) {return true;}
        }
        self.shared.thrown.lock().expect("снаряды").flying.iter().any(|p| p.uuid == uuid)
    }
    fn velocity(&mut self, uuid: String) -> Result<entities::Vector, String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if let Some(info) = self.shared.players.lock().expect("игроки").ui.info.get(&uuid) {return Ok(vector(info.plugin_state.velocity));}
        for dimension in Dimension::ALL {
            let items = self.shared.items_in(dimension).lock().expect("сущности");
            if let Some(item) = items.items().iter().find(|i| i.uuid == uuid) {return Ok(vector((item.vx, item.vy, item.vz)));}
            if let Some(mob) = items.mobs().iter().find(|m| m.uuid == uuid) {return Ok(vector((mob.vx, mob.vy, mob.vz)));}
        }
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
        if let Some(p) = self.shared.thrown.lock().expect("снаряды").flying.iter_mut().find(|p| p.uuid == uuid) {(p.vx,p.vy,p.vz)=(value.x,value.y,value.z);return Ok(());}
        Err("Сущность больше не существует".into())
    }
    fn remove(&mut self, uuid: String) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if self.shared.players.lock().expect("игроки").members().iter().any(|p| p.uuid == uuid) {return Err("Игрока нельзя удалить как сущность; используйте kickPlayer".into());}
        for dimension in Dimension::ALL {
            let mut items = self.shared.items_in(dimension).lock().expect("сущности");
            items.remove_items(|i| i.uuid == uuid);items.remove_mobs(|m| m.uuid == uuid);
        }
        self.shared.thrown.lock().expect("снаряды").flying.retain(|p| p.uuid != uuid);Ok(())
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
