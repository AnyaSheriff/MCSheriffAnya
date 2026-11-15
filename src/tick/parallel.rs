// Ограниченный пул только для независимой блочной части измерений.
// В рабочих заданиях нельзя выполнять команды, плагины или брать другой мир.
use std::{any::Any, panic::{self, AssertUnwindSafe}, sync::mpsc, thread};

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
        let helpers = if self.failed { 0 } else { helpers.min(self.workers.len()) };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::sync::{Arc, Barrier, atomic::{AtomicUsize, Ordering}};

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
    fn failed_pool_stays_serial_even_with_existing_helpers() {
        let mut pool = Pool::new();
        pool.run(2, vec![Box::new(|| 0), Box::new(|| 1)]);
        pool.failed = true;
        let main = thread::current().id();
        let results = pool.run(3, (0..3).map(|i| Box::new(move || (i, thread::current().id() == main)) as Job<_>).collect());
        assert_eq!(results, [(0,true),(1,true),(2,true)]);
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
