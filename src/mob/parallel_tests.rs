use super::*;
struct Floor;
impl View for Floor {
    fn block(&self, _:i32,y:i32,_:i32)->i32 { if y==0 { blocks::state_by_name("stone").unwrap() } else { 0 } }
    fn loaded(&self,_:i32,_:i32)->bool { true }
}
fn bytes(mobs: &[Mob]) -> Vec<u8> { let mut out=Vec::new(); for m in mobs { m.write(&mut out); } out }
fn fixture() -> Vec<Mob> {
    (0..512).map(|i| {
        let kind=match i%8 { 0|1=>Kind::Villager,2=>Kind::Zombie,3=>Kind::IronGolem,4=>Kind::Llama,5=>Kind::Wolf,6=>Kind::PolarBear,_=>Kind::Panda };
        let mut m=Mob::new(i+1,kind,((i%32) as f64,1.0,(i/32) as f64));
        m.ticks=200; m.villager.summon.slept=Some(0);
        if i%17==0 { m.no_ai=true; }
        if i%19==0 { m.vitals.dead=true; m.vitals.health=0.0; }
        if i%7==0 { m.land.target=Some(9000+i); }
        if kind==Kind::Panda { m.land.main_gene=6; m.land.hidden_gene=6; }
        m
    }).collect()
}
#[test]
fn whole_tick_keeps_rng_ids_saved_state_and_all_outcomes() {
    let initial=fixture();
    for width in [1,2,4] {
        let mut serial=crate::items::Items::new(); let mut candidate=crate::items::Items::new();
        for m in &initial { serial.add_mob(m.clone()); candidate.add_mob(m.clone()); }
        let mut a=Rng::new(8675309); let mut b=a.clone();
        let mut aid=10000; let mut bid=10000;
        let mut executor=parallel::Executor::new(); executor.set_width(width);
        let mut jobs=0;
        for tick in 200..204 {
            executor.begin_tick();
            let env=Env { difficulty:2, sky:15, sun_up:false, clock:6000, tick };
            let expected=step_all(&mut serial,&Floor,&[],env,&mut a,&mut || { aid+=1; aid });
            let actual=step_all_with_executor(&mut candidate,&Floor,&[],env,&mut b,&mut || { bid+=1; bid },Some(&mut executor));
            assert_eq!(format!("{expected:?}"),format!("{actual:?}"));
            assert_eq!(bytes(serial.mobs()),bytes(candidate.mobs()));
            // Сохранение не включает все краткоживущие поля и очереди событий.
            assert_eq!(format!("{:?}",serial.mobs()),format!("{:?}",candidate.mobs()));
            assert_eq!(aid,bid); assert_eq!(a.0,b.0);
            jobs+=executor.stats().jobs;
        }
        assert_eq!(jobs>0,width>1);
    }
}
#[test]
fn herd_matches_original_first_match_and_strict_radius_formulas() {
    let mut mobs=fixture();
    // Первая цель в списке побеждает более близкую; ровно десять не входит.
    for m in &mut mobs { if m.kind==Kind::Wolf { m.x=1000.0; m.z=1000.0; } }
    let wolf=Mob::new(8001,Kind::Wolf,(9.5,1.0,0.0));
    mobs.insert(0,wolf);
    mobs.insert(1,Mob::new(8002,Kind::Wolf,(1.0,1.0,0.0)));
    mobs.insert(2,Mob::new(8003,Kind::Wolf,(10.0,1.0,0.0)));
    mobs.push(Mob::new(8100,Kind::Llama,(0.0,1.0,0.0)));
    mobs.push(Mob::new(8101,Kind::Llama,(10000.0,1.0,0.0)));
    mobs.push(Mob::new(8102,Kind::Wolf,(10010.0,1.0,0.0)));
    mobs.push(Mob::new(8103,Kind::PolarBear,(20000.0,1.0,0.0)));
    let mut hurt_at_edge=Mob::new(8104,Kind::PolarBear,(20008.0,1.0,0.0));hurt_at_edge.land.target=Some(12345);mobs.push(hurt_at_edge);
    let wolves: Vec<_>=mobs.iter().filter(|m|m.kind==Kind::Wolf&&!m.tame.tamed()&&!m.is_dead()).map(|m|(m.entity_id,m.x,m.z)).collect();
    let hurt: Vec<_>=mobs.iter().filter_map(|m|m.land.target.map(|t|(m.kind,m.x,m.z,t))).collect();
    let expected: Vec<_>=mobs.iter().map(|m| {
        let mut target=m.land.target;
        if land::llama(m.kind)&&target.is_none() {target=wolves.iter().find(|w|(w.1-m.x).hypot(w.2-m.z)<10.0).map(|w|w.0);}
        if !m.is_baby()&&(m.kind==Kind::PolarBear||m.kind==Kind::Panda&&m.land.personality()==6) {target=target.or_else(||hurt.iter().find(|h|h.0==m.kind&&(h.1-m.x).hypot(h.2-m.z)<8.0).map(|h|h.3));}
        target
    }).collect();
    let mut executor=parallel::Executor::new();executor.set_width(4);
    land::herd_with_executor(&mut mobs,&[],&Floor,Some(&mut executor));
    assert!(executor.stats().jobs>0);
    assert_eq!(mobs.iter().map(|m|m.land.target).collect::<Vec<_>>(),expected);
    assert_eq!(mobs.iter().find(|m|m.entity_id==8100).unwrap().land.target,Some(8001));
    assert_eq!(mobs.iter().find(|m|m.entity_id==8101).unwrap().land.target,None);
    assert_eq!(mobs.iter().find(|m|m.entity_id==8103).unwrap().land.target,None);
}
#[test]
fn villagers_match_original_ranges_detection_and_sleeping_position_changes() {
    let mut mobs=fixture();
    // Сдвиг спящего жителя не должен менять снимок угроз следующего жителя.
    for (i,m) in mobs.iter_mut().enumerate() {
        if m.kind==Kind::Villager { m.villager.summon.bed=Some((1000+i as i32,1,1000)); }
    }
    let expected: Vec<_>=mobs.iter().map(|v| {
        let panic=mobs.iter().any(|m| {
            let r=match m.kind {Kind::Zombie|Kind::Husk|Kind::Drowned|Kind::ZombieVillager|Kind::Vex=>8.0,Kind::Vindicator|Kind::Zoglin=>10.0,Kind::Evoker|Kind::Illusioner|Kind::Ravager=>12.0,Kind::Pillager=>15.0,_=>0.0};
            !m.is_dead()&&r!=0.0&&(v.x-m.x).abs()<=r&&(v.y-m.y).abs()<=r&&(v.z-m.z).abs()<=r
        });
        let detected=v.ticks.is_multiple_of(200)&&mobs.iter().any(|m|m.kind==Kind::IronGolem&&!m.is_dead()&&(v.x-m.x).abs()<=16.0&&(v.y-m.y).abs()<=16.0&&(v.z-m.z).abs()<=16.0);
        (panic,detected)
    }).collect();
    let mut serial=mobs.clone();let mut a=Rng::new(42);let mut b=a.clone();let mut aid=10000;let mut bid=10000;
    let mut executor=parallel::Executor::new();executor.set_width(4);
    let born_a=village_golems::step(&mut serial,&Floor,200,18000,&mut a,&mut ||{aid+=1;aid});
    let born_b=village_golems::step_with_executor(&mut mobs,&Floor,200,18000,&mut b,&mut ||{bid+=1;bid},Some(&mut executor));
    assert!(executor.stats().jobs>0);
    assert_eq!(bytes(&serial),bytes(&mobs));assert_eq!(bytes(&born_a),bytes(&born_b));assert_eq!(a.0,b.0);assert_eq!(aid,bid);
    for (m,(panic,detected)) in mobs.iter().zip(expected) {
        if m.kind==Kind::Villager&&!m.is_dead()&&!m.no_ai { assert_eq!(m.villager.summon.panic,panic);if detected {assert_eq!(m.villager.summon.cooldown,600);} }
    }
}

// Отдельный замер наблюдения внутри одного измерения; не проверка TPS и не порог CI.
#[test]
#[ignore]
fn sensing_benchmark() {
    let initial: Vec<_>=(0..8192).map(|i| {
        let kind=if i<4096 {Kind::Llama}else{Kind::Wolf};
        let x=if i<4096 {i as f64}else{100000.0+i as f64};
        Mob::new(i+1,kind,(x,1.0,0.0))
    }).collect();
    let mut executor=parallel::Executor::new();executor.set_width(2);
    let mut warm=initial.clone();land::herd_with_executor(&mut warm,&[],&Floor,Some(&mut executor));
    let mut serial_ns=0u128;let mut parallel_ns=0u128;let mut jobs=0;
    for _ in 0..3 {
        let mut serial=initial.clone();let mut candidate=initial.clone();
        let start=std::time::Instant::now();land::herd(&mut serial,&[],&Floor);serial_ns+=start.elapsed().as_nanos();
        executor.begin_tick();
        let start=std::time::Instant::now();land::herd_with_executor(&mut candidate,&[],&Floor,Some(&mut executor));parallel_ns+=start.elapsed().as_nanos();
        assert_eq!(bytes(&serial),bytes(&candidate));
        assert_eq!(format!("{serial:?}"),format!("{candidate:?}"));
        jobs+=executor.stats().jobs;
    }
    assert!(jobs>0);
    println!("{{\"benchmark\":\"mob_sensing_same_dimension\",\"llamas\":4096,\"wolves\":4096,\"repetitions\":3,\"available_cpus\":{},\"width\":{},\"helper_threads\":{},\"jobs\":{},\"serial_ns\":{},\"parallel_ns\":{}}}",
        std::thread::available_parallelism().map_or(1,|n|n.get()),executor.width(),executor.stats().workers,jobs,serial_ns,parallel_ns);
}
