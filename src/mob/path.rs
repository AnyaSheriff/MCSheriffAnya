// Поиск пути: A* по клеткам, на которых можно стоять.
//
// Вики (Pathfinding): каждая клетка — узел с видом («Node types»): проходимый
// (walkable), закрытый (blocked), опасный (огонь, лава, кактус — обходится
// или закрыт), вода (штраф для сухопутного). Соседи — четыре стороны и
// диагонали, если обе соседние клетки свободны; шаг вверх на клетку — прыжком;
// шаг вниз — не глубже безопасного падения в три блока («safe fall distance»).
// Не найдя цели, моб идёт в самый близкий к ней достигнутый узел
// («if no path is found, the mob goes to the closest reachable point»).
//
// Что не сверено, помечено «[проверить]»: число просматриваемых узлов и
// цены — взяты правдоподобными; точные числа игры не известны.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use super::{View, is_lava, is_water, solid};

#[cfg(test)]
#[allow(dead_code)]
#[path = "path_reference.rs"]
mod reference;
use crate::blocks;

/// Клетка мира.
pub type Cell = (i32, i32, i32);

/// Сколько узлов просматривает поиск, прежде чем сдаться (вики, Pathfinding:
/// у мобов предел в несколько сотен узлов).
pub const MAX_NODES: usize = 400;

/// Насколько глубоко моб готов спрыгнуть (вики, Fall damage: три блока
/// безопасны).
pub const MAX_DROP: i32 = 3;

/// Ограда, стенка и калитка выше блока: на них не встать и через них не
/// перепрыгнуть (вики, Fence: «cannot be jumped over»).
fn tall(state: i32) -> bool {
    blocks::block_at_state(state).is_some_and(|name| name.ends_with("_fence") || name.ends_with("_fence_gate") || name.ends_with("_wall"))
}

/// Опасное для ног: огонь, лава, кактус, магма, ягодный куст, костёр.
fn danger(state: i32) -> bool {
    is_lava(state)
        || matches!(
            blocks::block_at_state(state),
            Some("fire" | "soul_fire" | "cactus" | "magma_block" | "sweet_berry_bush" | "campfire" | "soul_campfire" | "powder_snow" | "cobweb" | "wither_rose")
        )
}

const FREE: u8 = 1;
const FLOOR: u8 = 2;
const WATER: u8 = 4;
const SOLID: u8 = 8;
const TALL: u8 = 16;

fn classify(state: i32) -> u8 {
    let (solid, danger, tall, water) = (solid(state), danger(state), tall(state), is_water(state));
    (u8::from(!solid && !danger) * FREE)
        | (u8::from((solid && !tall && !danger) || water) * FLOOR)
        | (u8::from(water) * WATER) | (u8::from(solid) * SOLID) | (u8::from(tall) * TALL)
}

fn properties(state: i32) -> u8 {
    // Это свойства состояния, не клетки: правка мира не требует сброса.
    static TABLE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| (0..=u16::MAX).map(|s| classify(i32::from(s))).collect());
    table.get(state as usize).copied().unwrap_or_else(|| classify(state))
}

/// Свободна ли клетка для тела моба: без твёрдого блока и без опасного.
fn free(view: &dyn View, (x, y, z): Cell) -> bool {
    let state = view.block(x, y, z);

    properties(state) & FREE != 0
}

/// Можно ли стоять в клетке: клетка и клетка над ней свободны, под ногами —
/// твёрдое (или вода, по ней плывут).
pub fn standable(view: &dyn View, cell: Cell) -> bool { standable_height(view,cell,2) }

fn standable_height(view:&dyn View,(x,y,z):Cell,height:i32)->bool {
    if !(0..height).all(|dy|free(view,(x,y+dy,z))) {
        return false;
    }

    let below = view.block(x, y - 1, z);

    properties(below) & FLOOR != 0
}

/// Цена клетки: вода дороже (вики, Pathfinding: «water malus 8» у наземных).
fn cost_of(view: &dyn View, (x, y, z): Cell) -> f64 {
    if properties(view.block(x, y, z)) & WATER != 0 || properties(view.block(x, y - 1, z)) & WATER != 0 { 8.0 } else { 0.0 }
}

/// Куда можно шагнуть из клетки: (клетка, цена шага).
fn neighbours(view: &dyn View, (x, y, z): Cell, height:i32, out: &mut Vec<(Cell, f64)>) {
    out.clear();

    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
        let diagonal = dx != 0 && dz != 0;

        // Диагональ — только если обе клетки рядом свободны (вики, Pathfinding).
        if diagonal && !(standable_or_open(view, (x + dx, y, z),height) && standable_or_open(view, (x, y, z + dz),height)) {
            continue;
        }

        let step = if diagonal { std::f64::consts::SQRT_2 } else { 1.0 };
        let there = (x + dx, y, z + dz);

        if standable_height(view, there,height) {
            out.push((there, step + cost_of(view, there)));
            continue;
        }

        // Шаг вверх на одну клетку: над головой должно быть свободно.
        let up = (x + dx, y + 1, z + dz);

        if !diagonal && free(view, (x, y + height, z)) && standable_height(view, up,height) && properties(view.block(x + dx, y, z + dz)) & TALL == 0 {
            out.push((up, step + 0.5 + cost_of(view, up)));
            continue;
        }

        // Шаг вниз: до трёх блоков.
        if free(view, there) && free(view, (there.0, there.1 + 1, there.2)) {
            for drop in 1..=MAX_DROP {
                let down = (there.0, there.1 - drop, there.2);

                if !free(view, down) {
                    break;
                }

                if standable_height(view, down,height) {
                    out.push((down, step + f64::from(drop) * 0.5 + cost_of(view, down)));
                    break;
                }
            }
        }
    }

}

/// Клетка без твёрдого блока — через неё можно пройти по диагонали.
fn standable_or_open(view: &dyn View, cell: Cell,height:i32) -> bool {
    (0..height).all(|dy|properties(view.block(cell.0,cell.1+dy,cell.2)) & SOLID == 0)
}

/// Расстояние между клетками — прикидка для A*.
fn distance(a: Cell, b: Cell) -> f64 {
    let (dx, dy, dz) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1), f64::from(a.2 - b.2));

    (dx * dx + dy * dy * 0.25 + dz * dz).sqrt()
}

/// Путь от `from` к `to`: клетки по порядку, без начальной. Пусто — идти
/// некуда. Если цель недостижима, путь ведёт в самую близкую к ней
/// достигнутую клетку (если она ближе, чем начало).
pub fn find(view: &dyn View, from: Cell, to: Cell, max_nodes: usize) -> Vec<Cell> {
    find_height(view,from,to,max_nodes,2)
}

/// Хранителю нужен проход высотой три блока (вики, Warden: «2.9 blocks»).
pub fn find_height(view:&dyn View,from:Cell,to:Cell,max_nodes:usize,height:i32)->Vec<Cell> {
    // Цена в тысячных, чтобы упорядочивать целыми.
    let key = |cost: f64| (cost * 1000.0) as i64;

    let mut open: BinaryHeap<Reverse<(i64, i64, Cell)>> = BinaryHeap::new();
    let mut best: HashMap<Cell, f64> = HashMap::new();
    let mut came: HashMap<Cell, Cell> = HashMap::new();
    let mut closest = (distance(from, to), from);
    let mut seen = 0;
    let mut adjacent = Vec::with_capacity(8);

    best.insert(from, 0.0);
    open.push(Reverse((key(distance(from, to)), 0, from)));

    while let Some(Reverse((_, _, at))) = open.pop() {
        if at == to {
            closest = (0.0, at);
            break;
        }

        seen += 1;

        if seen > max_nodes {
            break;
        }

        let cost = best[&at];

        neighbours(view, at, height, &mut adjacent);
        for &(next, step) in &adjacent {
            let total = cost + step;

            if best.get(&next).is_none_or(|known| total < *known) {
                best.insert(next, total);
                came.insert(next, at);

                let to_go = distance(next, to);

                if to_go < closest.0 {
                    closest = (to_go, next);
                }

                open.push(Reverse((key(total + to_go), key(total), next)));
            }
        }
    }

    if closest.1 == from {
        return Vec::new();
    }

    let mut path = vec![closest.1];
    let mut at = closest.1;

    while let Some(&before) = came.get(&at) {
        if before == from {
            break;
        }

        path.push(before);
        at = before;
    }

    path.reverse();
    path
}

#[cfg(test)]
mod perf_tests {
    use super::*;

    struct Terrain { seed: u64, states: Vec<i32> }
    impl View for Terrain {
        fn block(&self, x: i32, y: i32, z: i32) -> i32 {
            if y < -2 { return blocks::state_by_name("stone").unwrap(); }
            if y > 3 { return 0; }
            let mut hash = self.seed ^ (x as u64).wrapping_mul(7919)
                ^ (y as u64).wrapping_mul(104729) ^ (z as u64).wrapping_mul(15485863);
            hash = hash.wrapping_mul(6364136223846793005).rotate_left(23);
            self.states[hash as usize % self.states.len()]
        }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }

    #[test]
    fn cached_flags_match_original_predicates() {
        for state in (-1..=i32::from(u16::MAX) + 1).chain([i32::MIN, i32::MAX]) {
            assert_eq!(properties(state), classify(state), "{state}");
        }
    }

    #[test]
    fn paths_and_standability_match_original_search() {
        let names = ["air", "air", "air", "stone", "water", "lava", "oak_fence",
            "oak_fence_gate", "cobweb", "snow", "oak_slab", "oak_stairs", "magma_block",
            "fire", "powder_snow", "cactus", "bubble_column", "lily_pad"];
        for seed in 0..48 {
            let view = Terrain { seed, states: names.iter().map(|n| blocks::state_by_name(n).unwrap()).collect() };
            for x in -3..=3 { for z in -3..=3 { for y in -2..=4 {
                assert_eq!(standable(&view, (x,y,z)), reference::standable(&view, (x,y,z)));
            }}}
            for height in [1, 2, 3] { for budget in [0, 1, 20, MAX_NODES] {
                for goal in [(0,0,0), (5,0,5), (-5,-1,2)] {
                    assert_eq!(find_height(&view, (0,0,0), goal, budget, height),
                        reference::find_height(&view, (0,0,0), goal, budget, height),
                        "seed {seed}, height {height}, budget {budget}, goal {goal:?}");
                }
            }}
        }
    }
}
