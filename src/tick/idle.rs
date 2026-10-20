//! Пауза пустого сервера. Публичное описание `pause-when-empty-seconds`:
//! docs.papermc.io/paper/reference/server-properties/#pause-when-empty-seconds
//! «after no player has been online». Код чужого сервера не используется.
//! У нас по решению владелицы по умолчанию 300 с; <=0 отключает паузу.

use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, MissedTickBehavior};
use crate::{dimension::Dimension, players::Presence, shared::Shared};

fn empty_deadline(presence: Presence, seconds: i32) -> Instant {
    let grace = presence.changed + Duration::from_secs(seconds.max(0) as u64);
    presence.console_until.map_or(grace, |until| grace.max(until))
}

fn due(presence: Presence, seconds: i32, now: Instant) -> bool {
    seconds > 0 && presence.online == 0 && now >= empty_deadline(presence, seconds)
}

/// Ждём сетку тактов, но срок пустоты и вход/выход видим независимо от
/// tick rate: даже очень редкие такты не отодвигают паузу через пять минут.
pub(super) async fn wait_tick(shared: &Shared, presence: &mut watch::Receiver<Presence>, next: Instant) -> Option<Duration> {
    let seconds = shared.properties.pause_when_empty_seconds;
    if seconds <= 0 { tokio::time::sleep_until(next).await; return None; }
    loop {
        let current = *presence.borrow_and_update();
        if due(current, seconds, Instant::now()) {
            if let Some(paused) = wait(shared, presence).await { return Some(paused); }
            continue; // игрок мог войти между двумя проверками
        }
        let empty_at = empty_deadline(current, seconds);
        tokio::select! {
            _ = tokio::time::sleep_until(next) => {
                if !due(*presence.borrow_and_update(), seconds, Instant::now()) { return None; }
            }
            _ = tokio::time::sleep_until(empty_at), if current.online == 0 => {}
            changed = presence.changed() => {
                if changed.is_err() { tokio::time::sleep_until(next).await; return None; }
            }
        }
    }
}

/// Возвращает длительность паузы по реальному времени, без ожидания её начала.
/// Обычные sleep/step/sprint принимаются только после этого барьера.
pub(super) async fn wait(shared: &Shared, presence: &mut watch::Receiver<Presence>) -> Option<Duration> {
    let seconds = shared.properties.pause_when_empty_seconds;
    if !due(*presence.borrow_and_update(), seconds, Instant::now()) { return None; }

    let began = Instant::now();
    // Никаких замков игроков через await. Очередь записи сама сохраняет и
    // повторяет неудачные записи: не блокируем вход ожиданием flush на диске.
    save(shared, true);
    crate::log_info!("Игроков нет {} с: мир поставлен на паузу до следующего входа", seconds);
    wait_until_active(presence, seconds, || save(shared, false)).await;
    let current = *presence.borrow();
    if current.online > 0 {
        crate::log_info!("Игрок вошёл: мир продолжает работу");
    } else if current.console_until.is_some_and(|until| until > Instant::now()) {
        crate::log_info!("Команда из консоли: мир продолжает работу на 30 с");
    } else {
        crate::log_info!("Игрок заходил: ожидание паузы начато заново");
    }
    Some(began.elapsed())
}

async fn wait_until_active(presence: &mut watch::Receiver<Presence>, seconds: i32, mut maintenance: impl FnMut()) {
    let mut saving = tokio::time::interval_at(Instant::now() + Duration::from_secs(1), Duration::from_secs(1));
    saving.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        // borrow_and_update и changed не теряют вход, случившийся между
        // чтением и засыпанием. Короткий вход/выход обновляет changed и
        // начинает полный срок ожидания заново, даже если online снова уже0.
        if !due(*presence.borrow_and_update(), seconds, Instant::now()) { return; }
        tokio::select! {
            changed = presence.changed() => { if changed.is_err() { return; } }
            _ = saving.tick() => maintenance(),
        }
    }
}

/// До паузы кладём мобов и все свежие чанки в существующую очередь записи.
/// Потом раз в секунду сохраняем только грязное/свежее: консоль и уже
/// запущенная асинхронная загрузка ещё могут менять мир, но не его часы.
fn save(shared: &Shared, entering: bool) {
    for dimension in Dimension::ALL {
        let mut world = shared.world_in(dimension).lock().expect("мир");
        if entering {
            let items = shared.items_in(dimension).lock().expect("предметы");
            world.save_before_idle(items.mobs());
        } else {
            {
                let items = shared.items_in(dimension).lock().expect("предметы");
                world.store_mobs_if_changed(items.mobs());
            }
            world.save_if_needed();
        }
    }
    if let Err(error) = shared.maps.lock().expect("карты").save() {
        crate::log_warn!("Карты не записаны во время паузы: {}", error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use crate::{players::{Member, Position}, world::World};

    fn member(name: &str, id: u8) -> Member {
        Member {
            uuid: [id;16], name: name.into(), game_mode: 1, entity_id: i32::from(id),
            position: Position { x: 0.5, y: 64.0, z: 0.5, yaw: 0.0, pitch: 0.0, on_ground: true },
            skin: None, skin_parts: 0x7f, main_hand: 1, held: None,
            dimension: Dimension::Overworld, sneaking: false, swings: 0,
            shoved: ((0.0,0.0,0.0),0), unsettled: 0,
        }
    }

    fn shared(seconds: i32) -> Arc<Shared> {
        let properties = crate::config::server_properties::ServerProperties {
            pause_when_empty_seconds: seconds, ..Default::default()
        };
        let shared = Arc::new(Shared::new(World::in_memory(), World::in_memory(),
            std::path::PathBuf::new(), std::path::PathBuf::new(), properties,
            crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsa-idle-ops-{}.json",std::process::id()))),
            crate::skins::Settings::default()));
        shared.spawn_mobs.store(false, Ordering::Relaxed);
        shared
    }

    #[tokio::test(start_paused = true)]
    async fn startup_empty_exact_300_second_boundary_and_disable() {
        let presence = Presence { online: 0, changed: Instant::now(), console_until: None };
        tokio::time::advance(Duration::from_secs(299)).await;
        assert!(!due(presence,300,Instant::now()));
        tokio::time::advance(Duration::from_millis(999)).await;
        assert!(!due(presence,300,Instant::now()));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(due(presence,300,Instant::now()));
        for disabled in [0,-1,i32::MIN] { assert!(!due(presence,disabled,Instant::now())); }
        assert!(!due(Presence { online: 1, ..presence },300,Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn empty_deadline_and_join_wake_ignore_slow_tick_rate() {
        let shared = shared(1);
        let mut presence = shared.players.lock().unwrap().presence();
        let copy = Arc::clone(&shared);
        let next = Instant::now()+Duration::from_secs(3600);
        let task = tokio::spawn(async move { wait_tick(&copy,&mut presence,next).await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(100)).await;
        tokio::task::yield_now().await;
        shared.players.lock().unwrap().join(member("Java",1),1).unwrap();
        let paused = task.await.unwrap().expect("пауза началась до часового такта");
        assert!(paused >= Duration::from_secs(100));
        assert!(Instant::now() < next,"вход разбудил до ближайшего такта");
    }

    #[tokio::test(start_paused = true)]
    async fn common_java_bedrock_membership_holds_server_awake() {
        let shared = shared(300);
        let mut presence = shared.players.lock().unwrap().presence();
        {
            // Оба production-входа используют именно этот общий join.
            let mut players = shared.players.lock().unwrap();
            assert!(players.join(member("Java",1),1).is_some());
            assert!(players.join(member("Bedrock",2),2).is_some());
        }
        assert_eq!(presence.borrow_and_update().online,2);
        shared.players.lock().unwrap().leave(&[1;16]);
        assert_eq!(presence.borrow_and_update().online,1);
        tokio::time::advance(Duration::from_secs(1000)).await;
        assert!(!due(*presence.borrow(),300,Instant::now()));
        shared.players.lock().unwrap().leave(&[2;16]);
        assert_eq!(presence.borrow_and_update().online,0);
        assert!(!due(*presence.borrow(),300,Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn short_join_leave_between_gate_reads_restarts_grace() {
        let shared = shared(300);
        let mut presence = shared.players.lock().unwrap().presence();
        presence.borrow_and_update();
        tokio::time::advance(Duration::from_secs(299)).await;
        {
            let mut players = shared.players.lock().unwrap();
            players.join(member("Brief",3),3).unwrap();
            assert!(players.leave(&[3;16]));
        }
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(presence.borrow_and_update().online,0);
        assert!(!due(*presence.borrow(),300,Instant::now()));
        tokio::time::advance(Duration::from_secs(299)).await;
        assert!(due(*presence.borrow(),300,Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn rejected_duplicate_and_unknown_departure_do_not_reset_grace() {
        let shared = shared(300);
        let mut presence = shared.players.lock().unwrap().presence();
        shared.players.lock().unwrap().join(member("Java",1),1).unwrap();
        let before = *presence.borrow_and_update();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(shared.players.lock().unwrap().join(member("java",9),9).is_none());
        assert!(!shared.players.lock().unwrap().leave(&[99;16]));
        assert_eq!(presence.borrow().changed,before.changed);
        assert!(!presence.has_changed().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn wake_before_wait_or_after_pending_is_not_lost() {
        let now = Instant::now();
        let (sender,mut presence) = watch::channel(Presence { online: 0, changed: now, console_until: None });
        tokio::time::advance(Duration::from_secs(300)).await;
        assert!(due(*presence.borrow_and_update(),300,Instant::now()));
        sender.send_replace(Presence { online: 1, changed: Instant::now(), console_until: None });
        // Вход уже после проверки due, но ещё до создания waiting future.
        wait_until_active(&mut presence,300,|| panic!("не должен засыпать")).await;

        sender.send_replace(Presence { online: 0, changed: Instant::now(), console_until: None });
        tokio::time::advance(Duration::from_secs(300)).await;
        let task = tokio::spawn(async move { wait_until_active(&mut presence,300,|| {}).await; });
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        sender.send_replace(Presence { online: 1, changed: Instant::now(), console_until: None });
        tokio::task::yield_now().await;
        assert!(task.is_finished());
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn paused_maintenance_does_not_generate_tick_notifications() {
        let shared = shared(1);
        let presence = shared.players.lock().unwrap().presence();
        let maintenance = Arc::new(AtomicUsize::new(0));
        let calls = Arc::clone(&maintenance);
        tokio::time::advance(Duration::from_secs(1)).await;
        let task = tokio::spawn(async move {
            let mut presence = presence;
            wait_until_active(&mut presence,1,|| { calls.fetch_add(1,Ordering::Relaxed); }).await;
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(60)).await;
        tokio::task::yield_now().await;
        assert!(maintenance.load(Ordering::Relaxed)>0);
        assert!(!task.is_finished());
        assert_eq!(*shared.ticks.borrow(),0);
        assert_eq!(*shared.tick_starts.borrow(),0);
        task.abort();
    }

    /// Настоящий run(), а не модель таймера: часы всех измерений и оба
    /// сигнала стоят. При входе ровно один первый такт, не догонялка.
    #[tokio::test(start_paused = true)]
    async fn real_tick_all_dimensions_pause_and_resume_without_catchup() {
        let shared = shared(1);
        let task = tokio::spawn(super::super::run(Arc::clone(&shared)));
        tokio::task::yield_now().await;
        for _ in 0..25 {
            tokio::time::advance(Duration::from_millis(50)).await;
            tokio::task::yield_now().await;
        }
        let before = *shared.ticks.borrow();
        let starts = *shared.tick_starts.borrow();
        let clocks = Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().clock());
        tokio::time::advance(Duration::from_secs(600)).await;
        tokio::task::yield_now().await;
        assert_eq!(*shared.ticks.borrow(),before);
        assert_eq!(*shared.tick_starts.borrow(),starts);
        assert_eq!(Dimension::ALL.map(|d| shared.world_in(d).lock().unwrap().clock()),clocks);
        shared.players.lock().unwrap().join(member("Java",1),1).unwrap();
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert_eq!(*shared.ticks.borrow(),before+1);
        tokio::time::advance(Duration::from_millis(49)).await;
        tokio::task::yield_now().await;
        assert_eq!(*shared.ticks.borrow(),before+1);
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(*shared.ticks.borrow(),before+2);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn console_does_not_shorten_startup_grace_and_repeated_commands_extend_it() {
        let shared = shared(300);
        let mut presence = shared.players.lock().unwrap().presence();
        let began = Instant::now();
        tokio::time::advance(Duration::from_secs(100)).await;
        shared.players.lock().unwrap().note_console_command();
        assert_eq!(empty_deadline(*presence.borrow_and_update(),300),began+Duration::from_secs(300));
        tokio::time::advance(Duration::from_secs(190)).await;
        shared.players.lock().unwrap().note_console_command();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(!due(*presence.borrow(),300,Instant::now()));
        tokio::time::advance(Duration::from_secs(20)).await;
        assert!(due(*presence.borrow(),300,Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn console_resumes_actual_pause_for_thirty_seconds_and_extends_again() {
        let shared = shared(1);
        let mut presence = shared.players.lock().unwrap().presence();
        tokio::time::advance(Duration::from_secs(1)).await;
        let copy = Arc::clone(&shared);
        let task = tokio::spawn(async move { wait(&copy,&mut presence).await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(100)).await;
        shared.players.lock().unwrap().note_console_command();
        assert!(task.await.unwrap().is_some(),"команда разбудила уже paused-сервер");
        let mut presence = shared.players.lock().unwrap().presence();
        tokio::time::advance(Duration::from_millis(29999)).await;
        assert!(!due(*presence.borrow_and_update(),1,Instant::now()));
        shared.players.lock().unwrap().note_console_command();
        tokio::time::advance(Duration::from_secs(30)).await;
        assert!(due(*presence.borrow(),1,Instant::now()),"ровно30с после последней команды");
    }

    #[tokio::test(start_paused = true)]
    async fn real_player_join_clears_console_deadline_and_leave_gets_normal_grace() {
        let shared = shared(300);
        let mut presence = shared.players.lock().unwrap().presence();
        shared.players.lock().unwrap().note_console_command();
        shared.players.lock().unwrap().join(member("Java",1),1).unwrap();
        assert!(presence.borrow_and_update().console_until.is_none());
        tokio::time::advance(Duration::from_secs(1000)).await;
        assert!(!due(*presence.borrow(),300,Instant::now()));
        shared.players.lock().unwrap().leave(&[1;16]);
        tokio::time::advance(Duration::from_secs(299)).await;
        assert!(!due(*presence.borrow(),300,Instant::now()));
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(due(*presence.borrow(),300,Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn idle_pause_does_not_consume_tick_step() {
        let shared = shared(1);
        {
            let mut ticking = shared.world_cmd.ticking.lock().unwrap();
            ticking.frozen = true; ticking.step_left = 3;
        }
        tokio::time::advance(Duration::from_secs(1)).await;
        let mut presence = shared.players.lock().unwrap().presence();
        let copy = Arc::clone(&shared);
        let task = tokio::spawn(async move { wait(&copy,&mut presence).await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(600)).await;
        tokio::task::yield_now().await;
        assert_eq!(shared.world_cmd.ticking.lock().unwrap().step_left,3);
        shared.players.lock().unwrap().join(member("Bedrock",2),2).unwrap();
        let paused = task.await.unwrap().unwrap();
        assert!(paused >= Duration::from_secs(600));
        assert!(shared.world_cmd.ticking.lock().unwrap().frozen);
        assert_eq!(shared.world_cmd.ticking.lock().unwrap().step_left,3);
        assert!(crate::commands_world::next_tick(&shared).run_world);
        assert_eq!(shared.world_cmd.ticking.lock().unwrap().step_left,2);
    }
}
