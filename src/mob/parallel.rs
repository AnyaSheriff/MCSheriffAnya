// Независимые снимки наблюдения; мир, случайность и события остаются в потоке такта.
use std::{any::Any, panic::{self, AssertUnwindSafe}, sync::{mpsc, Arc}, thread};

type Job = Box<dyn FnOnce() + Send>;
type Batch<R> = (usize, Result<Vec<R>, Box<dyn Any + Send>>);
struct Worker { sender: mpsc::Sender<Job>, handle: thread::JoinHandle<()> }
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats { pub jobs: usize, pub items: usize, pub workers: usize, pub cost: std::time::Duration, pub comparisons: usize }
pub struct Executor { workers: Vec<Worker>, width: usize, stats: Stats, failed: bool }
impl Default for Executor { fn default() -> Self { Self::new() } }
impl Executor {
    pub fn new() -> Self { Self { workers: Vec::new(), width: 1, stats: Stats::default(), failed: false } }
    pub fn set_width(&mut self, width: usize) { self.width = width.clamp(1, 8); }
    pub fn width(&self) -> usize { self.width }
    pub fn spawn_failed(&self) -> bool { self.failed }
    pub fn begin_tick(&mut self) { self.stats = Stats { workers: self.workers.len(), ..Stats::default() }; }
    pub fn stats(&self) -> Stats { self.stats }
    pub fn should_parallel(&self, items: usize, comparisons: usize) -> bool {
        self.width > 1 && items >= 64 && comparisons >= 16384 && !self.failed
    }
    pub fn record_work(&mut self, cost: std::time::Duration, items: usize, comparisons: usize) {
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
        let width = width.min(self.workers.len()+1);
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
                local = Some((index, panic::catch_unwind(AssertUnwindSafe(|| chunk.into_iter().map(|v| map(v)).collect()))));
            } else {
                let done = done.clone();
                self.workers[index].sender.send(Box::new(move || {
                    let value = panic::catch_unwind(AssertUnwindSafe(|| chunk.into_iter().map(|v| map(v)).collect()));
                    let _ = done.send((index, value));
                })).unwrap_or_else(|_| panic!("поток наблюдения мобов остановился"));
                self.stats.jobs += 1;
            }
        }
        drop(done);
        let mut collected: Vec<_> = results.into_iter().collect();
        collected.extend(local);
        collected.sort_by_key(|(index, _)| *index);
        let mut output = Vec::new();
        let mut failed = None;
        for (_, value) in collected {
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
