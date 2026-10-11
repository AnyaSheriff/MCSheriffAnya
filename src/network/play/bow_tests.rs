use super::*;
use crate::survival;
use crate::stackdata::{self, Data};

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/itemaudit2/unused-ops.json")), crate::skins::Settings::default())
}

fn player(weapon: &str, ammo: Stack, enchant: Option<&str>) -> PlayerState {
    let mut player = PlayerState::new([41; 16], "ItemAudit2".into(), None, None, 0x7F, 41, (0.5, 64.0, 0.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    player.yaw = 0.0;
    player.pitch = 0.0;
    let mut data = Data::default();
    if let Some(name) = enchant { data.set_enchant(stackdata::enchantment_number(name).unwrap(), 1); }
    player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named(weapon).unwrap(), 1).with_extras(data)));
    player.inventory.set(inventory::OFFHAND, Some(ammo));
    player
}

fn crossbow_potion(name: &str) {
    let shared = shared();
    let potion = survival::potion_number(name).unwrap();
    let ammo = Stack::new(blocks::item_named("tipped_arrow").unwrap(), 5).with_extras(Data { potion: Some(potion), ..Default::default() });
    let mut player = player("crossbow", ammo, None);
    bow::crossbow_load(&mut player, 0);
    assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().count, 4);
    let data = player.inventory.held().unwrap().extras();
    assert_eq!(stackdata::from_bytes(&stackdata::to_bytes(&data)), Some(data.clone()));
    let loaded = data.charged_arrow.unwrap();
    assert_eq!(loaded.item, ammo.item);
    assert_eq!(stackdata::get(loaded.data.unwrap()).potion, Some(potion));
    assert!(bow::crossbow_fire(&shared, &mut player, 0));
    let arrows = shared.arrows.lock().unwrap();
    let arrow = &arrows.list[0];
    let expected: Vec<_> = survival::potion_effects(potion).into_iter().map(|mut e| { e.ticks_left = (e.ticks_left / 8).max(1); e }).collect();
    assert_eq!(arrow.effects, expected, "{name}");
    assert_eq!(arrow.pickup_stack(), ammo.with_count(1));
    let after = player.inventory.held().unwrap().extras();
    assert_eq!(after.charged, 0);
    assert!(after.charged_arrow.is_none());
    assert_eq!(after.damage, 1);
}

macro_rules! potion_tests {
    ($($name:ident),+ $(,)?) => { $(#[test] fn $name() { crossbow_potion(stringify!($name)); })+ };
}
potion_tests!(night_vision, long_night_vision, invisibility, long_invisibility, leaping, long_leaping, strong_leaping,
    fire_resistance, long_fire_resistance, swiftness, long_swiftness, strong_swiftness, slowness, long_slowness,
    strong_slowness, turtle_master, long_turtle_master, strong_turtle_master, water_breathing, long_water_breathing,
    healing, strong_healing, harming, strong_harming, poison, long_poison, strong_poison, regeneration,
    long_regeneration, strong_regeneration, strength, long_strength, strong_strength, weakness, long_weakness,
    luck, slow_falling, long_slow_falling, wind_charged, weaving, oozing, infested);

#[test]
fn multishot_preserves_tipped_ammunition_in_all_three_projectiles() {
    let shared = shared();
    let ammo = Stack::new(blocks::item_named("tipped_arrow").unwrap(), 5).with_extras(Data { potion: survival::potion_number("weakness"), ..Default::default() });
    let mut player = player("crossbow", ammo, Some("multishot"));
    bow::crossbow_load(&mut player, 0);
    assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().count, 4);
    assert!(bow::crossbow_fire(&shared, &mut player, 0));
    let arrows = shared.arrows.lock().unwrap();
    assert_eq!(arrows.list.len(), 3);
    for (i, arrow) in arrows.list.iter().enumerate() {
        assert_eq!(arrow.pickup_stack(), ammo.with_count(1));
        assert_eq!(arrow.effects[0].kind, survival::EffectKind::Weakness);
        assert_eq!(arrow.pickup, if i == 0 { crate::arrow::Pickup::Yes } else { crate::arrow::Pickup::CreativeOnly });
    }
}

#[test]
fn infinity_only_saves_normal_arrows_and_spectral_has_its_own_entity() {
    for (name, remaining) in [("arrow", 5), ("tipped_arrow", 4), ("spectral_arrow", 4)] {
        let shared = shared();
        let ammo = Stack::new(blocks::item_named(name).unwrap(), 5);
        let mut player = player("bow", ammo, Some("infinity"));
        bow::release(&shared, &mut player, 0, 20);
        assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().count, remaining, "{name}");
        let arrows = shared.arrows.lock().unwrap();
        let arrow = &arrows.list[0];
        assert_eq!(arrow.pickup_stack(), ammo.with_count(1));
        if name == "spectral_arrow" {
            assert_eq!(arrow.entity_type(), 123);
            assert_eq!((arrow.effects[0].kind, arrow.effects[0].ticks_left), (survival::EffectKind::Glowing, 200));
        }
    }
}

#[test]
fn offhand_crossbow_uses_main_hand_special_arrow_before_inventory() {
    let mut player = player("crossbow", Stack::new(blocks::item_named("spectral_arrow").unwrap(), 3), None);
    let crossbow = player.inventory.held();
    let arrow = player.inventory.slot(inventory::OFFHAND);
    player.inventory.set(inventory::FIRST_HOTBAR, arrow);
    player.inventory.set(inventory::OFFHAND, crossbow);
    player.inventory.set(inventory::FIRST_MAIN, Some(Stack::new(blocks::item_named("arrow").unwrap(), 8)));
    bow::crossbow_load(&mut player, 1);
    assert_eq!(player.inventory.held().unwrap().count, 2);
    assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().extras().charged_arrow.unwrap().item, arrow.unwrap().item);
}

#[test]
fn snowballs_throw_from_either_hand_and_creative_preserves_stack() {
    for mode in [GAME_MODE_SURVIVAL, GAME_MODE_CREATIVE, GAME_MODE_SPECTATOR] {
        for hand in [0, 1] {
            let shared = shared();
            let ammo = Stack::new(blocks::item_named("snowball").unwrap(), 4);
            let mut player = player("bow", ammo, None);
            player.game_mode = mode;
            let slot = if hand == 0 { inventory::FIRST_HOTBAR } else { inventory::OFFHAND };
            player.inventory.set(slot, Some(ammo));
            assert_eq!(thrown::throw_potion(&shared, &mut player, hand), mode != GAME_MODE_SPECTATOR);
            assert_eq!(player.inventory.slot(slot).unwrap().count, if mode == GAME_MODE_SURVIVAL { 3 } else { 4 });
            let thrown = shared.thrown.lock().unwrap();
            if mode != GAME_MODE_SPECTATOR {
                assert_eq!(thrown.flying[0].kind, crate::thrown::Kind::Snowball);
                assert!(thrown.flying[0].vy.abs() < 0.1);
                assert!((thrown.flying[0].vz - 1.5).abs() < 0.2);
            }
        }
    }
}

#[test]
fn piercing_special_arrows_keep_effects_and_ignore_shield_flag_path() {
    let shared = shared();
    let ammo = Stack::new(blocks::item_named("tipped_arrow").unwrap(), 2).with_extras(Data { potion: survival::potion_number("poison"), ..Default::default() });
    let mut player = player("crossbow", ammo, Some("piercing"));
    bow::crossbow_load(&mut player, 0);
    bow::crossbow_fire(&shared, &mut player, 0);
    let arrows = shared.arrows.lock().unwrap();
    assert_eq!(arrows.list[0].pierce_left, 1);
    assert_eq!(arrows.list[0].effects[0].kind, survival::EffectKind::Poison);
}

#[test]
fn eggs_all_colors_throw_from_both_hands_and_keep_creative_stack() {
    for name in ["egg","blue_egg","brown_egg"] {
        for hand in [0,1] {
            for mode in [GAME_MODE_SURVIVAL,GAME_MODE_CREATIVE,GAME_MODE_SPECTATOR] {
                let shared=shared();let ammo=Stack::new(blocks::item_named(name).unwrap(),4);
                let mut p=player("bow",ammo,None);p.game_mode=mode;
                let slot=if hand==0 {p.inventory.selected_slot()} else {inventory::OFFHAND};
                p.inventory.set(slot,Some(ammo));
                assert_eq!(thrown::throw_potion(&shared,&mut p,hand),mode!=GAME_MODE_SPECTATOR);
                assert_eq!(p.inventory.slot(slot).unwrap().count,if mode==GAME_MODE_SURVIVAL {3} else {4});
                if mode!=GAME_MODE_SPECTATOR {
                    let shots=shared.thrown.lock().unwrap();
                    assert_eq!(shots.flying[0].entity_type(),39);
                    assert_eq!(shots.flying[0].stack,Some(ammo.with_count(1)));
                }
            }
        }
    }
    let mut chicks=0;
    for a in 0..8 {for b in 0..32 {chicks+=crate::thrown::egg_hatch(a,b);}}
    assert_eq!(chicks,35);
}

#[test]
fn pearl_cooldown_shared_by_hands_and_survival_consumption() {
    let shared=shared();let ammo=Stack::new(blocks::item_named("ender_pearl").unwrap(),4);
    let mut p=player("bow",ammo,None);p.inventory.set(p.inventory.selected_slot(),Some(ammo));
    assert!(thrown::throw_potion(&shared,&mut p,0));
    assert!(!thrown::throw_potion(&shared,&mut p,1));
    assert_eq!(p.inventory.held().unwrap().count,3);
    assert_eq!(p.inventory.slot(inventory::OFFHAND).unwrap().count,4);
    assert_eq!(p.item_cooldowns.get("ender_pearl"),Some(&20));
    let shots=shared.thrown.lock().unwrap();assert_eq!(shots.flying.len(),1);assert_eq!(shots.flying[0].entity_type(),44);
}

#[test]
fn experience_bottle_throws_lifted_and_breaks_into_three_to_eleven_xp() {
    let shared=shared();let ammo=Stack::new(blocks::item_named("experience_bottle").unwrap(),4);
    let mut p=player("bow",ammo,None);p.y=1.0;p.pitch=80.0;
    assert!(thrown::throw_potion(&shared,&mut p,1));
    assert_eq!(p.inventory.slot(inventory::OFFHAND).unwrap().count,3);
    { let shots=shared.thrown.lock().unwrap();assert_eq!(shots.flying[0].entity_type(),48); }
    let mut w=shared.world_in(p.dimension).lock().unwrap();
    for x in -2..=2 {for z in -2..=4 { w.set_block_silently(x,0,z,blocks::state_by_name("stone").unwrap()); }}
    for _ in 0..20 {crate::thrown::step(&shared,p.dimension,&mut w);}
    assert!(shared.thrown.lock().unwrap().flying.is_empty());
    let items=shared.items_in(p.dimension).lock().unwrap();
    let xp:i32=items.orbs().iter().map(|o|o.value).sum();assert!((3..=11).contains(&xp),"{xp}");
    let values:Vec<_>=(0..5).flat_map(|a|(0..5).map(move|b|crate::thrown::bottle_experience(a,b))).collect();
    assert_eq!(values.iter().sum::<i32>(),175);assert_eq!(values.iter().min(),Some(&3));assert_eq!(values.iter().max(),Some(&11));
}

#[test]
fn golden_dandelion_use_spends_once_and_cooldown_preserves_second_item() {
    let shared=shared();let flower=Stack::new(blocks::item_named("golden_dandelion").unwrap(),3);
    let mut p=player("bow",flower,None);p.inventory.set(p.inventory.selected_slot(),Some(flower));
    let mut calf=crate::mob::Mob::new(300,crate::mob::Kind::Cow,(p.x,p.y,p.z+2.0));calf.age = -20;
    shared.items_in(p.dimension).lock().unwrap().add_mob(calf);
    assert!(use_mob(&shared,&mut p,300,&mut Reaction::default()));
    assert_eq!(p.inventory.held().unwrap().count,2);
    assert!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].age_locked);
    assert!(use_mob(&shared,&mut p,300,&mut Reaction::default()));
    assert_eq!(p.inventory.held().unwrap().count,2);
}

#[test]
fn custom_effects_survive_multishot_disk_wire_and_reach_target() {
    let shared = shared();
    let (_, data) = stackdata::parse_item_spec(r#"tipped_arrow[potion_contents={potion:"poison",custom_color:1193046,custom_name:"audit",custom_effects:[{id:"slowness",amplifier:2,duration:803,show_particles:false,show_icon:true},{id:"weakness",duration:-1}]}]"#).unwrap();
    let ammo = Stack::new(blocks::item_named("tipped_arrow").unwrap(), 3).with_extras(data.clone());
    let mut player = player("crossbow", ammo, Some("multishot"));
    bow::crossbow_load(&mut player, 0);
    let charged = player.inventory.held().unwrap().extras();
    assert_eq!(stackdata::from_bytes(&stackdata::to_bytes(&charged)), Some(charged.clone()));
    let (_, bytes) = stackdata::encode_components(&charged);
    let mut decoded = Data::default();
    // DAMAGE/ENCHANTMENTS предшествуют charged_projectiles; отдельно кодируем payload.
    let payload = Data { charged: charged.charged, charged_arrow: charged.charged_arrow, ..Default::default() };
    let (_, encoded) = stackdata::encode_components(&payload);
    stackdata::read_component(stackdata::COMPONENT_CHARGED_PROJECTILES, &encoded[1..], &mut decoded);
    assert_eq!(decoded.charged_arrow, charged.charged_arrow, "{bytes:?}");
    assert!(bow::crossbow_fire(&shared, &mut player, 0));
    let arrows = shared.arrows.lock().unwrap();
    assert_eq!(arrows.list.len(), 3);
    for arrow in &arrows.list {
        assert_eq!(arrow.pickup_stack().extras(), data);
        assert_eq!(arrow.effects.iter().map(|e| e.ticks_left).collect::<Vec<_>>(), [112, 100, -1]);
        assert_eq!(arrow.effects[1].flags() & 6, 4);
        let mut cow = crate::mob::Mob::new(92, crate::mob::Kind::Cow, (0.0, 64.0, 0.0));
        crate::arrow::apply_effects(&mut cow, &arrow.effects);
        assert_eq!(cow.vitals.effect_level(survival::EffectKind::Slowness), 3);
        assert_eq!(cow.vitals.effects.iter().find(|e| e.kind == survival::EffectKind::Weakness).unwrap().ticks_left, -1);
    }
}
