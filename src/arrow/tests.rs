use super::*;

/// Пол по y = 0 (верх на y = 1), заданные клетки — камень.
struct Flat {
    stone: Vec<(i32, i32, i32)>,
}

impl View for Flat {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        if y <= 0 || self.stone.contains(&(x, y, z)) {
            blocks::state_by_name("stone").expect("камень")
        } else {
            blocks::state_by_name("air").expect("воздух")
        }
    }

    fn loaded(&self, _: i32, _: i32) -> bool {
        true
    }
}

fn open() -> Flat {
    Flat { stone: Vec::new() }
}

#[test]
fn flight_follows_gravity_and_drag_from_the_wiki() {
    let view = open();
    let mut arrow = Arrow::new(1, (0.0, 10.0, 0.0), (1.0, 0.0, 0.0), 0, "");

    // Такт: сдвиг на нынешнюю скорость, потом v *= 0,99, v.y -= 0,05.
    arrow.step(&view, &[]);
    assert_eq!((arrow.x, arrow.y), (1.0, 10.0));
    assert!((arrow.vx - 0.99).abs() < 1.0e-12);
    assert!((arrow.vy + 0.05).abs() < 1.0e-12);

    arrow.step(&view, &[]);
    assert!((arrow.y - 9.95).abs() < 1.0e-12);
    assert!((arrow.vy - (-0.05 * 0.99 - 0.05)).abs() < 1.0e-12);
}

#[test]
fn an_arrow_sticks_in_the_floor_and_lives_a_minute() {
    let view = open();
    let mut arrow = Arrow::new(1, (0.5, 3.0, 0.5), (0.0, -0.5, 0.0), 0, "");

    for _ in 0..40 {
        arrow.step(&view, &[]);

        if arrow.in_ground {
            break;
        }
    }

    assert!(arrow.in_ground);
    assert_eq!(arrow.stuck_in, (0, 0, 0));
    assert!(arrow.y >= 0.99 && arrow.y <= 1.06, "у верха пола: {}", arrow.y);
    assert_eq!((arrow.vx, arrow.vy, arrow.vz), (0.0, 0.0, 0.0));
    assert!(!arrow.pickup, "стрелы мобов не подобрать");

    for _ in 0..GROUND_LIFE - 1 {
        arrow.step(&view, &[]);
    }

    assert!(!arrow.gone);
    arrow.step(&view, &[]);
    assert!(arrow.gone, "через минуту исчезает");
}

#[test]
fn an_arrow_falls_again_when_its_block_goes() {
    let mut flat = Flat { stone: vec![(3, 1, 0)] };
    let mut arrow = Arrow::new(1, (0.5, 1.5, 0.5), (1.0, 0.0, 0.0), 0, "");

    for _ in 0..5 {
        arrow.step(&flat, &[]);
    }

    assert!(arrow.in_ground);
    flat.stone.clear();
    arrow.step(&flat, &[]);
    assert!(!arrow.in_ground);
    assert!(arrow.changes >= 2);
}

#[test]
fn an_arrow_hits_a_player_and_deals_damage_by_speed() {
    let view = open();
    let target = Target { id: 5, x: 6.0, y: 1.0, z: 0.5, width: 0.6, height: 1.8 };
    let mut arrow = Arrow::new(1, (0.0, 1.5, 0.5), (1.6, 0.0, 0.0), 9, "entity.minecraft.skeleton");
    let mut hit = None;

    for _ in 0..10 {
        if let Some(strike) = arrow.step(&view, &[target]) {
            hit = Some(strike);
            break;
        }
    }

    let strike = hit.expect("попала");

    assert_eq!(strike.target, 5);
    assert!(arrow.gone);
    // Скорость около 1,6 → урон 2 · 1,6 = 3,2 → 4 (вверх до целого).
    assert_eq!(strike.damage, 4.0);
    assert!((strike.dir.0 - 1.0).abs() < 1.0e-9);
}

#[test]
fn a_wall_stops_the_arrow_before_the_player() {
    let flat = Flat { stone: (0..4).map(|y| (3, y + 1, 0)).collect() };
    let target = Target { id: 5, x: 6.0, y: 1.0, z: 0.5, width: 0.6, height: 1.8 };
    let mut arrow = Arrow::new(1, (0.0, 1.5, 0.5), (1.6, 0.0, 0.0), 9, "");

    for _ in 0..10 {
        assert!(arrow.step(&flat, &[target]).is_none());
    }

    assert!(arrow.in_ground && !arrow.gone);
}

#[test]
fn an_arrow_that_misses_flies_past() {
    let view = open();
    let target = Target { id: 5, x: 6.0, y: 1.0, z: 5.5, width: 0.6, height: 1.8 };
    let mut arrow = Arrow::new(1, (0.0, 1.5, 0.5), (1.6, 0.0, 0.0), 9, "");

    for _ in 0..6 {
        assert!(arrow.step(&view, &[target]).is_none());
    }
}

#[test]
fn water_slows_an_arrow() {
    struct Pool;

    impl View for Pool {
        fn block(&self, _: i32, y: i32, _: i32) -> i32 {
            if y <= 0 { blocks::state_by_name("stone") } else { blocks::state_by_name("water") }.expect("блок")
        }

        fn loaded(&self, _: i32, _: i32) -> bool {
            true
        }
    }

    let mut arrow = Arrow::new(1, (0.5, 5.0, 0.5), (1.0, 0.0, 0.0), 0, "");

    arrow.step(&Pool, &[]);
    assert!((arrow.vx - 0.6).abs() < 1.0e-12);
}

#[test]
fn the_launch_aims_up_by_distance_and_keeps_the_speed() {
    let mut none = || 0.0;
    let v = launch((0.0, 1.0, 0.0), (10.0, 1.0, 0.0), 1.6, 6.0, &mut none);

    assert!(v.1 > 0.0, "наводка вверх на дальность");
    assert!((v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt() - 1.6 < 1.0e-9);

    // Разброс сдвигает вектор, но длина на выходе прежняя не гарантируется —
    // только направление в пределах error · 0,0075 на ось.
    let mut one = || 1.0;
    let off = launch((0.0, 1.0, 0.0), (10.0, 1.0, 0.0), 1.6, 6.0, &mut one);

    assert!((off.2 / 1.6 - 0.045).abs() < 0.005);
}

#[test]
fn the_rotation_follows_the_flight_direction() {
    let arrow = Arrow::new(1, (0.0, 0.0, 0.0), (0.0, 0.0, 1.0), 0, "");

    assert_eq!((arrow.yaw, arrow.pitch), (0.0, 0.0));

    let east = Arrow::new(2, (0.0, 0.0, 0.0), (1.0, 0.0, 0.0), 0, "");

    assert!((east.yaw - 90.0).abs() < 1.0e-4);

    let up = Arrow::new(3, (0.0, 0.0, 0.0), (0.0, 1.0, 1.0), 0, "");

    assert!((up.pitch - 45.0).abs() < 1.0e-4);
}
