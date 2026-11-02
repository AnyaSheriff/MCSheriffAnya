use super::*;
use crate::survival::{Effect, EffectKind, Vitals, INFINITE};

fn sample() -> PlayerData { parse(include_bytes!("../../testdata/playerdata-layout12.bin")).unwrap() }
fn snapshot() -> SavedEffects {
    let mut vit = Vitals::new();
    for (amplifier, ticks) in [(0, INFINITE), (1, 600), (2, 400), (3, 2)] {
        vit.give_effect_shown(Effect { kind: EffectKind::Speed, amplifier, ticks_left: ticks,
            ambient: amplifier == 2, visible: amplifier != 1, show_icon: amplifier != 0 });
    }
    vit.give_effect(EffectKind::Poison, 0, 25);
    vit.give_effect(EffectKind::Absorption, 1, 700);
    vit.absorption = 2.25;
    vit.saved_effects()
}

#[test]
fn frozen_layout_twelve_preserves_abilities_cursor_and_all_older_fields() {
    let bytes = include_bytes!("../../testdata/playerdata-layout12.bin");
    // Независимый fixture старого309d8903: frozen v9 + v12 cursor/abilities.
    assert_eq!(bytes.len(), 179);
    let data = sample();
    assert_eq!(data.effects, SavedEffects::default());
    assert_eq!(data.abilities, Abilities { flying: true, mayfly: true, instabuild: true,
        invulnerable: true, may_build: false, fly_speed: 0.123, walk_speed: 0.234 });
    assert_eq!(data.inventory.cursor(), Some(crate::inventory::Stack::new(1, 7)));
    assert_eq!(data.inventory.slot(36), Some(crate::inventory::Stack::new(1, 17)));
    assert_eq!((data.vitals.experience, data.vitals.experience_total, data.vitals.xp_seed), (7, 42, 123456));
    assert_eq!(data.ender[4], Some(crate::inventory::Stack::new(1, 7)));
    let upgraded = encode(&data).unwrap();
    assert_eq!(&upgraded[8..bytes.len()], &bytes[8..]);
    assert_eq!(parse(&upgraded), Some(data));
}

#[test]
fn effects_file_roundtrip_resumes_hidden_layers_poison_and_partial_absorption() {
    let mut data = sample(); data.effects = snapshot();
    let directory = std::env::temp_dir().join(format!("mcsa-effects-roundtrip-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap(); let uuid = [231; 16];
    save(&directory, &uuid, &data).unwrap();
    let loaded = load_checked(&directory, &uuid).unwrap().unwrap();
    assert_eq!(loaded, data);
    let mut expected = Vitals::restored_with_effects(&data.vitals, &data.effects);
    let mut restored = Vitals::restored_with_effects(&loaded.vitals, &loaded.effects);
    assert_eq!(restored.absorption, 2.25);
    assert_eq!(restored.effect_gives, 0);
    for tick in 0..405 {
        let a = expected.effects_tick(); let b = restored.effects_tick();
        assert_eq!(a.hurt, b.hurt); assert_eq!(a.gone, b.gone);
        if tick == 0 { assert_eq!(b.hurt.len(), 1); }
        assert_eq!(expected.saved_effects(), restored.saved_effects());
    }
    assert_eq!(restored.effect_level(EffectKind::Speed), 2);
    restored.remove_effects(None);
    assert!(restored.saved_effects().hidden.is_empty());
    assert!(restored.effects.is_empty());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn effects_restore_dead_player_resets_effects_and_keeps_health_boost_health_until_attributes() {
    let mut data = sample(); data.effects = snapshot(); data.vitals.health = 28.0;
    let vit = Vitals::restored_with_effects(&data.vitals, &data.effects);
    assert_eq!(vit.health, 28.0);
    data.vitals.health = 0.0;
    let vit = Vitals::restored_with_effects(&data.vitals, &data.effects);
    assert_eq!(vit.saved_effects(), SavedEffects::default());
    assert_eq!(vit.health, crate::survival::MAX_HEALTH);
}

#[test]
fn corrupt_effects_trailers_are_refused_without_touching_existing_bytes() {
    let mut data = sample(); data.effects = snapshot();
    let bytes = encode(&data).unwrap();
    let length = u32::from_be_bytes(bytes[bytes.len()-4..].try_into().unwrap()) as usize;
    let start = bytes.len() - 4 - length;
    let directory = std::env::temp_dir().join(format!("mcsa-effects-corrupt-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap(); let uuid = [232; 16]; let path = path_for(&directory, &uuid);
    let mut cases = Vec::new();
    for missing in 1..=length+4 { cases.push(bytes[..bytes.len()-missing].to_vec()); }
    for (offset, value) in [(start, f32::NAN.to_be_bytes()), (start, (-1.0f32).to_be_bytes()),
        (start + 4, 41u32.to_be_bytes()), (start + 8, 10201u32.to_be_bytes()), (bytes.len()-4, u32::MAX.to_be_bytes()),
        (start + 13, 256i32.to_be_bytes()), (start + 17, (-2i32).to_be_bytes())] {
        let mut bad = bytes.clone(); bad[offset..offset+4].copy_from_slice(&value); cases.push(bad);
    }
    for (offset, value) in [(start + 12, 255), (start + 21, 8)] {
        let mut bad = bytes.clone(); bad[offset] = value; cases.push(bad);
    }
    // Дубликат активного вида, скрытый без своего активного и слишком сильный слой.
    for (offset, value) in [(start + 22, EffectKind::Speed.id() as u8),
        (start + 42, EffectKind::Poison.id() as u8)] {
        let mut bad = bytes.clone(); bad[offset] = value; cases.push(bad);
    }
    let mut bad = bytes.clone(); bad[start + 43..start + 47].copy_from_slice(&4i32.to_be_bytes()); cases.push(bad);
    let mut bad = bytes.clone();
    // Второй скрытый слой получает тот же уровень, что и первый.
    bad[start + 53..start + 57].copy_from_slice(&0i32.to_be_bytes()); cases.push(bad);
    for bad in cases {
        fs::write(&path, &bad).unwrap();
        assert_eq!(load_checked(&directory, &uuid), Err(LoadError::InvalidData));
        assert_eq!(fs::read(&path).unwrap(), bad);
    }
    save(&directory, &uuid, &data).unwrap(); let previous = fs::read(&path).unwrap();
    data.effects.active.push(data.effects.active[0]);
    assert!(save(&directory, &uuid, &data).is_err());
    assert_eq!(fs::read(&path).unwrap(), previous);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn effects_save_cadence_tracks_layers_flags_and_absorption_without_tick_writes() {
    let old = snapshot(); let mut current = old.clone();
    current.active[2].ticks_left -= 99;
    assert!(!old.needs_save(&current));
    current.active[2].ticks_left -= 1;
    assert!(old.needs_save(&current));
    current = old.clone(); current.hidden[0].visible = !current.hidden[0].visible;
    assert!(old.needs_save(&current));
    current = old.clone(); current.absorption -= 1.0;
    assert!(old.needs_save(&current));
    current = old.clone(); current.hidden[0].ticks_left = 999;
    assert!(old.needs_save(&current));
}

#[test]
fn all_256_effect_levels_fit_the_hidden_chain_bound_without_normalization() {
    let mut vit = Vitals::new();
    for amplifier in 0..=255 { vit.give_effect(EffectKind::Speed, amplifier, 1000 - amplifier); }
    let mut data = sample(); data.effects = vit.saved_effects();
    assert_eq!(data.effects.hidden.len(), 255);
    assert_eq!(parse(&encode(&data).unwrap()), Some(data));
}
