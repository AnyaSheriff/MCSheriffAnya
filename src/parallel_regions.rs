//! Основа планирования, не исполнитель: игровой такт этот модуль пока не вызывает.
//!
//! Компоненты независимы только при ПОЛНОМ описании ресурсов вызывающей стороной.
//! Геометрическое расстояние не доказывает отсутствие зависимостей. Неизвестный
//! доступ требует Unknown, активные плагины — ForceSerial. Сам план не разрешает
//! передавать World между потоками и не меняет существующий бюджет помощников.

use std::collections::{BTreeMap, BTreeSet};

/// Любое u64, включая 0/MAX, допустимо; в одном плане ID обязаны быть уникальны.
pub type TaskId = u64;

/// Идентичность ресурса, а не координатная эвристика. world — внутренний ключ мира.
/// Между вариантами нет неявного покрытия: чтение занятости чанка и сущности
/// обязано явно перечислить оба ресурса. Неполное описание требует Unknown.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Resource {
    Chunk { world: u32, x: i32, z: i32 },
    Entity { world: u32, id: i32 },
    JobSiteClaims { world: u32 },
    GlobalRng,
    EntityIds,
    OrderedEffects,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    Read(Resource),
    Write(Resource),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Footprint {
    /// Полный набор чтений/записей всей задачи, включая транзитивные вызовы.
    Known(Vec<Access>),
    /// Неизвестная задача объединяется со ВСЕМИ, включая задачи других миров.
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub id: TaskId,
    pub footprint: Footprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    DeclaredDependencies,
    ForceSerial,
}

/// План относится к одному замороженному описанию входа одного логического такта.
/// epoch меняет координатор при изменении входов/владения до исполнения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub tick: u64,
    pub epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    stamp: Stamp,
    tasks: Vec<Task>,
    policy: Policy,
    groups: Vec<Vec<TaskId>>,
}

impl Plan {
    /// Порядок групп — по первому входному индексу, внутри — исходный serial-порядок.
    /// Численные TaskId не задают порядок и не пересортировываются.
    pub fn groups(&self) -> &[Vec<TaskId>] { &self.groups }

    /// Проверяет также входные ownership/access описания, даже если вызывающая
    /// сторона забыла поменять epoch. Не читает World и не доказывает полноту
    /// описания; реальные изменения мира обязан отразить его владелец.
    pub fn validate(&self, stamp: Stamp, tasks: &[Task], policy: Policy) -> Result<(), StalePlan> {
        if self.stamp != stamp { return Err(StalePlan::Stamp); }
        if self.policy != policy { return Err(StalePlan::Policy); }
        if self.tasks != tasks { return Err(StalePlan::Tasks); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StalePlan { Stamp, Policy, Tasks }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanError {
    DuplicateTaskId(TaskId),
}

/// Строит компоненты конфликтов read/write; read/read не является конфликтом.
/// Общий writer соединяет всех пользователей ресурса, даже находящихся далеко.
/// Зависимые операции сохраняют прежний порядок в одной группе. Независимость
/// не разрешает менять порядок видимых эффектов: они используют OrderedEffects.
pub fn build(stamp: Stamp, tasks: &[Task], policy: Policy) -> Result<Plan, PlanError> {
    let mut ids = BTreeSet::new();
    for task in tasks {
        if !ids.insert(task.id) { return Err(PlanError::DuplicateTaskId(task.id)); }
    }
    let plan = |groups| Plan { stamp, tasks: tasks.to_vec(), policy, groups };
    if tasks.is_empty() { return Ok(plan(Vec::new())); }
    if policy == Policy::ForceSerial || tasks.iter().any(|task| task.footprint == Footprint::Unknown) {
        return Ok(plan(vec![tasks.iter().map(|task| task.id).collect()]));
    }

    let mut users: BTreeMap<Resource, Vec<(usize, bool)>> = BTreeMap::new();
    for (index, task) in tasks.iter().enumerate() {
        let Footprint::Known(accesses) = &task.footprint else { unreachable!() };
        // Повторное чтение/запись ресурса не создаёт лишних рёбер; запись сильнее.
        let mut unique = BTreeMap::new();
        for access in accesses {
            let (resource, write) = match *access {
                Access::Read(resource) => (resource, false),
                Access::Write(resource) => (resource, true),
            };
            unique.entry(resource).and_modify(|old| *old |= write).or_insert(write);
        }
        for (resource, write) in unique {
            users.entry(resource).or_default().push((index, write));
        }
    }

    let mut components = Components((0..tasks.len()).collect());
    for entries in users.values() {
        if let Some(&(writer, _)) = entries.iter().find(|(_, write)| *write) {
            for &(index, _) in entries { components.join(writer, index); }
        }
    }
    let mut groups: BTreeMap<usize, Vec<TaskId>> = BTreeMap::new();
    for (index, task) in tasks.iter().enumerate() {
        groups.entry(components.root(index)).or_default().push(task.id);
    }
    Ok(plan(groups.into_values().collect()))
}

struct Components(Vec<usize>);

impl Components {
    fn root(&mut self, mut index: usize) -> usize {
        while self.0[index] != index {
            self.0[index] = self.0[self.0[index]];
            index = self.0[index];
        }
        index
    }

    fn join(&mut self, left: usize, right: usize) {
        let left = self.root(left);
        let right = self.root(right);
        self.0[left.max(right)] = left.min(right);
    }
}

#[cfg(test)]
#[path = "parallel_regions/tests.rs"]
mod tests;
