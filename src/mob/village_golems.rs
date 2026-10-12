// Вики, Iron Golem / Villager: «slept in the last 20 minutes»,
// «3 villagers» при панике, «5 villagers» при общении, cooldown 600 тактов.
// [проверить] Поиск кровати и общение пока упрощены: полноценного расписания
// сна/сплетен у жителей нет. Здесь память, кворум и проверка места появления.
use super::{blocks, Kind, Mob, Rng, View};

#[derive(Clone, Debug, Default)]
pub struct State {
    pub slept: Option<i64>,
    pub cooldown: u32,
    pub panic: bool,
    /// Изголовье кровати, в которой житель лежит сейчас (не сохраняется).
    pub bed: Option<(i32,i32,i32)>,
    /// Паника началась в этом такте (проверка кворума идёт сразу, см. `step`).
    pub edge: bool,
}

/// Вики, Villager/ED: память «minecraft:last_slept» хранит такт сна.
pub fn apply_data(mob: &mut Mob, data: &str) {
    if mob.kind != Kind::Villager { return; }
    if let Some(value) = crate::snbt::parse(data)
        .and_then(|v| v.get("Brain")?.get("memories")?.get("minecraft:last_slept")?.get("value")?.as_f64())
    {
        mob.villager.summon.slept = Some(value as i64);
    }
}

fn near(a: &Mob, b: &Mob, range: f64) -> bool {
    (a.x-b.x).abs() <= range && (a.y-b.y).abs() <= range && (a.z-b.z).abs() <= range
}

fn eligible(m: &Mob, now: i64) -> bool {
    m.kind == Kind::Villager && !m.is_dead() && !m.no_ai && m.villager.summon.cooldown == 0
        && m.villager.summon.slept.is_some_and(|t| (0..24_000).contains(&(now-t)))
}

fn threat(kind: Kind) -> f64 {
    match kind {
        Kind::Zombie | Kind::Husk | Kind::Drowned | Kind::ZombieVillager | Kind::Vex => 8.0,
        Kind::Vindicator | Kind::Zoglin => 10.0,
        Kind::Evoker | Kind::Illusioner | Kind::Ravager => 12.0,
        Kind::Pillager => 15.0,
        _ => 0.0,
    }
}

// Големы деревни могут появляться в воде (вики, Iron Golem). Общая
// проверка сухого появления здесь не подходит. [проверить] неполные формы.
fn space(view: &dyn View, at: (f64,f64,f64)) -> bool {
    let half=Kind::IronGolem.width()/2.0;
    for x in (at.0-half).floor() as i32..=(at.0+half).floor() as i32 {
        for z in (at.2-half).floor() as i32..=(at.2+half).floor() as i32 {
            if !view.loaded(x.div_euclid(16),z.div_euclid(16)) { return false; }
            for y in at.1.floor() as i32..(at.1+Kind::IronGolem.height()).ceil() as i32 {
                if super::solid(view.block(x,y,z)) { return false; }
            }
        }
    }
    true
}

pub fn step(mobs: &mut [Mob], view: &dyn View, now: i64, day: i64, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    let mut claimed = Vec::new();
    for i in 0..mobs.len() {
        if mobs[i].kind != Kind::Villager || mobs[i].is_dead() || mobs[i].no_ai { continue; }
        // Чёрный ящик, gaps2-villager-golem: зомби за барьером всё равно пугает —
        // прямая видимость не нужна, достаточно расстояния.
        let panic = mobs.iter().any(|m| {
            let range = threat(m.kind);
            !m.is_dead() && range != 0.0 && near(&mobs[i],m,range)
        });
        let detected = mobs[i].ticks.is_multiple_of(200) && mobs.iter().any(|m| m.kind == Kind::IronGolem && !m.is_dead() && near(&mobs[i],m,16.0));
        let m = &mut mobs[i];
        m.villager.summon.cooldown = m.villager.summon.cooldown.saturating_sub(1);
        if detected { m.villager.summon.cooldown = 600; }
        // Чёрный ящик, gaps2-villager-golem: оригинал ставит голема через 30–60 тактов
        // после появления зомби, пока жители ещё рядом; разбегающиеся у нас выходят
        // из десяти блоков, поэтому кворум проверяется и в такт начала паники.
        // [проверить] Точный такт проверки.
        m.villager.summon.edge = panic && !m.villager.summon.panic;
        m.villager.summon.panic = panic;
        if panic { m.brain.flee(20); }
        // Чёрный ящик, gaps2-villager-golem: лёгший в кровать лежит на месте до утра
        // и просыпается от паники; запоминает такт, когда проснулся (last_slept).
        // [проверить] Маршрут к далёкой кровати, поза сна, занятая кровать — не
        // сделаны; ложится только тот, кто стоит у свободного изголовья.
        let night = (12_000..24_000).contains(&day);
        if let Some((bx,_,bz)) = m.villager.summon.bed {
            if panic || !night {
                m.villager.summon.bed = None;
                m.villager.summon.slept = Some(now);
            } else {
                m.brain.stop();
                m.x = f64::from(bx) + 0.5; m.z = f64::from(bz) + 0.5;
                m.vx = 0.0; m.vz = 0.0;
                continue;
            }
        }
        if !panic && night {
            let (x,y,z) = m.cell();
            'bed: for bx in x-1..=x+1 { for by in y-1..=y { for bz in z-1..=z+1 {
                let state = view.block(bx,by,bz);
                if blocks::block_at_state(state).is_some_and(|n| n.ends_with("_bed"))
                    && blocks::orientation_of(state).and_then(|o| o.value(state,"part")) == Some("head")
                    && !claimed.contains(&(bx,by,bz))
                {
                    claimed.push((bx,by,bz));
                    m.villager.summon.slept = Some(now);
                    m.villager.summon.bed = Some((bx,by,bz));
                    m.brain.stop();
                    break 'bed;
                }
            } } }
        }
    }
    let mut born = Vec::new();
    for i in 0..mobs.len() {
        if !eligible(&mobs[i],now) { continue; }
        let panic = mobs[i].villager.summon.panic;
        if !(mobs[i].villager.summon.edge || mobs[i].ticks.is_multiple_of(if panic { 100 } else { 1200 })) { continue; }
        let count = mobs.iter().filter(|m| eligible(m,now) && near(&mobs[i],m,10.0)).count();
        if count < if panic { 3 } else { 5 } { continue; }
        let (x,y,z) = mobs[i].cell();
        let mut at = None;
        // Вики: десять попыток в объёме 17×13×17; [проверить] выбор пола.
        for _ in 0..10 {
            let (bx,bz) = (x-8+rng.below(17) as i32,z-8+rng.below(17) as i32);
            for by in (y-6..=y+6).rev() {
                let p = (f64::from(bx)+0.5,f64::from(by),f64::from(bz)+0.5);
                if blocks::solid_top(view.block(bx,by-1,bz)) && space(view,p) {
                    at = Some(p); break;
                }
            }
            if at.is_some() { break; }
        }
        if let Some(at) = at {
            born.push(Mob::new(next_id(),Kind::IronGolem,at));
            let origin = mobs[i].clone();
            for m in mobs.iter_mut().filter(|m| m.kind == Kind::Villager && near(&origin,m,10.0)) {
                m.villager.summon.cooldown = 600;
            }
        }
    }
    born
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Floor;
    impl View for Floor {
        fn block(&self, _:i32,y:i32,_:i32)->i32 { if y==0 { blocks::state_by_name("stone").unwrap() } else { 0 } }
        fn loaded(&self,_:i32,_:i32)->bool { true }
    }
    fn villagers(n: i32) -> Vec<Mob> {
        (0..n).map(|i| { let mut m=Mob::new(i+1,Kind::Villager,(i as f64,1.0,0.0));m.villager.summon.slept=Some(100);m.ticks=1200;m }).collect()
    }
    #[test]
    fn sleep_memory_survives_save_and_unslept_or_no_ai_villagers_do_not_count() {
        let mut m=villagers(1).remove(0);m.villager.summon.cooldown=345;
        let mut bytes=Vec::new();crate::villager::write(&m.villager,&mut bytes);
        let saved=crate::villager::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(saved.summon.slept,Some(100));assert_eq!(saved.summon.cooldown,345);
        m.villager.summon.cooldown=0;assert!(eligible(&m,101));
        m.no_ai=true;assert!(!eligible(&m,101));
        m.no_ai=false;m.villager.summon.slept=None;assert!(!eligible(&m,101));
    }

    #[test]
    fn quorum_sleep_detection_and_cooldown_gate_summoning() {
        let mut rng=Rng::new(42);
        let mut four=villagers(4);
        assert!(step(&mut four,&Floor,200,200,&mut rng,&mut || 99).is_empty());
        let mut five=villagers(5);
        assert_eq!(step(&mut five,&Floor,200,200,&mut rng,&mut || 99).len(),1);
        assert!(five.iter().all(|m|m.villager.summon.cooldown==600));
        assert!(step(&mut five,&Floor,201,201,&mut rng,&mut || 99).is_empty());
        let mut stale=villagers(5);
        assert!(step(&mut stale,&Floor,24_100,24_100,&mut rng,&mut || 99).is_empty());
        let mut three=villagers(3);
        three.push(Mob::new(50,Kind::Zombie,(2.0,1.0,2.0)));
        assert_eq!(step(&mut three,&Floor,200,200,&mut rng,&mut || 99).len(),1);
        let mut guarded=villagers(5);
        guarded.push(Mob::new(50,Kind::IronGolem,(2.0,1.0,2.0)));
        assert!(step(&mut guarded,&Floor,200,200,&mut rng,&mut || 99).is_empty());
    }
    struct Bed;
    impl View for Bed {
        fn block(&self, x:i32,y:i32,z:i32)->i32 {
            if (x,y,z)==(0,1,0) { blocks::state_from_text("red_bed[part=head,facing=south]").unwrap() }
            else if y==0 { blocks::state_by_name("stone").unwrap() } else { 0 }
        }
        fn loaded(&self,_:i32,_:i32)->bool { true }
    }
    #[test]
    fn villager_lies_in_the_bed_through_the_night_and_wakes_at_day_or_panic() {
        let mut rng=Rng::new(1);
        let mut v=vec![Mob::new(1,Kind::Villager,(0.5,1.0,0.5))];
        step(&mut v,&Bed,100,18_000,&mut rng,&mut || 9);
        assert_eq!(v[0].villager.summon.bed,Some((0,1,0)));
        v[0].x=0.9;
        step(&mut v,&Bed,101,18_001,&mut rng,&mut || 9);
        assert_eq!(v[0].x,0.5, "лежащий остаётся на кровати");
        step(&mut v,&Bed,300,6_000,&mut rng,&mut || 9);
        assert_eq!(v[0].villager.summon.bed,None);
        assert_eq!(v[0].villager.summon.slept,Some(300), "память о сне — такт пробуждения");
        let mut v=vec![Mob::new(1,Kind::Villager,(0.5,1.0,0.5)),Mob::new(2,Kind::Zombie,(3.5,1.0,0.5))];
        step(&mut v,&Bed,100,18_000,&mut rng,&mut || 9);
        assert_eq!(v[0].villager.summon.bed,None,"паника будит: рядом зомби");
    }
    #[test]
    fn panic_start_checks_the_quorum_at_once_without_line_of_sight() {
        let mut rng=Rng::new(3);
        let mut three=villagers(3);
        for m in &mut three { m.ticks=7; }
        three.push(Mob::new(50,Kind::Zombie,(2.0,1.0,2.0)));
        assert_eq!(step(&mut three,&Floor,200,6_000,&mut rng,&mut || 99).len(),1);
    }
}
