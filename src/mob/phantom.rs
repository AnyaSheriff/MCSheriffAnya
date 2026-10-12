// Фантом (вики, Phantom).
//
// Кода игры здесь нет; правила — со страницы minecraft.wiki (Phantom, Difficulty,
// «Regional difficulty», Cat) и из чёрного ящика (сценарии phantom-spawn,
// phantom-swoop). Что не сверено, помечено «[проверить]».
//
// Появление (вики, Phantom, «Spawning»):
// - попытка раз в 1–2 минуты (1200–2400 тактов) на весь мир;
// - ночью или в грозу; игрок не наблюдатель, выше уровня моря (Y ≥ 63) и
//   небо над ним открыто; местная сложность больше случайного числа 0…3;
// - статистика time_since_rest ≥ 72000 тактов: шанс попытки (x − 72000)/x;
// - группа в 20–34 блоках над игроком, ±10 по горизонтали; 1…(сложность + 1)
//   штук (лёгкая 1–2, обычная 1–3, сложная 1–4); размер у естественных — 0.
//
// Полёт (чёрный ящик, phantom-swoop, обычная сложность): кружит над игроком,
// первое пике — через 27 тактов после появления (звук swoop, громкость 10),
// пикирует 94–146 тактов до скорости 0,57 блока за такт, кусает (урон 2,
// событие мира 1039, толчок игроку), уходит вверх до 0,49 и ровно через 72
// такта пикирует снова. Траектория у оригинала плавная, но случайная
// (высота кружения 15–30, радиус), поэтому здесь — приближение по скоростям.
use super::ai::{Ctx, Plan};
use super::{Kind, Mob, Rng, View};

/// Сколько тактов без сна открывает появление фантомов (вики, Phantom: «72,000 ticks»).
pub const REST_NEEDED: i32 = 72_000;

/// Пауза между попытками: 1200…2400 тактов (вики, Phantom: «every 1–2 minutes»).
pub const WAIT_MIN: i32 = 1200;
pub const WAIT_SPREAD: u32 = 1201;

/// Уровень моря: ниже игрока фантомы не ищут (вики, Phantom: «above sea level (Y > 62)»).
pub const SEA_LEVEL: i32 = 63;

/// Скорость кружения, пике и отхода, блоков за такт (чёрный ящик, phantom-swoop:
/// кружит 0,1–0,15, в пике до 0,57, уходит вверх до 0,49).
pub const CIRCLE_SPEED: f64 = 0.15;
pub const SWOOP_SPEED: f64 = 0.6;
pub const RETREAT_SPEED: f64 = 0.5;

/// Доля разницы скоростей, что набирается за такт (чёрный ящик, phantom-swoop:
/// от нуля до 0,57 за 35 тактов).
pub const STEER: f64 = 0.04;

/// Дальность, с которой фантом видит игрока (вики, Phantom: «64 blocks»).
pub const SIGHT: f64 = 64.0;

/// Расстояние до кошек, ближе которого фантом не подлетает (вики, Phantom/Cat: «16 blocks»).
pub const CAT_FEAR: f64 = 16.0;

/// Пауза до первого пике и между пике после укуса (чёрный ящик, phantom-swoop).
pub const FIRST_WAIT: i32 = 27;
/// Пике начинаются раз в 8–12 с (вики, Phantom, Java; чёрный ящик, phantom-swoop: 196 и 164 такта между звуками).
pub const SWEEP_MIN: i32 = 160;
pub const SWEEP_SPREAD: u32 = 81;
/// Не меньше столько тактов отдыха после укуса или испуга.
pub const BITE_WAIT: i32 = 40;

/// Фаза полёта.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Кружит над игроком (вики, Phantom: «fly around in a roughly circular pattern»).
    #[default]
    Circle,

    /// Пикирует на игрока.
    Swoop,
}

/// Состояние фантома.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Размер 0…64 (вики, Phantom: «Ranges from 0 to 64»).
    pub size: i32,

    pub phase: Phase,

    /// Кого преследует (номер сущности игрока).
    pub target: Option<i32>,

    /// Тактов до следующего пике.
    pub wait: i32,

    /// Кружение: угол, радиус, высота над игроком.
    pub angle: f64,
    pub radius: f64,
    pub height: f64,

    /// Куда летит и с какой наибольшей скоростью (ставит мысль, шаг — в `fly`).
    pub goal: Option<(f64, f64, f64)>,
    pub vmax: f64,

    /// Сколько тактов длится пике: долгое обрывается.
    pub swooping: i32,
}

/// Игрок как возможная цель появления: где стоит и сколько не спал.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rest: i32,
}

/// Местная сложность (вики, Difficulty, «Regional difficulty»): сложность 1–3,
/// всего тактов мира, время жизни чанка с игроками и яркость луны (0…1).
pub fn regional(difficulty: u8, total: i64, inhabited: i64, moon: f64) -> f64 {
    if difficulty == 0 {
        return 0.0;
    }

    let days = total as f64 / 24_000.0;
    let daytime = if days > 63.0 {
        0.25
    } else if days < 3.0 {
        0.0
    } else {
        (total as f64 - 72_000.0) / 5_760_000.0
    };
    let mut chunk = (inhabited as f64 / 3_600_000.0).min(1.0);

    if difficulty != 3 {
        chunk *= 0.75;
    }

    chunk += (moon * 0.25).min(daytime);

    if difficulty == 1 {
        chunk *= 0.5;
    }

    let value = 0.75 + daytime + chunk;

    match difficulty {
        2 => value * 2.0,
        3 => value * 3.0,
        _ => value,
    }
}

/// Яркость луны по суткам: полнолуние 1, новолуние 0 (вики, Moon).
pub fn moon_brightness(day: i64) -> f64 {
    [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75][day.rem_euclid(8) as usize]
}

/// Попытка появления для одного игрока: годится ли он и где группа. Случайные
/// числа берутся в порядке: местная сложность, давность сна, место.
pub fn try_spot(view: &dyn View, spot: &Spot, regional: f64, rng: &mut Rng) -> Option<(i32, i32, i32)> {
    let (px, py, pz) = (spot.x.floor() as i32, spot.y.floor() as i32, spot.z.floor() as i32);

    if py < SEA_LEVEL || !view.open_sky(px, py, pz) || regional <= rng.unit() * 3.0 {
        return None;
    }

    // Шанс (x − 72000)/x: случайное число до x должно дойти до 72000.
    let rest = spot.rest.max(1) as u32;

    if rng.below(rest) < REST_NEEDED as u32 {
        return None;
    }

    let at = (px - 10 + rng.below(21) as i32, py + 20 + rng.below(15) as i32, pz - 10 + rng.below(21) as i32);

    // Клетка должна быть пустой: воздух.
    if !view.loaded(at.0 >> 4, at.2 >> 4) || view.block(at.0, at.1, at.2) != 0 {
        return None;
    }

    super::spawn::space::dry_kind(view, Kind::Phantom,
        (f64::from(at.0) + 0.5, f64::from(at.1), f64::from(at.2) + 0.5)).then_some(at)
}

/// Сколько фантомов в группе: 1…(сложность + 1) (вики, Phantom: «1-2 in Easy, 1-3 in Normal, 1-4 in Hard»).
pub fn group_size(difficulty: u8, rng: &mut Rng) -> u32 {
    1 + rng.below(u32::from(difficulty) + 1)
}

/// Новый фантом на этом месте.
pub fn spawn(id: i32, at: (i32, i32, i32)) -> Mob {
    let mut mob = Mob::new(id, Kind::Phantom, (f64::from(at.0) + 0.5, f64::from(at.1), f64::from(at.2) + 0.5));

    mob.persistent = false;
    mob
}

/// Мысль фантома за такт: цель, фаза, куда лететь.
pub fn think(mob: &mut Mob, _view: &dyn View, ctx: &Ctx, rng: &mut Rng, mut plan: Plan) -> Plan {
    mob.phantom.wait = (mob.phantom.wait - 1).max(0);

    // Вики, Phantom: «stay at least 16 blocks away». Отход нужен и без
    // игрока, и при кружении; [проверить] точная траектория испуга.
    if let Some(cat) = ctx.mates.iter().filter(|m| m.kind == Kind::Cat)
        .filter(|m| (m.x-mob.x).powi(2)+(m.y-mob.y).powi(2)+(m.z-mob.z).powi(2) <= CAT_FEAR.powi(2))
        .min_by(|a,b| ((a.x-mob.x).powi(2)+(a.y-mob.y).powi(2)+(a.z-mob.z).powi(2))
            .total_cmp(&((b.x-mob.x).powi(2)+(b.y-mob.y).powi(2)+(b.z-mob.z).powi(2))))
    {
        if mob.phantom.phase == Phase::Swoop { retreat(mob); }
        mob.phantom.wait = mob.phantom.wait.max(BITE_WAIT);
        mob.phantom.vmax = RETREAT_SPEED;
        let (dx, dy, dz) = (mob.x-cat.x, mob.y-cat.y, mob.z-cat.z);
        let length = (dx*dx+dy*dy+dz*dz).sqrt();
        let away = if length > 1e-6 { (dx/length,dy/length,dz/length) } else { (0.0,1.0,0.0) };
        mob.phantom.goal = Some((cat.x+away.0*17.0,cat.y+away.1*17.0,cat.z+away.2*17.0));
        return plan;
    }

    // Цель: ближайший живой игрок не в творческом режиме в 64 блоках.
    let target = ctx
        .players
        .iter()
        .filter(|player| player.targetable && !player.spectator && ctx.difficulty != crate::survival::PEACEFUL)
        .map(|player| (player, (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2)))
        .filter(|(_, dist)| *dist <= SIGHT * SIGHT)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(player, _)| *player);

    let Some(target) = target else {
        mob.phantom.target = None;
        mob.phantom.phase = Phase::Circle;

        // Без цели кружит на месте.
        mob.phantom.angle += 0.02;
        mob.phantom.goal = Some((mob.x + mob.phantom.angle.cos() * 8.0, mob.y, mob.z + mob.phantom.angle.sin() * 8.0));
        mob.phantom.vmax = CIRCLE_SPEED;

        return plan;
    };

    if mob.phantom.target != Some(target.id) {
        mob.phantom.target = Some(target.id);
        mob.phantom.radius = 5.0 + f64::from(rng.below(8));
        mob.phantom.height = 10.0 + f64::from(rng.below(20));
        mob.phantom.wait = FIRST_WAIT;
    }

    // Кошка в 16 блоках: к игроку не подлетает (вики, Phantom: «stay at least 16 blocks away»).
    let cat = ctx.mates.iter().any(|mate| mate.kind == Kind::Cat && (mate.x - mob.x).powi(2) + (mate.y - mob.y).powi(2) + (mate.z - mob.z).powi(2) < CAT_FEAR * CAT_FEAR);

    match mob.phantom.phase {
        Phase::Circle => {
            mob.phantom.angle += 0.02;

            let (radius, height) = (mob.phantom.radius, mob.phantom.height);

            mob.phantom.goal = Some((target.x + mob.phantom.angle.cos() * radius, target.y + height, target.z + mob.phantom.angle.sin() * radius));
            mob.phantom.vmax = if mob.y < target.y + height - 2.0 { RETREAT_SPEED } else { CIRCLE_SPEED };

            if mob.phantom.wait == 0 && !cat {
                mob.phantom.phase = Phase::Swoop;
                mob.phantom.swooping = 0;
                mob.phantom.wait = SWEEP_MIN + rng.below(SWEEP_SPREAD) as i32;

                let pitch = 0.95 + rng.unit() * 0.1;

                mob.events.push(super::Event::Sound { name: "minecraft:entity.phantom.swoop", volume: 10.0, pitch: pitch as f32 });
            }
        }
        Phase::Swoop => {
            mob.phantom.swooping += 1;
            mob.phantom.goal = Some((target.x, target.y + 1.0, target.z));
            mob.phantom.vmax = SWOOP_SPEED;

            let (dx, dz) = (target.x - mob.x, target.z - mob.z);
            let touching = dx.abs() < 0.75 && dz.abs() < 0.75 && mob.y + 0.5 > target.y && mob.y < target.y + 1.8;

            if cat || mob.wall {
                retreat(mob);
            } else if touching {
                // Укус: урон по сложности, толчок от фантома (вики, Phantom: «2 HP … 3 HP on Hard»).
                let damage = if ctx.difficulty >= 3 { 3.0 } else { 2.0 };
                let length = (dx * dx + dz * dz).sqrt().max(1.0e-6);

                plan.melee = Some(super::hostile::Melee { target: target.id, damage, push: (dx / length, dz / length), wither: false, hunger: false });
                mob.phantom.phase = Phase::Circle;
                mob.phantom.wait = mob.phantom.wait.max(BITE_WAIT);
                // После укуса уходит вверх: скорость разворачивается за 3–4 такта (чёрный
                // ящик, phantom-swoop: vy −0,17 → +0,18 за 4 такта).
                mob.vy = mob.vy.max(0.0) + 0.1;
                mob.phantom.radius = 5.0 + f64::from(rng.below(8));
                mob.phantom.height = 10.0 + f64::from(rng.below(20));
            } else if mob.phantom.swooping > 400 {
                retreat(mob);
            }
        }
    }

    plan
}

/// После удара или испуга пике прерывается сразу (вики, Phantom).
/// [проверить] точная траектория подъёма и пауза после испуга.
pub fn retreat(mob: &mut Mob) {
    mob.phantom.phase = Phase::Circle;
    mob.phantom.wait = mob.phantom.wait.max(BITE_WAIT);
    mob.phantom.swooping = 0;
    mob.phantom.goal = Some((mob.x, mob.y + 10.0, mob.z));
    mob.phantom.vmax = RETREAT_SPEED;
    mob.vy = mob.vy.max(0.0) + 0.1;
}

/// Полёт: скорость тянется к скорости в сторону цели, тяжести нет (вики,
/// Phantom: «flying»); глядит по ходу [проверить: углы у оригинала].
pub fn fly(mob: &mut Mob, view: &dyn View) {
    if let Some((gx, gy, gz)) = mob.phantom.goal {
        let (dx, dy, dz) = (gx - mob.x, gy - mob.y, gz - mob.z);
        let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-6);
        let want = mob.phantom.vmax.min(length * 0.06);

        mob.vx += (dx / length * want - mob.vx) * STEER;
        mob.vy += (dy / length * want - mob.vy) * STEER;
        mob.vz += (dz / length * want - mob.vz) * STEER;
    }

    mob.step(view, mob.vx, mob.vy, mob.vz);
    // Падения у летуна нет.
    mob.vitals.fall = 0.0;

    let flat = (mob.vx * mob.vx + mob.vz * mob.vz).sqrt();

    if flat > 1.0e-3 {
        mob.yaw = (-mob.vx.atan2(mob.vz)).to_degrees() as f32;
        mob.head_yaw = mob.yaw;
    }

    mob.pitch = (0.75 * mob.vy.atan2(flat.max(1.0e-3)).to_degrees()) as f32;
}
