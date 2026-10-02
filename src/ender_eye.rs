// Око Края, брошенное в воздух: летит к ближайшей крепости и через 80
// тактов падает предметом или раскалывается.
//
// Вики (Eye of Ender): «fly approximately 12 blocks in the direction of the
// nearest stronghold», ведёт к углу чанка старта («chunk coordinates 0, ~,
// 0»), ближе 12 блоков — опускается; «20% chance of shattering and an 80%
// chance of surviving». Сам полёт снят чёрным ящиком (eye-of-ender-far,
// eye-of-ender-near — своя копия оригинала с семенем 1234567890, ближайшая
// крепость [1584, ~, 480]): сверки раз в 4 такта совпали с законом ниже до
// тысячной на всех 20 сверках по горизонтали и вертикали.
//
// Цель — точка в 12 блоках по горизонтали к крепости и на 8 выше места
// появления; если крепость ближе 12 блоков — сама крепость. Такт: ход на
// скорость; горизонтальная скорость тянется к расстоянию до цели на 0,0025
// доли, направление — прямо на цель; ближе 1 блока по горизонтали обе
// скорости гасятся на 0,8; вертикальная тянется на 0,015 к +1 или −1 —
// смотря, ниже ли цели было око до хода.

use crate::dimension::Dimension;
use crate::inventory::Stack;

/// Номер вида сущности (minecraft-data 26.1, entities.json).
pub const EYE_TYPE: i32 = 50;

/// Сколько тактов око летит (eye-of-ender-far: раскол на +80).
pub const LIFETIME: u32 = 80;

/// Доля оков, которые падают предметом (вики: 80 %).
pub const SURVIVE: f32 = 0.8;

/// Как далеко око улетает к далёкой крепости и на сколько поднимается.
const REACH: f64 = 12.0;
const RISE: f64 = 8.0;

/// Место, скорость.
pub type State = ((f64, f64, f64), (f64, f64, f64));

/// Летящее око.
#[derive(Clone, Debug)]
pub struct Eye {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub dimension: Dimension,
    /// Предмет ока (свойство 8): одно око.
    pub stack: Stack,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub target: (f64, f64, f64),
    /// Сколько ходов сделано.
    pub age: u32,
    /// Место и скорость до хода этого такта: слежение показывает их.
    pub prev: State,
}

/// Куда летит око из `from` к крепости в точке `stronghold`.
pub fn aim(from: (f64, f64, f64), stronghold: (f64, f64, f64)) -> (f64, f64, f64) {
    let (dx, dz) = (stronghold.0 - from.0, stronghold.2 - from.2);
    let distance = dx.hypot(dz);

    if distance > REACH {
        (from.0 + dx / distance * REACH, from.1 + RISE, from.2 + dz / distance * REACH)
    } else {
        stronghold
    }
}

impl Eye {
    /// Око, брошенное игроком: появляется на середине его роста без
    /// скорости (eye-of-ender-far: появление на +0,9 от ног, скорость 0).
    pub fn thrown(entity_id: i32, dimension: Dimension, stack: Stack, at: (f64, f64, f64), stronghold: (f64, f64, f64)) -> Self {
        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            dimension,
            stack,
            x: at.0,
            y: at.1,
            z: at.2,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            target: aim(at, stronghold),
            age: 0,
            prev: (at, (0.0, 0.0, 0.0)),
        }
    }

    /// Один такт полёта. Возвращает, пора ли оку пропасть.
    pub fn step(&mut self) -> bool {
        self.prev = ((self.x, self.y, self.z), (self.vx, self.vy, self.vz));
        self.age += 1;

        let below = self.y < self.target.1;

        self.x += self.vx;
        self.y += self.vy;
        self.z += self.vz;

        let horizontal = self.vx.hypot(self.vz);
        let (dx, dz) = (self.target.0 - self.x, self.target.2 - self.z);
        let distance = dx.hypot(dz);
        let angle = dz.atan2(dx);
        let mut speed = horizontal + (distance - horizontal) * 0.0025;
        let mut vy = self.vy;

        if distance < 1.0 {
            speed *= 0.8;
            vy *= 0.8;
        }

        let pull = if below { 1.0 } else { -1.0 };

        self.vy = vy + (pull - vy) * 0.015;
        self.vx = angle.cos() * speed;
        self.vz = angle.sin() * speed;

        self.age > LIFETIME
    }
}

/// Око пропало в этом такте: слежение досылает его последнюю сверку, потом
/// звук entity.ender_eye.death, уход и у расколотого — событие 2003
/// (eye-of-ender-far); упавшее становится предметом.
#[derive(Clone, Debug)]
pub struct Gone {
    pub last: Eye,
    pub shattered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Полёт против записи оригинала (eye-of-ender-far: из (0,5; 150,9; 0,5)
    /// к крепости [1584, ~, 480]; сдвиги раз в 4 такта, в 1/4096).
    #[test]
    fn the_flight_matches_the_recording() {
        let mut eye = Eye::thrown(1, Dimension::Overworld, Stack::new(0, 1), (0.5, 150.9, 0.5), (1584.0, 0.0, 480.0));
        type Point = (f64, f64, f64);
        let recorded: [(u32, Point, Point); 6] = [
            (4, (0.172, 0.089, 0.052), (0.114, 0.059, 0.034)),
            (16, (3.240, 1.680, 0.981), (0.404, 0.215, 0.122)),
            (32, (11.152, 6.364, 3.377), (0.354, 0.253, 0.107)),
            (44, (11.456, 8.024, 3.469), (0.024, 0.081, 0.007)),
            (52, (11.490, 8.069, 3.479), (-0.004, -0.048, -0.001)),
            (76, (11.485, 8.038, 3.478), (0.0, 0.048, 0.0)),
        ];
        let mut tick = 0;

        for (at, place, velocity) in recorded {
            while tick < at {
                assert!(!eye.step());
                tick += 1;
            }

            let (p, v) = ((eye.x - 0.5, eye.y - 150.9, eye.z - 0.5), (eye.vx, eye.vy, eye.vz));

            assert!((p.0 - place.0).abs() < 2e-3 && (p.1 - place.1).abs() < 2e-3 && (p.2 - place.2).abs() < 2e-3, "+{}: {:?} против {:?}", at, p, place);
            assert!((v.0 - velocity.0).abs() < 2e-3 && (v.1 - velocity.1).abs() < 2e-3 && (v.2 - velocity.2).abs() < 2e-3, "+{}: {:?} против {:?}", at, v, velocity);
        }

        // Пропадает на 81-м ходу (раскол на +80).
        while tick < LIFETIME {
            assert!(!eye.step());
            tick += 1;
        }
        assert!(eye.step());
    }

    /// Крепость ближе 12 блоков: око летит прямо к углу чанка и вниз
    /// (eye-of-ender-near: скорость вниз сходится к −0,071).
    #[test]
    fn a_near_stronghold_pulls_the_eye_down() {
        let mut eye = Eye::thrown(1, Dimension::Overworld, Stack::new(0, 1), (1592.5, 150.9, 486.5), (1584.0, -20.0, 480.0));

        assert_eq!(eye.target, (1584.0, -20.0, 480.0));

        for _ in 0..52 {
            eye.step();
        }

        assert!((eye.x - 1592.5 - -8.504).abs() < 2e-3 && (eye.z - 486.5 - -6.503).abs() < 2e-3);
        assert!((eye.y - 150.9 - -8.632).abs() < 2e-3, "{}", eye.y - 150.9);
        assert!((eye.vy - -0.072).abs() < 2e-3);
    }
}
