// Табличные ожидания — утверждённая potion table и wiki Potion/Splash Potion/
// Lingering Potion/Tipped Arrow, не результаты вычисления тестируемой функции.
use super::*;
use crate::survival::{potion_number, EffectKind::*};

fn check(name: &str, expected: &[(EffectKind, i32, i32)]) {
    let (_, data) = crate::stackdata::parse_item_spec(&format!("potion[potion_contents=\"{name}\"]")).unwrap();
    let effects = data.potion_effects();
    assert_eq!(effects.iter().map(|e| (e.kind, e.amplifier, e.ticks_left)).collect::<Vec<_>>(), expected, "{name}");
    let arrow = crate::arrow::tipped_effects(potion_number(name));
    for (i, &(kind, amplifier, ticks)) in expected.iter().enumerate() {
        assert_eq!((arrow[i].kind, arrow[i].amplifier, arrow[i].ticks_left), (kind, amplifier, (ticks / 8).max(1)));
        if kind.instant() {
            let full = if kind == InstantHealth { 4 << amplifier } else { 6 << amplifier };
            assert_eq!(instant_amount(kind, amplifier, 1.0), full as f32);
            assert_eq!(instant_amount(kind, amplifier, 0.5), (full / 2) as f32);
        } else {
            for (scale, duration, want) in [(1.0, 1.0, ticks), (0.5, 1.0, (ticks + 1) / 2), (0.5, 0.5, ticks / 4)] {
                let mut deeds = Vec::new();
                apply(&mut deeds, &mut [], Struck::Player(42), [0; 16], &[effects[i]], scale, duration, None, None);
                assert!(matches!(deeds.as_slice(), [Deed::Effect(42, e)] if e.kind == kind && e.amplifier == amplifier && e.ticks_left == want), "{name}: {scale}/{duration}");
            }
        }
    }
}

macro_rules! potions {
    ($($name:ident => [$(($kind:ident, $amp:expr, $ticks:expr)),+]),+ $(,)?) => {
        $(#[test] fn $name() { check(stringify!($name), &[$(($kind, $amp, $ticks)),+]); })+
    };
}
potions!(
    night_vision => [(NightVision, 0, 3600)], long_night_vision => [(NightVision, 0, 9600)],
    invisibility => [(Invisibility, 0, 3600)], long_invisibility => [(Invisibility, 0, 9600)],
    leaping => [(JumpBoost, 0, 3600)], long_leaping => [(JumpBoost, 0, 9600)], strong_leaping => [(JumpBoost, 1, 1800)],
    fire_resistance => [(FireResistance, 0, 3600)], long_fire_resistance => [(FireResistance, 0, 9600)],
    swiftness => [(Speed, 0, 3600)], long_swiftness => [(Speed, 0, 9600)], strong_swiftness => [(Speed, 1, 1800)],
    slowness => [(Slowness, 0, 1800)], long_slowness => [(Slowness, 0, 4800)], strong_slowness => [(Slowness, 3, 400)],
    turtle_master => [(Slowness, 3, 400), (Resistance, 2, 400)],
    long_turtle_master => [(Slowness, 3, 800), (Resistance, 2, 800)],
    strong_turtle_master => [(Slowness, 5, 400), (Resistance, 3, 400)],
    water_breathing => [(WaterBreathing, 0, 3600)], long_water_breathing => [(WaterBreathing, 0, 9600)],
    healing => [(InstantHealth, 0, 1)], strong_healing => [(InstantHealth, 1, 1)],
    harming => [(InstantDamage, 0, 1)], strong_harming => [(InstantDamage, 1, 1)],
    poison => [(Poison, 0, 900)], long_poison => [(Poison, 0, 1800)], strong_poison => [(Poison, 1, 432)],
    regeneration => [(Regeneration, 0, 900)], long_regeneration => [(Regeneration, 0, 1800)], strong_regeneration => [(Regeneration, 1, 450)],
    strength => [(Strength, 0, 3600)], long_strength => [(Strength, 0, 9600)], strong_strength => [(Strength, 1, 1800)],
    weakness => [(Weakness, 0, 1800)], long_weakness => [(Weakness, 0, 4800)], luck => [(Luck, 0, 6000)],
    slow_falling => [(SlowFalling, 0, 1800)], long_slow_falling => [(SlowFalling, 0, 4800)],
    wind_charged => [(WindCharged, 0, 3600)], weaving => [(Weaving, 0, 3600)],
    oozing => [(Oozing, 0, 3600)], infested => [(Infested, 0, 3600)],
);

#[test]
fn short_and_infinite_cloud_effects_do_not_use_splash_cutoff() {
    assert_eq!(scaled_ticks(40, 0.5), None);
    assert_eq!(scaled_ticks(41, 0.5), Some(21));
    assert_eq!(delivery_ticks(7, 0.5, 0.5), Some(1));
    assert_eq!(delivery_ticks(803, 0.5, 0.5), Some(200));
    assert_eq!(delivery_ticks(-1, 0.5, 0.5), Some(-1));
    assert_eq!(delivery_ticks(-1, 0.5, 1.0), Some(-1));
    assert_eq!(delivery_ticks(0, 0.5, 0.5), None);
}
