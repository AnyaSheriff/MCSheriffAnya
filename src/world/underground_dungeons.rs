// Комнаты пересекают границы чанков. Проверка — по неизменяемому снимку
// до комнат, рисование — в порядке координат источников. Загруженные
// чанки и порядок запросов не участвуют в выборе [проверить: пересечения
// нескольких комнат у оригинала зависят от порядка прохода декоратора].
use super::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

type Key = (i64, u8, i32, i32);

struct Cache<T> {
    entries: HashMap<Key, Rc<T>>,
    order: VecDeque<Key>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self { Self { entries: HashMap::new(), order: VecDeque::new() } }
}

impl<T> Cache<T> {
    fn insert(&mut self, key: Key, value: Rc<T>, limit: usize) {
        if self.entries.contains_key(&key) { return; }
        while self.entries.len() >= limit {
            if let Some(old) = self.order.pop_front() { self.entries.remove(&old); }
        }
        self.order.push_back(key);
        self.entries.insert(key, value);
    }
}

struct Room {
    spot: DungeonSpot,
    canvas: Chunk,
}

thread_local! {
    static BASE: RefCell<Cache<Chunk>> = RefCell::default();
    static ROOMS: RefCell<Cache<Vec<Room>>> = RefCell::default();
}

fn cache_key(generator: &Generator, x: i32, z: i32) -> Option<Key> {
    match generator {
        Generator::Normal(t) => Some((t.seed(), t.generation_profile(), x, z)),
        _ => None,
    }
}

pub(in crate::world) fn base_chunk(generator: &Generator, x: i32, z: i32) -> Rc<Chunk> {
    let Some(key) = cache_key(generator, x, z) else { return Rc::new(Chunk::underground_generated(generator, x, z)); };
    if let Some(chunk) = BASE.with_borrow(|cache| cache.entries.get(&key).cloned()) { return chunk; }
    let chunk = Rc::new(Chunk::underground_generated(generator, x, z));
    BASE.with_borrow_mut(|cache| cache.insert(key, chunk.clone(), 512));
    chunk
}

fn rooms(generator: &Generator, cx: i32, cz: i32, palette: &Palette) -> Rc<Vec<Room>> {
    let key = cache_key(generator, cx, cz).expect("обычный мир");
    if let Some(rooms) = ROOMS.with_borrow(|cache| cache.entries.get(&key).cloned()) { return rooms; }
    let Generator::Normal(terrain) = generator else { unreachable!() };
    let mut result = Vec::new();
    let mut snapshots = HashMap::new();
    let mut block = |x: i32, y: i32, z: i32| {
        let source = (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE));
        snapshots.entry(source).or_insert_with(|| base_chunk(generator, source.0, source.1))
            .block(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
    };
    for (spot, seed) in dungeon_spots(world_key(terrain), cx, cz) {
        if spot.y < MIN_Y || spot.y + DUNGEON_HEIGHT > TOP { continue; }
        if !dungeon_openings(&mut block, spot, palette).is_some_and(|n| DUNGEON_OPENINGS.contains(&n)) { continue; }
        let mut canvas = Chunk::new();
        for x in 0..spot.size_x {
            for z in 0..spot.size_z {
                for y in spot.y..spot.y + DUNGEON_HEIGHT {
                    canvas.put_generated(x, y, z, block(spot.local_x + x, y, spot.local_z + z), true);
                }
            }
        }
        let local = DungeonSpot { local_x: 0, local_z: 0, ..spot };
        assert!(try_dungeon(&mut canvas, local, seed, palette));
        result.push(Room { spot, canvas });
    }
    let result = Rc::new(result);
    ROOMS.with_borrow_mut(|cache| cache.insert(key, result.clone(), 512));
    result
}

fn paint(chunk: &mut Chunk, cx: i32, cz: i32, room: &Room) {
    let spot = room.spot;
    for x in 0..spot.size_x {
        for z in 0..spot.size_z {
            let (wx, wz) = (spot.local_x + x, spot.local_z + z);
            if (wx.div_euclid(CHUNK_SIZE), wz.div_euclid(CHUNK_SIZE)) != (cx, cz) { continue; }
            let (lx, lz) = (wx.rem_euclid(CHUNK_SIZE), wz.rem_euclid(CHUNK_SIZE));
            // Крыша остаётся природной, а не ещё одним слоем булыжника.
            for y in spot.y..spot.y + DUNGEON_HEIGHT - 1 {
                chunk.put_generated(lx, y, lz, room.canvas.block(x, y, z), true);
                if let Some(spawner) = room.canvas.spawners.get(&(x, y, z)) {
                    chunk.put_generated_spawner(lx, y, lz, spawner.clone());
                }
            }
        }
    }
}

pub(in crate::world) fn build_dungeons(generator: &Generator, chunk: &mut Chunk, cx: i32, cz: i32) {
    if !matches!(generator, Generator::Normal(_)) { return; }
    let palette = Palette::new();
    for x in cx-1..=cx+1 {
        for z in cz-1..=cz+1 {
            for room in rooms(generator, x, z, &palette).iter() { paint(chunk, cx, cz, room); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_paint_crosses_negative_chunk_border_without_wrapping() {
        let palette = Palette::new();
        let mut canvas = Chunk::new();
        for x in 0..7 { for z in 0..9 { for y in 20..26 { canvas.put_generated(x,y,z,palette.stone,true); } } }
        canvas.put_generated(0,21,4,AIR,true);
        canvas.put_generated(0,22,4,AIR,true);
        let local = DungeonSpot { local_x: 0, local_z: 0, y: 20, size_x: 7, size_z: 9 };
        assert!(try_dungeon(&mut canvas,local,17,&palette));
        let room = Room { spot: DungeonSpot { local_x: -3, local_z: -4, ..local }, canvas };
        let mut chunks = HashMap::new();
        for (cx,cz) in [(0,0),(-1,-1),(0,-1),(-1,0)] {
            let mut chunk = Chunk::new(); paint(&mut chunk,cx,cz,&room); chunks.insert((cx,cz),chunk);
        }
        for x in -3i32..4 { for z in -4i32..5 { for y in 20..25 {
            assert_eq!(chunks[&(x.div_euclid(16),z.div_euclid(16))].block(x.rem_euclid(16),y,z.rem_euclid(16)),room.canvas.block(x+3,y,z+4));
        } } }
        assert_eq!(chunks.values().map(|c|c.spawners.len()).sum::<usize>(),1);
        assert_eq!(chunks[&(0,0)].block(15,21,15),AIR);
        assert_eq!(chunks[&(0,0)].block(0,25,0),AIR,"крыша не рисуется");
    }

    #[test]
    fn room_validity_checks_floor_roof_and_all_openings() {
        let p = Palette::new();
        let spot = DungeonSpot { local_x: -3, local_z: -4, y: 20, size_x: 7, size_z: 9 };
        let valid = |x,y,z| if x == -3 && z == 0 && (21..=22).contains(&y) { AIR } else { p.stone };
        assert_eq!(dungeon_openings(valid,spot,&p),Some(1));
        for level in [20,25] {
            assert_eq!(dungeon_openings(|x,y,z|if (x,y,z)==(3,level,4) { AIR } else { valid(x,y,z) },spot,&p),None);
        }
        assert_eq!(dungeon_openings(|_,_,_|p.stone,spot,&p),Some(0));
        assert_eq!(dungeon_openings(|x,y,z|if x == -3 && (-3..3).contains(&z) && (21..=22).contains(&y) { AIR } else { p.stone },spot,&p),Some(6));
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;

    #[test]
    fn source_plans_are_independent_of_cache_and_request_order() {
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340,terrain::Style::Vanilla));
        let signature = |chunk: &Chunk| {
            let mut result = Vec::new();
            for y in -64..96 { for z in 0..16 { for x in 0..16 { result.push(chunk.block(x,y,z)); } } }
            result
        };
        let generate = |cx,cz| {
            let mut c = (*base_chunk(&generator,cx,cz)).clone();
            build_dungeons(&generator,&mut c,cx,cz);
            signature(&c)
        };
        let coords = [(-1,-1),(0,-1),(0,0),(-1,0)];
        let forward: Vec<_> = coords.iter().map(|&(x,z)|generate(x,z)).collect();
        BASE.with_borrow_mut(|c|*c=Cache::default());
        ROOMS.with_borrow_mut(|c|*c=Cache::default());
        for (i,&(x,z)) in coords.iter().enumerate().rev() { assert_eq!(generate(x,z),forward[i]); }
    }
}
