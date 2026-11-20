// Независимые снимки наблюдения; мир, случайность и события остаются в потоке такта.
use std::{any::Any, panic::{self, AssertUnwindSafe}, sync::{mpsc, Arc}, thread, time::{Duration, Instant}};

type Job = Box<dyn FnOnce() + Send>;
type BatchOutput<R> = Result<Vec<R>, Box<dyn Any + Send>>;
type Batch<R> = (usize, Duration, BatchOutput<R>);
struct Worker { sender: mpsc::Sender<Job>, handle: thread::JoinHandle<()> }
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub jobs: usize,
    pub items: usize,
    pub workers: usize,
    pub cost: Duration,
    pub comparisons: usize,
    batch_time: Duration,
}
impl Stats {
    pub fn useful(&self) -> bool {
        // Удачное ускорение сокращает длительность этапа. Сумма времени отдельных
        // пачек оценивает объём работы и не гасит помощников только из-за выигрыша.
        self.items >= 64 && self.comparisons >= 16384 && self.cost.max(self.batch_time) >= Duration::from_millis(1)
    }
}

fn time_batch<R>(collect: impl FnOnce() -> Vec<R>) -> (Duration, BatchOutput<R>) {
    let started = Instant::now();
    let output = panic::catch_unwind(AssertUnwindSafe(collect));
    (started.elapsed(), output)
}

pub struct Executor { workers: Vec<Worker>, width: usize, stats: Stats, failed: bool }
impl Default for Executor { fn default() -> Self { Self::new() } }
impl Executor {
    pub fn new() -> Self { Self { workers: Vec::new(), width: 1, stats: Stats::default(), failed: false } }
    pub fn set_width(&mut self, width: usize) { self.width = width.clamp(1, 8); }
    #[cfg(test)]
    pub fn width(&self) -> usize { self.width }
    pub fn spawn_failed(&self) -> bool { self.failed }
    pub fn begin_tick(&mut self) { self.stats = Stats { workers: self.workers.len(), ..Stats::default() }; }
    pub fn stats(&self) -> Stats { self.stats }
    pub fn should_parallel(&self, items: usize, comparisons: usize) -> bool {
        self.width > 1 && items >= 64 && comparisons >= 16384 && !self.failed
    }
    pub fn record_work(&mut self, cost: Duration, items: usize, comparisons: usize) {
        if items >= 64 && comparisons >= 16384 { self.stats.cost += cost; self.stats.items += items; self.stats.comparisons += comparisons; }
    }
    fn grow(&mut self, count: usize) {
        while self.workers.len() < count && !self.failed {
            let (sender, receiver) = mpsc::channel::<Job>();
            match thread::Builder::new().name(format!("mob sensing {}", self.workers.len()+1)).spawn(move || {
                while let Ok(job) = receiver.recv() { job(); }
            }) {
                Ok(handle) => self.workers.push(Worker { sender, handle }),
                Err(_) => self.failed = true,
            }
        }
        self.stats.workers = self.workers.len();
    }
    /// Результаты возвращаются в порядке входа; паника передаётся только после барьера.
    pub fn ordered_map<T, R, F>(&mut self, input: Vec<T>, map: F) -> Vec<R>
    where T: Send + 'static, R: Send + 'static, F: Fn(T) -> R + Send + Sync + 'static {
        if input.is_empty() { return Vec::new(); }
        let width = self.width.min(input.len());
        self.grow(width.saturating_sub(1));
        let width = if self.failed { 1 } else { width.min(self.workers.len()+1) };
        if width == 1 { return input.into_iter().map(map).collect(); }
        let chunk_size = input.len().div_ceil(width);
        let mut source = input.into_iter();
        let chunks: Vec<Vec<T>> = (0..width).map(|_| source.by_ref().take(chunk_size).collect()).filter(|v: &Vec<T>| !v.is_empty()).collect();
        let map = Arc::new(map);
        let (done, results) = mpsc::channel::<Batch<R>>();
        let mut local = None;
        let count = chunks.len();
        for (index, chunk) in chunks.into_iter().enumerate() {
            let map = map.clone();
            if index+1 == count {
                let (elapsed, value) = time_batch(|| chunk.into_iter().map(|v| map(v)).collect());
                local = Some((index, elapsed, value));
            } else {
                let done = done.clone();
                let job: Job = Box::new(move || {
                    let (elapsed, value) = time_batch(|| chunk.into_iter().map(|v| map(v)).collect());
                    let _ = done.send((index, elapsed, value));
                });
                match self.workers[index].sender.send(job) {
                    Ok(()) => self.stats.jobs += 1,
                    Err(error) => {
                        // Очередь не приняла задание: оно ещё не исполнялось. Считаем
                        // снимок здесь и сохраняем общий барьер уже работающих задач.
                        self.failed = true;
                        (error.0)();
                    }
                }
            }
        }
        drop(done);
        let mut collected: Vec<_> = results.into_iter().collect();
        collected.extend(local);
        collected.sort_by_key(|(index, _, _)| *index);
        let mut output = Vec::new();
        let mut failed = None;
        for (_, elapsed, value) in collected {
            self.stats.batch_time += elapsed;
            match value { Ok(values) => output.extend(values), Err(error) => { failed.get_or_insert(error); } }
        }
        if let Some(error) = failed { panic::resume_unwind(error); }
        output
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        for Worker { sender, handle } in self.workers.drain(..) { drop(sender); let _ = handle.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faster_parallel_work_remains_useful_without_lowering_thresholds() {
        let mut work = Stats { jobs: 1, items: 64, comparisons: 16384,
            cost: Duration::from_micros(741), batch_time: Duration::from_micros(1400), ..Stats::default() };
        assert!(work.useful());
        work.items = 63;
        assert!(!work.useful());
        work.items = 64;
        work.comparisons = 16383;
        assert!(!work.useful());
        work.comparisons = 16384;
        work.batch_time = Duration::from_micros(999);
        assert!(!work.useful());
        work.cost = Duration::from_millis(1);
        assert!(work.useful());
        let mut executor = Executor::new();
        executor.stats = work;
        executor.begin_tick();
        assert!(!executor.stats().useful());
    }

    fn closed_worker() -> Worker {
        let (sender, receiver) = mpsc::channel::<Job>();
        let (ready, stopped) = mpsc::channel();
        let handle = thread::spawn(move || { drop(receiver); ready.send(()).unwrap(); });
        stopped.recv().unwrap();
        Worker { sender, handle }
    }

    #[test]
    fn closed_helper_preserves_last_alert_order_exactly_once_and_then_stays_serial() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut executor = Executor::new();
        executor.grow(1);
        executor.workers.push(closed_worker());
        executor.set_width(3);
        let coordinator = thread::current().id();
        let counts = Arc::new((0..8).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
        let seen = counts.clone();
        // Та же чистая граница выбора последнего зова, что у Bee, включая None.
        let calls = [((0.0_f64, 0.0_f64, 0.0_f64), Some(7), 100), ((16.0, 0.0, 0.0), None, 200)];
        let positions = [0.0_f64, 16.0, 17.0, -16.0, -17.0, 4.0, 2.0, 32.0];
        let actual = executor.ordered_map(positions.into_iter().enumerate().collect(), move |(index, x)| {
            seen[index].fetch_add(1, Ordering::SeqCst);
            let mut selected = None;
            for &(at, target, anger) in &calls {
                if (x-at.0).abs() <= 16.0 && at.1.abs() <= 10.0 && at.2.abs() <= 16.0 {
                    selected = Some((target, anger));
                }
            }
            (index, selected, thread::current().id() == coordinator)
        });
        let expected = [Some((None, 200)), Some((None, 200)), Some((None, 200)),
            Some((Some(7), 100)), None, Some((None, 200)), Some((None, 200)), Some((None, 200))];
        assert_eq!(actual.iter().map(|&(index, value, _)| (index, value)).collect::<Vec<_>>(),
            expected.into_iter().enumerate().collect::<Vec<_>>());
        assert!(actual[..3].iter().all(|value| !value.2));
        assert!(actual[3..].iter().all(|value| value.2));
        assert!(counts.iter().all(|count| count.load(Ordering::SeqCst) == 1));
        assert_eq!(executor.stats().jobs, 1);
        assert!(executor.spawn_failed());
        assert!(!executor.should_parallel(128, 32768));
        executor.begin_tick();
        executor.set_width(8);
        assert!(executor.ordered_map(vec![0, 1, 2], move |_| thread::current().id() == coordinator)
            .into_iter().all(|local| local));
        assert_eq!(executor.stats().jobs, 0);
    }

    #[test]
    fn unsent_job_panic_waits_for_live_helpers_without_replaying_any_input() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut executor = Executor::new();
        executor.grow(1);
        executor.workers.push(closed_worker());
        executor.set_width(3);
        let counts = Arc::new((0..6).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
        let seen = counts.clone();
        let failed = panic::catch_unwind(AssertUnwindSafe(|| executor.ordered_map((0..6).collect(), move |index| {
            if index == 0 { thread::sleep(std::time::Duration::from_millis(10)); }
            seen[index].fetch_add(1, Ordering::SeqCst);
            if index == 2 { panic!("unsent snapshot panic"); }
            index
        }))).expect_err("паника вычисления обязана дойти после барьера");
        assert_eq!(failed.downcast_ref::<&str>(), Some(&"unsent snapshot panic"));
        assert_eq!(counts.iter().map(|count| count.load(Ordering::SeqCst)).collect::<Vec<_>>(), [1, 1, 1, 0, 1, 1]);
        assert!(executor.spawn_failed());
        let coordinator = thread::current().id();
        assert!(executor.ordered_map(vec![0, 1], move |_| thread::current().id() == coordinator)
            .into_iter().all(|local| local));
    }

    #[test]
    fn every_closed_helper_completes_the_unsent_batches_on_coordinator() {
        let mut executor = Executor::new();
        executor.workers = vec![closed_worker(), closed_worker()];
        executor.set_width(3);
        let coordinator = thread::current().id();
        assert_eq!(executor.ordered_map((0..7).collect(), move |index| (index, thread::current().id() == coordinator)),
            (0..7).map(|index| (index, true)).collect::<Vec<_>>());
        assert_eq!(executor.stats().jobs, 0);
        assert!(executor.spawn_failed());
    }

    #[test]
    fn individually_small_batches_do_not_accumulate_pressure() {
        let mut executor=Executor::new();
        for _ in 0..2 { executor.record_work(std::time::Duration::from_secs(1),40,20000); }
        executor.record_work(std::time::Duration::from_secs(1),100,100);
        assert_eq!(executor.stats().items,0);
        assert_eq!(executor.stats().comparisons,0);
        assert_eq!(executor.stats().cost,std::time::Duration::ZERO);
        executor.record_work(std::time::Duration::from_millis(1),64,16384);
        assert_eq!(executor.stats().items,64);
        assert_eq!(executor.stats().cost,std::time::Duration::from_millis(1));
    }
    #[test]
    fn serial_is_lazy_and_parallel_order_is_stable_across_ticks() {
        let mut executor = Executor::new();
        assert_eq!(executor.ordered_map(vec![3,2,1], |n| n*2), [6,4,2]);
        assert_eq!(executor.stats().workers, 0);
        executor.set_width(4);
        for _ in 0..3 {
            executor.begin_tick();
            let main = thread::current().id();
            let values = executor.ordered_map((0..128).collect(), move |n| (n, thread::current().id()!=main));
            assert!(values.iter().enumerate().all(|(i,v)| i==v.0));
            assert!(values.iter().any(|v|v.1));
            assert_eq!(executor.stats().workers,3);
            assert_eq!(executor.stats().jobs,3);
        }
        executor.set_width(1); executor.begin_tick();
        let main = thread::current().id();
        assert!(executor.ordered_map((0..128).collect(), move |_| thread::current().id()==main).into_iter().all(|same|same));
        assert_eq!(executor.stats().jobs,0);
        assert_eq!(executor.stats().workers,3);
        executor.set_width(2); executor.begin_tick();
        assert_eq!(executor.ordered_map((0..128).collect(), |n|n), (0..128).collect::<Vec<_>>());
        assert_eq!(executor.stats().jobs,1);
        assert_eq!(executor.stats().workers,3);
    }
    #[test]
    fn failed_executor_uses_only_tick_thread_even_with_existing_helpers() {
        let mut executor = Executor::new(); executor.set_width(2);
        executor.ordered_map(vec![0,1], |v| v);
        executor.failed = true; executor.begin_tick();
        let main = thread::current().id();
        assert_eq!(executor.ordered_map(vec![0,1,2], move |v| (v, thread::current().id() == main)), [(0,true),(1,true),(2,true)]);
        assert_eq!(executor.stats().jobs, 0);
        assert!(!executor.should_parallel(100, 20000));
    }

    #[test]
    fn panic_waits_for_workers_and_executor_remains_reusable() {
        let mut executor=Executor::new(); executor.set_width(2);
        let finished=Arc::new(std::sync::atomic::AtomicBool::new(false));
        let other=finished.clone();
        assert!(panic::catch_unwind(AssertUnwindSafe(|| executor.ordered_map(vec![0,1],move |n| {
            if n==1 { panic!("проверка барьера"); }
            thread::sleep(std::time::Duration::from_millis(10));
            other.store(true,std::sync::atomic::Ordering::SeqCst); n
        }))).is_err());
        assert!(finished.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(executor.ordered_map(vec![2,3], |n|n),[2,3]);
    }
}
