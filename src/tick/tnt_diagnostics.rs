// Только явная диагностика: снимки одного выбранного такта, не суммы за интервал.
use crate::{dimension::Dimension, items::{Dropped, Items}, world::World};
use serde::Serialize;

const LIMIT: usize = 64;
const BUDGET: u16 = 120;

pub(super) fn enabled(value: Option<&str>) -> bool { value == Some("1") }

pub(super) struct Diagnostics { remaining: [u16; 3] }
impl Diagnostics {
    pub(super) fn from_env() -> Option<Self> {
        enabled(std::env::var("MCSA_TNT_DIAGNOSTICS").ok().as_deref())
            .then_some(Self { remaining: [BUDGET; 3] })
    }
    pub(super) fn due(&self, tick: u64, dimension: Dimension) -> bool {
        tick.is_multiple_of(20) && self.remaining[dimension.index()] > 0
    }
    pub(super) fn begin(&mut self, tick: u64, dimension: Dimension, items: &Items, world: &World, wait: u64) -> Option<Batch> {
        if !self.due(tick, dimension) { return None; }
        let before = snapshot(items, world);
        if before.total == 0 { return None; }
        self.remaining[dimension.index()] -= 1;
        Some(Batch { tick, dimension: dimension.name(), sampling: "one_step_every_20_ticks", budget_exhausted: self.remaining[dimension.index()] == 0,
            world_lock_wait_us: wait, item_stage_elapsed_us: 0, before, after_step: Snapshot::default(), exploding: 0,
            sampled_fuses_after_burn: Vec::new() })
    }
}

#[derive(Default, Serialize)]
struct Snapshot { total: usize, loaded: usize, omitted: usize, samples: Vec<Entity> }
#[derive(Serialize)]
struct Entity { id: i32, position: [f64; 3], velocity: [f64; 3], fuse: i32, chunk: [i32; 2], loaded: bool }
impl Entity {
    fn capture(item: &Dropped, world: &World) -> Self {
        let chunk = [(item.x.floor() as i32).div_euclid(16), (item.z.floor() as i32).div_euclid(16)];
        Self { id: item.entity_id, position: [item.x, item.y, item.z], velocity: [item.vx, item.vy, item.vz], fuse: item.fuse.expect("динамит"), chunk,
            loaded: world.has_chunk(chunk[0], chunk[1]) }
    }
}
fn snapshot(items: &Items, world: &World) -> Snapshot {
    let mut result = Snapshot::default();
    for item in items.items().iter().filter(|item| item.fuse.is_some()) {
        result.total += 1;
        let chunk = [(item.x.floor() as i32).div_euclid(16), (item.z.floor() as i32).div_euclid(16)];
        result.loaded += usize::from(world.has_chunk(chunk[0], chunk[1]));
        if result.samples.len() < LIMIT { result.samples.push(Entity::capture(item, world)); }
    }
    result.omitted = result.total.saturating_sub(result.samples.len());
    result
}

#[derive(Serialize)]
pub(super) struct Batch {
    tick: u64, dimension: &'static str, sampling: &'static str, budget_exhausted: bool,
    world_lock_wait_us: u64, item_stage_elapsed_us: u64,
    before: Snapshot, after_step: Snapshot, exploding: usize,
    // Все выбранные ID после burn_fuses: None означает удаление в exploding.
    sampled_fuses_after_burn: Vec<(i32, Option<i32>)>,
}
impl Batch {
    pub(super) fn after_step(&mut self, items: &Items, world: &World) { self.after_step = snapshot(items, world); }
    pub(super) fn finish(&mut self, items: &Items, exploding: usize, elapsed: u64) {
        self.exploding = exploding;
        self.item_stage_elapsed_us = elapsed;
        self.sampled_fuses_after_burn = self.before.samples.iter().map(|sample| {
            (sample.id, items.items().iter().find(|item| item.entity_id == sample.id).and_then(|item| item.fuse))
        }).collect();
    }
    pub(super) fn log(self) {
        if let Ok(json) = serde_json::to_string(&self) { crate::log_info!("MCSA_TNT_DIAGNOSTICS {}", json); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn primed(count: usize) -> Items {
        let mut items = Items::new();
        for id in 0..count { items.prime(id as i32, 0, (15.9, 64.0, 0.5), (0.5, 0.0, 0.0), 2, None); }
        items
    }
    #[test]
    fn opt_in_is_exact() {
        assert!(enabled(Some("1")));
        for value in [None, Some("0"), Some("true"), Some(" 1")] { assert!(!enabled(value)); }
    }
    #[test]
    fn snapshot_is_bounded_and_stages_keep_ids() {
        let mut world = World::in_memory(); world.ensure(0, 0);
        let mut items = primed(70);
        let mut diagnostics = Diagnostics { remaining: [BUDGET; 3] };
        let mut batch = diagnostics.begin(20, Dimension::Overworld, &items, &world, 3).unwrap();
        assert_eq!((batch.before.total, batch.before.loaded, batch.before.omitted, batch.before.samples.len()), (70, 70, 6, LIMIT));
        items.step(&|_, _, _| false, &|_, _, _| false, &|x, z| world.has_chunk(x, z));
        batch.after_step(&items, &world);
        let exploding = items.burn_fuses(&|x, z| world.has_chunk(x, z));
        batch.finish(&items, exploding.len(), 4);
        assert_eq!((batch.after_step.total, batch.after_step.loaded, batch.after_step.omitted), (70, 0, 6));
        assert_eq!(batch.before.samples[0].id, batch.after_step.samples[0].id);
        assert_eq!(batch.sampled_fuses_after_burn[0], (0, Some(2)));
        assert_eq!(batch.exploding, 0);
    }
    #[test]
    fn budget_counts_nonempty_samples_per_dimension() {
        let world = World::in_memory();
        let mut diagnostics = Diagnostics { remaining: [1; 3] };
        assert!(!diagnostics.due(19, Dimension::Overworld));
        assert!(diagnostics.begin(20, Dimension::Overworld, &Items::new(), &world, 0).is_none());
        assert!(diagnostics.due(20, Dimension::Overworld));
        let mut items = primed(1);
        let mut batch = diagnostics.begin(20, Dimension::Overworld, &items, &world, 0).unwrap();
        assert!(batch.budget_exhausted);
        assert!(!diagnostics.due(40, Dimension::Overworld));
        assert!(diagnostics.begin(40, Dimension::Overworld, &items, &world, 0).is_none());
        assert!(diagnostics.due(40, Dimension::Nether));
        items.items_mut()[0].fuse = Some(1);
        batch.after_step(&items, &world);
        let exploding = items.burn_fuses(&|_, _| true);
        batch.finish(&items, exploding.len(), 0);
        assert_eq!(batch.sampled_fuses_after_burn, vec![(0, None)]);
        assert_eq!(batch.exploding, 1);
    }
}
