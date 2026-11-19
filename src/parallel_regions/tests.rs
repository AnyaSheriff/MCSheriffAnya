use super::*;

const STAMP: Stamp = Stamp { tick: 70, epoch: 9 };
const POLICY: Policy = Policy::DeclaredDependencies;
const A: Resource = Resource::Chunk { world: 0, x: -40, z: 0 };
const B: Resource = Resource::Chunk { world: 0, x: 4000, z: 0 };

fn task(id: TaskId, accesses: Vec<Access>) -> Task {
    Task { id, footprint: Footprint::Known(accesses) }
}

fn groups(tasks: &[Task]) -> Vec<Vec<TaskId>> {
    build(STAMP, tasks, POLICY).unwrap().groups().to_vec()
}

#[test]
fn empty_and_pure_tasks_keep_input_order_and_all_u64_ids_are_valid() {
    assert!(groups(&[]).is_empty());
    let tasks = [task(u64::MAX, vec![]), task(0, vec![]), task(7, vec![])];
    assert_eq!(groups(&tasks), [vec![u64::MAX], vec![0], vec![7]]);
}

#[test]
fn duplicate_ids_are_rejected_even_with_unknown_or_forced_serial() {
    for policy in [POLICY, Policy::ForceSerial] {
        for footprint in [Footprint::Known(vec![]), Footprint::Unknown] {
            let tasks = [Task { id: 3, footprint }, task(3, vec![])];
            assert_eq!(build(STAMP, &tasks, policy), Err(PlanError::DuplicateTaskId(3)));
        }
    }
}

#[test]
fn read_read_shares_input_but_any_writer_merges_all_readers() {
    let mut tasks = vec![task(30, vec![Access::Read(A)]), task(2, vec![Access::Read(A)])];
    assert_eq!(groups(&tasks), [vec![30], vec![2]]);
    tasks.push(task(1, vec![Access::Write(A)]));
    assert_eq!(groups(&tasks), [vec![30, 2, 1]]);
}

#[test]
fn transitive_mechanism_is_joined_regardless_of_distance() {
    let tasks = [
        task(8, vec![Access::Write(A)]),
        task(90, vec![Access::Read(A), Access::Write(B)]),
        task(2, vec![Access::Read(B)]),
        task(1, vec![]),
    ];
    assert_eq!(groups(&tasks), [vec![8, 90, 2], vec![1]]);
}

#[test]
fn distinct_worlds_entities_and_resource_kinds_do_not_alias() {
    let resources = [
        A,
        Resource::Chunk { world: 1, x: -40, z: 0 },
        Resource::Chunk { world: 0, x: -39, z: 0 },
        Resource::Entity { world: 0, id: 12 },
        Resource::Entity { world: 1, id: 12 },
        Resource::Entity { world: 0, id: 13 },
        Resource::JobSiteClaims { world: 0 },
        Resource::JobSiteClaims { world: 1 },
        Resource::GlobalRng,
        Resource::EntityIds,
        Resource::OrderedEffects,
    ];
    let tasks: Vec<_> = resources.into_iter().enumerate()
        .map(|(index, resource)| task(index as u64, vec![Access::Write(resource)])).collect();
    assert_eq!(groups(&tasks), (0..11).map(|id| vec![id]).collect::<Vec<_>>());
}

#[test]
fn global_rng_ids_effects_and_claims_prevent_false_actor_independence() {
    for resource in [Resource::GlobalRng, Resource::EntityIds, Resource::OrderedEffects,
        Resource::JobSiteClaims { world: 0 }] {
        let tasks = [task(9, vec![Access::Write(A), Access::Write(resource)]),
            task(4, vec![Access::Write(B), Access::Write(resource)])];
        assert_eq!(groups(&tasks), [vec![9, 4]]);
    }
}

#[test]
fn unknown_and_plugin_policy_merge_even_unrelated_pure_tasks() {
    let mut tasks = vec![task(9, vec![Access::Write(A)]), task(4, vec![Access::Write(B)]), task(0, vec![])];
    assert_eq!(groups(&tasks).len(), 3);
    assert_eq!(build(STAMP, &tasks, Policy::ForceSerial).unwrap().groups(), [vec![9, 4, 0]]);
    tasks[1].footprint = Footprint::Unknown;
    assert_eq!(groups(&tasks), [vec![9, 4, 0]]);
}

#[test]
fn normalization_ignores_access_order_repetition_and_read_before_write() {
    let tasks = [task(3, vec![Access::Read(A), Access::Write(B), Access::Read(A), Access::Write(A)]),
        task(2, vec![Access::Read(B)]), task(1, vec![Access::Read(A)])];
    let normalized = [task(3, vec![Access::Write(A), Access::Write(B)]), tasks[1].clone(), tasks[2].clone()];
    assert_eq!(groups(&tasks), groups(&normalized));
    let mut reversed_accesses = tasks.clone();
    for task in &mut reversed_accesses {
        if let Footprint::Known(accesses) = &mut task.footprint { accesses.reverse(); }
    }
    assert_eq!(groups(&tasks), groups(&reversed_accesses));
}

#[test]
fn validation_rejects_changed_stamp_policy_order_or_ownership() {
    let tasks = [task(10, vec![Access::Write(A)]), task(1, vec![Access::Read(A)])];
    let plan = build(STAMP, &tasks, POLICY).unwrap();
    assert_eq!(plan.validate(STAMP, &tasks, POLICY), Ok(()));
    for stamp in [Stamp { tick: 71, ..STAMP }, Stamp { epoch: 10, ..STAMP }] {
        assert_eq!(plan.validate(stamp, &tasks, POLICY), Err(StalePlan::Stamp));
    }
    assert_eq!(plan.validate(STAMP, &tasks, Policy::ForceSerial), Err(StalePlan::Policy));
    let mut moved = tasks.clone();
    moved[0].footprint = Footprint::Known(vec![Access::Write(B)]);
    assert_eq!(plan.validate(STAMP, &moved, POLICY), Err(StalePlan::Tasks));
    let mut reordered = tasks.clone(); reordered.reverse();
    assert_eq!(plan.validate(STAMP, &reordered, POLICY), Err(StalePlan::Tasks));
    assert_eq!(plan.validate(STAMP, &tasks[..1], POLICY), Err(StalePlan::Tasks));
}

#[test]
fn completed_groups_reassemble_into_original_serial_order() {
    let tasks = [
        task(u64::MAX, vec![Access::Write(A)]),
        task(0, vec![Access::Write(B)]),
        task(7, vec![Access::Read(A)]),
    ];
    let plan = build(STAMP, &tasks, POLICY).unwrap();
    assert_eq!(plan.groups(), [vec![u64::MAX, 7], vec![0]]);
    let completed = plan.groups()
        .iter()
        .rev()
        .flatten()
        .map(|&id| TaskResult { stamp: STAMP, id, value: Box::new(id) })
        .collect();
    let ordered = plan.collect_results(STAMP, &tasks, POLICY, completed).unwrap();
    assert_eq!(ordered, [Box::new(u64::MAX), Box::new(0), Box::new(7)]);

    let empty = build(STAMP, &[], POLICY).unwrap();
    assert!(empty.collect_results::<()>(STAMP, &[], POLICY, vec![]).unwrap().is_empty());
}

#[test]
fn collecting_results_requires_exactly_one_value_for_each_task() {
    let tasks = [task(8, vec![]), task(2, vec![])];
    let plan = build(STAMP, &tasks, POLICY).unwrap();
    let collect = |ids: &[TaskId]| {
        let completed = ids.iter().map(|&id| TaskResult { stamp: STAMP, id, value: () }).collect();
        plan.collect_results(STAMP, &tasks, POLICY, completed)
    };
    assert_eq!(collect(&[8, 8]), Err(ResultError::DuplicateResult(8)));
    assert_eq!(collect(&[8, 5]), Err(ResultError::UnexpectedTask(5)));
    assert_eq!(collect(&[8]), Err(ResultError::MissingResult(2)));
    assert_eq!(collect(&[]), Err(ResultError::MissingResult(8)));
}

#[test]
fn collecting_results_rejects_stale_inputs_and_results_for_reused_ids() {
    let tasks = [task(8, vec![Access::Read(A)])];
    let plan = build(STAMP, &tasks, POLICY).unwrap();
    let completed = || vec![TaskResult { stamp: STAMP, id: 8, value: () }];
    let moved = [task(8, vec![Access::Read(B)])];
    assert_eq!(
        plan.collect_results(STAMP, &moved, POLICY, completed()),
        Err(ResultError::StalePlan(StalePlan::Tasks)),
    );
    assert_eq!(
        plan.collect_results(STAMP, &tasks, Policy::ForceSerial, completed()),
        Err(ResultError::StalePlan(StalePlan::Policy)),
    );
    for stamp in [Stamp { tick: 71, ..STAMP }, Stamp { epoch: 10, ..STAMP }] {
        assert_eq!(
            plan.collect_results(stamp, &tasks, POLICY, completed()),
            Err(ResultError::StalePlan(StalePlan::Stamp)),
        );
        let current = build(stamp, &tasks, POLICY).unwrap();
        assert_eq!(
            current.collect_results(stamp, &tasks, POLICY, completed()),
            Err(ResultError::StaleResult(8)),
        );
    }
}

#[test]
fn new_tick_can_split_after_bridge_disappears_without_retained_ownership() {
    let tasks = [task(10, vec![Access::Write(A)]),
        task(7, vec![Access::Read(A), Access::Write(B)]), task(1, vec![Access::Read(B)])];
    let old = build(STAMP, &tasks, POLICY).unwrap();
    assert_eq!(old.groups(), [vec![10, 7, 1]]);
    let next = [tasks[0].clone(), tasks[2].clone()];
    let stamp = Stamp { tick: STAMP.tick + 1, epoch: STAMP.epoch + 1 };
    let new = build(stamp, &next, POLICY).unwrap();
    assert_eq!(new.groups(), [vec![10], vec![1]]);
    assert_eq!(old.validate(stamp, &next, POLICY), Err(StalePlan::Stamp));
    assert_eq!(new.validate(stamp, &next, POLICY), Ok(()));
}

// Независимый oracle: попарный граф и транзитивное замыкание, без union/find.
fn reference(tasks: &[Task]) -> Vec<Vec<TaskId>> {
    let count = tasks.len();
    let mut connected = vec![vec![false; count]; count];
    for (left, a) in tasks.iter().enumerate() {
        connected[left][left] = true;
        for (right, b) in tasks.iter().enumerate() {
            let (Footprint::Known(a), Footprint::Known(b)) = (&a.footprint, &b.footprint) else { panic!() };
            connected[left][right] |= a.iter().any(|a| b.iter().any(|b| match (a, b) {
                (Access::Read(_), Access::Read(_)) => false,
                (Access::Read(a) | Access::Write(a), Access::Read(b) | Access::Write(b)) => a == b,
            }));
        }
    }
    let mut via = 0;
    while via < count {
        let mut left = 0;
        while left < count {
            let mut right = 0;
            while right < count {
                connected[left][right] |= connected[left][via] && connected[via][right];
                right += 1;
            }
            left += 1;
        }
        via += 1;
    }
    let mut seen = vec![false; count];
    let mut groups = Vec::new();
    for (first, _) in tasks.iter().enumerate() {
        if seen[first] { continue; }
        let mut group = Vec::new();
        for (index, task) in tasks.iter().enumerate() {
            if connected[first][index] { seen[index] = true; group.push(task.id); }
        }
        groups.push(group);
    }
    groups
}

fn encoded_tasks(mut encoded: u32) -> Vec<Task> {
    [40, 2, 19, 0].into_iter().map(|id| {
        let mut accesses = Vec::new();
        for resource in [A, B] {
            match encoded % 3 {
                1 => accesses.push(Access::Read(resource)),
                2 => accesses.push(Access::Write(resource)),
                _ => (),
            }
            encoded /= 3;
        }
        task(id, accesses)
    }).collect()
}

#[test]
fn exhaustive_6561_graphs_match_reference_partition_and_stable_order() {
    for encoded in 0..3_u32.pow(8) {
        let tasks = encoded_tasks(encoded);
        assert_eq!(groups(&tasks), reference(&tasks), "набор доступов {encoded}");
    }
}

// Модель наблюдаемых чтений/записей, а не игровых Mob::tick. Чтения влияют
// на результат и все записи задачи: перестановка настоящей зависимости видна.
fn execute(tasks: &[Task], order: &[TaskId]) -> ([u64; 2], Vec<u64>) {
    let mut state = [11_u64, 23_u64];
    let mut observed = vec![0; tasks.len()];
    for id in order {
        let index = tasks.iter().position(|task| task.id == *id).unwrap();
        let Footprint::Known(accesses) = &tasks[index].footprint else { panic!() };
        let sum = accesses.iter().fold(0_u64, |sum, access| {
            let (Access::Read(resource) | Access::Write(resource)) = access;
            sum.wrapping_mul(13).wrapping_add(state[usize::from(*resource == B)])
        });
        observed[index] = sum;
        for access in accesses {
            if let Access::Write(resource) = access {
                let target = &mut state[usize::from(*resource == B)];
                *target = target.wrapping_mul(17).wrapping_add(sum).wrapping_add(*id);
            }
        }
    }
    (state, observed)
}

#[test]
fn exhaustive_6561_models_preserve_reads_and_writes_when_groups_finish_in_reverse() {
    for encoded in 0..3_u32.pow(8) {
        let tasks = encoded_tasks(encoded);
        let serial: Vec<_> = tasks.iter().map(|task| task.id).collect();
        let reversed: Vec<_> = groups(&tasks).iter().rev().flatten().copied().collect();
        assert_eq!(execute(&tasks, &serial), execute(&tasks, &reversed), "набор доступов {encoded}");
    }
}

#[test]
fn model_oracle_detects_illegal_reordering_of_real_dependency() {
    let tasks = [task(40, vec![Access::Write(A)]), task(2, vec![Access::Read(A), Access::Write(B)])];
    assert_ne!(execute(&tasks, &[40, 2]), execute(&tasks, &[2, 40]));
    assert_eq!(groups(&tasks), [vec![40, 2]]);
}
