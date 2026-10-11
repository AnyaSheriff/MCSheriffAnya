// Поводок, узел на заборе и бирка — общее для всех мобов.
//
// Кода игры здесь нет; поведение — со страниц minecraft.wiki (Lead, Name Tag)
// и из чёрного ящика (сценарии lead-attach и name-tag):
//
// - поводок цепляется щелчком по мобу с поводком в руке; клиентам уходит
//   Link Entities (moб → держатель) в такте щелчка, поводок тратится;
// - щелчок по забору переносит все поводки игрока (мобы в 7 блоках) на узел:
//   узел появляется на заборе, звук item.lead.tied — в точке узла;
// - дальше `SNAP` блоков поводок рвётся: звук item.lead.break в точке
//   держателя, поводок падает у моба, связь снимается (держатель 0);
// - дальше `PULL` блоков моба тянет к держателю [проверить: сила тяги].
//
// Узел — `Kind::LeashKnot` в списке мобов: так его видит слежение. Без
// привязанных мобов узел пропадает (вики, Lead: «the knot disappears»).

use super::{Event, Kind, Mob};

/// Дальше этого (блоков) поводок рвётся (вики, Lead: «more than 12 blocks»
/// с 1.21.6; чёрный ящик, lead-attach: на 17 блоках рвётся в такте ухода).
pub const SNAP: f64 = 12.0;

/// Дальше этого моба тянет к держателю (вики, Lead: «6 blocks») [проверить].
pub const PULL: f64 = 6.0;

/// Мобы на поводке игрока в стольких блоках переходят на узел при щелчке
/// по забору (вики, Lead: «within a 7 block radius»).
pub const KNOT_REACH: f64 = 7.0;

/// Звуки поводка (чёрный ящик, lead-attach).
pub const TIED: &str = "minecraft:item.lead.tied";
pub const BROKEN: &str = "minecraft:item.lead.break";

/// Кто держит поводок.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    /// Игрок или торговец: номер (0 — не найден после загрузки) и UUID.
    Player { id: i32, uuid: [u8; 16] },

    /// Узел на заборе: номер сущности узла (0 — ещё не заведён) и блок.
    Knot { id: i32, at: (i32, i32, i32) },
}

impl Holder {
    /// Номер сущности держателя (0 — неизвестен).
    pub fn id(&self) -> i32 {
        match self {
            Holder::Player { id, .. } | Holder::Knot { id, .. } => *id,
        }
    }
}

/// Точка узла на заборе: середина блока, 0,375 над низом (чёрный ящик,
/// lead-attach: звук item.lead.tied в x+0,5, y+0,375, z+0,5).
pub fn knot_point((x, y, z): (i32, i32, i32)) -> (f64, f64, f64) {
    (f64::from(x) + 0.5, f64::from(y) + 0.375, f64::from(z) + 0.5)
}

/// Новый узел на заборе.
pub fn new_knot(entity_id: i32, at: (i32, i32, i32)) -> Mob {
    let mut knot = Mob::new(entity_id, Kind::LeashKnot, knot_point(at));

    knot.on_ground = false;
    knot
}

/// Блок узла (у моба-узла — по его точке).
pub fn knot_block(knot: &Mob) -> (i32, i32, i32) {
    (knot.x.floor() as i32, knot.y.floor() as i32, knot.z.floor() as i32)
}

/// Привязать моба к держателю: связь уходит клиентам событием.
pub fn attach(mob: &mut Mob, holder: Holder) {
    mob.leash = Some(holder);
    mob.events.push(Event::Leash(holder.id()));
}

/// Можно ли взять моба на поводок (вики, Lead: «most passive and neutral
/// mobs»; враждебных — нет) [проверить: полный список].
pub fn leashable(kind: Kind) -> bool {
    if super::horse::equine(kind) { return true; }
    // Зоглин на поводок берётся (копия), прочие враждебные — нет.
    (kind == Kind::Zoglin || !kind.hostile()) && !kind.object() && !super::water::fish(kind)
        && !matches!(kind, Kind::EnderDragon | Kind::Villager | Kind::WanderingTrader | Kind::ZombieVillager | Kind::Turtle | Kind::Tadpole | Kind::Bat)
}

/// Что сделал поводок за такт.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Held,
    /// Порвался: точка держателя (там звучит обрыв).
    Broke((f64, f64, f64)),
}

/// Такт поводка: держатель в `holder_at` (None — держателя нет в мире).
/// Рвётся дальше `SNAP` и без держателя (снимает тогда `step_all`); дальше
/// `PULL` тянет к держателю.
pub fn step(mob: &mut Mob, holder_at: Option<(f64, f64, f64)>) -> Step {
    let Some(at) = holder_at else {
        return Step::Broke((mob.x, mob.y, mob.z));
    };

    let (dx, dy, dz) = (at.0 - mob.x, at.1 - mob.y, at.2 - mob.z);
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();

    if distance > SNAP {
        return Step::Broke(at);
    }

    // Тяга: упругая, пропорционально лишней длине [проверить: у оригинала
    // с 1.21.6 своя «упругая» модель].
    if distance > PULL && !mob.no_ai {
        let pull = (distance - PULL) * 0.04 / distance;

        mob.vx += dx * pull;
        mob.vy += dy * pull * 0.5;
        mob.vz += dz * pull;
    }

    Step::Held
}

/// Снять поводок: связь 0 клиентам и (если `drop`) сам поводок у моба.
pub fn drop_leash(mob: &mut Mob, drop: bool) {
    if mob.leash.take().is_none() {
        return;
    }

    if drop && let Some(lead) = crate::blocks::item_named("lead") {
        mob.events.push(Event::Loot(vec![crate::inventory::Stack::new(lead, 1)]));
    }

    mob.events.push(Event::Leash(0));
}

/// Поводки за такт всех мобов: держатели — игроки (номер, место) и узлы из
/// того же списка. Порвавшиеся звучат в точке держателя; узлы без мобов
/// пропадают. Возвращает точки обрывов (для проверок).
pub fn step_all(mobs: &mut [Mob], players: &[(i32, (f64, f64, f64))]) -> Vec<(f64, f64, f64)> {
    let knots: Vec<(i32, (f64, f64, f64))> = mobs.iter().filter(|mob| mob.kind == Kind::LeashKnot && !mob.h.despawn).map(|mob| (mob.entity_id, (mob.x, mob.y, mob.z))).collect();
    let holders: Vec<_> = mobs.iter().filter(|m| !m.is_dead() && !m.h.despawn).map(|m| (m.entity_id,m.uuid,(m.x,m.y,m.z))).collect();
    let mut broke = Vec::new();

    for mob in mobs.iter_mut() {
        let Some(holder) = mob.leash else {
            continue;
        };

        // Мёртвый роняет поводок (вики, Lead: «drops when the mob dies»).
        if mob.is_dead() {
            drop_leash(mob, true);
            continue;
        }

        let at = match holder {
            Holder::Player { id, uuid } => players.iter().find(|(player, _)| *player == id).map(|(_, at)| (at.0, at.1, at.2)).or_else(|| holders.iter().find(|h| h.1 == uuid).map(|h| h.2)),
            Holder::Knot { id, .. } => knots.iter().find(|(knot, _)| *knot == id).map(|(_, at)| *at),
        };

        // Держатель ещё не найден после загрузки (номер 0): ждём [проверить].
        if holder.id() == 0 {
            continue;
        }

        // Обрыв: сперва звук у держателя, потом поводок падает и связь
        // снимается (чёрный ящик, lead-attach: такой порядок в такте).
        if let Step::Broke(place) = step(mob, at) {
            mob.events.push(Event::SoundAt { name: BROKEN, volume: 1.0, pitch: 1.0, at: place });
            drop_leash(mob, true);
            broke.push(place);
        }
    }

    // Узел без единого моба пропадает.
    let used: Vec<i32> = mobs.iter().filter_map(|mob| mob.leash).map(|holder| holder.id()).collect();

    for knot in mobs.iter_mut().filter(|mob| mob.kind == Kind::LeashKnot) {
        if !used.contains(&knot.entity_id) && knot.ticks > 0 {
            knot.h.despawn = true;
        }
    }

    broke
}

/// Привязанные после загрузки к узлу: узел по блоку получает номер.
pub fn bind_knots(mobs: &mut [Mob]) {
    let knots: Vec<(i32, (i32, i32, i32))> = mobs.iter().filter(|mob| mob.kind == Kind::LeashKnot).map(|mob| (mob.entity_id, knot_block(mob))).collect();

    for mob in mobs.iter_mut() {
        if let Some(Holder::Knot { id: 0, at }) = mob.leash
            && let Some((id, _)) = knots.iter().find(|(_, place)| *place == at)
        {
            mob.leash = Some(Holder::Knot { id: *id, at });
        }
    }
}

/// Хвост записи моба: имя, «не исчезать», поводок.
pub fn write_tail(mob: &Mob, out: &mut Vec<u8>) {
    let name = mob.name.as_deref().unwrap_or("");

    let nether = matches!(mob.kind, Kind::Hoglin | Kind::Piglin | Kind::PiglinBrute);
    // Бит 16 — «не взрослеть» (AgeLocked), без своих байтов.
    out.push(u8::from(mob.name.is_some()) | (u8::from(mob.persistent_tag) << 1) | (u8::from(nether) << 2) | (u8::from(mob.h.trial_gear.is_some()) << 3) | (u8::from(mob.age_locked) << 4));
    out.extend_from_slice(&(name.len() as u16).to_be_bytes());
    out.extend_from_slice(name.as_bytes());

    match mob.leash {
        None => out.push(0),
        Some(Holder::Player { uuid, .. }) => {
            out.push(1);
            out.extend_from_slice(&uuid);
        }
        Some(Holder::Knot { at, .. }) => {
            out.push(2);

            for value in [at.0, at.1, at.2] {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
    if nether {
        out.extend_from_slice(&mob.newmobs.outside.to_be_bytes());
        out.push(u8::from(mob.nether.immune));
    }
    if let Some(gear)=&mob.h.trial_gear {super::trial_gear::write(gear,out);}
}

/// Прочитанный хвост.
pub struct Tail {
    name: Option<String>,
    persistent: bool,
    age_locked: bool,
    leash: Option<Holder>,
    nether: Option<(i32,bool)>,
    trial_gear: Option<super::trial_gear::Gear>,
}

/// Читает хвост `write_tail`; None — испорчен.
pub fn read_tail(bytes: &mut &[u8]) -> Option<Tail> {
    let mut take = |count: usize| -> Option<&[u8]> {
        let (head, rest) = bytes.split_at_checked(count)?;
        *bytes = rest;
        Some(head)
    };

    let flags = take(1)?[0];
    let length = u16::from_be_bytes(take(2)?.try_into().ok()?) as usize;
    let name = String::from_utf8(take(length)?.to_vec()).ok()?;
    let leash = match take(1)?[0] {
        0 => None,
        1 => Some(Holder::Player { id: 0, uuid: take(16)?.try_into().ok()? }),
        2 => {
            let mut at = [0i32; 3];

            for value in &mut at {
                *value = i32::from_be_bytes(take(4)?.try_into().ok()?);
            }

            Some(Holder::Knot { id: 0, at: (at[0], at[1], at[2]) })
        }
        _ => return None,
    };

    let nether=if flags & 4 != 0 {Some((i32::from_be_bytes(take(4)?.try_into().ok()?),take(1)?[0]!=0))} else {None};
    let trial_gear=if flags & 8 != 0 {Some(super::trial_gear::read(bytes)?)} else {None};
    Some(Tail { trial_gear, name: (flags & 1 != 0).then_some(name), persistent: flags & 2 != 0, age_locked: flags & 16 != 0, leash, nether })
}

/// Кладёт прочитанный хвост в моба.
pub fn apply_tail(mob: &mut Mob, tail: Tail) {
    mob.h.trial_gear=tail.trial_gear;
    mob.name = tail.name;
    mob.persistent_tag = tail.persistent;
    mob.age_locked = tail.age_locked;
    mob.leash = tail.leash;
    if let Some((outside,immune))=tail.nether {mob.newmobs.outside=outside;mob.nether.immune=immune;}
}

/// Бирка: имя мобу (вики, Name Tag: «renames the mob»; чёрный ящик, name-tag:
/// свойство 2 в такте щелчка, бирка уходит). Игрока и дракона не переименовать;
/// то же имя — бирка не тратится [проверить]. Возвращает, ушла ли бирка.
pub fn name_tag(mob: &mut Mob, name: Option<&str>) -> bool {
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        return false;
    };

    if mob.kind.object() || mob.name.as_deref() == Some(name) {
        return false;
    }

    mob.name = Some(name.to_string());
    mob.persistent_tag = true;
    true
}
