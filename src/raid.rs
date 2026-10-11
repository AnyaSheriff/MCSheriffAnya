//! Набеги и патрули разбойников (вики, Raid, Bad Omen, Raid Omen, Hero of
//! the Village, Patrol).
//!
//! Набег живёт в мире Обычного измерения (`World::raids`). Начинается он
//! так: игрок с «Дурным знамением» входит в деревню — эффект сменяется
//! «Знамением набега» на 30 секунд (network/play/vitals.rs), а когда тот
//! кончается, игрок оставляет заявку (`Raids::requests`). Такт мира
//! (tick.rs → `tick`) разбирает заявки, ведёт волны, полосу босса, героев
//! и празднование.
//!
//! Деревня у нас — места работы, занятые жителями (villager.rs): кроватей и
//! колоколов жители пока не занимают, поэтому «точками интереса» считаются
//! только места работы (вики, Raid: «claimed POI (bed, job site or bell)»)
//! [проверить: кровати и колокола].

use crate::mob::ai::Watcher;
use crate::mob::{Kind, Mob, Rng, View};

/// «Знамение набега» длится 30 секунд (вики, Raid Omen: «30 seconds»).
pub const RAID_OMEN_TICKS: i32 = 600;

/// Пауза перед волной: полоса наполняется 15 секунд (вики, Raid: «300 ticks»).
pub const PRE_WAVE: i32 = 300;

/// Кто видит полосу и кто присоединяется к набегу (вики, Raid: «within 96
/// blocks»); дальше 112 блоков разбойник выходит из набега.
pub const BAR_RANGE: f64 = 96.0;
pub const LEAVE_RANGE: f64 = 112.0;

/// Набег кончается сам через 48 000 тактов (вики, Raid: «40 minutes»).
pub const MAX_ACTIVE: u64 = 48_000;

/// Празднование после победы или поражения (вики, Raid: «600 ticks»).
pub const CELEBRATION: i32 = 600;

/// «Герой деревни» на 40 минут (вики, Hero of the Village: «40:00»).
pub const HERO_TICKS: i32 = 48_000;

/// Где искать занятые места вокруг игрока для середины набега (вики, Raid:
/// «claimed POIs within 64 blocks»).
pub const POI_RANGE: f64 = 64.0;

/// Звон колокола подсвечивает разбойников в 48 блоках на 3 секунды (вики,
/// Bell: «raiders within 48 blocks … glowing») [проверить: срок].
pub const BELL_RANGE: f64 = 48.0;
pub const BELL_GLOW: i32 = 60;

/// Патрули: попытка раз в 12 000–13 199 тактов, с пятого дня, шанс 1/5,
/// в 24–48 блоках от игрока (вики, Pillager, «Patrols») [проверить: шанс].
pub const PATROL_WAIT: i32 = 12_000;
pub const PATROL_SPREAD: u32 = 1_200;
pub const PATROL_DAYS: i64 = 5;

/// Заявка на набег: игрок, у которого кончилось «Знамение набега».
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub player: i32,
    pub at: (f64, f64, f64),
    /// Уровень знамения: 1…5.
    pub level: i32,
}

/// Чем кончился набег.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ongoing,
    Victory,
    Loss,
    Stopped,
}

/// Один набег.
#[derive(Clone, Debug)]
pub struct Raid {
    pub id: u32,
    pub center: (f64, f64, f64),
    /// Уровень знамения 1…5: от него лишняя волна и сила героя.
    pub level: i32,
    /// Сколько всего волн (с лишней) и сколько уже вышло.
    pub waves: i32,
    pub spawned: i32,
    /// Пауза до следующей волны.
    pub cooldown: i32,
    pub active: u64,
    pub status: Status,
    /// Тактов празднования осталось (после победы или поражения).
    pub post: i32,
    /// Номера разбойников этой волны и тех, кто присоединился.
    pub raiders: Vec<i32>,
    /// Здоровье всей волны в момент её выхода (для доли полосы).
    pub total_health: f32,
    /// Игроки (номера сущностей), убившие разбойника этого набега.
    pub heroes: Vec<i32>,
    /// Кому полоса уже показана.
    pub shown: Vec<i32>,
    /// Что на полосе сейчас: доля и заголовок.
    pub share: f32,
    pub title: Title,
    /// Найденное место следующей волны.
    pub spot: Option<(f64, f64, f64)>,
}

/// Заголовок полосы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    Raid,
    Remaining(usize),
    Victory,
    Defeat,
}

/// Все набеги измерения, заявки и звоны колоколов.
#[derive(Clone, Debug)]
pub struct Raids {
    pub list: Vec<Raid>,
    pub requests: Vec<Request>,
    /// Звоны колоколов этого такта (redstone/components.rs и щелчок).
    pub bells: Vec<(i32, i32, i32)>,
    pub next_id: u32,
    /// Тактов до следующей попытки патруля.
    pub patrol_wait: i32,
}

impl Default for Raids {
    fn default() -> Self {
        Self { list: Vec::new(), requests: Vec::new(), bells: Vec::new(), next_id: 1, patrol_wait: PATROL_WAIT }
    }
}

/// Что такт набегов просит сделать миру.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Пакеты одному игроку (номер сущности, тело пакета).
    pub private: Vec<(i32, Vec<u8>)>,
    /// Эффект игроку (герой деревни).
    pub effects: Vec<(i32, crate::survival::Effect)>,
    /// Эффект мобу (свечение после звона колокола).
    pub mob_effects: Vec<(i32, crate::survival::Effect)>,
    /// Новые мобы (волна, патруль).
    pub born: Vec<Mob>,
    /// Звук у колокола: место.
    pub resonate: Vec<(i32, i32, i32)>,
}

/// Правила, от которых зависит такт.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub difficulty: u8,
    pub raids: bool,
    pub patrols: bool,
    /// Номер дня мира (часы / 24 000).
    pub day: i64,
}

/// Сколько волн на сложности (вики, Raid: «Easy 3, Normal 5, Hard 7»).
pub fn waves_for(difficulty: u8) -> i32 {
    match difficulty {
        0 => 0,
        1 => 3,
        2 => 5,
        _ => 7,
    }
}

/// Основа волны по номеру 1…7 (вики, Raid, «Wave composition»: столбцы
/// таблицы без случайной добавки): разбойники, поборники, разорители,
/// ведьмы, заклинатели.
const PILLAGERS: [i32; 8] = [0, 4, 3, 3, 4, 4, 4, 2];
const VINDICATORS: [i32; 8] = [0, 0, 2, 0, 1, 4, 2, 5];
const RAVAGERS: [i32; 8] = [0, 0, 0, 1, 0, 1, 0, 2];
const WITCHES: [i32; 8] = [0, 0, 0, 0, 3, 0, 0, 1];
const EVOKERS: [i32; 8] = [0, 0, 0, 0, 0, 1, 1, 2];

/// Состав волны: вид и сколько. Лишняя волна — как последняя по сложности
/// (вики, Raid: «the same strength as the final wave»). Случайная добавка
/// (вики, Raid): на лёгкой 1/4 — ещё разбойник, с третьей волны 1/4 —
/// поборник; на обычной 1/2 — разбойник и 1/2 — поборник; на трудной —
/// один из двух; ведьма с третьей волны 1/2 на обычной и трудной
/// [проверить: формула добавки].
pub fn wave(number: i32, difficulty: u8, last: i32, rng: &mut Rng) -> Vec<(Kind, i32)> {
    let index = number.min(last).clamp(1, 7) as usize;
    let mut pillagers = PILLAGERS[index];
    let mut vindicators = VINDICATORS[index];
    let mut witches = WITCHES[index];

    match difficulty {
        1 => {
            pillagers += i32::from(index >= 2 && rng.below(4) == 0);
            vindicators += i32::from(index >= 3 && rng.below(4) == 0);
        }
        2 => {
            pillagers += i32::from(rng.below(2) == 0);
            vindicators += i32::from(rng.below(2) == 0);
            witches += i32::from(matches!(index, 3 | 5 | 6) && rng.below(2) == 0);
        }
        _ => {
            pillagers += rng.below(3) as i32;
            vindicators += rng.below(3) as i32;
            witches += i32::from(matches!(index, 3 | 5 | 6) && rng.below(2) == 0);
        }
    }

    [(Kind::Pillager, pillagers), (Kind::Vindicator, vindicators), (Kind::Ravager, RAVAGERS[index]), (Kind::Witch, witches), (Kind::Evoker, EVOKERS[index])]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect()
}

/// Участник набега: разбойники, разоритель, ведьма (вики, Raid).
pub fn joins(kind: Kind) -> bool {
    crate::mob::illager::raider(kind) || kind == Kind::Witch
}

/// Середина набега: среднее занятых мест в 64 блоках (вики, Raid).
pub fn center_of(pois: &[(i32, i32, i32)], at: (f64, f64, f64)) -> Option<(f64, f64, f64)> {
    let near: Vec<&(i32, i32, i32)> = pois.iter().filter(|poi| dist2((f64::from(poi.0) + 0.5, f64::from(poi.1), f64::from(poi.2) + 0.5), at) <= POI_RANGE * POI_RANGE).collect();

    if near.is_empty() {
        return None;
    }

    let count = near.len() as f64;
    let sum = near.iter().fold((0.0, 0.0, 0.0), |sum, poi| (sum.0 + f64::from(poi.0), sum.1 + f64::from(poi.1), sum.2 + f64::from(poi.2)));

    Some(((sum.0 / count).floor() + 0.5, (sum.1 / count).floor(), (sum.2 / count).floor() + 0.5))
}

/// В деревне ли место: занятое место в соседних секциях 3 × 3 × 3 (вики,
/// Raid: «3×3×3 subchunk region containing a claimed POI»).
pub fn in_village(pois: &[(i32, i32, i32)], at: (f64, f64, f64)) -> bool {
    let section = |value: f64| (value.floor() as i32).div_euclid(16);
    let here = (section(at.0), section(at.1), section(at.2));

    pois.iter().any(|poi| {
        let there = (poi.0.div_euclid(16), poi.1.div_euclid(16), poi.2.div_euclid(16));

        (there.0 - here.0).abs() <= 1 && (there.1 - here.1).abs() <= 1 && (there.2 - here.2).abs() <= 1
    })
}

fn dist2(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)
}

/// UUID полосы набега: свой у каждого набега.
fn bar_uuid(id: u32) -> [u8; 16] {
    let mut uuid = [0x52, 0x41, 0x49, 0x44, 0x7e, 0x11, 0x40, 0x00, 0x80, 0, 0, 0, 0, 0, 0, 0];

    uuid[12..16].copy_from_slice(&id.to_be_bytes());
    uuid
}

/// Заголовок полосы текстом (вики, Raid: «Raid», «Raid - Victory», «Raid -
/// Defeat», число оставшихся при меньше трёх) [проверить: вид составного текста].
fn title_value(title: Title) -> serde_json::Value {
    let raid = serde_json::json!({ "translate": "event.minecraft.raid" });

    match title {
        Title::Raid => raid,
        Title::Remaining(count) => serde_json::json!({ "text": "", "extra": [raid, " - ", { "translate": "event.minecraft.raid.raiders_remaining", "with": [count.to_string()] }] }),
        Title::Victory => serde_json::json!({ "translate": "event.minecraft.raid.victory.full" }),
        Title::Defeat => serde_json::json!({ "translate": "event.minecraft.raid.defeat.full" }),
    }
}

/// Пакеты полосы (Boss Event): добавить — красная, деления по 10 (вики, Raid:
/// «red … segmented into tenths»).
fn bar_add(raid: &Raid) -> Vec<u8> {
    use crate::network::varint::encode_varint;

    let mut body = encode_varint(crate::scoreboard::BOSS_PACKET);
    body.extend_from_slice(&bar_uuid(raid.id));
    body.extend(encode_varint(0));
    body.extend(crate::scoreboard::encode_component(&title_value(raid.title)));
    body.extend_from_slice(&raid.share.to_be_bytes());
    body.extend(encode_varint(2));
    body.extend(encode_varint(2));
    body.push(0);
    body
}

fn bar_update(raid: &Raid, action: i32) -> Vec<u8> {
    use crate::network::varint::encode_varint;

    let mut body = encode_varint(crate::scoreboard::BOSS_PACKET);
    body.extend_from_slice(&bar_uuid(raid.id));
    body.extend(encode_varint(action));

    match action {
        2 => body.extend_from_slice(&raid.share.to_be_bytes()),
        3 => body.extend(crate::scoreboard::encode_component(&title_value(raid.title))),
        _ => {}
    }

    body
}

/// Звук рога одному игроку: в 13 блоках от него в сторону волны (вики,
/// Raid: «plays from the wave's spawn direction, 13 blocks from the player»;
/// громкость 64 — слышно издалека) [проверить: громкость и раздел].
fn horn(player: &Watcher, from: (f64, f64, f64), seed: i64) -> Vec<u8> {
    use crate::network::varint::{encode_string, encode_varint};

    let (dx, dz) = (from.0 - player.x, from.2 - player.z);
    let length = (dx * dx + dz * dz).sqrt().max(1.0e-6);
    let at = (player.x + dx / length * 13.0, player.y, player.z + dz / length * 13.0);
    let mut body = encode_varint(0x75);

    body.extend(encode_varint(0));
    body.extend(encode_string("minecraft:event.raid.horn"));
    body.push(0);
    body.extend(encode_varint(6));

    for value in [at.0, at.1, at.2] {
        body.extend_from_slice(&((value * 8.0) as i32).to_be_bytes());
    }

    body.extend_from_slice(&64.0f32.to_be_bytes());
    body.extend_from_slice(&1.0f32.to_be_bytes());
    body.extend_from_slice(&seed.to_be_bytes());
    body
}

/// Поиск места для волны (вики, Raid, Java: «8 attempts … every 5 ticks»
/// за паузу, «starts checking far from the raid center» и ближе к концу;
/// в загруженном чанке, не в деревне, по высоте ближе 96 к середине,
/// «only the topmost non-air blocks», пол годен для появления — стекло нет).
/// Расстояния 64 → 48 → 32 по третям паузы — [проверить]: у оригинала
/// (raid-start-wave1) разбойники показались роботу не ближе 62 блоков.
pub const SPOT_EVERY: i32 = 5;
pub const SPOT_TRIES: usize = 8;
pub const LAST_GROUPS: usize = 6;
pub const LAST_TRIES: usize = 20;

pub fn spot_radius(elapsed: i32) -> f64 {
    match elapsed * 3 / PRE_WAVE {
        0 => 64.0,
        1 => 48.0,
        _ => 32.0,
    }
}

fn spawn_spot(view: &dyn View, center: (f64, f64, f64), pois: &[(i32, i32, i32)], radius: f64, tries: usize, rng: &mut Rng) -> Option<(f64, f64, f64)> {
    for _ in 0..tries {
        let angle = rng.unit() * std::f64::consts::TAU;
        let x = (center.0 + angle.cos() * radius).floor() as i32 + rng.below(5) as i32;
        let z = (center.2 + angle.sin() * radius).floor() as i32 + rng.below(5) as i32;

        if !view.loaded(x.div_euclid(16), z.div_euclid(16)) {
            continue;
        }

        // Самый верхний непустой блок в пределах ±96 от середины.
        let bottom = center.1.floor() as i32 - 96;
        let mut y = center.1.floor() as i32 + 96;

        while y > bottom && view.block(x, y - 1, z) == 0 {
            y -= 1;
        }

        let at = (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5);
        let floor = crate::blocks::block_at_state(view.block(x, y - 1, z)).unwrap_or("");

        if y > bottom && !floor.contains("glass") && crate::mob::spawn::spawnable(view, x, y, z, 2) && crate::mob::spawn::space::dry_kind(view, Kind::Ravager, at) && !in_village(pois, at) {
            return Some(at);
        }
    }

    None
}

/// Такт набегов и патрулей: заявки, волны, полоса, конец набега.
#[allow(clippy::too_many_arguments)]
pub fn tick(raids: &mut Raids, mobs: &mut [Mob], view: &dyn View, players: &[Watcher], pois: &[(i32, i32, i32)], rules: Rules, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Outcome {
    let mut out = Outcome::default();

    // Заявки: середина по занятым местам; рядом уже идёт набег — он
    // становится сильнее (вики, Raid Omen: уровень до V).
    for request in std::mem::take(&mut raids.requests) {
        if !rules.raids || rules.difficulty == 0 {
            continue;
        }

        let Some(center) = center_of(pois, request.at) else { continue };

        if let Some(raid) = raids.list.iter_mut().find(|raid| raid.status == Status::Ongoing && dist2(raid.center, center) <= BAR_RANGE * BAR_RANGE) {
            raid.level = (raid.level + request.level).min(5);
            raid.waves = waves_for(rules.difficulty) + i32::from(raid.level > 1);
            continue;
        }

        let level = request.level.clamp(1, 5);
        let id = raids.next_id;

        raids.next_id += 1;
        raids.list.push(Raid {
            id,
            center,
            level,
            waves: waves_for(rules.difficulty) + i32::from(level > 1),
            spawned: 0,
            cooldown: PRE_WAVE,
            active: 0,
            status: Status::Ongoing,
            post: 0,
            raiders: Vec::new(),
            total_health: 0.0,
            heroes: Vec::new(),
            shown: Vec::new(),
            share: 0.0,
            title: Title::Raid,
            spot: None,
        });
    }

    for raid in &mut raids.list {
        step(raid, mobs, view, players, pois, rules, rng, next_id, &mut out);
    }

    // Окончившиеся набеги убираются, когда празднование прошло.
    raids.list.retain(|raid| raid.status == Status::Ongoing || raid.post > 0 || !raid.shown.is_empty());

    bells(raids, mobs, &mut out);
    patrols(raids, view, players, rules, rng, next_id, &mut out);

    out
}

/// Ход одного набега.
#[allow(clippy::too_many_arguments)]
fn step(raid: &mut Raid, mobs: &mut [Mob], view: &dyn View, players: &[Watcher], pois: &[(i32, i32, i32)], rules: Rules, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32, out: &mut Outcome) {
    let before = (raid.share, raid.title);

    raid.active += 1;

    if raid.status == Status::Ongoing {
        // Ушедшие далеко и мёртвые выходят из набега; убитые игроком
        // делают его героем (вики, Hero of the Village: «killed at least one raider»).
        for mob in mobs.iter_mut().filter(|mob| mob.illager.raid == raid.id) {
            if mob.is_dead() && mob.by_player && mob.illager.hit_by != 0 && !raid.heroes.contains(&mob.illager.hit_by) {
                raid.heroes.push(mob.illager.hit_by);
            }

            if !mob.is_dead() && dist2((mob.x, mob.y, mob.z), raid.center) > LEAVE_RANGE * LEAVE_RANGE {
                mob.illager.raid = 0;
            }
        }

        raid.raiders.retain(|id| mobs.iter().any(|mob| mob.entity_id == *id && mob.illager.raid == raid.id && !mob.is_dead()));

        // Деревни больше нет — поражение (вики, Raid: «the village stops
        // counting as a village»); вышел срок — набег молча кончается.
        if raid.active > MAX_ACTIVE {
            raid.status = Status::Stopped;
        } else if !pois.iter().any(|poi| dist2((f64::from(poi.0) + 0.5, f64::from(poi.1), f64::from(poi.2) + 0.5), raid.center) <= POI_RANGE * POI_RANGE) {
            raid.status = Status::Loss;
            raid.post = CELEBRATION;
            raid.title = Title::Defeat;
        } else if raid.raiders.is_empty() && raid.spawned >= raid.waves && raid.spawned > 0 {
            raid.status = Status::Victory;
            raid.post = CELEBRATION;
            raid.title = Title::Victory;

            // Герой деревни: уровень набега, 40 минут.
            for hero in &raid.heroes {
                out.effects.push((*hero, crate::survival::Effect { kind: crate::survival::EffectKind::HeroOfTheVillage, amplifier: raid.level - 1, ticks_left: HERO_TICKS, ambient: false, visible: true, show_icon: true }));
            }
        } else if raid.raiders.is_empty() {
            sweat(raid, mobs, rng);

            // Пауза перед волной: полоса наполняется.
            raid.cooldown -= 1;
            raid.share = ((PRE_WAVE - raid.cooldown) as f32 / PRE_WAVE as f32).clamp(0.0, 1.0);
            raid.title = Title::Raid;

            let elapsed = PRE_WAVE - raid.cooldown;

            if raid.spot.is_none() && elapsed % SPOT_EVERY == 0 {
                raid.spot = spawn_spot(view, raid.center, pois, spot_radius(elapsed), SPOT_TRIES, rng);
            }

            if raid.cooldown <= 0 {
                // Последняя надежда: 6 кругов по 20 мест, ближе к середине; не
                // нашлось — набег кончается без победы (вики, Raid).
                // Место могло затопить за паузу; ящик разорителя покрывает
                // размеры остальных участников волны (вики, Raid: «ravager»).
                let spot = raid.spot.take().filter(|at| crate::mob::spawn::space::dry_kind(view, Kind::Ravager, *at)).or_else(|| (0..LAST_GROUPS).find_map(|group| spawn_spot(view, raid.center, pois, 32.0 - 4.0 * group as f64, LAST_TRIES, rng)));

                match spot {
                    Some(spot) => spawn_wave(raid, spot, mobs, players, rules, rng, next_id, out),
                    None => raid.status = Status::Stopped,
                }

                raid.cooldown = PRE_WAVE;
            }
        } else {
            let health: f32 = raid.raiders.iter().filter_map(|id| mobs.iter().find(|mob| mob.entity_id == *id)).map(|mob| mob.vitals.health.max(0.0)).sum();

            raid.share = if raid.total_health > 0.0 { (health / raid.total_health).clamp(0.0, 1.0) } else { 0.0 };
            raid.title = if raid.raiders.len() <= 2 { Title::Remaining(raid.raiders.len()) } else { Title::Raid };

            sweat(raid, mobs, rng);

            // Разбойники идут к середине деревни (mob/illager.rs).
            for mob in mobs.iter_mut().filter(|mob| mob.illager.raid == raid.id) {
                mob.illager.raid_center = Some(raid.center);
            }
        }
    } else {
        raid.post -= 1;

        // Поражение: разбойники празднуют (свойство 16, вики, Raid).
        if raid.status == Status::Loss {
            for mob in mobs.iter_mut().filter(|mob| mob.illager.raid == raid.id) {
                mob.illager.celebrating = raid.post > 0;
            }
        }
    }

    // Полоса: видят игроки в 96 блоках; по окончании — убирается.
    let alive = raid.status == Status::Ongoing || raid.post > 0;
    let near: Vec<i32> = if alive {
        players.iter().filter(|player| !player.spectator && dist2((player.x, player.y, player.z), raid.center) <= BAR_RANGE * BAR_RANGE).map(|player| player.id).collect()
    } else {
        Vec::new()
    };

    for gone in raid.shown.iter().filter(|id| !near.contains(id)) {
        out.private.push((*gone, bar_update(raid, 1)));
    }

    for player in &near {
        if !raid.shown.contains(player) {
            // Новая полоса приходит с прежней долей, а доля этого такта —
            // следом (чёрный ящик, raid-start-wave1: 0, затем 0,003).
            let now = (raid.share, raid.title);

            (raid.share, raid.title) = before;
            out.private.push((*player, bar_add(raid)));
            (raid.share, raid.title) = now;

            if raid.share != before.0 {
                out.private.push((*player, bar_update(raid, 2)));
            }
        } else {
            if raid.share != before.0 {
                out.private.push((*player, bar_update(raid, 2)));
            }

            if raid.title != before.1 {
                out.private.push((*player, bar_update(raid, 3)));
            }
        }
    }

    raid.shown = near;
}

/// Жители в 96 блоках от середины идущего набега «потеют» — событие 42
/// (вики, Raid: «emit "sweat particles"»; чёрный ящик, raid-start-wave1:
/// десять раз за 900 тактов у одного жителя). Шанс 1/80 за такт — по этому
/// счёту [проверить].
pub const SWEAT_CHANCE: u32 = 80;

fn sweat(raid: &Raid, mobs: &mut [Mob], rng: &mut Rng) {
    for villager in mobs.iter_mut().filter(|mob| mob.kind == Kind::Villager && !mob.is_dead() && mob.age >= 0) {
        if dist2((villager.x, villager.y, villager.z), raid.center) <= BAR_RANGE * BAR_RANGE && rng.below(SWEAT_CHANCE) == 0 {
            villager.events.push(crate::mob::Event::Status(42));
        }
    }
}

/// Выход волны: рог у всех игроков рядом, разбойники на месте, ближние
/// одиночки присоединяются (вики, Raid).
#[allow(clippy::too_many_arguments)]
fn spawn_wave(raid: &mut Raid, spot: (f64, f64, f64), mobs: &mut [Mob], players: &[Watcher], rules: Rules, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32, out: &mut Outcome) {
    raid.spawned += 1;

    let last = waves_for(rules.difficulty);
    let mut captain = true;

    for (kind, count) in wave(raid.spawned, rules.difficulty, last, rng) {
        for _ in 0..count {
            let mut mob = Mob::new(next_id(), kind, spot);

            mob.persistent = true;
            mob.illager.raid = raid.id;
            mob.illager.raid_center = Some(raid.center);

            // Капитан волны — первый разбойник: зловещий флаг (вики, Raid:
            // «captain»); в набеге бутылку он не роняет.
            if captain && matches!(kind, Kind::Pillager | Kind::Vindicator) {
                mob.illager.captain = true;
                captain = false;
            }

            raid.raiders.push(mob.entity_id);
            raid.total_health += mob.vitals.health;
            out.born.push(mob);
        }
    }

    // Разбойники без набега рядом присоединяются (вики, Raid: «not spawned
    // by the raid … join … within 96 blocks»).
    for mob in mobs.iter_mut().filter(|mob| joins(mob.kind) && mob.illager.raid == 0 && !mob.is_dead()) {
        if dist2((mob.x, mob.y, mob.z), raid.center) <= BAR_RANGE * BAR_RANGE {
            mob.illager.raid = raid.id;
            mob.illager.raid_center = Some(raid.center);
            raid.raiders.push(mob.entity_id);
            raid.total_health += mob.vitals.health;
        }
    }

    for player in players.iter().filter(|player| dist2((player.x, player.y, player.z), raid.center) <= BAR_RANGE * BAR_RANGE) {
        out.private.push((player.id, horn(player, spot, rng.next_u64() as i64)));
    }

    raid.share = 1.0;
}

/// Колокол: разбойники в 48 блоках светятся (вики, Bell).
fn bells(raids: &mut Raids, mobs: &[Mob], out: &mut Outcome) {
    for bell in std::mem::take(&mut raids.bells) {
        let at = (f64::from(bell.0) + 0.5, f64::from(bell.1) + 0.5, f64::from(bell.2) + 0.5);
        let mut any = false;

        for mob in mobs.iter().filter(|mob| joins(mob.kind) && !mob.is_dead() && dist2((mob.x, mob.y, mob.z), at) <= BELL_RANGE * BELL_RANGE) {
            out.mob_effects.push((mob.entity_id, crate::survival::Effect { kind: crate::survival::EffectKind::Glowing, amplifier: 0, ticks_left: BELL_GLOW, ambient: false, visible: true, show_icon: true }));
            any = true;
        }

        if any {
            out.resonate.push(bell);
        }
    }
}

/// Патруль: с пятого дня, раз в 10–11 минут, шанс 1/5 — 1–5 разбойников с
/// капитаном в 24–48 блоках от случайного игрока (вики, Pillager, «Patrols»).
fn patrols(raids: &mut Raids, view: &dyn View, players: &[Watcher], rules: Rules, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32, out: &mut Outcome) {
    if !rules.patrols || rules.difficulty == 0 {
        return;
    }

    raids.patrol_wait -= 1;

    if raids.patrol_wait > 0 {
        return;
    }

    raids.patrol_wait = PATROL_WAIT + rng.below(PATROL_SPREAD) as i32;

    let alive: Vec<&Watcher> = players.iter().filter(|player| !player.spectator).collect();

    if rules.day < PATROL_DAYS || alive.is_empty() || rng.below(5) != 0 {
        return;
    }

    let player = alive[rng.below(alive.len() as u32) as usize];
    let sign = |rng: &mut Rng| if rng.below(2) == 0 { -1 } else { 1 };
    let x = player.x.floor() as i32 + (24 + rng.below(24) as i32) * sign(rng);
    let z = player.z.floor() as i32 + (24 + rng.below(24) as i32) * sign(rng);

    if !view.loaded(x.div_euclid(16), z.div_euclid(16)) {
        return;
    }

    // Величина патруля по сложности: на лёгкой 1–2, обычной 1–3, трудной
    // 1–5 [проверить: «depending on regional difficulty»].
    let size = 1 + rng.below(match rules.difficulty { 1 => 2, 2 => 3, _ => 5 }) as i32;

    for index in 0..size {
        let (px, pz) = (x + rng.below(5) as i32 - 2, z + rng.below(5) as i32 - 2);
        let mut y = player.y.floor() as i32 + 32;

        while y > player.y.floor() as i32 - 32 && !crate::mob::solid(view.block(px, y - 1, pz)) {
            y -= 1;
        }

        if !crate::mob::spawn::spawnable(view, px, y, pz, 2) {
            continue;
        }

        let mut mob = Mob::new(next_id(), Kind::Pillager, (f64::from(px) + 0.5, f64::from(y), f64::from(pz) + 0.5));

        mob.persistent = true;
        if !crate::mob::spawn::fits(view, &mob) { continue; }
        mob.illager.captain = index == 0;
        mob.illager.patrol = Some((player.x, player.y, player.z));
        out.born.push(mob);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrols_reject_fluid_at_feet_or_head() {
        struct Pad { feet:i32, head:i32 }
        impl View for Pad {
            fn block(&self,_:i32,y:i32,_:i32)->i32 {if y<=60 {1} else if y==61 {self.feet} else if y==62 {self.head} else {0}}
            fn loaded(&self,_:i32,_:i32)->bool {true}
        }
        let player=Watcher {id:1,x:0.5,y:61.0,z:0.5,eye:1.62,holding:[None,None],spectator:false,targetable:true,look:(0.0,0.0),pumpkin:false,sprinting:false,riding:false,gold_armor:false};
        let rules=Rules {difficulty:2,raids:false,patrols:true,day:6};
        let mut dry=0;
        for seed in 0..100 {
            let mut out=Outcome::default();
            patrols(&mut Raids {patrol_wait:0,..Default::default()},&Pad {feet:0,head:0},&[player],rules,&mut Rng::new(seed),&mut || 1,&mut out);
            dry+=out.born.len();
            for name in ["water","lava","oak_sign[waterlogged=true]"] {
                let fluid=crate::blocks::state_from_text(name).unwrap();
                for pad in [Pad {feet:fluid,head:0},Pad {feet:0,head:fluid}] {
                    let mut out=Outcome::default();
                    patrols(&mut Raids {patrol_wait:0,..Default::default()},&pad,&[player],rules,&mut Rng::new(seed),&mut || 1,&mut out);
                    assert!(out.born.is_empty(),"{name}");
                }
            }
        }
        assert!(dry>0);
    }

    #[test]
    fn wave1_scenario_caged_villager_claims_composter() {
        // Та же клетка, что tools/blackbox/scenarios/raid-start-wave1.json.
        struct Pad;
        impl View for Pad {
            fn block(&self, x:i32,y:i32,z:i32)->i32 {
                let name=if y==0 { "stone" }
                    else if (x,y,z)==(3,1,-3) { "composter" }
                    else if ((1..=2).contains(&y) && matches!((x,z),(1,-3)|(2,-2)|(2,-4)))
                        || matches!((x,y,z),(3,2,-3)|(2,3,-3)) { "glass" } else { "air" };
                crate::blocks::state_by_name(name).unwrap()
            }
            fn loaded(&self,_x:i32,_z:i32)->bool { true }
        }
        let mut villager=Mob::new(1,Kind::Villager,(2.5,1.0,-2.5));
        let mut rng=Rng::new(5);
        for clock in 6000..6200 { crate::villager::step(&mut villager,&Pad,clock,&[],&mut rng); }
        assert_eq!(villager.villager.job_site,Some((3,1,-3)));
        assert!(in_village(&[villager.villager.job_site.unwrap()],(0.5,1.0,0.5)));
    }

    #[test]
    fn waves_follow_difficulty() {
        assert_eq!(waves_for(1), 3);
        assert_eq!(waves_for(2), 5);
        assert_eq!(waves_for(3), 7);
    }

    #[test]
    fn first_normal_wave_is_pillagers() {
        let mut rng = Rng::new(7);
        let first = wave(1, 2, 5, &mut rng);
        let pillagers = first.iter().find(|(kind, _)| *kind == Kind::Pillager).map_or(0, |(_, count)| *count);

        // Вики, Raid: обычная, первая волна — 4–5 разбойников.
        assert!((4..=5).contains(&pillagers));
        assert!(first.iter().all(|(kind, _)| *kind != Kind::Ravager));
    }

    #[test]
    fn village_is_three_sections_around_a_claimed_site() {
        let pois = [(3, -60, -3)];

        assert!(in_village(&pois, (0.5, -60.0, 0.5)));
        assert!(!in_village(&pois, (60.5, -60.0, 0.5)));
        assert_eq!(center_of(&pois, (0.5, -60.0, 0.5)), Some((3.5, -60.0, -2.5)));
    }

    #[test]
    fn request_starts_a_raid_with_charging_bar() {
        struct Flat;

        impl View for Flat {
            fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
                if y < -60 { 1 } else { 0 }
            }

            fn loaded(&self, _x: i32, _z: i32) -> bool {
                true
            }
        }

        let mut raids = Raids::default();
        let player = Watcher { id: 5, x: 0.5, y: -60.0, z: 0.5, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false };
        let rules = Rules { difficulty: 2, raids: true, patrols: false, day: 0 };
        let mut rng = Rng::new(1);
        let mut id = 100;

        raids.requests.push(Request { player: 5, at: (0.5, -60.0, 0.5), level: 1 });

        let out = tick(&mut raids, &mut [], &Flat, &[player], &[(3, -60, -3)], rules, &mut rng, &mut || { id += 1; id });

        assert_eq!(raids.list.len(), 1);
        assert_eq!(raids.list[0].waves, 5);
        // Полоса показана игроку сразу: добавление и доля этого такта.
        assert_eq!(out.private.len(), 2);

        let mut born = Vec::new();

        for _ in 0..PRE_WAVE {
            born.extend(tick(&mut raids, &mut [], &Flat, &[player], &[(3, -60, -3)], rules, &mut rng, &mut || { id += 1; id }).born);
        }

        assert_eq!(raids.list[0].spawned, 1);
        assert!(born.len() >= 4);
        assert!(born.iter().any(|mob| mob.illager.captain));

        // Волна — далеко от середины и вне деревни (вики, Raid).
        let far = dist2((born[0].x, born[0].y, born[0].z), raids.list[0].center).sqrt();

        assert!(far >= 32.0, "волна в {far} блоках");
        assert!(!in_village(&[(3, -60, -3)], (born[0].x, born[0].y, born[0].z)));
    }

    #[test]
    fn spot_search_starts_far() {
        assert_eq!(spot_radius(0), 64.0);
        assert_eq!(spot_radius(PRE_WAVE / 2), 48.0);
        assert_eq!(spot_radius(PRE_WAVE - 1), 32.0);
    }

    #[test]
    fn new_bar_comes_with_previous_share_then_update() {
        struct Flat;

        impl View for Flat {
            fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
                if y < -60 { 1 } else { 0 }
            }

            fn loaded(&self, _x: i32, _z: i32) -> bool {
                true
            }
        }

        let mut raids = Raids::default();
        let player = Watcher { id: 5, x: 0.5, y: -60.0, z: 0.5, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false };
        let rules = Rules { difficulty: 2, raids: true, patrols: false, day: 0 };
        let mut rng = Rng::new(3);
        let mut id = 0;

        raids.requests.push(Request { player: 5, at: (0.5, -60.0, 0.5), level: 1 });

        let out = tick(&mut raids, &mut [], &Flat, &[player], &[(3, -60, -3)], rules, &mut rng, &mut || { id += 1; id });

        // Добавление с долей 0 и сразу доля этого такта (raid-start-wave1).
        assert_eq!(out.private.len(), 2);
        assert_eq!(out.private[0].1, { let mut fresh = raids.list[0].clone(); fresh.share = 0.0; bar_add(&fresh) });
    }
}
