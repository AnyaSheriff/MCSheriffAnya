// Команда `advancement` (вики, Commands/advancement):
//   advancement grant|revoke <игроки> only <достижение> [<условие>]
//   advancement grant|revoke <игроки> from|until|through <достижение>
//   advancement grant|revoke <игроки> everything
// Сама команда только считает, что выдать, и передаёт игрокам поручение
// (`Order::Advancement`): состояние лежит у подключения игрока.

use crate::advancements::{self, Def};
use crate::commands::{Answer, find_targets, names_of};
use crate::players::Order;
use crate::shared::Shared;

pub(crate) fn advancement(parts: &[&str], source: &crate::commands::Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: advancement grant|revoke <игроки> only|from|until|through|everything […]".to_string());

    let [action @ ("grant" | "revoke"), who, mode, rest @ ..] = parts else {
        return usage();
    };
    let grant = *action == "grant";

    let (ids, criterion): (Vec<&'static Def>, Option<String>) = match (*mode, rest) {
        ("everything", []) => (advancements::TREE.iter().collect(), None),
        ("only", [name]) => match advancements::find(name) {
            Some(def) => (vec![def], None),
            None => return Answer::saying(format!("Неизвестное достижение: {name}")),
        },
        ("only", [name, criterion]) => match advancements::find(name) {
            Some(def) if def.criteria.names().iter().any(|have| have == criterion) => (vec![def], Some((*criterion).to_string())),
            Some(_) => return Answer::saying(format!("У достижения {name} нет условия {criterion}")),
            None => return Answer::saying(format!("Неизвестное достижение: {name}")),
        },
        ("from" | "until" | "through", [name]) => {
            let Some(def) = advancements::find(name) else {
                return Answer::saying(format!("Неизвестное достижение: {name}"));
            };
            let mut list: Vec<&'static Def> = Vec::new();

            // from — само и потомки; until — предки и само; through — предки, само и потомки.
            if matches!(*mode, "until" | "through") {
                list.extend(advancements::ancestors(def.id));
            }

            list.push(def);

            if matches!(*mode, "from" | "through") {
                list.extend(advancements::descendants(def.id));
            }

            (list, None)
        }
        _ => return usage(),
    };

    let targets = find_targets(who, source, shared);

    if targets.is_empty() {
        return Answer::saying("Такого игрока нет на сервере".to_string());
    }

    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

    let mut changed = 0;
    let mut changed_players = 0;
    let mut states = shared.advancement_states.lock().expect("достижения захвачены другим потоком");
    for (uuid, _) in &targets {
        let (state, revision) = states.entry(*uuid).or_insert_with(|| (crate::advancements::AdvState::load(uuid), 0));
        let count = ids.iter().filter(|def| match (grant, &criterion) {
            (true, Some(c)) => state.grant(def.id, c),
            (true, None) => state.grant_all(def.id),
            (false, Some(c)) => state.revoke(def.id, c),
            (false, None) => state.revoke_all(def.id),
        }).count();
        changed += count;
        changed_players += usize::from(count > 0);
        *revision += 1;
        players.order(uuid, Order::Advancement { grant, ids: ids.iter().map(|def| def.id).collect(), criterion: criterion.clone(), revision: *revision });
    }
    let verb = if grant { "Выдано" } else { "Отнято" };
    let mut answer = if shared.catalog().changed_rules() {
        Answer::saying(format!("{verb} достижений: {changed} у игроков: {changed_players}"))
    } else {
        Answer::saying(format!("{verb} достижений: {} у {}", ids.len(), names_of(&targets)))
    };
    if shared.catalog().changed_rules() { answer.result = (changed > 0).then_some(changed as i32); }
    answer
}
