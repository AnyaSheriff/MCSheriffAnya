use super::*;
use survival::{EffectKind::*, Vitals};
use crate::attributes::*;
fn player() -> PlayerState { PlayerState::new([4;16],"EffectsAudit".into(),None,None,0x7f,4,(0.0,64.0,0.0)) }
fn near(got: f64,want:f64) { assert!((got-want).abs()<0.00001,"{got} != {want}"); }

macro_rules! attribute_tests {
    ($($name:ident: $kind:ident, $attr:ident, [$($want:expr),+]);+ $(;)?) => { $(
        #[test] fn $name() {
            let mut p=player(); let base=p.attrs.value($attr);
            for (amp,want) in [$($want),+].into_iter().enumerate() {
                p.vit.remove_effects(None);p.vit.give_effect($kind,amp as i32,100);
                apply_effect_modifiers(&mut p);near(p.attrs.value($attr),want);
                p.vit.remove_effects(None);apply_effect_modifiers(&mut p);near(p.attrs.value($attr),base);
            }
        }
    )+ };
}
attribute_tests!(
    strength: Strength, ATTACK_DAMAGE, [4.0,7.0,10.0];
    weakness: Weakness, ATTACK_DAMAGE, [0.0,0.0,0.0];
    haste: Haste, ATTACK_SPEED, [4.4,4.8,5.2];
    mining_fatigue: MiningFatigue, ATTACK_SPEED, [3.6,3.2,2.8];
    speed: Speed, MOVEMENT_SPEED, [0.12,0.14,0.16];
    slowness: Slowness, MOVEMENT_SPEED, [0.085,0.07,0.055];
    jump_boost: JumpBoost, JUMP_STRENGTH, [0.52,0.62,0.72];
    health_boost: HealthBoost, MAX_HEALTH, [24.0,28.0,32.0];
    absorption: Absorption, MAX_ABSORPTION, [4.0,8.0,12.0];
);
#[test]
fn strength_and_weakness_stack_before_cooldown_and_critical() {
    let mut p=player();p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("diamond_sword").unwrap(),1)));
    update_equipment(&mut p,0);p.vit.give_effect(Strength,1,100);p.vit.give_effect(Weakness,0,100);apply_effect_modifiers(&mut p);
    near(p.attrs.value(ATTACK_DAMAGE),9.0);
    near(f64::from(crate::combat::strike(p.attrs.value(ATTACK_DAMAGE),0.0,0.0,1.0,true).damage),13.5);
}
#[test]
fn resistance() {
    for (amp,damage) in [(0,8.0),(1,6.0),(2,4.0),(3,2.0),(4,0.0),(5,0.0)] {
        let mut v=Vitals::new();v.give_effect(Resistance,amp,100);
        near(f64::from(v.hurt(10.0,Source::Magic,false).unwrap().taken),damage);
        v.invulnerable=0;near(f64::from(v.hurt(10.0,Source::OutOfWorld,false).unwrap().taken),10.0);
    }
}
fn periodic(kind: survival::EffectKind, periods: &[i32]) {
    for (amp,&period) in periods.iter().enumerate() {
        let mut v=Vitals::new();v.max_health=1000.0;v.health=100.0;v.give_effect(kind,amp as i32,100);
        let mut events=0;
        for remaining in (1..=100).rev() {
            let before=v.health;let tick=v.effects_tick();
            let applied=if kind==Regeneration { v.health-before } else { tick.hurt.iter().map(|v|v.0).sum() };
            assert_eq!(applied,if remaining%period==0 {1.0} else {0.0});
            events+=applied as i32;
        }
        assert_eq!(events,100/period);assert!(!v.has_effect(kind));
    }
}
#[test] fn regeneration() { periodic(Regeneration,&[50,25,12,6,3,1,1]); }
#[test] fn poison() { periodic(Poison,&[25,12,6,3,1,1,1]);let mut v=Vitals::new();v.health=1.0;v.give_effect(Poison,3,100);assert!(v.effects_tick().hurt.is_empty()); }
#[test] fn wither() { periodic(Wither,&[40,20,10,5,2,1,1]); }
fn instant(kind:survival::EffectKind,base:i32) {
    for amp in [0,1,2,5,20,32,33,64,255] {
        let expected=base.wrapping_shl(amp as u32);
        assert_eq!(survival::instant_base(kind,amp),expected);
    }
    for undead in [false,true] {
        let mut v=Vitals::new();v.health=10.0;v.give_effect(kind,1,1);
        let tick=v.effects_tick_for(undead);
        if (kind==InstantHealth) != undead { assert_eq!(v.health,18.0);assert!(tick.hurt.is_empty()); }
        else { assert_eq!(tick.hurt,vec![(12.0,Source::Magic)]); }
        assert!(!v.has_effect(kind));
    }
}
#[test] fn instant_health() { instant(InstantHealth,4); }
#[test] fn instant_damage() { instant(InstantDamage,6); }

#[test]
fn health_boost_does_not_heal_and_removal_clamps_health() {
    let mut p = player();
    p.vit.health = 12.0;
    p.vit.give_effect(HealthBoost, 2, 1);
    apply_effect_modifiers(&mut p);
    assert_eq!((p.vit.health, p.vit.max_health), (12.0, 32.0));
    p.vit.heal(100.0);
    assert_eq!(p.vit.health, 32.0);
    p.vit.effects_tick();
    apply_effect_modifiers(&mut p);
    assert_eq!((p.vit.health, p.vit.max_health), (20.0, 20.0));
}

#[test]
fn absorption_upgrade_expiry_and_health_are_independent() {
    let mut p = player();
    p.vit.give_effect(Absorption, 0, 100);
    apply_effect_modifiers(&mut p);
    p.vit.hurt(3.0, Source::Magic, false).unwrap();
    assert_eq!((p.vit.health, p.vit.absorption), (20.0, 1.0));
    p.vit.give_effect(Absorption, 2, 200);
    apply_effect_modifiers(&mut p);
    assert_eq!((p.vit.health, p.vit.absorption), (20.0, 12.0));
    p.vit.remove_effects(Some(Absorption));
    apply_effect_modifiers(&mut p);
    assert_eq!((p.vit.health, p.vit.absorption), (20.0, 0.0));
}

#[tokio::test]
async fn all_potion_payloads_drink_in_both_hands_after_32_ticks() {
    // Только локальная пара сокетов для Outgoing, без сервера Minecraft.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let _client = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut out = Outgoing::new(&mut socket);
    let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/itemaudit4/no-ops-drink.json")), crate::skins::Settings::default());
    for name in survival::POTIONS.iter().skip(4) {
        for hand in [0, 1] {
            let mut p = player();
            p.game_mode = GAME_MODE_SURVIVAL;
            let before = if name.contains("harming") { 20.0 } else { 6.0 };
            p.vit.health = before;
            let (_, data) = crate::stackdata::parse_item_spec(&format!("potion[potion_contents=\"{name}\"]")).unwrap();
            let expected = data.potion_effects();
            let slot = if hand == 1 { inventory::OFFHAND } else { p.inventory.selected_slot() };
            p.inventory.set(slot, Some(Stack::new(blocks::item_named("potion").unwrap(), 1).with_extras(data)));
            assert!(start_eating(&mut p, hand));
            for _ in 0..31 { effects_and_eating_tick(&mut out, &shared, &mut p, 2).await.unwrap(); }
            assert!(p.vit.effects.is_empty(), "{name}: до глотка");
            assert_eq!(p.vit.health, before);
            effects_and_eating_tick(&mut out, &shared, &mut p, 2).await.unwrap();
            assert!(p.net.eating.is_none());
            assert_eq!(blocks::item_name(p.inventory.slot(slot).unwrap().item), Some("glass_bottle"));
            for effect in expected {
                match effect.kind {
                    InstantHealth => assert_eq!(p.vit.health, (before + (4 << effect.amplifier) as f32).min(20.0)),
                    InstantDamage => assert_eq!(p.vit.health, (before - (6 << effect.amplifier) as f32).max(0.0)),
                    _ => assert!(p.vit.effects.contains(&effect), "{name}: {effect:?}"),
                }
            }
        }
    }
}
