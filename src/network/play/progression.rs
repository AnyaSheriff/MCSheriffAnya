// Привязанные к положению условия (вики, Advancement и Statistics).
// [проверить] точный такт выдачи: проверки позиции раз в 20 тактов.
use super::*;

#[derive(Default)]
pub(super) struct Location {
    nether_origin: Option<(f64, f64, f64)>,
    levitation_origin: Option<f64>,
    landed_from_world_height: bool,
    structure: Option<StructureProbe>,
}

// Холодные кольца крепостей считаются долго: ни сеть, ни замок мира не ждут их.
// Вики, Advancement: «Enter a ...». Отложенный такт выдачи [проверить].
struct StructureProbe {
    dimension: Dimension,
    at: (i32, i32, i32),
    result: tokio::sync::oneshot::Receiver<Option<&'static str>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for StructureProbe {
    fn drop(&mut self) { self.task.abort(); }
}

fn structure_slots() -> &'static Arc<tokio::sync::Semaphore> {
    static SLOTS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
}

impl Location {
    fn schedule_structure(&mut self, slots: &Arc<tokio::sync::Semaphore>, dimension: Dimension,
        at: (i32, i32, i32), check: impl FnOnce() -> Option<&'static str> + Send + 'static,
    ) {
        if self.structure.is_some() { return; }
        let Ok(permit) = slots.clone().try_acquire_owned() else { return; };
        let (sender, result) = tokio::sync::oneshot::channel();
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _ = sender.send(check());
        });
        self.structure = Some(StructureProbe { dimension, at, result, task });
    }

    fn poll_structure(&mut self, dimension: Dimension, at: (i32, i32, i32)) -> Option<&'static str> {
        let probe = self.structure.as_mut()?;
        let result = match probe.result.try_recv() {
            Ok(result) => result,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => return None,
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => None,
        };
        let current = probe.dimension == dimension && probe.at == at;
        self.structure.take();
        if current { result } else { None }
    }
}

fn location_block(player: &PlayerState) -> (i32, i32, i32) {
    (player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32)
}

pub(super) fn poll_structure(player: &mut PlayerState) {
    let at = location_block(player);
    if let Some(id) = player.progress_location.poll_structure(player.dimension, at) {
        player.progress.trigger(id);
    }
}

pub(super) fn changed_dimension(player: &mut PlayerState, next: Dimension, at: (f64, f64, f64)) {
    if player.dimension == Dimension::Overworld && next == Dimension::Nether {
        player.progress_location.nether_origin = Some((player.x, player.y, player.z));
    } else if player.dimension == Dimension::Nether {
        if next == Dimension::Overworld
            && let Some(from) = player.progress_location.nether_origin
            && (at.0 - from.0).powi(2) + (at.1 - from.1).powi(2) + (at.2 - from.2).powi(2) >= 7000.0_f64.powi(2)
        {
            player.progress.trigger("nether/fast_travel");
        }
        player.progress_location.nether_origin = None;
    }
    player.progress_location.structure.take();
    player.progress_location.levitation_origin = None;
    player.progress_location.landed_from_world_height = false;
}

pub(super) fn landed(player: &mut PlayerState, fallen: f32) {
    // Проверяем выживание после обработки урона, включая спасение тотемом.
    if player.dimension == Dimension::Overworld && player.y <= -59.0
        && fallen > 379.0 && player.y + f64::from(fallen) >= 319.0
    {
        player.progress_location.landed_from_world_height = true;
    }
}

pub(super) fn effects(player: &mut PlayerState) {
    if std::mem::take(&mut player.progress_location.landed_from_world_height) && !player.vit.dead {
        player.progress.trigger("adventure/fall_from_world_height");
    }
    if player.vit.effects.iter().any(|e| e.kind == crate::survival::EffectKind::Levitation && e.ticks_left > 0) {
        let start = *player.progress_location.levitation_origin.get_or_insert(player.y);
        if (player.y - start).abs() >= 50.0 { player.progress.trigger("end/levitate"); }
    } else {
        player.progress_location.levitation_origin = None;
    }
}

pub(super) fn location(shared: &Shared, player: &mut PlayerState) {
    let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let (x, y, z) = location_block(player);
    if player.sneaking && redstone::near_sculk_sensor(&world, (player.x, player.y, player.z)) { player.progress.trigger("adventure/avoid_vibration"); }
    player.progress.biome(player.dimension, world.mob_biome(x, y, z));
    let structure = world.progress_structure_check(x, y, z);
    if player.on_ground && blocks::block_at_state(world.get_block(x, (player.y - 0.2).floor() as i32, z)) == Some("powder_snow")
        && player.inventory.slot(8).and_then(|s| blocks::item_name(s.item)) == Some("leather_boots")
    {
        player.progress.trigger("adventure/walk_on_powder_snow_with_leather_boots");
    }
    drop(world);
    player.progress_location.schedule_structure(structure_slots(), player.dimension, (x, y, z), structure);
}

/// Выдача только за установку участником, а не за нахождение рядом.
pub(super) fn placed(world: &World, player: &mut PlayerState, name: &str, at: (i32, i32, i32)) {
    if matches!(name, "comparator" | "chiseled_bookshelf") && redstone::reads_bookshelf(world, at) {
        player.progress.trigger("adventure/read_power_of_chiseled_bookshelf");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> PlayerState {
        let mut player = PlayerState::new([216; 16], "ProgressTest".to_string(), None, None, 0x7F, 11, (0.0, 64.0, 0.0));
        player.progress.stats = crate::stats::Stats::new();
        player.progress.adv = crate::advancements::AdvState::new();
        player
    }

    fn probe_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_time().max_blocking_threads(1).build().unwrap()
    }

    #[test]
    fn cold_structure_probe_leaves_network_running_and_deduplicates() {
        probe_runtime().block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let mut location = Location::default();
            let (release, gate) = std::sync::mpsc::channel();
            let (entered, started) = tokio::sync::oneshot::channel();
            location.schedule_structure(&slots, Dimension::Overworld, (1, 64, 2), move || {
                entered.send(()).unwrap();
                gate.recv().unwrap();
                Some("story/follow_ender_eye")
            });
            started.await.unwrap();
            location.schedule_structure(&slots, Dimension::Overworld, (1, 64, 2), || panic!("повторная проверка"));
            let pending = location.poll_structure(Dimension::Overworld, (1, 64, 2));
            // Задача сети исполняется, хотя холодная проверка ещё не закончилась.
            let network = tokio::spawn(async { tokio::task::yield_now().await; 777 });
            let response = tokio::time::timeout(Duration::from_secs(1), network).await;
            release.send(()).unwrap();
            (&mut location.structure.as_mut().unwrap().task).await.unwrap();
            assert!(pending.is_none());
            assert_eq!(response.unwrap().unwrap(), 777);
            assert_eq!(location.poll_structure(Dimension::Overworld, (1, 64, 2)), Some("story/follow_ender_eye"));
            assert!(location.poll_structure(Dimension::Overworld, (1, 64, 2)).is_none());
            assert_eq!(slots.available_permits(), 1);
        });
    }

    #[test]
    fn structure_probe_discards_changed_position_and_dimension() {
        probe_runtime().block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            for (dimension, at) in [(Dimension::Nether, (1, 64, 2)), (Dimension::Overworld, (2, 64, 2))] {
                let mut location = Location::default();
                location.schedule_structure(&slots, Dimension::Overworld, (1, 64, 2), || Some("story/follow_ender_eye"));
                (&mut location.structure.as_mut().unwrap().task).await.unwrap();
                assert!(location.poll_structure(dimension, at).is_none());
                assert!(location.structure.is_none());
            }
        });
    }

    #[test]
    fn structure_probe_bounds_work_and_cancels_queued_on_drop() {
        probe_runtime().block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let (release, gate) = std::sync::mpsc::channel();
            let (entered, started) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || { entered.send(()).unwrap(); gate.recv().unwrap(); });
            started.await.unwrap();
            let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let observed = executed.clone();
            let mut first = Location::default();
            first.schedule_structure(&slots, Dimension::Overworld, (0, 64, 0), move || {
                observed.store(true, std::sync::atomic::Ordering::SeqCst); None
            });
            let mut second = Location::default();
            second.schedule_structure(&slots, Dimension::Overworld, (0, 64, 0), || panic!("превышен предел"));
            let limited = second.structure.is_none();
            drop(first);
            release.send(()).unwrap();
            blocker.await.unwrap();
            // Барьер в том же однопоточном пуле: отменённая работа уже убрана.
            tokio::task::spawn_blocking(|| {}).await.unwrap();
            assert!(limited);
            assert!(!executed.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(slots.available_permits(), 1);
        });
    }

    #[test]
    fn structure_probe_panic_releases_capacity_and_clears_pending() {
        probe_runtime().block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let mut location = Location::default();
            location.schedule_structure(&slots, Dimension::Overworld, (0, 64, 0), || panic!("ошибка расчёта"));
            assert!((&mut location.structure.as_mut().unwrap().task).await.unwrap_err().is_panic());
            assert!(location.poll_structure(Dimension::Overworld, (0, 64, 0)).is_none());
            assert!(location.structure.is_none());
            assert_eq!(slots.available_permits(), 1);
        });
    }

    #[test]
    fn nether_travel_uses_the_two_overworld_endpoints() {
        for (distance, expected) in [(6999.0, false), (7000.0, true)] {
            let mut p = player();
            changed_dimension(&mut p, Dimension::Nether, (0.0, 64.0, 0.0));
            p.dimension = Dimension::Nether;
            changed_dimension(&mut p, Dimension::Overworld, (distance, 64.0, 0.0));
            assert_eq!(p.progress.adv.is_done("nether/fast_travel"), expected);
            assert!(p.progress_location.nether_origin.is_none());
        }
        let mut p = player();
        p.dimension = Dimension::Nether;
        changed_dimension(&mut p, Dimension::Overworld, (10000.0, 64.0, 0.0));
        assert!(!p.progress.adv.is_done("nether/fast_travel"));
    }

    #[test]
    fn levitation_distance_resets_when_effect_ends() {
        let mut p = player();
        p.vit.give_effect(crate::survival::EffectKind::Levitation, 0, 200);
        effects(&mut p);
        p.y += 49.0;
        effects(&mut p);
        assert!(!p.progress.adv.is_done("end/levitate"));
        p.vit.effects.clear();
        effects(&mut p);
        p.vit.give_effect(crate::survival::EffectKind::Levitation, 0, 200);
        effects(&mut p);
        p.y += 1.0;
        effects(&mut p);
        assert!(!p.progress.adv.is_done("end/levitate"));
        p.y += 49.0;
        effects(&mut p);
        assert!(p.progress.adv.is_done("end/levitate"));
    }

    #[test]
    fn world_height_fall_requires_landing_and_surviving() {
        for (y, distance, dead, dimension, expected) in [
            (-61.0, 380.0, false, Dimension::Overworld, true),
            (-60.0, 379.0, false, Dimension::Overworld, false),
            (-62.0, 380.0, false, Dimension::Overworld, false),
            (-61.0, 380.0, true, Dimension::Overworld, false),
            (-61.0, 380.0, false, Dimension::Nether, false),
        ] {
            let mut p = player();
            p.y = y;
            p.dimension = dimension;
            landed(&mut p, distance);
            assert!(!p.progress.adv.is_done("adventure/fall_from_world_height"));
            p.vit.dead = dead;
            effects(&mut p);
            assert_eq!(p.progress.adv.is_done("adventure/fall_from_world_height"), expected, "{y}/{distance}/{dead}");
            assert!(!p.progress_location.landed_from_world_height);
        }
    }
}
