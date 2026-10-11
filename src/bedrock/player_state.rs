// Второе представление общих Vitals. Форматы: Mojang bedrock-protocol-docs
// r/26_u1, MobEffectPacket, SetActorDataPacket, AnimatePacket и
// minecraft-data data/bedrock/1.26.10/protocol.json (протокол 944).
// Вики, Effect: «Bedrock Edition»; Java-only эффекты не подменяем чужими.
use super::codec::Out;
use crate::survival::{Effect, EffectKind, Vitals};

/// Номера Bedrock, а не Java registry id. Новые номера [проверить] клиентом.
/// У Bedrock нет Glowing, Luck, Unluck и Dolphin's Grace; сервер их хранит.
pub fn effect_id(kind: EffectKind) -> Option<i32> {
    use EffectKind::*;
    Some(match kind {
        Speed => 1, Slowness => 2, Haste => 3, MiningFatigue => 4,
        Strength => 5, InstantHealth => 6, InstantDamage => 7, JumpBoost => 8,
        Nausea => 9, Regeneration => 10, Resistance => 11, FireResistance => 12,
        WaterBreathing => 13, Invisibility => 14, Blindness => 15, NightVision => 16,
        Hunger => 17, Weakness => 18, Poison => 19, Wither => 20, HealthBoost => 21,
        Absorption => 22, Saturation => 23, Levitation => 24, ConduitPower => 26,
        SlowFalling => 27, BadOmen => 28, HeroOfTheVillage => 29, Darkness => 30,
        TrialOmen => 31, WindCharged => 32, Weaving => 33, Oozing => 34,
        Infested => 35, RaidOmen => 36, BreathOfTheNautilus => 37,
        Glowing | Luck | Unluck | DolphinsGrace => return None,
    })
}

/// ShowIcon отсутствует в MobEffectPacket 944: отдельный значок управляется
/// клиентом. Не прячем Speed/Levitation ради значка: они нужны предсказанию.
pub fn mob_effect(id: u64, event: u8, effect: Effect) -> Option<Vec<u8>> {
    let mut out = Out::packet(0x1c);
    out.varint64(id).u8(event).zigzag32(effect_id(effect.kind)?).zigzag32(effect.amplifier)
        .bool(shows_particles(effect)).zigzag32(effect.ticks_left).varint64(0).bool(effect.ambient);
    Some(out.bytes)
}

/// У BDS Bad Omen и Darkness приходят без частиц даже по `/effect … 30 1`
/// (br-record, effects: add с particles=false), остальные — с частицами.
fn shows_particles(effect: Effect) -> bool {
    effect.visible && !matches!(effect.kind, EffectKind::BadOmen | EffectKind::Darkness)
}

/// Ключ метаданных `visible_mob_effects` (long) — types.yml, ActorData 131.
const VISIBLE_MOB_EFFECTS: u32 = 131;

/// Метаданные «видимые эффекты». У BDS это упакованный список: на каждый
/// эффект с частицами по 7 бит `(номер << 1) | признак`, самый свежий — в
/// младших битах (br-record, multi: speed → 2; +slowness → 260 = 4 + (2<<7);
/// +haste → 33286 = 6 + (4<<7) + (2<<14)). Признак (младший бит) — фоновый
/// эффект [проверить]: командой его не получить.
pub fn visible_effects(id: u64, value: i64) -> Vec<u8> {
    let mut out = Out::packet(0x27);
    out.varint64(id).varint(1).varint(VISIBLE_MOB_EFFECTS).varint(7).zigzag64(value)
        .varint(0).varint(0).varint64(0);
    out.bytes
}

/// Упаковка `visible_effects`: `newest_first` — виды в порядке «свежие первыми».
fn pack_visible(newest_first: &[Effect]) -> i64 {
    newest_first.iter().filter(|e| shows_particles(**e)).filter_map(|e| effect_id(e.kind).map(|n| (n, e.ambient)))
        .take(9).enumerate().fold(0i64, |acc, (i, (n, ambient))| acc | ((i64::from(n) << 1 | i64::from(ambient)) << (7 * i)))
}

/// Номера звуков LevelSoundEvent (types.yml): исчезновение и появление при
/// невидимости, знамения при наложении (br-record: Disappeared/Reappeared,
/// ApplyEffect*Omen на игроке).
const SOUND_DISAPPEARED: u32 = 430;
const SOUND_REAPPEARED: u32 = 431;
const SOUND_OMEN_BAD: u32 = 523;
const SOUND_OMEN_RAID: u32 = 524;
const SOUND_OMEN_TRIAL: u32 = 525;

fn effect_sound(kind: EffectKind, applied: bool) -> Option<u32> {
    match (kind, applied) {
        (EffectKind::Invisibility, true) => Some(SOUND_DISAPPEARED),
        (EffectKind::Invisibility, false) => Some(SOUND_REAPPEARED),
        (EffectKind::BadOmen, true) => Some(SOUND_OMEN_BAD),
        (EffectKind::RaidOmen, true) => Some(SOUND_OMEN_RAID),
        (EffectKind::TrialOmen, true) => Some(SOUND_OMEN_TRIAL),
        _ => None,
    }
}

/// Animate 944: action uint8, actor varuint64, data float, optional source.
pub fn animation(id: u64, java_action: u8) -> Vec<u8> {
    // Java 0/3 — руки, 2 — пробуждение; Bedrock Swing=1, WakeUp=3.
    let action = match java_action { 0 | 3 => 1, 2 => 3, 4 => 4, 5 => 5, _ => 0 };
    let mut out = Out::packet(0x2c);
    out.u8(action).varint64(id).lf32(0.0).bool(false);
    out.bytes
}

pub fn cooldown(category: &str, ticks: i32) -> Vec<u8> {
    let mut out = Out::packet(0xb0);
    out.string(category).zigzag32(ticks);
    out.bytes
}

/// Воздух — short, сила замерзания — float 0..1 (ActorDataIDs 7,42,120).
pub fn environment(id: u64, air: i32, frozen: i32) -> Vec<u8> {
    let mut out = Out::packet(0x27);
    out.varint64(id).varint(3);
    out.varint(7).varint(1).li16(air.clamp(-20, 300) as i16);
    out.varint(42).varint(1).li16(300);
    out.varint(120).varint(3).lf32(frozen.clamp(0, 140) as f32 / 140.0);
    out.varint(0).varint(0).varint64(0);
    out.bytes
}

/// Атрибуты, известные клиенту Bedrock; Java-only атрибуты остаются в теле.
///
/// BDS (br-record, effects: speed/slowness) шлёт при эффекте только `movement`
/// — пределы [0; 3,4028235e38], «по умолчанию» 0,1 и модификатор эффекта
/// (операция 2, операнд 2); силу удара и сопротивление отдаче не шлёт.
pub fn combat_attributes(id: u64, attrs: &crate::attributes::Attributes, effects: &[Effect]) -> Vec<u8> {
    use crate::attributes::*;
    const SPEED_ID: &str = "91aeaa56-376b-4498-935b-2f7f68070635";
    const SLOWDOWN_ID: &str = "7107de5e-7ce8-4030-940e-514c1f160890";
    let mut out = Out::packet(super::session::UPDATE_ATTRIBUTES);
    // Уровень эффекта: Speed — +20 % за уровень, Slowness — −15 % (BDS: 0,4 и −0,3 при уровне II).
    let modifiers: Vec<(&str, &str, f32)> = effects.iter().filter_map(|e| match e.kind {
        EffectKind::Speed => Some((SPEED_ID, "MovementSpeed", 0.2 * (e.amplifier + 1) as f32)),
        EffectKind::Slowness => Some((SLOWDOWN_ID, "MovementSlowdown", -0.15 * (e.amplifier + 1) as f32)),
        _ => None,
    }).collect();
    out.varint64(id).varint(1);
    out.lf32(0.0).lf32(f32::MAX).lf32(attrs.value(MOVEMENT_SPEED) as f32)
        .lf32(0.0).lf32(f32::MAX).lf32(player_base(MOVEMENT_SPEED) as f32).string("minecraft:movement").varint(modifiers.len() as u32);
    for (modifier_id, name, amount) in modifiers { out.string(modifier_id).string(name).lf32(amount).li32(2).li32(2).bool(false); }
    out.varint64(0);
    out.bytes
}

/// MobArmorEquipment 944 содержит и пятый (body) слот; игроку он пуст.
pub fn armor(id: u64, worn: &[Option<crate::inventory::Stack>; 5]) -> Vec<u8> {
    let mut out = Out::packet(0x20); out.varint64(id);
    for stack in &worn[1..] { super::inventory::write_item(&mut out, *stack, 0); }
    super::inventory::write_item(&mut out, None, 0);
    out.bytes
}

pub fn offhand(id: u64, stack: Option<crate::inventory::Stack>) -> Vec<u8> {
    let mut out = Out::packet(0x1f); out.varint64(id);
    super::inventory::write_item(&mut out, stack, 0);
    out.u8(0).u8(0).u8(119); // WindowID Offhand
    out.bytes
}

/// Флаги посылаются целиком: ACTION не должен стирать INVISIBLE или FIRE.
#[derive(Clone, Copy, Default)]
pub struct Flags {
    pub using: bool, pub sneaking: bool, pub sprinting: bool, pub invisible: bool,
    pub burning: bool, pub blocking: bool, pub spinning: bool, pub spectator: bool,
    /// Огнестойкость: бит FIRE_IMMUNE (50) — br-record, effects: fire_resistance.
    pub fire_immune: bool,
    /// Свои флаги игрока: у BDS без CAN_SHOW_NAMETAG и ALWAYS_SHOW_NAMETAG (биты 14, 15),
    /// они нужны только тем, кто смотрит на игрока со стороны.
    pub own: bool,
}

pub fn flags(id: u64, f: Flags) -> Vec<u8> {
    let mut low: i64 = (1 << 19) | (1 << 35);
    if !f.own { low |= (1 << 14) | (1 << 15); }
    if !f.spectator { low |= (1 << 48) | (1 << 49); }
    for (enabled, bit) in [(f.burning, 0), (f.sneaking, 1), (f.sprinting, 3), (f.using, 4),
        (f.invisible || f.spectator, 5), (f.fire_immune, 50), (f.spinning, 56)] { if enabled { low |= 1 << bit; } }
    // MetadataFlags2 начинается после флага laying_down (64), BLOCKING=72.
    let high: i64 = if f.blocking { 1 << 8 } else { 0 };
    let mut out = Out::packet(0x27);
    out.varint64(id).varint(2).varint(0).varint(7).zigzag64(low)
        .varint(92).varint(7).zigzag64(high).varint(0).varint(0).varint64(0);
    out.bytes
}

#[derive(Default)]
pub struct View {
    /// Остаток и такт последней отправки: повторная выдача не сдвигает базу сравнения.
    effects: Vec<(Effect, u64)>,
    /// Эффекты в том виде, как ушли клиенту (срок — из последнего пакета).
    sent: Vec<Effect>,
    /// Порядок наложения, свежие первыми: от него зависит упаковка `visible_effects`.
    order: Vec<EffectKind>,
    visible: i64,
    environment: Option<(i32, i32)>,
    attributes: Vec<u8>,
    flags: Vec<u8>,
}

impl View {
    pub fn flags(&mut self, id: u64, f: Flags, packets: &mut Vec<Vec<u8>>) {
        let packet = flags(id, f);
        if self.flags != packet { self.flags.clone_from(&packet); packets.push(packet); }
    }

    /// `at` — ноги игрока: звуки Invisibility и знамений идут из этой точки.
    pub fn sync(&mut self, id: u64, vit: &Vitals, frozen: i32, tick: u64, at: (f32, f32, f32), packets: &mut Vec<Vec<u8>>) {
        let sound = |sound: u32, packets: &mut Vec<Vec<u8>>| packets.push(super::effects::level_sound_with(sound, at, -1, "minecraft:player"));
        // Снятый эффект уходит с теми же сроком и уровнем, что в последнем
        // пакете о нём (BDS: remove несёт прежние 600, а не остаток).
        for old in self.sent.iter().filter(|old| !vit.effects.iter().any(|e| e.kind == old.kind)) {
            packets.extend(mob_effect(id, 3, *old));
            if let Some(n) = effect_sound(old.kind, false) { sound(n, packets); }
        }
        self.sent.retain(|old| vit.effects.iter().any(|e| e.kind == old.kind));
        self.order.retain(|kind| vit.effects.iter().any(|e| e.kind == *kind));
        for effect in &vit.effects {
            let old = self.effects.iter().find(|(e, _)| e.kind == effect.kind);
            // Срок убывает на такт; фаза счётчика у клиента и у тела может расходиться
            // на единицу, поэтому «то же» — тот же уровень и вид, срок не вырос и не упал
            // больше, чем на прошедшее (+2) с последнего пакета [br-record: лишний update сразу после add].
            let same = old.is_some_and(|(old, sent_at)| {
                let elapsed = tick.saturating_sub(*sent_at).min(i32::MAX as u64) as i32;
                let decay = if old.ticks_left == -1 || effect.ticks_left == -1 { old.ticks_left == effect.ticks_left }
                    else { effect.ticks_left <= old.ticks_left && old.ticks_left.saturating_sub(elapsed).abs_diff(effect.ticks_left) <= 2 };
                decay && Effect { ticks_left: old.ticks_left, ..*effect } == *old
            });
            if same { continue; }
            // Такт, в котором эффект выдали, уже вычел единицу: клиенту — полный
            // срок, как у BDS (30 секунд — 600, не 599).
            // Выданное командой — целые секунды: 599 и 601 — те же 600.
            let ticks_left = match effect.ticks_left % 20 {
                19 if effect.ticks_left > 0 => effect.ticks_left + 1,
                1 if effect.ticks_left > 1 => effect.ticks_left - 1,
                _ => effect.ticks_left,
            };
            // У BDS «мгновенные» эффекты несут не такты, а число из команды: saturation 30 — d30, saturation 1 — d1.
            let ticks_left = if matches!(effect.kind, EffectKind::Saturation | EffectKind::InstantHealth | EffectKind::InstantDamage) && ticks_left > 0 { (ticks_left + 19) / 20 } else { ticks_left };
            let shown = Effect { ticks_left, ..*effect };
            let known = self.sent.iter().any(|e| e.kind == effect.kind);
            packets.extend(mob_effect(id, if known { 2 } else { 1 }, shown));
            self.sent.retain(|e| e.kind != effect.kind);
            self.sent.push(shown);
            self.effects.retain(|(e, _)| e.kind != effect.kind);
            self.effects.push((*effect, tick));
            if !known {
                self.order.insert(0, effect.kind);
                if let Some(n) = effect_sound(effect.kind, true) { sound(n, packets); }
            }
        }
        let newest_first: Vec<Effect> = self.order.iter().filter_map(|kind| vit.effects.iter().find(|e| e.kind == *kind)).copied().collect();
        let visible = pack_visible(&newest_first);
        if visible != self.visible { self.visible = visible; packets.push(visible_effects(id, visible)); }
        self.effects.retain(|(e, _)| vit.effects.iter().any(|v| v.kind == e.kind));
        let env = (vit.air, frozen);
        if self.environment != Some(env) {
            packets.push(environment(id, env.0, env.1));
            self.environment = Some(env);
        }
    }

    pub fn attributes(&mut self, id: u64, attrs: &crate::attributes::Attributes, effects: &[Effect], packets: &mut Vec<Vec<u8>>) {
        let packet = combat_attributes(id, attrs, effects);
        if self.attributes.is_empty() {
            // Клиент знает обычную скорость с входа: без отклонений первым пакетом не шлём.
            let plain = combat_attributes(id, &crate::attributes::Attributes::player(), &[]);
            self.attributes = plain;
        }
        if self.attributes != packet {
            self.attributes.clone_from(&packet);
            packets.push(packet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::codec::{In, packet_id};
    fn effect(kind: EffectKind) -> Effect {
        Effect { kind, amplifier: 3, ticks_left: -1, ambient: true, visible: false, show_icon: true }
    }
    #[test]
    fn shield_spin_and_visibility_flags_and_equipment_round_trip() {
        let bytes = flags(7, Flags { using: true, invisible: true, burning: true, spinning: true, blocking: true, ..Default::default() });
        let (id, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!(id, 39); assert_eq!((r.varint64(), r.varint()), (Some(7), Some(2)));
        assert_eq!((r.varint(), r.varint()), (Some(0), Some(7)));
        let low = r.zigzag64().unwrap();
        for bit in [0, 4, 5, 19, 35, 48, 49, 56] { assert_ne!(low & (1 << bit), 0); }
        assert_eq!((r.varint(), r.varint(), r.zigzag64()), (Some(92), Some(7), Some(1 << 8)));
        assert_eq!((r.varint(), r.varint(), r.varint64(), r.remaining()), (Some(0), Some(0), Some(0), 0));
        assert_eq!(armor(7, &[None; 5]), [32, 7, 0, 0, 0, 0, 0]);
        assert_eq!(offhand(7, None), [31, 7, 0, 0, 0, 119]);
        let stack = crate::inventory::Stack::new(crate::blocks::item_named("shield").unwrap(), 1);
        let bytes = offhand(7, Some(stack)); let (_, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!(r.varint64(), Some(7)); assert_eq!(r.zigzag32(), Some(super::super::inventory::bedrock_item(stack.item).unwrap().0));
        assert_eq!(r.take(2), Some(1u16.to_le_bytes().as_slice()));
        assert!(r.varint().is_some()); assert_eq!(r.bool(), Some(false)); assert!(r.zigzag32().is_some());
        let size = r.varint().unwrap() as usize; assert_eq!(size, 18); assert!(r.take(size).is_some());
        assert_eq!((r.u8(), r.u8(), r.u8(), r.remaining()), (Some(0), Some(0), Some(119), 0));
    }

    #[test]
    fn every_effect_is_explicit_and_native_ids_are_unique() {
        let mut ids = Vec::new();
        for kind in EffectKind::ALL {
            if let Some(id) = effect_id(kind) {
                assert!(!ids.contains(&id)); ids.push(id);
                let bytes = mob_effect(300, 2, effect(kind)).unwrap();
                let (packet, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
                assert_eq!(packet, 28);
                assert_eq!((r.varint64(), r.u8(), r.zigzag32(), r.zigzag32()), (Some(300), Some(2), Some(id), Some(3)));
                assert_eq!((r.bool(), r.zigzag32(), r.varint64(), r.bool()), (Some(false), Some(-1), Some(0), Some(true)));
                assert_eq!(r.remaining(), 0);
            }
        }
        assert_eq!(ids.len(), 36);
        assert_eq!(effect_id(EffectKind::Levitation), Some(24));
        assert_eq!(effect_id(EffectKind::WindCharged), Some(32));
    }
    #[test]
    fn effects_add_refresh_expire_and_do_not_resend_every_tick() {
        let mut view = View::default(); let mut vit = Vitals::new(); let mut packets = Vec::new();
        vit.give_effect_shown(Effect { ticks_left: 10, ..effect(EffectKind::Darkness) });
        view.sync(1, &vit, 0, 1, (0.0, 0.0, 0.0), &mut packets); assert_eq!(packets.len(), 2);
        packets.clear(); vit.effects_tick(); view.sync(1, &vit, 0, 2, (0.0, 0.0, 0.0), &mut packets); assert!(packets.is_empty());
        vit.effects[0].ticks_left = 50; view.sync(1, &vit, 0, 2, (0.0, 0.0, 0.0), &mut packets); assert_eq!(packets.len(), 1);
        packets.clear(); vit.remove_effects(None); view.sync(1, &vit, 0, 3, (0.0, 0.0, 0.0), &mut packets);
        assert_eq!(packets.len(), 1); assert_eq!(packets[0][2], 3);
    }
    #[test]
    fn animation_cooldown_and_environment_round_trip() {
        for (java, bedrock) in [(0, 1), (3, 1), (2, 3), (4, 4), (5, 5)] {
            assert_eq!(animation(7, java), [44, bedrock, 7, 0, 0, 0, 0, 0]);
        }
        let bytes = cooldown("shield", 100); let (id, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!(id, 176); assert_eq!(r.string().as_deref(), Some("shield")); assert_eq!(r.zigzag32(), Some(100)); assert_eq!(r.remaining(), 0);
        let bytes = environment(7, -20, 140); let (id, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!(id, 39); assert_eq!((r.varint64(), r.varint()), (Some(7), Some(3)));
        for (key, value) in [(7, -20i16), (42, 300)] {
            assert_eq!((r.varint(), r.varint()), (Some(key), Some(1))); assert_eq!(r.take(2), Some(value.to_le_bytes().as_slice()));
        }
        assert_eq!((r.varint(), r.varint(), r.lf32()), (Some(120), Some(3), Some(1.0)));
        assert_eq!((r.varint(), r.varint(), r.varint64(), r.remaining()), (Some(0), Some(0), Some(0), 0));
    }
    #[test]
    fn attributes_use_documented_six_float_layout() {
        let attrs = crate::attributes::Attributes::player();
        let bytes = combat_attributes(123, &attrs, &[]); let (id, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!(id, 29); assert_eq!((r.varint64(), r.varint()), (Some(123), Some(1)));
        for _ in 0..6 { assert!(r.lf32().unwrap().is_finite()); }
        assert_eq!(r.string().as_deref(), Some("minecraft:movement")); assert_eq!(r.varint(), Some(0));
        assert_eq!((r.varint64(), r.remaining()), (Some(0), 0));
    }
    #[test]
    fn speed_effect_adds_the_bds_modifier() {
        // br-record, effects: Speed II — movement 0,14, модификатор MovementSpeed +0,4 (операция 2, операнд 2).
        let mut attrs = crate::attributes::Attributes::player();
        let speed = Effect { amplifier: 1, ticks_left: 600, ambient: false, visible: true, ..effect(EffectKind::Speed) };
        let bytes = combat_attributes(1, &attrs, &[speed]); let (_, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body);
        assert_eq!((r.varint64(), r.varint()), (Some(1), Some(1)));
        let floats: Vec<f32> = (0..6).map(|_| r.lf32().unwrap()).collect();
        assert_eq!((floats[0], floats[1], floats[5]), (0.0, f32::MAX, 0.1));
        assert_eq!(r.string().as_deref(), Some("minecraft:movement")); assert_eq!(r.varint(), Some(1));
        assert_eq!(r.string().as_deref(), Some("91aeaa56-376b-4498-935b-2f7f68070635"));
        assert_eq!(r.string().as_deref(), Some("MovementSpeed"));
        assert_eq!((r.lf32(), r.li32(), r.li32(), r.bool()), (Some(0.4), Some(2), Some(2), Some(false)));
        // Без отклонений от обычного первым пакетом ничего не уходит.
        let mut view = View::default(); let mut packets = Vec::new();
        view.attributes(1, &attrs, &[], &mut packets); assert!(packets.is_empty());
        attrs = crate::attributes::Attributes::player();
        view.attributes(1, &attrs, &[speed], &mut packets); assert_eq!(packets.len(), 1);
    }
    #[test]
    fn own_flags_drop_the_nametag_bits_and_fire_resistance_sets_bit_50() {
        // br-record, effects: fire_resistance 844459290394624 + 2^50; свои флаги без битов 14 и 15.
        let low = |f: Flags| { let bytes = flags(1, f); let (_, body) = packet_id(&bytes).unwrap(); let mut r = In::new(body); r.varint64(); r.varint(); r.varint(); r.varint(); r.zigzag64().unwrap() };
        let others = low(Flags::default());
        let own = low(Flags { own: true, ..Default::default() });
        assert_eq!(others & (3 << 14), 3 << 14);
        assert_eq!(own & (3 << 14), 0);
        assert_eq!(own, 844459290394624);
        assert_eq!(low(Flags { own: true, fire_immune: true, ..Default::default() }), own | 1 << 50);
    }
    #[test]
    fn visible_effects_pack_newest_first_like_bds() {
        // br-record, multi: speed → 2; +slowness → 260; +haste → 33286; скрытый (без частиц) не входит.
        let shown = |kind| Effect { visible: true, ambient: false, ..effect(kind) };
        assert_eq!(pack_visible(&[shown(EffectKind::Speed)]), 2);
        assert_eq!(pack_visible(&[shown(EffectKind::Slowness), shown(EffectKind::Speed)]), 260);
        assert_eq!(pack_visible(&[shown(EffectKind::Haste), shown(EffectKind::Slowness), shown(EffectKind::Speed)]), 33286);
        assert_eq!(pack_visible(&[Effect { visible: false, ..shown(EffectKind::Slowness) }, shown(EffectKind::Speed)]), 2);
        assert_eq!(pack_visible(&[shown(EffectKind::Darkness)]), 0);
        assert_eq!(pack_visible(&[shown(EffectKind::Absorption)]), 44);
    }
    #[test]
    fn effects_keep_their_full_duration_and_remove_with_the_sent_one() {
        // BDS: /effect … 30 → add с 600, remove — снова 600; невидимость — звуки, видимые эффекты — метаданные.
        let mut view = View::default(); let mut vit = Vitals::new(); let mut packets = Vec::new();
        vit.give_effect_shown(Effect { ticks_left: 600, visible: true, ambient: false, ..effect(EffectKind::Invisibility) });
        vit.effects_tick();
        view.sync(1, &vit, 0, 1, (0.0, 0.0, 0.0), &mut packets);
        let (_, body) = packet_id(&packets[0]).unwrap(); let mut r = In::new(body);
        assert_eq!((r.varint64(), r.u8(), r.zigzag32(), r.zigzag32(), r.bool(), r.zigzag32()), (Some(1), Some(1), Some(14), Some(3), Some(true), Some(600)));
        assert!(packets.iter().any(|p| packet_id(p).is_some_and(|(id, _)| id == 0x7b)), "звук Disappeared");
        assert!(packets.iter().any(|p| p == &visible_effects(1, 28)));
        packets.clear(); vit.effects_tick(); vit.effects_tick(); vit.remove_effects(None);
        view.sync(1, &vit, 0, 3, (0.0, 0.0, 0.0), &mut packets);
        let (_, body) = packet_id(&packets[0]).unwrap(); let mut r = In::new(body);
        assert_eq!((r.varint64(), r.u8(), r.zigzag32(), r.zigzag32(), r.bool(), r.zigzag32()), (Some(1), Some(3), Some(14), Some(3), Some(true), Some(600)));
        assert!(packets.iter().any(|p| p == &visible_effects(1, 0)));
    }
    #[test]
    fn continuously_renewed_effect_is_refreshed_before_client_expiry() {
        let mut view = View::default(); let mut vit = Vitals::new(); let mut packets = Vec::new();
        vit.give_effect_shown(Effect { ticks_left: 40, ..effect(EffectKind::NightVision) });
        view.sync(1,&vit,0,0,(0.0,0.0,0.0),&mut packets); packets.clear();
        for tick in 1..40 { view.sync(1,&vit,0,tick,(0.0,0.0,0.0),&mut packets); }
        assert!(packets.iter().any(|p| packet_id(p).is_some_and(|(id,_)| id == 0x1c)));
        packets.clear();
        for tick in 40..80 { vit.effects_tick(); view.sync(1,&vit,0,tick,(0.0,0.0,0.0),&mut packets); }
        assert!(!packets.iter().any(|p| p.get(2) == Some(&2)),"обычное убывание не требует обновлений");
    }

}
