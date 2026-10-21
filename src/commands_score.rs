// Команды табло и всего, что рядом: scoreboard, team, tag, bossbar, trigger,
// teammsg/tm, function, schedule, datapack, return и execute с условиями и
// записью результата (вики: Commands/scoreboard, team, tag, bossbar,
// trigger, teammsg, function, schedule, datapack, return, execute).
//
// Ответы — переводимые тексты оригинала (`commands.scoreboard.…`): клиент
// Java сам переводит их на язык игрока; консоли и Bedrock уходит русский
// текст из таблицы `russian`. Ключи и вид доводов сверены чёрным ящиком
// (сценарии score-*, team-basic, bossbar-basic, tag-selectors,
// trigger-teammsg, execute-score).

use serde_json::Value;

use crate::commands::{Answer, Effect, Source};
use crate::scoreboard::{self, literal, translate, BossBar, NumberFormat, Objective, Scoreboard, Team};
use crate::shared::Shared;

/// Что вышло у команды: перевод для Java, текст для консоли, итог.
struct Out {
    text: Value,
    result: Option<i32>,
    /// Ещё сообщения следом (строки `players list <держатель>`).
    more: Vec<Value>,
}

impl Out {
    fn ok(key: &str, with: Vec<Value>, result: i32) -> Self {
        Self { text: translate(key, with), result: Some(result), more: Vec::new() }
    }

    /// Отказ: красным, как у оригинала.
    /// Отказ: `{"text":"","color":"red","extra":[перевод]}`, как у оригинала.
    fn fail(key: &str, with: Vec<Value>) -> Self {
        Self { text: serde_json::json!({"text": "", "color": "red", "extra": [translate(key, with)]}), result: None, more: Vec::new() }
    }

    fn answer(self) -> Answer {
        let mut reply = scoreboard::plain_text(&self.text);
        let mut rich = vec![scoreboard::encode_component(&self.text)];
        for line in &self.more {
            reply.push('\n');
            reply.push_str(&scoreboard::plain_text(line));
            rich.push(scoreboard::encode_component(line));
        }
        Answer { reply, effect: Effect::None, component: None, rich, result: self.result }
    }
}

/// Ответ без слов (успех).
fn silent(result: i32) -> Answer {
    Answer { reply: String::new(), effect: Effect::None, component: None, rich: Vec::new(), result: Some(result) }
}

/// Разбор не удался: общий отказ оригинала («Incorrect argument…»).
fn bad(what: &str) -> Out {
    Out::fail("command.unknown.argument", vec![literal(what)])
}

/// Русский текст ответа по ключу перевода (для консоли и Bedrock).
pub fn russian(key: &str, args: &[String]) -> String {
    let pattern = match key {
        "chat.square_brackets" => "[%s]",
        "command.unknown.argument" => "Неверный довод команды: %s",
        "commands.scoreboard.objectives.add.success" => "Создана цель %s",
        "commands.scoreboard.objectives.add.duplicate" => "Цель с таким именем уже есть",
        "commands.scoreboard.objectives.remove.success" => "Удалена цель %s",
        "commands.scoreboard.objectives.list.empty" => "Целей нет",
        "commands.scoreboard.objectives.list.success" => "Целей: %s — %s",
        "commands.scoreboard.objectives.display.alreadyEmpty" => "Ничего не изменилось: место показа и так пусто",
        "commands.scoreboard.objectives.display.alreadySet" => "Ничего не изменилось: в этом месте уже показана эта цель",
        "commands.scoreboard.objectives.display.cleared" => "Место показа %s очищено",
        "commands.scoreboard.objectives.display.set" => "В месте показа %s теперь цель %s",
        "commands.scoreboard.objectives.modify.displayname" => "Название цели %s изменено на %s",
        "commands.scoreboard.objectives.modify.rendertype" => "Изменён вид показа цели %s",
        "commands.scoreboard.objectives.modify.objectiveFormat.set" => "Изменён вид чисел цели %s",
        "commands.scoreboard.objectives.modify.objectiveFormat.clear" => "Вид чисел цели %s сброшен",
        "commands.scoreboard.objectives.modify.displayAutoUpdate.enable" => "Самообновление показа цели %s включено",
        "commands.scoreboard.objectives.modify.displayAutoUpdate.disable" => "Самообновление показа цели %s выключено",
        "commands.scoreboard.players.set.success.single" => "Счёт %s у %s теперь %s",
        "commands.scoreboard.players.set.success.multiple" => "Счёт %s у %s сущностей теперь %s",
        "commands.scoreboard.players.add.success.single" => "Прибавлено %s к %s у %s (теперь %s)",
        "commands.scoreboard.players.add.success.multiple" => "Прибавлено %s к %s у %s сущностей",
        "commands.scoreboard.players.remove.success.single" => "Отнято %s от %s у %s (теперь %s)",
        "commands.scoreboard.players.remove.success.multiple" => "Отнято %s от %s у %s сущностей",
        "commands.scoreboard.players.get.success" => "У %s %s %s",
        "commands.scoreboard.players.get.null" => "Нельзя узнать %s у %s: счёт не задан",
        "commands.scoreboard.players.list.empty" => "Нет отслеживаемых сущностей",
        "commands.scoreboard.players.list.success" => "Отслеживаемых сущностей: %s — %s",
        "commands.scoreboard.players.list.entity.empty" => "У %s нет счёта",
        "commands.scoreboard.players.list.entity.success" => "У %s счетов: %s",
        "commands.scoreboard.players.list.entity.entry" => "%s: %s",
        "commands.scoreboard.players.reset.all.single" => "Сброшен весь счёт %s",
        "commands.scoreboard.players.reset.all.multiple" => "Сброшен весь счёт %s сущностей",
        "commands.scoreboard.players.reset.specific.single" => "Сброшен %s у %s",
        "commands.scoreboard.players.reset.specific.multiple" => "Сброшен %s у %s сущностей",
        "commands.scoreboard.players.enable.success.single" => "Триггер %s разрешён для %s",
        "commands.scoreboard.players.enable.success.multiple" => "Триггер %s разрешён для %s сущностей",
        "commands.scoreboard.players.enable.failed" => "Ничего не изменилось: триггер уже разрешён",
        "commands.scoreboard.players.enable.invalid" => "enable работает только с целями-триггерами",
        "commands.scoreboard.players.operation.success.single" => "Счёт %s у %s теперь %s",
        "commands.scoreboard.players.operation.success.multiple" => "Изменён %s у %s сущностей",
        "commands.scoreboard.players.display.name.set.success.single" => "Название %s в %s изменено на %s",
        "commands.scoreboard.players.display.name.clear.success.single" => "Название %s в %s сброшено",
        "commands.scoreboard.players.display.numberFormat.set.success.single" => "Изменён вид числа %s в %s",
        "commands.scoreboard.players.display.numberFormat.clear.success.single" => "Сброшен вид числа %s в %s",
        "arguments.objective.notFound" => "Нет цели «%s»",
        "arguments.objective.readonly" => "Цель «%s» только для чтения",
        "argument.criteria.invalid" => "Неизвестное условие «%s»",
        "arithmetic.divideByZero" => "Делить на ноль нельзя",
        "argument.entity.notfound.entity" => "Сущность не найдена",
        "argument.entity.notfound.player" => "Игрок не найден",
        "team.notFound" => "Нет команды «%s»",
        "commands.team.add.success" => "Создана команда %s",
        "commands.team.add.duplicate" => "Команда с таким именем уже есть",
        "commands.team.remove.success" => "Удалена команда %s",
        "commands.team.empty.success" => "Из команды %2$s убрано участников: %1$s",
        "commands.team.empty.unchanged" => "Ничего не изменилось: команда и так пуста",
        "commands.team.join.success.single" => "%s вступил в команду %s",
        "commands.team.join.success.multiple" => "%s участников вступили в команду %s",
        "commands.team.leave.success.single" => "%s вышел из всех команд",
        "commands.team.leave.success.multiple" => "%s участников вышли из всех команд",
        "commands.team.list.teams.empty" => "Команд нет",
        "commands.team.list.teams.success" => "Команд: %s — %s",
        "commands.team.list.members.empty" => "В команде %s никого нет",
        "commands.team.list.members.success" => "В команде %s участников: %s — %s",
        "commands.team.option.color.success" => "Цвет команды %s теперь %s",
        "commands.team.option.color.unchanged" => "Ничего не изменилось: у команды уже этот цвет",
        "commands.team.option.name.success" => "Название команды %s изменено",
        "commands.team.option.name.unchanged" => "Ничего не изменилось: у команды уже это название",
        "commands.team.option.prefix.success" => "Приставка команды теперь %s",
        "commands.team.option.suffix.success" => "Окончание команды теперь %s",
        "commands.team.option.friendlyfire.enabled" => "Дружеский огонь в команде %s включён",
        "commands.team.option.friendlyfire.disabled" => "Дружеский огонь в команде %s выключен",
        "commands.team.option.friendlyfire.alreadyEnabled" => "Ничего не изменилось: дружеский огонь уже включён",
        "commands.team.option.friendlyfire.alreadyDisabled" => "Ничего не изменилось: дружеский огонь уже выключен",
        "commands.team.option.seeFriendlyInvisibles.enabled" => "Команда %s теперь видит невидимых своих",
        "commands.team.option.seeFriendlyInvisibles.disabled" => "Команда %s больше не видит невидимых своих",
        "commands.team.option.seeFriendlyInvisibles.alreadyEnabled" => "Ничего не изменилось: и так видит",
        "commands.team.option.seeFriendlyInvisibles.alreadyDisabled" => "Ничего не изменилось: и так не видит",
        "commands.team.option.nametagVisibility.success" => "Таблички с именами команды %s теперь «%s»",
        "commands.team.option.nametagVisibility.unchanged" => "Ничего не изменилось: таблички и так такие",
        "commands.team.option.deathMessageVisibility.success" => "Сообщения о смерти команды %s теперь «%s»",
        "commands.team.option.deathMessageVisibility.unchanged" => "Ничего не изменилось: сообщения о смерти и так такие",
        "commands.team.option.collisionRule.success" => "Толкание в команде %s теперь «%s»",
        "commands.team.option.collisionRule.unchanged" => "Ничего не изменилось: толкание и так такое",
        "commands.teammsg.failed.noteam" => "Ты не в команде",
        "commands.tag.add.success.single" => "Метка «%s» добавлена %s",
        "commands.tag.add.success.multiple" => "Метка «%s» добавлена %s сущностям",
        "commands.tag.add.failed" => "У цели уже есть эта метка или меток слишком много",
        "commands.tag.remove.success.single" => "Метка «%s» снята с %s",
        "commands.tag.remove.success.multiple" => "Метка «%s» снята с %s сущностей",
        "commands.tag.remove.failed" => "У цели нет этой метки",
        "commands.tag.list.single.empty" => "У %s нет меток",
        "commands.tag.list.single.success" => "У %s меток: %s — %s",
        "commands.tag.list.multiple.empty" => "У %s сущностей нет меток",
        "commands.tag.list.multiple.success" => "У %s сущностей меток: %s — %s",
        "commands.bossbar.create.success" => "Создана полоса %s",
        "commands.bossbar.create.failed" => "Полоса «%s» уже есть",
        "commands.bossbar.remove.success" => "Удалена полоса %s",
        "commands.bossbar.unknown" => "Нет полосы «%s»",
        "commands.bossbar.list.bars.none" => "Полос нет",
        "commands.bossbar.list.bars.some" => "Полос: %s — %s",
        "commands.bossbar.set.players.success.none" => "У полосы %s больше нет зрителей",
        "commands.bossbar.set.players.success.some" => "У полосы %s зрителей: %s — %s",
        "commands.bossbar.set.players.unchanged" => "Ничего не изменилось: зрители и так эти",
        "commands.bossbar.set.name.success" => "Полоса %s переименована",
        "commands.bossbar.set.name.unchanged" => "Ничего не изменилось: название и так это",
        "commands.bossbar.set.color.success" => "Цвет полосы %s изменён",
        "commands.bossbar.set.color.unchanged" => "Ничего не изменилось: цвет и так этот",
        "commands.bossbar.set.style.success" => "Вид полосы %s изменён",
        "commands.bossbar.set.style.unchanged" => "Ничего не изменилось: вид и так этот",
        "commands.bossbar.set.value.success" => "Значение полосы %s теперь %s",
        "commands.bossbar.set.value.unchanged" => "Ничего не изменилось: значение и так это",
        "commands.bossbar.set.max.success" => "Наибольшее значение полосы %s теперь %s",
        "commands.bossbar.set.max.unchanged" => "Ничего не изменилось: наибольшее и так это",
        "commands.bossbar.set.visible.success.visible" => "Полоса %s видна",
        "commands.bossbar.set.visible.success.hidden" => "Полоса %s скрыта",
        "commands.bossbar.set.visibility.unchanged.visible" => "Ничего не изменилось: полоса и так видна",
        "commands.bossbar.set.visibility.unchanged.hidden" => "Ничего не изменилось: полоса и так скрыта",
        "commands.bossbar.get.value" => "Значение полосы %s — %s",
        "commands.bossbar.get.max" => "Наибольшее значение полосы %s — %s",
        "commands.bossbar.get.visible.visible" => "Полоса %s видна",
        "commands.bossbar.get.visible.hidden" => "Полоса %s скрыта",
        "commands.bossbar.get.players.none" => "У полосы %s нет зрителей на сервере",
        "commands.bossbar.get.players.some" => "У полосы %s зрителей на сервере: %s — %s",
        "commands.trigger.failed.unprimed" => "Этот триггер пока нельзя",
        "commands.trigger.failed.invalid" => "Можно только цели-триггеры",
        "commands.trigger.simple.success" => "Триггер %s сработал",
        "commands.trigger.add.success" => "Триггер %s сработал (прибавлено %s)",
        "commands.trigger.set.success" => "Триггер %s сработал (задано %s)",
        "commands.execute.conditional.pass" => "Проверка пройдена",
        "commands.execute.conditional.pass_count" => "Проверка пройдена, число: %s",
        "commands.execute.conditional.fail" => "Проверка не пройдена",
        "commands.execute.conditional.fail_count" => "Проверка не пройдена, число: %s",
        "commands.function.success.single" => "Выполнено команд: %s из функции %s",
        "commands.function.success.single.result" => "Функция %2$s вернула %1$s",
        "commands.function.success.multiple" => "Выполнено команд: %s из %s функций",
        "commands.function.success.multiple.result" => "Выполнено функций: %s",
        "arguments.function.unknown" => "Нет функции %s",
        "commands.schedule.created.function" => "Функция «%s» запланирована через %s тактов, на такт %s",
        "commands.schedule.cleared.success" => "Снято с расписания: %s, имя %s",
        "commands.schedule.cleared.failure" => "Ничего не запланировано под именем %s",
        "commands.schedule.same_tick" => "Нельзя запланировать на этот же такт",
        "commands.datapack.list.enabled.success" => "Включено наборов данных: %s — %s",
        "commands.datapack.list.enabled.none" => "Включённых наборов данных нет",
        "commands.datapack.list.available.success" => "Доступно наборов данных: %s — %s",
        "commands.datapack.list.available.none" => "Других наборов данных нет",
        "commands.datapack.enable.failed" => "Набор «%s» уже включён",
        "commands.datapack.disable.failed" => "Набор «%s» не включён",
        "commands.datapack.modify.enable" => "Включается набор %s",
        "commands.datapack.modify.disable" => "Выключается набор %s",
        "commands.datapack.unknown" => "Нет набора данных «%s»",
        "commands.return.fail" => "Возврат с ошибкой",
        "chat.type.team.sent" => "-> %s <%s> %s",
        "chat.type.team.text" => "%s <%s> %s",
        _ => return format!("{key} {}", args.join(" ")),
    };

    fill(pattern, args)
}

/// Подставляет доводы в `%s` и `%N$s`.
fn fill(pattern: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut rest = pattern;

    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];

        if let Some(tail) = tail.strip_prefix('s') {
            out.push_str(args.get(next).map_or("", String::as_str));
            next += 1;
            rest = tail;
        } else if let Some((number, tail)) = tail.split_once("$s") {
            let index = number.parse::<usize>().unwrap_or(1).saturating_sub(1);
            out.push_str(args.get(index).map_or("", String::as_str));
            rest = tail;
        } else {
            out.push('%');
            rest = tail;
        }
    }

    out.push_str(rest);
    out
}

// ---- имена в ответах ----

/// Имя игрока в ответе — как у оригинала: щелчок предлагает `/tell`,
/// наведение показывает игрока; в команде — с её приставкой, окончанием и
/// цветом (`{"text":"","extra":[приставка, ник, окончание]}`). `entity` —
/// наведение «сущность» (tag, bossbar), иначе — текст-ник (держатель
/// счёта в scoreboard и team; чёрный ящик, score-players, tag-selectors).
pub(crate) fn player_component(board: &Scoreboard, name: &str, uuid: &[u8; 16], entity: bool) -> Value {
    let mut map = serde_json::Map::new();

    match board.team_of(name) {
        Some(team) => {
            map.insert("text".into(), literal(""));
            map.insert("extra".into(), Value::Array(vec![team.prefix.clone(), literal(name), team.suffix.clone()]));
            if team.color != scoreboard::COLOR_RESET {
                map.insert("color".into(), literal(scoreboard::COLORS[team.color as usize]));
            }
        }
        None => {
            map.insert("text".into(), literal(name));
        }
    }

    map.insert("insertion".into(), literal(name));
    map.insert("click_event".into(), serde_json::json!({"action": "suggest_command", "command": format!("/tell {name} ")}));

    let hover = if entity {
        let ints: Vec<Value> = uuid.chunks(4).map(|c| Value::from(i32::from_be_bytes([c[0], c[1], c[2], c[3]]))).collect();
        serde_json::json!({"action": "show_entity", "id": "minecraft:player", "uuid": ints, "name": name})
    } else {
        serde_json::json!({"action": "show_text", "value": name})
    };
    map.insert("hover_event".into(), hover);
    Value::Object(map)
}

/// `[название]` с подсказкой-именем — так оригинал называет цель в ответах.
fn bracketed(display: &Value, hover: &str) -> Value {
    let mut inner = as_map(display);
    inner.insert("hover_event".into(), serde_json::json!({"action": "show_text", "value": hover}));
    translate("chat.square_brackets", vec![Value::Object(inner)])
}

fn as_map(value: &Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(map) => map.clone(),
        other => {
            let mut map = serde_json::Map::new();
            map.insert("text".into(), Value::String(scoreboard::plain_text(other)));
            map
        }
    }
}

fn objective_text(objective: &Objective) -> Value {
    bracketed(&objective.display, &objective.name)
}

/// Команда в ответе: `[название]` её цветом, с подсказкой-именем и
/// вставкой имени по щелчку с Shift (чёрный ящик, team-basic).
fn team_text(team: &Team) -> Value {
    let mut inner = as_map(&team.display);
    inner.insert("insertion".into(), literal(&team.name));
    inner.insert("hover_event".into(), serde_json::json!({"action": "show_text", "value": team.name}));
    let mut outer = as_map(&translate("chat.square_brackets", vec![Value::Object(inner)]));
    if team.color != scoreboard::COLOR_RESET {
        outer.insert("color".into(), literal(scoreboard::COLORS[team.color as usize]));
    }
    Value::Object(outer)
}

/// Цвет текста для цвета полосы (вики, Commands/bossbar) [проверить: кроме
/// red → dark_red и white, сверенных чёрным ящиком].
const BAR_CHAT_COLORS: [&str; 7] = ["light_purple", "blue", "dark_red", "green", "yellow", "dark_blue", "white"];

/// Полоса в ответе: `[название]` её цветом, подсказка и вставка — её имя.
fn bar_text(bar: &BossBar) -> Value {
    let mut outer = as_map(&translate("chat.square_brackets", vec![bar.name.clone()]));
    outer.insert("color".into(), literal(BAR_CHAT_COLORS[bar.color]));
    outer.insert("insertion".into(), literal(&bar.id));
    outer.insert("hover_event".into(), serde_json::json!({"action": "show_text", "value": bar.id}));
    Value::Object(outer)
}

/// Список через серую «, » (вики: formatList); один элемент — как есть.
fn joined(mut items: Vec<Value>) -> Value {
    if items.len() == 1 {
        return items.pop().unwrap_or_default();
    }
    let mut extra = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            extra.push(serde_json::json!({"color": "gray", "text": ", "}));
        }
        extra.push(item);
    }
    serde_json::json!({"text": "", "extra": extra})
}

/// Держатели, участники, метки — в порядке хеш-таблицы оригинала
/// (`scoreboard::open_hash_order`).
fn in_java_order<T: Clone + AsRef<str>>(items: &[T]) -> Vec<T> {
    scoreboard::open_hash_order(items).into_iter().map(|index| items[index].clone()).collect()
}

/// Цели и команды — по имени (чёрный ящик: «sbo_a, sbo_b», «tb_blue,
/// tb_red») [проверить на больших списках].
fn by_name(items: &[String]) -> Vec<String> {
    let mut sorted = items.to_vec();
    sorted.sort();
    sorted
}

/// Зелёное имя — участник команды, метка (чёрный ящик, team-basic).
fn green(text: &str) -> Value {
    serde_json::json!({"color": "green", "text": text})
}

fn number(value: impl Into<i64>) -> Value {
    Value::from(value.into())
}

// ---- держатели и цели ----

/// Держатель счёта: ключ (ник или имя) и как его назвать в ответе.
struct Holder {
    key: String,
    uuid: Option<[u8; 16]>,
}

fn holder_text(board: &Scoreboard, holder: &Holder) -> Value {
    match holder.uuid {
        Some(uuid) => player_component(board, &holder.key, &uuid, false),
        None => literal(&holder.key),
    }
}

fn is_selector(text: &str) -> bool {
    text.starts_with('@')
}

/// Держатели по записи: выбор `@…` — игроки, `*` — все отслеживаемые (если
/// можно), иначе — имя как есть.
fn holders(text: &str, source: &Source, shared: &Shared, star: bool) -> Result<Vec<Holder>, Out> {
    if text == "*" && star {
        let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
        return Ok(scoreboard::holders(&board).into_iter().map(|key| Holder { key, uuid: None }).collect());
    }

    if is_selector(text) {
        let found = crate::commands::find_targets(text, source, shared);
        if found.is_empty() {
            return Err(Out::fail("argument.entity.notfound.entity", vec![]));
        }
        return Ok(found.into_iter().map(|(uuid, name)| Holder { key: name, uuid: Some(uuid) }).collect());
    }

    // Ник игрока на сервере — тоже игрок (с именем-ссылкой в ответе).
    let online = shared.players.lock().expect("список игроков захвачен другим потоком").find(text);
    Ok(vec![match online {
        Some(member) => Holder { key: member.name, uuid: Some(member.uuid) },
        None => Holder { key: text.to_string(), uuid: None },
    }])
}

fn objective_of(board: &Scoreboard, name: &str) -> Result<Objective, Out> {
    board.objective(name).cloned().ok_or_else(|| Out::fail("arguments.objective.notFound", vec![literal(name)]))
}

/// Только для чтения: здоровье, опыт и прочее, что сервер считает сам.
fn read_only(criteria: &str) -> bool {
    matches!(criteria, "health" | "food" | "air" | "armor" | "xp" | "level")
}

fn writable(board: &Scoreboard, name: &str) -> Result<Objective, Out> {
    let objective = objective_of(board, name)?;
    if read_only(&objective.criteria) {
        return Err(Out::fail("arguments.objective.readonly", vec![literal(name)]));
    }
    Ok(objective)
}

/// Годно ли условие цели (вики, Scoreboard#Criteria).
fn valid_criteria(criteria: &str) -> bool {
    const SIMPLE: [&str; 11] = ["dummy", "trigger", "deathCount", "playerKillCount", "totalKillCount", "health", "xp", "level", "food", "air", "armor"];
    const STATS: [&str; 9] = ["custom", "mined", "broken", "crafted", "used", "picked_up", "dropped", "killed", "killed_by"];

    if SIMPLE.contains(&criteria) {
        return true;
    }

    if let Some(color) = criteria.strip_prefix("teamkill.").or_else(|| criteria.strip_prefix("killedByTeam.")) {
        return scoreboard::COLORS.contains(&color);
    }

    let Some((kind, what)) = criteria.split_once(':') else {
        return false;
    };
    let kind = kind.strip_prefix("minecraft.").unwrap_or(kind);
    STATS.contains(&kind) && !what.is_empty()
}

/// Имя цели, команды: буквы, цифры и `_ - . +` (вики: unquoted string).
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-.+".contains(c))
}

fn int(text: &str) -> Result<i32, Out> {
    text.parse::<i32>().map_err(|_| bad(text))
}

/// Хвост команды с `from`-го слова — для текстов с пробелами.
fn tail(words: &[&str], from: usize) -> String {
    words.get(from..).map(|rest| rest.join(" ")).unwrap_or_default()
}

fn component_arg(words: &[&str], from: usize) -> Result<Value, Out> {
    let text = tail(words, from);
    scoreboard::parse_component(&text).ok_or_else(|| bad(&text))
}

// ---- вход ----

/// Выполнить команду этой группы. `line` — команда целиком без косой черты.
pub fn run(line: &str, source: &Source, shared: &Shared) -> Answer {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(&name) = words.first() else {
        return silent(0);
    };

    let out = match name {
        "scoreboard" => scoreboard_command(&words[1..], source, shared),
        "team" => team_command(&words[1..], source, shared),
        "tag" => tag_command(&words[1..], source, shared),
        "bossbar" => bossbar_command(&words[1..], source, shared),
        "trigger" => trigger_command(&words[1..], source, shared),
        "teammsg" | "tm" => return teammsg(&words[1..], source, shared),
        "function" => return crate::functions::function_command(&words[1..], source, shared),
        "schedule" => return crate::functions::schedule_command(&words[1..], shared),
        "datapack" => return crate::functions::datapack_command(&words[1..], shared),
        "return" => return crate::functions::return_command(&words[1..], source, shared),
        "execute" => return execute(&words[1..], source, shared),
        _ => Err(bad(name)),
    };

    match out {
        Ok(out) | Err(out) => out.answer(),
    }
}

// ---- scoreboard ----

fn scoreboard_command(words: &[&str], source: &Source, shared: &Shared) -> Result<Out, Out> {
    match words {
        ["objectives", rest @ ..] => objectives(rest, words, shared),
        ["players", rest @ ..] => players(rest, words, source, shared),
        _ => Err(bad(words.first().copied().unwrap_or(""))),
    }
}

fn objectives(rest: &[&str], words: &[&str], shared: &Shared) -> Result<Out, Out> {
    let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");

    match rest {
        ["add", name, criteria, ..] => {
            if !valid_name(name) {
                return Err(bad(name));
            }
            if !valid_criteria(criteria) {
                return Err(Out::fail("argument.criteria.invalid", vec![literal(criteria)]));
            }
            if board.objective(name).is_some() {
                return Err(Out::fail("commands.scoreboard.objectives.add.duplicate", vec![]));
            }
            let display = if rest.len() > 3 { component_arg(words, 4)? } else { literal(name) };
            let objective = Objective {
                name: name.to_string(),
                criteria: criteria.to_string(),
                display,
                // Здоровье по умолчанию — сердечками (вики, Scoreboard).
                hearts: *criteria == "health",
                auto_update: false,
                format: None,
            };
            let text = objective_text(&objective);
            board.add_objective(objective);
            let count = board.objectives.len() as i32;
            Ok(Out::ok("commands.scoreboard.objectives.add.success", vec![text], count))
        }
        ["remove", name] => {
            let objective = objective_of(&board, name)?;
            board.remove_objective(name);
            let count = board.objectives.len() as i32;
            Ok(Out::ok("commands.scoreboard.objectives.remove.success", vec![objective_text(&objective)], count))
        }
        ["list"] => {
            if board.objectives.is_empty() {
                return Ok(Out::ok("commands.scoreboard.objectives.list.empty", vec![], 0));
            }
            let count = board.objectives.len();
            let names: Vec<String> = board.objectives.iter().map(|objective| objective.name.clone()).collect();
            let list = joined(by_name(&names).iter().filter_map(|name| board.objective(name)).map(objective_text).collect());
            Ok(Out::ok("commands.scoreboard.objectives.list.success", vec![number(count as i64), list], count as i32))
        }
        ["setdisplay", slot, more @ ..] => {
            let slot_index = scoreboard::slot_by_name(slot).ok_or_else(|| bad(slot))?;
            match more {
                [] => {
                    if board.display[slot_index].is_none() {
                        return Err(Out::fail("commands.scoreboard.objectives.display.alreadyEmpty", vec![]));
                    }
                    board.set_display(slot_index, None);
                    Ok(Out::ok("commands.scoreboard.objectives.display.cleared", vec![literal(slot)], 0))
                }
                [name] => {
                    let objective = objective_of(&board, name)?;
                    if board.display[slot_index].as_deref() == Some(*name) {
                        return Err(Out::fail("commands.scoreboard.objectives.display.alreadySet", vec![]));
                    }
                    board.set_display(slot_index, Some(name.to_string()));
                    Ok(Out::ok("commands.scoreboard.objectives.display.set", vec![literal(slot), objective.display.clone()], 0))
                }
                _ => Err(bad(more[1])),
            }
        }
        ["modify", name, what, ..] => {
            let objective = objective_of(&board, name)?;
            match *what {
                "displayname" => {
                    let display = component_arg(words, 4)?;
                    board.modify_objective(name, |objective| objective.display = display.clone());
                    Ok(Out::ok("commands.scoreboard.objectives.modify.displayname", vec![literal(name), objective_text(board.objective(name).unwrap_or(&objective))], 0))
                }
                "rendertype" => {
                    let hearts = match rest.get(3) {
                        Some(&"hearts") => true,
                        Some(&"integer") => false,
                        other => return Err(bad(other.copied().unwrap_or(""))),
                    };
                    if objective.hearts != hearts {
                        board.modify_objective(name, |objective| objective.hearts = hearts);
                    }
                    Ok(Out::ok("commands.scoreboard.objectives.modify.rendertype", vec![objective_text(&objective)], 0))
                }
                "numberformat" => {
                    let format = number_format(words, 4)?;
                    let cleared = format.is_none();
                    board.modify_objective(name, |objective| objective.format = format);
                    let key = if cleared { "commands.scoreboard.objectives.modify.objectiveFormat.clear" } else { "commands.scoreboard.objectives.modify.objectiveFormat.set" };
                    Ok(Out::ok(key, vec![literal(name)], 0))
                }
                "displayautoupdate" => {
                    let on = match rest.get(3) {
                        Some(&"true") => true,
                        Some(&"false") => false,
                        other => return Err(bad(other.copied().unwrap_or(""))),
                    };
                    board.modify_objective(name, |objective| objective.auto_update = on);
                    let key = if on { "commands.scoreboard.objectives.modify.displayAutoUpdate.enable" } else { "commands.scoreboard.objectives.modify.displayAutoUpdate.disable" };
                    Ok(Out::ok(key, vec![literal(name), objective_text(&objective)], 0))
                }
                _ => Err(bad(what)),
            }
        }
        _ => Err(bad(rest.first().copied().unwrap_or(""))),
    }
}

/// Вид чисел из команды: ничего (сброс), `blank`, `fixed <текст>`,
/// `styled <оформление>`.
fn number_format(words: &[&str], from: usize) -> Result<Option<NumberFormat>, Out> {
    match words.get(from) {
        None => Ok(None),
        Some(&"blank") => Ok(Some(NumberFormat::Blank)),
        Some(&"fixed") => Ok(Some(NumberFormat::Fixed(component_arg(words, from + 1)?))),
        Some(&"styled") => {
            let style = component_arg(words, from + 1)?;
            if !style.is_object() {
                return Err(bad(&tail(words, from + 1)));
            }
            Ok(Some(NumberFormat::Styled(style)))
        }
        Some(other) => Err(bad(other)),
    }
}

fn players(rest: &[&str], words: &[&str], source: &Source, shared: &Shared) -> Result<Out, Out> {
    match rest {
        [action @ ("set" | "add" | "remove"), who, objective, amount] => {
            let amount = int(amount)?;
            if *action != "set" && amount < 0 {
                return Err(bad(&amount.to_string()));
            }
            let targets = holders(who, source, shared, true)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = writable(&board, objective)?;
            let mut total = 0i32;
            let mut last = 0;

            for holder in &targets {
                last = board.change_score(&holder.key, &objective.name, |score| {
                    score.value = match *action {
                        "set" => amount,
                        "add" => score.value.wrapping_add(amount),
                        _ => score.value.wrapping_sub(amount),
                    }
                });
                total = total.wrapping_add(last);
            }

            let text = objective_text(&objective);
            let key = format!("commands.scoreboard.players.{action}.success");
            Ok(match (targets.as_slice(), *action) {
                ([one], "set") => Out::ok(&format!("{key}.single"), vec![text, holder_text(&board, one), number(amount)], total),
                ([one], _) => Out::ok(&format!("{key}.single"), vec![number(amount), text, holder_text(&board, one), number(last)], total),
                (_, "set") => Out::ok(&format!("{key}.multiple"), vec![text, number(targets.len() as i64), number(amount)], total),
                _ => Out::ok(&format!("{key}.multiple"), vec![number(amount), text, number(targets.len() as i64)], total),
            })
        }
        ["get", who, objective] => {
            let targets = holders(who, source, shared, false)?;
            let [holder] = targets.as_slice() else {
                return Err(Out::fail("argument.entity.toomany", vec![]));
            };
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = objective_of(&board, objective)?;
            match board.score(&holder.key, &objective.name) {
                Some(score) => Ok(Out::ok("commands.scoreboard.players.get.success", vec![holder_text(&board, holder), number(score.value), objective_text(&objective)], score.value)),
                None => Err(Out::fail("commands.scoreboard.players.get.null", vec![literal(&objective.name), literal(&holder.key)])),
            }
        }
        ["list"] => {
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let list = scoreboard::holders(&board);
            if list.is_empty() {
                return Ok(Out::ok("commands.scoreboard.players.list.empty", vec![], 0));
            }
            let count = list.len();
            Ok(Out::ok("commands.scoreboard.players.list.success", vec![number(count as i64), joined(in_java_order(&list).iter().map(|key| literal(key)).collect())], count as i32))
        }
        ["list", who] => {
            let targets = holders(who, source, shared, false)?;
            let [holder] = targets.as_slice() else {
                return Err(Out::fail("argument.entity.toomany", vec![]));
            };
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let scores: Vec<(Objective, i32)> = board
                .objectives
                .iter()
                .filter_map(|objective| board.score(&holder.key, &objective.name).map(|score| (objective.clone(), score.value)))
                .collect();
            if scores.is_empty() {
                return Ok(Out::ok("commands.scoreboard.players.list.entity.empty", vec![holder_text(&board, holder)], 0));
            }
            // Заголовок и по сообщению на счёт (чёрный ящик, score-players).
            let mut out = Out::ok("commands.scoreboard.players.list.entity.success", vec![holder_text(&board, holder), number(scores.len() as i64)], scores.len() as i32);
            for (objective, value) in &scores {
                out.more.push(translate("commands.scoreboard.players.list.entity.entry", vec![objective_text(objective), number(*value)]));
            }
            Ok(out)
        }
        ["reset", who, more @ ..] => {
            let targets = holders(who, source, shared, true)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = match more {
                [] => None,
                [name] => Some(objective_of(&board, name)?),
                _ => return Err(bad(more[1])),
            };
            for holder in &targets {
                board.reset_score(&holder.key, objective.as_ref().map(|objective| objective.name.as_str()));
            }
            let count = targets.len() as i32;
            Ok(match (targets.as_slice(), objective) {
                ([one], None) => Out::ok("commands.scoreboard.players.reset.all.single", vec![holder_text(&board, one)], count),
                (_, None) => Out::ok("commands.scoreboard.players.reset.all.multiple", vec![number(count)], count),
                ([one], Some(objective)) => Out::ok("commands.scoreboard.players.reset.specific.single", vec![objective_text(&objective), holder_text(&board, one)], count),
                (_, Some(objective)) => Out::ok("commands.scoreboard.players.reset.specific.multiple", vec![objective_text(&objective), number(count)], count),
            })
        }
        ["enable", who, objective] => {
            let targets = holders(who, source, shared, true)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = objective_of(&board, objective)?;
            if objective.criteria != "trigger" {
                return Err(Out::fail("commands.scoreboard.players.enable.invalid", vec![]));
            }
            let mut changed = 0;
            for holder in &targets {
                if board.score(&holder.key, &objective.name).is_none_or(|score| score.locked) {
                    board.change_score(&holder.key, &objective.name, |score| score.locked = false);
                    changed += 1;
                }
            }
            if changed == 0 {
                return Err(Out::fail("commands.scoreboard.players.enable.failed", vec![]));
            }
            Ok(match targets.as_slice() {
                [one] => Out::ok("commands.scoreboard.players.enable.success.single", vec![objective_text(&objective), holder_text(&board, one)], changed),
                _ => Out::ok("commands.scoreboard.players.enable.success.multiple", vec![objective_text(&objective), number(targets.len() as i64)], changed),
            })
        }
        ["operation", who, objective, operation, from, from_objective] => {
            let targets = holders(who, source, shared, true)?;
            let sources = holders(from, source, shared, true)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = writable(&board, objective)?;
            let from_objective = objective_of(&board, from_objective)?;
            let mut total = 0i32;

            for target in &targets {
                board.touch_score(&target.key, &objective.name);
                for other in &sources {
                    board.touch_score(&other.key, &from_objective.name);
                    let theirs = board.score(&other.key, &from_objective.name).map_or(0, |score| score.value);
                    let mine = board.score(&target.key, &objective.name).map_or(0, |score| score.value);
                    let (new_mine, new_theirs) = match *operation {
                        "=" => (theirs, None),
                        "+=" => (mine.wrapping_add(theirs), None),
                        "-=" => (mine.wrapping_sub(theirs), None),
                        "*=" => (mine.wrapping_mul(theirs), None),
                        "/=" | "%=" if theirs == 0 => return Err(Out::fail("arguments.operation.div0", vec![])),
                        "/=" => (mine.wrapping_div_euclid_floor(theirs), None),
                        "%=" => (mine.rem_euclid_floor(theirs), None),
                        "<" => (mine.min(theirs), None),
                        ">" => (mine.max(theirs), None),
                        "><" => (theirs, Some(mine)),
                        other => return Err(bad(other)),
                    };
                    board.set_score(&target.key, &objective.name, new_mine);
                    if let Some(swapped) = new_theirs {
                        board.set_score(&other.key, &from_objective.name, swapped);
                    }
                }
                total = total.wrapping_add(board.score(&target.key, &objective.name).map_or(0, |score| score.value));
            }

            Ok(match targets.as_slice() {
                [one] => Out::ok("commands.scoreboard.players.operation.success.single", vec![objective_text(&objective), holder_text(&board, one), number(total)], total),
                _ => Out::ok("commands.scoreboard.players.operation.success.multiple", vec![objective_text(&objective), number(targets.len() as i64)], total),
            })
        }
        ["display", "name", who, objective, ..] => {
            let targets = holders(who, source, shared, true)?;
            let display = if rest.len() > 4 { Some(component_arg(words, 5)?) } else { None };
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = objective_of(&board, objective)?;
            for holder in &targets {
                board.change_score(&holder.key, &objective.name, |score| score.display = display.clone());
            }
            let count = targets.len() as i32;
            let one = targets.first().map(|holder| holder_text(&board, holder)).unwrap_or_default();
            Ok(match display {
                Some(display) => Out::ok("commands.scoreboard.players.display.name.set.success.single", vec![display, one, objective_text(&objective)], count),
                None => Out::ok("commands.scoreboard.players.display.name.clear.success.single", vec![one, objective_text(&objective)], count),
            })
        }
        ["display", "numberformat", who, objective, ..] => {
            let targets = holders(who, source, shared, true)?;
            let format = number_format(words, 5)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let objective = objective_of(&board, objective)?;
            let cleared = format.is_none();
            for holder in &targets {
                board.change_score(&holder.key, &objective.name, |score| score.format = format.clone());
            }
            let count = targets.len() as i32;
            let one = targets.first().map(|holder| holder_text(&board, holder)).unwrap_or_default();
            let key = if cleared { "commands.scoreboard.players.display.numberFormat.clear.success.single" } else { "commands.scoreboard.players.display.numberFormat.set.success.single" };
            Ok(Out::ok(key, vec![one, objective_text(&objective)], count))
        }
        _ => Err(bad(rest.first().copied().unwrap_or(""))),
    }
}

/// Деление и остаток с округлением вниз — как `Math.floorDiv` / `floorMod`
/// (вики, Scoreboard: «/= … rounded down», «%= … sign of divisor»).
trait FloorOps {
    fn wrapping_div_euclid_floor(self, by: i32) -> i32;
    fn rem_euclid_floor(self, by: i32) -> i32;
}

impl FloorOps for i32 {
    fn wrapping_div_euclid_floor(self, by: i32) -> i32 {
        let quotient = self.wrapping_div(by);
        if (self % by != 0) && ((self < 0) != (by < 0)) { quotient - 1 } else { quotient }
    }

    fn rem_euclid_floor(self, by: i32) -> i32 {
        let rest = self.wrapping_rem(by);
        if rest != 0 && ((rest < 0) != (by < 0)) { rest + by } else { rest }
    }
}

// ---- team ----

fn team_command(words: &[&str], source: &Source, shared: &Shared) -> Result<Out, Out> {
    let full: Vec<&str> = std::iter::once("team").chain(words.iter().copied()).collect();

    match words {
        ["add", name, ..] => {
            if !valid_name(name) {
                return Err(bad(name));
            }
            let display = if words.len() > 2 { component_arg(&full, 3)? } else { literal(name) };
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            if board.team(name).is_some() {
                return Err(Out::fail("commands.team.add.duplicate", vec![]));
            }
            let team = Team {
                name: name.to_string(),
                display,
                color: scoreboard::COLOR_RESET,
                prefix: literal(""),
                suffix: literal(""),
                friendly_fire: true,
                see_invisible: true,
                nametag: 0,
                death_messages: 0,
                collision: 0,
                members: Vec::new(),
            };
            let text = team_text(&team);
            board.add_team(team);
            let count = board.teams.len() as i32;
            Ok(Out::ok("commands.team.add.success", vec![text], count))
        }
        ["remove", name] => {
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let team = board.team(name).cloned().ok_or_else(|| Out::fail("team.notFound", vec![literal(name)]))?;
            board.remove_team(name);
            let count = board.teams.len() as i32;
            Ok(Out::ok("commands.team.remove.success", vec![team_text(&team)], count))
        }
        ["empty", name] => {
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let team = board.team(name).cloned().ok_or_else(|| Out::fail("team.notFound", vec![literal(name)]))?;
            if team.members.is_empty() {
                return Err(Out::fail("commands.team.empty.unchanged", vec![]));
            }
            let count = board.leave_team(&team.members) as i32;
            Ok(Out::ok("commands.team.empty.success", vec![number(count), team_text(&team)], count))
        }
        ["join", name, more @ ..] => {
            let targets = match more {
                [] => holders("@s", source, shared, false)?,
                [who] => holders(who, source, shared, true)?,
                _ => return Err(bad(more[1])),
            };
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let team = board.team(name).cloned().ok_or_else(|| Out::fail("team.notFound", vec![literal(name)]))?;
            let keys: Vec<String> = targets.iter().map(|holder| holder.key.clone()).collect();
            board.join_team(name, &keys);
            let count = targets.len() as i32;
            Ok(match targets.as_slice() {
                [one] => Out::ok("commands.team.join.success.single", vec![holder_text(&board, one), team_text(&team)], count),
                _ => Out::ok("commands.team.join.success.multiple", vec![number(count), team_text(&team)], count),
            })
        }
        ["leave", who] => {
            let targets = holders(who, source, shared, true)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let keys: Vec<String> = targets.iter().map(|holder| holder.key.clone()).collect();
            // Оригинал не отказывает, если игрок и так вне команд [проверить].
            board.leave_team(&keys);
            let count = targets.len() as i32;
            Ok(match targets.as_slice() {
                [one] => Out::ok("commands.team.leave.success.single", vec![holder_text(&board, one)], count),
                _ => Out::ok("commands.team.leave.success.multiple", vec![number(count)], count),
            })
        }
        ["list"] => {
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            if board.teams.is_empty() {
                return Ok(Out::ok("commands.team.list.teams.empty", vec![], 0));
            }
            let count = board.teams.len();
            let names: Vec<String> = board.teams.iter().map(|team| team.name.clone()).collect();
            let list = joined(by_name(&names).iter().filter_map(|name| board.team(name)).map(team_text).collect());
            Ok(Out::ok("commands.team.list.teams.success", vec![number(count as i64), list], count as i32))
        }
        ["list", name] => {
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let team = board.team(name).cloned().ok_or_else(|| Out::fail("team.notFound", vec![literal(name)]))?;
            if team.members.is_empty() {
                return Ok(Out::ok("commands.team.list.members.empty", vec![team_text(&team)], 0));
            }
            let count = team.members.len();
            Ok(Out::ok("commands.team.list.members.success", vec![team_text(&team), number(count as i64), joined(in_java_order(&team.members).iter().map(|member| green(member)).collect())], count as i32))
        }
        ["modify", name, option, ..] => {
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let team = board.team(name).cloned().ok_or_else(|| Out::fail("team.notFound", vec![literal(name)]))?;
            let value = words.get(3).copied().unwrap_or("");
            let pick = |names: &[&str]| names.iter().position(|known| *known == value).ok_or_else(|| bad(value));
            let flag = || match value {
                "true" => Ok(true),
                "false" => Ok(false),
                other => Err(bad(other)),
            };

            match *option {
                "color" => {
                    let color = if value == "reset" { scoreboard::COLOR_RESET } else { pick(&scoreboard::COLORS)? as i32 };
                    if team.color == color {
                        return Err(Out::fail("commands.team.option.color.unchanged", vec![]));
                    }
                    board.modify_team(name, |team| team.color = color);
                    let team = board.team(name).cloned().unwrap_or(team);
                    Ok(Out::ok("commands.team.option.color.success", vec![team_text(&team), literal(value)], 0))
                }
                "displayName" => {
                    let display = component_arg(&full, 4)?;
                    if team.display == display {
                        return Err(Out::fail("commands.team.option.name.unchanged", vec![]));
                    }
                    board.modify_team(name, |team| team.display = display);
                    let team = board.team(name).cloned().unwrap_or(team);
                    Ok(Out::ok("commands.team.option.name.success", vec![team_text(&team)], 0))
                }
                "prefix" | "suffix" => {
                    let text = component_arg(&full, 4)?;
                    let is_prefix = *option == "prefix";
                    let shown = text.clone();
                    board.modify_team(name, |team| if is_prefix { team.prefix = text } else { team.suffix = text });
                    Ok(Out::ok(&format!("commands.team.option.{option}.success"), vec![shown], 1))
                }
                "friendlyFire" | "seeFriendlyInvisibles" => {
                    let on = flag()?;
                    let fire = *option == "friendlyFire";
                    let now = if fire { team.friendly_fire } else { team.see_invisible };
                    let base = if fire { "commands.team.option.friendlyfire" } else { "commands.team.option.seeFriendlyInvisibles" };
                    if now == on {
                        return Err(Out::fail(&format!("{base}.{}", if on { "alreadyEnabled" } else { "alreadyDisabled" }), vec![]));
                    }
                    board.modify_team(name, |team| if fire { team.friendly_fire = on } else { team.see_invisible = on });
                    Ok(Out::ok(&format!("{base}.{}", if on { "enabled" } else { "disabled" }), vec![team_text(&team)], 0))
                }
                "nametagVisibility" | "deathMessageVisibility" | "collisionRule" => {
                    let names: &[&str] = if *option == "collisionRule" { &scoreboard::COLLISIONS } else { &scoreboard::VISIBILITIES };
                    let index = pick(names)?;
                    let now = match *option {
                        "nametagVisibility" => team.nametag,
                        "deathMessageVisibility" => team.death_messages,
                        _ => team.collision,
                    };
                    if now == index {
                        return Err(Out::fail(&format!("commands.team.option.{option}.unchanged"), vec![]));
                    }
                    let which = *option;
                    board.modify_team(name, |team| match which {
                        "nametagVisibility" => team.nametag = index,
                        "deathMessageVisibility" => team.death_messages = index,
                        _ => team.collision = index,
                    });
                    // Название правила — переводом (`team.visibility.…`, `team.collision.…`).
                    let kind = if which == "collisionRule" { "collision" } else { "visibility" };
                    Ok(Out::ok(&format!("commands.team.option.{which}.success"), vec![team_text(&team), translate(&format!("team.{kind}.{value}"), vec![])], 0))
                }
                other => Err(bad(other)),
            }
        }
        _ => Err(bad(words.first().copied().unwrap_or(""))),
    }
}

// ---- tag ----

fn tag_command(words: &[&str], source: &Source, shared: &Shared) -> Result<Out, Out> {
    let [who, action, rest @ ..] = words else {
        return Err(bad(words.first().copied().unwrap_or("")));
    };
    let targets: Vec<([u8; 16], String)> = crate::commands::find_targets(who, source, shared);
    if targets.is_empty() {
        return Err(Out::fail("argument.entity.notfound.entity", vec![]));
    }
    let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
    let one = |board: &Scoreboard| player_component(board, &targets[0].1, &targets[0].0, true);

    match (*action, rest) {
        ("add" | "remove", [tag]) => {
            if !valid_name(tag) {
                return Err(bad(tag));
            }
            let adding = *action == "add";
            let changed = targets
                .iter()
                .filter(|(uuid, _)| {
                    let key = scoreboard::tag_key(uuid);
                    if adding { board.add_tag(&key, tag) } else { board.remove_tag(&key, tag) }
                })
                .count() as i32;
            if changed == 0 {
                return Err(Out::fail(&format!("commands.tag.{action}.failed"), vec![]));
            }
            Ok(if targets.len() == 1 {
                Out::ok(&format!("commands.tag.{action}.success.single"), vec![literal(tag), one(&board)], changed)
            } else {
                Out::ok(&format!("commands.tag.{action}.success.multiple"), vec![literal(tag), number(targets.len() as i64)], changed)
            })
        }
        ("list", []) => {
            let mut all: Vec<String> = Vec::new();
            for (uuid, _) in &targets {
                for tag in board.tags_of(&scoreboard::tag_key(uuid)) {
                    if !all.contains(tag) {
                        all.push(tag.clone());
                    }
                }
            }
            let count = all.len() as i32;
            let list = joined(in_java_order(&all).iter().map(|tag| green(tag)).collect());
            Ok(match (targets.len(), all.is_empty()) {
                (1, true) => Out::ok("commands.tag.list.single.empty", vec![one(&board)], 0),
                (1, false) => Out::ok("commands.tag.list.single.success", vec![one(&board), number(count), list], count),
                (n, true) => Out::ok("commands.tag.list.multiple.empty", vec![number(n as i64)], 0),
                (n, false) => Out::ok("commands.tag.list.multiple.success", vec![number(n as i64), number(count), list], count),
            })
        }
        _ => Err(bad(action)),
    }
}

// ---- bossbar ----

fn bossbar_id(text: &str) -> String {
    if text.contains(':') { text.to_string() } else { format!("minecraft:{text}") }
}

fn bossbar_command(words: &[&str], source: &Source, shared: &Shared) -> Result<Out, Out> {
    let full: Vec<&str> = std::iter::once("bossbar").chain(words.iter().copied()).collect();

    match words {
        ["add", id, ..] => {
            let id = bossbar_id(id);
            let name = component_arg(&full, 3)?;
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            if board.bossbar(&id).is_some() {
                return Err(Out::fail("commands.bossbar.create.failed", vec![literal(&id)]));
            }
            let bar = BossBar { uuid: scoreboard::fresh_uuid(&id), id, name, color: 6, style: 0, value: 0, max: 100, visible: true, players: Vec::new() };
            let text = bar_text(&bar);
            board.add_bossbar(bar);
            let count = board.bossbars.len() as i32;
            Ok(Out::ok("commands.bossbar.create.success", vec![text], count))
        }
        ["remove", id] => {
            let id = bossbar_id(id);
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let bar = board.bossbar(&id).cloned().ok_or_else(|| Out::fail("commands.bossbar.unknown", vec![literal(&id)]))?;
            board.remove_bossbar(&id);
            let count = board.bossbars.len() as i32;
            Ok(Out::ok("commands.bossbar.remove.success", vec![bar_text(&bar)], count))
        }
        ["list"] => {
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            if board.bossbars.is_empty() {
                return Ok(Out::ok("commands.bossbar.list.bars.none", vec![], 0));
            }
            let count = board.bossbars.len();
            Ok(Out::ok("commands.bossbar.list.bars.some", vec![number(count as i64), joined(board.bossbars.iter().map(bar_text).collect())], count as i32))
        }
        ["get", id, what] => {
            let id = bossbar_id(id);
            let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let bar = board.bossbar(&id).cloned().ok_or_else(|| Out::fail("commands.bossbar.unknown", vec![literal(&id)]))?;
            match *what {
                "value" => Ok(Out::ok("commands.bossbar.get.value", vec![bar_text(&bar), number(bar.value)], bar.value)),
                "max" => Ok(Out::ok("commands.bossbar.get.max", vec![bar_text(&bar), number(bar.max)], bar.max)),
                "visible" => Ok(Out::ok(if bar.visible { "commands.bossbar.get.visible.visible" } else { "commands.bossbar.get.visible.hidden" }, vec![bar_text(&bar)], i32::from(bar.visible))),
                "players" => {
                    let online = online_viewers(&board, &bar, shared);
                    if online.is_empty() {
                        Ok(Out::ok("commands.bossbar.get.players.none", vec![bar_text(&bar)], 0))
                    } else {
                        let count = online.len() as i32;
                        let list = joined(online.iter().map(|(uuid, name)| player_component(&board, name, uuid, true)).collect());
                        Ok(Out::ok("commands.bossbar.get.players.some", vec![bar_text(&bar), number(count), list], count))
                    }
                }
                other => Err(bad(other)),
            }
        }
        ["set", id, what, rest @ ..] => {
            let id = bossbar_id(id);
            let targets = match (*what, rest) {
                ("players", [who, ..]) => crate::commands::find_targets(who, source, shared),
                _ => Vec::new(),
            };
            let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
            let bar = board.bossbar(&id).cloned().ok_or_else(|| Out::fail("commands.bossbar.unknown", vec![literal(&id)]))?;
            let text = bar_text(&bar);
            let value = rest.first().copied().unwrap_or("");

            match *what {
                "value" | "max" => {
                    let amount = int(value)?;
                    let is_max = *what == "max";
                    if is_max && amount < 1 || !is_max && amount < 0 {
                        return Err(bad(value));
                    }
                    if (if is_max { bar.max } else { bar.value }) == amount {
                        return Err(Out::fail(&format!("commands.bossbar.set.{what}.unchanged"), vec![]));
                    }
                    board.modify_bossbar(&id, Some(2), |bar| if is_max { bar.max = amount } else { bar.value = amount });
                    Ok(Out::ok(&format!("commands.bossbar.set.{what}.success"), vec![text, number(amount)], amount))
                }
                "color" | "style" => {
                    let names: &[&str] = if *what == "color" { &scoreboard::BAR_COLORS } else { &scoreboard::BAR_STYLES };
                    let index = names.iter().position(|name| *name == value).ok_or_else(|| bad(value))?;
                    let is_color = *what == "color";
                    if (if is_color { bar.color } else { bar.style }) == index {
                        return Err(Out::fail(&format!("commands.bossbar.set.{what}.unchanged"), vec![]));
                    }
                    board.modify_bossbar(&id, Some(4), |bar| if is_color { bar.color = index } else { bar.style = index });
                    let text = board.bossbar(&id).map_or(text, bar_text);
                    Ok(Out::ok(&format!("commands.bossbar.set.{what}.success"), vec![text], 0))
                }
                "name" => {
                    let name = component_arg(&full, 4)?;
                    if bar.name == name {
                        return Err(Out::fail("commands.bossbar.set.name.unchanged", vec![]));
                    }
                    board.modify_bossbar(&id, Some(3), |bar| bar.name = name);
                    let bar = board.bossbar(&id).cloned().unwrap_or(bar);
                    Ok(Out::ok("commands.bossbar.set.name.success", vec![bar_text(&bar)], 0))
                }
                "visible" => {
                    let on = match value {
                        "true" => true,
                        "false" => false,
                        other => return Err(bad(other)),
                    };
                    if bar.visible == on {
                        return Err(Out::fail(if on { "commands.bossbar.set.visibility.unchanged.visible" } else { "commands.bossbar.set.visibility.unchanged.hidden" }, vec![]));
                    }
                    board.modify_bossbar(&id, None, |bar| bar.visible = on);
                    Ok(Out::ok(if on { "commands.bossbar.set.visible.success.visible" } else { "commands.bossbar.set.visible.success.hidden" }, vec![text], 0))
                }
                "players" => {
                    if !rest.is_empty() && targets.is_empty() {
                        return Err(Out::fail("argument.entity.notfound.player", vec![]));
                    }
                    let uuids: Vec<[u8; 16]> = targets.iter().map(|(uuid, _)| *uuid).collect();
                    let mut sorted_new = uuids.clone();
                    let mut sorted_old = bar.players.clone();
                    sorted_new.sort();
                    sorted_old.sort();
                    if sorted_new == sorted_old {
                        return Err(Out::fail("commands.bossbar.set.players.unchanged", vec![]));
                    }
                    board.modify_bossbar(&id, None, |bar| bar.players = uuids.clone());
                    if targets.is_empty() {
                        Ok(Out::ok("commands.bossbar.set.players.success.none", vec![text], 0))
                    } else {
                        let count = targets.len() as i32;
                        let list = joined(targets.iter().map(|(uuid, name)| player_component(&board, name, uuid, true)).collect());
                        Ok(Out::ok("commands.bossbar.set.players.success.some", vec![text, number(count), list], count))
                    }
                }
                other => Err(bad(other)),
            }
        }
        _ => Err(bad(words.first().copied().unwrap_or(""))),
    }
}

/// Зрители полосы, что сейчас на сервере.
fn online_viewers(_board: &Scoreboard, bar: &BossBar, shared: &Shared) -> Vec<([u8; 16], String)> {
    // Замок игроков после табло: порядок замков — игроки раньше табло, но
    // здесь игроки только читаются копией через try_lock [проверить].
    let Ok(players) = shared.players.try_lock() else {
        return Vec::new();
    };
    players.members().iter().filter(|member| bar.players.contains(&member.uuid)).map(|member| (member.uuid, member.name.clone())).collect()
}

// ---- trigger, teammsg ----

fn trigger_command(words: &[&str], source: &Source, shared: &Shared) -> Result<Out, Out> {
    let Source::Player { name, .. } = source else {
        return Err(Out::fail("permissions.requires.player", vec![]));
    };
    let (objective_name, mode, amount) = match words {
        [objective] => (*objective, "simple", 1),
        [objective, "add", amount] => (*objective, "add", int(amount)?),
        [objective, "set", amount] => (*objective, "set", int(amount)?),
        _ => return Err(bad(words.first().copied().unwrap_or(""))),
    };
    let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
    let objective = objective_of(&board, objective_name)?;
    if objective.criteria != "trigger" {
        return Err(Out::fail("commands.trigger.failed.invalid", vec![]));
    }
    if board.score(name, objective_name).is_none_or(|score| score.locked) {
        return Err(Out::fail("commands.trigger.failed.unprimed", vec![]));
    }
    let value = board.change_score(name, objective_name, |score| {
        score.locked = true;
        score.value = match mode {
            "set" => amount,
            _ => score.value.wrapping_add(amount),
        };
    });
    let text = objective_text(&objective);
    Ok(match mode {
        "simple" => Out::ok("commands.trigger.simple.success", vec![text], value),
        _ => Out::ok(&format!("commands.trigger.{mode}.success"), vec![text, number(amount)], value),
    })
}

/// `teammsg <сообщение>`: сообщение своей команде (вики, Commands/teammsg).
fn teammsg(words: &[&str], source: &Source, shared: &Shared) -> Answer {
    let Source::Player { uuid, name } = source else {
        return Out::fail("permissions.requires.entity", vec![]).answer();
    };
    let message = words.join(" ");
    let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
    let Some(team) = board.team_of(name).cloned() else {
        return Out::fail("commands.teammsg.failed.noteam", vec![]).answer();
    };
    let team_name = team_text(&team);
    let me = player_component(&board, name, uuid, false);
    drop(board);

    // Отправителю — «-> [команда] <ник> текст», остальным участникам —
    // «[команда] <ник> текст» [проверить: у оригинала это чат игрока с
    // типом team_msg_command, а не системное сообщение].
    let to_others = translate("chat.type.team.text", vec![team_name.clone(), me.clone(), literal(&message)]);
    let to_me = translate("chat.type.team.sent", vec![team_name, me, literal(&message)]);
    let members: Vec<[u8; 16]> = {
        let players = shared.players.lock().expect("список игроков захвачен другим потоком");
        players.members().iter().filter(|member| team.members.contains(&member.name) && &member.uuid != uuid).map(|member| member.uuid).collect()
    };
    shared.scoreboard.lock().expect("табло захвачено другим потоком").send_chat(members, scoreboard::encode_component(&to_others));

    Answer { reply: scoreboard::plain_text(&to_me), effect: Effect::None, component: None, rich: vec![scoreboard::encode_component(&to_me)], result: Some(1) }
}

// ---- execute ----

/// `execute …`: as, if/unless score|entity|block, store result|success
/// score|bossbar, run. Остальные подкоманды — в src/commands.rs (`in`).
fn execute(words: &[&str], source: &Source, shared: &Shared) -> Answer {
    let mut sources = vec![clone_source(source)];
    let mut stores: Vec<(bool, Store)> = Vec::new();
    let mut at = 0;

    while at < words.len() {
        match words[at] {
            "as" => {
                let Some(who) = words.get(at + 1) else {
                    return bad("as").answer();
                };
                let mut next = Vec::new();
                for source in &sources {
                    for (uuid, name) in crate::commands::find_targets(who, source, shared) {
                        next.push(Source::Player { uuid, name });
                    }
                }
                sources = next;
                at += 2;
            }
            // Место исполнения у нас — место игрока-исполнителя: `at @s` ничего
            // не меняет [проверить: `at <другой>` не поддержан].
            "at" if words.get(at + 1) == Some(&"@s") => at += 2,
            mode @ ("if" | "unless") => {
                let wanted = mode == "if";
                let Some((used, test)) = condition(&words[at + 1..]) else {
                    return bad(words.get(at + 1).copied().unwrap_or(mode)).answer();
                };
                at += 1 + used;
                let last = at >= words.len();
                let mut passed = Vec::new();
                let mut count_total = 0;

                for source in sources {
                    match test(&source, shared) {
                        Ok(count) => {
                            if (count > 0) == wanted {
                                count_total += if wanted { count } else { 1 };
                                passed.push(source);
                            }
                        }
                        Err(out) => return out.answer(),
                    }
                }

                if last {
                    // Без `run`: ответ о проверке и её итог (вики, execute:
                    // «if … test passed»).
                    let ok = !passed.is_empty();
                    let result = if ok { count_total.max(1) } else { 0 };
                    for (success_only, store) in &stores {
                        store.write(shared, &passed.first().map(clone_source).unwrap_or_else(|| clone_source(source)), if *success_only { i32::from(ok) } else { result });
                    }
                    return if ok {
                        Out::ok("commands.execute.conditional.pass", vec![], result).answer()
                    } else {
                        Out::fail("commands.execute.conditional.fail", vec![]).answer()
                    };
                }

                sources = passed;
            }
            "store" => {
                let success_only = match words.get(at + 1) {
                    Some(&"result") => false,
                    Some(&"success") => true,
                    other => return bad(other.copied().unwrap_or("store")).answer(),
                };
                match words.get(at + 2..at + 5) {
                    Some(["score", who, objective]) => {
                        stores.push((success_only, Store::Score { who: who.to_string(), objective: objective.to_string() }));
                        at += 5;
                    }
                    Some(["bossbar", id, what @ ("value" | "max")]) => {
                        stores.push((success_only, Store::Bossbar { id: bossbar_id(id), max: *what == "max" }));
                        at += 5;
                    }
                    _ => return bad(words.get(at + 2).copied().unwrap_or("store")).answer(),
                }
            }
            "run" => {
                let command = words[at + 1..].join(" ");
                let mut last = silent(0);
                let mut successes = 0;

                for source in &sources {
                    let answer = crate::commands::run(&command, source, shared);
                    let ok = answer.result.is_some();
                    successes += i32::from(ok);
                    for (success_only, store) in &stores {
                        store.write(shared, source, if *success_only { i32::from(ok) } else { answer.result.unwrap_or(0) });
                    }
                    last = answer;
                }

                if sources.len() > 1 {
                    last.result = (successes > 0).then_some(successes);
                }
                return last;
            }
            other => return bad(other).answer(),
        }
    }

    bad("execute").answer()
}

fn clone_source(source: &Source) -> Source {
    match source {
        Source::Player { uuid, name } => Source::Player { uuid: *uuid, name: name.clone() },
        Source::Console => Source::Console,
        Source::Block { x, y, z, dimension, name } => Source::Block { x: *x, y: *y, z: *z, dimension: *dimension, name: name.clone() },
    }
}

/// Куда записать итог (`execute store`).
enum Store {
    Score { who: String, objective: String },
    Bossbar { id: String, max: bool },
}

impl Store {
    fn write(&self, shared: &Shared, source: &Source, value: i32) {
        match self {
            Store::Score { who, objective } => {
                let Ok(targets) = holders(who, source, shared, true) else {
                    return;
                };
                let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
                if board.objective(objective).is_some_and(|objective| !read_only(&objective.criteria)) {
                    for holder in targets {
                        board.set_score(&holder.key, objective, value);
                    }
                }
            }
            Store::Bossbar { id, max } => {
                let max = *max;
                shared.scoreboard.lock().expect("табло захвачено другим потоком").modify_bossbar(id, Some(2), |bar| if max { bar.max = value.max(1) } else { bar.value = value.max(0) });
            }
        }
    }
}

type Test = Box<dyn Fn(&Source, &Shared) -> Result<i32, Out>>;

/// Условие `if|unless …`: сколько слов заняло и проверка (число совпадений).
fn condition(words: &[&str]) -> Option<(usize, Test)> {
    match words {
        ["score", who, objective, "matches", range, ..] => {
            let (low, high) = int_range(range)?;
            let (who, objective) = (who.to_string(), objective.to_string());
            Some((5, Box::new(move |source, shared| {
                let value = one_score(&who, &objective, source, shared)?;
                Ok(i32::from(value.is_some_and(|value| low.is_none_or(|low| value >= low) && high.is_none_or(|high| value <= high))))
            })))
        }
        ["score", who, objective, operation @ ("<" | "<=" | "=" | ">=" | ">"), other, other_objective, ..] => {
            let (who, objective, other, other_objective, operation) = (who.to_string(), objective.to_string(), other.to_string(), other_objective.to_string(), operation.to_string());
            Some((6, Box::new(move |source, shared| {
                let (Some(mine), Some(theirs)) = (one_score(&who, &objective, source, shared)?, one_score(&other, &other_objective, source, shared)?) else {
                    return Ok(0);
                };
                Ok(i32::from(match operation.as_str() {
                    "<" => mine < theirs,
                    "<=" => mine <= theirs,
                    "=" => mine == theirs,
                    ">=" => mine >= theirs,
                    _ => mine > theirs,
                }))
            })))
        }
        ["entity", who, ..] => {
            let who = who.to_string();
            Some((2, Box::new(move |source, shared| Ok(crate::commands::find_targets(&who, source, shared).len() as i32))))
        }
        ["block", x, y, z, block, ..] => {
            let (x, y, z, block) = (x.to_string(), y.to_string(), z.to_string(), block.to_string());
            Some((5, Box::new(move |source, shared| {
                let origin = crate::commands::origin_of(source, shared);
                let coordinate = |text: &str, from: f64| -> Option<i32> {
                    match text.strip_prefix('~') {
                        Some("") => Some(from.floor() as i32),
                        Some(shift) => Some((from + shift.parse::<f64>().ok()?).floor() as i32),
                        None => text.parse().ok(),
                    }
                };
                let (Some(x), Some(y), Some(z)) = (coordinate(&x, origin.0), coordinate(&y, origin.1), coordinate(&z, origin.2)) else {
                    return Err(bad(&x));
                };
                let dimension = crate::commands::dimension_of(source, shared);
                let (catalog, state) = {
                    let world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
                    (world.catalog(), world.get_block(x, y, z))
                };
                let same = matches_block_predicate(catalog, &block, state).map_err(|_| bad(&block))?;
                Ok(i32::from(same))
            })))
        }
        _ => None,
    }
}

/// Предикат команды сверяет лишь названные свойства, а не состояние с
/// подставленными defaults. Ordinary777: lever[powered=true] совпадает с
/// напольным рычагом; запись wool-sculk-discovery-01/server.log:454.
/// Ошибка — отдельный результат: `unless` не должен превращать её в успех.
fn matches_block_predicate(catalog: crate::gameplay_catalog::Catalog, text: &str, state: i32) -> Result<bool, ()> {
    let (name, properties) = match text.split_once('[') {
        Some((name, rest)) => {
            let properties = rest.strip_suffix(']').ok_or(())?;
            if properties.contains('[') || properties.contains(']') { return Err(()); }
            (name, Some(properties))
        }
        None => (text, None),
    };
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    catalog.default_state(name).map_err(|_| ())?.ok_or(())?;
    let orientation = catalog.orientation(name).map_err(|_| ())?;
    let mut requested = Vec::new();
    if let Some(properties) = properties.filter(|properties| !properties.trim().is_empty()) {
        for pair in properties.split(',') {
            let (key, value) = pair.split_once('=').ok_or(())?;
            let (key, value) = (key.trim(), value.trim());
            if requested.iter().any(|(previous, _)| *previous == key) { return Err(()); }
            let property = orientation.ok_or(())?.properties.iter().find(|property| property.name == key).ok_or(())?;
            if !property.values.contains(&value) { return Err(()); }
            requested.push((key, value));
        }
    }
    // Даже при другом блоке некорректные свойства выше остаются ошибкой.
    let actual = catalog.name(crate::gameplay_catalog::Kind::State, state).map_err(|_| ())?;
    if actual != name { return Ok(false); }
    Ok(requested.into_iter().all(|(key, value)| orientation.is_some_and(|rule| rule.value(state, key) == Some(value))))
}

/// Счёт одного держателя (None — не задан).
fn one_score(who: &str, objective: &str, source: &Source, shared: &Shared) -> Result<Option<i32>, Out> {
    let targets = holders(who, source, shared, false)?;
    let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
    objective_of(&board, objective)?;
    Ok(targets.first().and_then(|holder| board.score(&holder.key, objective)).map(|score| score.value))
}

/// Промежуток `N`, `N..`, `..M`, `N..M`.
pub(crate) fn int_range(text: &str) -> Option<(Option<i32>, Option<i32>)> {
    match text.split_once("..") {
        Some((low, high)) => {
            let low = if low.is_empty() { None } else { Some(low.parse().ok()?) };
            let high = if high.is_empty() { None } else { Some(high.parse().ok()?) };
            (low.is_some() || high.is_some()).then_some((low, high))
        }
        None => {
            let value = text.parse().ok()?;
            Some((Some(value), Some(value)))
        }
    }
}

// ---- выбор игроков с условиями ----

/// Условия выбора `@a[…]`, которые касаются табло: `tag=`, `team=`,
/// `scores={…}` (вики, Target selectors). Остальное разбирает commands.rs.
#[derive(Default, Debug)]
pub(crate) struct BoardFilter {
    tags: Vec<(bool, String)>,
    teams: Vec<(bool, String)>,
    scores: Vec<(String, Option<i32>, Option<i32>)>,
}

impl BoardFilter {
    /// Принять условие; false — не наше.
    pub(crate) fn take(&mut self, key: &str, value: &str) -> Option<bool> {
        let (negated, bare) = match value.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, value),
        };

        match key {
            "tag" => self.tags.push((!negated, bare.to_string())),
            "team" => self.teams.push((!negated, bare.to_string())),
            "scores" => {
                let inner = value.strip_prefix('{')?.strip_suffix('}')?;
                for pair in inner.split(',').filter(|pair| !pair.is_empty()) {
                    let (objective, range) = pair.split_once('=')?;
                    let (low, high) = int_range(range)?;
                    self.scores.push((objective.to_string(), low, high));
                }
            }
            _ => return Some(false),
        }

        Some(true)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.teams.is_empty() && self.scores.is_empty()
    }

    /// Подходит ли игрок.
    pub(crate) fn matches(&self, board: &Scoreboard, uuid: &[u8; 16], name: &str) -> bool {
        let tags = board.tags_of(&scoreboard::tag_key(uuid));
        let tags_ok = self.tags.iter().all(|(wanted, tag)| {
            // `tag=` — без меток вовсе, `tag=!` — с любой меткой.
            let has = if tag.is_empty() { !tags.is_empty() } else { tags.iter().any(|known| known == tag) };
            if tag.is_empty() { has != *wanted } else { has == *wanted }
        });
        let team = board.team_of(name).map(|team| team.name.as_str());
        let teams_ok = self.teams.iter().all(|(wanted, wanted_team)| {
            let has = if wanted_team.is_empty() { team.is_some() } else { team == Some(wanted_team.as_str()) };
            if wanted_team.is_empty() { has != *wanted } else { has == *wanted }
        });
        let scores_ok = self.scores.iter().all(|(objective, low, high)| {
            board.score(name, objective).is_some_and(|score| low.is_none_or(|low| score.value >= low) && high.is_none_or(|high| score.value <= high))
        });

        tags_ok && teams_ok && scores_ok
    }

    /// Сущность-не-игрок: меток, команд и счёта у неё у нас нет.
    pub(crate) fn matches_entity(&self) -> bool {
        self.tags.iter().all(|(wanted, tag)| !*wanted || tag.is_empty() && !*wanted)
            && self.teams.iter().all(|(wanted, team)| !*wanted || team.is_empty())
            && self.scores.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_fills_arguments() {
        assert_eq!(russian("commands.scoreboard.players.get.success", &["a".into(), "5".into(), "[t]".into()]), "У a 5 [t]");
        assert_eq!(fill("%2$s и %1$s", &["a".into(), "b".into()]), "b и a");
    }

    #[test]
    fn criteria_and_names_are_checked() {
        assert!(valid_criteria("dummy"));
        assert!(valid_criteria("minecraft.killed:minecraft.zombie"));
        assert!(valid_criteria("teamkill.red"));
        assert!(!valid_criteria("nosuch"));
        assert!(valid_name("a.b-c+d_1"));
        assert!(!valid_name("a:b"));
    }

    #[test]
    fn floor_division_and_modulo() {
        assert_eq!((-7).wrapping_div_euclid_floor(2), -4);
        assert_eq!(7.wrapping_div_euclid_floor(2), 3);
        assert_eq!((-7).rem_euclid_floor(3), 2);
        assert_eq!(7.rem_euclid_floor(-3), -2);
    }

    #[test]
    fn ranges_and_filters() {
        assert_eq!(int_range("3..5"), Some((Some(3), Some(5))));
        assert_eq!(int_range("..3"), Some((None, Some(3))));
        assert_eq!(int_range("4"), Some((Some(4), Some(4))));
        assert_eq!(int_range(".."), None);

        let mut filter = BoardFilter::default();
        assert_eq!(filter.take("tag", "a"), Some(true));
        assert_eq!(filter.take("scores", "{s=1..}"), Some(true));
        assert_eq!(filter.take("limit", "1"), Some(false));

        let mut board = Scoreboard::new();
        let uuid = [1; 16];
        board.add_objective(Objective { name: "s".into(), criteria: "dummy".into(), display: literal("s"), hearts: false, auto_update: false, format: None });
        assert!(!filter.matches(&board, &uuid, "p"));
        board.add_tag(&scoreboard::tag_key(&uuid), "a");
        board.set_score("p", "s", 2);
        assert!(filter.matches(&board, &uuid, "p"));
    }
}


#[cfg(test)]
mod block_predicate_tests {
    use super::*;
    use crate::{blocks, dimension::Dimension, gameplay_catalog::{Catalog, Profile, SlabHalf, slabs}, world::World};

    fn server(profile: Profile) -> Shared {
        let catalog = Catalog::new(profile);
        let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
            Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/execute-block-predicate-ops.json")), crate::skins::Settings::default());
        // setblock меняет лишь загруженный чанк — как и в настоящем мире.
        shared.world_in(Dimension::Overworld).lock().unwrap().ensure(0, 0);
        assert!(crate::commands::run("scoreboard objectives add predicate dummy", &Source::Console, &shared).result.is_some());
        shared
    }
    fn state(shared: &Shared) -> i32 {
        shared.world_in(Dimension::Overworld).lock().unwrap().get_block(0, 64, 0)
    }
    fn check(shared: &Shared, predicate: &str, matches: bool) {
        let before = state(shared);
        for (mode, passes) in [("if", matches), ("unless", !matches)] {
            let command = format!("execute {mode} block 0 64 0 {predicate}");
            assert_eq!(crate::commands::run(&command, &Source::Console, shared).result,
                passes.then_some(1), "{command}");
            crate::commands::run("scoreboard players set guard predicate 0", &Source::Console, shared);
            crate::commands::run(&format!("{command} run scoreboard players add guard predicate 1"), &Source::Console, shared);
            assert_eq!(shared.scoreboard.lock().unwrap().score("guard", "predicate").unwrap().value, i32::from(passes), "{command} side effect");
            assert_eq!(state(shared), before);
        }
    }
    fn invalid(shared: &Shared, predicate: &str) {
        let before = state(shared);
        for mode in ["if", "unless"] {
            crate::commands::run("scoreboard players set guard predicate 0", &Source::Console, shared);
            let command = format!("execute {mode} block 0 64 0 {predicate}");
            assert_eq!(crate::commands::run(&command, &Source::Console, shared).result, None, "{command}");
            assert_eq!(crate::commands::run(&format!("{command} run scoreboard players add guard predicate 1"), &Source::Console, shared).result,
                None, "неверный предикат не запускает run: {command}");
            assert_eq!(shared.scoreboard.lock().unwrap().score("guard", "predicate").unwrap().value, 0);
            assert_eq!(state(shared), before);
        }
    }

    #[test]
    fn native_partial_lever_properties_do_not_require_omitted_defaults_for_if_or_unless() {
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let shared = server(profile);
            assert!(crate::commands::run("setblock 0 63 0 stone", &Source::Console, &shared).result.is_some());
            assert!(crate::commands::run("setblock 0 64 0 lever[face=floor,facing=east,powered=true]", &Source::Console, &shared).result.is_some());
            let expected = blocks::state_from_text("lever[face=floor,facing=east,powered=true]").unwrap();
            assert_eq!(state(&shared), expected);
            for text in ["lever", "minecraft:lever", "lever[]", "lever[powered=true]", "minecraft:lever[facing=east]", "lever[face=floor]", "lever[face=floor,facing=east,powered=true]"] {
                check(&shared, text, true);
            }
            for text in ["stone", "minecraft:air", "lever[powered=false]", "lever[face=wall]", "lever[facing=north]"] { check(&shared, text, false); }
        }
    }

    #[test]
    fn all_32_slab_names_and_only_requested_properties_use_primary_catalog() {
        let shared = server(Profile::Preview26_3);
        let catalog = shared.catalog();
        let rows: Vec<_> = slabs().collect();
        for (index, row) in rows.iter().enumerate() {
            let other_family = rows[(index + 16) % 32];
            for (half, name) in [(SlabHalf::Top, "top"), (SlabHalf::Bottom, "bottom"), (SlabHalf::Double, "double")] {
                for wet in [false, true] {
                    let text = format!("{}[type={name},waterlogged={wet}]", row.name);
                    assert!(crate::commands::run(&format!("setblock 0 64 0 {text}"), &Source::Console, &shared).result.is_some(), "setup {text}");
                    assert_eq!(state(&shared), catalog.slab_state_for(row, half, wet).unwrap());
                    for query in [row.name.to_owned(), format!("minecraft:{}", row.name), text,
                        format!("{}[type={name}]", row.name), format!("{}[waterlogged={wet}]", row.name)] {
                        check(&shared, &query, true);
                    }
                    check(&shared, &format!("{}[waterlogged={}]", row.name, !wet), false);
                    check(&shared, &format!("{}[type={name}]", other_family.name), false);
                }
            }
        }
        let native = server(Profile::Java26_1_2);
        assert!(crate::commands::run("setblock 0 64 0 stone", &Source::Console, &native).result.is_some());
        assert_eq!(state(&native), blocks::state_by_name("stone").unwrap());
        for row in rows {
            invalid(&native, row.name);
            invalid(&native, &format!("minecraft:{}[type=top]", row.name));
        }
    }

    #[test]
    fn invalid_properties_names_or_syntax_never_succeed_even_for_unless_and_different_actual_block() {
        let shared = server(Profile::Preview26_3);
        assert!(crate::commands::run("setblock 0 64 0 stone", &Source::Console, &shared).result.is_some());
        assert_eq!(state(&shared), blocks::state_by_name("stone").unwrap());
        for text in ["not_a_block", "minecraft:not_a_block", "other:stone", "#minecraft:wool",
            "lever[unknown=true]", "lever[powered=maybe]", "lever[powered=true,powered=true]",
            "lever[", "lever[powered=true]junk", "lever[powered=true][facing=east]", "lever[powered=true,]",
            "stone[powered=true]", "air[type=top]", "white_wool_slab[type=bad]", "white_wool_slab[type=top,type=bottom]"] {
            invalid(&shared, text);
        }
        let catalog = shared.catalog();
        assert!(matches_block_predicate(catalog, "air", i32::MAX).is_err());
        assert!(matches_block_predicate(catalog, "air", -1).is_err());
        assert_eq!(matches_block_predicate(catalog, "stone[powered=true]", blocks::state_by_name("lever").unwrap()), Err(()));
        // Ошибка неизвестного actual state не становится совпадением с воздухом.
        assert_eq!(matches_block_predicate(Catalog::new(Profile::Java26_1_2), "air", rows_first_wool_state()), Err(()));
    }
    fn rows_first_wool_state() -> i32 { crate::gameplay_catalog::WOOL_SLABS[0].first_state }
}
