// Стрела: сущность-снаряд.
//
// Кода игры здесь нет; числа — со страницы minecraft.wiki «Arrow»: ящик 0,5 на
// 0,5; тяжесть 0,05 за такт; сопротивление воздуха 0,99 за такт (в воде 0,6);
// урон — «damage NBT tag · speed, rounded up» (у оригинала damage по умолчанию
// 2); воткнувшаяся в блок стрела живёт минуту (1200 тактов); стрелы, выпущенные
// мобами, подобрать нельзя. Что не сверено, помечено «[проверить]» — числа,
// которых на вики нет (запас при попадании, звук, разлёт), снимаются чёрным
// ящиком (сценарий skeleton-arrow).
//
// Такт стрелы: сперва луч от нынешнего места на нынешнюю скорость — блок или
// игрок на пути; потом сдвиг, `v *= 0,99`, `v.y -= 0,05`.

use crate::blocks;
use crate::mob::View;

/// Тяжесть и сопротивление воздуха за такт (вики, Arrow).
pub const GRAVITY: f64 = 0.05;
pub const DRAG: f64 = 0.99;
pub const WATER_DRAG: f64 = 0.6;

/// Размер ящика (вики, Arrow: 0,5 × 0,5).
pub const SIZE: f64 = 0.5;

/// Сколько тактов живёт воткнувшаяся в блок стрела (вики, Arrow: «one minute»).
pub const GROUND_LIFE: i32 = 1200;

/// Запас, на который ящик игрока раздувается для попадания: подобрано так,
/// как раздувается ящик цели у снаряда (`0,3`) [проверить].
const MARGIN: f64 = 0.3;

/// Номер вида сущности «стрела» (minecraft-data 26.1, entities.json).
pub const JAVA_TYPE: i32 = 6;

/// Стрела.
#[derive(Clone, Debug)]
pub struct Arrow {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub yaw: f32,
    pub pitch: f32,

    /// Кто выпустил (номер сущности); 0 — никто.
    pub owner: i32,

    /// Ключ имени стрелка для сообщения о смерти.
    pub killer: &'static str,

    /// Воткнулась в блок.
    pub in_ground: bool,

    /// Блок, в который воткнулась (по нему видно, что опору убрали).
    pub stuck_in: (i32, i32, i32),

    /// Сколько тактов сидит в блоке.
    pub life: i32,

    /// Сколько тактов стрела живёт вообще.
    pub ticks: u64,

    /// Базовый урон (вики: 2), умножается на скорость.
    pub base_damage: f64,

    /// Можно ли подобрать (у стрел мобов — нельзя; подбора игроком пока нет).
    #[allow(dead_code)]
    pub pickup: bool,

    /// Убрать: попала или истекла.
    pub gone: bool,

    /// Сколько раз менялись свойства (воткнулась) — клиентам уходит обновление.
    pub changes: u32,
}

/// Игрок как цель стрелы: номер, ноги, размер ящика.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub width: f64,
    pub height: f64,
}

/// Попадание в игрока.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub target: i32,
    pub damage: f32,
    /// Направление полёта по горизонтали: отсюда отдача.
    pub dir: (f64, f64),
}

impl Arrow {
    pub fn new(entity_id: i32, at: (f64, f64, f64), velocity: (f64, f64, f64), owner: i32, killer: &'static str) -> Self {
        let mut arrow = Self {
            entity_id,
            uuid: uuid_for(entity_id),
            x: at.0,
            y: at.1,
            z: at.2,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            yaw: 0.0,
            pitch: 0.0,
            owner,
            killer,
            in_ground: false,
            stuck_in: (0, 0, 0),
            life: 0,
            ticks: 0,
            base_damage: 2.0,
            pickup: false,
            gone: false,
            changes: 0,
        };

        arrow.aim();
        arrow
    }

    /// Скорость, блоков за такт.
    pub fn speed(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy + self.vz * self.vz).sqrt()
    }

    /// Поворот по направлению полёта (у снарядов угол без знака минус:
    /// yaw = atan2(vx, vz), pitch = atan2(vy, |v_xz|)).
    pub fn aim(&mut self) {
        let flat = (self.vx * self.vx + self.vz * self.vz).sqrt();

        self.yaw = self.vx.atan2(self.vz).to_degrees() as f32;
        self.pitch = self.vy.atan2(flat).to_degrees() as f32;
    }

    /// Урон при попадании: `урон · скорость`, вверх до целого (вики, Arrow).
    pub fn damage(&self) -> f32 {
        (self.base_damage * self.speed()).ceil() as f32
    }

    /// Такт стрелы. Возвращает попадание в игрока, если оно было.
    pub fn step(&mut self, view: &dyn View, targets: &[Target]) -> Option<Strike> {
        self.ticks += 1;

        let cell = (self.x.floor() as i32, self.y.floor() as i32, self.z.floor() as i32);

        if !view.loaded(cell.0.div_euclid(16), cell.2.div_euclid(16)) {
            return None;
        }

        // Воткнувшаяся: живёт, пока держит опора; убрали блок — падает дальше.
        if self.in_ground {
            let (x, y, z) = self.stuck_in;

            if crate::mob::solid(view.block(x, y, z)) {
                self.life += 1;

                if self.life >= GROUND_LIFE {
                    self.gone = true;
                }

                return None;
            }

            self.in_ground = false;
            self.changes += 1;
            self.life = 0;
        }

        let from = (self.x, self.y, self.z);
        let to = (self.x + self.vx, self.y + self.vy, self.z + self.vz);
        let block = first_block(view, from, to);
        let block_at = block.map_or(f64::INFINITY, |(t, _)| t);
        let mut best: Option<(f64, i32)> = None;

        for target in targets {
            if let Some(t) = hits_box(from, to, target) && t < block_at && best.is_none_or(|(before, _)| t < before) {
                best = Some((t, target.id));
            }
        }

        if let Some((t, id)) = best {
            let flat = (self.vx * self.vx + self.vz * self.vz).sqrt().max(1.0e-9);
            let strike = Strike { target: id, damage: self.damage(), dir: (self.vx / flat, self.vz / flat) };

            self.x = from.0 + (to.0 - from.0) * t;
            self.y = from.1 + (to.1 - from.1) * t;
            self.z = from.2 + (to.2 - from.2) * t;
            self.gone = true;

            return Some(strike);
        }

        if let Some((t, cell)) = block {
            // Втыкается в точке входа в блок; скорость гаснет.
            self.x = from.0 + (to.0 - from.0) * t;
            self.y = from.1 + (to.1 - from.1) * t;
            self.z = from.2 + (to.2 - from.2) * t;
            self.vx = 0.0;
            self.vy = 0.0;
            self.vz = 0.0;
            self.in_ground = true;
            self.stuck_in = cell;
            self.life = 0;
            self.changes += 1;

            return None;
        }

        self.x = to.0;
        self.y = to.1;
        self.z = to.2;

        let water = crate::mob::is_water(view.block(self.x.floor() as i32, self.y.floor() as i32, self.z.floor() as i32));
        let drag = if water { WATER_DRAG } else { DRAG };

        self.vx *= drag;
        self.vy = self.vy * drag - GRAVITY;
        self.vz *= drag;
        self.aim();

        None
    }
}

/// Первое твёрдое на пути: доля пути `t` до входа в блок и клетка блока.
fn first_block(view: &dyn View, from: (f64, f64, f64), to: (f64, f64, f64)) -> Option<(f64, (i32, i32, i32))> {
    let (dx, dy, dz) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    let steps = (length / 0.05).ceil().max(1.0) as i32;

    for step in 1..=steps {
        let t = f64::from(step) / f64::from(steps);
        let cell = ((from.0 + dx * t).floor() as i32, (from.1 + dy * t).floor() as i32, (from.2 + dz * t).floor() as i32);

        if blocks::stops_items(view.block(cell.0, cell.1, cell.2)) {
            // Точка входа в блок: делением пополам между прошлым шагом и этим.
            let (mut low, mut high) = (f64::from(step - 1) / f64::from(steps), t);

            for _ in 0..24 {
                let mid = (low + high) / 2.0;
                let inside = blocks::stops_items(view.block(
                    (from.0 + dx * mid).floor() as i32,
                    (from.1 + dy * mid).floor() as i32,
                    (from.2 + dz * mid).floor() as i32,
                ));

                if inside {
                    high = mid;
                } else {
                    low = mid;
                }
            }

            return Some((high, cell));
        }
    }

    None
}

/// Пересечение отрезка с ящиком цели, раздутым на запас: доля пути `t`.
fn hits_box(from: (f64, f64, f64), to: (f64, f64, f64), target: &Target) -> Option<f64> {
    let half = target.width / 2.0 + MARGIN + SIZE / 2.0 - 0.25;
    let min = [target.x - half, target.y - MARGIN, target.z - half];
    let max = [target.x + half, target.y + target.height + MARGIN, target.z + half];
    let (start, delta) = ([from.0, from.1, from.2], [to.0 - from.0, to.1 - from.1, to.2 - from.2]);
    let (mut near, mut far) = (0.0_f64, 1.0_f64);

    for axis in 0..3 {
        if delta[axis].abs() < 1.0e-12 {
            if start[axis] < min[axis] || start[axis] > max[axis] {
                return None;
            }

            continue;
        }

        let (a, b) = ((min[axis] - start[axis]) / delta[axis], (max[axis] - start[axis]) / delta[axis]);
        let (low, high) = if a < b { (a, b) } else { (b, a) };

        near = near.max(low);
        far = far.min(high);

        if near > far {
            return None;
        }
    }

    Some(near)
}

/// Опознаватель сущности.
pub fn uuid_for(entity_id: i32) -> [u8; 16] {
    let mut uuid = crate::items::uuid_for(entity_id);

    uuid[12..].copy_from_slice(b"arw!");
    uuid
}

/// Начальная скорость выстрела: от точки `from` в точку `aim`, с поправкой
/// вверх на дальность (`+ дальность · 0,2` по высоте — так целятся стрелки,
/// чтобы стрела не упала раньше цели) и разбросом: каждая ось получает
/// нормальную добавку `error · 0,0075`; длина вектора приводится к `speed`.
/// `gauss` выдаёт нормальные числа с нулевым средним и единичным разбросом.
pub fn launch(from: (f64, f64, f64), aim: (f64, f64, f64), speed: f64, error: f64, gauss: &mut dyn FnMut() -> f64) -> (f64, f64, f64) {
    let (dx, mut dy, dz) = (aim.0 - from.0, aim.1 - from.1, aim.2 - from.2);
    let flat = (dx * dx + dz * dz).sqrt();

    dy += flat * 0.2;

    let len = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);
    let (nx, ny, nz) = (dx / len, dy / len, dz / len);
    let spread = 0.0075 * error;
    let (vx, vy, vz) = (nx + gauss() * spread, ny + gauss() * spread, nz + gauss() * spread);

    (vx * speed, vy * speed, vz * speed)
}

#[cfg(test)]
mod tests;
