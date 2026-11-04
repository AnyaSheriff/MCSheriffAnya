// Лодки, плоты и лодки с сундуком: ход пустой лодки по воде, суше и воздуху.
//
// Правила — вики (Boat: «Motion details», «Flotation», «Destruction») и
// чёрный ящик (boat-float, boat-slide, boat-ride, boat-drive, boat-break).
// Лодку с гребцом ведёт клиент: он шлёт Move Vehicle, и сервер ставит лодку
// туда, куда сказано (network/play/vehicle.rs), а своего хода у неё тогда
// нет. Свой ход — у пустой (и у той, где сидит только моб).
//
// Числа сняты с оригинала чёрным ящиком, не взяты из кода: боксы 1,375 ×
// 0,5625 (minecraft-data), тяжесть 0,04 без сопротивления воздуха по
// вертикали (boat-float: −0,12, −0,24, −0,36 на 3, 6, 9-м тактах), трение
// 0,9 по горизонтали в воде и воздухе, на суше — скольжение блока под
// лодкой (boat-slide: 0,4 → 0,0864 за три такта на камне), всплытие —
// пружина по глубине погружения. Что не сверено, помечено «[проверить]».

use super::{Kind, Minecart, View};
use crate::fluids;

/// Ящик лодки (minecraft-data 26.1, entities.json: 1,375 × 0,5625).
pub const WIDTH: f64 = 1.375_f32 as f64;
pub const HEIGHT: f64 = 0.5625_f32 as f64;

/// Виды: имя (оно же предмет), номер вида сущности Java (minecraft-data
/// 26.1, entities.json), с сундуком ли.
pub const BOATS: [(&str, i32, bool); 22] = [
    ("oak_boat", 89, false),
    ("oak_chest_boat", 90, true),
    ("spruce_boat", 125, false),
    ("spruce_chest_boat", 126, true),
    ("birch_boat", 12, false),
    ("birch_chest_boat", 13, true),
    ("jungle_boat", 74, false),
    ("jungle_chest_boat", 75, true),
    ("acacia_boat", 0, false),
    ("acacia_chest_boat", 1, true),
    ("dark_oak_boat", 33, false),
    ("dark_oak_chest_boat", 34, true),
    ("mangrove_boat", 81, false),
    ("mangrove_chest_boat", 82, true),
    ("cherry_boat", 23, false),
    ("cherry_chest_boat", 24, true),
    ("pale_oak_boat", 94, false),
    ("pale_oak_chest_boat", 95, true),
    ("bamboo_raft", 9, false),
    ("bamboo_chest_raft", 8, true),
    // 26.3, только профиль Preview: внутренние номера 157/158 за последним видом 775;
    // клиенту 777 уходят 106/107 через ContentCodec (mcsa-protocol, content.rs).
    ("poplar_boat", 157, false),
    ("poplar_chest_boat", 158, true),
];

/// Все виды лодок — для подсказок в командах.
pub fn boat_kinds() -> impl Iterator<Item = Kind> {
    (0..BOATS.len() as u8).map(Kind::Boat)
}

/// По имени сущности: `oak_boat`, `bamboo_chest_raft`…
pub fn from_name(name: &str) -> Option<Kind> {
    BOATS.iter().position(|boat| boat.0 == name).map(|index| Kind::Boat(index as u8))
}

/// Сколько седоков помещается (вики, Boat: «can hold two entities»).
pub const SEATS: usize = 2;

/// Звуки лодки (minecraft-data 26.1, sounds.json).
pub const SPLASH_SOUND: &str = "minecraft:entity.generic.splash";
pub const PADDLE_WATER_SOUND: &str = "minecraft:entity.boat.paddle_water";
pub const PADDLE_LAND_SOUND: &str = "minecraft:entity.boat.paddle_land";

/// Что под лодкой и вокруг неё.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Status {
    /// Только что появилась: прошлого такта не было. Поставленная на воду
    /// лодка не «плюхается» с высоты, а садится плавно (boat-ride: с 8/9
    /// воды за 12 тактов −0,433, скорость −0,061 / −0,054 / −0,022 / +0,003).
    #[default]
    Fresh,
    /// В воздухе.
    Air,
    /// Стоит на суше.
    Land,
    /// Плавает: низ ящика в воде.
    InWater,
    /// Верх ящика ушёл под стоячую воду.
    Under,
    /// Верх ящика ушёл под текущую воду.
    UnderFlowing,
}

/// Состояние воды вокруг лодки между тактами.
#[derive(Clone, Copy, Debug, Default)]
pub struct Water {
    pub status: Status,
    /// Поверхность воды в ящике (верх самой высокой жидкости).
    pub level: f64,
    /// Трение суши: среднее скольжение блоков под лодкой.
    pub ground: f64,
}

/// Тяжесть лодки (boat-float).
const GRAVITY: f64 = 0.04;
/// Тяжесть под текущей водой [проверить].
const FLOWING_GRAVITY: f64 = 0.0007;
/// Трение в воде и воздухе, под водой.
const WET_FRICTION: f64 = 0.9;
const UNDER_FRICTION: f64 = 0.45;
/// Подъёмная сила: прибавка к скорости на долю погружения, потом общее
/// гашение 0,75 (boat-float: из воздуха лодка садится на воду так, что
/// на 12-м такте скорость −0,001, а потом сходится к глубине 0,366 ящика
/// (0,65 высоты: там тяжесть и пружина равны).
const BUOYANCY: f64 = 0.061_538_460_162_969_73;
const DAMPING: f64 = 0.75;
/// Всплывшая из воздуха лодка ставится на воду так: верх клетки воды минус
/// высота ящика плюс 0,101 (boat-float: 1,5385 при воде в клетке 1…2).
const SPLASH_LIFT: f64 = 0.101;
/// Вёсла: поворот весла за такт гребли — π/8, полный оборот — 16 тактов;
/// весло бьёт по воде, когда его угол проходит π/4. Счёт одинарной
/// точности: в первом обороте угол ровно π/4 попадает в окно два такта
/// подряд (boat-drive: плеск вёсел на +21, +22 и +37 от начала гребли
/// на +20).
const PADDLE_STEP: f32 = std::f32::consts::FRAC_PI_8;
const PADDLE_TURN: f32 = std::f32::consts::TAU;
const PADDLE_HIT: f32 = std::f32::consts::FRAC_PI_4;
/// Где слышен плеск весла: сбоку от лодки на 1 блок (левое — слева по
/// ходу), на высоте низа ящика (boat-drive: лодка в x = 3,5 — звуки в 4,5
/// и 2,5).
const PADDLE_SIDE: f64 = 1.0;

/// Скольжение блока: лёд, плотный лёд, голубой лёд, слизь; прочее — 0,6.
/// Вики, Boat: 0,98 / 0,989 / 0,8 / 0,6.
fn slipperiness(state: i32) -> f64 {
    match crate::blocks::block_at_state(state).map(|name| name.strip_prefix("minecraft:").unwrap_or(name)) {
        Some("ice" | "packed_ice" | "frosted_ice") => 0.98,
        Some("blue_ice") => 0.989,
        Some("slime_block") => 0.8,
        _ => 0.6,
    }
}

/// Высота воды в клетке: источник 8/9, течение (8−уровень)/9, падающая —
/// целая; если сверху тоже вода — целая.
fn fluid_height(state: i32, above: i32) -> Option<f64> {
    let (kind, level) = fluids::fluid_at(state)?;

    if kind != fluids::Kind::Water {
        return None;
    }

    if fluids::fluid_at(above).is_some_and(|(kind, _)| kind == fluids::Kind::Water) || level >= 8 {
        return Some(1.0);
    }

    // Высота жидкости у оригинала — число одинарной точности: поставленная на
    // поверхность лодка (низ ровно в 8/9) уже «в воде» (boat-ride: садится
    // плавно, без броска вниз).
    Some(f64::from((8 - level) as f32 / 9.0))
}

impl Minecart {
    /// Что под лодкой сейчас: статус, поверхность воды, трение суши.
    /// Второе число — верх клетки воды (куда ставится всплывшая из воздуха).
    fn boat_status<V: View>(&self, view: &V) -> (Water, f64) {
        let half = WIDTH / 2.0;
        let (low, high) = ((self.x - half, self.y, self.z - half), (self.x + half, self.y + HEIGHT, self.z + half));
        let cells = |from: f64, to: f64| (from.floor() as i32)..=((to - 1e-9).floor() as i32);
        let mut level = f64::MIN;
        let mut top = f64::MIN;
        let mut in_water = false;

        for bx in cells(low.0, high.0) {
            for bz in cells(low.2, high.2) {
                for by in cells(low.1, high.1) {
                    if let Some(height) = fluid_height(view.block(bx, by, bz), view.block(bx, by + 1, bz)) {
                        let surface = f64::from(by) + height;

                        level = level.max(surface);

                        if low.1 < surface {
                            in_water = true;
                            top = top.max(f64::from(by) + 1.0);
                        }
                    }
                }
            }
        }

        let mut water = Water { status: Status::Air, level, ground: 0.6 };

        // Верх ящика в воде: клетка на уровне верха — жидкость и выше её
        // поверхности верх не выступает.
        let (tx, ty, tz) = (self.x.floor() as i32, high.1.floor() as i32, self.z.floor() as i32);

        if let Some(height) = fluid_height(view.block(tx, ty, tz), view.block(tx, ty + 1, tz))
            && high.1 < f64::from(ty) + height
        {
            let flowing = fluids::fluid_at(view.block(tx, ty, tz)).is_some_and(|(_, level)| level != 0);

            water.status = if flowing { Status::UnderFlowing } else { Status::Under };

            return (water, top);
        }

        if in_water {
            water.status = Status::InWater;

            return (water, top);
        }

        // Суша: блоки под низом ящика; скольжение — среднее по ним.
        let below = (low.1 - 0.001).floor() as i32;
        let (mut sum, mut count) = (0.0, 0);

        for bx in cells(low.0 + 0.001, high.0 - 0.001) {
            for bz in cells(low.2 + 0.001, high.2 - 0.001) {
                let state = view.block(bx, below, bz);

                if crate::blocks::has_box(state) && (f64::from(below) + 1.0 - low.1).abs() < 0.002 {
                    sum += slipperiness(state);
                    count += 1;
                }
            }
        }

        if count > 0 {
            water.status = Status::Land;
            water.ground = sum / f64::from(count);
        }

        (water, top)
    }

    /// Такт лодки. Лодку с гребцом ведёт клиент — у неё только затухание
    /// удара и скорость нуль (`driver`).
    pub(super) fn step_boat<V: View>(&mut self, view: &V) {
        if self.driver.is_some() {
            self.vx = 0.0;
            self.vy = 0.0;
            self.vz = 0.0;
            self.on_ground = false;
            self.paddle_sounds(view);

            return;
        }

        // Впервые коснулась воды — плеск, как у предметов: громкость по
        // скорости (вертикаль весит впятеро больше). Проверка — в начале
        // такта, по месту и скорости после прошлого шага (boat-ride:
        // поставленная на воду — на +1, 0,006 = 0,2 · 0,03, в 1,859; boat-mob:
        // призванная на 1,9 — на +1, 0,008 = 0,2 · 0,04, в 1,86).
        let wet = self.touches_water(view);

        if wet && !self.wet {
            let volume = ((self.vx * self.vx * 0.2 + self.vy * self.vy + self.vz * self.vz * 0.2).sqrt() * 0.2).min(1.0) as f32;

            self.sounds.push((SPLASH_SOUND, (self.x, self.y, self.z), volume));
        }

        self.wet = wet;

        let old = self.water.status;
        let (mut water, top) = self.boat_status(view);
        let splash = old == Status::Air && !matches!(water.status, Status::Air | Status::Land);

        if splash {
            // Из воздуха в воду: ставится на воду, вертикальная скорость
            // гаснет, в этот такт ничего больше (boat-float: +11).
            self.y = top - HEIGHT + SPLASH_LIFT;
            self.vy = 0.0;
            water.status = Status::InWater;
            self.water = water;
        } else {
            self.water = water;
            self.float_boat();
        }

        self.set_paddle((false, false));
        self.move_by(view, self.vx, self.vy, self.vz);
        self.on_ground = self.on_ground && matches!(self.water.status, Status::Land);
    }

    /// Касается ли ящик лодки воды: ящик, ужатый на 0,001 со всех сторон,
    /// ниже поверхности хоть одной клетки (boat-ride: лодка, поставленная
    /// ровно на поверхность 8/9, в первый такт воды ещё не касается — плеск
    /// только тактом позже, когда она осела на 0,03).
    fn touches_water<V: View>(&self, view: &V) -> bool {
        const SHRINK: f64 = 0.001;

        let half = WIDTH / 2.0 - SHRINK;
        let (low, high) = (self.y + SHRINK, self.y + HEIGHT - SHRINK);
        let cells = |from: f64, to: f64| (from.floor() as i32)..=(to.floor() as i32);

        cells(self.x - half, self.x + half).any(|bx| {
            cells(self.z - half, self.z + half).any(|bz| {
                cells(low, high).any(|by| {
                    fluid_height(view.block(bx, by, bz), view.block(bx, by + 1, bz)).is_some_and(|height| low < f64::from(by) + height)
                })
            })
        })
    }

    /// Вёсла гребущего: опущенное весло поворачивается на π/8 за такт, на
    /// проходе угла π/4 — плеск (по воде) или стук (по суше).
    /// [проверить]: что с углом поднятого весла и звук в воздухе.
    fn paddle_sounds<V: View>(&mut self, view: &V) {
        let sound = match self.boat_status(view).0.status {
            Status::InWater | Status::Under | Status::UnderFlowing => PADDLE_WATER_SOUND,
            Status::Land => PADDLE_LAND_SOUND,
            Status::Air | Status::Fresh => return,
        };
        let (sin, cos) = f64::from(self.yaw).to_radians().sin_cos();

        for (index, down) in [self.paddle.0, self.paddle.1].into_iter().enumerate() {
            if !down {
                continue;
            }

            let phase = self.paddle_phase[index];

            if phase % PADDLE_TURN <= PADDLE_HIT && (phase + PADDLE_STEP) % PADDLE_TURN >= PADDLE_HIT {
                // Левое весло — слева по ходу: при взгляде на юг (+z) это +x.
                let side = if index == 0 { PADDLE_SIDE } else { -PADDLE_SIDE };

                self.sounds.push((sound, (self.x + cos * side, self.y, self.z + sin * side), 1.0));
            }

            self.paddle_phase[index] = phase + PADDLE_STEP;
        }
    }

    /// Стоящий на лодке игрок для неё — твёрдое тело: всплывать сквозь него
    /// она не может (boat-ride, boat-drive: слезший встаёт на верх лодки, и
    /// та, сидящая глубже покоя, больше не всплывает — ни сдвига, ни
    /// скорости 220 тактов). `below` — низ ящика до шага.
    /// [проверить]: игрок сбоку и снизу, мобы.
    pub(super) fn held_by(&mut self, players: &[(f64, f64, f64)], below: f64) {
        const PLAYER_HALF: f64 = 0.3;
        const EPSILON: f64 = 1.0e-7;

        let reach = WIDTH / 2.0 + PLAYER_HALF;

        for &(x, y, z) in players {
            let over = (x - self.x).abs() < reach && (z - self.z).abs() < reach;

            if over && y >= below + HEIGHT - EPSILON && self.y + HEIGHT > y {
                self.y = y - HEIGHT;
                self.vy = 0.0;
            }
        }
    }

    /// Вёсла: весло опущено или поднято (Paddle Boat от водителя).
    pub fn set_paddle(&mut self, paddle: (bool, bool)) {
        if self.paddle != paddle {
            self.paddle = paddle;
            self.dirty |= super::DIRTY_PADDLE;
        }
    }

    /// Водитель поставил лодку в точку (Move Vehicle): скорость сервера
    /// нулевая, её место — слово клиента.
    pub fn drive_to(&mut self, (x, y, z): (f64, f64, f64), yaw: f32) {
        (self.x, self.y, self.z) = (x, y, z);
        self.yaw = yaw;
        (self.vx, self.vy, self.vz) = (0.0, 0.0, 0.0);
    }

    /// Всплытие, тяжесть и трение (по статусу).
    fn float_boat(&mut self) {
        let (mut gravity, mut lift, friction) = (-GRAVITY, 0.0, WET_FRICTION);
        let mut friction = friction;

        match self.water.status {
            Status::InWater => lift = (self.water.level - self.y) / HEIGHT,
            Status::UnderFlowing => gravity = -FLOWING_GRAVITY,
            Status::Under => {
                lift = 0.01;
                friction = UNDER_FRICTION;
            }
            Status::Air | Status::Fresh => {}
            Status::Land => friction = self.water.ground,
        }

        self.vx *= friction;
        self.vz *= friction;
        self.vy += gravity;

        if lift > 0.0 {
            self.vy = (self.vy + lift * BUOYANCY) * DAMPING;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Бассейн как в сценарии boat-float: вода в клетках y = 0…1, камень вокруг.
    fn pool(x: i32, y: i32, z: i32) -> i32 {
        let stone = crate::blocks::state_by_name("stone").expect("камень есть");
        let inner = (1..=5).contains(&x) && (1..=5).contains(&z);
        let outer = (0..=6).contains(&x) && (0..=6).contains(&z);

        match y {
            ..=-1 => stone,
            0..=1 if inner => fluids::state_of(fluids::Kind::Water, 0),
            0..=1 if outer => stone,
            _ => crate::world::AIR,
        }
    }

    #[test]
    fn falls_into_water_and_floats_like_the_original() {
        // boat-float, чёрный ящик: из (3,5; 4; 3,5) на 12-м такте y = 1,5374,
        // скорость −0,001; потом глубина сходится к 1,5226.
        let mut boat = Minecart::new(1, Kind::Boat(0), (3.5, 4.0, 3.5));

        for _ in 0..12 {
            boat.step(&pool);
        }

        assert!((boat.y - 1.5374).abs() < 0.001, "y на 12-м такте: {}", boat.y);
        assert!((boat.vy + 0.001).abs() < 0.0005, "vy: {}", boat.vy);

        for _ in 0..108 {
            boat.step(&pool);
        }

        assert!((boat.y - 1.5226).abs() < 0.002, "y в покое: {}", boat.y);
    }

    #[test]
    fn placed_on_the_surface_settles_softly_and_splashes_a_tick_later() {
        // boat-ride: поставлена на 1 + 8/9 — без броска вниз; за 3 такта
        // −0,141, скорость −0,061; плеск — на втором шаге, громкость 0,006.
        let surface = 1.0 + f64::from(8.0_f32 / 9.0) - 1.0e-9;
        let mut boat = Minecart::new(1, Kind::Boat(0), (3.5, surface, 3.5));

        boat.step(&pool);
        assert!(boat.sounds.is_empty(), "в первый такт плеска нет");
        boat.step(&pool);
        let (name, _, volume) = boat.sounds[0];
        assert_eq!(name, SPLASH_SOUND);
        assert!((volume - 0.006).abs() < 1.0e-4, "громкость {}", volume);
        boat.step(&pool);
        assert!((boat.y - (surface - 0.1411)).abs() < 0.0003, "y: {}", boat.y);
        assert!((boat.vy + 0.061).abs() < 0.0005, "vy: {}", boat.vy);
    }

    #[test]
    fn paddles_splash_on_ticks_two_three_and_eighteen() {
        // boat-drive: гребля с +20 — плеск вёсел на +21, +22 и +37.
        let mut boat = Minecart::new(1, Kind::Boat(0), (3.5, 1.4, 3.5));
        let mut heard = Vec::new();

        boat.driver = Some(7);
        boat.paddle = (true, true);

        for tick in 0..30 {
            boat.step(&pool);

            if !std::mem::take(&mut boat.sounds).is_empty() {
                heard.push(tick);
            }
        }

        assert_eq!(heard, vec![1, 2, 17]);
    }

    #[test]
    fn player_on_top_holds_a_rising_boat() {
        // boat-ride: слезший стоит на верху лодки — она больше не всплывает.
        let mut boat = Minecart::new(1, Kind::Boat(0), (3.5, 1.4556, 3.5));

        for _ in 0..20 {
            let below = boat.y;

            boat.step(&pool);
            boat.held_by(&[(3.5, 1.4556 + HEIGHT, 3.5)], below);
        }

        assert!((boat.y - 1.4556).abs() < 1.0e-9 && boat.vy == 0.0, "y {} vy {}", boat.y, boat.vy);
    }

    #[test]
    fn free_fall_has_no_vertical_drag() {
        let mut boat = Minecart::new(1, Kind::Boat(0), (3.5, 40.0, 3.5));

        for _ in 0..9 {
            boat.step(&pool);
        }

        assert!((boat.vy + 0.36).abs() < 1e-9);
        assert!((boat.y - (40.0 - 1.8)).abs() < 1e-9);
    }

    #[test]
    fn slides_on_stone_with_point_six_friction() {
        // boat-slide: 0,4 → 0,0864 за три такта на камне.
        let floor = |_x: i32, y: i32, _z: i32| if y < 0 { crate::blocks::state_by_name("stone").expect("камень") } else { crate::world::AIR };
        let mut boat = Minecart::new(1, Kind::Boat(0), (1.5, 0.0, 1.5));

        boat.vx = 0.4;

        for _ in 0..3 {
            boat.step(&floor);
        }

        assert!((boat.vx - 0.4 * 0.6_f64.powi(3)).abs() < 1e-9, "vx: {}", boat.vx);
        assert!((boat.x - (1.5 + 0.24 + 0.144 + 0.0864)).abs() < 1e-9);
    }

    #[test]
    fn driven_boat_stays_where_the_client_put_it() {
        let mut boat = Minecart::new(1, Kind::Boat(3), (3.5, 4.0, 3.5));

        boat.driver = Some(7);
        boat.drive_to((3.5, 6.0, 9.0), 90.0);
        boat.step(&pool);

        assert_eq!((boat.x, boat.y, boat.z), (3.5, 6.0, 9.0));
        assert_eq!(boat.kind.name(), "spruce_chest_boat");
    }

    #[test]
    fn every_boat_has_a_name_a_type_and_an_item_twin() {
        for kind in boat_kinds() {
            assert_eq!(Kind::from_name(kind.name()), Some(kind));
            assert!(kind.java_type() >= 0);
        }

        assert_eq!(boat_kinds().count(), 22);
    }
}
