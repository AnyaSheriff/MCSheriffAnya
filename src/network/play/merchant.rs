// Окно торговли с жителем для клиента Java: щелчок по жителю, Open Screen,
// Merchant Offers, выбор сделки, щелчки по ячейкам, закрытие.
//
// Порядок и содержимое пакетов сняты с оригинала чёрным ящиком (сценарии
// villager-trade, villager-levelup, villager-shake): окно 19 с заголовком
// `entity.minecraft.villager.<профессия>`, содержимое окна, список сделок;
// выбор сделки перекладывает оплату из инвентаря (всю стопку), ячейки
// приходят по порядку номеров; сделка — звук «да» (не чаще раза в 20
// тактов), шарик опыта у жителя, ячейки; закрытие — курсор, потом оплата
// возвращаются в инвентарь. Правила сделок — src/villager.rs.
//
// Что не сверено, помечено «[проверить]».

use super::*;
use crate::villager::{self, Merchant};

/// Serverbound Select Trade (minecraft-data 26.1: 0x33, `select_trade`).
pub(super) const SELECT_TRADE: i32 = 0x33;

/// Clientbound Merchant Offers (minecraft-data 26.1: 0x34, `trade_list`).
const MERCHANT_OFFERS: i32 = 0x34;

/// Вид меню «торговец» (чёрный ящик, villager-trade: окно 19).
const MERCHANT_MENU: i32 = 19;

/// Ячеек жителя в окне: две оплаты и выход; дальше рюкзак и панель.
const SLOTS: usize = 3;
const RESULT: usize = 2;

/// Где в окне рюкзак и панель (окно торговли: 3–29 и 30–38).
const MAIN: std::ops::Range<usize> = 3..30;
const HOTBAR: std::ops::Range<usize> = 30..39;

/// Щелчок по жителю: детёныш, безработный, бестолковый и житель без сделок
/// мотают головой со звуком «нет» (чёрный ящик, villager-shake), прочие
/// открывают окно торговли. Возвращает номер жителя, чьё окно открыть.
pub(super) fn use_villager(items: &mut crate::items::Items, index: usize, player_id: i32, uuid: [u8; 16], shared: &Shared) -> Option<i32> {
    let mob = &mut items.mobs_mut()[index];

    let wandering = mob.kind == crate::mob::Kind::WanderingTrader;
    if wandering && mob.villager.offers.is_empty() {
        mob.villager.offers = crate::mob::trader::offers(&mut crate::mob::Rng::new(mob.entity_id as u64 ^ mob.ticks));
    }
    // С жителем уже торгуют [проверить: у оригинала второму отказ молча].
    if mob.villager.trading.is_some() {
        return None;
    }

    // Сделок нет, а профессия есть (призван без Offers): сделки уровней до
    // нынешнего (вики, Trading) [проверить].
    if !wandering && !mob.is_baby() && villager::trades(mob.villager.profession) && mob.villager.offers.is_empty() {
        let mut rng = crate::mob::Rng::new(u64::from(mob.entity_id as u32) ^ mob.ticks.wrapping_mul(0x77));

        for level in 1..=mob.villager.level {
            mob.villager.add_trades(level, &mut rng);
        }
    }

    let opened = if mob.is_baby() || (!wandering && !villager::trades(mob.villager.profession)) || mob.villager.offers.is_empty() {
        mob.villager.unhappy = villager::UNHAPPY_TICKS;
        mob.events.push(crate::mob::Event::Sound { name: "minecraft:entity.villager.no", volume: 1.0, pitch: mob.voice_pitch() });
        None
    } else {
        mob.villager.trading = Some(player_id);

        // Скидка вылечившему (чёрный ящик, zombie-villager-cured-discount:
        // special = −⌊репутация · множитель⌋ у каждой сделки).
        let reputation = mob.villager.gossip.iter().find(|(who, _)| *who == uuid).map_or(0, |(_, value)| *value);

        if reputation != 0 {
            for offer in &mut mob.villager.offers {
                offer.special = -((reputation as f32 * offer.multiplier).floor() as i32);
            }
        }

        Some(mob.entity_id)
    };

    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(player_id as u32)));
    opened
}

/// Открывает окно торговли: Open Screen, содержимое окна, Merchant Offers
/// (чёрный ящик, villager-trade: в этом порядке).
pub(super) async fn open_merchant_window(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, villager_id: i32) -> io::Result<()> {
    let found = shared
        .items_in(player.dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .mobs()
        .iter()
        .find(|mob| mob.entity_id == villager_id)
        .map(|mob| (mob.villager.clone(), mob.kind == crate::mob::Kind::WanderingTrader));

    let Some((state, wandering)) = found else {
        return Ok(());
    };

    player.window_counter = player.window_counter % WINDOW_IDS + 1;

    let id = player.window_counter;
    let mut body = encode_varint(OPEN_SCREEN);

    body.extend_from_slice(&encode_varint(id));
    body.extend_from_slice(&encode_varint(MERCHANT_MENU));
    body.extend_from_slice(&nbt::translated(&if wandering { "entity.minecraft.wandering_trader".to_string() } else { state.title() }));
    write_body(stream, body).await?;

    player.window = Some(OpenWindow {
        id,
        halves: Vec::new(),
        cart: None,
        shown: vec![None; SLOTS],
        lectern: false,
        crafter: false,
        kind: None,
        props: Vec::new(),
        merchant: Some(Merchant::new(villager_id)),
        station: None,
        ender: false,
    });
    send_window_content(stream, player).await?;
    write_body(stream, offers_packet(id, &state, wandering)).await
}

/// Merchant Offers (minecraft-data 26.1, `trade_list`): сделки с ценой без
/// поправок (её считает клиент по спросу и особой цене), уровень и опыт
/// жителя, «обычный житель» и «пополняется».
fn offers_packet(window: i32, state: &villager::State, wandering: bool) -> Vec<u8> {
    let mut body = encode_varint(MERCHANT_OFFERS);

    body.extend_from_slice(&encode_varint(window));
    body.extend_from_slice(&encode_varint(state.offers.len() as i32));

    for offer in &state.offers {
        body.extend_from_slice(&encode_varint(offer.buy.item));
        body.extend_from_slice(&encode_varint(offer.buy.count));
        let (count,components)=crate::stackdata::encode_components(&offer.buy.extras());
        body.extend_from_slice(&encode_varint(count)); body.extend_from_slice(&components);
        push_slot(&mut body, Some(offer.sell));

        match offer.buy_b {
            Some(extra) => {
                body.push(1);
                body.extend_from_slice(&encode_varint(extra.item));
                body.extend_from_slice(&encode_varint(extra.count));
                let (count,components)=crate::stackdata::encode_components(&extra.extras());
                body.extend_from_slice(&encode_varint(count)); body.extend_from_slice(&components);
            }
            None => body.push(0),
        }

        body.push(u8::from(offer.out_of_stock()));

        for value in [offer.uses, offer.max_uses, offer.xp, offer.special] {
            body.extend_from_slice(&value.to_be_bytes());
        }

        body.extend_from_slice(&offer.multiplier.to_be_bytes());
        body.extend_from_slice(&offer.demand.to_be_bytes());
    }

    body.extend_from_slice(&encode_varint(i32::from(state.level)));
    body.extend_from_slice(&encode_varint(state.xp));
    body.push(u8::from(!wandering));
    body.push(u8::from(!wandering));
    body
}

/// Ячейки окна целиком: житель (3), рюкзак, панель.
fn snapshot(merchant: &Merchant, inventory: &Inventory) -> Vec<Option<Stack>> {
    let mut all = merchant.slots.to_vec();

    for own in (inventory::FIRST_MAIN..=inventory::LAST_MAIN).chain(inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR) {
        all.push(inventory.slot(own));
    }

    all
}

/// Окно торговли игрока, если открыто оно.
fn merchant_of(player: &PlayerState) -> Option<Merchant> {
    player.window.as_ref().and_then(|window| window.merchant.clone())
}

fn store(player: &mut PlayerState, merchant: Merchant) {
    if let Some(window) = player.window.as_mut() {
        window.shown = merchant.slots.to_vec();
        window.merchant = Some(merchant);
    }
}

/// Оплата сменилась: выход пересчитывается, житель говорит «да» (есть что
/// отдать) или «нет» — не чаще раза в 20 тактов (чёрный ящик,
/// villager-trade, villager-levelup) [проверить: «нет»].
fn payments_changed(items: &mut crate::items::Items, merchant: &mut Merchant) {
    let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == merchant.villager) else {
        merchant.slots[RESULT] = None;
        return;
    };

    merchant.update_result(&mob.villager.offers);

    if crate::mob::ai::trade_sound_due(mob) {
        let name = match (mob.kind == crate::mob::Kind::WanderingTrader, merchant.slots[RESULT].is_some()) {
            (true,true) => "minecraft:entity.wandering_trader.yes", (true,false) => "minecraft:entity.wandering_trader.no",
            (false,true) => "minecraft:entity.villager.yes", (false,false) => "minecraft:entity.villager.no",
        };

        mob.events.push(crate::mob::Event::Sound { name, volume: 1.0, pitch: mob.voice_pitch() });
    }
}

/// Сделка: оплата снята, житель получил опыт, игрок — шарик опыта у жителя
/// (чёрный ящик, villager-trade: шарик на высоте +0,5 с толчком, как у
/// выпавшего из моба). Возвращает, состоялась ли.
fn trade(shared: &Shared, items: &mut crate::items::Items, merchant: &mut Merchant) -> bool {
    let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == merchant.villager) else {
        return false;
    };

    let Some(index) = villager::pay(merchant, &mut mob.villager.offers) else {
        return false;
    };

    let villager = merchant.villager;

    // Сперва выход по новой оплате (звук «да», если пора), потом сброс
    // счёта голоса самой сделкой (чёрный ящик, villager-trade).
    settle(shared, items, villager, index, |items| payments_changed(items, merchant));
    true
}

/// Всё, что следует за оплаченной сделкой `index`: опыт жителю, отсчёт до
/// повышения, частицы, звук (`sound` — его решает вызывающий: у Java он
/// зависит от ячеек окна), шарик опыта игроку. Общее для Java (`trade`) и
/// Bedrock (`trade_paid`): оплату и счётчик использований вызывающий уже
/// списал.
fn settle(shared: &Shared, items: &mut crate::items::Items, villager_id: i32, index: usize, sound: impl FnOnce(&mut crate::items::Items)) {
    let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == villager_id) else {
        return;
    };

    let offer = mob.villager.offers[index].clone();

    mob.villager.xp += offer.xp;

    let levels_up = mob.kind != crate::mob::Kind::WanderingTrader && villager::should_level_up(mob.villager.level, mob.villager.xp);

    // Повышение — через 40 тактов без торговли (чёрный ящик,
    // villager-levelup-ai); у NoAI таймер стоит (villager-levelup).
    mob.villager.level_timer = villager::LEVEL_UP_DELAY;
    mob.villager.level_pending |= levels_up;

    // У жителя с ИИ сделка — частицы «доволен» (чёрный ящик,
    // villager-levelup-ai: событие 14 в такте сделки; у NoAI его нет) [проверить].
    if !mob.no_ai {
        mob.events.push(crate::mob::Event::Status(14));
    }

    let at = (mob.x, mob.y + 0.5, mob.z);
    let mut rng = crate::mob::Rng::new(u64::from(mob.entity_id as u32) ^ mob.ticks.wrapping_mul(0x9E37) ^ (offer.uses as u64) << 32);

    sound(items);

    if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == villager_id) {
        crate::mob::ai::hush(mob);
    }

    items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);

    if offer.reward_exp {
        let value = villager::reward(&mut rng, levels_up);
        let velocity = ((rng.unit() * 0.2 - 0.1) * 2.0, rng.unit() * 0.2 * 2.0, (rng.unit() * 0.2 - 0.1) * 2.0);

        items.add_orb(shared.next_entity_id(), value, at, velocity);
    }
}

/// Сделка Bedrock: оплату списал сам запрос, здесь — счётчик использований
/// и общий хвост (`settle`). Звук «да» — не чаще раза в 20 тактов (BDS:
/// `HaggleYes` при каждой сделке). Возвращает, есть ли такой житель и такая
/// сделка.
pub(crate) fn trade_paid(shared: &Shared, dimension: crate::dimension::Dimension, villager_id: i32, index: usize) -> bool {
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    let Some(offer) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == villager_id).and_then(|mob| mob.villager.offers.get_mut(index)) else {
        return false;
    };

    offer.uses += 1;

    settle(shared, &mut items, villager_id, index, |items| {
        if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == villager_id)
            && crate::mob::ai::trade_sound_due(mob)
        {
            mob.events.push(crate::mob::Event::Sound { name: "minecraft:entity.villager.yes", volume: 1.0, pitch: mob.voice_pitch() });
        }
    });

    true
}

/// Select Trade: выбор сделки. Оплата из ячеек уходит в инвентарь (с конца
/// панели, как Shift-щелчок), потом в ячейки ложится всё нужное из
/// инвентаря — до полной стопки (чёрный ящик, villager-trade: 64 пшеницы).
pub(super) fn select_trade(shared: &Shared, player: &mut PlayerState, payload: &[u8], reaction: &mut Reaction) {
    let Some(index) = Cursor::new(payload).varint() else {
        return;
    };
    let Some(mut merchant) = merchant_of(player) else {
        return;
    };

    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
    let Some(offer) = items.mobs().iter().find(|mob| mob.entity_id == merchant.villager).and_then(|mob| mob.villager.offers.get(usize::try_from(index).ok()?).cloned()) else {
        return;
    };

    let before = snapshot(&merchant, &player.inventory);

    merchant.selected = index as usize;

    let result = merchant.slots[RESULT].take();
    let targets: Vec<usize> = (SLOTS..SLOTS + window::PLAYER_SLOTS).rev().collect();
    let mut window = window::Window { container: &mut merchant.slots, inventory: &mut player.inventory, kind: None, horse: None };
    let cleared = window.move_to(0, &targets) && window.move_to(1, &targets);

    if cleared {
        for (slot, want) in [(0, Some(offer.buy)), (1, offer.buy_b)] {
            let Some(want) = want else {
                continue;
            };

            for from in SLOTS..SLOTS + window::PLAYER_SLOTS {
                let have = window.get(slot).map_or(0, |stack| stack.count);
                let Some(there) = window.get(from).filter(|there| there.stacks_with(&want)) else {
                    continue;
                };

                let moved = (there.limit() - have).min(there.count);

                if moved <= 0 {
                    break;
                }

                window.set(slot, Some(there.with_count(have + moved)));
                window.set(from, inventory::remainder(there, there.count - moved));
            }
        }
    }

    merchant.slots[RESULT] = result;

    if merchant.slots[..RESULT] != before[..RESULT] {
        payments_changed(&mut items, &mut merchant);
        items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(merchant.villager as u32)));
    }

    drop(items);

    let after = snapshot(&merchant, &player.inventory);

    store(player, merchant);

    for (slot, stack) in after.iter().enumerate() {
        if *stack != before[slot] {
            reaction.window_slots.push((slot, *stack));
        }
    }
}

/// Щелчок в окне торговли. Выход берётся только целиком и только как
/// сделка; Shift — сделки подряд, пока товар влезает в инвентарь;
/// Shift из инвентаря — между рюкзаком и панелью (вики, Trading) [проверить].
/// Ответ — ячейки, что клиент предсказал не так, и курсор.
pub(super) fn click(
    shared: &Shared,
    player: &mut PlayerState,
    (slot, button, mode): (i32, i8, i32),
    predicted: &[(usize, Option<(i32, i32)>)],
    predicted_cursor: Option<Option<(i32, i32)>>,
    readable: bool,
    reaction: &mut Reaction,
) {
    let Some(mut merchant) = merchant_of(player) else {
        return;
    };

    let before = snapshot(&merchant, &player.inventory);
    let mut thrown = None;

    {
        let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

        if slot == RESULT as i32 {
            take_result(shared, &mut items, player, &mut merchant, mode);
        } else {
            // Выход щелчок по прочим ячейкам не трогает.
            let result = merchant.slots[RESULT].take();
            let payments = (merchant.slots[0], merchant.slots[1]);
            let mut window = window::Window { container: &mut merchant.slots, inventory: &mut player.inventory, kind: None, horse: None };
            let index = usize::try_from(slot).ok();

            match (mode, index) {
                (1, Some(index)) if MAIN.contains(&index) => {
                    window.move_to(index, &HOTBAR.collect::<Vec<_>>());
                }
                (1, Some(index)) if HOTBAR.contains(&index) => {
                    window.move_to(index, &MAIN.collect::<Vec<_>>());
                }
                _ => thrown = window.click(slot, button, mode).thrown,
            }

            merchant.slots[RESULT] = result;

            if (merchant.slots[0], merchant.slots[1]) != payments {
                payments_changed(&mut items, &mut merchant);
            }
        }

        items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(merchant.villager as u32)));
    }

    if let Some(stack) = thrown {
        throw_out(shared, player, stack);
    }

    let after = snapshot(&merchant, &player.inventory);

    store(player, merchant);

    // Чего клиент ждёт в каждой ячейке: предсказанное — там, где он его
    // прислал, прежнее — в остальных (как у хранилищ, click_window).
    let as_pair = |stack: Option<Stack>| stack.map(|stack| (stack.item, stack.count));

    for (index, stack) in after.iter().enumerate() {
        let expected = predicted.iter().rev().find(|(place, _)| *place == index).map_or(as_pair(before[index]), |(_, item)| *item);

        if !readable || expected != as_pair(*stack) {
            reaction.window_slots.push((index, *stack));
        }
    }

    if !readable || predicted_cursor != Some(as_pair(player.inventory.cursor())) {
        reaction.cursor = Some(player.inventory.cursor());
    }
}

/// Щелчок по выходу: обычный — товар в курсор (пустой или с тем же
/// товаром, если влезет), Shift — товар в инвентарь, сделка за сделкой
/// (чёрный ящик, villager-trade: три сделки одним щелчком).
fn take_result(shared: &Shared, items: &mut crate::items::Items, player: &mut PlayerState, merchant: &mut Merchant, mode: i32) {
    let Some(result) = merchant.slots[RESULT] else {
        return;
    };

    match mode {
        0 => {
            let cursor = player.inventory.cursor();
            let fits = match cursor {
                None => true,
                Some(held) => held.stacks_with(&result) && held.count + result.count <= held.limit(),
            };

            if fits && trade(shared, items, merchant) {
                player.progress.traded();
                if player.dimension == Dimension::Overworld && player.y >= 319.0 {
                    player.progress.trigger("adventure/trade_at_world_height");
                }
                player.inventory.set_cursor(Some(result.with_count(cursor.map_or(0, |held| held.count) + result.count)));
            }
        }
        1 => {
            let targets: Vec<usize> = (SLOTS..SLOTS + window::PLAYER_SLOTS).rev().collect();

            // Не больше стопки сделок за щелчок — от зацикливания.
            for _ in 0..64 {
                let Some(now) = merchant.slots[RESULT] else {
                    break;
                };

                if !now.stacks_with(&result) {
                    break;
                }

                let saved = player.inventory.clone();
                let mut window = window::Window { container: &mut merchant.slots, inventory: &mut player.inventory, kind: None, horse: None };

                if !window.move_to(RESULT, &targets) {
                    player.inventory = saved;
                    merchant.slots[RESULT] = Some(now);
                    break;
                }

                merchant.slots[RESULT] = Some(now);

                if !trade(shared, items, merchant) {
                    player.inventory = saved;
                    break;
                }

                player.progress.traded();
                if player.dimension == Dimension::Overworld && player.y >= 319.0 {
                    player.progress.trigger("adventure/trade_at_world_height");
                }
            }
        }
        _ => {}
    }
}

/// Окно торговли закрыто: курсор, потом оплата — в инвентарь (чёрный ящик,
/// villager-trade: хлеб из курсора в 36, изумруды из оплаты в 37), житель
/// свободен. Возвращает изменившиеся ячейки инвентаря и не влезшее.
pub(super) fn close(shared: &Shared, player: &mut PlayerState) -> (Vec<i32>, Vec<Stack>) {
    let Some(merchant) = merchant_of(player) else {
        return (Vec::new(), Vec::new());
    };

    let mut changed = Vec::new();
    let mut thrown = Vec::new();
    let returned = player.inventory.cursor().into_iter().chain(merchant.slots[0]).chain(merchant.slots[1]);

    player.inventory.set_cursor(None);

    for stack in returned.collect::<Vec<_>>() {
        let (slots, left) = player.inventory.put(stack);

        changed.extend(slots);
        thrown.extend(left);
    }

    if let Some(window) = player.window.as_mut() {
        window.merchant = Some(Merchant::new(merchant.villager));
    }

    release(shared, player.dimension, merchant.villager);
    changed.sort_unstable();
    changed.dedup();
    (changed, thrown)
}

/// Житель больше ни с кем не торгует.
pub(super) fn release(shared: &Shared, dimension: crate::dimension::Dimension, villager_id: i32) {
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == villager_id) {
        mob.villager.trading = None;
    }
}

/// Житель с открытым окном пропал или умер — окно закрывается (вики,
/// Trading: «If the merchant takes damage or leaves the player's reach, the
/// trading menu closes») [проверить: удар и дальность].
pub(super) fn villager_gone(shared: &Shared, player: &PlayerState, villager_id: i32) -> bool {
    !shared
        .items_in(player.dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .mobs()
        .iter()
        .any(|mob| mob.entity_id == villager_id && !mob.is_dead())
}
