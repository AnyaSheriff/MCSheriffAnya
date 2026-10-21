// Стойка для брони: Wiki, Armor Stand — «two quick punches».
// Поля NBT и позы из описания Entity data, сериализаторы minecraft-data.
// [проверить] точный выбор ячейки и поведение стрел/взрывов.
use super::{Mob,Kind};
use crate::{inventory::Stack,snbt::Value};
#[derive(Clone,Debug,PartialEq)]
pub struct State {
    pub small: bool, pub arms: bool, pub no_base: bool, pub invisible: bool, pub marker: bool, pub no_gravity: bool,
    pub pose: [[f32;3];6], pub equipment: [Option<Stack>;6], pub last_hit: Option<u64>, pub creative_break: bool,
    /// Такт моба, с которого клиенту уходят модификаторы брони (не хранится): чёрный ящик
    /// armorstand-equip-break — надели +2 такта, сняли +1. [проверить]
    pub armor_due: Option<u64>,
    /// Запреты ячеек (`DisabledSlots`, вики Armor Stand): биты 0–5 — менять/ставить, 8–13 — снимать/менять, 16–21 — только ставить;
    /// порядок ячеек: рука, обувь, штаны, нагрудник, шлем, вторая рука.
    pub disabled: i32,
}
impl Default for State {
    fn default()->Self { Self {small:false,arms:false,no_base:false,invisible:false,marker:false,no_gravity:false,
        pose:[[0.0;3],[0.0;3],[-10.0,0.0,-10.0],[-15.0,0.0,10.0],[-1.0,0.0,-1.0],[1.0,0.0,1.0]],
        equipment:[None;6],last_hit:None,creative_break:false,armor_due:None,disabled:0} }
}
impl State {
    pub fn flags(&self)->u8 { u8::from(self.small)|(u8::from(self.arms)<<2)|(u8::from(self.no_base)<<3)|(u8::from(self.marker)<<4) }
    pub fn write(&self,out:&mut Vec<u8>) {
        out.push(self.flags()|(u8::from(self.invisible)<<5)|(u8::from(self.no_gravity)<<6));
        for p in self.pose { for v in p { out.extend(v.to_be_bytes()); } }
        out.extend(self.disabled.to_be_bytes());
        for s in self.equipment { out.push(u8::from(s.is_some())); if let Some(s)=s { out.extend(s.stored_item().to_be_bytes()); out.extend(s.count.to_be_bytes()); s.write_extras(out); } }
    }
    pub fn read(bytes:&mut &[u8])->Option<Self> {
        let mut take=|n| {let (a,b)=bytes.split_at_checked(n)?;*bytes=b;Some(a)};
        let f=take(1)?[0]; let mut s=Self{small:f&1!=0,arms:f&4!=0,no_base:f&8!=0,marker:f&16!=0,invisible:f&32!=0,no_gravity:f&64!=0,..Self::default()};
        for p in &mut s.pose {for v in p { *v=f32::from_be_bytes(take(4)?.try_into().ok()?);if !v.is_finite(){return None;} }}
        s.disabled=i32::from_be_bytes(take(4)?.try_into().ok()?);
        for slot in &mut s.equipment {if take(1)?[0]!=0 {
            let raw=i32::from_be_bytes(take(4)?.try_into().ok()?);let count=i32::from_be_bytes(take(4)?.try_into().ok()?);
            let extra=Stack::read_extras_strict(raw,&mut take)?; *slot=Some(Stack::from_stored_strict(raw,count,extra)?);
        }} Some(s)
    }
}
pub fn apply(mob:&mut Mob,text:&str) {
    if mob.kind!=Kind::ArmorStand {return;}
    let Some(v)=crate::snbt::parse(text) else{return;}; let s=&mut mob.stand;
    for (key,flag) in [("Small",&mut s.small),("ShowArms",&mut s.arms),("NoBasePlate",&mut s.no_base),("Invisible",&mut s.invisible),("Marker",&mut s.marker),("NoGravity",&mut s.no_gravity)] {
        if let Some(v)=v.get(key) { *flag=v.as_f64().is_some_and(|n|n!=0.0)||v.as_str()==Some("true"); }
    }
    if let Some(pose)=v.get("Pose") {
        for (i,key) in ["Head","Body","LeftArm","RightArm","LeftLeg","RightLeg"].iter().enumerate() {
            if let Some(Value::List(values))=pose.get(key) && values.len()==3 {
                for (j,v) in values.iter().enumerate() { if let Some(n)=v.as_f64().filter(|n|n.is_finite()) {s.pose[i][j]=n as f32;} }
            }
        }
    }
    // Старые ArmorItems/HandItems оригинал в призыве не читает (чёрный ящик armorstand-nbt: ни снаряжения, ни пакета) — только `equipment`.
    if let Some(n)=v.get("DisabledSlots").and_then(Value::as_f64) {s.disabled=n as i32;}
    if let Some(equipment)=v.get("equipment") {
        for (key,slot) in [("mainhand",0),("offhand",1),("feet",2),("legs",3),("chest",4),("head",5)] {
            if let Some(item)=equipment.get(key) {s.equipment[slot]=crate::commands::item_from_text(&format!("{{Item:{}}}",crate::commands::component_text(item))).map(|s|s.with_count(1));}
        }
    }
    mob.attr_changes=mob.attr_changes.wrapping_add(1);
}
pub fn equip(mob:&mut Mob,held:&mut Option<Stack>,creative:bool)->bool {
    equip_at(mob,held,creative,None,0)
}
pub fn equip_at(mob:&mut Mob,held:&mut Option<Stack>,creative:bool,hit_y:Option<f64>,hand:usize)->bool {
    let s=&mut mob.stand; if s.marker || s.invisible {return false;}
    let slot=held.and_then(|s|crate::blocks::item_name(s.item)).map(|n| {
        if n.ends_with("_boots") {2} else if n.ends_with("_leggings") {3} else if n.ends_with("_chestplate")||n=="elytra" {4}
        else if n.ends_with("_helmet")||n.ends_with("_head")||n.ends_with("_skull")||n=="carved_pumpkin" {5} else {hand}
    }).unwrap_or_else(|| {
        let Some(y)=hit_y.map(|y|if s.small {y*2.0} else {y}) else {return (2..6).rev().find(|i|s.equipment[*i].is_some()).unwrap_or(hand);};
        // Wiki, Armor Stand: «clicking on a specific part». [проверить] границы.
        if (0.1..0.55).contains(&y) && s.equipment[2].is_some() {2}
        else if (0.9..1.6).contains(&y) && s.equipment[4].is_some() {4}
        else if (0.4..1.2).contains(&y) && s.equipment[3].is_some() {3}
        else if y>=1.6 && s.equipment[5].is_some() {5}
        else {hand}
    });
    if slot<2 && !s.arms {return false;}
    // Ячейка запретов: рука 0, обувь 1, штаны 2, нагрудник 3, шлем 4, вторая рука 5.
    let bit=match slot {0=>0,1=>5,n=>n-1};
    let (adding,changing,removing)=(s.disabled>>(16+bit)&1!=0,s.disabled>>bit&1!=0,s.disabled>>(8+bit)&1!=0);
    match (*held,s.equipment[slot]) {
        (Some(_),None) if changing||adding => return false,
        (Some(_),Some(_)) if changing||removing => return false,
        (None,Some(_)) if removing => return false,
        _=>{}
    }
    let placed=match (*held,s.equipment[slot]) {
        (Some(item),None)=> {s.equipment[slot]=Some(item.with_count(1));if !creative {*held=crate::inventory::take_one(*held);}Some(item)}
        (Some(item),Some(old)) if item.count==1=>{s.equipment[slot]=Some(item);*held=Some(old);Some(item)}
        (None,Some(old))=>{*held=Some(old);s.equipment[slot]=None;None}
        _=>return false,
    };
    // Модификаторы брони уходят позже самой экипировки (чёрный ящик armorstand-equip-break: +2 надели, +1 сняли).
    s.armor_due=Some(mob.ticks+if armor_mods(s).is_empty() {1} else {2});
    // Звук надевания — у надетой вещи; снятие беззвучно (там же).
    if let Some(item)=placed && let Some(info)=crate::combat_table::ITEMS.iter().find(|i|i.item==item.item) && !info.equip_sound.is_empty() {
        mob.events.push(super::Event::Sound{name:info.equip_sound,volume:1.0,pitch:1.0});
    }
    mob.attr_changes=mob.attr_changes.wrapping_add(1); true
}
/// Модификаторы атрибутов от надетой брони: (атрибут, имя, прибавка) — порядок: обувь, штаны, нагрудник, шлем.
/// Из таблицы боевых свойств предметов (место 4–7 — группа ячеек обувь…шлем).
pub fn armor_mods(s:&State)->Vec<(i32,&'static str,f64)> {
    let mut out=Vec::new();
    for (slot,group) in [(2,4),(3,5),(4,6),(5,7)] {
        let Some(stack)=s.equipment[slot] else {continue;};
        let Some(info)=crate::combat_table::ITEMS.iter().find(|i|i.item==stack.item) else {continue;};
        out.extend(info.mods.iter().filter(|m|m.slot==group).map(|m|(m.attribute,m.id,m.amount)));
    }
    out
}
/// Урон стойке: в выживании первый удар только качает (событие 32), второй в течение пяти тактов
/// ломает; творческий, стрела и смерть-команда ломают сразу (Wiki, Armor Stand — «two quick punches»).
/// Урона-события, опыта и смерти нет: клиентам уходят звук и частицы поломки (чёрный ящик armorstand-equip-break).
pub fn hurt(mob:&mut Mob,amount:f32,source:crate::survival::Source)->Option<crate::survival::Hit> {
    use crate::survival::Source;
    if source!=Source::GenericKill && (mob.stand.marker || mob.stand.invisible) {return None;}
    if mob.is_dead() {return None;}
    let hit=|died|Some(crate::survival::Hit{taken:0.0,absorbed:0.0,resisted:0.0,died});
    // Vanilla enchant-mob-equipment-drops: /kill удаляет стойку без её
    // предмета, снаряжения, звука и частиц обычной поломки.
    if source == Source::GenericKill {
        mob.vitals.health=0.0; mob.vitals.dead=true; mob.dead_for=1; mob.h.despawn=true;
        return hit(true);
    }
    let punch=matches!(source,Source::PlayerAttack|Source::MaceSmash);
    let breaks=if punch && !mob.stand.creative_break {
        if mob.stand.last_hit.is_none_or(|t|mob.ticks.saturating_sub(t)>5) {
            mob.stand.last_hit=Some(mob.ticks);
            mob.events.push(super::Event::Status(32));
            return hit(false);
        }
        true
    } else if source==Source::Arrow || mob.stand.creative_break {true} else {
        mob.vitals.health-=amount; mob.vitals.health<=0.0
    };
    if !breaks {return hit(false);}
    mob.vitals.health=0.0; mob.vitals.dead=true;
    mob.die();
    hit(true)
}
pub fn place(world:&mut crate::world::World,items:&mut crate::items::Items,shared:&crate::shared::Shared,clicked:(i32,i32,i32),face:i32,yaw:f32)->bool {
    if face==0{return false;}let (dx,dy,dz)=crate::frame::step_of(face as u8);
    let pos=(clicked.0+dx,clicked.1+dy,clicked.2+dz);
    if crate::blocks::has_box(world.get_block(pos.0,pos.1,pos.2))||crate::blocks::has_box(world.get_block(pos.0,pos.1+1,pos.2)){return false;}
    let at=(pos.0 as f64+0.5,pos.1 as f64,pos.2 as f64+0.5);
    if items.mobs().iter().any(|m|(m.x-at.0).abs()<0.5&&(m.z-at.2).abs()<0.5&&(m.y-at.1).abs()<1.975){return false;}
    let mut m=Mob::new(shared.next_entity_id(),Kind::ArmorStand,at);m.yaw=((yaw+180.0)/45.0).round()*45.0;items.add_mob(m);
    world.note_sound(pos.0,pos.1,pos.2,"minecraft:entity.armor_stand.place",0.75,0.8);true
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn pose_equipment_and_flags_survive() {
        let mut m=Mob::new(1,Kind::ArmorStand,(0.0,0.0,0.0)); apply(&mut m,"{Small:1b,ShowArms:1b,NoBasePlate:1b,Pose:{Head:[10f,20f,30f]}}");
        let mut held=Some(Stack::new(crate::blocks::item_named("iron_helmet").unwrap(),1));
        assert!(equip(&mut m,&mut held,false)); assert!(held.is_none());
        let mut b=Vec::new(); m.write(&mut b); let back=Mob::read(&mut b.as_slice()).unwrap(); m.stand.armor_due=None; assert_eq!(back.stand,m.stand);
        assert_eq!(back.stand.flags(),13); assert_eq!(back.stand.pose[0],[10.0,20.0,30.0]);
    }
    #[test] fn empty_hand_targets_the_clicked_equipment_and_two_quick_hits_break() {
        let mut m=Mob::new(1,Kind::ArmorStand,(0.0,0.0,0.0));
        let boots=Stack::new(crate::blocks::item_named("iron_boots").unwrap(),1);
        let helmet=Stack::new(crate::blocks::item_named("iron_helmet").unwrap(),1);
        m.stand.equipment[2]=Some(boots);m.stand.equipment[5]=Some(helmet);
        let mut hand=None;
        assert!(equip_at(&mut m,&mut hand,false,Some(0.3),0));
        assert_eq!(hand,Some(boots));assert_eq!(m.stand.equipment[5],Some(helmet));
        assert!(m.hurt(1.0,crate::survival::Source::PlayerAttack,None).is_some_and(|h|!h.died));assert!(!m.is_dead());
        m.ticks+=6;
        assert!(m.hurt(1.0,crate::survival::Source::PlayerAttack,None).is_some_and(|h|!h.died));assert!(!m.is_dead());
        m.ticks+=1;
        assert!(m.hurt(1.0,crate::survival::Source::PlayerAttack,None).is_some_and(|h|h.died));assert!(m.is_dead());
        assert!(m.events.iter().any(|e|matches!(e,crate::mob::Event::BlockBurst{count:10,..})));assert!(!m.events.contains(&crate::mob::Event::Death));
        let mut marker=Mob::new(2,Kind::ArmorStand,(0.0,0.0,0.0));marker.stand.marker=true;
        assert!(marker.hurt(100.0,crate::survival::Source::PlayerAttack,None).is_none());
        assert!(marker.hurt(100.0,crate::survival::Source::GenericKill,None).is_some());
    }
}
