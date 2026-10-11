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
use crate::survival::{Source, Vitals};

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
        let mut done = Done::default();

        match act {
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
                use_held(shared, p, hand, now);
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
fn use_held(shared: &Shared, p: &mut PlayerState, hand: i32, now: u64) {
    if thrown::throw_potion(shared, p, hand) {
        return;
    }

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
            crate::survival::EffectKind::InstantDamage => done.damage.push((crate::survival::instant_base(crate::survival::EffectKind::InstantDamage, effect.amplifier) as f32, Source::Magic)),
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
    if strike.source != Source::Thorns
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
        }

        return;
    }

    let (armor, toughness) = (p.attrs.value(crate::attributes::ARMOR) as f32, p.attrs.value(crate::attributes::ARMOR_TOUGHNESS) as f32);
    let points: i32 = combat::ARMOR_SLOTS.iter().filter_map(|(slot, _)| p.inventory.slot(*slot)).map(|stack| combat::protection_points(&stack.extras(), strike.source)).sum();
    damage = combat::mitigated_damage(damage, armor, toughness, points, strike.source, strike.breach_scale);

    // Внутри неуязвимости проходит только то, что сильнее прошлого удара.
    let landed = damage > 0.0 && !(p.vit.invulnerable > 10 && !strike.source.bypasses_cooldown() && damage <= p.vit.last_hurt());

    done.damage.push((damage, strike.source));

    if !landed {
        return;
    }

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
