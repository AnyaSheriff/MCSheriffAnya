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

/// Настоящий цикл с включённой настройкой: форсируем лишь начальную ширину,
/// чтобы тест не зависел от скорости CPU; тайминги политики проверяются отдельно.
#[tokio::test(start_paused = true)]
async fn enabled_real_tick_keeps_one_publication_and_freeze_stops_all_world_clocks() {
    let mut shared = server();
    Arc::get_mut(&mut shared).unwrap().settings.parallel_dimension_ticks = true;
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    let before = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
    let mut policy = parallel::Policy::new(3, 3);
    policy.width = 3;
    let task = tokio::spawn(run_with_block_policy(shared.clone(), policy));
    for _ in 0..5 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    let finished = *shared.ticks.borrow();
    assert!(finished > 0);
    for dimension in Dimension::ALL {
        assert_eq!(shared.world_in(dimension).lock().unwrap().tick(), before[dimension.index()] + finished);
    }
    shared.world_cmd.ticking.lock().unwrap().frozen = true;
    let clocks = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick());
    for _ in 0..5 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    assert!(*shared.ticks.borrow() > finished);
    assert_eq!(Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().tick()), clocks);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}
