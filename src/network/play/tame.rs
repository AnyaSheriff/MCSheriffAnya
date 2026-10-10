// Щелчки по мобам и заборам: бирка, поводок, приручение волка и кошки
// (mob/leash.rs, mob/tame.rs). Чёрный ящик: name-tag, lead-attach,
// wolf-tame, cat-tame, wolf-sit-follow.

use super::*;
use crate::mob::leash::{self, Holder};
use crate::mob::{tame, Kind};

/// Щелчок по мобу `index` с предметом в руке: бирка, поводок, приручение,
/// приказ. Возвращает, обработан ли щелчок (тогда дальше не идёт).
pub(super) fn use_on_mob(items: &mut items::Items, index: usize, player: &mut PlayerState, shared: &Shared, reaction: &mut Reaction) -> bool {
    let held_stack = player.inventory.held();
    let held = held_stack.and_then(|stack| blocks::item_name(stack.item));
    let survival = player.game_mode != GAME_MODE_CREATIVE;
    let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ items.mobs()[index].ticks.wrapping_mul(0x9E37_79B9));
    let spend = |player: &mut PlayerState, reaction: &mut Reaction| {
        if let Some(stack) = held_stack { player.progress.used(stack.item); }
        if survival && let Some(slot) = player.inventory.spend_held() {
            reaction.slots.push(slot);
        }
    };

    // Бирка: имя мобу (вики, Name Tag; чёрный ящик, name-tag).
    if held == Some("name_tag") {
        let name = held_stack.and_then(|stack| stack.extras().name);

        if leash::name_tag(&mut items.mobs_mut()[index], name.as_deref()) {
            spend(player, reaction);
        }

        return true;
    }

    // Поводок: снять свой — поводок падает (вики, Lead: «right-clicking the
    // leashed mob again detaches the lead») [проверить: отдаётся в руку?].
    if let Some(Holder::Player { id, .. }) = items.mobs()[index].leash
        && id == player.entity_id
    {
        leash::drop_leash(&mut items.mobs_mut()[index], survival);
        flush(items, shared, &mut rng);
        return true;
    }

    // Поводок в руке: взять моба (чёрный ящик, lead-attach: связь в такте
    // щелчка, звука нет).
    if held == Some("lead") && leash::leashable(items.mobs()[index].kind) && items.mobs()[index].leash.is_none() {
        leash::attach(&mut items.mobs_mut()[index], Holder::Player { id: player.entity_id, uuid: player.uuid });
        flush(items, shared, &mut rng);
        spend(player, reaction);
        return true;
    }

    // Волк и кошка: приручение, лечение, ошейник, приказ.
    if matches!(items.mobs()[index].kind, Kind::Wolf | Kind::Cat) {
        let was_in_love = items.mobs()[index].love > 0;
        let was_tamed = items.mobs()[index].tame.tamed();
        let used = tame::interact(&mut items.mobs_mut()[index], (player.uuid, player.entity_id), held, &mut rng);

        if !was_in_love && items.mobs()[index].love > 0 { items.mobs_mut()[index].love_player = Some(player.uuid); }
        let mob = &items.mobs()[index];
        if !was_tamed && mob.tame.tamed() {
            let variants = if mob.kind == Kind::Wolf { &tame::WOLF_VARIANTS[..] } else { &tame::CAT_VARIANTS[..] };
            if let Some(variant) = variants.get(usize::from(mob.tame.variant)) {
                player.progress.tamed(mob.kind.name(), variant);
            }
        }
        flush(items, shared, &mut rng);

        match used {
            tame::Used::Spent => spend(player, reaction),
            tame::Used::Kept => {}
            tame::Used::Pass => return false,
        }

        return true;
    }

    false
}

fn flush(items: &mut items::Items, shared: &Shared, rng: &mut crate::mob::Rng) {
    items.flush_mob_events(&mut || shared.next_entity_id(), rng);
}

/// Щелчок по забору: все мобы на поводке игрока в 7 блоках переходят на узел
/// (вики, Lead: «Using a lead on a fence … attaches all leashed mobs»; чёрный
/// ящик, lead-attach: узел появляется, звук item.lead.tied в точке узла,
/// повтор блока и над ним). Без таких мобов щелчок идёт дальше.
pub(super) fn use_fence(world: &mut World, shared: &Shared, player: &PlayerState, clicked: (i32, i32, i32), face: i32, sequence: i32) -> Option<Reaction> {
    let name = blocks::block_at_state(world.get_block(clicked.0, clicked.1, clicked.2))?;

    if !name.ends_with("_fence") {
        return None;
    }

    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
    let near = |mob: &crate::mob::Mob| (mob.x - player.x).powi(2) + (mob.y - player.y).powi(2) + (mob.z - player.z).powi(2) <= leash::KNOT_REACH * leash::KNOT_REACH;
    let held: Vec<usize> = items
        .mobs()
        .iter()
        .enumerate()
        .filter(|(_, mob)| matches!(mob.leash, Some(Holder::Player { id, .. }) if id == player.entity_id) && near(mob))
        .map(|(index, _)| index)
        .collect();

    if held.is_empty() {
        return None;
    }

    // Узел на этом заборе — уже есть или новый.
    let knot = match items.mobs().iter().find(|mob| mob.kind == Kind::LeashKnot && leash::knot_block(mob) == clicked && !mob.h.despawn) {
        Some(knot) => knot.entity_id,
        None => {
            let id = shared.next_entity_id();

            items.add_mob(leash::new_knot(id, clicked));
            id
        }
    };

    for index in held {
        leash::attach(&mut items.mobs_mut()[index], Holder::Knot { id: knot, at: clicked });
    }

    let mut rng = crate::mob::Rng::new(u64::from(knot as u32));

    items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);
    drop(items);

    world.note_sound_neutral_at(leash::knot_point(clicked), leash::TIED, 1.0, 1.0);

    let (bx, by, bz) = offset_towards(clicked.0, clicked.1, clicked.2, face);

    world.tell_directly(clicked.0, clicked.1, clicked.2);
    world.tell_directly(bx, by, bz);

    Some(Reaction::accepted(sequence))
}
