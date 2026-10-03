// Торговля с жителем для игроков Bedrock: пакет Update Trade, ячейки окна,
// метка «торгует с игроком» у жителя и сверка сделки запроса.
//
// Все правила — общие с Java (villager.rs: сделки, цены, запас, спрос, опыт;
// network/play/merchant.rs: звук, опыт жителю и игроку); здесь только то, как
// это выглядит в протоколе Bedrock. Снято с официального BDS 1.26.13.1
// (запись tmp/br-trade/bds-*.jsonl, `summon villager … minecraft:become_librarian`):
//
// - щелчок по жителю: Update Trade (окно, вид 15 «trading», `size` 0, ярус,
//   номер жителя и «1» вместо игрока — оба varint64 с зигзагом, ключ
//   перевода `entity.villager.librarian`, два флага «новый экран» и «спрос»,
//   сетевой NBT с `Recipes` и `TierExpRequirements`), Inventory Content окна
//   (три пустые ячейки), Set Actor Data жителя `trading_player_eid` (ключ 68);
//   Container Open не шлётся вовсе;
// - каждая сделка — один Item Stack Request: `take` оплаты из рюкзака в ячейку
//   `trade2_ingredient1` (47, ячейка 4) и `trade2_ingredient2` (48, ячейка 5),
//   потом `craft_recipe` с `netId` сделки, `results_deprecated` с итогом,
//   `consume` оплаты и `take` из `creative_output` (ячейка 50, номер стопки —
//   номер запроса) в курсор или рюкзак. Пустой `results_deprecated` у BDS
//   роняет сервер, неверный итог — отказ;
// - после сделки сервер шлёт шарики опыта, звук `HaggleYes` и Set Actor Data
//   жителя с `trade_experience` (ключ 103); на десятом очке опыта у новичка —
//   `trade_tier` 1 сразу; Update Trade заново не шлётся;
// - закрытие: сервер отвечает только `trading_player_eid` = 0.
//
// Что не сверено, помечено «[проверить]».

use super::codec::Out;
use super::mobdata::{self, Value};
use super::session;
use crate::inventory::Stack;
use crate::villager::{self, Offer};

/// Вид окна «торговля» (WindowType 15, minecraft-data).
pub const WINDOW_TYPE: i8 = 15;

/// Ключ данных жителя «с кем торгует» (long, ключ 68).
const KEY_TRADING_PLAYER: u32 = 68;

/// Первый `netId` сделок: сделки одного окна идут подряд.
const NET_ID_BASE: u32 = 3000;

/// Номер сделки в запросе клиента.
pub fn net_id(index: usize) -> u32 {
    NET_ID_BASE + index as u32
}

/// Что окно знает о сделке для сверки запроса: оплата (с ценой по спросу),
/// итог и есть ли ещё запас.
#[derive(Clone, Debug, PartialEq)]
pub struct Price {
    pub net_id: u32,
    pub a: (i32, i32),
    pub b: Option<(i32, i32)>,
    pub sell: Stack,
    pub stocked: bool,
}

impl Price {
    pub fn of(index: usize, offer: &Offer) -> Self {
        Self {
            net_id: net_id(index),
            a: (offer.buy.item, offer.cost_a()),
            b: offer.buy_b.map(|stack| (stack.item, stack.count)),
            sell: offer.sell,
            stocked: !offer.out_of_stock(),
        }
    }

    /// Хватает ли оплаты в двух ячейках — в этом порядке или наоборот
    /// (как `Merchant::matching` у Java).
    pub fn paid_by(&self, first: Option<Stack>, second: Option<Stack>) -> bool {
        let fits = |want: Option<(i32, i32)>, have: Option<Stack>| match (want, have) {
            (None, _) => true,
            (Some((item, count)), Some(have)) => have.item == item && have.count >= count,
            (Some(_), None) => false,
        };

        (fits(Some(self.a), first) && fits(self.b, second)) || (fits(Some(self.a), second) && fits(self.b, first))
    }

    /// Израсходовано ли ровно столько, сколько стоит сделка, и только из
    /// ячеек интерфейса: (предмет, сколько, из интерфейса ли).
    pub fn covers(&self, consumed: &[(i32, i32, bool)]) -> bool {
        if consumed.iter().any(|&(_, _, in_ui)| !in_ui) {
            return false;
        }

        let mut want: Vec<(i32, i32)> = vec![self.a];

        match (self.b, want.iter_mut().find(|(item, _)| Some(*item) == self.b.map(|b| b.0))) {
            (Some((_, count)), Some(same)) => same.1 += count,
            (Some(extra), None) => want.push(extra),
            (None, _) => {}
        }

        want.iter().all(|&(item, count)| consumed.iter().filter(|c| c.0 == item).map(|c| c.1).sum::<i32>() == count)
            && consumed.iter().all(|c| want.iter().any(|w| w.0 == c.0))
    }
}

/// Сделки жителя для окна.
pub fn prices(state: &villager::State) -> Vec<Price> {
    state.offers.iter().enumerate().map(|(index, offer)| Price::of(index, offer)).collect()
}

// Теги сетевого NBT Bedrock: имя — строка с длиной varint, int — зигзаг,
// short и float — little-endian.
const BYTE: u8 = 1;
const SHORT: u8 = 2;
const INT: u8 = 3;
const FLOAT: u8 = 5;
const STRING: u8 = 8;
const LIST: u8 = 9;
const COMPOUND: u8 = 10;

/// Предмет сделки: `{Count, Damage, Name, WasPickedUp}` и у итога с
/// зачарованиями — `tag:{ench:[{id,lvl}]}`. `any` — «любые данные» (32767),
/// как у оплаты в записи BDS. None — предмета нет у Bedrock.
fn item_tag(out: &mut Out, name: &str, stack: Stack, any: bool) -> Option<()> {
    let (id, metadata, _) = super::inventory::bedrock_item(stack.item)?;
    let item = super::frames::bedrock_item_name(id)?;

    out.u8(COMPOUND).string(name);
    out.u8(BYTE).string("Count").u8(stack.count.clamp(0, 255) as u8);
    out.u8(SHORT).string("Damage").li16(if any { 0x7fff } else { metadata as i16 });
    out.u8(STRING).string("Name").string(&format!("minecraft:{item}"));
    out.u8(BYTE).string("WasPickedUp").u8(0);

    let data = stack.extras();
    let enchants: Vec<(u8, i32)> = data
        .enchants
        .iter()
        .chain(&data.stored)
        .filter_map(|&(number, level)| Some((super::stations::bedrock_enchantment(crate::stackdata::ENCHANTMENTS.get(usize::from(number))?.0)?, level)))
        .collect();

    // Зачарованная книга у Bedrock — тоже `ench` (запись BDS, сделка
    // библиотекаря: `tag:{ench:[{id:9,lvl:1}]}`).
    if !any && !enchants.is_empty() {
        out.u8(COMPOUND).string("tag");
        out.u8(LIST).string("ench").u8(COMPOUND).zigzag32(enchants.len() as i32);

        for (id, level) in enchants {
            out.u8(SHORT).string("id").li16(i16::from(id));
            out.u8(SHORT).string("lvl").li16(level as i16);
            out.u8(0);
        }

        out.u8(0);
    }

    out.u8(0);
    Some(())
}

/// Сделка как NBT-компаунд (ключи — в порядке записи BDS). None — оплаты или
/// итога нет у Bedrock: такую сделку клиенту не показать.
fn offer_tag(out: &mut Out, index: usize, offer: &Offer, tier: i32) -> Option<()> {
    // Составляем отдельно: неудавшаяся сделка не оставляет обрывка в пакете.
    let mut part = Out::default();

    item_tag(&mut part, "buyA", offer.buy, true)?;

    if let Some(extra) = offer.buy_b {
        item_tag(&mut part, "buyB", extra, true)?;
    }

    part.u8(INT).string("buyCountA").zigzag32(offer.buy.count);
    part.u8(INT).string("buyCountB").zigzag32(offer.buy_b.map_or(0, |extra| extra.count));
    part.u8(INT).string("demand").zigzag32(offer.demand);
    part.u8(INT).string("maxUses").zigzag32(offer.max_uses);
    part.u8(INT).string("netId").zigzag32(net_id(index) as i32);
    part.u8(FLOAT).string("priceMultiplierA").lf32(offer.multiplier);
    part.u8(FLOAT).string("priceMultiplierB").lf32(if offer.buy_b.is_some() { offer.multiplier } else { 0.0 });
    part.u8(BYTE).string("rewardExp").u8(u8::from(offer.reward_exp));
    item_tag(&mut part, "sell", offer.sell, false)?;
    part.u8(INT).string("tier").zigzag32(tier);
    part.u8(INT).string("traderExp").zigzag32(offer.xp);
    part.u8(INT).string("uses").zigzag32(offer.uses);
    part.u8(0);
    out.raw(&part.bytes);
    Some(())
}

/// Update Trade: окно, житель с его ярусом, список сделок.
pub fn update_packet(window: u8, villager_id: i32, state: &villager::State) -> Vec<u8> {
    let tier = i32::from(state.level.max(1)) - 1;
    let name = villager::PROFESSIONS.get(usize::from(state.profession)).copied().unwrap_or("none");
    let mut out = Out::packet(session::UPDATE_TRADE);

    // BDS пишет `size` нулём; номера жителя и «игрока» — varint64 от зигзага
    // уникального номера (у жителя — он сам, у игрока — −1) [запись BDS].
    out.u8(window).u8(WINDOW_TYPE as u8).varint(0).varint(tier as u32);
    out.varint64(((i64::from(villager_id) << 1) ^ (i64::from(villager_id) >> 63)) as u64).varint64(1);
    out.string(&format!("entity.villager.{name}")).bool(true).bool(true);

    out.u8(COMPOUND).string("");
    out.u8(LIST).string("Recipes").u8(COMPOUND);

    // Список пишем заранее, чтобы знать, сколько сделок достижимо.
    let mut list = Out::default();
    let mut count = 0;

    for (index, offer) in state.offers.iter().enumerate() {
        // Ярус сделки: по две на уровень [проверить: у BDS есть поле `tier`].
        let at = (index as i32 / 2).min(tier);

        if offer_tag(&mut list, index, offer, at).is_some() {
            count += 1;
        }
    }

    out.zigzag32(count).raw(&list.bytes);
    out.u8(LIST).string("TierExpRequirements").u8(COMPOUND).zigzag32(villager::LEVEL_XP.len() as i32);

    for (level, xp) in villager::LEVEL_XP.iter().enumerate() {
        out.u8(INT).string(&level.to_string()).zigzag32(*xp);
        out.u8(0);
    }

    out.u8(0);
    out.bytes
}

/// Inventory Content окна торговли: три пустые ячейки (оплата 1, оплата 2,
/// итог), нулевое имя контейнера — как у BDS.
pub fn content_packet(window: u8) -> Vec<u8> {
    let mut out = Out::packet(session::INVENTORY_CONTENT);

    out.varint(u32::from(window)).varint(3);

    for _ in 0..3 {
        out.zigzag32(0);
    }

    out.u8(super::containers::WINDOW_CONTAINER).u8(0).zigzag32(0);
    out.bytes
}

/// Set Actor Data жителя: с кем торгует (0 — ни с кем).
pub fn trading_player_packet(villager_id: i32, player: i32) -> Vec<u8> {
    mobdata::set_data_packet(villager_id, &[(KEY_TRADING_PLAYER, Value::Long(i64::from(player)))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks;

    fn offer(buy: &str, count: i32, extra: Option<(&str, i32)>, sell: &str) -> Offer {
        let stack = |name: &str, count: i32| Stack::new(blocks::item_named(name).expect("предмет"), count);

        Offer {
            buy: stack(buy, count),
            buy_b: extra.map(|(name, count)| stack(name, count)),
            sell: stack(sell, 1),
            uses: 0,
            max_uses: 16,
            xp: 2,
            reward_exp: true,
            multiplier: 0.05,
            special: 0,
            demand: 0,
        }
    }

    fn state() -> villager::State {
        let mut state = villager::State::new();

        state.profession = villager::profession_named("librarian").expect("профессия");
        state.offers = vec![offer("paper", 24, None, "emerald"), offer("emerald", 7, Some(("book", 1)), "bookshelf")];
        state
    }

    #[test]
    fn the_packet_has_the_bds_header() {
        let packet = update_packet(2, 38, &state());

        // Номер пакета, окно 2, вид 15, size 0, ярус 0.
        assert_eq!(&packet[..5], &[session::UPDATE_TRADE as u8, 2, 15, 0, 0]);

        let text = String::from_utf8_lossy(&packet);

        assert!(text.contains("entity.villager.librarian"));
        assert!(text.contains("Recipes") && text.contains("TierExpRequirements"));
        assert!(text.contains("minecraft:paper") && text.contains("minecraft:emerald"));
    }

    #[test]
    fn the_villager_id_is_zigzag_and_the_player_is_one() {
        let packet = update_packet(2, 38, &state());

        // 38 → зигзаг 76 одним байтом, затем 1.
        assert_eq!(&packet[5..7], &[76, 1]);
    }

    #[test]
    fn prices_follow_demand_and_stock() {
        let mut state = state();

        state.offers[0].demand = 20;
        state.offers[1].uses = 16;

        let list = prices(&state);

        assert_eq!(list[0].a.1, state.offers[0].cost_a());
        assert!(list[0].stocked && !list[1].stocked);
        assert_eq!(list[1].net_id, net_id(1));
    }

    #[test]
    fn payment_may_lie_in_either_cell() {
        let state = state();
        let price = &prices(&state)[1];
        let emeralds = Some(state.offers[1].buy);
        let books = state.offers[1].buy_b;

        assert!(price.paid_by(emeralds, books));
        assert!(price.paid_by(books, emeralds));
        assert!(!price.paid_by(emeralds, None));
    }

    #[test]
    fn a_request_must_consume_exactly_the_price() {
        let state = state();
        let price = &prices(&state)[1];
        let (e, b) = (state.offers[1].buy.item, state.offers[1].buy_b.expect("вторая оплата").item);

        assert!(price.covers(&[(e, 7, true), (b, 1, true)]));
        assert!(!price.covers(&[(e, 6, true), (b, 1, true)]));
        assert!(!price.covers(&[(e, 7, true)]));
        assert!(!price.covers(&[(e, 7, true), (b, 1, false)]));
        assert!(!price.covers(&[(e, 7, true), (b, 1, true), (e, 1, true)]));
    }

    #[test]
    fn the_trading_player_tag_reads_back() {
        let packet = trading_player_packet(38, 12);

        assert_eq!(packet[0], session::SET_ENTITY_DATA as u8);
        // Номер, одна запись, ключ 68, тип long (7), значение 12 зигзагом.
        assert_eq!(&packet[1..6], &[38, 1, 68, 7, 24]);
    }

    #[test]
    fn the_content_has_three_empty_cells() {
        let packet = content_packet(2);

        assert_eq!(&packet[1..6], &[2, 3, 0, 0, 0]);
    }

}
