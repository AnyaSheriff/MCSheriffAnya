// Тело игрока Bedrock для общего кода боя и использования предметов.
//
// Бой, лук, арбалет, трезубец, еда и щит у Java живут в `PlayerState`
// (play.rs): удар, натяжение, отпускание считают поля этого состояния.
// Чтобы Bedrock-игрок бил, стрелял и ел ровно так же, а не по копии правил,
// у него есть такое состояние-«тело»: подключение Bedrock перед каждым
// действием кладёт в него то, чем владеет само (положение, инвентарь,
// здоровье), вызывает тот же код, что у Java, и забирает изменённое обратно.
// Тянущееся действие (лук натянут, щит поднят, еда идёт) хранится в самом
// теле между вызовами.
//
// Что не сверено с оригиналом, помечено «[проверить]».

use super::*;

use crate::combat::PlayerStrike;
use crate::combat;
use crate::survival::{self, Source, Vitals};

/// Сколько тактов до конца еды ещё считается «доел»: отпускание кнопки у
/// клиента приходит раньше такта сервера на задержку сети [проверить].
const EAT_SLACK: i32 = 6;

/// Где игрок и в каком он состоянии — то, чем владеет Bedrock-подключение.
pub(crate) struct Stance {
    pub game_mode: i32,
    pub feet: (f64, f64, f64),
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub sneaking: bool,
    pub dimension: Dimension,
}

/// Что сделал игрок (или с ним).
pub(crate) enum Act<'a> {
    /// Удар по сущности с этим номером (бой игроков, моб, вагонетка).
    Attack(i32),
    /// Правая кнопка на сущности: кормление, стрижка, огниво, рамка, окно
    /// вагонетки и торговля жителя — тот же `use_entity`, что у Java.
    Interact(i32),
    /// Правая кнопка в руке `hand`: бросок, еда, лук, арбалет, щит.
    Use(i32),
    /// Кнопка отпущена: лук спущен, еда либо доедена, либо прервана.
    Release,
    /// Такт: зарядка арбалета, еда.
    Tick,
    /// Среда и реакции эффектов: только адаптер Bedrock.
    Environment,
    Reactions,
    Attribute(&'a crate::commands_world::AttributeEdit),
    Spawn((f64, f64, f64)),
    RespawnBlock { at: (i32, i32, i32), face: i32 },
    MobHit { amount: f32, source: Source, attacker: i32, killer: &'static str, push: Option<(f64, f64)>, at: (f64, f64, f64) },
    /// Удар игрока по этому игроку (из журнала мира): щит, броня, отдача.
    Struck(&'a PlayerStrike),
    /// Предмет в руке щёлкнут по блоку: ракета фейерверка летит из точки
    /// щелчка `at` от грани `face` (тот же пуск, что у Java; тратится не в
    /// творческом режиме).
    UseOnBlock { at: (f64, f64, f64), face: i32 },
}

/// Что изменилось после действия.
#[derive(Default)]
pub(crate) struct Done {
    /// Слоты инвентаря, которые стали другими.
    pub slots: Vec<i32>,
    /// Скорость, которую надо сообщить клиенту (блоков за такт).
    pub velocity: Option<(f64, f64, f64)>,
    /// Урон, который надо принять: уже после брони и щита.
    pub damage: Vec<(f32, Source)>,
    /// Вагонетка-хранилище, чьё окно надо открыть.
    /// [Bedrock: потом] Java использует это поле и для конского окна.
    pub open_cart: Option<i32>,
    /// Житель, с которым надо открыть торговлю.
    pub open_merchant: Option<i32>,
    /// Доеденный или допитый предмет (номер Java): Bedrock-клиенту — Completed
    /// Using Item и, для еды, отрыжка (так у BDS).
    pub consumed: Option<(i32, bool)>,
    pub cooldown: Option<(&'static str, i32)>,
    pub shield_blocked: bool,
    pub landed: bool,
    pub block_used: bool,
    /// Урон до брони (среда, шипы, магия), в отличие от damage.
    pub raw_damage: Vec<(f32, Source)>,
}

pub(crate) struct Body {
    state: Box<PlayerState>,
}

impl Body {
    pub(crate) fn new(uuid: [u8; 16], name: String, entity_id: i32) -> Self {
        let mut state = PlayerState::new(uuid, name, None, None, 0, 0, (0.0, 0.0, 0.0));

        state.entity_id = entity_id;

        Self { state: Box::new(state) }
    }

    /// Bedrock покидает транспорт тем же кодом, который освобождает водителя Java.
    pub(crate) fn dismount(&mut self, shared: &Shared, dimension: Dimension) {
        self.state.dimension = dimension;
        vehicle::dismount(shared, &self.state);
        self.state.ride.vehicle = None;
    }

    /// PlayerAuthInput называет управляемый транспорт; движение проходит
    /// те же проверки водителя и лошади, что Move Vehicle у Java.
    pub(crate) fn drive(&mut self, shared: &Shared, stance: &Stance, target: i32, at: (f64, f64, f64), rotation: (f32, f32), flags: u128) {
        let p = &mut *self.state;
        let allowed = {
            let riding = shared.riding.lock().expect("учёт седоков");
            riding.vehicle_of(p.entity_id) == Some(target) && riding.driver(target) == Some(p.entity_id)
        };
        if !allowed || !at.0.is_finite() || !at.1.is_finite() || !at.2.is_finite() || !rotation.0.is_finite() || !rotation.1.is_finite() { return; }
        let near = {
            let items = shared.items_in(stance.dimension).lock().expect("предметы");
            let old = items.carts().iter().find(|c| c.entity_id == target).map(|c| (c.x, c.y, c.z))
                .or_else(|| items.mobs().iter().find(|m| m.entity_id == target && !m.is_dead()).map(|m| (m.x, m.y, m.z)));
            old.is_some_and(|old| (old.0 - at.0).hypot(old.1 - at.1).hypot(old.2 - at.2) <= 8.0)
        };
        if !near { return; }
        p.dimension = stance.dimension;
        p.game_mode = stance.game_mode;
        p.ride.vehicle = Some(target);
        let input = u8::from(flags & (1 << 10) != 0) | (u8::from(flags & (1 << 11) != 0) << 1)
            | (u8::from(flags & (1 << 12) != 0) << 2) | (u8::from(flags & (1 << 13) != 0) << 3)
            | (u8::from(flags & (1 << 6) != 0) << 4);
        horse::input(shared, p, input);
        vehicle::input(input, p, shared);
        let mut payload = Vec::new();
        for v in [at.0, at.1, at.2] { push_f64(&mut payload, v); }
        push_f32(&mut payload, rotation.0); push_f32(&mut payload, rotation.1);
        payload.push(u8::from(flags & (1 << 50) != 0));
        if !horse::vehicle_move(&payload, p, shared, &mut Reaction::default()) { vehicle::vehicle_move(&payload, p, shared); }
        vehicle::steer_boat(&[u8::from(flags & (1 << 46) != 0), u8::from(flags & (1 << 47) != 0)], p, shared);
    }
    pub(crate) fn restore_respawn(&mut self, point: Option<crate::bed::RespawnPoint>) { self.state.respawn = point; }
    pub(crate) fn respawn_point(&self) -> Option<crate::bed::RespawnPoint> { self.state.respawn }
    pub(crate) fn respawn_target(&mut self, shared: &Shared, fallback: (f64, f64, f64)) -> (Dimension, (f64, f64, f64), Option<f32>, bool) {
        self.state.spawn_point = fallback;
        let target = sleep::respawn_target(shared, &mut self.state);
        (target.dimension, target.at, target.yaw, target.missing)
    }
    pub(crate) fn moved(&mut self, delta: (f64, f64, f64)) { self.state.net.last_delta = delta; }
    pub(crate) fn spinning(&self) -> bool { self.state.net.spin_since.is_some() }
    pub(crate) fn blocking(&self, now: u64) -> bool { vitals::shield_up(&self.state, now).is_some() }
    pub(crate) fn mob_hit_landed(&self) -> bool { self.state.net.mob_hit_landed }
    pub(crate) fn attributes(&self) -> &crate::attributes::Attributes { &self.state.attrs }
    pub(crate) fn frozen(&self) -> i32 { vitals::bedrock_frozen(&self.state) }
    pub(crate) fn respawned(&mut self) { vitals::respawned(&mut self.state); }

    /// Тянется ли что-то, чему нужен такт (еда, зарядка арбалета).
    pub(crate) fn busy(&self) -> bool {
        self.state.net.using.is_some() || self.state.net.eating.is_some()
    }

    fn load(&mut self, stance: &Stance, inventory: &Inventory, vit: &Vitals, last_attack: u64) {
        let p = &mut *self.state;

        p.game_mode = stance.game_mode;
        (p.x, p.y, p.z) = stance.feet;
        p.yaw = stance.yaw;
        p.pitch = stance.pitch;
        p.on_ground = stance.on_ground;
        p.sneaking = stance.sneaking;
        p.dimension = stance.dimension;
        p.inventory = inventory.clone();
        p.vit = vit.clone();
        p.last_attack = last_attack;
        p.net.velocity = None;
        p.net.slot_update = None;
        p.net.slots_dirty.clear();
    }

    /// Вещи на теле и бой — в том виде, в каком это видят остальные игроки.
    pub(crate) fn gear(&mut self, shared: &Shared, stance: &Stance, inventory: &Inventory, vit: &Vitals, last_attack: u64) -> crate::players::Gear {
        self.load(stance, inventory, vit, last_attack);
        vitals::update_equipment(&mut self.state, *shared.ticks.borrow());
        self.state.gear()
    }

    /// Выполняет действие общим кодом и возвращает изменённое.
    pub(crate) fn act(&mut self, shared: &Shared, stance: &Stance, inventory: &mut Inventory, vit: &mut Vitals, last_attack: &mut u64, act: Act) -> Done {
        self.load(stance, inventory, vit, *last_attack);

        let now = *shared.ticks.borrow();
        let p = &mut *self.state;
        p.ride.vehicle = shared.riding.lock().expect("учёт седоков").vehicle_of(p.entity_id);
        let mut done = Done::default();
        vitals::update_equipment(p, now);
        vitals::apply_effect_modifiers(p);

        match act {
            Act::Spawn(at) => sleep::set_forced(p, at),
            Act::RespawnBlock { at, face } => {
                let mut world = shared.world_in(p.dimension).lock().expect("мир");
                let state = world.get_block(at.0, at.1, at.2);
                if crate::bed::is_anchor(state) {
                    let held = p.inventory.held().and_then(|s| blocks::item_name(s.item));
                    let charges = crate::bed::anchor_charges(state).unwrap_or(0);
                    if held == Some("glowstone") && charges < 4 {
                        if let Some(next) = crate::bed::with_charges(state, charges + 1) {
                            world.set_block(at.0, at.1, at.2, next);
                            world.note_sound(at.0, at.1, at.2, "minecraft:block.respawn_anchor.charge", 1.0, 1.0);
                            if p.game_mode != GAME_MODE_CREATIVE { p.inventory.spend_held(); }
                        }
                        done.block_used = true;
                    } else { done.block_used = sleep::use_anchor(&mut world, shared, p, at, face, held, 0).is_some(); }
                } else if crate::bed::beds_work(p.dimension) && let Some(head) = crate::bed::head_of(&world, at) {
                    // Точка кровати общая; анимация сна Bedrock [проверить].
                    if crate::bed::bed_in_reach(&world, head, (p.x, p.y, p.z)) && !crate::bed::bed_obstructed(&world, head) {
                        p.respawn = Some(crate::bed::RespawnPoint { dimension: p.dimension, pos: head, yaw: p.yaw, forced: false });
                    }
                    done.block_used = true;
                }
            }
            Act::MobHit { amount, source, attacker, killer, push, at } => {
                let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
                let amount = if source.scales_with_difficulty() { survival::scaled(amount, difficulty) } else { amount };
                let strike = PlayerStrike { victim: p.entity_id, attacker, attacker_uuid: [0; 16], attacker_name: killer.to_string(),
                    from: at, yaw: 0.0, damage: amount, source, extra_knockback: 0.0, weapon: None,
                    disables_shield_ticks: if killer == "entity.minecraft.warden" { 100 } else { 0 }, breach_scale: 1.0,
                    fire_ticks: 0, ignores_shield: !(matches!(source, Source::MobAttack | Source::Sting) || source.is_projectile()),
                    thorns_slots: Vec::new(), effects: Vec::new() };
                struck(shared, p, &strike, now, &mut done);
                p.net.mob_hit_landed = done.landed;
                if done.landed {
                    pvp::retaliate(shared, p, source, attacker);
                    if killer == "entity.minecraft.cave_spider" && let Some(effect) = crate::mob::arthropod::poison(difficulty) { p.vit.give_effect_shown(effect); }
                    if source == Source::Sting && let Some(effect) = crate::mob::wildlife::poison(difficulty) { p.vit.give_effect_shown(effect); }
                }
                // Sonic Boom: общий расчёт силы хранителя, включая стойкость.
                if done.landed && source == Source::SonicBoom {
                    let delta = (p.x - at.0, p.y + 1.62 - at.1, p.z - at.2);
                    let sonic = crate::mob::warden::sonic_velocity(delta, p.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE));
                    let flat = (p.x - at.0).hypot(p.z - at.2).max(1.0e-9);
                    p.net.velocity = Some((((p.x - at.0) / flat * 0.4 + sonic.0) * 0.546,
                        (0.3606 + sonic.1 - 0.08) * 0.98, ((p.z - at.2) / flat * 0.4 + sonic.2) * 0.546));
                } else if p.net.velocity.is_some() {
                    p.net.velocity = push.map(|(x, z)| (x * 0.218, if p.on_ground { 0.275 } else { p.net.last_delta.1 }, z * 0.218));
                }
            }
            Act::Attribute(edit) => { crate::commands_world::apply_attribute_edit(&mut p.attrs, edit); vitals::apply_effect_modifiers(p); }
            Act::Environment => done.raw_damage.extend(vitals::bedrock_environment(shared, p)),
            Act::Reactions => {
                vitals::effect_reactions(shared, p, p.vit.dead);
                if p.vit.dead { p.net.using = None; p.net.eating = None; p.net.spin_since = None; }
            },
            // Вики, Melee attack: общие charge/strike и reach 3+3 из pvp.rs.
            // Клиент Bedrock не рисует заряд: частые клики ослабляются тем же
            // серверным таймером; отдельного полного урона нет [проверить UX].
            Act::Attack(target) => attack(target, p, shared),
            Act::Interact(target) => {
                let reaction = use_entity(target, p, shared);

                log_debug!("Bedrock-тело: щелчок по {}, в руке {:?}, режим {}", target, p.inventory.held().and_then(|stack| blocks::item_name(stack.item)), p.game_mode);

                done.open_cart = reaction.open_cart;
                done.open_merchant = reaction.open_merchant;
            }
            Act::UseOnBlock { at, face } => {
                if super::thrown::launch_rocket(shared, p, at, face)
                    && p.game_mode != crate::commands::GAME_MODE_CREATIVE
                    && let Some(slot) = p.inventory.spend_held()
                {
                    done.slots.push(slot);
                }
            }
            Act::Use(hand) => {
                // Новая правая кнопка — старое натяжение, раз оно не
                // кончилось отпусканием (клиент мог его бросить), забыто.
                p.net.using = None;
                p.net.eating = None;
                use_held(shared, p, hand, now, &mut done);
            }
            Act::Release => {
                if let Some(eating) = p.net.eating.take()
                    && eating.ticks_left <= EAT_SLACK
                    && vitals::eating_matches(p, &eating)
                {
                    finish(shared, p, eating, &mut done);
                }

                vitals::release_use(p, shared);
            }
            Act::Tick => {
                // Bedrock поднимает щит приседанием (вики, Shield: «sneaking»).
                // Задержка и отключение — общие Java-правила смешанного PvP.
                if p.net.using.is_some_and(|using| using.kind == vitals::UseKind::Shield) {
                    let hand = p.net.using.unwrap().hand;
                    let held = p.inventory.slot(if hand == 1 { inventory::OFFHAND } else { p.inventory.selected_slot() });
                    if !p.sneaking || held.and_then(|s| blocks::item_name(s.item)) != Some("shield") { p.net.using = None; }
                }
                if p.sneaking && p.net.using.is_none() {
                    for hand in [0, 1] {
                        let held = p.inventory.slot(if hand == 1 { inventory::OFFHAND } else { p.inventory.selected_slot() });
                        if held.and_then(|s| blocks::item_name(s.item)) == Some("shield") { vitals::start_using(shared, p, hand, now); break; }
                    }
                }
                vitals::tick_spin(shared, p, now);
                vitals::tick_crossbow(shared, p, now);

                if p.net.eating.as_ref().is_some_and(|eating| !vitals::eating_matches(p, eating)) {
                    vitals::stop_eating(p);
                }
                if let Some(eating) = &mut p.net.eating {
                    eating.ticks_left -= 1;

                    if eating.ticks_left <= 0
                        && let Some(eating) = p.net.eating.take()
                    {
                        finish(shared, p, eating, &mut done);
                    }
                }
            }
            Act::Struck(strike) => struck(shared, p, strike, now, &mut done),
        }

        // Общие атаки могут отложить шипы/магию. Их нельзя оставлять в
        // Java net.hits навсегда: у Bedrock другой цикл здоровья.
        for hit in std::mem::take(&mut p.net.hits) {
            done.raw_damage.push((hit.amount, hit.source));
            if let Some(dir) = hit.push {
                vitals::knock_back(p, dir, crate::mob::BASE_KNOCKBACK, p.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE));
            }
        }
        done.velocity = p.net.velocity.take();

        for slot in 0..inventory::SLOTS as i32 {
            if p.inventory.slot(slot) != inventory.slot(slot) {
                done.slots.push(slot);
            }
        }

        *inventory = p.inventory.clone();
        *vit = p.vit.clone();
        *last_attack = p.last_attack;

        done
    }
}

/// Правая кнопка: то же, что Use Item у Java (`use_item` в play.rs), без
/// ведра (его Bedrock ведёт своим путём).
fn use_held(shared: &Shared, p: &mut PlayerState, hand: i32, now: u64, done: &mut Done) {
    let held = p.inventory.slot(if hand == 1 { inventory::OFFHAND } else { p.inventory.selected_slot() });
    if fishing::use_rod(shared, p, hand).is_some() || vehicle::place_boat(shared, p, hand) {
        return;
    }
    if thrown::throw_potion(shared, p, hand) {
        if held.and_then(|s| blocks::item_name(s.item)) == Some("ender_pearl") { done.cooldown = Some(("ender_pearl", 20)); }
        return;
    }
    if thrown::throw_wind_charge(shared, p, hand).is_some() {
        done.cooldown = Some(("wind_charge", crate::wind::COOLDOWN));
        return;
    }
    if super::maps::fill(shared,p,hand).is_some() {return;}
    if super::bundle_use::start(shared,p,hand,now) {return;}

    let thrown_eye = thrown::throw_eye(shared, p, hand);

    vitals::start_using_eye(p, hand, now);

    if thrown_eye || thrown::fill_breath(shared, p, hand) {
        return;
    }

    if vitals::start_eating_at(p, hand, *shared.ticks.borrow()) {
        return;
    }

    if bow::crossbow_fire(shared, p, hand) {
        return;
    }

    vitals::start_using(shared, p, hand, now);
}

/// Доел или допил: то же, что `finish_eating` и `finish_drinking` у Java,
/// без пакетов — звуки еды клиент Bedrock играет сам [проверить].
fn finish(shared: &Shared, p: &mut PlayerState, eating: vitals::Eating, done: &mut Done) {
    done.consumed = p.inventory.slot(eating.slot).map(|stack| (stack.item, eating.drink.is_none()));
    let name = blocks::item_name(eating.stack.item).unwrap_or("");
    let remainder;

    match eating.drink {
        Some(drink) => {
            remainder = drink.remainder();
            if drink == vitals::Drink::Milk {
                p.vit.remove_effects(None);
            } else if drink == vitals::Drink::Honey {
                p.vit.eat(6, 1.2);
                p.vit.remove_effects(Some(crate::survival::EffectKind::Poison));
            }
        }
        None => {
            p.vit.eat(eating.nutrition, eating.saturation);

            remainder = if matches!(name, "suspicious_stew" | "mushroom_stew" | "rabbit_stew" | "beetroot_soup") { "bowl" } else { "" };
        }
    }

    if let Some(left) = vitals::consume_stack(p, eating.slot, remainder) {
        super::throw_out(shared, p, left);
    }

    for effect in eating.effects {
        match effect.kind {
            crate::survival::EffectKind::InstantHealth => p.vit.heal(crate::survival::instant_base(crate::survival::EffectKind::InstantHealth, effect.amplifier) as f32),
            crate::survival::EffectKind::InstantDamage => done.raw_damage.push((crate::survival::instant_base(crate::survival::EffectKind::InstantDamage, effect.amplifier) as f32, Source::Magic)),
            _ => p.vit.give_effect_shown(effect),
        }
    }
}

/// Игрок-жертва получает удар из журнала мира: то же, что `take_strike` у
/// Java (щит, броня, отдача, износ), но урон не применяется, а отдаётся
/// наружу — его считает здоровье Bedrock.
fn struck(shared: &Shared, p: &mut PlayerState, strike: &PlayerStrike, now: u64, done: &mut Done) {
    let mortal = p.game_mode == GAME_MODE_SURVIVAL || p.game_mode == GAME_MODE_ADVENTURE;

    if !mortal || p.vit.dead {
        return;
    }

    vitals::update_equipment(p, now);

    let mut damage = strike.damage;

    // Щит (вики, Shield): держит удар из передней полусферы.
    if combat::shield_blocks(strike.source)
        && !strike.ignores_shield
        && damage > 0.0
        && let Some(hand) = vitals::shield_up(p, now)
        && combat::shield_covers(p.yaw, (p.x, p.z), (strike.from.0, strike.from.2))
    {
        let slot = if hand == 1 { inventory::OFFHAND } else { p.inventory.selected_slot() };

        if let Some(shield) = p.inventory.slot(slot) {
            let wear = combat::shield_wear(strike.damage);

            if wear > 0 {
                let dimension = p.dimension;
                let mut roll = || vitals::world_random(shared, dimension);
                let worn = combat::wear(&shield, wear, false, &mut roll);

                if worn > 0 {
                    pvp::replace_worn(shared, p, slot, shield, worn);
                }
            }
        }

        if strike.disables_shield_ticks > 0 {
            p.net.shield_disabled_until = now + u64::from(strike.disables_shield_ticks);
            p.net.using = None;
            done.cooldown = Some(("shield", strike.disables_shield_ticks.min(i32::MAX as u32) as i32));
        }
        done.shield_blocked = true;
        return;
    }

    let (armor, toughness) = (p.attrs.value(crate::attributes::ARMOR) as f32, p.attrs.value(crate::attributes::ARMOR_TOUGHNESS) as f32);
    let points: i32 = combat::ARMOR_SLOTS.iter().filter_map(|(slot, _)| p.inventory.slot(*slot)).map(|stack| combat::protection_points(&stack.extras(), strike.source)).sum();
    damage = combat::mitigated_damage(damage, armor, toughness, points, strike.source, strike.breach_scale);

    // Внутри неуязвимости проходит только то, что сильнее прошлого удара.
    let landed = p.vit.clone().hurt(damage, strike.source, false).is_some();

    done.damage.push((damage, strike.source));

    if !landed { return; }
    done.landed = true;
    for effect in &strike.effects { p.vit.give_effect_shown(*effect); }

    if !combat::bypasses_armor(strike.source) {
        pvp::wear_armor(shared, p, strike.damage);
    }

    for slot in &strike.thorns_slots {
        if let Some(stack) = p.inventory.slot(*slot) {
            pvp::replace_worn(shared, p, *slot, stack, 2);
        }
    }

    let (at, from) = ((p.x, p.z), strike.from);
    let away = (at.0 - from.0, at.1 - from.2);
    let length = away.0.hypot(away.1);
    let dir = if length > 1.0e-6 { (away.0 / length, away.1 / length) } else { crate::mob::look_direction(strike.yaw) };
    let resistance = p.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE);

    vitals::knock_back(p, dir, crate::mob::BASE_KNOCKBACK, resistance);

    if strike.extra_knockback > 0.0 && strike.source != Source::Arrow {
        let look = crate::mob::look_direction(strike.yaw);

        vitals::knock_back(p, look, f64::from(strike.extra_knockback) * 0.5, resistance);
    }

    if strike.fire_ticks > 0 {
        vitals::ignite(p, strike.fire_ticks);
    }

    p.net.pin = None;
}

// ---------------------------------------------------------------------------
// Торговля и вагонетки-хранилища: окна Bedrock берут то же, что и Java
// ---------------------------------------------------------------------------

/// Житель с этим номером: его сделки и опыт (для окна торговли Bedrock).
pub(crate) fn villager_state(shared: &Shared, dimension: Dimension, villager_id: i32) -> Option<crate::villager::State> {
    shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").mobs().iter().find(|mob| mob.entity_id == villager_id).map(|mob| mob.villager.clone())
}

/// Жителя больше нет или он мёртв — окно торговли закрывается.
pub(crate) fn villager_missing(shared: &Shared, dimension: Dimension, villager_id: i32) -> bool {
    !shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").mobs().iter().any(|mob| mob.entity_id == villager_id && !mob.is_dead())
}

/// Сделка Bedrock состоялась: счёт, опыт, звук, шарик (merchant.rs).
pub(crate) fn trade_paid(shared: &Shared, dimension: Dimension, villager_id: i32, index: usize) -> bool {
    merchant::trade_paid(shared, dimension, villager_id, index)
}

/// Житель больше ни с кем не торгует.
pub(crate) fn release_villager(shared: &Shared, dimension: Dimension, villager_id: i32) {
    merchant::release(shared, dimension, villager_id);
}

/// Вагонетка-хранилище для окна: вид, место и ячейки.
pub(crate) type CartInfo = (crate::minecart::Kind, (f64, f64, f64), Vec<Option<Stack>>);

/// Вагонетка-хранилище: вид, место и ячейки.
pub(crate) fn cart_info(shared: &Shared, dimension: Dimension, entity_id: i32) -> Option<CartInfo> {
    shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .carts()
        .iter()
        .find(|cart| cart.entity_id == entity_id)
        .map(|cart| (cart.kind, (cart.x, cart.y, cart.z), cart.slots.clone()))
}

/// Ячейки вагонетки, как их хранит мир.
pub(crate) fn cart_contents(shared: &Shared, dimension: Dimension, entity_id: i32) -> Option<Vec<Option<Stack>>> {
    cart_slots(shared, dimension, entity_id)
}

/// Кладёт ячейки в вагонетку (после запроса игрока).
pub(crate) fn store_cart(shared: &Shared, dimension: Dimension, entity_id: i32, slots: &[Option<Stack>]) {
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    if let Some(cart) = items.carts_mut().iter_mut().find(|found| found.entity_id == entity_id) {
        cart.slots.clone_from_slice(slots);
    }
}

#[cfg(test)]
mod bedrock_entity_tests {
    use super::*;
    use crate::mob::{Kind, Mob};
    fn shared() -> Shared {
        Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/bedrock-entity-test-ops.json")), crate::skins::Settings::default())
    }
    fn stance() -> Stance {
        Stance { game_mode: GAME_MODE_SURVIVAL, feet: (0.5, 0.0, 0.5), yaw: 0.0, pitch: 0.0, on_ground: true, sneaking: false, dimension: Dimension::Overworld }
    }
    fn use_mob(shared: &Shared, kind: Kind, item: &str) -> (Inventory, Done) {
        let mob = Mob::new(100, kind, (0.5, 0.0, 2.5));
        shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(mob);
        let mut inv = Inventory::new();
        inv.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named(item).unwrap(), 1)));
        let mut body = Body::new([13; 16], "BedrockTest".into(), 10);
        let done = body.act(shared, &stance(), &mut inv, &mut Vitals::new(), &mut 0, Act::Interact(100));
        (inv, done)
    }
    #[test]
    fn feeding_and_shearing_mutate_the_shared_mob() {
        let shared = shared();
        let (inv, done) = use_mob(&shared, Kind::Cow, "wheat");
        assert!(inv.held().is_none());
        assert!(!done.slots.is_empty());
        assert!(shared.items_in(Dimension::Overworld).lock().unwrap().mobs()[0].love > 0);
        let shared2 = self::shared();
        let (inv, _) = use_mob(&shared2, Kind::Sheep, "shears");
        assert_eq!(inv.held().and_then(|s| blocks::item_name(s.item)), Some("shears"));
        assert!(shared2.items_in(Dimension::Overworld).lock().unwrap().mobs()[0].sheared);
    }
    #[test]
    fn bucketing_removes_shared_fish_and_returns_its_data() {
        let shared = shared();
        let (inv, _) = use_mob(&shared, Kind::Axolotl, "water_bucket");
        assert_eq!(inv.held().and_then(|s| blocks::item_name(s.item)), Some("axolotl_bucket"));
        assert!(shared.items_in(Dimension::Overworld).lock().unwrap().mobs().is_empty());
    }
    #[test]
    fn only_current_driver_can_move_the_named_vehicle() {
        let shared = shared();
        let mut cart = crate::minecart::Minecart::new(100, crate::minecart::Kind::Boat(0), (0.5, 0.0, 2.5));
        cart.driver = Some(10);
        shared.items_in(Dimension::Overworld).lock().unwrap().add_cart(cart);
        shared.riding.lock().unwrap().mount(10, 100, 2);
        shared.riding.lock().unwrap().mount(11, 100, 2);
        let mut rider = Body::new([14; 16], "Passenger".into(), 11);
        rider.drive(&shared, &stance(), 100, (2.0, 0.0, 2.5), (0.0, 0.0), 0);
        assert_eq!(shared.items_in(Dimension::Overworld).lock().unwrap().carts()[0].x, 0.5);
        let mut driver = Body::new([13; 16], "Driver".into(), 10);
        driver.drive(&shared, &stance(), 100, (2.0, 0.0, 2.5), (0.0, 0.0), 1 << 46);
        let items = shared.items_in(Dimension::Overworld).lock().unwrap();
        assert_eq!(items.carts()[0].x, 2.0);
        assert_eq!(items.carts()[0].paddle, (true, false));
        drop(items);
        driver.drive(&shared, &stance(), 100, (200.0, 0.0, 2.5), (0.0, 0.0), 0);
        driver.drive(&shared, &stance(), 101, (3.0, 0.0, 2.5), (0.0, 0.0), 0);
        assert_eq!(shared.items_in(Dimension::Overworld).lock().unwrap().carts()[0].x, 2.0);
    }

    #[test]
    fn bedrock_dismount_releases_shared_driver() {
        let shared = shared();
        let mut body = Body::new([13; 16], "BedrockTest".into(), 10);
        let mut horse = Mob::new(100, Kind::Horse, (0.5, 0.0, 2.5));
        horse.horse.rider = Some(([13; 16], 10));
        shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(horse);
        assert!(shared.riding.lock().unwrap().mount(10, 100, 1));
        body.dismount(&shared, Dimension::Overworld);
        assert!(shared.riding.lock().unwrap().vehicle_of(10).is_none());
        assert!(shared.items_in(Dimension::Overworld).lock().unwrap().mobs()[0].horse.rider.is_none());
    }
}

#[cfg(test)]
mod combat_tests {
    use super::*;

    fn shared() -> Shared {
        Shared::new(World::in_memory(), World::in_memory(), std::path::PathBuf::new(), std::path::PathBuf::new(),
            Default::default(), crate::ops::Ops::open(&std::env::temp_dir().join("bedrock-combat-test-ops.json")), crate::skins::Settings::default())
    }

    fn stance() -> Stance {
        Stance { game_mode: 0, feet: (0.0, 64.0, 0.0), yaw: -90.0, pitch: 0.0, on_ground: true, sneaking: false, dimension: Dimension::Overworld }
    }

    #[test]
    fn bedrock_attacks_share_java_reach_and_charge() {
        for (distance, accepted) in [(6.25, true), (6.4, false)] {
            let shared = shared(); let mut body = Body::new([1; 16], "Bedrock".into(), 1);
            let victim = PlayerState::new([2; 16], "Java".into(), None, None, 0, 0, (distance, 64.0, 0.0));
            shared.players.lock().unwrap().join(victim.member(), 2).unwrap();
            let reader = shared.next_reader();
            let from = shared.world_in(Dimension::Overworld).lock().unwrap().watch_events(reader);
            let mut inventory = Inventory::new();
            inventory.set(inventory.selected_slot(), Some(Stack::new(blocks::item_named("diamond_sword").unwrap(), 1)));
            let mut vit = Vitals::new(); let mut last = 0;
            shared.ticks.send_replace(100);
            body.act(&shared, &stance(), &mut inventory, &mut vit, &mut last, Act::Attack(2));
            shared.ticks.send_replace(101);
            body.act(&shared, &stance(), &mut inventory, &mut vit, &mut last, Act::Attack(2));
            let (events, _) = shared.world_in(Dimension::Overworld).lock().unwrap().events_since(from, reader);
            let hits: Vec<_> = events.iter().filter_map(|e| match e { WorldEvent::PlayerStrike(s) => Some(s.damage), _ => None }).collect();
            if accepted { assert_eq!(hits.len(), 2); assert!(hits[0] > hits[1] * 3.0, "{hits:?}"); }
            else { assert!(hits.is_empty()); }
        }
    }

    #[test]
    fn bedrock_sneak_shield_disables_and_recovers_on_server_timer() {
        let shared = shared(); let mut body = Body::new([1; 16], "Bedrock".into(), 1);
        let mut stance = stance(); stance.sneaking = true; stance.yaw = 0.0;
        let mut inventory = Inventory::new(); let mut vit = Vitals::new(); let mut last = 0;
        inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("shield").unwrap(), 1)));
        shared.ticks.send_replace(100);
        body.act(&shared, &stance, &mut inventory, &mut vit, &mut last, Act::Tick);
        assert!(!body.blocking(104)); assert!(body.blocking(105));
        shared.ticks.send_replace(105);
        let strike = PlayerStrike { victim: 1, attacker: 2, attacker_uuid: [2;16], attacker_name: "Java".into(),
            from: (0.0,64.0,2.0), yaw: 180.0, damage: 9.0, source: Source::PlayerAttack, extra_knockback: 0.0,
            weapon: None, disables_shield_ticks: 100, breach_scale: 1.0, fire_ticks: 0, ignores_shield: false,
            thorns_slots: Vec::new(), effects: Vec::new() };
        let done = body.act(&shared, &stance, &mut inventory, &mut vit, &mut last, Act::Struck(&strike));
        assert!(done.shield_blocked); assert!(done.damage.is_empty()); assert_eq!(done.cooldown, Some(("shield",100)));
        assert!(!body.blocking(205));
        shared.ticks.send_replace(205);
        body.act(&shared, &stance, &mut inventory, &mut vit, &mut last, Act::Tick);
        assert!(!body.blocking(209)); assert!(body.blocking(210));
    }

    #[test]
    fn body_keeps_smash_forgiveness_and_applies_effect_attributes() {
        let shared = shared(); let mut body = Body::new([1;16], "Bedrock".into(), 1);
        let mut inventory = Inventory::new(); let mut vit = Vitals::new(); let mut last = 0;
        vit.smash_forgives = true;
        vit.give_effect(survival::EffectKind::Strength, 1, 100);
        vit.give_effect(survival::EffectKind::HealthBoost, 1, 100);
        body.act(&shared, &stance(), &mut inventory, &mut vit, &mut last, Act::Tick);
        assert!(vit.smash_forgives); assert_eq!(vit.max_health, 28.0);
        assert_eq!(body.attributes().value(crate::attributes::ATTACK_DAMAGE), 7.0);
        assert!(vit.land(-10.0, true, false, 1.0).is_none());
    }
}
