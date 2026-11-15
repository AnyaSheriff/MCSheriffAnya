use super::*;

fn server() -> Arc<Shared> {
    Arc::new(Shared::new(World::in_memory(), World::in_memory(),
        std::path::PathBuf::new(), std::path::PathBuf::new(), Default::default(),
        crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsa-parallel-ops-{}.json", std::process::id()))),
        crate::skins::Settings::default()))
}

fn prime(shared: &Shared) -> [usize; 3] {
    Dimension::ALL.map(|dimension| {
        let mut world = shared.world_in(dimension).lock().unwrap();
        let stone = blocks::state_by_name("stone").unwrap();
        let note = blocks::state_by_name("note_block").unwrap();
        world.set_block(0, 63, 0, stone);
        world.set_block(0, 64, 0, note);
        // Событие ноты, а не только пустое вычисление: должно уйти после Broadcast.
        world.queue_block_event(0, 64, 0, 0, 0);
        let cursor = world.watch_events(91);
        world.publish_events();
        cursor
    })
}

#[test]
fn real_world_jobs_advance_all_dimensions_once_and_publish_only_after_barrier() {
    let shared = server();
    let cursors = prime(&shared);
    let clocks = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
    let mut pool = parallel::Pool::new();
    let mut phases = PhaseStats::default();
    parallel_dimension_blocks(&shared, &mut pool, 3, &mut false, &mut phases, &mut [Duration::ZERO; 3]);
    assert_eq!(*shared.ticks.borrow(), 0, "рабочие задания не публикуют такт");
    for dimension in Dimension::ALL {
        let mut world = shared.world_in(dimension).lock().unwrap();
        assert_eq!(world.tick(), clocks[dimension.index()] + 1);
        assert!(world.take_block_event().is_none(), "блочные события обработаны");
        assert!(world.published_events_since(cursors[dimension.index()], 91).0.is_empty());
    }
    publish_finished_tick(&shared, 1);
    assert_eq!(*shared.ticks.borrow(), 1);
    for dimension in Dimension::ALL {
        let events = shared.world_in(dimension).lock().unwrap().published_events_since(cursors[dimension.index()], 91).0;
        let broadcast = events.iter().position(|event| matches!(event, crate::world::WorldEvent::Broadcast)).unwrap();
        let sound = events.iter().position(|event| matches!(event, crate::world::WorldEvent::Sound(_))).expect("нота прозвучала");
        assert!(broadcast < sound, "порядок фаз не изменился");
        assert_eq!(events.iter().filter(|event| matches!(event, crate::world::WorldEvent::Broadcast)).count(), 1);
    }
}

#[test]
fn loaded_command_blocks_disable_parallel_phase_until_removed() {
    let mut world = World::in_memory();
    assert!(!world.has_command_blocks());
    let block = blocks::orientation("command_block").unwrap().min;
    world.set_block(0, 64, 0, block);
    assert!(world.has_command_blocks());
    world.set_block(0, 64, 0, 0);
    assert!(!world.has_command_blocks());
}

#[test]
fn disabled_dimensions_neither_advance_nor_publish_in_parallel_block_phase() {
    for (nether, end) in [(false, false), (true, false), (false, true)] {
        let mut shared = server();
        let settings = &mut Arc::get_mut(&mut shared).unwrap().settings;
        settings.enable_nether = nether;
        settings.enable_end = end;
        let cursors = prime(&shared);
        let before = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
        let mut pool = parallel::Pool::new();
        parallel_dimension_blocks(&shared, &mut pool, 3, &mut false, &mut PhaseStats::default(), &mut [Duration::ZERO; 3]);
        publish_finished_tick(&shared, 1);
        for dimension in Dimension::ALL {
            let mut world = shared.world_in(dimension).lock().unwrap();
            let enabled = shared.dimension_enabled(dimension);
            assert_eq!(world.tick(), before[dimension.index()] + u64::from(enabled));
            let events = world.published_events_since(cursors[dimension.index()], 91).0;
            assert_eq!(events.iter().any(|event| matches!(event, crate::world::WorldEvent::Broadcast)), enabled);
            if !enabled { assert!(world.take_block_event().is_some(), "даже отложенные события не выполнялись"); }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn disabled_dimensions_remain_stopped_in_actual_tick_loop() {
    let mut shared = server();
    let settings = &mut Arc::get_mut(&mut shared).unwrap().settings;
    settings.enable_nether = false;
    settings.enable_end = false;
    settings.experimental_parallel_mobs = true;
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    let before = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
    let task = tokio::spawn(run(shared.clone()));
    for _ in 0..5 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    let published = *shared.ticks.borrow();
    assert!(published > 0);
    for dimension in Dimension::ALL {
        assert_eq!(shared.world_in(dimension).lock().unwrap().tick(),
            before[dimension.index()] + if dimension == Dimension::Overworld { published } else { 0 });
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[test]
fn serial_and_parallel_world_block_phases_keep_same_clock_and_event_order() {
    let serial = server();
    let concurrent = server();
    let from_serial = prime(&serial);
    let from_concurrent = prime(&concurrent);
    let mut pool = parallel::Pool::new();
    let mut phases = PhaseStats::default();
    parallel_dimension_blocks(&concurrent, &mut pool, 2, &mut false, &mut phases, &mut [Duration::ZERO;3]);
    for dimension in Dimension::ALL {
        let world = serial.world_in(dimension).lock().unwrap();
        let (mut world, chunks) = prepare_dimension_blocks(&serial, dimension, world, &mut false, &mut phases);
        random_dimension_blocks(&mut world, &chunks, serial.properties.simulation_distance, crate::farming::random_tick_speed(), &mut phases);
    }
    for dimension in Dimension::ALL {
        let tag = |event: &crate::world::WorldEvent| match event {
            crate::world::WorldEvent::Broadcast => "broadcast",
            crate::world::WorldEvent::Sound(_) => "sound",
            crate::world::WorldEvent::Action(_) => "action",
            crate::world::WorldEvent::Change(_) => "change",
            _ => "other",
        };
        let snapshot = |shared: &Shared, from| {
            let mut world = shared.world_in(dimension).lock().unwrap();
            (world.tick(), world.get_block(0,64,0), world.events_since(from,91).0.iter().map(tag).collect::<Vec<_>>())
        };
        assert_eq!(snapshot(&serial, from_serial[dimension.index()]), snapshot(&concurrent, from_concurrent[dimension.index()]));
    }
}

// Идентичная нагрузка для настоящего цикла и точного сравнения фаз.
fn add_mixed_mob_fixture(shared: &Shared) {
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    for dimension in shared.enabled_dimensions() {
        let mut items = shared.items_in(dimension).lock().unwrap();
        for i in 0..512 {
            let kind = match i % 4 {
                0 => crate::mob::Kind::Villager,
                1 => crate::mob::Kind::Zombie,
                2 => crate::mob::Kind::Llama,
                _ => crate::mob::Kind::Wolf,
            };
            let id = shared.next_entity_id();
            let mut mob = crate::mob::Mob::new(id, kind, ((i % 16) as f64, 65.0, (i / 16) as f64));
            mob.ticks = 200;
            items.add_mob(mob);
        }
    }
}

/// Настоящий цикл с обеими настройками: форсируем начальную ширину и прошлую
/// полезную нагрузку. Это проверка путей выполнения; порог времени проверяется отдельно.
#[tokio::test(start_paused = true)]
async fn enabled_real_tick_keeps_one_publication_and_freeze_stops_all_world_clocks() {
    let mut shared = server();
    let settings = &mut Arc::get_mut(&mut shared).unwrap().settings;
    settings.parallel_dimension_ticks = true;
    settings.experimental_parallel_mobs = true;
    add_mixed_mob_fixture(&shared);
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    let before = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
    let mut policy = adaptive_mobs::Policy::new(3);
    policy.width = 3;
    let block_phases = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    policy.block_phases = Some(block_phases.clone());
    let mob_jobs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    policy.mob_jobs = Some(mob_jobs.clone());
    let task = tokio::spawn(run_with_block_policy(shared.clone(), policy));
    for _ in 0..5 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    assert!(block_phases.load(std::sync::atomic::Ordering::SeqCst) > 0, "настоящий цикл прошёл параллельную блочную фазу");
    assert!(mob_jobs.load(std::sync::atomic::Ordering::SeqCst) > 0, "настоящий цикл выполнял наблюдение мобов в помощниках");
    let finished = *shared.ticks.borrow();
    assert!(finished > 0);
    for dimension in Dimension::ALL {
        assert_eq!(shared.world_in(dimension).lock().unwrap().tick(), before[dimension.index()] + finished);
    }
    shared.world_cmd.ticking.lock().unwrap().frozen = true;
    let clocks = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
    let frozen_mob_jobs = mob_jobs.load(std::sync::atomic::Ordering::SeqCst);
    for _ in 0..5 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    assert_eq!(mob_jobs.load(std::sync::atomic::Ordering::SeqCst), frozen_mob_jobs, "заморозка не отправляет задания мобов");
    assert!(*shared.ticks.borrow() > finished);
    assert_eq!(Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick()), clocks);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[test]
fn shared_world_block_barrier_then_mob_workers_keep_serial_state_and_rng() {
    let serial = server();
    let parallel = server();
    prime(&serial); prime(&parallel);
    add_mixed_mob_fixture(&serial);
    parallel.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    for dimension in serial.enabled_dimensions() {
        for mob in serial.items_in(dimension).lock().unwrap().mobs().to_vec() {
            assert_eq!(parallel.next_entity_id(), mob.entity_id);
            parallel.items_in(dimension).lock().unwrap().add_mob(mob);
        }
    }
    let mut pool = parallel::Pool::new();
    parallel_dimension_blocks(&parallel, &mut pool, 3, &mut false,
        &mut PhaseStats::default(), &mut [Duration::ZERO; 3]);
    // Возврат из функции — барьер всех миров до начала любых заданий мобов.
    for dimension in serial.enabled_dimensions() {
        let world = serial.world_in(dimension).lock().unwrap();
        let (mut world, chunks) = prepare_dimension_blocks(&serial, dimension, world, &mut false, &mut PhaseStats::default());
        random_dimension_blocks(&mut world, &chunks, serial.properties.simulation_distance,
            crate::farming::random_tick_speed(), &mut PhaseStats::default());
    }
    let mut a = crate::mob::Rng::new(12345);
    let mut b = a.clone();
    let mut executor = crate::mob::parallel::Executor::new(); executor.set_width(3); executor.begin_tick();
    for dimension in serial.enabled_dimensions() {
        assert_eq!(parallel.world_in(dimension).lock().unwrap().tick(), 1);
        let mut world = serial.world_in(dimension).lock().unwrap();
        step_mobs(&serial, dimension, &mut world, &mut a, 200);
        drop(world);
        let mut world = parallel.world_in(dimension).lock().unwrap();
        step_mobs_with_executor(&parallel, dimension, &mut world, &mut b, 200, Some(&mut executor));
        drop(world);
        assert_eq!(format!("{:?}", serial.items_in(dimension).lock().unwrap().mobs()),
            format!("{:?}", parallel.items_in(dimension).lock().unwrap().mobs()));
    }
    assert!(executor.stats().jobs > 0, "наблюдения действительно выполнялись помощниками");
    // Последовательность RNG проверена через последующие выдачи, без доступа к закрытому состоянию.
    for _ in 0..16 { assert_eq!(a.next_u64(), b.next_u64()); }
}
