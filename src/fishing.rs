// Рыбалка: minecraft.wiki/w/Fishing. [проверить] физика, частицы и случайные
// такты требуют замеров; сервер/сценарии здесь не запускались.
use crate::{blocks, dimension::Dimension, inventory::Stack, shared::Shared, world::World};

#[derive(Clone, Debug)]
pub struct Bobber {
    pub id: i32,
    pub owner: i32,
    pub dimension: Dimension,
    pub at: (f64, f64, f64),
    pub velocity: (f64, f64, f64),
    pub age: u32,
    pub grounded: bool,
    pub ground_ticks: u32,
    pub hooked: Option<i32>,
    pub waiting: i32,
    pub approaching: i32,
    pub bite: i32,
    pub open: bool,
    pub lure: i32,
    pub luck: f64,
    pub angle: f64,
    /// Только что заброшен: первый ход — в следующем такте (чёрный ящик, fishing-cast).
    pub fresh: bool,
    /// Состояние на начало такта: слежение шлёт его, а не конец хода
    /// (в оригинале пакет слежения идёт раньше хода сущности).
    pub prev_at: (f64, f64, f64),
    pub prev_velocity: (f64, f64, f64),
    pub prev_hooked: Option<i32>,
    pub prev_bite: bool,
    pub in_water: bool,
    /// Поклёвка меняет скорость рывком: слежение шлёт её не дожидаясь 5-го такта.
    pub force: bool,
    pub prev_force: bool,
}

impl Bobber {
    pub fn new(id: i32, owner: i32, dimension: Dimension, at: (f64,f64,f64), velocity: (f64,f64,f64), lure: i32, luck: f64) -> Self {
        Self { id, owner, dimension, at, velocity, lure, luck, age: 0, grounded: false, ground_ticks: 0, hooked: None, waiting: 0, approaching: 0, bite: 0, open: true, angle: 0.0, fresh: true, prev_at: at, prev_velocity: velocity, prev_hooked: None, prev_bite: false, in_water: false, force: false, prev_force: false }
    }
}

pub fn wait_ticks(roll: u32, lure: i32) -> i32 { 100 + (roll % 501) as i32 - lure.clamp(0, 255) * 100 }

/// «5×4×5 vicinity»: каждый слой целиком водяной или целиком воздушный;
/// над первым воздушным слоем вода уже недопустима.
pub fn open_water(w: &World, p: (i32,i32,i32)) -> bool {
    let mut air_seen = false;
    for y in p.1-1..=p.1+2 {
        let mut layer = None;
        for x in p.0-2..=p.0+2 { for z in p.2-2..=p.2+2 {
            if !w.has_chunk(x >> 4,z >> 4) { return false; }
            let s=w.get_block(x,y,z);
            let name=blocks::block_at_state(s).unwrap_or("");
            let water=(!blocks::has_box(s) && crate::fluids::fluid_at(s)==Some((crate::fluids::Kind::Water,crate::fluids::SOURCE)))
                || (name != "water" && crate::mob::is_water(s)) || (!blocks::has_box(s) && blocks::orientation_of(s).and_then(|o| o.value(s,"waterlogged"))==Some("true"));
            let air=matches!(name,"air"|"cave_air"|"void_air"|"lily_pad");
            if !water && !air { return false; }
            if layer.is_some_and(|l| l!=air) { return false; }
            layer=Some(air);
        } }
        let air=layer.unwrap_or(false);
        if !air && air_seen { return false; }
        if y==p.1-1 && air { return false; }
        air_seen |= air;
    }
    true
}

pub fn pull(from: (f64,f64,f64), to: (f64,f64,f64), loot: bool) -> (f64,f64,f64) {
    let d=(to.0-from.0,to.1-from.1,to.2-from.2);
    let lift=if loot { (d.0*d.0+d.1*d.1+d.2*d.2).sqrt().sqrt()*0.08 } else { 0.0 };
    (d.0*0.1,d.1*0.1+lift,d.2*0.1)
}

/// Веса групп: fish 85/-1, junk 10/-2, treasure 5/+2.
pub fn weights(luck: f64, open: bool) -> [u32;3] {
    [(85.0-luck).floor().max(0.0) as u32,(10.0-2.0*luck).floor().max(0.0) as u32,if open {(5.0+2.0*luck).floor().max(0.0) as u32} else {0}]
}

pub fn loot(w: &mut World, luck: f64, open: bool, jungle: bool) -> Stack {
    let weights=weights(luck,open);
    let roll=w.random_bits()%weights.iter().sum::<u32>().max(1);
    let mut count=1;
    let name=if roll<weights[0] {
        match w.random_bits()%100 {0..=59=>"cod",60..=84=>"salmon",85..=86=>"tropical_fish",_=>"pufferfish"}
    } else if roll<weights[0]+weights[1] {
        let mut junk=vec![("lily_pad",17),("bowl",10),("fishing_rod",2),("leather",10),("leather_boots",10),("rotten_flesh",10),("stick",5),("string",5),("potion",10),("bone",10),("ink_sac",1),("tripwire_hook",10)];
        if jungle { junk.push(("bamboo",10)); }
        let mut r=w.random_bits()%junk.iter().map(|(_,n)|n).sum::<u32>();
        let mut name="bowl";
        for (n,weight) in junk { if r<weight {name=n;break;} r-=weight; }
        if name=="ink_sac" {count=10;}
        name
    } else {
        ["name_tag","saddle","bow","fishing_rod","book","nautilus_shell"][(w.random_bits()%6) as usize]
    };
    let mut stack=Stack::new(blocks::item_named(name).expect("добыча рыбалки"),count);
    if roll>=weights[0]+weights[1] && matches!(name,"book"|"bow"|"fishing_rod") {
        stack = crate::enchanting::fishing_treasure(i64::from(w.random_bits()), stack);
    }
    if matches!(name,"bow"|"fishing_rod"|"leather_boots") {
        let max=crate::combat::max_damage(&stack);
        let mut data=stack.extras();
        // Junk: «10%–100% damaged»; treasure — сильно изношен.
        // [проверить] точное округление и распределение остатка прочности.
        let minimum = if roll >= weights[0] + weights[1] { 0.75 } else { 0.10 };
        data.damage = ((minimum + (1.0 - minimum) * w.random_unit()) * max as f32) as i32;
        data.damage = data.damage.min((max - 1).max(0));
        stack=stack.with_extras(data);
    }
    if name=="potion" { let mut data=stack.extras(); data.potion=crate::survival::potion_number("water"); stack=stack.with_extras(data); }
    stack
}

pub fn step(shared: &Shared, dimension: Dimension, w: &mut World) {
    let owners=shared.players.lock().expect("игроки").members().to_vec();
    let items=shared.items_in(dimension).lock().expect("предметы");
    let mut bobbers=shared.fishing.lock().expect("поплавки");
    bobbers.retain_mut(|b| {
        if b.dimension!=dimension { return true; }
        let Some(owner)=owners.iter().find(|m|m.entity_id==b.owner && m.dimension==dimension && m.game_mode!=3) else{return false;};
        let dx=owner.position.x-b.at.0; let dy=owner.position.y-b.at.1; let dz=owner.position.z-b.at.2;
        if dx*dx+dy*dy+dz*dz>1024.0 || b.at.1 < -128.0 {return false;}
        if b.fresh { b.fresh=false; return true; }
        b.prev_at=b.at; b.prev_velocity=b.velocity; b.prev_hooked=b.hooked; b.prev_bite=b.bite>0; b.prev_force=b.force; b.force=false;
        b.age+=1;
        if let Some(id)=b.hooked {
            if let Some(m)=items.mobs().iter().find(|m|m.entity_id==id && !m.vitals.dead) { b.at=(m.x,m.y+0.5,m.z);return true; }
            if let Some(i)=items.items().iter().find(|i|i.entity_id==id) { b.at=(i.x,i.y,i.z);return true; }
            if let Some(m)=owners.iter().find(|m|m.entity_id==id && m.dimension==dimension) { b.at=(m.position.x,m.position.y+0.8,m.position.z);return true; }
            b.hooked=None;
        }
        if b.grounded {
            let cell = (b.at.0.floor() as i32, b.at.1.floor() as i32, b.at.2.floor() as i32);
            if blocks::has_box(w.get_block(cell.0, cell.1, cell.2)) {
                b.ground_ticks += 1;
                return b.ground_ticks < 1200;
            }
            b.grounded = false;
            b.ground_ticks = 0;
        }
        let p=(b.at.0.floor() as i32,b.at.1.floor() as i32,b.at.2.floor() as i32);
        if !w.has_chunk(p.0>>4,p.2>>4) {return true;}
        let s=w.get_block(p.0,p.1,p.2);
        let water = crate::mob::is_water(s) || blocks::orientation_of(s).and_then(|o| o.value(s, "waterlogged")) == Some("true");
        if water {
            float(b, w, p);
        } else {
            if b.bite == 0 { b.waiting = 0; b.approaching = 0; b.open = true; }
            b.velocity.1-=0.03;
        }
        let next=(b.at.0+b.velocity.0,b.at.1+b.velocity.1,b.at.2+b.velocity.2);
        let distance=(b.velocity.0*b.velocity.0+b.velocity.1*b.velocity.1+b.velocity.2*b.velocity.2).sqrt();
        let steps=(distance/0.1).ceil().max(1.0) as i32;
        for i in 1..=steps {
            let f=i as f64/steps as f64;
            let at=(b.at.0+b.velocity.0*f,b.at.1+b.velocity.1*f,b.at.2+b.velocity.2*f);
            let cell=(at.0.floor() as i32,at.1.floor() as i32,at.2.floor() as i32);
            if blocks::has_box(w.get_block(cell.0,cell.1,cell.2)) { b.at=at; b.velocity=(0.0,0.0,0.0);b.grounded=true;return true; }
            let near=|x:f64,y:f64,z:f64| (at.0-x).abs()<0.4&&(at.1-y).abs()<0.7&&(at.2-z).abs()<0.4;
            b.hooked=items.mobs().iter().find(|m|!m.is_dead()&&near(m.x,m.y+0.6,m.z)).map(|m|m.entity_id)
                .or_else(||items.items().iter().find(|i|near(i.x,i.y,i.z)).map(|i|i.entity_id))
                .or_else(||owners.iter().find(|m|m.entity_id!=b.owner&&m.game_mode!=3&&m.dimension==dimension&&near(m.position.x,m.position.y+0.8,m.position.z)).map(|m|m.entity_id));
            if b.hooked.is_some() {b.at=at;return true;}
        }
        b.at=next;
        // Сопротивление 0,92 по всем осям — и в воздухе, и в воде (чёрный ящик,
        // fishing-cast: 1,0407·(1+0,92+0,92²+0,92³) = 3,689 блока за четыре такта).
        b.velocity.0*=0.92;b.velocity.1*=0.92;b.velocity.2*=0.92;
        // Первый такт в воде: скорость гасится сразу (там же: 0,745→0,224 и
        // −0,45→−0,09 к пакету +5) и звучит всплеск (там же, +4, громкость
        // растёт со скоростью падения).
        let in_water_now=w.has_chunk((b.at.0.floor() as i32)>>4,(b.at.2.floor() as i32)>>4) && {
            let c=w.get_block(b.at.0.floor() as i32,b.at.1.floor() as i32,b.at.2.floor() as i32);
            crate::mob::is_water(c) || blocks::orientation_of(c).and_then(|o| o.value(c, "waterlogged")) == Some("true")
        };
        if in_water_now && !b.in_water {
            let speed=(b.velocity.0*b.velocity.0+b.velocity.1*b.velocity.1+b.velocity.2*b.velocity.2).sqrt();
            let pitch=0.6+w.random_unit()*0.8;
            w.note_sound_neutral_at(b.at,"minecraft:entity.generic.splash",(speed*0.2).min(1.0) as f32,pitch);
            b.velocity.0*=0.3;b.velocity.2*=0.3;b.velocity.1*=0.2;
        }
        b.in_water=in_water_now;
        true
    });
    // Слежение за поплавками идёт тем же проходом, что и за снарядами; если
    // снарядов нет, thrown::step выходит рано и сам его не запускает.
    let active=bobbers.iter().any(|b|b.dimension==dimension);
    drop(bobbers);
    if active && !shared.thrown.lock().expect("снаряды").any_in(dimension) { w.note_thrown_tracker(false); }
}

fn float(b: &mut Bobber, w: &mut World, p: (i32, i32, i32)) {
    b.velocity.0*=0.9; b.velocity.2*=0.9;
    // Подъёмная сила: подгонка под fishing-cast (колебания около p.1+0,875).
    b.velocity.1=(b.velocity.1+(p.1 as f64+0.875-b.at.1)*0.14)*0.9;
    // Открытость считается у поверхности: нырнувший поплавок не лишает улов сокровищ.
    let mut surface=p;
    while surface.1<p.1+4 && crate::mob::is_water(w.get_block(surface.0,surface.1+1,surface.2)) { surface.1+=1; }
    b.open &= open_water(w,surface);
    let speed=1+i32::from(w.rain_at(p.0,p.1+1,p.2)&&w.random_unit()<0.25)-i32::from(!w.open_sky(p.0,p.1+1,p.2)&&w.random_unit()<0.5);
    if b.bite>0 {
        b.bite-=1; b.velocity.1-=f64::from(w.random_unit())*0.02;
    } else if b.approaching>0 {
        b.approaching-=speed;
        b.angle+=(f64::from(w.random_unit())-0.5)*0.2;
        let r=f64::from(b.approaching.max(0))*0.1;
        // Чёрный ящик, fishing-catch: каждый такт две частицы «fishing»
        // (count 0, speed 1, смещение — скорость ±0,04 поперёк потока) на
        // высоте клетки +1,0; время от времени «bubble» (смещение — направление
        // потока, 0,1 вверх) на клетке +0,9.
        let (sa,ca)=(b.angle.sin(),b.angle.cos());
        let (px,pz)=(b.at.0+sa*r,b.at.2+ca*r);
        let top=f64::from(p.1)+1.0;
        let (ox,oz)=((ca*0.04) as f32,(-sa*0.04) as f32);
        w.note_particle(31,None,(px,top,pz),(ox,0.01,oz),1.0,0);
        w.note_particle(31,None,(px,top,pz),(-ox,0.01,-oz),1.0,0);
        if w.random_unit()<0.15 { w.note_particle(3,None,(px,f64::from(p.1)+0.9,pz),(sa as f32,0.1,ca as f32),0.0,1); }
        if b.approaching<=0 {
            b.bite=20+(w.random_bits()%21) as i32;
            // Чёрный ящик, fishing-catch: в такт поклёвки поплавок ныряет на 0,35
            // (скорость в пакете −0,324 = −0,352·0,92).
            b.velocity.1 -= 0.353;
            b.force = true;
            let pitch=0.6+w.random_unit()*0.8;
            w.note_sound_neutral_at(b.at,"minecraft:entity.fishing_bobber.splash",0.25,pitch);
            let above=(b.at.0,b.at.1+0.5,b.at.2);
            w.note_particle(3,None,above,(0.25,0.0,0.25),0.2,6);
            w.note_particle(31,None,above,(0.25,0.0,0.25),0.2,6);
        }
    } else if b.waiting>0 {
        b.waiting-=speed;
        if b.waiting<=0 {b.approaching=20+(w.random_bits()%61) as i32;b.angle=f64::from(w.random_unit())*std::f64::consts::TAU;}
    } else { b.waiting=wait_ticks(w.random_bits(),b.lure); b.open=open_water(w,surface); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_reaches_bite_and_keeps_treasure_eligible_in_two_deep_pool() {
        let mut w = World::in_memory();
        for x in 0..5 { for z in 0..5 {
            w.set_block(x, 68, z, blocks::state_by_name("stone").unwrap());
            for y in 69..=70 { w.set_block(x, y, z, blocks::state_by_name("water").unwrap()); }
        } }
        let mut b = Bobber::new(2, 1, Dimension::Overworld, (2.5, 70.875, 2.5), (0.0, 0.0, 0.0), 0, 0.0);
        let mut bites = 0;
        for _ in 0..2000 {
            let p = (2, b.at.1.floor() as i32, 2);
            let old = b.bite;
            float(&mut b, &mut w, p);
            b.at.1 += b.velocity.1;
                        // Движение без столкновений, как в игре, тут не моделируется: ошибка высоты вне воды не в счёт.
            if b.bite > 0 && (69.0..71.0).contains(&b.at.1) { assert!(b.open); }
            if old == 0 && b.bite > 0 { bites += 1; }
        }
        assert!(bites >= 2);
    }

    #[test]
    fn timing_and_luck() {
        assert_eq!(wait_ticks(0,0),100); assert_eq!(wait_ticks(500,0),600);
        assert_eq!(wait_ticks(500,3),300);
        assert_eq!(weights(0.5, true), [84, 9, 6]);
        assert_eq!(weights(0.0,true),[85,10,5]);assert_eq!(weights(3.0,true),[82,4,11]);assert_eq!(weights(3.0,false),[82,4,0]);
    }
    #[test]
    fn open_water_rejects_roof_and_flowing_water() {
        let mut w=World::in_memory();
        for x in 0..5 {for z in 0..5 {for y in 69..=70 {w.set_block(x,y,z,blocks::state_by_name("water").unwrap());}}}
        assert!(open_water(&w,(2,70,2)));
        w.set_block(0, 69, 0, blocks::state_by_name("kelp_plant").unwrap());
        assert!(open_water(&w, (2, 70, 2)));
        w.set_block(0, 69, 0, blocks::state_from_text("campfire[waterlogged=true,lit=false]").unwrap());
        assert!(!open_water(&w, (2, 70, 2)));
        w.set_block(0, 69, 0, blocks::state_by_name("water").unwrap());
        w.set_block(0,72,0,blocks::state_by_name("stone").unwrap());assert!(!open_water(&w,(2,70,2)));
        w.set_block(0,72,0,0);w.set_block(0,69,0,crate::fluids::state_of(crate::fluids::Kind::Water,1));assert!(!open_water(&w,(2,70,2)));
    }
}
