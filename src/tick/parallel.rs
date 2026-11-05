// Ограниченный пул только для независимой блочной части измерений.
// В рабочих заданиях нельзя выполнять команды, плагины или брать другой мир.
use std::{any::Any, panic::{self, AssertUnwindSafe}, sync::mpsc, thread, time::Duration};

pub(super) type Job<R> = Box<dyn FnOnce() -> R + Send>;
type Outcome<R> = Result<R, Box<dyn Any + Send>>;
struct Work<R> { job: Job<R>, done: mpsc::Sender<Outcome<R>> }
struct Worker<R> { sender: mpsc::Sender<Work<R>>, thread: thread::JoinHandle<()> }

pub(super) struct Pool<R: Send + 'static> { workers: Vec<Worker<R>>, failed: bool }
impl<R: Send + 'static> Pool<R> {
    pub(super) fn new() -> Self { Self { workers: Vec::new(), failed: false } }

    /// Создаём не больше двух помощников, лишь когда политика впервые их запросила.
    /// Оставленные без задания потоки спят в recv(), не опрашивают очередь.
    fn grow(&mut self, helpers: usize) {
        while self.workers.len() < helpers.min(2) && !self.failed {
            let (sender, receiver) = mpsc::channel::<Work<R>>();
            let started = thread::Builder::new().name(format!("dimension blocks {}", self.workers.len() + 1))
                .spawn(move || {
                    while let Ok(work) = receiver.recv() {
                        let result = panic::catch_unwind(AssertUnwindSafe(work.job));
                        if work.done.send(result).is_err() { break; }
                    }
                });
            match started {
                Ok(thread) => self.workers.push(Worker { sender, thread }),
                Err(_) => self.failed = true,
            }
        }
    }

    /// Барьер: даже при панике дожидаемся остальных, затем передаём её потоку такта.
    /// Повторять задание нельзя: оно уже могло изменить мир.
    pub(super) fn run(&mut self, width: usize, mut jobs: Vec<Job<R>>) -> Vec<R> {
        let helpers = width.clamp(1, 3).min(jobs.len()).saturating_sub(1);
        self.grow(helpers);
        let helpers = helpers.min(self.workers.len());
        let (done, results) = mpsc::channel();
        for (worker, job) in self.workers.iter().take(helpers).zip(jobs.drain(..helpers)) {
            worker.sender.send(Work { job, done: done.clone() }).unwrap_or_else(|_| panic!("поток блочной части остановился"));
        }
        drop(done);
        let mut outcomes: Vec<_> = jobs.into_iter().map(|job| panic::catch_unwind(AssertUnwindSafe(job))).collect();
        for _ in 0..helpers { outcomes.push(results.recv().expect("ответ блочной части")); }
        let mut values = Vec::with_capacity(outcomes.len());
        let mut failed = None;
        for result in outcomes {
            match result { Ok(value) => values.push(value), Err(error) => { failed.get_or_insert(error); } }
        }
        if let Some(error) = failed { panic::resume_unwind(error); }
        values
    }

    pub(super) fn spawn_failed(&self) -> bool { self.failed }
}
impl<R: Send + 'static> Drop for Pool<R> {
    fn drop(&mut self) {
        for Worker { sender, thread } in self.workers.drain(..) {
            drop(sender);
            let _ = thread.join();
        }
    }
}

pub(super) struct Policy { cap: usize, pub(super) width: usize, pressure: u16, quiet: u16 }
impl Policy {
    pub(super) fn new(available: usize, configured: usize) -> Self {
        let cap = available.max(1).min(if configured == 0 { 3 } else { configured.clamp(1, 3) });
        Self { cap, width: 1, pressure: 0, quiet: 0 }
    }
    pub(super) fn reset(&mut self) { self.width = 1; self.pressure = 0; self.quiet = 0; }
    /// Сравниваем с длительностью такта, чтобы учитывать tick rate/sprint.
    /// Один тяжёлый мир сам по себе не повод занимать второй CPU.
    pub(super) fn observe(&mut self, costs: [Duration; 3], period: Duration, allowed: bool) {
        if !allowed { self.reset(); return; }
        let period = period.max(Duration::from_millis(1));
        let useful = costs.iter().filter(|&&cost| cost >= Duration::from_micros(250)).count();
        let total: Duration = costs.into_iter().sum();
        if useful > self.width && total >= period.mul_f64(0.4) && self.width < self.cap {
            self.pressure += 1;
            self.quiet = 0;
            if self.pressure >= 20 { self.width += 1; self.pressure = 0; }
        } else {
            self.pressure = 0;
            if useful < self.width || total < period.mul_f64(0.15) {
                self.quiet += 1;
                if self.quiet >= 200 { self.width = self.width.saturating_sub(1).max(1); self.quiet = 0; }
            } else { self.quiet = 0; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, atomic::{AtomicUsize, Ordering}};

    #[test]
    fn grows_one_at_a_time_after_sustained_work_and_sleeps_when_quiet() {
        let mut policy = Policy::new(8, 0);
        let busy = [Duration::from_millis(12); 3];
        for _ in 0..19 { policy.observe(busy, Duration::from_millis(50), true); }
        assert_eq!(policy.width, 1);
        policy.observe(busy, Duration::from_millis(50), true);
        assert_eq!(policy.width, 2);
        for _ in 0..20 { policy.observe(busy, Duration::from_millis(50), true); }
        assert_eq!(policy.width, 3);
        for expected in [2, 1] {
            for _ in 0..200 { policy.observe([Duration::ZERO; 3], Duration::from_millis(50), true); }
            assert_eq!(policy.width, expected);
        }
    }

    #[test]
    fn respects_cpu_cap_manual_cap_single_busy_world_and_fallback() {
        for (available, configured, cap) in [(1,0,1), (2,0,2), (64,0,3), (64,1,1), (64,2,2)] {
            let mut policy = Policy::new(available, configured);
            for _ in 0..1000 { policy.observe([Duration::from_millis(20);3], Duration::from_millis(50), true); }
            assert_eq!(policy.width, cap);
            policy.observe([Duration::ZERO;3], Duration::from_millis(50), false);
            assert_eq!(policy.width, 1);
        }
        let mut policy = Policy::new(8, 0);
        for _ in 0..1000 { policy.observe([Duration::from_millis(80),Duration::ZERO,Duration::ZERO], Duration::from_millis(50), true); }
        assert_eq!(policy.width, 1);
    }

    #[test]
    fn pool_runs_three_jobs_together_once_and_reuses_workers() {
        let mut pool = Pool::new();
        for _ in 0..3 {
            // Тайм-аут внутри заданий позволяет тесту закончиться, если нет перекрытия.
            let entered = Arc::new(AtomicUsize::new(0));
            let jobs = (0..3).map(|id| {
                let entered = entered.clone();
                Box::new(move || {
                    entered.fetch_add(1, Ordering::SeqCst);
                    let deadline = std::time::Instant::now() + Duration::from_secs(5);
                    while entered.load(Ordering::SeqCst) != 3 {
                        assert!(std::time::Instant::now() < deadline, "задания не перекрылись");
                        thread::yield_now();
                    }
                    id
                }) as Job<_>
            }).collect();
            let mut result = pool.run(3, jobs);
            result.sort();
            assert_eq!(result, [0,1,2]);
        }
        assert_eq!(pool.workers.len(), 2);
    }

    #[test]
    fn serial_mode_spawns_nobody_and_preserves_order() {
        let mut pool = Pool::new();
        let result = pool.run(1, (0..3).map(|id| Box::new(move || id) as Job<_>).collect());
        assert_eq!(result, [0,1,2]);
        assert!(pool.workers.is_empty());
    }

    #[test]
    fn panic_waits_for_other_jobs_and_never_retries() {
        let mut pool = Pool::new();
        let barrier = Arc::new(Barrier::new(2));
        let completed = Arc::new(AtomicUsize::new(0));
        let a = barrier.clone();
        let b = barrier.clone();
        let count = completed.clone();
        let result = panic::catch_unwind(AssertUnwindSafe(|| pool.run(2, vec![
            Box::new(move || { a.wait(); panic!("проверка ошибки рабочего потока"); }),
            Box::new(move || { b.wait(); count.fetch_add(1, Ordering::SeqCst); }),
        ])));
        assert!(result.is_err());
        assert_eq!(completed.load(Ordering::SeqCst), 1);
        assert_eq!(pool.run(2, vec![Box::new(|| ()), Box::new(|| ())]).len(), 2);
    }
}
