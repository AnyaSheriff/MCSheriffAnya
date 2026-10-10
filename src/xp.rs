// Шарики опыта — сущности мира.
//
// Шарик общий: его видят все рядом, он летит к ближайшему игроку и подбирается
// тем, до кого долетел (вики, Experience orb). Живёт он в общем списке
// сущностей (`Items`), как предметы, а рассылка идёт слежением: появление —
// по журналу, движение — как у мобов, но раз в 20 тактов (чёрный ящик,
// xp-orb-far, cow-xp).
//
// Что снято с оригинала чёрным ящиком:
// - призванный командой шарик появляется без скорости, выпавший из моба — с
//   толчком `(2·(r−0,5)·0,2; r·0,4; 2·(r−0,5)·0,2)`;
// - падает с тяжестью 0,03 и сопротивлением 0,98; на землю ложится с
//   коротким отскоком (у падения с 3 блоков через 20 тактов он на 0,315 выше
//   пола и почти в верхней точке) — отскок подобран под этот замер,
//   множитель `BOUNCE` [проверить: на других высотах];
// - подбирается по одному в 2 такта на игрока (вики: «10 orbs/second»);
// - три шарика в одной точке (1, 1 и 3) не слились за 60 тактов.

/// Тяжесть за такт (вики, Experience orb; чёрный ящик xp-orb-far).
const GRAVITY: f64 = 0.03;

/// Сопротивление воздуха и трение о землю (блок с трением 0,6).
const DRAG: f64 = 0.98;
const GROUND_FRICTION: f64 = 0.6;

/// Какая часть скорости падения возвращается отскоком (подбор по замеру
/// xp-orb-far: 0,315 над полом на 20-м такте после падения с 3 блоков).
const BOUNCE: f64 = 0.4082;

/// Отскок слабее этой скорости гасится: шарик лежит.
const BOUNCE_MIN: f64 = 0.03;

/// Дальше этого от игрока шарик не тянет (вики: 7,25 от ног до центра шарика;
/// сама тяга — `(1 − d/8)²·0,1` в сторону центра тела).
const PULL_RANGE: f64 = 8.0;

/// Через сколько тактов шарик пропадает (вики: 5 минут).
pub const LIFETIME: u64 = 6000;

/// Сколько тактов игрок ждёт между двумя шариками (10 в секунду).
pub const TAKE_DELAY: u32 = 2;

/// Как далеко до шарика дотягивается игрок: рамка 0,6 × 1,8, раздвинутая на
/// блок по сторонам и на полблока по высоте, задевает рамку шарика 0,5 × 0,5.
const REACH: f64 = 0.3 + 1.0 + 0.25;
const REACH_ABOVE: f64 = 1.8 + 0.5;
const REACH_BELOW: f64 = 0.5 + 0.5;

#[derive(Clone, Debug)]
pub struct XpOrb {
    pub entity_id: i32,
    pub uuid: [u8; 16],

    /// Сколько очков даёт одно подбирание.
    pub value: i32,

    /// Сколько раз его ещё можно подобрать (слитые шарики).
    pub count: i32,

    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub on_ground: bool,

    /// Тактов жизни: по нему идёт и рассылка.
    pub age: u64,
    pub ticks: u64,
}

impl XpOrb {
    pub fn new(entity_id: i32, value: i32, at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Self {
        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            value,
            count: 1,
            x: at.0,
            y: at.1,
            z: at.2,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            on_ground: false,
            age: 0,
            ticks: 0,
        }
    }

    /// Такт: тяжесть, тяга к игроку `pull` (центр его тела, если он ближе
    /// 8 блоков), движение, сопротивление, отскок.
    pub fn step(&mut self, pull: Option<(f64, f64, f64)>, solid: &dyn Fn(i32, i32, i32) -> bool) {
        self.vy -= GRAVITY;

        if let Some((px, py, pz)) = pull {
            let toward = (px - self.x, py - self.y, pz - self.z);
            let distance = (toward.0 * toward.0 + toward.1 * toward.1 + toward.2 * toward.2).sqrt();

            if distance < PULL_RANGE && distance > 1e-6 {
                let strength = (1.0 - distance / PULL_RANGE).powi(2) * 0.1 / distance;

                self.vx += toward.0 * strength;
                self.vy += toward.1 * strength;
                self.vz += toward.2 * strength;
            }
        }

        self.x += self.vx;
        self.z += self.vz;

        let mut y = self.y + self.vy;

        self.on_ground = false;

        if self.vy < 0.0 {
            // Пол — верх первого твёрдого блока на пути вниз.
            let (bx, bz) = (self.x.floor() as i32, self.z.floor() as i32);
            let mut by = self.y.floor() as i32;
            let bottom = y.floor() as i32;

            while by >= bottom {
                if solid(bx, by, bz) {
                    let floor = f64::from(by) + 1.0;

                    if floor > y && floor <= self.y + 1e-9 {
                        y = floor;
                        self.on_ground = true;
                    }

                    break;
                }

                by -= 1;
            }
        }

        self.y = y;

        let friction = if self.on_ground { DRAG * GROUND_FRICTION } else { DRAG };

        self.vx *= friction;
        self.vz *= friction;
        self.vy *= DRAG;

        if self.on_ground {
            self.vy = if self.vy < -BOUNCE_MIN { -self.vy * BOUNCE } else { 0.0 };
        }

        self.age += 1;
        self.ticks += 1;
    }

    /// Дотягивается ли до шарика игрок с ногами в `feet` (по месту шарика
    /// после его хода в этом такте: чёрный ящик, xp-orb).
    pub fn reached_by(&self, feet: (f64, f64, f64)) -> bool {
        let (x, y, z) = (self.x, self.y, self.z);

        (x - feet.0).abs() < REACH && (z - feet.2).abs() < REACH && y < feet.1 + REACH_ABOVE && y > feet.1 - REACH_BELOW
    }

    pub fn expired(&self) -> bool {
        self.age >= LIFETIME
    }
}

/// Выбирает ближайшего игрока в пределах тяги; возвращает центр его тела.
pub fn nearest_pull(at: (f64, f64, f64), players: &[(f64, f64, f64)]) -> Option<(f64, f64, f64)> {
    players
        .iter()
        .map(|&(x, y, z)| (x, y + 0.9, z))
        .map(|body| ((body.0 - at.0).powi(2) + (body.1 - at.1).powi(2) + (body.2 - at.2).powi(2), body))
        .filter(|(d2, _)| *d2 < PULL_RANGE * PULL_RANGE)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, body)| body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(_x: i32, y: i32, _z: i32) -> bool {
        y < 0
    }

    #[test]
    fn a_dropped_orb_falls_bounces_once_and_lies() {
        let mut orb = XpOrb::new(1, 1, (0.5, 3.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..20 {
            orb.step(None, &floor);
        }

        // Замер xp-orb-far: на 20-м такте 0,315 над полом, скорость ~0,001.
        assert!((orb.y - 0.3154).abs() < 0.002, "{}", orb.y);
        assert!(orb.vy.abs() < 0.002);

        for _ in 0..40 {
            orb.step(None, &floor);
        }

        assert_eq!((orb.y, orb.vy), (0.0, 0.0));
        assert!(orb.on_ground);
    }

    #[test]
    fn a_near_player_pulls_and_a_far_one_does_not() {
        let players = [(0.5, 0.0, 6.5)];

        assert!(nearest_pull((0.5, 0.0, 0.5), &players).is_some());
        assert!(nearest_pull((0.5, 0.0, -3.0), &players).is_none());
        assert_eq!(nearest_pull((0.5, 0.0, 0.5), &[(0.5, 0.0, 7.5), (0.5, 0.0, 3.5)]), Some((0.5, 0.9, 3.5)));
    }

    #[test]
    fn a_pulled_orb_reaches_the_player_in_a_few_ticks() {
        let mut orb = XpOrb::new(1, 1, (3.5, 0.0, 3.5), (0.1976, 0.1068, -0.0895));
        let player = (3.5, 0.0, 0.5);
        let mut ticks = 0;

        loop {
            orb.step(nearest_pull((orb.x, orb.y, orb.z), &[player]), &floor);
            ticks += 1;

            if orb.reached_by(player) || ticks > 30 {
                break;
            }
        }

        assert!(ticks <= 8, "{ticks}");
    }

    #[test]
    fn reach_is_a_box_around_the_player() {
        let mut orb = XpOrb::new(1, 1, (1.4, 0.0, 0.0), (0.0, 0.0, 0.0));

        assert!(orb.reached_by((0.0, 0.0, 0.0)));

        orb.x = 1.7;
        assert!(!orb.reached_by((0.0, 0.0, 0.0)));

        orb.x = 0.0;
        orb.y = 2.5;
        assert!(!orb.reached_by((0.0, 0.0, 0.0)));
    }

    #[test]
    fn an_old_orb_expires_by_ticks() {
        let mut orb = XpOrb::new(1, 1, (0.5, 0.0, 0.5), (0.0, 0.0, 0.0));

        orb.age = LIFETIME - 1;
        assert!(!orb.expired());
        orb.step(None, &floor);
        assert!(orb.expired());
    }
}
