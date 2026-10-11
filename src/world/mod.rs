// Мир: хранение блоков.
//
// Представление простое и «в лоб»: блоки хранятся по чанкам, состояния блоков —
// числами (теми же, какими они передаются клиенту в пакетах). Ни сжатия, ни
// регионов: мир состоит из ровной земли и того, что игроки в ней изменили.
//
// Чанки складываются по мере надобности — когда игрок до них дошёл (см. flat.rs
// и `ensure`), и с этого мгновения хранятся вместе с остальным миром.
//
// Состояние блока — число из таблицы состояний самой игры. Сервер обязан
// знать эти числа: по протоколу клиент их не сообщает. Нам нужны только те
// состояния, которые мы сами ставим, поэтому таблица ограничена константами
// ниже; правильность каждого числа проверяется глазами в игре.
//
// Мир лежит на диске одним файлом: список непустых блоков по чанкам. Пустые
// блоки не записываются — иначе файл распух бы в 384 раза на ровном месте.
// Сохранение происходит сразу при каждом изменении: сервер маленький, и
// терять постройки при падении не хочется.

mod codec;
mod spawn_search;
#[cfg(test)]
#[allow(dead_code)]
mod seedmap;
#[cfg(test)]
mod seedmap_tests;
mod legacy;
pub mod cave_biomes;
pub mod flat;
pub mod light;
pub mod nether;
pub mod nether_bastion;
pub mod nether_fortress;
pub mod end_city;
pub mod stronghold;
pub mod village;
pub mod monument;
pub mod trial_chambers;
pub mod ancient_city;
pub mod pillager_outpost;
pub mod mineshaft;
pub mod temples;
pub mod ocean_ruin;
pub mod ruined_portal;
pub mod shipwreck;
pub mod mansion;
pub mod template;
pub mod trail_ruins;
pub mod the_end;
pub mod noise;
pub mod ocean;
pub mod region;
pub mod terrain;
pub mod trees;
pub mod underground;
mod writer;
pub mod vegetation;

use std::cell::Cell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::config::server_properties::WorldKind;
use crate::containers;
use crate::fluids;
use crate::journal::{self, Journal};
use crate::redstone;
use crate::world::terrain::{Column, Style, Terrain};
use crate::{log_debug, log_error, log_info, log_warn};

include!("gen_limits.rs");

/// Секций в чанке.
pub const SECTIONS: i32 = WORLD_HEIGHT / 16;

/// Блоков в секции: 16 × 16 × 16.
pub const SECTION_BLOCKS: usize = 4096;

/// Сколько тактов длятся сутки.
pub const DAY_LENGTH: i64 = 24_000;

/// Одно изменение блока — то, что нужно разослать остальным игрокам.
#[derive(Clone, Copy)]
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: i32,

    /// Отправить отдельным сообщением, не складывая в пачку с соседями по
    /// секции: так уходят повторы блока после щелчка игрока.
    pub direct: bool,
}

/// Действие блока — то, что клиент проигрывает сам.
///
/// Так устроен ход поршня: сервер сообщает «поршень в этом месте поехал
/// туда-то», и клиент двигает блоки у себя плавно, пока сервер выжидает
/// положенные два такта.
#[derive(Clone, Copy)]
pub struct BlockAction {
    pub x: i32,
    pub y: i32,
    pub z: i32,

    /// Что происходит: у поршня 0 — выдвигается, 1 — задвигается,
    /// 2 — отменяет начатое выдвигание.
    pub action: u8,

    /// Уточнение: у поршня — сторона, куда он смотрит.
    pub param: u8,

    /// Номер блока (не состояния): по нему клиент сверяет, к чему это
    /// относится.
    pub block: i32,
}

/// Едущий блок: откуда, куда, состояние Java.
pub type PistonCargo = ((i32, i32, i32), (i32, i32, i32), i32);

/// Ход поршня, как его видит Bedrock: у Java клиент рисует ход сам по
/// Block Action, а Bedrock нужно сказать, какие блоки едут, откуда и куда.
#[derive(Clone, Debug, PartialEq)]
pub struct PistonMove {
    /// Основание поршня.
    pub base: (i32, i32, i32),
    pub extending: bool,

    /// Липкий ли поршень (основание, не «бросивший блок»).
    pub sticky: bool,

    /// Клетка головы и состояние головы Java, если она встаёт (выдвигание).
    pub head: (i32, i32, i32),
    pub head_state: Option<i32>,

    /// Едущие блоки, дальний от поршня первым: откуда, куда, состояние Java.
    pub cargo: Vec<PistonCargo>,
}

/// Звук в точке мира: щелчок рычага, ход поршня.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldSound {
    pub x: i32,
    pub y: i32,
    pub z: i32,

    /// Имя звука как в игре, с приставкой `minecraft:`.
    pub name: &'static str,
    pub volume: f32,
    pub pitch: f32,

    /// Точное место, если звук не от блока, а от сущности (плеск предмета).
    pub exact: Option<(f64, f64, f64)>,

    /// Звук блока, но в точке не посередине (сундук, бочка): раздел звука —
    /// «блоки», как у звука в середине блока.
    pub of_block: bool,

    /// Звук игрока (удар): раздел звука «игроки».
    pub of_player: bool,

    /// Кому звук не слать: игрок, который открыл дверь рукой, слышит её
    /// у себя сам, и сервер ему звук не повторяет (чёрный ящик, door-click:
    /// щёлкнувшему роботу звук двери не приходит).
    pub except: Option<i32>,
}

/// Событие мира, которое надо донести до клиентов: изменение блока или
/// действие блока.
///
/// Журнал у них один: порядок между ними важен. Действие поршня обязано
/// уйти раньше, чем изменение «поршень выдвинут», а воздух на месте
/// сломанного блока — раньше действия, в которое клиент этот воздух
/// заполнит. Два журнала такого порядка не держат: подключение выливало бы
/// сперва один целиком, потом другой, и действие более позднего такта
/// обгоняло бы изменения более раннего.
#[derive(Clone)]
pub enum WorldEvent {
    /// Достижение для игроков в заданной рамке; [Bedrock: потом].
    ProgressNear { id: &'static str, at: (f64, f64, f64), radius: (f64, f64, f64) },
    /// Полоса босса дракона: доля здоровья; None — убрать.
    BossBar(Option<f32>),
    /// Полоса босса иссушителя `entity_id`: новая доля (mob/wither.rs) — тем,
    /// кому он показан.
    WitherBar { entity_id: i32, share: f32 },
    Change(BlockChange),
    Action(BlockAction),

    /// Ход поршня начался (сразу после его Block Action): для Bedrock, у
    /// которого поршень и едущие блоки — блок-акторы (bedrock/pistons.rs).
    Piston(std::sync::Arc<PistonMove>),

    /// Звук, который слышно в этом месте.
    Sound(WorldSound),

    /// Событие мира, которое клиент проигрывает сам (World Event): звук и
    /// дым выбрасывателя, например. `effect` и `data` — как в пакете.
    Level { x: i32, y: i32, z: i32, effect: i32, data: i32, except: Option<i32> },

    /// Вибрация пошла от места `from` к датчику скалка `to` и долетит за
    /// `ticks` тактов: клиент рисует её частицей `vibration`.
    /// [Bedrock: потом] частица связи сердца (minecraft-data: trail).
    Trail { from: (f64,f64,f64), to: (f64,f64,f64), color: i32, ticks: i32 },
    Vibration { from: (i32, i32, i32), to: (i32, i32, i32), ticks: i32 },
    /// То же от точного места (событие сущности, хранитель слышит).
    VibrationAt { from: (f64, f64, f64), to: (i32, i32, i32), ticks: i32 },

    /// Нота над играющим проигрывателем: частица `note` в точке `at`,
    /// `color` — её цвет (доля от 0 до 1, уходит разбросом по X).
    Note { at: (f64, f64, f64), color: f32 },

    /// Игрок `viewer` (номер его сущности) открыл хранилище: окно уходит
    /// ему в этом месте потока — после звука открытия, до повтора блока.
    OpenWindow { viewer: i32, x: i32, y: i32, z: i32 },
    /// Окно правки таблички (Open Sign Editor): сторона — передняя или нет.
    OpenSign { viewer: i32, x: i32, y: i32, z: i32, front: bool },

    /// Метка «здесь изменения блоков уходят клиентам пачкой». У оригинала
    /// изменения блоков копятся и рассылаются в одной точке такта — после
    /// запланированных тактов, до блочных событий, — а звуки, события мира
    /// и действия блоков уходят сразу. Снято чёрным ящиком: у раздатчика
    /// звук ведра и дым раньше воды, у ловушки лампы раньше крышки.
    Broadcast,

    /// Метка «здесь слежение рассылает мобов»: у оригинала слежение стоит
    /// раньше хода сущностей, и их звуки этого такта (плита под свиньёй)
    /// идут после пакетов мобов (чёрный ящик, mob-plate).
    Tracker,

    /// Появление моба с облачком (чешуйница из заражённого блока): пакеты
    /// сущности уходят сразу, раньше пакета блока, остальное — в слежении
    /// конца такта (чёрный ящик, silverfish-infested).
    MobSpawn(Box<crate::mob::Mob>),

    /// Метка «здесь слежение рассылает стрелы»: после хода стрел (arrow.rs).
    ArrowTracker,

    /// Ракета фейерверка взорвалась: событие сущности 17 тем, кто её видит
    /// (firework.rs).
    RocketBlast { entity_id: i32 },
    /// Брошено око Края: оригинал показывает его сразу, раньше звука броска
    /// (чёрный ящик, eye-of-ender-far: появление, предмет, звук).
    EyeLaunch { entity_id: i32 },
    /// Метка «здесь слежение рассылает брошенные зелья и облака» (thrown.rs):
    /// `early` — в начале их хода (облака, которые слежение видит на такт
    /// позже), иначе — в конце.
    ThrownTracker { early: bool },

    /// Зелье или облако наложило эффект на игрока (в порядке такта: раньше
    /// события раскола, позже отстающего слежения за облаком).
    PotionEffect { entity_id: i32, effect: crate::survival::Effect },

    /// Зелье или облако вылечило игрока.
    PotionHeal { entity_id: i32, amount: f32 },

    /// Метка «здесь такт дошёл до игроков»: открытые окна получают
    /// перемены содержимого, опубликованные к этому мгновению
    /// (`World::published`).
    Contents,

    /// Взрыв: клиенту — пакет Explosion (звук, вспышка, толчок своему
    /// игроку). Сами разрушенные блоки идут обычными изменениями.
    Explosion(std::sync::Arc<Blast>),

    /// Данные блок-сущности (Block Entity Data): уходят после пачки
    /// изменений, в которую попали, — как у оригинала у полки и статуи
    /// голема (сценарии shelf, golem-statue). `nbt` — сетевой NBT.
    BlockEntity { x: i32, y: i32, z: i32, kind: i32, nbt: std::sync::Arc<[u8]> },

    /// Игроку `entity_id` нанесён урон (взрыв): подключение применяет его в
    /// этом месте потока — раньше пакета взрыва (чёрный ящик, tnt-damage).
    /// `amount` — урон до поправки на сложность, `source` — виновник
    /// (номер сущности динамита; 0 — нет).
    Hurt { entity_id: i32, amount: f32, source: i32, killer: &'static str },

    /// Игроку `entity_id` нанёс урон моб или стрела (mob/hostile.rs, arrow.rs):
    /// вид урона, номер виновника, ключ его имени для сообщения о смерти и
    /// толчок (скорость после отдачи, если есть).
    MobHurt { entity_id: i32, amount: f32, source: crate::survival::Source, attacker: i32, killer: &'static str, push: Option<(f64, f64)>, at: (f64, f64, f64) },

    /// Эффект игроку от моба (снаряд шалкера — левитация): подключение
    /// игрока даёт его, как команда `effect give`.
    MobEffect { entity_id: i32, effect: crate::survival::Effect },

    /// Помощь аксолотлю: накопить регенерацию и снять усталость (вики, Axolotl).
    AxolotlAssist { entity_id: i32 },

    /// То же, но только если удар моба, что шёл перед этим, нанёс урон (вики,
    /// Husk и Wither Skeleton: эффект — только от удара, что ранил).
    MobHitEffect { entity_id: i32, effect: crate::survival::Effect },

    /// Игрок бьёт игрока (или стрела попала): жертвино подключение считает
    /// щит, броню и отдачу в этом месте потока (combat.rs).
    PlayerStrike(std::sync::Arc<crate::combat::PlayerStrike>),

    /// Частицы (Level Particles): вид, блок для частицы блока, место,
    /// разброс, скорость, число.
    Particle { kind: i32, block: Option<i32>, at: (f64, f64, f64), offset: (f32, f32, f32), speed: f32, count: i32, skip: i32 },

    /// Анимация сущности (Entity Animation): 4 — критический удар, 5 —
    /// удар с зачарованием.
    Animation { entity_id: i32, kind: u8 },
    /// Поломка экипировки: клиент рисует частицы и играет звук.
    EquipmentBreak { entity_id: i32, status: u8 },
    /// Звук от сущности (Entity Sound Effect).
    EntitySound { entity_id: i32, name: &'static str, volume: f32, pitch: f32 },

    /// Игрок `entity_id` получил урон: остальные игроки видят Damage Event
    /// (номер вида, виновник и прямой виновник уже со сдвигом +1; чёрный
    /// ящик, pvp-thorns: «other damage_event»).
    PlayerDamaged { entity_id: i32, type_id: i32, culprit: i32, direct: i32, at: Option<(f64, f64, f64)> },

    /// Game Event одному игроку (6 — стрела попала в игрока: стрелку звук
    /// попадания; чёрный ящик, pvp-bow-power).
    GameEvent { entity_id: i32, reason: u8, value: f32 },
    /// Проклятие древнего стража игроку `entity_id`: «Утомление» III и событие
    /// игры 10 (mob/guardian.rs); у тихого стража — без звука.
    ElderCurse { entity_id: i32, silent: bool },

    /// Game Event погоды всем игрокам мира (weather.rs).
    Weather { reason: u8, value: f32 },

    /// Игрока `entity_id` задела молния (lightning.rs).
    Struck { entity_id: i32 },

    /// Игрок `by` подобрал стрелу `entity_id`: всем Take Item Entity и
    /// сразу за ним — убрать стрелу (чёрный ящик, arrow-pickup).
    Collect { entity_id: i32, by: i32, count: i32 },

    /// Готовый пакет (номер и тело) только игроку `viewer` — в общем порядке
    /// событий: «Respawn point set» у кровати идёт раньше взмаха и повтора
    /// блоков (чёрный ящик, bed-sleep-day).
    Private { viewer: i32, body: std::sync::Arc<[u8]> },
}

/// Взрыв для клиентов: где, какой силы, сколько блоков разрушил и кого из
/// игроков как толкнул — номер сущности и прибавка к скорости. Игрока
/// двигает его клиент, поэтому толчок уходит в пакете (вики, Java Edition
/// protocol/Packets, Explosion: «Velocity difference of the player being
/// pushed by the explosion»).
#[derive(Clone, Debug, PartialEq)]
pub struct Blast {
    pub centre: (f64, f64, f64),
    pub power: f32,
    pub blocks: i32,
    pub pushed: Vec<(i32, (f64, f64, f64))>,
    /// Взрыв ветра («порыв ветра» булавы): свои частицы и звук, без дыма,
    /// толчок — только в самом пакете.
    pub wind: bool,
    /// Чей взрыв ветра: у заряда игрока вихрь малый, у бриза свой звук
    /// (чёрный ящик, wind-charge-throw, breeze-charge-flight).
    pub gust: Gust,
}

/// Чей взрыв ветра.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gust {
    /// «Порыв ветра» булавы: вихрь gust_emitter_large, звук wind_charge.wind_burst.
    #[default]
    Mace,
    /// Заряд ветра игрока: вихрь gust_emitter_small, тот же звук.
    Charge,
    /// Заряд бриза: вихрь gust_emitter_large, звук breeze.wind_burst.
    Breeze,
}

/// Блоки одного чанка — по секциям.
///
/// Секция — это куб 16×16×16, и хранится она отдельно. Причина простая:
/// почти весь мир — воздух. Чанк целиком занял бы больше трети мегабайта,
/// и сотня пройденных чанков съела бы десятки мегабайт ни за чем; а так
/// воздушная секция не занимает ничего.
///
/// Порядок блоков внутри секции — тот же, в котором они идут в пакете чанка:
/// сначала по Y (снизу вверх), внутри — по Z, внутри — по X. Поэтому секцию
/// не приходится перекладывать при отправке.
struct Chunk {
    /// Секции снизу вверх. None — секция целиком из воздуха.
    ///
    /// Состояние блока хранится в двух байтах, а не в четырёх: номеров
    /// состояний в игре около тридцати тысяч, и в два байта они влезают
    /// с запасом. На дальности в тридцать два чанка это половина памяти
    /// сервера — сотни мегабайт.
    ///
    /// Секция лежит под `Arc`: свет чанка считается вне захвата мира, и ему
    /// достаётся не копия блоков, а та же секция. Правка в мире в это время
    /// заводит себе новую копию секции, а считающий свет дочитывает старую.
    sections: Vec<Option<Arc<[Packed]>>>,

    /// Биомы чанка: по одному на клетку 4×4×4 блока — так же дробно, как их
    /// передаёт протокол, и в том же порядке: снизу вверх, внутри слоя по Z,
    /// потом по X. У поверхности и выше — биом столбца, в толще под ней —
    /// пещерный, где его выбрали климат и глубина.
    ///
    /// Номер биома хранится в двух байтах: биомов в реестре меньше сотни.
    biomes: Vec<BiomeId>,

    /// Собственный свет чанка — посчитанный только по его блокам (см.
    /// light.rs) — и номер правки блоков, к которой он относится. Не сходится
    /// номер с `revision` — свет устарел и будет пересчитан при отправке.
    light: Option<(u32, Arc<light::ChunkLight>)>,

    /// Номер правки блоков: растёт при каждом изменении.
    revision: u32,

    /// Хранилища чанка (сундуки, воронки…) по месту внутри чанка:
    /// (x, y, z), где x и z — от 0 до 15, а y — настоящая высота.
    containers: HashMap<(i32, i32, i32), containers::Container>,

    /// Хранилища, чей блок сменился другим: их содержимое высыплется
    /// (мировые координаты). Забирает мир — `World::take_spilled`.
    spilled: Vec<((i32, i32, i32), containers::Container)>,

    /// Особый динамит (custom_tnt.rs): какой вид стоит в этом месте чанка
    /// (x, z — от 0 до 15, y — настоящая высота). Запись живёт, пока там
    /// динамит. Список, а не словарь: таких мест в чанке единицы, а чанк
    /// должен оставаться маленьким.
    custom_tnt: Vec<((i32, i32, i32), crate::custom_tnt::KindId)>,

    /// Рамки для предметов (frame.rs), висящие в этом чанке, — то, что
    /// ложится в файл. Живая сущность — в `Items`, здесь её запись (место —
    /// мировое, номер сущности не в счёт).
    frames: Vec<crate::frame::Frame>,

    /// Мобы (mob/mod.rs), стоявшие в этом чанке при последнем сохранении:
    /// то, что ложится в файл. Живой моб — в `Items`; записи обновляет
    /// `World::store_mobs` перед записью на диск и выгрузкой.
    mobs: Vec<crate::mob::Mob>,

    /// Данные командных блоков (command_block.rs) по месту внутри чанка, как
    /// у хранилищ. Запись живёт, пока на месте стоит любой из трёх видов.
    command_blocks: HashMap<(i32, i32, i32), crate::command_block::Data>,

    /// Данные спавнеров (spawner.rs) по месту внутри чанка.
    spawners: HashMap<(i32, i32, i32), crate::spawner::Data>,

    /// Испытательные спавнеры и хранилища (trial.rs): место в чанке → данные.
    trials: HashMap<(i32, i32, i32), crate::trial::Data>,

    /// Таблички с текстом (sign.rs): место внутри чанка.
    signs: HashMap<(i32, i32, i32), crate::sign::Data>,
    hives: HashMap<(i32, i32, i32), crate::hive::Data>,
    archaeology: HashMap<(i32,i32,i32),crate::archaeology::Data>,
}

/// Выброшенный хранилищем предмет: откуда, с какой скоростью и что.
pub type Ejected = ((f64, f64, f64), (f64, f64, f64), crate::inventory::Stack);

/// Подожжённый динамит, который такт ещё не сделал сущностью: где, с какой
/// скоростью, сколько тактов запала и какой он, если особый.
pub type Primed = ((f64, f64, f64), (f64, f64, f64), i32, Option<crate::custom_tnt::KindId>);

/// Быстрый хеш для ключей-координат (номер чанка, место блока): умножение
/// с поворотом, как у FxHash. Обычный хеш стойкий к подбору ключей, но
/// медленный, а мир спрашивает чанк на каждый блок — ядерный динамит
/// (custom_tnt.rs) делает это сотни тысяч раз за такт. Порядок обхода от
/// хеша не зависит ни у кого: он и прежде был случайным.
#[derive(Default, Clone, Copy)]
pub struct CoordHasher(u64);

impl std::hash::Hasher for CoordHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }

    fn write_i32(&mut self, value: i32) {
        self.write_u64(u64::from(value as u32));
    }

    fn write_u64(&mut self, value: u64) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(SEED);
    }
}

/// Сборщик `CoordHasher` для HashMap и HashSet.
pub type CoordHash = std::hash::BuildHasherDefault<CoordHasher>;

/// Номер биома в реестре, как он лежит в памяти чанка.
pub type BiomeId = u16;

/// Состояние блока в том виде, в каком оно лежит в памяти чанка.
type Packed = u16;

/// Ужимает состояние для хранения. Номеров состояний в игре около тридцати
/// тысяч; если однажды их станет больше, чем помещается, лучше об этом
/// узнать сразу, а не получить чужой блок в мире.
fn pack(state: i32) -> Packed {
    Packed::try_from(state).unwrap_or_else(|_| panic!("состояние {} не влезает в два байта", state))
}

/// Разворачивает хранимое состояние обратно.
fn unpack(state: Packed) -> i32 {
    state as i32
}

/// Сколько клеток биома приходится на сторону чанка: биом задаётся на каждые
/// четыре блока, так же как в пакете чанка.
const BIOME_CELLS: usize = (CHUNK_SIZE / 4) as usize;

/// Слоёв клеток биома по высоте мира.
const BIOME_LAYERS: usize = (WORLD_HEIGHT / 4) as usize;

/// Клеток биома в слое 4×4.
const BIOME_LAYER: usize = BIOME_CELLS * BIOME_CELLS;

/// Клеток биома в секции: 4×4×4.
pub const SECTION_BIOMES: usize = BIOME_LAYER * 4;

/// Место клетки биома в `Chunk::biomes`: слой по высоте (от дна мира), затем
/// клетка по Z и по X.
fn biome_cell(layer: usize, cell_x: usize, cell_z: usize) -> usize {
    layer * BIOME_LAYER + cell_z * BIOME_CELLS + cell_x
}

impl Chunk {
    fn new() -> Self {
        Self {
            sections: (0..SECTIONS).map(|_| None).collect(),
            biomes: vec![0; BIOME_LAYERS * BIOME_LAYER],
            light: None,
            revision: 0,
            containers: HashMap::new(),
            spilled: Vec::new(),
            custom_tnt: Vec::new(),
            frames: Vec::new(),
            mobs: Vec::new(),
            command_blocks: HashMap::new(),
            spawners: HashMap::new(),
            trials: HashMap::new(),
            signs: HashMap::new(),
            hives: HashMap::new(),
            archaeology: HashMap::new(),
        }
    }

    /// Считает собственный свет чанка по его нынешним блокам.
    fn light_up(&mut self) {
        self.light = Some((self.revision, Arc::new(light::chunk_light(&self.sections))));
    }

    /// Собственный свет, если он не устарел.
    fn fresh_light(&self) -> Option<&Arc<light::ChunkLight>> {
        match &self.light {
            Some((revision, light)) if *revision == self.revision => Some(light),
            _ => None,
        }
    }

    /// Чанк сложенного мира: рельеф, биомы и всё, что из них следует.
    fn generated(generator: &Generator, chunk_x: i32, chunk_z: i32) -> Self {
        // У Незера пол и потолок — он складывается целиком своим путём.
        if let Generator::Nether(nether) = generator {
            let mut chunk = nether.chunk(chunk_x, chunk_z);

            // Разрушенные порталы Незера (ruined_portal.rs).
            ruined_portal::build_nether(nether, &mut chunk, chunk_x, chunk_z);

            // Спавнеры ифритов в комнатах трона крепости (вики, Nether Fortress:
            // «blaze spawner»): блок в образце уже лежит, блок-сущность — здесь.
            if let Some(spawner) = crate::blocks::state_by_name("spawner") {
                for y in 1..127 {
                    for z in 0..CHUNK_SIZE {
                        for x in 0..CHUNK_SIZE {
                            if chunk.block(x, y, z) == spawner {
                                chunk.put_generated_spawner(x, y, z, crate::spawner::Data::of("blaze"));
                            }
                        }
                    }
                }
            }

            chunk.light_up();
            return chunk;
        }

        // Энд — тоже целиком своим путём.
        if let Generator::End(end) = generator {
            let mut chunk = end.chunk(chunk_x, chunk_z);
            chunk.light_up();
            return chunk;
        }

        let mut chunk = Self::new();

        // Шумы считаются один раз на столбец, а не на каждый блок: иначе на
        // чанк приходилось бы сто тысяч обращений к шуму. Берём и кайму
        // в блок шириной — крутизну склона видно только по соседям.
        const SIDE: i32 = CHUNK_SIZE + 2;
        let (from_x, from_z) = (chunk_x * CHUNK_SIZE - 1, chunk_z * CHUNK_SIZE - 1);
        let grid = generator.relief_grid(from_x, from_z, from_x + SIDE - 1, from_z + SIDE - 1);
        let mut columns = Vec::with_capacity((SIDE * SIDE) as usize);

        for local_x in -1..=CHUNK_SIZE {
            for local_z in -1..=CHUNK_SIZE {
                let (x, z) = (
                    chunk_x * CHUNK_SIZE + local_x,
                    chunk_z * CHUNK_SIZE + local_z,
                );

                columns.push(generator.column_in(x, z, grid.as_ref()));
            }
        }

        // Шумы пещер — тоже по сетке, до самого высокого столбца чанка.
        let highest = columns
            .iter()
            .map(|column| column.height)
            .max()
            .unwrap_or(MIN_Y);
        let caves = generator.cave_grid(chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE, highest);

        let at =
            |local_x: i32, local_z: i32| columns[((local_x + 1) * SIDE + local_z + 1) as usize];

        for local_x in 0..CHUNK_SIZE {
            for local_z in 0..CHUNK_SIZE {
                let x = chunk_x * CHUNK_SIZE + local_x;
                let z = chunk_z * CHUNK_SIZE + local_z;

                // Крутизна — как у оригинала: южный сосед выше северного
                // на четыре и больше или западный выше восточного.
                let mut column = at(local_x, local_z);
                column.steep = at(local_x, local_z + 1).height - at(local_x, local_z - 1).height
                    >= 4
                    || at(local_x - 1, local_z).height - at(local_x + 1, local_z).height >= 4;

                // Столбец заполняется сверху вниз: так видно, где кончается
                // толща камня над блоком, — от её верха считается глубина
                // для травы, земли и песка.
                let top = generator.top_of(&column);
                let mut run_top = top;

                for y in (MIN_Y..=top).rev() {
                    let solid = generator.solid_in(grid.as_ref(), &column, x, y, z);

                    if !solid {
                        run_top = y - 1;
                    }

                    let state =
                        generator.block_with(&column, caves.as_ref(), x, y, z, solid, run_top);

                    if state == AIR {
                        continue;
                    }

                    let (section, inside) = split(y);
                    let index = (local_z * CHUNK_SIZE + local_x) as usize;

                    chunk.filled_section(section)[inside + index] = pack(state);
                }

                // Биом клетки у всех её столбцов один (его решает середина
                // клетки), поэтому решаем его один раз — по середине: и
                // поверхностный, и пещерные под ним.
                if local_x % 4 == 0 && local_z % 4 == 0 {
                    let middle = at(local_x + 2, local_z + 2);
                    let surface = generator.biome_of(&column);
                    // Редкие осенние пятна в обычном лесу — свой номер биома,
                    // чтобы клиент красил листву и траву по-осеннему.
                    let autumn = surface == terrain::Biome::Forest
                        && matches!(generator, Generator::Normal(terrain) if terrain.autumn_at(x + 2, z + 2));
                    let surface = if autumn { terrain::autumn_forest_number() } else { surface.number() as BiomeId };

                    chunk.fill_biome_column(
                        (local_x / 4) as usize,
                        (local_z / 4) as usize,
                        surface,
                        matches!(generator, Generator::Normal(_)).then_some(&middle),
                    );
                }
            }
        }

        // Руда и пятна камня — после рельефа и пещер: им надо знать, где
        // камень и где воздух. Деревья — после руды: им до неё дела нет.
        generator.lay_ores(&mut chunk, chunk_x, chunk_z);
        underground::dig(generator, &mut chunk, chunk_x, chunk_z);
        // Заброшенные шахты — после пещер: проходы режут камень и пещеры,
        // над пустотой — мосты (mineshaft.rs).
        mineshaft::build(generator, &mut chunk, chunk_x, chunk_z);
        stronghold::build(generator, &mut chunk, chunk_x, chunk_z);
        // Разрушенные порталы — до деревьев: у оригинала строения раньше
        // растительности (ruined_portal.rs).
        ruined_portal::build(generator, &mut chunk, chunk_x, chunk_z);
        // Кораблекрушения и зарытые сокровища (shipwreck.rs).
        shipwreck::build(generator, &mut chunk, chunk_x, chunk_z);
        // Руины в океане (ocean_ruin.rs).
        ocean_ruin::build(generator, &mut chunk, chunk_x, chunk_z);
        // Руины тропы (trail_ruins.rs).
        trail_ruins::build(generator, &mut chunk, chunk_x, chunk_z);
        generator.grow_trees(&mut chunk, chunk_x, chunk_z, &columns);
        vegetation::decorate(generator, &mut chunk, chunk_x, chunk_z, &columns);
        village::build(generator, &mut chunk, chunk_x, chunk_z);
        // Лесные особняки — после деревьев: воздух комнат вырезает деревья
        // в рамке, фундамент заливает стволы под полом (mansion.rs).
        mansion::build(generator, &mut chunk, chunk_x, chunk_z);
        // Храмы пустыни и джунглей, хижины ведьм (temples.rs).
        temples::build(generator, &mut chunk, chunk_x, chunk_z);
        // Аванпосты разбойников (pillager_outpost.rs).
        pillager_outpost::build(generator, &mut chunk, chunk_x, chunk_z);
        ocean::decorate(generator, &mut chunk, chunk_x, chunk_z, &columns);
        // Подводные крепости — после украшений океана: в рамке всё уходит под
        // воду и призмарин (monument.rs).
        monument::build(generator, &mut chunk, chunk_x, chunk_z);
        cave_biomes::decorate(generator, &mut chunk, chunk_x, chunk_z, &columns, grid.as_ref());
        // Древние города — после украшений глубокой тьмы: в кусках города
        // порода и скалк уступают залам (ancient_city.rs).
        ancient_city::build(generator, &mut chunk, chunk_x, chunk_z);
        // Испытательные камеры — последними: в своих кусках они важнее и
        // породы, и пещерных украшений (trial_chambers.rs).
        trial_chambers::build(generator, &mut chunk, chunk_x, chunk_z);

        // Спавнеры и хранилища камер — со своими данными по замеру (trial.rs).
        if let Generator::Normal(terrain) = generator {
            for (x, y, z, kind, data) in trial_chambers::block_entities(terrain, chunk_x, chunk_z) {
                if let Some(entry) = crate::trial::Data::generated(kind, data) {
                    chunk.put_generated_trial(x - chunk_x * CHUNK_SIZE, y, z - chunk_z * CHUNK_SIZE, entry);
                }
            }
        }

        // Свет — когда все блоки на местах.
        chunk.light_up();

        chunk
    }

    /// Заполняет биомы одного столбца клеток 4×4: у поверхности и выше —
    /// `surface`, в толще — пещерный биом, если его выбирают климат и
    /// глубина под серединой клетки `middle`. Без `middle` пещерных биомов
    /// нет (ровный мир).
    fn fill_biome_column(
        &mut self,
        cell_x: usize,
        cell_z: usize,
        surface: BiomeId,
        middle: Option<&Column>,
    ) {

        for layer in 0..BIOME_LAYERS {
            // Середина клетки по высоте.
            let y = MIN_Y + layer as i32 * 4 + 2;
            let biome = middle
                .and_then(|column| cave_biomes::biome_at(column, y))
                .map_or(surface, |cave| cave.number() as BiomeId);

            self.biomes[biome_cell(layer, cell_x, cell_z)] = biome;
        }
    }

    /// Биом клетки, в которой лежит блок (местные координаты чанка).
    fn biome(&self, local_x: i32, y: i32, local_z: i32) -> BiomeId {
        let layer = ((y - MIN_Y) / 4) as usize;

        self.biomes[biome_cell(layer, (local_x / 4) as usize, (local_z / 4) as usize)]
    }

    /// Ставит блок дерева, если он попал в этот чанк, и не затирает то, что
    /// уже стоит: ствол важнее листвы.
    fn put_generated(&mut self, x: i32, y: i32, z: i32, state: i32, over: bool) {
        if !(0..CHUNK_SIZE).contains(&x) || !(0..CHUNK_SIZE).contains(&z) {
            return;
        }

        if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
            return;
        }

        let (section, inside) = split(y);
        let index = (z * CHUNK_SIZE + x) as usize;
        let place = &mut self.filled_section(section)[inside + index];

        if over || unpack(*place) == AIR {
            *place = pack(state);
        }

        if crate::hive::is_hive(state) && self.block(x,y,z) == state {
            self.hives.entry((x,y,z)).or_insert_with(||crate::hive::Data::generated((x as u64).wrapping_mul(73471) ^ (z as u64).wrapping_mul(317) ^ y as u64));
        }

        // Спавнер, поставленный генерацией: вид по месту — подземелье и
        // крепость выбирают его случайно (вики, Monster Spawner: скелет,
        // зомби ×2, паук) [проверить: доли].
        if crate::blocks::block_at_state(state) == Some("spawner") {
            let pick = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663) ^ z.wrapping_mul(83_492_791)).rem_euclid(4);
            let kind = ["skeleton", "zombie", "zombie", "spider"][pick as usize];

            self.spawners.insert((x, y, z), crate::spawner::Data::of(kind));
        }
    }

    /// Даёт данные спавнерам-блокам, у которых их нет: в Незере — ифрит
    /// (вики, Monster Spawner: «blaze in Nether fortresses»), иначе — по месту,
    /// как у `put_generated`.
    fn fill_missing_spawners(&mut self, nether: bool) {
        let Some(spawner) = crate::blocks::state_by_name("spawner") else {
            return;
        };
        let mut found = Vec::new();

        for (index, section) in self.sections.iter().enumerate() {
            let Some(states) = section.as_deref() else {
                continue;
            };

            for (at, state) in states.iter().enumerate() {
                if unpack(*state) == spawner {
                    found.push((at as i32 & 15, MIN_Y + index as i32 * 16 + (at as i32 >> 8), (at as i32 >> 4) & 15));
                }
            }
        }

        for place in found {
            if self.spawners.contains_key(&place) {
                continue;
            }

            let (x, y, z) = place;
            let pick = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663) ^ z.wrapping_mul(83_492_791)).rem_euclid(4);
            let kind = if nether { "blaze" } else { ["skeleton", "zombie", "zombie", "spider"][pick as usize] };

            self.spawners.insert(place, crate::spawner::Data::of(kind));
        }
    }

    /// Спавнер, сложенный генерацией, с видом моба: x, z — внутри чанка.
    /// Сгенерированный испытательный блок (испытательные залы): данные по
    /// месту внутри чанка.
    fn put_generated_trial(&mut self, x: i32, y: i32, z: i32, data: crate::trial::Data) {
        if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
            self.trials.insert((x, y, z), data);
        }
    }

    fn put_generated_spawner(&mut self, x: i32, y: i32, z: i32, data: crate::spawner::Data) {
        if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
            self.spawners.insert((x, y, z), data);
        }
    }

    /// Хранилище, сложенное генерацией (сундук постройки с добычей):
    /// x, z — внутри чанка.
    fn put_generated_container(&mut self, x: i32, y: i32, z: i32, container: containers::Container) {
        if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
            self.containers.insert((x, y, z), container);
        }
    }

    /// Чанк ровного мира: земля одинаковой высоты по всей площади.
    #[cfg(test)]
    fn flat() -> Self {
        let mut chunk = Self::new();

        for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
            let state = flat::block_at(y);

            if state == AIR {
                continue;
            }

            for index in 0..(CHUNK_SIZE * CHUNK_SIZE) as usize {
                let (section, inside) = split(y);

                chunk.filled_section(section)[inside + index] = pack(state);
            }
        }

        chunk
    }

    /// Состояние блока внутри чанка.
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        let (section, inside) = split(y);

        match &self.sections[section] {
            Some(blocks) => unpack(blocks[inside + corner(x, z)]),
            None => AIR,
        }
    }

    /// Ставит блок. Возвращает false, если он уже был таким.
    fn set(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let (section, inside) = split(y);
        let index = inside + corner(x, z);

        // Тот же блок ставить незачем, а в воздушную секцию воздух — тем
        // более: заводить её ради этого не надо.
        let old = self.block(x, y, z);

        if old == state {
            return false;
        }

        self.filled_section(section)[index] = pack(state);

        // Хранилище живёт, пока на месте стоит блок того же вида: сундук,
        // повёрнутый или спаренный с соседом, своё содержимое сохраняет,
        // а сундук, сменённый воронкой, — нет.
        let (kind, was) = (containers::Kind::of(state), containers::Kind::of(old));

        if kind != was {
            let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

            if let Some(mut gone) = self.containers.remove(&place) {
                gone.block = old;
                self.spilled.push(((x, y, z), gone));
            }

            if let Some(kind) = kind {
                self.containers.insert(place, containers::Container::new(kind));
            }
        }

        if crate::hive::is_hive(state) != crate::hive::is_hive(old) {
            let place=(x.rem_euclid(CHUNK_SIZE),y,z.rem_euclid(CHUNK_SIZE));
            if crate::hive::is_hive(state) { self.hives.insert(place,crate::hive::Data::default()); }
            else { self.hives.remove(&place); }
        }

        // Текст таблички уходит вместе с блоком.
        if !self.signs.is_empty() && !crate::redstone::is_sign(state) {
            self.signs.remove(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)));
        }

        // Командный блок: данные переживают смену вида (окно меняет режим —
        // блок другой, команда та же) и уходят вместе с блоком.
        let (command, was_command) = (crate::command_block::mode_of(state).is_some(), crate::command_block::mode_of(old).is_some());

        if command != was_command {
            let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

            if command {
                self.command_blocks.insert(place, crate::command_block::Data::default());
            } else {
                self.command_blocks.remove(&place);
            }
        }

        // Спавнер: данные рождаются с блоком и уходят вместе с ним.
        let (spawner, was_spawner) = (crate::blocks::block_at_state(state) == Some("spawner"), crate::blocks::block_at_state(old) == Some("spawner"));

        if spawner != was_spawner {
            let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

            if spawner {
                self.spawners.insert(place, crate::spawner::Data::default());
            } else {
                self.spawners.remove(&place);
            }
        }

        // Испытательный спавнер и хранилище: данные рождаются с блоком и
        // уходят вместе с ним (смена состояния того же блока их не трогает).
        let (name, was) = (crate::blocks::block_at_state(state), crate::blocks::block_at_state(old));

        if name != was && (matches!(name, Some("trial_spawner" | "vault")) || matches!(was, Some("trial_spawner" | "vault"))) {
            let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

            match name.and_then(crate::trial::Data::for_block) {
                Some(data) => {
                    self.trials.insert(place, data);
                }
                None => {
                    self.trials.remove(&place);
                }
            }
        }

        if name!=was {
            let place=(x.rem_euclid(CHUNK_SIZE),y,z.rem_euclid(CHUNK_SIZE));
            if crate::archaeology::suspicious(state) {self.archaeology.insert(place,crate::archaeology::Data::default());}
            else {self.archaeology.remove(&place);}
        }

        // Динамит сменился чем-то другим — вид его больше не нужен.
        if !self.custom_tnt.is_empty() && !crate::tnt::is_tnt(state) {
            let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

            self.custom_tnt.retain(|(there, _)| *there != place);
        }

        true
    }

    /// Верх самой высокой непустой секции: выше него в чанке только воздух.
    fn highest(&self) -> i32 {
        match self.sections.iter().rposition(Option::is_some) {
            Some(section) => MIN_Y + (section as i32 + 1) * 16 - 1,
            None => MIN_Y - 1,
        }
    }

    /// Секция, готовая к записи: воздушная заводится на месте.
    ///
    /// Всякая правка блоков идёт через неё, поэтому здесь же растёт номер
    /// правки: собственный свет чанка с этого мгновения устарел.
    fn filled_section(&mut self, section: usize) -> &mut [Packed] {
        self.revision = self.revision.wrapping_add(1);

        let blocks = self.sections[section].get_or_insert_with(|| Arc::from([pack(AIR); SECTION_BLOCKS]));

        Arc::make_mut(blocks)
    }

    /// Сколько в чанке блоков, отличных от воздуха.
    #[cfg(test)]
    fn not_air(&self) -> usize {
        self.sections
            .iter()
            .flatten()
            .map(|blocks| blocks.iter().filter(|state| unpack(**state) != AIR).count())
            .sum()
    }

    /// Блоки секции. None — секция целиком из воздуха.
    fn section(&self, section: usize) -> Option<&[Packed]> {
        self.sections[section].as_deref()
    }

    /// Места, куда вода и лава должны потечь сразу: пустые клетки (или
    /// рыхлый снег, или другая жидкость) сбоку и снизу от них. Такт
    /// жидкости решает, чем станет само место, поэтому будить надо не
    /// источник — он и так останется источником, — а соседнюю пустоту: она
    /// по такту станет текущей водой. Так у оригинала подземные водопады и
    /// лава в пещерах текут с первого мгновения.
    ///
    /// Соседи за краем чанка тоже попадают в список: если соседа ещё нет,
    /// такт подождёт его. Координаты мировые, вместе с задержкой такта.
    fn fluids_to_wake(&self, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), u64)> {
        let mut found = Vec::new();
        // Состояния воды и лавы идут подряд: жидкость узнаётся сравнением
        // номера, без разбора каждого блока.
        let (water, lava) = (fluids::Kind::Water.states(), fluids::Kind::Lava.states());
        let liquid = |state: i32| (water.0..=water.1).contains(&state) || (lava.0..=lava.1).contains(&state);

        for (section, blocks) in self.sections.iter().enumerate() {
            let Some(blocks) = blocks else {
                continue;
            };

            for (index, packed) in blocks.iter().enumerate() {
                if !liquid(unpack(*packed)) {
                    continue;
                }

                let Some((kind, _)) = fluids::fluid_at(unpack(*packed)) else {
                    continue;
                };

                let (x, z) = ((index % 16) as i32, (index / 16 % 16) as i32);
                let y = MIN_Y + section as i32 * 16 + (index / 256) as i32;
                let open = |state: i32| match fluids::fluid_at(state) {
                    Some((other, _)) => other != kind,
                    None => fluids::can_flow_into(state),
                };

                for (dx, dy, dz) in [(0, -1, 0), (-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1)] {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);

                    if ny < MIN_Y {
                        continue;
                    }

                    // Соседа за краем чанка отсюда не видно: край проверит
                    // мир, когда чанк ляжет рядом с соседом (`wake_border`).
                    let inside = (0..16).contains(&nx) && (0..16).contains(&nz);

                    // Будится сама жидкость: пустое место её такта не ждёт.
                    if inside && open(self.block(nx, ny, nz)) {
                        found.push(((chunk_x * 16 + x, y, chunk_z * 16 + z), kind.delay()));
                    }
                }
            }
        }

        found.sort_unstable();
        found.dedup();
        found
    }

}

/// Секция, в которой лежит эта высота, и начало её слоя внутри секции.
fn split(y: i32) -> (usize, usize) {
    let from_bottom = (y - MIN_Y) as usize;
    let height = CHUNK_SIZE as usize;

    (
        from_bottom / height,
        (from_bottom % height) * (CHUNK_SIZE * CHUNK_SIZE) as usize,
    )
}

/// Место блока в слое секции.
fn corner(x: i32, z: i32) -> usize {
    let local_x = x.rem_euclid(CHUNK_SIZE) as usize;
    let local_z = z.rem_euclid(CHUNK_SIZE) as usize;

    local_z * CHUNK_SIZE as usize + local_x
}

/// Приоритет жидкостей в очереди: после всех блочных тактов, как в игре.
const FLUID_PRIORITY: i8 = 1;

/// Какой это такт: блочный (редстоун) или жидкостный. В игре это две
/// разные очереди, и блочные идут первыми.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TickKind {
    Block,
    Fluid,
}

/// Ключ ожидающего такта: место и вид такта.
type Waiting = ((i32, i32, i32), TickKind);

/// Запись в очереди запланированных тактов.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Scheduled {
    due: u64,
    priority: i8,
    order: u64,
    place: (i32, i32, i32),
    kind: TickKind,
}

/// Директория с регионами внутри директории мира. Название как в оригинале.
const REGIONS: &str = "region";

/// Версия записи чанка. Меняется, если меняется раскладка байтов или само
/// устройство мира, чтобы старая запись не была прочитана неправильно.
///
/// С третьей версии биомы объёмные: клетка 4×4×4, записаны «номер и сколько
/// подряд».
///
/// Четвёртая версия — палитры с битовыми номерами (codec.rs); файл региона
/// `.mcs` сжимает её ещё и zstd. Первые три версии писались в `.rcw`, их
/// читает только перенос мира (legacy.rs).
const CHUNK_VERSION: u8 = 4;

/// Вторая версия: состояния по два байта, биом один на столбец клетки 4×4.
/// Такие чанки читаются, их биом растягивается на всю высоту.
const CHUNK_VERSION_FLAT_BIOMES: u8 = 2;

/// Первый вид записи чанка: состояния по четыре байта, биомы как во второй.
/// Такие файлы мы ещё читаем — у кого-то мир записан ими, и терять его
/// незачем.
const CHUNK_VERSION_WIDE: u8 = 1;


/// Мир целиком: чанки и список изменений.
pub struct World {
    pub fire_tick: bool,
    pub fire_difficulty: u8,
    /// Ноты для тихонь и вылупившиеся нюхачи (mob/small_world.rs).
    pub allay_notes: Vec<(i32, i32, i32)>,
    pub ominous_births:Vec<(crate::inventory::Stack,(f64,f64,f64))>,
    pub snifflet_births: Vec<(i32, i32, i32)>,
    /// Состояние генератора случайных чисел — см. `random_unit`.
    noise: u32,

    /// Кто сейчас действует рукой (номер сущности игрока): звуки, которые
    /// он так вызвал, ему самому не шлются — см. `note_sound_by_hand`.
    pub actor: Option<i32>,

    /// Случайный такт (farming.rs): в каких секциях чанка есть блоки со
    /// случайным тактом — по номеру правки чанка, чтобы не искать заново.
    pub random_sections: HashMap<(i32, i32), (u32, u32)>,

    /// Номер сущности динамита, чей взрыв сейчас считается (0 — нет): он
    /// назван виновником в уроне игрокам.
    pub blast_source: i32,

    /// Взрыв по вине игрока (ударил кристалл): урон вида «player_explosion».
    pub blast_by_player: bool,

    /// Ключ имени виновника взрыва для сообщения о смерти (крипер): пусто — нет.
    pub blast_killer: &'static str,

    /// Где сидят кошки (середина ног): сундук под сидящей кошкой не открывается
    /// (вики, Cat: «A chest with a cat sitting on top of it becomes unusable»).
    /// Обновляется каждый такт (tick.rs).
    pub wildlife_griefing: bool,
    pub sitting_cats: Vec<(f64, f64, f64)>,

    /// Набеги, заявки на них и звоны колоколов (raid.rs). Не сохраняются
    /// [проверить: у оригинала — raids.dat].
    pub raids: crate::raid::Raids,

    /// Места, о которых клиентам скажут позже: (место, такт, как есть).
    /// «Как есть» — служебный блок не подменяется пустотой.
    late: Vec<((i32, i32, i32), u64, bool)>,

    chunks: HashMap<(i32, i32), Chunk, CoordHash>,

    /// Все события мира по порядку: изменения блоков и действия блоков.
    /// Каждое подключение помнит, сколько записей оно уже разослало своему
    /// клиенту, и досылает остальные. Так изменение, сделанное одним
    /// игроком, доходит до остальных.
    events: Journal<WorldEvent>,

    /// Места, о которых клиентам скажут позже, и когда именно.
    ///

    /// Такты, чей срок пришёл, пока их чанк был выгружен. Ждут, когда чанк
    /// прочитают снова, — как в игре, где такты хранятся вместе с чанком.
    /// Выбросить их нельзя: без них схема в этом чанке умерла бы (MC-711).
    parked: Vec<Scheduled>,


    /// Файл, в котором мир лежит на диске.
    path: PathBuf,

    /// Сколько тактов мир уже прожил.
    ///
    /// Такт — это шаг времени мира, примерно двадцатая доля секунды. По нему
    /// считается всё, что происходит само: пока это только течение жидкостей.
    tick: u64,

    /// Запланированные такты: места, которые надо пересчитать, и когда.
    ///
    /// Очередь упорядочена по сроку, потом по приоритету (меньше — раньше),
    /// потом по порядку записи. Так требует редстоун: повторители, смотрящие
    /// в другие повторители, обязаны сработать раньше остальных, иначе схемы
    /// ведут себя по-разному при одинаковой постройке.
    scheduled: BinaryHeap<Reverse<Scheduled>>,

    /// Что сейчас ждёт своего такта: срок, номер записи и приоритет. По
    /// сроку и номеру видно, ждёт ли место уже, и отличается свежая запись
    /// от устаревшей: перезаписанная на более ранний срок старая остаётся
    /// в очереди и просто пропускается. Приоритет лежит здесь же, чтобы при
    /// записи на диск не искать его перебором очереди.
    ///
    /// Блочные и жидкостные такты ждут порознь: у одного места может быть
    /// и тот и другой, как в игре, — иначе запланированная вода мешала бы
    /// повторителю сработать вовремя.
    waiting: HashMap<Waiting, WaitingTick>,

    /// Номер следующей записи в очереди — для порядка среди равных.
    next_order: u64,

    /// Блоки, изменившиеся с прошлого разбора, и что в них стояло раньше:
    /// редстоун сообщает об изменении соседям, а по прежнему блоку понимает,
    /// кого ещё надо предупредить, когда деталь сняли. Складываются стопкой,
    /// чтобы разбор шёл вглубь — так же, как это делает игра.
    changed: Vec<((i32, i32, i32), i32)>,

    /// Места, из которых блок должен свалиться: под ним пусто.
    /// Сам мир сущностей не заводит, поэтому этим займётся такт.
    falling: Vec<(i32, i32, i32)>,

    /// Насколько часы мира переведены относительно его возраста.
    time_offset: i64,

    /// Сколько раз часы переводили (команда `time`, сон): клиентам часы
    /// уходят в Update Time только после перевода — в обычной ежесекундной
    /// сверке оригинал шлёт пустой список часов (зонд 02.10: `clockUpdates:
    /// []`; запись часов — при входе, `time set` и пропуске ночи, чёрный
    /// ящик bed-sleep-night).
    clock_changes: u64,

    /// Который час поставлен последним переводом: клиенту уходит именно он,
    /// а не час к моменту рассылки (чёрный ящик, bed-sleep-night: `time set
    /// 14000` — 14000, утро после сна — 0).
    clock_set_to: i64,

    /// Семя мира: из него складывается рельеф. Живёт в level.dat, чтобы мир
    /// оставался тем же и после перезапуска.
    seed: i64,

    /// Тип мира: обычный, суперплоский или «Супер плавность». Живёт
    /// в level.dat рядом с семенем.
    kind: WorldKind,

    /// Правила, по которым складывается ещё не сложенный кусок мира.
    /// Под Arc: сеть берёт их себе и складывает чанк, не держа замок мира.
    generator: Arc<Generator>,

    /// Блочные события: то, что делается не в свой запланированный такт,
    /// а отдельной фазой в конце такта мира. Так устроен ход поршня.
    block_events: std::collections::VecDeque<((i32, i32, i32), u8, u8)>,

    /// Память редстоуна: то, что не помещается в состояние блока.
    pub redstone: redstone::Memory,

    /// Блоки, сломанные не игроком, а самим миром: поршень раздавил траву,
    /// у провода убрали опору. Из них должны выпасть предметы, но заводить
    /// их здесь нечем — этим займётся такт.
    destroyed: Vec<((i32, i32, i32), i32)>,

    /// Чанки, изменившиеся с последней записи на диск.
    ///
    /// Записывать файл на каждый блок нельзя: он пишется целиком, а вода за
    /// один такт меняет десятки блоков — сервер только и делал бы, что писал.
    /// Поэтому то, что меняется само, копится и пишется раз в секунду, а
    /// сделанное игроком записывается сразу же: терять постройки нельзя.
    ///
    /// Когда мир будет храниться по регионам, запись подешевеет, и копить
    /// станет незачем.
    dirty: HashSet<(i32, i32)>,

    /// Сложенные заново чанки, которых ещё нет на диске. Их тоже надо
    /// записать, как это делает оригинал: иначе после смены генератора
    /// нетронутая земля сложилась бы по-новому и не сошлась со старой.
    /// Пишутся понемногу за раз (`FRESH_PER_SAVE`), а до записи из памяти
    /// не выгружаются.
    fresh: HashSet<(i32, i32)>,

    /// Сколько раз выгружались чанки — «поколение» выгрузки. Готовый чанк,
    /// взятый до выгрузки своего места, устарел: его могли за это время
    /// изменить и записать (см. `accept`).
    unload_epoch: u64,

    /// Какие чанки в каком поколении выгружены. Старые отметки
    /// выбрасываются (`UNLOAD_MEMORY`), а чанк, пришедший снова, свою
    /// отметку снимает.
    unloaded: HashMap<(i32, i32), u64>,

    /// Точка появления: ищется один раз и хранится в level.dat.
    spawn: std::sync::Mutex<Option<(f64, f64, f64)>>,

    /// Когда мир последний раз записывался — для `save_if_due`.
    last_save: Option<Instant>,

    /// Проёмы порталов этого мира (нижний угол) — чтобы найти парный, не
    /// обходя сотни чанков. Живут в `portals.dat`.
    portals: Vec<(i32, i32, i32)>,

    /// Отпечаток последнего записанного `ticks.dat`: неизменную очередь
    /// (такты в выгруженных чанках лежат неделями) не переписывать.
    ticks_written: Cell<u64>,

    /// Когда и с каким содержимым (без возраста) последний раз писался
    /// `level.dat`: возраст мира меняется каждый такт, но ради него одного
    /// переписывать файл со сбросом на диск каждую секунду незачем.
    level_written: Cell<Option<(Instant, u64)>>,

    /// Места, где блок сменила команда (`setblock`, `fill`): такая смена
    /// того же блока — просто замена состояния, без собственных обновлений
    /// детали (разбирает редстоун).
    commanded: HashSet<(i32, i32, i32)>,

    /// Запись в чанк, которого нет в памяти, заводит его пустым. Только для
    /// проверок, которые строят мир блок за блоком без земли; у настоящего
    /// мира запись в незагруженный чанк отбрасывается (см.
    /// `set_block_silently`).
    create_missing: bool,

    /// Хранилища, чей блок сменился другим: их содержимое высыплется.
    spilled: Vec<((i32, i32, i32), containers::Container)>,

    /// Командные блоки, чей запланированный такт пришёл: команду выполнит
    /// такт уже без замка мира (command_block::run_queued).
    command_runs: Vec<(i32, i32, i32)>,

    /// Спавнеры, что поставили мобов в прошлом такте: в этом клиентам уходят
    /// блок, новые данные (задержка) и событие блока 1/0 (чёрный ящик,
    /// spawner-zombie: на такт позже постановки).
    spawner_followups: Vec<(i32, i32, i32)>,

    /// Сколько игроков смотрят в хранилище (по месту каждой половины).
    viewers: HashMap<(i32, i32, i32), u32>,

    /// Бой с драконом Края (dragon_fight.rs); у прочих миров пуст.
    pub fight: crate::dragon_fight::Fight,

    /// Погода (weather.rs): идёт только у Обычного мира, живёт в level.dat.
    pub weather: crate::weather::Weather,

    /// Предметы, выброшенные выбрасывателем или раздатчиком: где, с какой
    /// скоростью и что. Заводит сущности такт (`take_ejected`).
    ejected: Vec<Ejected>,

    /// Подожжённый динамит: сущности заводит такт (`take_primed`).
    primed: Vec<Primed>,

    /// Блоки, которые большой взрыв особого динамита (custom_tnt.rs) ещё
    /// не дорушил: такт рушит их по частям (`tnt::continue_blasts`), чтобы
    /// ядерный динамит не остановил мир. Не пишется на диск.
    blast_rest: std::collections::VecDeque<(i32, i32, i32)>,

    /// Содержимое хранилищ, каким его видят игроки: снимается в фазе
    /// сущностей, как у оригинала, — то, что воронки перенесли позже в том же
    /// такте, окна увидят на следующем.
    pub published: HashMap<(i32, i32, i32), Vec<Option<crate::inventory::Stack>>>,

    /// Рамки, прочитанные из файла вместе с чанком: такт заводит их в мир
    /// как сущности (`take_loaded_frames`).
    loaded_frames: Vec<crate::frame::Frame>,

    /// Мобы, прочитанные из файла вместе с чанком: такт заводит их в мир
    /// (`take_loaded_mobs`).
    loaded_mobs: Vec<crate::mob::Mob>,

    /// Чанки, только что созданные генерацией (не с диска): в них могут
    /// появиться животные (`take_new_chunks`).
    new_chunks: Vec<(i32, i32)>,

    /// То же для вагонеток-хранилищ: номер сущности — ячейки.
    pub published_carts: HashMap<i32, Vec<Option<crate::inventory::Stack>>>,

    /// Свойства окон печей и стоек, снятые вместе с содержимым
    /// (furnace::properties).
    pub published_props: HashMap<(i32, i32, i32), Vec<i16>>,

    /// Хранилища в порядке появления в мире — для их хода в такте.
    container_order: Vec<(i32, i32, i32)>,

    /// Места, где сменилось содержимое хранилища, — для сравнителей.
    contents_changed: Vec<(i32, i32, i32)>,

    /// Свет чанков, сшитый с соседями, — для датчиков дневного света: они
    /// спрашивают небесный свет раз в 20 тактов. Ключ — чанк, при нём номера
    /// правки девяти чанков, по которым свет считан: сменился хоть один —
    /// считаем заново.
    sky_cache: HashMap<(i32, i32), ([u32; 9], Arc<light::ChunkLight>)>,

    /// Замок папки мира (`session.lock`): пока он взят, второй сервер ту же
    /// папку не откроет. Отпускается вместе с файлом, когда мир закрывается.
    _session: Option<File>,
}

/// Такт, ждущий своего срока: срок, номер записи, приоритет.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct WaitingTick {
    due: u64,
    order: u64,
    priority: i8,

    /// Для какой жидкости заведён такт жидкости (`fluid_type`): он её, и
    /// с ней пропадает — у оригинала такт жидкости исполняется, только если
    /// она всё ещё там, причём источник и текущая вода считаются разными
    /// жидкостями [проверить] (чёрный ящик, dispenser-water: забранный
    /// ведром источник уносит свой такт).
    fluid: u8,
}

/// Сколько сложенных заново чанков записывать за одно сохранение. Сохранение
/// идёт раз в секунду под замком мира, а при входе игрока новых чанков —
/// тысячи: все разом встали бы такту поперёк.
const FRESH_PER_SAVE: usize = 256;

/// Как часто переписывать level.dat, если в нём сменился только возраст.
const LEVEL_PAUSE: Duration = Duration::from_secs(30);

/// Не чаще скольки раз писать мир из `save_if_due`.
const SAVE_PAUSE: Duration = Duration::from_secs(1);

/// Сколько поколений выгрузки помнить, какой чанк когда выгружен. Выгрузка
/// идёт раз в несколько секунд, так что это минуты — чанк столько не
/// складывается. Готовый чанк старше этого считается устаревшим на всякий
/// случай.
const UNLOAD_MEMORY: u64 = 64;

/// Как далеко от начала координат могут лежать чанки: граница мира у
/// оригинала — тридцать миллионов блоков. Дальше чанки не читаются и не
/// складываются: в счёте координат блоков начались бы переполнения.
const CHUNK_LIMIT: i32 = 30_000_000 / CHUNK_SIZE + 1;

/// Лежит ли чанк в пределах мира.
fn chunk_in_limits(chunk_x: i32, chunk_z: i32) -> bool {
    let inside = -CHUNK_LIMIT..=CHUNK_LIMIT;

    inside.contains(&chunk_x) && inside.contains(&chunk_z)
}

impl World {
    /// Пустой мир без файла на диске — только для проверок.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            fire_tick: true,
            fire_difficulty: 2,
            noise: 0x9E37_79B9,
            allay_notes: Vec::new(),
            ominous_births:Vec::new(),
            snifflet_births: Vec::new(),
            actor: None,
            blast_source: 0,
            blast_by_player: false,
            blast_killer: "",
            wildlife_griefing: true,
            sitting_cats: Vec::new(),
            raids: crate::raid::Raids::default(),
            late: Vec::new(),
            chunks: HashMap::default(),
            events: Journal::new(),
            parked: Vec::new(),
            path: PathBuf::new(),
            tick: 0,
            scheduled: BinaryHeap::new(),
            waiting: HashMap::new(),
            next_order: 0,
            changed: Vec::new(),
            falling: Vec::new(),
            time_offset: 0,
            clock_changes: 0,
            clock_set_to: 0,
            seed: 0,
            kind: WorldKind::Flat,
            generator: Arc::new(Generator::Flat),
            block_events: std::collections::VecDeque::new(),
            redstone: redstone::Memory::new(),
            destroyed: Vec::new(),
            dirty: HashSet::new(),
            fresh: HashSet::new(),
            unload_epoch: 0,
            unloaded: HashMap::new(),
            spawn: std::sync::Mutex::new(None),
            last_save: None,
            ticks_written: Cell::new(0),
            level_written: Cell::new(None),
            commanded: HashSet::new(),
            portals: Vec::new(),
            create_missing: true,
            spilled: Vec::new(),
            viewers: HashMap::new(),
            fight: crate::dragon_fight::Fight::default(),
            weather: crate::weather::Weather::default(),
            random_sections: HashMap::new(),
            ejected: Vec::new(),
            primed: Vec::new(),
            blast_rest: Default::default(),
            published: HashMap::new(),
            published_carts: HashMap::new(),
            published_props: HashMap::new(),
            loaded_frames: Vec::new(),
            loaded_mobs: Vec::new(),
            new_chunks: Vec::new(),
            container_order: Vec::new(),
            contents_changed: Vec::new(),
            command_runs: Vec::new(),
            spawner_followups: Vec::new(),
            sky_cache: HashMap::new(),
            _session: None,
        }
    }

    /// Открывает мир: готовит директорию, в которой лежат регионы.
    ///
    /// Сами чанки не читаются: каждый читается тогда, когда до него дошли.
    pub fn open(
        directory: impl AsRef<Path>,
        wanted_seed: Option<i64>,
        wanted_kind: WorldKind,
    ) -> io::Result<Self> {
        let path = directory.as_ref().to_path_buf();

        fs::create_dir_all(path.join(REGIONS))?;

        // Папку мира держит только один сервер: двое писали бы одни и те же
        // регионы вперемешку и испортили бы их.
        let session = lock_session(&path)?;

        // Мир прежнего образца лежал одним файлом. Читать его нечем, и
        // устройство мира с тех пор поменялось, поэтому просто откладываем
        // его в сторону.
        let single_file = path.join("blocks.mcrw");

        if single_file.exists() {
            let old = single_file.with_extension("old");

            log_warn!(
                "Мир прежнего образца отложен: {}. Новый мир хранится по регионам в {}",
                old.display(),
                path.join(REGIONS).display()
            );

            let _ = fs::rename(&single_file, &old);
        }

        // Регионы прежнего вида (`.rcw`) переносятся в `.mcs` до всего
        // остального: дальше мир видит только новые файлы.
        legacy::migrate(&path.join(REGIONS));

        log_info!("Подготовка мира \"{}\"", path.display());
        log_debug!("Мир: регионы в {}", path.join(REGIONS).display());

        // Сколько мир уже прожил, который в нём час и что ждёт своего
        // такта — из файла мира. Нет файла — мир новый, время утреннее.
        let mut level = load_level(&path)?;
        read_ticks(&path.join(TICKS), &mut level);
        let (tick, time_offset) = (level.age, level.time_offset);

        // Семя: у сложенного мира своё, записанное в level.dat, и менять его
        // нельзя — иначе новые чанки не сойдутся со старыми. У нового мира
        // семя берётся из настроек, а если там пусто — выбирается само.
        let seed = match (level.seed, wanted_seed) {
            (Some(saved), wanted) => {
                if let Some(wanted) = wanted
                    && wanted != saved
                {
                    log_warn!(
                        "Семя мира из настроек ({}) не применяется: у мира уже своё ({})",
                        wanted,
                        saved
                    );
                }

                saved
            }
            (None, Some(wanted)) => wanted,
            (None, None) => random_seed(),
        };

        log_info!("Семя мира: {}", seed);

        // Тип мира — так же, как семя: у сложенного мира свой, записанный,
        // и его нельзя сменить правкой настроек, иначе новые чанки не
        // сойдутся со старыми.
        let kind = match level.kind {
            Some(saved) => {
                if saved != wanted_kind {
                    log_warn!(
                        "Тип мира из настроек ({}) не применяется: у мира уже свой ({})",
                        wanted_kind.name(),
                        saved.name()
                    );
                }

                saved
            }
            None => wanted_kind,
        };

        log_info!("Тип мира: {}", kind.name());

        let mut world = Self {
            fire_tick: true,
            fire_difficulty: 2,
            allay_notes: Vec::new(),
            ominous_births:Vec::new(),
            snifflet_births: Vec::new(),
            noise: 0x9E37_79B9 ^ std::process::id(),
            actor: None,
            blast_source: 0,
            blast_by_player: false,
            blast_killer: "",
            wildlife_griefing: true,
            sitting_cats: Vec::new(),
            raids: crate::raid::Raids::default(),
            late: Vec::new(),
            chunks: HashMap::default(),
            events: Journal::new(),
            parked: Vec::new(),
            path,
            tick,
            scheduled: BinaryHeap::new(),
            waiting: HashMap::new(),
            next_order: 0,
            changed: Vec::new(),
            falling: Vec::new(),
            time_offset,
            clock_changes: 0,
            clock_set_to: 0,
            seed,
            kind,
            generator: Arc::new(match kind {
                WorldKind::Flat => Generator::Flat,
                WorldKind::Normal => Generator::Normal(Terrain::new(seed, Style::Vanilla)),
                WorldKind::SuperSmooth => Generator::Normal(Terrain::new(seed, Style::Smooth)),
                WorldKind::Nether => Generator::Nether(nether::Nether::new(seed)),
                WorldKind::End => Generator::End(the_end::End::new(seed)),
            }),
            block_events: std::collections::VecDeque::new(),
            redstone: redstone::Memory::new(),
            destroyed: Vec::new(),
            dirty: HashSet::new(),
            fresh: HashSet::new(),
            unload_epoch: 0,
            unloaded: HashMap::new(),
            spawn: std::sync::Mutex::new(None),
            last_save: None,
            ticks_written: Cell::new(0),
            level_written: Cell::new(None),
            commanded: HashSet::new(),
            portals: Vec::new(),
            create_missing: false,
            spilled: Vec::new(),
            viewers: HashMap::new(),
            fight: level.fight.clone(),
            weather: level.weather.clone(),
            random_sections: HashMap::new(),
            ejected: Vec::new(),
            primed: Vec::new(),
            blast_rest: Default::default(),
            published: HashMap::new(),
            published_carts: HashMap::new(),
            published_props: HashMap::new(),
            loaded_frames: Vec::new(),
            loaded_mobs: Vec::new(),
            new_chunks: Vec::new(),
            container_order: Vec::new(),
            contents_changed: Vec::new(),
            command_runs: Vec::new(),
            spawner_followups: Vec::new(),
            sky_cache: HashMap::new(),
            _session: Some(session),
        };

        if let Some(spawn) = level.spawn {
            *world.spawn.lock().expect("точка появления захвачена другим потоком") = Some(spawn);
        }

        world.portals = read_portals(&world.path.join(PORTALS));

        if !level.waiting.is_empty() {
            log_debug!("Мир: ждут своего такта мест — {}", level.waiting.len());
        }

        for (place, kind, delay, priority) in level.waiting {
            let due = world.tick + delay;

            world.push_scheduled(place, kind, due, priority);
        }

        // Ходы поршней, начатые до остановки сервера.
        for (pos, extending, facing) in level.moving {
            let Some(facing) = redstone::Dir::by_number(facing) else {
                continue;
            };

            let cargo = level
                .cargo
                .iter()
                .filter(|(piston, _, _)| *piston == pos)
                .map(|(_, place, state)| (*place, *state))
                .collect();

            world
                .redstone
                .restore_moving(pos, redstone::Moving::restored(pos, extending, facing, cargo));
        }

        // Новый мир сразу получает свой level.dat: иначе, оборвись работа
        // после записи первых чанков, но до первого level.dat, мир бы
        // больше не открылся (регионы есть, семени нет).
        if level.new {
            world.save_level();
        }

        Ok(world)
    }

    /// Записывает изменившиеся чанки на диск.
    ///
    /// Пишутся только те чанки, которых коснулись, и каждый — в свой регион.
    /// Поэтому запись стоит столько же и в маленьком мире, и в большом.
    /// Чанки одного региона пишутся одной пачкой: файл открывается и
    /// сбрасывается на диск один раз на регион, а не на каждый чанк.
    ///
    /// Сложенных заново, но не тронутых чанков за раз пишется не больше
    /// `FRESH_PER_SAVE`; остальные дождутся следующего раза. Всё сразу
    /// пишет `save_all` — для остановки сервера.
    pub fn save_if_needed(&mut self) {
        self.save(false);
    }

    /// Записывает всё, что ещё не на диске, включая все сложенные заново
    /// чанки, — при остановке сервера.
    pub fn save_all(&mut self) {
        self.save(true);
    }

    /// Записывает мир, если с прошлой записи прошла секунда; иначе ничего не
    /// делает — изменения и так уйдут с ближайшей общей записью в такте.
    ///
    /// Для частых мест вроде постановки блока: там запись на каждое действие
    /// стоила бы сброса на диск под замком мира.
    #[allow(dead_code)] // для постановки блоков и команд (placing.rs, play.rs)
    pub fn save_if_due(&mut self) {
        if self.last_save.is_some_and(|at| at.elapsed() < SAVE_PAUSE) {
            return;
        }

        self.save(false);
    }

    fn save(&mut self, everything: bool) {
        if self.path.as_os_str().is_empty() {
            self.dirty.clear();
            self.fresh.clear();
            return;
        }

        self.last_save = Some(Instant::now());

        let directory = self.path.join(REGIONS);

        // Что писать: всё изменённое и часть сложенного заново.
        let limit = if everything { usize::MAX } else { FRESH_PER_SAVE };
        let untouched: Vec<(i32, i32)> =
            self.fresh.iter().filter(|key| !self.dirty.contains(key)).take(limit).copied().collect();
        let mut wanted: Vec<(i32, i32)> = self.dirty.drain().collect();

        wanted.extend(untouched);

        // Чанки по регионам: координаты чанка и его байты.
        type Batch = Vec<(i32, i32, Vec<u8>)>;

        let mut by_region: HashMap<(i32, i32), Batch> = HashMap::new();

        for (chunk_x, chunk_z) in wanted {
            self.fresh.remove(&(chunk_x, chunk_z));

            let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
                continue;
            };

            by_region
                .entry(region::region_of(chunk_x, chunk_z))
                .or_default()
                .push((chunk_x, chunk_z, chunk.to_bytes()));
        }

        // Пишет свой поток (`writer`): такт диска не ждёт. Не записалось —
        // поток повторяет сам, а байты до тех пор читаются из его памяти.
        for (_, chunks) in by_region {
            writer::write_chunks(&directory, chunks);
        }

        // Файл мира — после чанков, а не до. В нём записаны начатые ходы
        // поршней, а едущие блоки уже сняты с мест в чанках; если сервер
        // прервут между двумя записями, пусть лучше пропадёт запись о ходе,
        // чем в чанках останутся старые блоки, а ход доиграется поверх них.
        // Очередь тактов пишется всегда, когда изменилась; сам level.dat —
        // когда сменилось что-то кроме возраста, раз в полминуты и при
        // остановке.
        if everything {
            self.level_written.set(None);
        }

        self.save_level();

        // При остановке — дождаться, пока всё ляжет на диск.
        if everything {
            writer::flush();
        }
    }

    /// Состояние блока по мировым координатам. Всё, что не задано, — воздух.
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> i32 {
        // Выше неба и ниже дна блоков нет. Спрашивают об этом постоянно —
        // поршень у потолка, провод у дна, — и отвечать надо спокойно,
        // а не падать.
        if !in_bounds(y) {
            return AIR;
        }

        let Some(chunk) = self.chunks.get(&chunk_key(x, z)) else {
            return AIR;
        };

        chunk.block(x, y, z)
    }

    /// Состояние блока, если его чанк в памяти. None — чанк не загружен, и
    /// что там стоит, неизвестно: отвечать «воздух» тут было бы неправдой.
    #[allow(dead_code)] // для механик, которым важно отличить воздух от незагруженного
    pub fn get_block_loaded(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        let chunk = self.chunks.get(&chunk_key(x, z))?;

        Some(if in_bounds(y) { chunk.block(x, y, z) } else { AIR })
    }

    /// Ставит блок и запоминает изменение для рассылки.
    /// Возвращает false, если блок уже был таким.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let old = self.get_block(x, y, z);
        let changed = self.set_block_silently(x, y, z, state);

        if changed {
            if crate::blocks::block_at_state(state) == Some("frogspawn") {
                let delay = 3600 + (self.random_unit() * 8401.0) as u64;
                self.schedule_once(x,y,z,delay,0);
            }

            // Яйцо черепахи на песке: при постановке — событие мира 2012/15 раньше
            // самого блока (чёрный ящик turtle-egg-hatch).
            if crate::blocks::block_at_state(state) == Some("turtle_egg") && crate::blocks::block_at_state(old) != Some("turtle_egg")
                && matches!(crate::blocks::block_at_state(self.get_block(x, y - 1, z)), Some("sand" | "red_sand"))
            {
                self.note_level_event(x, y, z, 2012, 15);
            }

            self.events.push(WorldEvent::Change(BlockChange { x, y, z, state, direct: false }));

            // Соседей надо пересчитать: рядом могла стоять вода, которой
            // теперь есть куда течь, — или наоборот, её путь перекрыли.
            self.schedule_around(x, y, z);
            if crate::blocks::block_at_state(state) == Some("sniffer_egg") && crate::blocks::block_at_state(old) != Some("sniffer_egg") {
                crate::mob::small_world::schedule_egg(self, (x,y,z));
            }

            // А редстоун узнает об изменении при ближайшем разборе.
            self.changed.push(((x, y, z), old));

            // На диск это попадёт вместе с остальными изменениями: запись
            // идёт раз в секунду, а не на каждый блок.
            self.dirty.insert(chunk_key(x, z));
        }

        changed
    }

    /// Debug Stick меняет состояние без обновления соседей (wiki, Debug Stick).
    pub fn set_debug_state(&mut self, (x,y,z): (i32,i32,i32), state: i32) {
        if self.set_block_silently(x,y,z,state) {
            self.events.push(WorldEvent::Change(BlockChange { x,y,z,state,direct:false }));
            self.dirty.insert(chunk_key(x,z));
        }
    }

    /// Ставит блок, не запоминая изменение: для начальной расстановки, когда
    /// рассылать ещё некому.
    ///
    /// В чанк, которого нет в памяти, ничего не пишется (возвращается false):
    /// заведённый на его месте пустой чанк при записи затёр бы настоящий на
    /// диске, а прочитать настоящий потом было бы уже нельзя — место занято.
    pub fn set_block_silently(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        if !in_bounds(y) {
            return false;
        }

        // Номер состояния, не влезающий в хранение, — ошибка того, кто его
        // прислал; падать из-за неё под замком мира нельзя.
        if Packed::try_from(state).is_err() {
            log_warn!("Мир: состояние {} в {} {} {} не поставлено — такого номера нет", state, x, y, z);
            return false;
        }

        let key = chunk_key(x, z);

        let chunk = match self.chunks.entry(key) {
            std::collections::hash_map::Entry::Occupied(chunk) => chunk.into_mut(),
            std::collections::hash_map::Entry::Vacant(place) if self.create_missing => place.insert(Chunk::new()),
            std::collections::hash_map::Entry::Vacant(_) => {
                log_debug!("Мир: блок {} {} {} не поставлен — чанк {} {} не загружен", x, y, z, key.0, key.1);
                return false;
            }
        };

        let changed = chunk.set(x, y, z, state);

        if !chunk.spilled.is_empty() {
            self.spilled.append(&mut chunk.spilled);
        }

        if changed && containers::Kind::of(state).is_some() {
            self.container_order.push((x, y, z));
        }

        // Редстоун помнит горящую руду и датчики света: их такт идёт не по
        // очереди, а сам (случайный такт и раз в 20 тактов).
        if changed {
            self.redstone.note_state((x, y, z), state);
            if matches!(crate::blocks::block_at_state(state), Some("fire" | "soul_fire")) {
                let delay = 30 + (self.random_bits() % 10) as u64;
                self.schedule_block_once(x, y, z, delay);
            }
            // Сердце скрипуна рассылает пустые данные блока при каждой перемене (creaking-freeze: тип 10, {}).
            if crate::blocks::block_at_state(state) == Some("creaking_heart") {
                self.note_block_entity(x, y, z, 10, vec![0x0A, 0x00]);
            }
        }

        changed
    }

    /// Какой особый динамит стоит в этом месте. None — обычный или не
    /// динамит вовсе.
    pub fn custom_tnt_at(&self, (x, y, z): (i32, i32, i32)) -> Option<crate::custom_tnt::KindId> {
        let chunk = self.chunks.get(&chunk_key(x, z))?;

        if chunk.custom_tnt.is_empty() {
            return None;
        }

        let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

        chunk.custom_tnt.iter().find(|(there, _)| *there == place).map(|&(_, kind)| kind)
    }

    /// Запоминает вид особого динамита в этом месте (там должен стоять
    /// динамит — иначе ничего не делается). None — снова обычный.
    pub fn set_custom_tnt(&mut self, (x, y, z): (i32, i32, i32), kind: Option<crate::custom_tnt::KindId>) {
        if !crate::tnt::is_tnt(self.get_block(x, y, z)) {
            return;
        }

        let Some(chunk) = self.chunks.get_mut(&chunk_key(x, z)) else {
            return;
        };
        let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));

        chunk.custom_tnt.retain(|(there, _)| *there != place);

        if let Some(kind) = kind {
            chunk.custom_tnt.push((place, kind));
        }

        self.dirty.insert(chunk_key(x, z));
    }

    /// Запоминает рамку в её чанке — чтобы она легла в файл. Рамка с тем же
    /// местом и стороной заменяется. Чанк не в памяти — молча ничего.
    pub fn store_frame(&mut self, frame: &crate::frame::Frame) {
        let Some(chunk) = self.chunks.get_mut(&chunk_key(frame.pos.0, frame.pos.2)) else {
            return;
        };

        let mut local = frame.clone();

        local.pos = (frame.pos.0.rem_euclid(CHUNK_SIZE), frame.pos.1, frame.pos.2.rem_euclid(CHUNK_SIZE));
        chunk.frames.retain(|there| (there.pos, there.facing) != (local.pos, local.facing));
        chunk.frames.push(local);
        self.dirty.insert(chunk_key(frame.pos.0, frame.pos.2));
    }

    /// Мобы, что пришли вместе с загруженными чанками, — забирает такт.
    pub fn queue_water_hatch(&mut self, mob: crate::mob::Mob) { self.loaded_mobs.push(mob); }

    pub fn take_loaded_mobs(&mut self) -> Vec<crate::mob::Mob> {
        std::mem::take(&mut self.loaded_mobs)
    }

    /// Только что созданные генерацией чанки — забирает такт, чтобы поселить
    /// в них животных.
    pub fn take_new_chunks(&mut self) -> Vec<(i32, i32)> {
        std::mem::take(&mut self.new_chunks)
    }

    /// Записывает мобов в их чанки: список каждого загруженного чанка
    /// заменяется тем, кто в нём сейчас стоит. Чанк без мобов, у которого их
    /// и не было, не трогается — не переписывается зря.
    pub fn store_mobs(&mut self, mobs: &[crate::mob::Mob]) {
        let mut by_chunk: HashMap<(i32, i32), Vec<crate::mob::Mob>> = HashMap::new();

        for mob in mobs.iter().filter(|mob| !mob.is_dead() && mob.persistent) {
            let (x, z) = mob.chunk();

            by_chunk.entry((x, z)).or_default().push(mob.clone());
        }

        let mut dirty = Vec::new();

        for (key, chunk) in &mut self.chunks {
            let now = by_chunk.remove(key).unwrap_or_default();

            if now.is_empty() && chunk.mobs.is_empty() {
                continue;
            }

            chunk.mobs = now;
            dirty.push(*key);
        }

        self.dirty.extend(dirty);
    }

    /// Стирает запись рамки: она сломана или отвалилась.
    pub fn forget_frame(&mut self, pos: (i32, i32, i32), facing: u8) {
        let Some(chunk) = self.chunks.get_mut(&chunk_key(pos.0, pos.2)) else {
            return;
        };

        let local = (pos.0.rem_euclid(CHUNK_SIZE), pos.1, pos.2.rem_euclid(CHUNK_SIZE));

        chunk.frames.retain(|there| (there.pos, there.facing) != (local, facing));
        self.dirty.insert(chunk_key(pos.0, pos.2));
    }

    /// Рамки, что пришли вместе с загруженными чанками, — забирает такт.
    pub fn take_loaded_frames(&mut self) -> Vec<crate::frame::Frame> {
        std::mem::take(&mut self.loaded_frames)
    }

    /// Хранилище в этом месте, если оно там есть и его чанк в памяти.
    pub fn container(&self, x: i32, y: i32, z: i32) -> Option<&containers::Container> {
        self.chunks.get(&chunk_key(x, z))?.containers.get(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Правит хранилище в этом месте. Чанк помечается изменённым, а место —
    /// «содержимое сменилось»: сравнители рядом пересчитаются
    /// (`take_contents_changed`). None — хранилища нет.
    pub fn with_container<R>(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        change: impl FnOnce(&mut containers::Container) -> R,
    ) -> Option<R> {
        let chunk = self.chunks.get_mut(&chunk_key(x, z))?;
        let container = chunk.containers.get_mut(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))?;
        let before = container.slots.clone();
        let result = change(container);

        if container.slots != before {
            // Печь начинает плавку заново, если вход сменили.
            crate::furnace::after_slots_changed(container, &before);
            self.contents_changed.push((x, y, z));
        }

        self.dirty.insert(chunk_key(x, z));

        Some(result)
    }

    /// Данные командного блока в этом месте, если он там стоит и чанк в памяти.
    pub fn command_block(&self, x: i32, y: i32, z: i32) -> Option<&crate::command_block::Data> {
        self.chunks.get(&chunk_key(x, z))?.command_blocks.get(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Правит данные командного блока; чанк помечается изменённым. None —
    /// командного блока тут нет.
    pub fn with_command_block<R>(&mut self, x: i32, y: i32, z: i32, change: impl FnOnce(&mut crate::command_block::Data) -> R) -> Option<R> {
        let chunk = self.chunks.get_mut(&chunk_key(x, z))?;

        // Блок без записи (мир чужой записи, правка руками) получает чистую.
        crate::command_block::mode_of(chunk.block(x, y, z))?;

        let data = chunk.command_blocks.entry((x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))).or_default();
        let result = change(data);

        self.dirty.insert(chunk_key(x, z));

        Some(result)
    }

    /// Данные улья в этом месте, если он там стоит и чанк в памяти.
    pub fn hive(&self, x: i32, y: i32, z: i32) -> Option<&crate::hive::Data> {
        self.chunks.get(&chunk_key(x, z))?.hives.get(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Правит данные улья; чанк помечается изменённым. None — улья тут нет.
    pub fn with_hive<R>(&mut self, x: i32, y: i32, z: i32, change: impl FnOnce(&mut crate::hive::Data) -> R) -> Option<R> {
        let chunk = self.chunks.get_mut(&chunk_key(x, z))?;
        let data = chunk.hives.get_mut(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))?;
        let result = change(data);

        self.dirty.insert(chunk_key(x, z));

        Some(result)
    }

    /// Все ульи загруженных чанков: место в мире.
    pub fn hive_places(&self) -> Vec<(i32, i32, i32)> {
        let mut found = Vec::new();

        for (&(chunk_x, chunk_z), chunk) in &self.chunks {
            found.extend(chunk.hives.keys().map(|&(x, y, z)| (chunk_x * CHUNK_SIZE + x, y, chunk_z * CHUNK_SIZE + z)));
        }

        found.sort_unstable();
        found
    }

    /// Данные спавнера в этом месте, если он там стоит и чанк в памяти.
    pub fn spawner(&self, x: i32, y: i32, z: i32) -> Option<&crate::spawner::Data> {
        self.chunks.get(&chunk_key(x, z))?.spawners.get(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Правит данные спавнера; чанк помечается изменённым. None — спавнера тут нет.
    pub fn with_spawner<R>(&mut self, x: i32, y: i32, z: i32, change: impl FnOnce(&mut crate::spawner::Data) -> R) -> Option<R> {
        let chunk = self.chunks.get_mut(&chunk_key(x, z))?;
        let data = chunk.spawners.get_mut(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))?;
        let result = change(data);

        self.dirty.insert(chunk_key(x, z));

        Some(result)
    }

    /// Все спавнеры загруженных чанков: место в мире.
    pub fn spawner_places(&self) -> Vec<(i32, i32, i32)> {
        let mut found = Vec::new();

        for (&(chunk_x, chunk_z), chunk) in &self.chunks {
            found.extend(chunk.spawners.keys().map(|&(x, y, z)| (chunk_x * CHUNK_SIZE + x, y, chunk_z * CHUNK_SIZE + z)));
        }

        found.sort_unstable();
        found
    }

    /// Данные спавнеров чанка для пакета чанка: место внутри чанка и NBT.
    pub fn chunk_spawners(&self, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), Vec<u8>)> {
        self.chunks.get(&(chunk_x, chunk_z)).map(|chunk| chunk.spawners.iter().map(|(&place, data)| (place, data.nbt())).collect()).unwrap_or_default()
    }

    pub fn with_archaeology<R>(&mut self, at:(i32,i32,i32), change:impl FnOnce(&mut crate::archaeology::Data)->R)->Option<R> {
        let chunk=self.chunks.get_mut(&chunk_key(at.0,at.2))?;
        let data=chunk.archaeology.get_mut(&(at.0.rem_euclid(16),at.1,at.2.rem_euclid(16)))?;
        let result=change(data);self.dirty.insert(chunk_key(at.0,at.2));Some(result)
    }
    pub fn archaeology_places(&self)->Vec<(i32,i32,i32)> {
        self.chunks.iter().flat_map(|(&(cx,cz),c)|c.archaeology.iter().filter(|(_,d)|d.strokes>0 && d.last.is_some()).map(move |(&(x,y,z),_)|(cx*16+x,y,cz*16+z))).collect()
    }

    /// Испытательный спавнер или хранилище в этом месте (trial.rs).
    pub fn trial(&self, x: i32, y: i32, z: i32) -> Option<&crate::trial::Data> {
        self.chunks.get(&chunk_key(x, z))?.trials.get(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Правит данные испытательного блока; чанк помечается изменённым.
    pub fn with_trial<R>(&mut self, x: i32, y: i32, z: i32, change: impl FnOnce(&mut crate::trial::Data) -> R) -> Option<R> {
        let chunk = self.chunks.get_mut(&chunk_key(x, z))?;
        let data = chunk.trials.get_mut(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))?;
        let result = change(data);

        self.dirty.insert(chunk_key(x, z));

        Some(result)
    }

    /// Все испытательные блоки загруженных чанков: место в мире.
    pub fn trial_places(&self) -> Vec<(i32, i32, i32)> {
        let mut found = Vec::new();

        for (&(chunk_x, chunk_z), chunk) in &self.chunks {
            found.extend(chunk.trials.keys().map(|&(x, y, z)| (chunk_x * CHUNK_SIZE + x, y, chunk_z * CHUNK_SIZE + z)));
        }

        found.sort_unstable();
        found
    }

    /// Данные испытательных блоков чанка для пакета чанка: место и NBT.
    pub fn chunk_trials(&self, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), Vec<u8>)> {
        let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return Vec::new();
        };

        chunk
            .trials
            .iter()
            .map(|(&place, data)| {
                let ominous = crate::trial::ominous_state(chunk.block(place.0, place.1, place.2));

                (place, data.nbt(ominous).1)
            })
            .collect()
    }

    /// Спавнер поставил мобов: рассказать клиентам в следующем такте.
    pub fn queue_spawner_followup(&mut self, x: i32, y: i32, z: i32) {
        self.spawner_followups.push((x, y, z));
    }

    /// Забирает спавнеры, о которых пора рассказать: блок, данные, событие 1/0.
    pub fn flush_spawner_followups(&mut self) {
        for (x, y, z) in std::mem::take(&mut self.spawner_followups) {
            let state = self.get_block(x, y, z);

            let (Some(block), Some(nbt)) = (crate::blocks::block_at_state(state).and_then(crate::blocks::block_id), self.spawner(x, y, z).map(crate::spawner::Data::nbt)) else {
                continue;
            };

            self.tell_directly(x, y, z);
            self.note_block_entity(x, y, z, crate::spawner::BLOCK_ENTITY, nbt);
            self.note_action(BlockAction { x, y, z, action: 1, param: 0, block });
        }
    }

    /// Командный блок дождался своего такта: команду выполнит такт.
    pub fn queue_command_run(&mut self, x: i32, y: i32, z: i32) {
        self.command_runs.push((x, y, z));
    }

    /// Забирает командные блоки, ждущие выполнения.
    pub fn take_command_runs(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.command_runs)
    }

    /// Хранилища чанка для пакета чанка: место внутри чанка (x, z), высота
    /// и вид.
    pub fn chunk_containers(&self, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, i32, containers::Kind)> {
        let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return Vec::new();
        };
        let mut found: Vec<_> = chunk.containers.iter().map(|(&(x, y, z), c)| (x, y, z, c.kind)).collect();

        found.sort_unstable_by_key(|&(x, y, z, _)| (y, z, x));
        found
    }

    /// Сколько игроков смотрят в хранилище в этом месте.
    pub fn viewers(&self, x: i32, y: i32, z: i32) -> u32 {
        self.viewers.get(&(x, y, z)).copied().unwrap_or(0)
    }

    /// Меняет число смотрящих и возвращает новое.
    pub fn add_viewers(&mut self, x: i32, y: i32, z: i32, delta: i32) -> u32 {
        let count = self.viewers.entry((x, y, z)).or_insert(0);

        *count = (*count as i32 + delta).max(0) as u32;

        let now = *count;

        if now == 0 {
            self.viewers.remove(&(x, y, z));
        }

        now
    }

    /// Полоса босса дракона для всех в этом мире.
    pub fn note_progress_near(&mut self, id: &'static str, at: (f64, f64, f64), radius: (f64, f64, f64)) {
        self.events.push(WorldEvent::ProgressNear { id, at, radius });
    }

    pub fn note_boss_bar(&mut self, share: Option<f32>) {
        self.events.push(WorldEvent::BossBar(share));
    }

    pub fn note_wither_bar(&mut self, entity_id: i32, share: f32) {
        self.events.push(WorldEvent::WitherBar { entity_id, share });
    }

    /// Точка такта, где слежение рассылает клиентам мобов.
    pub fn note_tracker(&mut self) {
        self.events.push(WorldEvent::Tracker);
    }

    /// Точка такта, где клиентам показывается только что вышедший моб.
    pub fn note_mob_spawn(&mut self, mob: crate::mob::Mob) {
        self.events.push(WorldEvent::MobSpawn(Box::new(mob)));
    }

    /// Точка такта, где слежение рассылает клиентам стрелы.
    pub fn note_arrow_tracker(&mut self) {
        self.events.push(WorldEvent::ArrowTracker);
    }

    /// Точка такта, где слежение рассылает брошенные зелья и облака.
    pub fn note_rocket_blast(&mut self, entity_id: i32) {
        self.events.push(WorldEvent::RocketBlast { entity_id });
    }

    pub fn note_eye_launch(&mut self, entity_id: i32) {
        self.events.push(WorldEvent::EyeLaunch { entity_id });
    }

    pub fn note_thrown_tracker(&mut self, early: bool) {
        self.events.push(WorldEvent::ThrownTracker { early });
    }

    /// Эффект зелья игроку: см. `WorldEvent::PotionEffect`.
    pub fn note_potion_effect(&mut self, entity_id: i32, effect: crate::survival::Effect) {
        self.events.push(WorldEvent::PotionEffect { entity_id, effect });
    }

    /// Лечение зельем: см. `WorldEvent::PotionHeal`.
    pub fn note_potion_heal(&mut self, entity_id: i32, amount: f32) {
        self.events.push(WorldEvent::PotionHeal { entity_id, amount });
    }

    /// Точка такта, где накопленные изменения блоков уходят клиентам.
    pub fn note_broadcast(&mut self) {
        self.events.push(WorldEvent::Broadcast);
    }

    /// Событие мира, которое клиент проигрывает сам (World Event).
    pub fn note_level_event(&mut self, x: i32, y: i32, z: i32, effect: i32, data: i32) {
        self.events.push(WorldEvent::Level { x, y, z, effect, data, except: None });
    }

    /// Событие мира от руки игрока (`actor`): всем, кроме него самого.
    pub fn note_level_event_by_hand(&mut self, x: i32, y: i32, z: i32, effect: i32, data: i32) {
        let except = self.actor;

        self.events.push(WorldEvent::Level { x, y, z, effect, data, except });
    }

    /// Вибрация к датчику скалка — частица на месте события.
    pub fn note_trail(&mut self, from: (f64,f64,f64), to: (f64,f64,f64), color: i32, ticks: i32) {
        self.events.push(WorldEvent::Trail {from,to,color,ticks});
    }

    pub fn note_vibration(&mut self, from: (i32, i32, i32), to: (i32, i32, i32), ticks: i32) {
        self.events.push(WorldEvent::Vibration { from, to, ticks });
    }

    /// Вибрация от точного места (хранитель слышит событие сущности).
    pub fn note_vibration_at(&mut self, from: (f64, f64, f64), to: (i32, i32, i32), ticks: i32) {
        self.events.push(WorldEvent::VibrationAt { from, to, ticks });
    }

    /// Нота над проигрывателем — частицей в общем потоке событий.
    pub fn note_music_particle(&mut self, at: (f64, f64, f64), color: f32) {
        self.events.push(WorldEvent::Note { at, color });
    }

    /// Окно хранилища для игрока — в общем потоке событий.
    pub fn note_open_window(&mut self, viewer: i32, x: i32, y: i32, z: i32) {
        self.events.push(WorldEvent::OpenWindow { viewer, x, y, z });
    }

    /// Окно правки таблички открыто игроку `viewer`.
    pub fn note_open_sign(&mut self, viewer: i32, x: i32, y: i32, z: i32, front: bool) {
        self.events.push(WorldEvent::OpenSign { viewer, x, y, z, front });
    }

    /// Данные таблички в этом месте, если они есть (у чистой их нет).
    pub fn sign(&self, x: i32, y: i32, z: i32) -> Option<&crate::sign::Data> {
        self.chunks.get(&chunk_key(x, z))?.signs.get(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Правит данные таблички (заводит, если их не было); чанк помечается
    /// изменённым. Табличка без текста данных не держит.
    pub fn with_sign<R>(&mut self, x: i32, y: i32, z: i32, change: impl FnOnce(&mut crate::sign::Data) -> R) -> Option<R> {
        let chunk = self.chunks.get_mut(&chunk_key(x, z))?;
        let place = (x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));
        let data = chunk.signs.entry(place).or_default();
        let result = change(data);

        if data.is_default() {
            chunk.signs.remove(&place);
        }

        self.dirty.insert(chunk_key(x, z));

        Some(result)
    }

    /// Таблички чанка с их данными для клиента: место внутри чанка и сетевой
    /// NBT.
    pub fn chunk_signs(&self, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), Vec<u8>)> {
        self.chunks.get(&(chunk_x, chunk_z)).map(|chunk| chunk.signs.iter().map(|(&place, data)| (place, data.nbt())).collect()).unwrap_or_default()
    }

    /// Выброшенный предмет: сущность заведёт такт.
    pub fn eject(&mut self, at: (f64, f64, f64), velocity: (f64, f64, f64), stack: crate::inventory::Stack) {
        self.ejected.push((at, velocity, stack));
    }

    /// Забирает выброшенные предметы.
    pub fn take_ejected(&mut self) -> Vec<Ejected> {
        std::mem::take(&mut self.ejected)
    }

    /// Подожжённый динамит: сущность заведёт такт.
    pub fn prime(&mut self, at: (f64, f64, f64), velocity: (f64, f64, f64), fuse: i32, custom: Option<crate::custom_tnt::KindId>) {
        self.primed.push((at, velocity, fuse, custom));
    }

    /// Забирает подожжённый динамит.
    pub fn take_primed(&mut self) -> Vec<Primed> {
        std::mem::take(&mut self.primed)
    }

    /// Откладывает недорушенное большим взрывом на следующие такты.
    pub fn defer_blast(&mut self, rest: impl IntoIterator<Item = (i32, i32, i32)>) {
        self.blast_rest.extend(rest);
    }

    /// Есть ли недорушенное большим взрывом.
    pub fn blast_pending(&self) -> bool {
        !self.blast_rest.is_empty()
    }

    /// Следующая часть отложенного, не больше `limit` мест. Пусто — нечего.
    pub fn take_blast_batch(&mut self, limit: usize) -> Vec<(i32, i32, i32)> {
        let count = limit.min(self.blast_rest.len());

        self.blast_rest.drain(..count).collect()
    }

    /// Сообщает игроку об уроне (взрыв): см. `WorldEvent::Hurt`.
    pub fn note_hurt(&mut self, entity_id: i32, amount: f32, source: i32) {
        self.events.push(WorldEvent::Hurt { entity_id, amount, source, killer: self.blast_killer });
    }

    /// Урон игроку от моба или стрелы: см. `WorldEvent::MobHurt`.
    #[allow(clippy::too_many_arguments)]
    pub fn note_mob_hurt(&mut self, entity_id: i32, amount: f32, source: crate::survival::Source, attacker: i32, killer: &'static str, push: Option<(f64, f64)>, at: (f64, f64, f64)) {
        self.events.push(WorldEvent::MobHurt { entity_id, amount, source, attacker, killer, push, at });
    }

    /// Эффект игроку от моба: см. `WorldEvent::MobEffect`.
    pub fn note_mob_effect(&mut self, entity_id: i32, effect: crate::survival::Effect) {
        self.events.push(WorldEvent::MobEffect { entity_id, effect });
    }

    pub fn note_axolotl_assist(&mut self, entity_id: i32) {
        self.events.push(WorldEvent::AxolotlAssist { entity_id });
    }

    /// Эффект от удара моба: см. `WorldEvent::MobHitEffect`.
    pub fn note_mob_hit_effect(&mut self, entity_id: i32, effect: crate::survival::Effect) {
        self.events.push(WorldEvent::MobHitEffect { entity_id, effect });
    }

    /// Открыто ли небо над клеткой: выше только воздух и прозрачное (стекло
    /// солнце не держит, вики, Zombie: «Transparent blocks, such as glass,
    /// do not prevent them from burning»). Незагруженный чанк — нет.
    pub fn open_sky(&self, x: i32, y: i32, z: i32) -> bool {
        let (chunk_x, chunk_z) = chunk_key(x, z);
        let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return false;
        };

        // Ниже дна мира (упавший в пустоту моб) неба над клеткой нет [проверить].
        if y < MIN_Y {
            return false;
        }

        (y + 1..=chunk.highest()).all(|above| {
            let state = chunk.block(x, above, z);

            state == AIR || light::filter(state) == 0
        })
    }

    /// Идёт ли в этой клетке дождь (вики, Weather): дождь в мире, выше
    /// клетки — ничего, что держит движение или жидкость (стекло и листва
    /// дождь держат), и биом клетки дождливый — не снежный (холоднее 0,15 с
    /// поправкой на высоту) и не сухой (пустыня, саванна, пустошь: «not in
    /// certain dry biomes»).
    pub fn rain_at(&self, x: i32, y: i32, z: i32) -> bool {
        if !self.weather.is_raining() {
            return false;
        }

        let (chunk_x, chunk_z) = chunk_key(x, z);
        let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return false;
        };

        // Ниже дна мира (упавший в пустоту) дождя нет: без этой проверки
        // `chunk.block` получал y далеко под миром и ронял такт (регрессия
        // полного прогона чёрного ящика: упавший вечно игрок под дождём).
        if y < MIN_Y {
            return false;
        }

        let open = (y..=chunk.highest()).all(|above| {
            let state = chunk.block(x, above, z);

            state == AIR || (!crate::blocks::stops_items(state) && fluids::fluid_at(state).is_none())
        });

        if !open || !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
            return open;
        }

        let number = chunk.biome(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));
        let biome = if number == terrain::autumn_forest_number() {
            Some(terrain::Biome::Forest)
        } else {
            terrain::Biome::ALL.get(usize::from(number)).copied()
        };

        biome.is_some_and(|biome| biome.climate().0 < 2.0 && !biome.snows_at(y))
    }

    /// Температура биома в клетке с поправкой на высоту (выше 80 холоднее на
    /// 1/800 за блок, как у `snows_at`): по ней снежный голем оставляет снег
    /// и тает (вики, Snow Golem). Незер жарок (2,0), Энд — 0,5; неизвестный
    /// биом — как равнины (0,8) [проверить].
    pub fn temperature_at(&self, x: i32, y: i32, z: i32) -> f32 {
        if self.is_nether() {
            return 2.0;
        }

        if self.is_end() {
            return 0.5;
        }

        let (chunk_x, chunk_z) = chunk_key(x, z);
        let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return 0.8;
        };

        if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
            return 0.8;
        }

        let number = chunk.biome(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE));
        let biome = if number == terrain::autumn_forest_number() { Some(terrain::Biome::Forest) } else { terrain::Biome::ALL.get(usize::from(number)).copied() };

        biome.map_or(0.8, |biome| biome.climate().0 - 0.00125 * ((y - 80).max(0) as f32))
    }

    /// Клетка над самым верхним блоком столбца, что держит движение или
    /// жидкость (вики, Lightning: «the block just above the highest block
    /// that is liquid or obstructs movement»). Незагруженный — None.
    pub fn rain_top(&self, x: i32, z: i32) -> Option<i32> {
        let (chunk_x, chunk_z) = chunk_key(x, z);
        let chunk = self.chunks.get(&(chunk_x, chunk_z))?;
        let mut y = chunk.highest();

        while y >= MIN_Y {
            let state = chunk.block(x, y, z);

            if state != AIR && (crate::blocks::stops_items(state) || fluids::fluid_at(state).is_some()) {
                break;
            }

            y -= 1;
        }

        Some(y + 1)
    }

    /// Верхняя непустая клетка столбца для карты; незагруженный чанк не рисуется.
    pub fn column_top(&self, x: i32, z: i32) -> Option<i32> {
        let chunk = self.chunks.get(&chunk_key(x, z))?;
        let mut y = chunk.highest();
        while y >= MIN_Y && chunk.block(x, y, z) == AIR { y -= 1; }
        Some(y)
    }

    /// Под дождём ли сущность: в клетке ног или макушки (`height` — рост)
    /// [проверить]: какие клетки смотрит оригинал.
    pub fn in_rain(&self, x: f64, y: f64, z: f64, height: f64) -> bool {
        let (bx, bz) = (x.floor() as i32, z.floor() as i32);

        self.rain_at(bx, y.floor() as i32, bz) || self.rain_at(bx, (y + height).floor() as i32, bz)
    }

    /// Небесный и блочный свет в клетке, с учётом соседей: считается и
    /// запоминается (как `sky_light`), для мобов — спавн и пауки.
    pub fn light_full(&mut self, x: i32, y: i32, z: i32) -> (u8, u8) {
        if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
            return (light::FULL, 0);
        }

        let (chunk_x, chunk_z) = chunk_key(x, z);

        if !self.chunks.contains_key(&(chunk_x, chunk_z)) {
            return (light::FULL, 0);
        }

        let revisions: [u32; 9] = std::array::from_fn(|slot| {
            let place = (chunk_x + slot as i32 % 3 - 1, chunk_z + slot as i32 / 3 - 1);

            self.chunks.get(&place).map_or(u32::MAX, |chunk| chunk.revision)
        });
        let (local_x, local_z) = (x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));

        if let Some((seen, light)) = self.sky_cache.get(&(chunk_x, chunk_z))
            && *seen == revisions
        {
            return light.at(local_x, y, local_z);
        }

        let Some(job) = self.light_job(chunk_x, chunk_z) else {
            return (light::FULL, 0);
        };
        let (light, recounted) = job.finish();

        self.store_light(recounted);

        let level = light.at(local_x, y, local_z);

        self.sky_cache.insert((chunk_x, chunk_z), (revisions, Arc::new(light)));
        level
    }

    /// То же, но только из уже посчитанного (`light_full`): без замка на
    /// записи. Нет в запоминании — None.
    pub fn light_cached(&self, x: i32, y: i32, z: i32) -> Option<(u8, u8)> {
        if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
            return None;
        }

        let (chunk_x, chunk_z) = chunk_key(x, z);
        let (_, light) = self.sky_cache.get(&(chunk_x, chunk_z))?;

        Some(light.at(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE)))
    }

    /// Удар игрока по игроку — жертве (`WorldEvent::PlayerStrike`).
    pub fn note_player_strike(&mut self, strike: crate::combat::PlayerStrike) {
        self.events.push(WorldEvent::PlayerStrike(std::sync::Arc::new(strike)));
    }

    /// Частицы для всех, кто рядом.
    pub fn note_particle(&mut self, kind: i32, block: Option<i32>, at: (f64, f64, f64), offset: (f32, f32, f32), speed: f32, count: i32) {
        self.events.push(WorldEvent::Particle { kind, block, at, offset, speed, count, skip: 0 });
    }

    /// Частицы для всех рядом, кроме сущности `skip` (ей они уже ушли).
    #[allow(clippy::too_many_arguments)]
    pub fn note_particle_except(&mut self, skip: i32, kind: i32, block: Option<i32>, at: (f64, f64, f64), offset: (f32, f32, f32), speed: f32, count: i32) {
        self.events.push(WorldEvent::Particle { kind, block, at, offset, speed, count, skip });
    }

    /// Подбор стрелы игроком.
    pub fn note_collect(&mut self, entity_id: i32, by: i32, count: i32) {
        self.events.push(WorldEvent::Collect { entity_id, by, count });
    }

    /// Такт погоды (weather.rs): у мира без неба её нет. Изменения уходят
    /// всем игрокам мира Game Event-ами.
    pub fn tick_weather(&mut self, cycle: bool) {
        if self.is_nether() || self.is_end() {
            return;
        }

        let mut weather = std::mem::take(&mut self.weather);
        let events = weather.tick(cycle, &mut |n| self.random_bits() % n);

        self.weather = weather;

        for (reason, value) in events {
            self.events.push(WorldEvent::Weather { reason, value });
        }
    }

    /// Молния задела игрока.
    pub fn note_struck(&mut self, entity_id: i32) {
        self.events.push(WorldEvent::Struck { entity_id });
    }

    /// Game Event игроку `entity_id`.
    pub fn note_elder_curse(&mut self, entity_id: i32, silent: bool) {
        self.events.push(WorldEvent::ElderCurse { entity_id, silent });
    }

    pub fn note_game_event(&mut self, entity_id: i32, reason: u8, value: f32) {
        self.events.push(WorldEvent::GameEvent { entity_id, reason, value });
    }

    /// Урон игрока — остальным игрокам Damage Event.
    pub fn note_command_damage(&mut self, entity_id: i32, type_id: i32, culprit: i32, direct: i32, at: Option<(f64, f64, f64)>) {
        self.events.push(WorldEvent::PlayerDamaged { entity_id, type_id, culprit, direct, at });
    }

    pub fn note_equipment_break(&mut self, entity_id: i32, status: u8) {
        self.events.push(WorldEvent::EquipmentBreak { entity_id, status });
    }

    pub fn note_player_damaged(&mut self, entity_id: i32, type_id: i32, culprit: i32, direct: i32) {
        self.events.push(WorldEvent::PlayerDamaged { entity_id, type_id, culprit, direct, at: None });
    }

    /// Звук от самой сущности (раздел «игроки»): клиенты играют его там,
    /// где она есть, — «тягун» (чёрный ящик, trident-riptide: Entity Sound
    /// item.trident.riptide_N и самому бросившему, и соседям).
    pub fn note_entity_sound(&mut self, entity_id: i32, name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::EntitySound { entity_id, name, volume, pitch });
    }

    /// Анимация сущности всем, кто её видит.
    pub fn note_animation(&mut self, entity_id: i32, kind: u8) {
        self.events.push(WorldEvent::Animation { entity_id, kind });
    }

    /// Звук игрока, который сам его не слышит (ушиб: `except` — его сущность).
    pub fn note_sound_player_except(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32, except: i32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: false,
            of_player: true,
            except: Some(except),
        }));
    }

    /// Звук существа (раздел «существа») в точке: стрела в блоке.
    pub fn note_sound_neutral_at(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: false,
            of_player: false,
            except: None,
        }));
    }

    /// Звук окружения (раздел ambient) в точке: запуск ракеты.
    pub fn note_sound_ambient_at(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: true,
            of_player: true,
            except: None,
        }));
    }

    /// Звук игрока с высотой и громкостью (выстрел, щит).
    pub fn note_sound_player_at(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: false,
            of_player: true,
            except: None,
        }));
    }

    /// Сообщает клиентам о взрыве.
    pub fn note_explosion(&mut self, blast: Blast) {
        self.events.push(WorldEvent::Explosion(std::sync::Arc::new(blast)));
    }

    /// Снимает содержимое хранилищ, в которые кто-то смотрит, — это увидят
    /// окна, — и ставит в поток метку, где это случилось.
    pub fn publish_contents(&mut self, carts: HashMap<i32, Vec<Option<crate::inventory::Stack>>>) {
        self.published_carts = carts;

        if self.viewers.is_empty() && self.published_carts.is_empty() {
            self.published.clear();
            self.published_props.clear();
            return;
        }

        let places: Vec<_> = self.viewers.keys().copied().collect();

        self.published_props = places
            .iter()
            .filter_map(|&(x, y, z)| Some(((x, y, z), crate::furnace::properties(self.container(x, y, z)?))))
            .filter(|(_, props)| !props.is_empty())
            .collect();

        self.published = places
            .into_iter()
            .filter_map(|(x, y, z)| Some(((x, y, z), self.container(x, y, z)?.slots.clone())))
            .collect();
        self.events.push(WorldEvent::Contents);
    }

    /// Места, где с прошлого раза сменилось содержимое хранилища.
    pub fn take_contents_changed(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.contents_changed)
    }

    /// Хранилища, чей блок сменился другим, — с их содержимым.
    pub fn take_spilled(&mut self) -> Vec<((i32, i32, i32), containers::Container)> {
        std::mem::take(&mut self.spilled)
    }

    /// Хранилища мира в порядке появления — в нём они и работают в такте,
    /// как у оригинала. Места, где хранилища уже нет, по пути вычищаются.
    pub fn containers_in_order(&mut self) -> Vec<(i32, i32, i32)> {
        let chunks = &self.chunks;

        self.container_order.retain(|&(x, y, z)| {
            chunks
                .get(&chunk_key(x, z))
                .is_some_and(|chunk| chunk.containers.contains_key(&(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))))
        });

        let mut seen = HashSet::new();

        self.container_order.retain(|place| seen.insert(*place));
        self.container_order.clone()
    }


    /// Ставит блок, ничего не сообщая клиентам.
    ///
    /// Нужно для служебных блоков, о которых клиент знать не должен:
    /// «движущийся поршень» сервер держит у себя, чтобы помнить, что куда
    /// едет, а клиент рисует движение сам.
    pub fn set_block_quietly(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let old = self.get_block(x, y, z);
        let changed = self.set_block_silently(x, y, z, state);

        if changed {
            self.schedule_around(x, y, z);
            self.changed.push(((x, y, z), old));
            self.dirty.insert(chunk_key(x, z));
        }

        changed
    }




    /// Помечает чанк изменившимся — для проверок, которые правят блоки
    /// в обход обычного пути.
    #[cfg(test)]
    pub fn mark_dirty_for_tests(&mut self, chunk_x: i32, chunk_z: i32) {
        self.dirty.insert((chunk_x, chunk_z));
    }

    /// Блоки одной секции чанка в порядке пакета чанка.
    ///
    /// None — секции нет вовсе: там воздух. Клиенту такая секция и
    /// отправляется как пустая.
    pub fn section_blocks(&self, chunk_x: i32, chunk_z: i32, section: usize) -> Option<&[Packed]> {
        self.chunks
            .get(&(chunk_x, chunk_z))
            .and_then(|chunk| chunk.section(section))
    }

    /// Тело пакета чанка: секции и биомы в том виде, в каком они уходят
    /// клиенту. Само составление передано сюда снаружи — мир не знает, как
    /// устроены пакеты.
    ///
    /// Готовое тело нарочно не запоминается: на дальности в тридцать два
    /// чанка это триста мегабайт памяти, а собирается оно быстрее, чем
    /// уходит по сети (проверено замером — скорость та же).
    pub fn chunk_packet<T>(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        build: impl FnOnce(&[Option<Arc<[Packed]>>], &[BiomeId]) -> T,
    ) -> Option<T> {
        let chunk = self.chunks.get(&(chunk_x, chunk_z))?;

        Some(build(&chunk.sections, &chunk.biomes))
    }

    /// Всё, что нужно, чтобы посчитать свет чанка вне захвата мира: блоки и
    /// собственный свет его и восьми соседей. None — чанка нет в памяти.
    ///
    /// Блоки не копируются: секции общие с миром (см. `Chunk::sections`).
    pub fn light_job(&self, chunk_x: i32, chunk_z: i32) -> Option<light::LightJob> {
        self.chunks.get(&(chunk_x, chunk_z))?;

        Some(light::LightJob {
            chunks: std::array::from_fn(|slot| {
                let place = (chunk_x + slot as i32 % 3 - 1, chunk_z + slot as i32 / 3 - 1);
                let chunk = self.chunks.get(&place)?;

                Some(light::LightInput {
                    place,
                    revision: chunk.revision,
                    sections: chunk.sections.clone(),
                    light: chunk.fresh_light().cloned(),
                })
            }),
        })
    }

    /// Кладёт в чанки пересчитанный собственный свет — если их блоки с тех
    /// пор не менялись.
    pub fn store_light(&mut self, recounted: Vec<light::Recounted>) {
        for fresh in recounted {
            if let Some(chunk) = self.chunks.get_mut(&fresh.place)
                && chunk.revision == fresh.revision
            {
                chunk.light = Some((fresh.revision, fresh.light));
            }
        }
    }

    /// Небесный и блочный свет в блоке — с учётом соседних чанков. Для
    /// проверок: считает свет всего чанка.
    #[cfg(test)]
    pub fn light_at(&mut self, x: i32, y: i32, z: i32) -> (u8, u8) {
        let Some(job) = self.light_job(x.div_euclid(16), z.div_euclid(16)) else {
            return (light::FULL, 0);
        };
        let (light, recounted) = job.finish();

        self.store_light(recounted);
        light.at(x.rem_euclid(16), y, z.rem_euclid(16))
    }

    /// Небесный свет в блоке — тот же, что уходит клиенту: свет чанка,
    /// сшитый с восемью соседями. Нужен датчику дневного света.
    ///
    /// Под открытым небом (выше только воздух) это всегда 15 — считать
    /// ничего не надо. Иначе свет чанка считается и запоминается до первой
    /// правки блоков в нём или у соседей. Незагруженный чанк — 15: спросить
    /// не у чего, а датчик в нём и не тикает.
    pub fn sky_light(&mut self, x: i32, y: i32, z: i32) -> u8 {
        if y >= MIN_Y + WORLD_HEIGHT {
            return light::FULL;
        }

        if y < MIN_Y {
            return 0;
        }

        let (chunk_x, chunk_z) = chunk_key(x, z);
        let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return light::FULL;
        };

        if (y + 1..=chunk.highest()).all(|above| chunk.block(x, above, z) == AIR) {
            return light::FULL;
        }

        let revisions: [u32; 9] = std::array::from_fn(|slot| {
            let place = (chunk_x + slot as i32 % 3 - 1, chunk_z + slot as i32 / 3 - 1);

            self.chunks.get(&place).map_or(u32::MAX, |chunk| chunk.revision)
        });

        let (local_x, local_z) = (x.rem_euclid(CHUNK_SIZE), z.rem_euclid(CHUNK_SIZE));

        if let Some((seen, light)) = self.sky_cache.get(&(chunk_x, chunk_z))
            && *seen == revisions
        {
            return light.at(local_x, y, local_z).0;
        }

        let Some(job) = self.light_job(chunk_x, chunk_z) else {
            return light::FULL;
        };
        let (light, recounted) = job.finish();

        self.store_light(recounted);

        let level = light.at(local_x, y, local_z).0;

        self.sky_cache.insert((chunk_x, chunk_z), (revisions, Arc::new(light)));
        level
    }

    /// Откуда брать ещё не готовый чанк. Его можно унести с собой: чтение
    /// с диска и складывание нового чанка — работа долгая, и держать на ней
    /// замок мира нельзя, иначе встанет такт.
    pub fn source(&self) -> ChunkSource {
        ChunkSource {
            path: self.path.clone(),
            generator: Arc::clone(&self.generator),
            epoch: self.unload_epoch,
        }
    }

    /// Есть ли этот чанк в памяти.
    /// Номер правки чанка: растёт с каждым изменённым блоком. None — чанка
    /// нет в памяти.
    pub fn chunk_revision(&self, chunk_x: i32, chunk_z: i32) -> Option<u32> {
        self.chunks.get(&(chunk_x, chunk_z)).map(|chunk| chunk.revision)
    }

    pub fn has_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.chunks.contains_key(&(chunk_x, chunk_z))
    }

    /// Кладёт готовый чанк в мир. Если его успели положить, пока он
    /// складывался, — оставляем тот, что уже есть.
    ///
    /// Готовый чанк мог и устареть: пока он читался в стороне, это место
    /// успели загрузить, изменить, записать и снова выгрузить — тогда на
    /// диске уже новее. Такой чанк не кладётся, а читается заново, здесь же.
    /// Так же — если чанк в стороне прочитать не удалось.
    pub fn accept(&mut self, chunk_x: i32, chunk_z: i32, ready: ReadyChunk) {
        let key = (chunk_x, chunk_z);

        if self.chunks.contains_key(&key) {
            return;
        }

        let stale = ready.epoch + UNLOAD_MEMORY < self.unload_epoch
            || self.unloaded.get(&key).is_some_and(|at| *at > ready.epoch);

        match ready.taken {
            Taken::Loaded { chunk, from_disk, wake } if !stale => {
                self.insert_chunk(chunk_x, chunk_z, chunk, from_disk, &wake);
            }
            Taken::Loaded { .. } => {
                log_debug!("Мир: чанк {} {} выгружался, пока читался, — читаю заново", chunk_x, chunk_z);
                self.ensure(chunk_x, chunk_z);
            }
            Taken::Unavailable => {
                self.ensure(chunk_x, chunk_z);
            }
        }
    }

    /// Кладёт чанк в мир и делает всё, что следует за его появлением.
    fn insert_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        chunk: Chunk,
        from_disk: bool,
        wake: &[((i32, i32, i32), u64)],
    ) {
        let key = (chunk_x, chunk_z);
        let mut chunk = chunk;

        // Спавнеры, что генерация поставила блоком без данных (крепость Незера
        // и прочие плоские холсты): вид по измерению.
        if !from_disk {
            chunk.fill_missing_spawners(self.is_nether());
        }

        let mut places: Vec<_> = chunk
            .containers
            .keys()
            .map(|&(x, y, z)| (chunk_x * CHUNK_SIZE + x, y, chunk_z * CHUNK_SIZE + z))
            .collect();

        places.sort_unstable();
        self.container_order.extend(places);
        // В чанке рамки записаны с местом внутри него; миру нужно мировое.
        self.loaded_frames.extend(chunk.frames.iter().cloned().map(|mut frame| {
            frame.pos.0 += chunk_x * CHUNK_SIZE;
            frame.pos.2 += chunk_z * CHUNK_SIZE;
            frame
        }));
        self.loaded_mobs.extend(chunk.mobs.iter().cloned());
        self.chunks.insert(key, chunk);
        self.unloaded.remove(&key);

        // Сложенный заново чанк записывается, даже если его не тронут.
        if !from_disk {
            self.fresh.insert(key);
            self.new_chunks.push(key);
        }

        // Такты, дожидавшиеся этого чанка, возвращаются в очередь, а
        // прочитанное с диска чинится: сервер могли остановить посреди хода
        // поршня. Жидкостям, которым есть куда течь, — свой такт.
        self.after_chunk_arrived(chunk_x, chunk_z, from_disk);
        let mut fires = Vec::new();
        for section in 0..SECTIONS as usize {
            if let Some(states) = self.section_blocks(chunk_x, chunk_z, section) {
                for (index, &state) in states.iter().enumerate() {
                    if matches!(crate::blocks::block_at_state(state as i32), Some("fire" | "soul_fire")) {
                        fires.push((chunk_x * 16 + (index as i32 & 15), MIN_Y + section as i32 * 16 + (index as i32 >> 8), chunk_z * 16 + ((index as i32 >> 4) & 15)));
                    }
                }
            }
        }
        for (x, y, z) in fires { let delay = 30 + (self.random_bits() % 10) as u64; self.schedule_block_once(x, y, z, delay); }

        self.wake_fluids(wake);
        self.wake_border(chunk_x, chunk_z);
    }

    /// Вода и лава на краю чанка, которым есть куда течь в соседний чанк
    /// (или из соседа — в этот): видно, только когда оба лежат в мире.
    fn wake_border(&mut self, chunk_x: i32, chunk_z: i32) {
        let mut places = Vec::new();

        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (Some(here), Some(there)) =
                (self.chunks.get(&(chunk_x, chunk_z)), self.chunks.get(&(chunk_x + dx, chunk_z + dz)))
            else {
                continue;
            };

            for step in 0..16 {
                // Пара столбцов по обе стороны края.
                let (hx, hz, tx, tz) = match (dx, dz) {
                    (1, _) => (15, step, 0, step),
                    (-1, _) => (0, step, 15, step),
                    (_, 1) => (step, 15, step, 0),
                    _ => (step, 0, step, 15),
                };

                for section in 0..SECTIONS as usize {
                    if here.sections[section].is_none() && there.sections[section].is_none() {
                        continue;
                    }

                    for y in MIN_Y + section as i32 * 16..MIN_Y + section as i32 * 16 + 16 {
                        let (a, b) = (here.block(hx, y, hz), there.block(tx, y, tz));
                        let flows = |from: i32, to: i32| {
                            fluids::fluid_at(from).is_some_and(|(kind, _)| match fluids::fluid_at(to) {
                                Some((other, _)) => other != kind,
                                None => fluids::can_flow_into(to),
                            })
                        };

                        // Будится та жидкость, которой есть куда течь.
                        if flows(a, b) {
                            places.push(((
                                chunk_x * 16 + hx, y, chunk_z * 16 + hz,
                            ), fluids::fluid_at(a).map_or(fluids::WATER_DELAY, |(kind, _)| kind.delay())));
                        }

                        if flows(b, a) {
                            places.push(((
                                (chunk_x + dx) * 16 + tx, y, (chunk_z + dz) * 16 + tz,
                            ), fluids::fluid_at(b).map_or(fluids::WATER_DELAY, |(kind, _)| kind.delay())));
                        }
                    }
                }
            }
        }

        self.wake_fluids(&places);
    }

    /// Ставит в очередь на растекание места рядом с водой и лавой.
    fn wake_fluids(&mut self, places: &[((i32, i32, i32), u64)]) {
        for &((x, y, z), delay) in places {
            self.schedule(x, y, z, delay);
        }
    }

    /// Что делается, когда чанк появился в мире: разбудить отложенные такты
    /// и починить прочитанное с диска.
    fn after_chunk_arrived(&mut self, chunk_x: i32, chunk_z: i32, from_disk: bool) {
        let woken: Vec<Scheduled> = self
            .parked
            .extract_if(.., |entry| chunk_key(entry.place.0, entry.place.2) == (chunk_x, chunk_z))
            .collect();

        for entry in woken {
            self.waiting.remove(&(entry.place, entry.kind));
            self.push_scheduled(entry.place, entry.kind, self.tick + 1, entry.priority);
        }

        if from_disk {
            redstone::repair_chunk(self, chunk_x, chunk_z);
        }
    }

    /// Готовит чанк, если его ещё нет: складывает землю ровного мира.
    ///
    /// Чанки появляются по мере надобности — когда игрок до них дошёл, а не
    /// все сразу: мир бесконечен, и заранее его не наготовишь.
    ///
    /// Возвращает false, если чанк уже был — или если его не удалось ни
    /// прочитать, ни сложить: диск не ответил, чанк за пределами мира.
    /// Тогда в памяти его нет, и запись в это место отбрасывается.
    pub fn ensure(&mut self, chunk_x: i32, chunk_z: i32) -> bool {
        if self.chunks.contains_key(&(chunk_x, chunk_z)) {
            return false;
        }

        match take_chunk(&self.path, &self.generator, chunk_x, chunk_z) {
            Taken::Loaded { chunk, from_disk, wake } => {
                self.insert_chunk(chunk_x, chunk_z, chunk, from_disk, &wake);
                true
            }
            Taken::Unavailable => false,
        }
    }

    /// Выгружает из памяти чанки, до которых никому нет дела.
    ///
    /// `keep` отвечает, нужен ли чанк. Всё изменённое перед этим
    /// записывается, поэтому выгрузка ничего не теряет: понадобится — чанк
    /// прочитается с диска заново.
    ///
    /// Возвращает, сколько чанков выгружено.
    pub fn unload_far<F>(&mut self, keep: F) -> usize
    where
        F: Fn(i32, i32) -> bool,
    {
        self.save_if_needed();

        let before = self.chunks.len();

        // Ещё не отданное на запись не выгружается: до сложенного заново
        // чанка не дошла очередь. Отданное выгружать можно — пока оно не на
        // диске, чтение берёт его из очереди записи (`writer`).
        let (dirty, fresh) = (&self.dirty, &self.fresh);
        let mut gone = Vec::new();

        self.chunks.retain(|key, _| {
            let stays = keep(key.0, key.1) || dirty.contains(key) || fresh.contains(key);

            if !stays {
                gone.push(*key);
            }

            stays
        });

        // Отметка о выгрузке: готовый чанк, взятый раньше неё, устарел.
        if !gone.is_empty() {
            self.unload_epoch += 1;

            let epoch = self.unload_epoch;

            self.unloaded.extend(gone.into_iter().map(|key| (key, epoch)));
            self.unloaded.retain(|_, at| *at + UNLOAD_MEMORY >= epoch);
        }

        // Ждать пересчёта в выгруженных чанках больше некому.
        self.redstone.forget_outside(|x, z| self.chunks.contains_key(&chunk_key(x, z)));
        self.sky_cache.retain(|key, _| self.chunks.contains_key(key));

        before - self.chunks.len()
    }

    /// События мира, начиная с записи `from` — то, что осталось разослать.
    ///
    /// Возвращает копию, а не срез: рассылка — дело сетевое, и держать мир
    /// захваченным, пока идёт отправка, нельзя.
    pub fn events_since(&mut self, from: usize, reader: journal::Reader) -> (Vec<WorldEvent>, usize) {
        (self.events.since(from, reader), self.events.count())
    }

    /// То же, что `events_since`, и заодно — отстал ли читатель: часть
    /// событий до него уже не дойдёт (журнал выбросил их, не дождавшись).
    /// Отставшему клиенту надо заново прислать чанки вокруг него целиком.
    pub fn events_since_checked(&mut self, from: usize, reader: journal::Reader) -> (Vec<WorldEvent>, usize, bool) {
        let (events, lagged) = self.events.since_checked(from, reader);

        (events, self.events.count(), lagged)
    }

    /// Только изменения блоков из журнала — для проверок.
    #[cfg(test)]
    pub fn changes_since(&mut self, from: usize, reader: journal::Reader) -> (Vec<BlockChange>, usize) {
        let (events, count) = self.events_since(from, reader);
        let changes = events
            .into_iter()
            .filter_map(|event| match event {
                WorldEvent::Change(change) => Some(change),
                _ => None,
            })
            .collect();

        (changes, count)
    }

    /// Только действия блоков из журнала — для проверок.
    #[cfg(test)]
    pub fn actions_since(&mut self, from: usize, reader: journal::Reader) -> (Vec<BlockAction>, usize) {
        let (events, count) = self.events_since(from, reader);
        let actions = events
            .into_iter()
            .filter_map(|event| match event {
                WorldEvent::Action(action) => Some(action),
                _ => None,
            })
            .collect();

        (actions, count)
    }

    /// Записывает читателя событий: с этого места ему будет досылаться всё
    /// новое, а прочитанное всеми — выбрасываться.
    pub fn watch_events(&mut self, reader: journal::Reader) -> usize {
        self.events.watch(reader)
    }

    /// То же под старым именем — для проверок.
    #[cfg(test)]
    pub fn watch_changes(&mut self, reader: journal::Reader) -> usize {
        self.watch_events(reader)
    }

    /// То же под старым именем — для проверок.
    #[cfg(test)]
    pub fn watch_actions(&mut self, reader: journal::Reader) -> usize {
        self.watch_events(reader)
    }

    /// Сообщает клиентам Bedrock, какие блоки едут по ходу поршня.
    pub fn note_piston_move(&mut self, piston: PistonMove) {
        self.events.push(WorldEvent::Piston(std::sync::Arc::new(piston)));
    }

    /// Сообщает всем клиентам о действии блока.
    pub fn note_action(&mut self, action: BlockAction) {
        self.events.push(WorldEvent::Action(action));
    }

    /// Сообщает всем клиентам о звуке в этом месте.
    pub fn note_sound(&mut self, x: i32, y: i32, z: i32, name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound { x, y, z, name, volume, pitch, exact: None, of_block: true, of_player: false, except: None }));
    }

    /// Звук блока всем, кроме игрока `except` (кисть: сам чистящий его не
    /// слышит — чёрный ящик, brush-sand).
    pub fn note_sound_except(&mut self, (x, y, z): (i32, i32, i32), name: &'static str, volume: f32, pitch: f32, except: i32) {
        self.events.push(WorldEvent::Sound(WorldSound { x, y, z, name, volume, pitch, exact: None, of_block: true, of_player: false, except: Some(except) }));
    }

    /// Звук блока, вызванный рукой игрока (`actor`): всем, кроме него.
    pub fn note_sound_by_hand(&mut self, x: i32, y: i32, z: i32, name: &'static str, volume: f32, pitch: f32) {
        let except = self.actor;

        self.events.push(WorldEvent::Sound(WorldSound { x, y, z, name, volume, pitch, exact: None, of_block: true, of_player: false, except }));
    }

    /// Звук сущности в точном месте — плеск предмета, вошедшего в воду.
    pub fn note_sound_at(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: false,
            of_player: false,
            except: None,
        }));
    }

    /// Звук блока в точном месте (раздел «blocks»): улей жужжит из точки
    /// (x+0,5; y; z+0,5), а не из середины блока.
    pub fn note_sound_block_at(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: true,
            of_player: false,
            except: None,
        }));
    }

    /// Звук игрока в его точке — удар (раздел «игроки»).
    pub fn note_player_sound(&mut self, at: (f64, f64, f64), name: &'static str) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume: 1.0,
            pitch: 1.0,
            exact: Some(at),
            of_block: false,
            of_player: true,
            except: None,
        }));
    }

    /// Звук блока в точке не посередине блока: открытие большого сундука
    /// слышно между половинами, бочки — у крышки.
    pub fn note_sound_at_block(&mut self, at: (f64, f64, f64), name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound {
            x: at.0.floor() as i32,
            y: at.1.floor() as i32,
            z: at.2.floor() as i32,
            name,
            volume,
            pitch,
            exact: Some(at),
            of_block: true,
            of_player: false,
            except: None,
        }));
    }

    /// Данные блок-сущности клиентам — после ближайшей пачки изменений.
    pub fn note_private(&mut self, viewer: i32, body: Vec<u8>) {
        self.events.push(WorldEvent::Private { viewer, body: body.into() });
    }

    pub fn note_block_entity(&mut self, x: i32, y: i32, z: i32, kind: i32, nbt: Vec<u8>) {
        self.events.push(WorldEvent::BlockEntity { x, y, z, kind, nbt: nbt.into() });
    }

    /// Рассылает то, что стоит в месте, как обычное изменение (пачкой) и
    /// сообщает редстоуну: блок уже поставлен молча, прежний был `old`.
    pub fn announce_change(&mut self, x: i32, y: i32, z: i32, old: i32) {
        let state = self.get_block(x, y, z);

        self.events.push(WorldEvent::Change(BlockChange { x, y, z, state, direct: false }));
        self.changed.push(((x, y, z), old));
        self.dirty.insert(chunk_key(x, z));
    }

    /// Такт блока в этом месте через `delay`, если он ещё не ждёт своего.
    pub fn schedule_block_once(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        self.schedule_once(x, y, z, delay, 0);
    }

    /// Случайное число от 0 до 1 — для высоты звуков и прочего, где в игре
    /// тоже случайность. Простой сдвиговый генератор: точность ему не нужна.
    pub fn random_unit(&mut self) -> f32 {
        let mut x = self.noise.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        (x % 10_000) as f32 / 10_000.0
    }

    /// Случайное число во всю ширину генератора — там, где нужна точная
    /// доля вроде 3/4096 (случайный такт), а `random_unit` для неё грубовата.
    pub fn random_bits(&mut self) -> u32 {
        let mut x = self.noise.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        x
    }

    /// Сообщает клиентам, что стоит в этом месте сейчас, — даже если для
    /// мира это не изменение, — отдельным сообщением, не в пачке: так
    /// оригинал повторяет клиенту блок, по которому тот щёлкнул, и блок
    /// за гранью щелчка.
    pub fn tell_directly(&mut self, x: i32, y: i32, z: i32) {
        self.tell_state(x, y, z, true, false);
    }

    /// Сообщает клиентам, что стоит в этом месте, через столько-то тактов.
    ///
    /// Так уходят блоки, поставленные ходом поршня: у оригинала изменения
    /// этой фазы такта рассылаются только в следующем такте (сверено
    /// чёрным ящиком). Что там стоит, читается в момент отправки.
    pub fn tell_later(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        self.late.push(((x, y, z), self.tick + delay.max(1), false));
    }

    /// То же, но служебный «движущийся поршень» уходит как есть: так
    /// оригинал сообщает о месте сломанного поршнем блока.
    pub fn tell_later_as_is(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        self.late.push(((x, y, z), self.tick + delay.max(1), true));
    }

    fn tell_state(&mut self, x: i32, y: i32, z: i32, direct: bool, as_is: bool) {
        let mut state = self.get_block(x, y, z);

        // Служебный «движущийся поршень» клиенту обычно не показывают:
        // для него это место пустое, и так же говорит игра.
        if !as_is
            && let Some((low, high)) = crate::blocks::states_of("moving_piston")
            && (low..=high).contains(&state)
        {
            state = AIR;
        }

        self.events.push(WorldEvent::Change(BlockChange { x, y, z, state, direct }));
    }

    /// Отписывает читателя: подключение закрылось.
    pub fn forget_reader(&mut self, reader: journal::Reader) {
        self.events.forget(reader);
    }

    /// Просит пересчитать это место через столько-то тактов — для жидкостей.
    ///
    /// Если место уже ждёт, берётся более ранний срок: раз его потревожили
    /// снова, тянуть незачем.
    pub fn schedule(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        let due = self.tick + delay.max(1);

        // Место уже ждёт такта жидкости не позже этого срока — новой просьбы
        // не нужно; ждёт позже — просьба ставится на более ранний срок
        // (сверено сценариями waterfall и stream-redirect: с правилом «вторую
        // не ставить» вода у нас растекалась иначе). Такт этого же такта, ещё
        // не исполненный, не мешает: вода может завести новый, пока
        // исполняется прежний (вики, Tick, 18w10c).
        // Прежний такт был заведён для другой жидкости (раздатчик забрал
        // источник ведром): он пропадёт вместе с ней, и новый не ждёт его.
        let fluid = fluid_type(self.get_block(x, y, z));

        if self.waits_ahead((x, y, z), TickKind::Fluid)
            && self
                .waiting
                .get(&((x, y, z), TickKind::Fluid))
                .is_some_and(|waiting| waiting.due <= due && waiting.fluid == fluid)
        {
            return;
        }

        self.push_scheduled((x, y, z), TickKind::Fluid, due, FLUID_PRIORITY);
    }

    /// Просит пересчитать место через столько-то тактов с таким приоритетом —
    /// для редстоуна.
    ///
    /// Место может ждать только одного такта: пока он не прошёл, новые
    /// просьбы отбрасываются. Возвращает false, если просьба отброшена.
    /// Так устроено в игре, и от этого зависит поведение повторителей.
    pub fn schedule_once(&mut self, x: i32, y: i32, z: i32, delay: u64, priority: i8) -> bool {
        if self.waiting.contains_key(&((x, y, z), TickKind::Block)) {
            return false;
        }

        self.push_scheduled((x, y, z), TickKind::Block, self.tick + delay.max(1), priority);
        true
    }

    /// Просит пересчитать место, даже если оно уже чего-то ждёт: прежняя
    /// заявка заменяется новой.
    ///
    /// Правило «одна заявка за раз» вики формулирует только для повторителя;
    /// кнопке оно вредит — её отпускание просто пропало бы, и она залипла бы
    /// навсегда.
    pub fn schedule_block(&mut self, x: i32, y: i32, z: i32, delay: u64, priority: i8) {
        self.push_scheduled((x, y, z), TickKind::Block, self.tick + delay.max(1), priority);
    }

    /// Кладёт запись в очередь, помечая прежнюю запись этого места устаревшей.
    fn push_scheduled(&mut self, place: (i32, i32, i32), kind: TickKind, due: u64, priority: i8) {
        let order = self.next_order;
        self.next_order += 1;

        let fluid = if kind == TickKind::Fluid { fluid_type(self.get_block(place.0, place.1, place.2)) } else { 0 };

        self.waiting.insert((place, kind), WaitingTick { due, order, priority, fluid });
        self.scheduled.push(Reverse(Scheduled {
            due,
            priority,
            order,
            place,
            kind,
        }));
    }

    /// Будит жидкость в самом месте и у его соседей — для жидкостей.
    ///
    /// Как у оригинала, такт заводится только жидкости: изменение рядом
    /// будит её, и своим тактом она пересчитывает себя и растекается
    /// (`tick::flow`). Пустому месту такт не нужен — его зальёт сосед.
    /// Срок у каждой жидкости свой: лава течёт заметно медленнее воды.
    fn schedule_around(&mut self, x: i32, y: i32, z: i32) {
        for (dx, dy, dz) in AROUND {
            let (x, y, z) = (x + dx, y + dy, z + dz);
            let state = self.get_block(x, y, z);

            if !wet(state) {
                continue;
            }

            let delay = match fluids::fluid_at(state) {
                Some((kind, _)) => kind.delay_in(self.is_nether()),
                None => fluids::WATER_DELAY,
            };

            self.schedule(x, y, z, delay);
        }
    }

    /// Исполнять ли такт жидкости в этом месте: он заведён для той жидкости,
    /// что здесь сейчас. Такт без записи (заведённый в обход очереди)
    /// исполняется.
    pub fn fluid_tick_holds(&self, place: (i32, i32, i32)) -> bool {
        self.waiting
            .get(&(place, TickKind::Fluid))
            .is_none_or(|waiting| waiting.fluid == fluid_type(self.get_block(place.0, place.1, place.2)))
    }

    /// Двигает время мира на такт вперёд и отдаёт места, которым пора, —
    /// по приоритету, а среди равных по порядку записи. У каждого места
    /// сказано, блочный это такт или жидкостный.
    ///
    /// За один такт мир разбирает не больше `limit` мест: столько же, сколько
    /// в игре. Остальные не пропадают, а остаются ждать следующего такта —
    /// иначе схема, наплодившая много работы, теряла бы часть навсегда.
    pub fn advance(&mut self, limit: usize) -> Vec<((i32, i32, i32), TickKind)> {
        self.tick += 1;

        // Места, о которых пришла пора сказать: берём то, что стоит там
        // сейчас, а не то, что стояло, когда решили сказать.
        let tick = self.tick;
        let late: Vec<((i32, i32, i32), bool)> = self
            .late
            .extract_if(.., |(_, when, _)| *when <= tick)
            .map(|(place, _, as_is)| (place, as_is))
            .collect();

        for ((x, y, z), as_is) in late {
            self.tell_state(x, y, z, false, as_is);
        }

        let mut due = Vec::new();

        while let Some(Reverse(next)) = self.scheduled.peek() {
            if next.due > self.tick || due.len() >= limit {
                break;
            }

            let next = *next;
            self.scheduled.pop();

            // Устаревшая запись: место перезаписали на другой срок.
            if self.waiting.get(&(next.place, next.kind)).map(|waiting| (waiting.due, waiting.order))
                != Some((next.due, next.order))
            {
                continue;
            }

            // Чанк выгружен — такт подождёт, пока его прочитают снова.
            // В `waiting` он остаётся: и чтобы не завёлся второй, и чтобы
            // попасть на диск.
            if !self.chunks.contains_key(&chunk_key(next.place.0, next.place.2)) {
                self.parked.push(next);
                continue;
            }

            // С учёта такт снимается, когда до него дойдёт очередь
            // (`begin_tick`), а не сейчас: пока он не исполнен, он ещё
            // ожидает, и повторитель или факел второго такта не заведут
            // (MC-189954: так ведут себя все, кроме наблюдателя).
            due.push((next.place, next.kind));
        }

        due
    }

    /// Такт в этом месте начинает исполняться: снимается с учёта.
    pub fn begin_tick(&mut self, place: (i32, i32, i32), kind: TickKind) {
        if self.waiting.get(&(place, kind)).is_some_and(|waiting| waiting.due <= self.tick) {
            self.waiting.remove(&(place, kind));
        }
    }

    /// Ждёт ли место такта, который ещё не пришёл (срок впереди).
    fn waits_ahead(&self, place: (i32, i32, i32), kind: TickKind) -> bool {
        self.waiting.get(&(place, kind)).is_some_and(|waiting| waiting.due > self.tick)
    }

    /// Заводит блочное событие: место, вид события и его параметр (у
    /// поршня вид — «выдвинуться» или «задвинуться», у сундука — крышка и
    /// число смотрящих).
    ///
    /// Как у оригинала (вики, Piston; techmcdocs, GameTick): события
    /// исполняются в своей фазе такта по порядку появления, повтор —
    /// совпадающий целиком: место, вид и параметр — не заводится, а
    /// заведённое во время фазы исполняется в ней же.
    pub fn queue_block_event(&mut self, x: i32, y: i32, z: i32, kind: u8, param: u8) {
        let event = ((x, y, z), kind, param);

        if !self.block_events.contains(&event) {
            self.block_events.push_back(event);
        }
    }

    /// Следующее блочное событие по очереди.
    pub fn take_block_event(&mut self) -> Option<((i32, i32, i32), u8, u8)> {
        self.block_events.pop_front()
    }

    /// Записывает возраст мира, часы и то, что ждёт своего такта.
    ///
    /// Без запланированных тактов после перезапуска встал бы весь редстоун,
    /// который держится на времени: часы на факелах, повторители в середине
    /// задержки, начатый ход поршня.
    fn save_level(&self) {
        let mut text = format!(
            "age = {}\ntime-offset = {}\nseed = {}\ntype = {}\n",
            self.tick,
            self.time_offset,
            self.seed,
            self.kind.name()
        );

        if let Some((x, y, z)) = *self.spawn.lock().expect("точка появления захвачена другим потоком") {
            text.push_str(&format!("spawn = {} {} {}\n", x, y, z));
        }

        if self.fight.started {
            text.push_str(&self.fight.save());
        }

        if self.weather != crate::weather::Weather::default() {
            text.push_str(&self.weather.save());
        }

        // Такты и ходы поршней — в своём файле: их бывают десятки тысяч
        // (вода в выгруженных чанках ждёт, пока чанк снова загрузят), и
        // переписывать их вместе с возрастом мира раз в секунду незачем.
        self.save_ticks();

        // Без возраста содержимое меняется редко (время перевели, нашлась
        // точка появления): тогда — сразу, иначе раз в полминуты. Возраст
        // мира, потерянный при обрыве, — не больше полуминуты.
        let print = {
            use std::hash::{Hash, Hasher};

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            // Таймеры погоды тоже идут каждый такт: в отпечаток — только
            // её флаги («weather = 1 0 …» → «weather = 1 0»).
            text.lines()
                .skip(1)
                .map(|line| match line.strip_prefix("weather = ") {
                    Some(rest) => rest.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
                    None => line.to_string(),
                })
                .collect::<Vec<_>>()
                .hash(&mut hasher);
            hasher.finish()
        };

        if let Some((at, seen)) = self.level_written.get()
            && seen == print
            && at.elapsed() < LEVEL_PAUSE
        {
            return;
        }

        let path = self.path.clone();

        writer::write_file(LEVEL, move || write_level(&path, &text));
        self.level_written.set(Some((Instant::now(), print)));
    }

    /// Записывает `ticks.dat`: что ждёт своего такта и начатые ходы поршней.
    /// Срок пишется по возрасту мира, а не «сколько ждать», — тогда
    /// неизменная очередь даёт тот же текст, и файл не переписывается.
    fn save_ticks(&self) {
        let mut text = String::new();

        // Порядок постоянный, иначе один и тот же набор давал бы разный
        // текст и файл переписывался бы зря.
        let mut waiting: Vec<_> = self.waiting.iter().collect();
        waiting.sort_by_key(|((place, kind), _)| (*place, matches!(kind, TickKind::Fluid)));

        for ((place, kind), waiting) in waiting {
            let kind = match kind {
                TickKind::Block => "block",
                TickKind::Fluid => "fluid",
            };

            // Приоритет тоже записываем: от него зависит, кто сработает
            // раньше при одинаковом сроке, а на этом держатся схемы.
            text.push_str(&format!(
                "due = {} {} {} {} {} {}\n",
                place.0, place.1, place.2, kind, waiting.due, waiting.priority
            ));
        }

        // Начатые ходы поршней: без них едущий блок пропал бы совсем —
        // с мест он уже снят, а куда ехал, знает только память.
        for (pos, moving) in self.redstone.moving_pistons() {
            text.push_str(&format!(
                "moving = {} {} {} {} {}\n",
                pos.0,
                pos.1,
                pos.2,
                u8::from(moving.extending),
                moving.facing.number()
            ));

            for (place, state) in &moving.cargo {
                text.push_str(&format!(
                    "cargo = {} {} {} {} {} {} {}\n",
                    pos.0, pos.1, pos.2, place.0, place.1, place.2, state
                ));
            }
        }

        let print = {
            use std::hash::{Hash, Hasher};

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            hasher.finish()
        };

        if print == self.ticks_written.get() {
            return;
        }

        let path = self.path.clone();

        writer::write_file(TICKS, move || write_atomically(&path, TICKS, &text));
        self.ticks_written.set(print);
    }

    /// Сидит ли кошка на блоке: ящик кошки (0,6 × 0,7) задевает ящик над
    /// блоком — как у оригинала, где сундук ищет кошек в ящике над собой.
    pub fn cat_sits_above(&self, x: i32, y: i32, z: i32) -> bool {
        let (fx, fy, fz) = (f64::from(x), f64::from(y + 1), f64::from(z));

        self.sitting_cats.iter().any(|&(cx, cy, cz)| cx + 0.3 > fx && cx - 0.3 < fx + 1.0 && cy + 0.7 > fy && cy < fy + 1.0 && cz + 0.3 > fz && cz - 0.3 < fz + 1.0)
    }

    /// Который сейчас час в мире: такты суток, от 0 до 23 999.
    ///
    /// Сутки — 24 000 тактов. Отдельно от возраста мира: время можно
    /// перевести командой, а возраст идёт своим ходом.
    pub fn time_of_day(&self) -> i64 {
        (self.tick as i64 + self.time_offset).rem_euclid(DAY_LENGTH)
    }

    /// Часы мира от начала: день × 24000 + час (жители считают пополнения по дням).
    pub fn clock(&self) -> i64 {
        self.tick as i64 + self.time_offset
    }

    /// Переводит часы мира.
    pub fn set_time_of_day(&mut self, time: i64) {
        self.time_offset = time.rem_euclid(DAY_LENGTH) - self.tick as i64;
        self.clock_changes += 1;
        self.clock_set_to = time.rem_euclid(DAY_LENGTH);
    }

    /// Сколько раз переводили часы и на который час — последний раз (см.
    /// `clock_changes`).
    pub fn clock_changes(&self) -> (u64, i64) {
        (self.clock_changes, self.clock_set_to)
    }

    /// Который сейчас такт мира.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Семя мира.
    pub fn seed(&self) -> i64 {
        self.seed
    }

    /// Внутри ли клетка куска крепости Незера (для появления мобов крепости).
    pub fn in_fortress(&self, x: i32, y: i32, z: i32) -> bool {
        match &*self.generator {
            Generator::Nether(nether) => nether.in_fortress(x, y, z),
            _ => false,
        }
    }

    /// Вики, Advancement: «Enter a ...» — внутри рамки строения, а не рядом
    /// с похожими блоками. Рамки нашей генерации; [проверить] границы оригинала.
    pub fn progress_structure(&self, x: i32, y: i32, z: i32) -> Option<&'static str> {
        match &*self.generator {
            Generator::Nether(nether) => nether.progress_structure(x, y, z),
            Generator::Normal(terrain) if stronghold::contains(terrain, x, y, z) => Some("story/follow_ender_eye"),
            Generator::End(end) if end.in_city(x, y, z) => Some("end/find_end_city"),
            _ => None,
        }
    }

    /// Биом поверхности столбца по генератору (команда `locate biome`):
    /// имя в реестре без «minecraft:». Ровный мир — равнины.
    pub fn surface_biome(&self, x: i32, z: i32) -> &'static str {
        match &*self.generator {
            Generator::Normal(terrain) => terrain.column_in(x, z, None).biome.name(),
            Generator::Nether(nether) => nether.biome_at(x, z).name(),
            Generator::End(end) => end.biome_at(x, z).name(),
            Generator::Flat => "plains",
        }
    }

    /// Биом загруженной клетки, включая пещерные биомы и fillbiome.
    /// Слизни не появляются в deep_dark (вики, Slime).
    pub fn mob_biome(&self, x: i32, y: i32, z: i32) -> &'static str {
        if (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y)
            && let Some(chunk) = self.chunks.get(&(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE)))
            && let Some(biome) = terrain::Biome::ALL.get(usize::from(chunk.biome(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))))
        {
            return biome.name();
        }
        self.surface_biome(x, z)
    }

    /// Ближайшая к чанку постройка этого вида, какую кладёт наша генерация
    /// (`locate structure`): чанк старта. Ищется по регионам до `regions`
    /// регионов вокруг. В обычном мире — крепости (stronghold.rs).
    pub fn nearest_structure(&self, name: &str, chunk_x: i32, chunk_z: i32, regions: i32) -> Option<(i32, i32)> {
        type Start<'a> = Box<dyn Fn(i32, i32) -> Option<(i32, i32)> + 'a>;

        // Крепости обычного мира — 128 стартов на кольцах, не по регионам.
        if let (Generator::Normal(terrain), "stronghold") = (&*self.generator, name) {
            return stronghold::nearest(terrain, chunk_x, chunk_z);
        }

        // Подводные крепости — по областям 32×32 чанка (monument.rs).
        if let (Generator::Normal(terrain), "monument") = (&*self.generator, name) {
            return monument::nearest(terrain, chunk_x, chunk_z, regions.max(1) * 27 / monument::SPACING + 1);
        }

        // Древние города — по областям 24×24 чанка (ancient_city.rs).
        if let (Generator::Normal(terrain), "ancient_city") = (&*self.generator, name) {
            return ancient_city::nearest(terrain, chunk_x, chunk_z, regions.max(1) * 27 / ancient_city::SPACING + 1);
        }

        // Аванпосты разбойников — по областям 32×32 чанка (pillager_outpost.rs).
        if let (Generator::Normal(terrain), "pillager_outpost") = (&*self.generator, name) {
            return pillager_outpost::nearest(terrain, chunk_x, chunk_z, regions.max(1) * 27 / pillager_outpost::SPACING + 1);
        }

        // Испытательные камеры — по областям 34×34 чанка (trial_chambers.rs).
        if let (Generator::Normal(terrain), "trial_chambers") = (&*self.generator, name) {
            return trial_chambers::nearest(terrain, chunk_x, chunk_z, regions.max(1) * 27 / trial_chambers::SPACING + 1);
        }

        // Заброшенные шахты — 0,4 % чанков (mineshaft.rs).
        if let (Generator::Normal(terrain), "mineshaft" | "mineshaft_mesa") = (&*self.generator, name) {
            return mineshaft::nearest(terrain, name == "mineshaft_mesa", chunk_x, chunk_z, (regions.max(1) * 27).min(200));
        }

        // Иглу — по областям 32×32 чанка (temples.rs).
        if let (Generator::Normal(terrain), "igloo") = (&*self.generator, name) {
            return temples::nearest_igloo(terrain, chunk_x, chunk_z, regions.max(1) * 27 / temples::SPACING + 1);
        }

        // Храмы и хижины — по областям 32×32 чанка (temples.rs).
        if let (Generator::Normal(terrain), Some(kind)) = (&*self.generator, temples::Kind::ALL.into_iter().find(|kind| kind.name() == name)) {
            return temples::nearest(kind, terrain, chunk_x, chunk_z, regions.max(1) * 27 / temples::SPACING + 1);
        }

        // Разрушенные порталы — по областям 40×40 чанков (ruined_portal.rs).
        if let Some(variant) = ruined_portal::Variant::by_name(name) {
            return ruined_portal::nearest(&self.generator, variant, chunk_x, chunk_z, regions.max(1) * 27 / ruined_portal::SPACING + 1);
        }

        // Кораблекрушения — по областям 24×24 чанка, клады — по чанкам
        // (shipwreck.rs).
        if let (Generator::Normal(terrain), "shipwreck" | "shipwreck_beached") = (&*self.generator, name) {
            return shipwreck::nearest(terrain, name == "shipwreck_beached", chunk_x, chunk_z, regions.max(1) * 27 / shipwreck::SPACING + 1);
        }

        // Руины в океане — по областям 20×20 чанков (ocean_ruin.rs).
        if let (Generator::Normal(terrain), "ocean_ruin_cold" | "ocean_ruin_warm") = (&*self.generator, name) {
            return ocean_ruin::nearest(terrain, name == "ocean_ruin_warm", chunk_x, chunk_z, regions.max(1) * 27 / ocean_ruin::SPACING + 1);
        }

        // Лесные особняки — по областям 80×80 чанков (mansion.rs).
        if let (Generator::Normal(terrain), "mansion") = (&*self.generator, name) {
            return mansion::nearest(terrain, chunk_x, chunk_z, regions.max(1) * 27 / mansion::SPACING + 1);
        }

        // Руины тропы — по областям 34×34 чанка (trail_ruins.rs).
        if let (Generator::Normal(terrain), "trail_ruins") = (&*self.generator, name) {
            return trail_ruins::nearest(terrain, chunk_x, chunk_z, regions.max(1) * 27 / trail_ruins::SPACING + 1);
        }

        if let (Generator::Normal(terrain), "buried_treasure") = (&*self.generator, name) {
            return shipwreck::nearest_treasure(terrain, chunk_x, chunk_z, 100);
        }

        // Деревни — по областям 34×34 чанка (village.rs).
        if let (Generator::Normal(terrain), Some(kind)) = (&*self.generator, name.strip_prefix("village_").and_then(|kind| village::Kind::ALL.into_iter().find(|known| known.name() == kind))) {
            return village::nearest(terrain, kind, chunk_x, chunk_z, regions.max(1) * 27 / village::SPACING + 1);
        }

        let (size, start): (i32, Start<'_>) = match (&*self.generator, name) {
            (Generator::Nether(nether), "fortress") => (27, Box::new(|x, z| nether.fortress_start(x, z))),
            (Generator::Nether(nether), "bastion_remnant") => (27, Box::new(|x, z| nether.bastion_start(x, z))),
            (Generator::End(end), "end_city") => (end_city::REGION, Box::new(|x, z| end.city_at(x, z).map(|(x, z, _)| (x, z)))),
            _ => return None,
        };

        let (home_x, home_z) = (chunk_x.div_euclid(size), chunk_z.div_euclid(size));
        let distance = |(x, z): (i32, i32)| (i64::from(x - chunk_x)).pow(2) + (i64::from(z - chunk_z)).pow(2);
        let mut best: Option<(i32, i32)> = None;

        for ring in 0..=regions {
            for dx in -ring..=ring {
                for dz in -ring..=ring {
                    if dx.abs() != ring && dz.abs() != ring {
                        continue;
                    }

                    if let Some(found) = start(home_x + dx, home_z + dz)
                        && best.is_none_or(|known| distance(found) < distance(known))
                    {
                        best = Some(found);
                    }
                }
            }

            // Ближе найденного следующий круг регионов уже не даст.
            if let Some(found) = best
                && (distance(found) as f64).sqrt() < f64::from(ring * size)
            {
                break;
            }
        }

        best
    }

    /// Куда зовёт око Края из точки (x, z): угол чанка старта ближайшей
    /// крепости (вики, Eye of Ender: «chunk coordinates 0, ~, 0») на высоте
    /// верха её винтовой лестницы [проверить высоту: оригинал зовёт вниз
    /// всякий раз, как крепость ближе 12 блоков, eye-of-ender-near]. Для
    /// чёрного ящика в ровном мире крепость задаётся переменной окружения
    /// `MCSA_STRONGHOLD_AT=x,z` (ровный мир оригинала на 25566 без построек,
    /// а полёт снимается со своей копии оригинала — eye-of-ender-far).
    pub fn stronghold_target(&self, x: f64, z: f64) -> Option<(f64, f64, f64)> {
        if let Ok(text) = std::env::var("MCSA_STRONGHOLD_AT") {
            let numbers: Vec<f64> = text.split(',').filter_map(|part| part.trim().parse().ok()).collect();

            if let [sx, sz] = numbers[..] {
                return Some((sx, -20.0, sz));
            }
        }

        let Generator::Normal(terrain) = &*self.generator else {
            return None;
        };
        let (chunk_x, chunk_z) = stronghold::nearest(terrain, (x.floor() as i32).div_euclid(16), (z.floor() as i32).div_euclid(16))?;

        Some((f64::from(chunk_x * 16), f64::from(stronghold::entrance_y(terrain.seed(), chunk_x, chunk_z)), f64::from(chunk_z * 16)))
    }

    /// Ставит биом клеткам 4×4×4 в загруженных чанках (`fillbiome`): только
    /// тем, где сейчас `only` (если задан). Сколько клеток сменилось.
    pub fn fill_biome(&mut self, from: (i32, i32, i32), to: (i32, i32, i32), biome: BiomeId, only: Option<BiomeId>) -> usize {
        let mut changed = 0;

        for cell_x in from.0.div_euclid(4)..=to.0.div_euclid(4) {
            for cell_z in from.2.div_euclid(4)..=to.2.div_euclid(4) {
                for cell_y in from.1.div_euclid(4)..=to.1.div_euclid(4) {
                    let (x, y, z) = (cell_x * 4, cell_y * 4, cell_z * 4);

                    if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
                        continue;
                    }

                    let key = chunk_key(x, z);
                    let Some(chunk) = self.chunks.get_mut(&key) else {
                        continue;
                    };

                    let layer = ((y - MIN_Y) / 4) as usize;
                    let cell = biome_cell(layer, (x.rem_euclid(CHUNK_SIZE) / 4) as usize, (z.rem_euclid(CHUNK_SIZE) / 4) as usize);

                    if chunk.biomes[cell] == biome || only.is_some_and(|wanted| chunk.biomes[cell] != wanted) {
                        continue;
                    }

                    chunk.biomes[cell] = biome;
                    changed += 1;
                    self.dirty.insert(key);
                }
            }
        }

        changed
    }

    /// Незер ли это: там свои правила для жидкостей.
    pub fn is_nether(&self) -> bool {
        self.kind == WorldKind::Nether
    }

    /// Энд ли это: ни солнца, ни неба.
    pub fn is_end(&self) -> bool {
        self.kind == WorldKind::End
    }

    /// Древние стражи подводных крепостей в только что сложенном чанке.
    /// Вагонетки с сундуком шахт, стоящие в чанке: клетка рельсы и случай
    /// добычи (mineshaft.rs).
    pub fn mineshaft_carts(&self, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), u64)> {
        match &*self.generator {
            Generator::Normal(terrain) => mineshaft::carts(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Ведьмы и чёрные кошки хижин, стоящих в чанке (temples.rs).
    pub fn hut_dwellers(&self, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
        match &*self.generator {
            Generator::Normal(terrain) => temples::hut_dwellers(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Житель и зомби-житель подвалов иглу в чанке (temples.rs).
    pub fn igloo_dwellers(&self, chunk_x: i32, chunk_z: i32) -> Vec<((f64, f64, f64), bool)> {
        match &*self.generator {
            Generator::Normal(terrain) => temples::igloo_dwellers(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Внутри ли рамки хижины ведьмы клетка (правило появления ведьм).
    pub fn in_swamp_hut(&self, x: i32, y: i32, z: i32) -> bool {
        match &*self.generator {
            Generator::Normal(terrain) => temples::in_hut(terrain, x, y, z),
            _ => false,
        }
    }

    pub fn monument_elders(&self, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
        match &*self.generator {
            Generator::Normal(terrain) => monument::elders(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Сущности-блоки испытательных камер в только что сложенном чанке:
    /// x, y, z мира, вид («spawner», «vault», «chest», «barrel», «dispenser»,
    /// «decorated_pot») и данные (trial_chambers::block_entities).
    #[allow(dead_code)] // [проверить] спавнеры и хранилища заберут их, когда будут готовы
    pub fn trial_chamber_block_entities(&self, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, i32, &'static str, &'static str)> {
        match &*self.generator {
            Generator::Normal(terrain) => trial_chambers::block_entities(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Поборники и вызыватели лесных особняков в только что сложенном чанке
    /// (mansion.rs).
    pub fn mansion_dwellers(&self, chunk_x: i32, chunk_z: i32) -> Vec<((f64, f64, f64), &'static str)> {
        match &*self.generator {
            Generator::Normal(terrain) => mansion::dwellers(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Утопленники руин в океане в только что сложенном чанке.
    pub fn ruin_drowned(&self, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
        match &*self.generator {
            Generator::Normal(terrain) => ocean_ruin::drowned(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// В рамке ли древнего города клетка: там мобы сами не появляются
    /// (ancient_city.rs).
    pub fn in_ancient_city(&self, x: i32, y: i32, z: i32) -> bool {
        match &*self.generator {
            Generator::Normal(terrain) => ancient_city::inside(terrain, x, y, z),
            _ => false,
        }
    }

    /// В области появления разбойников аванпоста ли клетка
    /// (pillager_outpost.rs).
    pub fn in_outpost(&self, x: i32, y: i32, z: i32) -> bool {
        match &*self.generator {
            Generator::Normal(terrain) => pillager_outpost::inside(terrain, x, y, z),
            _ => false,
        }
    }

    /// Железные големы клеток аванпостов в только что сложенном чанке
    /// (pillager_outpost.rs).
    pub fn outpost_golems(&self, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
        match &*self.generator {
            Generator::Normal(terrain) => pillager_outpost::golems(terrain, chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    pub fn outpost_allays(&self, chunk_x: i32, chunk_z: i32) -> Vec<(f64,f64,f64)> {
        match &*self.generator {
            Generator::Normal(terrain) => pillager_outpost::allays(terrain,chunk_x,chunk_z),
            _ => Vec::new(),
        }
    }

    /// В рамке ли подводной крепости клетка (правило появления стражей).
    pub fn in_monument(&self, x: i32, y: i32, z: i32) -> bool {
        match &*self.generator {
            Generator::Normal(terrain) => monument::inside(terrain, x, y, z),
            _ => false,
        }
    }

    /// Шалкеры энд-городов в только что сложенном чанке (в прочих мирах — нет).
    pub fn city_sentries(&self, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, i32)> {
        match &*self.generator {
            Generator::End(end) => end.city_sentries(chunk_x, chunk_z),
            _ => Vec::new(),
        }
    }

    /// Проёмы порталов, которые мир помнит.
    pub fn portals(&self) -> &[(i32, i32, i32)] {
        &self.portals
    }

    /// Запоминает новый проём портала и сразу пишет список на диск: их мало,
    /// а потерять портал значило бы построить при возвращении второй.
    pub fn note_portal(&mut self, corner: (i32, i32, i32)) {
        if !self.portals.contains(&corner) {
            self.portals.push(corner);
            self.save_portals();
        }
    }

    /// Забывает проём, которого больше нет.
    pub fn forget_portal(&mut self, corner: (i32, i32, i32)) {
        let before = self.portals.len();
        self.portals.retain(|known| *known != corner);

        if self.portals.len() != before {
            self.save_portals();
        }
    }

    fn save_portals(&self) {
        // Мир в памяти (тесты) на диск не пишет.
        if self.path.as_os_str().is_empty() {
            return;
        }

        let text: String = self.portals.iter().map(|(x, y, z)| format!("portal = {} {} {}\n", x, y, z)).collect();

        if let Err(error) = write_atomically(&self.path, PORTALS, &text) {
            log_error!("Не удалось записать {}: {}", PORTALS, error);
        }
    }

    /// Изменившиеся блоки, которые редстоун ещё не разобрал, и что там
    /// стояло раньше: последний изменившийся — первым.
    pub fn pop_changed(&mut self) -> Option<((i32, i32, i32), i32)> {
        self.changed.pop()
    }

    /// Помечает место: блок в нём сейчас сменит команда.
    pub fn note_commanded(&mut self, x: i32, y: i32, z: i32) {
        self.commanded.insert((x, y, z));
    }

    /// Сменила ли блок в этом месте команда (пометка снимается).
    pub fn take_commanded(&mut self, pos: (i32, i32, i32)) -> bool {
        self.commanded.remove(&pos)
    }

    /// Запоминает, что блок сломал сам мир: из него должен выпасть предмет.
    pub fn note_destroyed(&mut self, x: i32, y: i32, z: i32) {
        let state = self.get_block(x, y, z);

        if state != AIR {
            self.destroyed.push(((x, y, z), state));
        }
    }

    /// Запоминает, что блоку не на чем держаться и он должен упасть.
    pub fn note_falling(&mut self, x: i32, y: i32, z: i32) {
        let place = (x, y, z);

        if !self.falling.contains(&place) {
            self.falling.push(place);
        }
    }

    /// Забирает места, откуда пора падать.
    pub fn take_falling(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.falling)
    }

    /// Забирает сломанное миром — чтобы такт уронил предметы.
    pub fn take_destroyed(&mut self) -> Vec<((i32, i32, i32), i32)> {
        std::mem::take(&mut self.destroyed)
    }

    /// Отмечает место изменившимся, хотя блок в нём тот же: у сравнителя
    /// может смениться сила на выходе, а состояние блока — нет.
    pub fn note_changed(&mut self, x: i32, y: i32, z: i32) {
        let state = self.get_block(x, y, z);

        self.changed.push(((x, y, z), state));
    }

    /// Сколько мест ждут пересчёта — по этому видно, что мир не «кипит»:
    /// когда всё утекло, ждать нечему. Нужно для проверок.
    #[cfg(test)]
    pub fn scheduled_count(&self) -> usize {
        self.waiting.len()
    }
}

/// Что стало с чанком, который взялись приготовить.
#[allow(clippy::large_enum_variant)] // чанк большой, но живёт в очереди мгновение; коробка ради этого не нужна
enum Taken {
    /// Чанк готов: прочитан с диска или сложен заново. Рядом — места у воды
    /// и лавы, которые надо сразу пустить в такт.
    Loaded {
        chunk: Chunk,
        from_disk: bool,
        wake: Vec<((i32, i32, i32), u64)>,
    },

    /// Чанка нет и складывать его нельзя: диск не ответил (запись на диске
    /// может быть цела — затирать её новым чанком нельзя), или испорченную
    /// запись не удалось отложить, или чанк за пределами мира.
    Unavailable,
}

/// Читает чанк с диска, а если его там нет — складывает заново.
///
/// Свободная функция, а не метод: читать чанк нужно и тогда, когда мира под
/// рукой нет — например, пока он занят тактом.
///
/// «Нет на диске» и «не читается» — разные вещи. Нет — складываем. Запись
/// испорчена — сперва откладываем её байты в сторону (`set_aside`), и только
/// потом складываем чанк заново: он ляжет на её место. Диск не ответил —
/// не делаем ничего, чанк попробуют прочитать снова.
fn take_chunk(path: &Path, generator: &Generator, chunk_x: i32, chunk_z: i32) -> Taken {
    if !chunk_in_limits(chunk_x, chunk_z) {
        log_debug!("Мир: чанк {} {} за пределами мира", chunk_x, chunk_z);
        return Taken::Unavailable;
    }

    let generated = || {
        let chunk = Chunk::generated(generator, chunk_x, chunk_z);
        let wake = chunk.fluids_to_wake(chunk_x, chunk_z);

        Taken::Loaded { chunk, from_disk: false, wake }
    };

    if path.as_os_str().is_empty() {
        return generated();
    }

    let directory = path.join(REGIONS);

    let (raw, why) = match region::read_stored(&directory, chunk_x, chunk_z) {
        Ok(region::Stored::Missing) => return generated(),
        Ok(region::Stored::Found(bytes)) => match Chunk::from_bytes(&bytes) {
            // Свет на диске не хранится: он считается заново по блокам.
            Some(mut chunk) => {
                chunk.light_up();

                let wake = chunk.fluids_to_wake(chunk_x, chunk_z);

                return Taken::Loaded { chunk, from_disk: true, wake };
            }
            None => (bytes, "содержимое чанка не разбирается"),
        },
        Ok(region::Stored::Damaged { raw, why }) => (raw, why),
        Err(e) => {
            log_error!("Не удалось прочитать чанк {} {}: {} — попробую позже", chunk_x, chunk_z, e);
            return Taken::Unavailable;
        }
    };

    match set_aside(&directory, chunk_x, chunk_z, &raw) {
        Ok(copy) => {
            log_error!(
                "Чанк {} {} испорчен ({}): его байты отложены в {}, складываю чанк заново",
                chunk_x,
                chunk_z,
                why,
                copy.display()
            );

            generated()
        }
        Err(e) => {
            log_error!(
                "Чанк {} {} испорчен ({}), и отложить его не удалось: {} — чанк не загружен, запись на диске не тронута",
                chunk_x,
                chunk_z,
                why,
                e
            );

            Taken::Unavailable
        }
    }
}

/// Откладывает испорченную запись чанка в `region/corrupt/`, чтобы её можно
/// было разобрать руками. Возвращает, куда отложено.
fn set_aside(directory: &Path, chunk_x: i32, chunk_z: i32, raw: &[u8]) -> io::Result<PathBuf> {
    let corrupt = directory.join("corrupt");

    fs::create_dir_all(&corrupt)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let copy = corrupt.join(format!("c.{}.{}.{}.bin", chunk_x, chunk_z, now));

    let mut file = File::create(&copy)?;

    file.write_all(raw)?;
    file.sync_all()?;
    region::sync_directory(&corrupt);

    Ok(copy)
}

/// Сколько блоков над землёй может занять растительность: кактус — самый
/// высокий из того, что мы сажаем.
const PLANT_ROOM: i32 = 4;

/// Готовый чанк, который осталось положить в мир.
pub struct ReadyChunk {
    /// Сам чанк — или то, что его не удалось приготовить.
    taken: Taken,

    /// Поколение выгрузки, в котором чанк взялись готовить (см. `accept`).
    epoch: u64,
}

/// Откуда берутся ещё не готовые чанки: папка мира и правила складывания.
///
/// Живёт отдельно от мира нарочно: пока чанк читается с диска или
/// складывается заново, замок мира держать нельзя — такт не должен ждать.
pub struct ChunkSource {
    path: PathBuf,
    generator: Arc<Generator>,

    /// Поколение выгрузки на момент, когда источник взят.
    epoch: u64,
}

impl ChunkSource {
    /// Читает чанк с диска, а если его там нет — складывает заново.
    pub fn take(&self, chunk_x: i32, chunk_z: i32) -> ReadyChunk {
        ReadyChunk {
            taken: take_chunk(&self.path, &self.generator, chunk_x, chunk_z),
            epoch: self.epoch,
        }
    }
}

/// Чем складывается ещё не сложенный кусок мира.
///
/// Разница в размере вариантов не важна: генератор у мира один и живёт
/// в `Arc`, копий не делается.
#[allow(clippy::large_enum_variant)]
enum Generator {
    /// Обычный мир: рельеф, биомы, море.
    Normal(Terrain),

    /// Ровный мир — настройка `level-type=minecraft:flat` у оригинала.
    /// Земля одинаковой высоты по всей площади.
    Flat,

    /// Незер: пол и потолок, море лавы.
    Nether(nether::Nether),

    /// Энд: главный остров, пустота и внешние острова.
    End(the_end::End),
}

impl World {
    /// Где появляется игрок, зашедший впервые.
    ///
    /// В ровном мире это начало координат. В обычном там может оказаться
    /// море, поэтому от начала координат расходимся кругами и ищем первое
    /// место на суше — как делает игра, выбирая точку появления.
    ///
    /// Ищется один раз: дальше берётся запомненная (она же пишется в
    /// level.dat), а не обходятся заново тысячи столбцов под замком мира.
    pub fn spawn_position(&self) -> (f64, f64, f64) {
        let mut spawn = self.spawn.lock().expect("точка появления захвачена другим потоком");

        *spawn.get_or_insert_with(|| self.find_spawn())
    }

    /// Новая общая точка появления (`setworldspawn`); пишется в level.dat.
    pub fn set_spawn_position(&self, place: (f64, f64, f64)) {
        *self.spawn.lock().expect("точка появления захвачена другим потоком") = Some(place);
    }

    fn find_spawn(&self) -> (f64, f64, f64) {
        // В Энде появляются на площадке, а не по суше у начала координат.
        if matches!(*self.generator, Generator::End(_)) {
            let (x, y, z) = the_end::PLATFORM;

            return (f64::from(x) + 0.5, f64::from(y + 1), f64::from(z) + 0.5);
        }

        let place = spawn_search::find(|x, z| self.generator.column_at(x, z));
        log_info!("Точка появления: {} {} {}", place.0, place.1, place.2);
        place
    }
}

impl Generator {
    /// Всё про столбец в этом месте: высота поверхности и биом. Считается
    /// один раз на место — дальше столбец заполняется по нему.
    fn column_at(&self, x: i32, z: i32) -> Column {
        match self {
            Generator::Normal(terrain) => terrain.column_at(x, z),
            Generator::Flat | Generator::Nether(_) | Generator::End(_) => Column::flat(flat::GROUND),
        }
    }

    /// Сетка плотности для участка — только у «Обычного» мира: остальные
    /// обходятся картой высот.
    fn relief_grid(
        &self,
        x_from: i32,
        z_from: i32,
        x_to: i32,
        z_to: i32,
    ) -> Option<terrain::ReliefGrid> {
        match self {
            Generator::Normal(terrain) if terrain.style() == terrain::Style::Vanilla => {
                Some(terrain.relief_grid(x_from, z_from, x_to, z_to))
            }
            _ => None,
        }
    }

    /// Столбец, когда сетка плотности уже построена.
    fn column_in(&self, x: i32, z: i32, grid: Option<&terrain::ReliefGrid>) -> Column {
        match self {
            Generator::Normal(terrain) => terrain.column_in(x, z, grid),
            Generator::Flat | Generator::Nether(_) | Generator::End(_) => Column::flat(flat::GROUND),
        }
    }

    /// Камень ли в этом месте по рельефу.
    fn solid_in(
        &self,
        grid: Option<&terrain::ReliefGrid>,
        column: &Column,
        x: i32,
        y: i32,
        z: i32,
    ) -> bool {
        match self {
            Generator::Normal(terrain) => terrain.solid_in(grid, column, x, y, z),
            Generator::Flat | Generator::Nether(_) | Generator::End(_) => y <= column.height,
        }
    }

    /// Сетка шумов пещер над чанком — где пещеры есть.
    fn cave_grid(&self, x: i32, z: i32, y_to: i32) -> Option<terrain::CaveGrid> {
        match self {
            Generator::Normal(terrain) => {
                Some(terrain.cave_grid(x, z, x + CHUNK_SIZE - 1, z + CHUNK_SIZE - 1, y_to))
            }
            Generator::Flat | Generator::Nether(_) | Generator::End(_) => None,
        }
    }

    /// Что стоит в этом месте столбца.
    #[allow(clippy::too_many_arguments)]
    fn block_with(
        &self,
        column: &Column,
        caves: Option<&terrain::CaveGrid>,
        x: i32,
        y: i32,
        z: i32,
        solid: bool,
        run_top: i32,
    ) -> i32 {
        match (self, caves) {
            (Generator::Normal(terrain), Some(caves)) => {
                terrain.block_with(column, caves, x, y, z, solid, run_top)
            }
            _ => flat::block_at(y),
        }
    }

    /// Кладёт в чанк руду и пятна земли, гравия, гранита и прочего.
    ///
    /// Где лежат гнёзда, решает рельеф (`Terrain::ore_blocks_in`); здесь —
    /// только можно ли положить блок на это место: гнездо ложится лишь
    /// в камень, глубинный сланец, гранит, диорит, андезит и туф (вики,
    /// «Ore (feature)»), а блок, соседствующий с воздухом, пропускается
    /// с шансом из таблицы — каждый блок отдельно.
    fn lay_ores(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let Generator::Normal(terrain) = self else {
            return;
        };

        let stone = terrain::block_named("stone");
        let deepslate = terrain::block_named("deepslate");
        let takes = [
            stone,
            deepslate,
            terrain::block_named("granite"),
            terrain::block_named("diorite"),
            terrain::block_named("andesite"),
            terrain::block_named("tuff"),
        ];
        let water = terrain::block_named("water");
        let lava = terrain::block_named("lava");

        for (x, y, z, placement) in terrain.ore_blocks_in(chunk_x, chunk_z, chunk.highest()) {
            let (local_x, local_z) = (x - chunk_x * CHUNK_SIZE, z - chunk_z * CHUNK_SIZE);

            if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
                continue;
            }

            let here = chunk.block(local_x, y, local_z);

            if !takes.contains(&here) {
                continue;
            }

            if placement.air_skip > 0.0 {
                // Соседи за краем чанка нам не видны — считаем их камнем.
                let open = [
                    (1, 0, 0),
                    (-1, 0, 0),
                    (0, 1, 0),
                    (0, -1, 0),
                    (0, 0, 1),
                    (0, 0, -1),
                ]
                .iter()
                .any(|(dx, dy, dz)| {
                    let (nx, ny, nz) = (local_x + dx, y + dy, local_z + dz);

                    (0..CHUNK_SIZE).contains(&nx)
                        && (0..CHUNK_SIZE).contains(&nz)
                        && (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&ny)
                        && [AIR, water, lava].contains(&chunk.block(nx, ny, nz))
                });

                if open && terrain::chance_at(terrain.seed(), x, y, z, placement.air_skip) {
                    continue;
                }
            }

            // Под нулём — в глубинном сланце — у руды своя разновидность.
            let deep = placement.has_deep && (here == deepslate || y < 0);
            let name = if deep {
                format!("deepslate_{}", placement.block)
            } else {
                placement.block.to_string()
            };

            if let Some(state) = crate::blocks::state_by_name(&name) {
                chunk.put_generated(local_x, y, local_z, state, true);
            }
        }
    }

    /// Сажает деревья этого чанка.
    ///
    /// Смотреть приходится шире самого чанка: ствол у соседа, а крона —
    /// у нас. Поэтому обходим полосу вокруг чанка и рисуем то, что попало
    /// внутрь. Соседний чанк посчитает то же самое и нарисует свою часть —
    /// договариваться им не о чем.
    fn grow_trees(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, columns: &[Column]) {
        const REACH: i32 = terrain::TREE_REACH;

        let Generator::Normal(terrain) = self else {
            return;
        };

        let ground = TreeGround { seed: terrain.seed(), ..TreeGround::new() };
        let near = TreeColumns::new(self, chunk_x, chunk_z, columns);

        for local_x in -REACH..(CHUNK_SIZE + REACH) {
            for local_z in -REACH..(CHUNK_SIZE + REACH) {
                let x = chunk_x * CHUNK_SIZE + local_x;
                let z = chunk_z * CHUNK_SIZE + local_z;

                // Дерево редкое, а столбец считать дорого: сперва дешёвая
                // проверка «а не здесь ли ствол вообще», и только потом сам
                // столбец.
                if !terrain::tree_possible(terrain.seed(), x, z) {
                    continue;
                }

                let column = near.column(x, z);

                let Some(tree) = terrain.tree_at(&column, x, z) else {
                    continue;
                };

                // Земля годится под каждым столбцом ствола: толстый стоит
                // на четырёх, и земля под ними не дальше `TRUNK_DOWN` от
                // высоты ствола: ниже бревно до неё не дотянется
                // (`draw_tree`), выше — ствол ушёл бы в склон.
                let spots: &[(i32, i32)] = if tree.wide() { &[(0, 0), (1, 0), (0, 1), (1, 1)] } else { &[(0, 0)] };
                let height = |x: i32, z: i32| near.column(x, z).height;
                let fits = spots.iter().all(|&(dx, dz)| {
                    let spot = if (dx, dz) == (0, 0) { column } else { near.column(x + dx, z + dz) };

                    (column.height - spot.height).abs() <= TRUNK_DOWN && terrain.tree_ground_by(&spot, x + dx, z + dz, height)
                });

                if !fits {
                    continue;
                }

                draw_tree(chunk, &tree, local_x, local_z, column.height, x, z, &ground);
            }
        }

        snow_on_crowns(terrain, chunk, chunk_x, chunk_z, columns);
    }

    /// Какой биом у этого столбца.
    fn biome_of(&self, column: &Column) -> terrain::Biome {
        match self {
            Generator::Normal(_) => column.biome,
            Generator::Flat | Generator::Nether(_) | Generator::End(_) => terrain::Biome::Plains,
        }
    }

    /// До какой высоты имеет смысл считать столбец: выше неё пусто.
    ///
    /// Чуть выше поверхности: там стоят трава, цветы, снег и кактусы.
    fn top_of(&self, column: &Column) -> i32 {
        match self {
            Generator::Normal(_) => column.height.max(terrain::SEA) + PLANT_ROOM,
            Generator::Flat | Generator::Nether(_) | Generator::End(_) => flat::GROUND,
        }
    }
}

/// Снег в холодных местах лежит на самом верхнем блоке столбца — у дерева
/// на кроне, а не на земле под ней: так у оригинала (замер мира: в заснеженной
/// тайге на 97% крон снег, под кроной трава без снега). Поэтому после деревьев
/// снег со земли под кроной снимается, трава перестаёт быть заснеженной,
/// а на верхний лист или бревно ложится слой снега. Решается по своему
/// столбцу — одинаково при любом порядке сложения чанков.
fn snow_on_crowns(terrain: &Terrain, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, columns: &[Column]) {
    const SIDE: i32 = CHUNK_SIZE + 2;

    let (Some(snow), Some(grass), Some(plain_grass)) = (
        crate::blocks::states_of("snow"),
        crate::blocks::states_of("grass_block"),
        crate::blocks::state_by_name("grass_block"),
    ) else {
        return;
    };
    let layer = terrain::block_named("snow");
    let top = chunk.highest().min(MIN_Y + WORLD_HEIGHT - 1);

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let Some(column) = columns.get(((local_x + 1) * SIDE + local_z + 1) as usize) else {
                continue;
            };
            let ground = column.height;

            // Верхний непустой блок столбца: выше слоя снега на земле — значит,
            // над землёй дерево.
            let Some(y) = (ground + 2..=top).rev().find(|&y| chunk.block(local_x, y, local_z) != AIR) else {
                continue;
            };
            let (x, z) = (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);
            let here = chunk.block(local_x, y, local_z);
            let name = crate::blocks::block_at_state(here).unwrap_or("");

            if (name.ends_with("_leaves") || name.ends_with("_log"))
                && y < MIN_Y + WORLD_HEIGHT - 1
                && terrain.snow_falls(column, x, y + 1, z)
            {
                chunk.put_generated(local_x, y + 1, local_z, layer, true);
            }

            if (MIN_Y..MIN_Y + WORLD_HEIGHT - 1).contains(&ground)
                && (snow.0..=snow.1).contains(&chunk.block(local_x, ground + 1, local_z))
            {
                chunk.put_generated(local_x, ground + 1, local_z, AIR, true);

                if (grass.0..=grass.1).contains(&chunk.block(local_x, ground, local_z)) {
                    chunk.put_generated(local_x, ground, local_z, plain_grass, true);
                }
            }
        }
    }
}

/// На сколько блоков бревно низа ствола тянется вниз до земли, если под ним
/// пусто: у толстого ствола на склоне соседние столбцы ниже.
const TRUNK_DOWN: i32 = 3;

/// Столбцы вокруг чанка на время посадки деревьев. Свои (с каймой) чанк уже
/// посчитал; прочие считаются по одной сетке плотности на всю полосу вокруг
/// чанка и запоминаются: соседи ствола нужны проверке земли, и считать
/// каждый столбец отдельно в разы дороже. Ответ тот же, что у `column_at`.
struct TreeColumns<'a> {
    generator: &'a Generator,
    chunk_x: i32,
    chunk_z: i32,
    /// Столбцы чанка с каймой в блок, как их считает `Chunk::generated`.
    columns: &'a [Column],
    grid: std::cell::OnceCell<Option<terrain::ReliefGrid>>,
    more: std::cell::RefCell<HashMap<(i32, i32), Column>>,
}

impl<'a> TreeColumns<'a> {
    fn new(generator: &'a Generator, chunk_x: i32, chunk_z: i32, columns: &'a [Column]) -> TreeColumns<'a> {
        TreeColumns {
            generator,
            chunk_x,
            chunk_z,
            columns,
            grid: std::cell::OnceCell::new(),
            more: std::cell::RefCell::new(HashMap::new()),
        }
    }

    fn column(&self, x: i32, z: i32) -> Column {
        const SIDE: i32 = CHUNK_SIZE + 2;
        // Полоса, где могут стоять стволы, и ещё блок — для соседей.
        const EDGE: i32 = terrain::TREE_REACH + 2;

        let (local_x, local_z) = (x - self.chunk_x * CHUNK_SIZE, z - self.chunk_z * CHUNK_SIZE);

        if (-1..=CHUNK_SIZE).contains(&local_x)
            && (-1..=CHUNK_SIZE).contains(&local_z)
            && let Some(column) = self.columns.get(((local_x + 1) * SIDE + local_z + 1) as usize)
        {
            return *column;
        }

        if let Some(column) = self.more.borrow().get(&(x, z)) {
            return *column;
        }

        let inside = (-EDGE..CHUNK_SIZE + EDGE).contains(&local_x) && (-EDGE..CHUNK_SIZE + EDGE).contains(&local_z);
        let column = if inside {
            let grid = self.grid.get_or_init(|| {
                let (x0, z0) = (self.chunk_x * CHUNK_SIZE - EDGE, self.chunk_z * CHUNK_SIZE - EDGE);

                self.generator.relief_grid(x0, z0, x0 + CHUNK_SIZE + 2 * EDGE - 1, z0 + CHUNK_SIZE + 2 * EDGE - 1)
            });

            self.generator.column_in(x, z, grid.as_ref())
        } else {
            self.generator.column_at(x, z)
        };

        self.more.borrow_mut().insert((x, z), column);
        column
    }
}

/// Что дерево видит под собой: воду и землю, которую может заменить.
/// Ищется по имени один раз на чанк.
struct TreeGround {
    water: i32,
    /// Земля, которую дерево может заменить подзолом или корнями.
    soil: Vec<(i32, i32)>,
    /// Трава: под нижним бревном она становится простой землёй, прочая
    /// земля — подзол, ил — остаётся как есть.
    grass: Option<(i32, i32)>,
    /// Слой снега: земля под ним видна деревьям, как под воздухом.
    snow: Option<(i32, i32)>,
    dirt: i32,
    /// Семя мира: форма дерева своя в каждом мире.
    seed: i64,
}

impl TreeGround {
    fn new() -> TreeGround {
        TreeGround {
            seed: 0,
            water: terrain::block_named("water"),
            snow: crate::blocks::states_of("snow"),
            dirt: terrain::block_named("dirt"),
            soil: [
                "grass_block",
                "dirt",
                "coarse_dirt",
                "podzol",
                "rooted_dirt",
                "mud",
            ]
            .iter()
            .filter_map(|name| crate::blocks::states_of(name))
            .collect(),
            grass: crate::blocks::states_of("grass_block"),
        }
    }

    fn is_soil(&self, state: i32) -> bool {
        self.soil.iter().any(|(low, high)| (*low..=*high).contains(&state))
    }

    fn is_snow(&self, state: i32) -> bool {
        self.snow.is_some_and(|(low, high)| (low..=high).contains(&state))
    }

    /// Может ли бревно встать на место этого блока. Бревно не режет
    /// толщу — камень, руду, песок, землю: ветвь, упёршаяся в склон,
    /// обрывается, а не прорастает сквозь гору. Встаёт в
    /// воздух, воду, листву, траву и снег и на место другого бревна; низ
    /// ствола (`dy` ≤ 1) — и в землю: у толстого ствола на неровном месте
    /// он уходит в неё.
    fn holds_log(&self, state: i32, dy: i32) -> bool {
        if state == AIR || state == self.water || !crate::blocks::conductive(state) {
            return true;
        }

        if dy <= 1 && self.is_soil(state) {
            return true;
        }

        crate::blocks::block_at_state(state)
            .is_some_and(|name| name.ends_with("_log") || name.ends_with("_wood") || name.contains("mushroom"))
    }
}

/// Расстояние каждого листа дерева до ближайшего его бревна — обход по
/// шести соседям, не дальше семи.
pub(crate) fn leaf_distances(blocks: &[(i32, i32, i32, terrain::Part)]) -> std::collections::HashMap<(i32, i32, i32), i32> {
    use std::collections::{HashMap, HashSet, VecDeque};

    let leaves: HashSet<(i32, i32, i32)> = blocks
        .iter()
        .filter(|(_, _, _, part)| *part == terrain::Part::Leaves)
        .map(|(x, y, z, _)| (*x, *y, *z))
        .collect();
    let mut distance: HashMap<(i32, i32, i32), i32> = HashMap::new();
    let mut queue: VecDeque<((i32, i32, i32), i32)> = blocks
        .iter()
        .filter(|(_, _, _, part)| matches!(part, terrain::Part::Log(_)))
        .map(|(x, y, z, _)| ((*x, *y, *z), 0))
        .collect();

    while let Some(((x, y, z), d)) = queue.pop_front() {
        if d >= 7 {
            continue;
        }

        for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let next = (x + dx, y + dy, z + dz);

            if leaves.contains(&next) && !distance.contains_key(&next) {
                distance.insert(next, d + 1);
                queue.push_back((next, d + 1));
            }
        }
    }

    distance
}

/// Рисует в чанке ту часть дерева, что попала в него. Ствол стоит в (x, z)
/// — местные (local_x, local_z) — на земле высоты `height`.
#[allow(clippy::too_many_arguments)]
fn draw_tree(
    chunk: &mut Chunk,
    tree: &trees::Tree,
    local_x: i32,
    local_z: i32,
    height: i32,
    x: i32,
    z: i32,
    ground: &TreeGround,
) {
    let (water, grass) = (ground.water, ground.grass);
    let inside = |bx: i32, by: i32, bz: i32| {
        (0..CHUNK_SIZE).contains(&bx) && (0..CHUNK_SIZE).contains(&bz) && (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&by)
    };

    let by_text = |text: &str| {
        crate::blocks::state_from_text(text)
            .unwrap_or_else(|| terrain::block_named(text))
    };
    let leaves = by_text(tree.leaves);
    // У огромных грибов «ствол» записан полным состоянием, оси
    // у него нет.
    let log = |axis: terrain::Axis| {
        if tree.log.contains('[') {
            return by_text(tree.log);
        }

        let axis = match axis {
            terrain::Axis::X => "x",
            terrain::Axis::Y => "y",
            terrain::Axis::Z => "z",
        };

        crate::blocks::state_from_text(&format!("{}[axis={}]", tree.log, axis))
            .unwrap_or_else(|| terrain::block_named(tree.log))
    };
    let logs = [
        log(terrain::Axis::X),
        log(terrain::Axis::Y),
        log(terrain::Axis::Z),
    ];
    let blocks = tree.blocks_in(ground.seed, x, z);

    // Земля под деревом — первой: пока над ней не выросли ни
    // корни, ни ствол, её верх виден сразу. Где нашлась земля, запоминаем:
    // ковёр мха ляжет прямо на неё.
    let mut found: Vec<((i32, i32), i32)> = Vec::new();

    for (dx, dy, dz, part) in &blocks {
        if let terrain::Part::Ground(text) = part {
            let (bx, bz) = (local_x + dx, local_z + dz);

            if !(0..CHUNK_SIZE).contains(&bx) || !(0..CHUNK_SIZE).contains(&bz) {
                continue;
            }

            let near = height + dy;
            let lowest = (near - 3).max(MIN_Y);
            let highest = (near + 3).min(MIN_Y + WORLD_HEIGHT - 1);

            // Сверху вниз до первого блока, что не воздух, не вода и не
            // слой снега; меняем его, только если это земля.
            let Some(y) = (lowest..=highest).rev().find(|&y| {
                let here = chunk.block(bx, y, bz);

                here != AIR && here != water && !ground.is_snow(here)
            }) else {
                continue;
            };

            let here = chunk.block(bx, y, bz);

            if !ground.is_soil(here) {
                continue;
            }

            found.push(((bx, bz), y));

            if *text == "dirt"
                && !grass.is_some_and(|(low, high)| (low..=high).contains(&here))
            {
                continue;
            }

            if let Some(state) = crate::blocks::state_from_text(text) {
                chunk.put_generated(bx, y, bz, state, true);
            }
        }
    }

    // У огромного гриба грань, упёртая в соседний блок того же гриба, —
    // поры, свободная — шляпка (так и в оригинале). Каждый блок получает
    // своё состояние: скрытые грани у Java не видны, а Bedrock по ним
    // выбирает вариант блока — иначе у крайних блоков стенки не та грань.
    let mushroom = tree.leaves.contains("mushroom_block");
    let taken: std::collections::HashSet<(i32, i32, i32)> = if mushroom {
        blocks.iter().filter(|(_, _, _, part)| !matches!(part, terrain::Part::Ground(_))).map(|(x, y, z, _)| (*x, *y, *z)).collect()
    } else {
        std::collections::HashSet::new()
    };
    let cap = |text: &str, (x, y, z): (i32, i32, i32)| mushroom_cap(text, |dx, dy, dz| taken.contains(&(x + dx, y + dy, z + dz)));

    // Листва не затирает стволы и ветви, поэтому ставится раньше,
    // а брёвна — поверх неё. В воду лист встаёт затопленным: нижняя
    // листва мангра уходит под воду болота.
    //
    // У каждого листа — расстояние до ближайшего бревна своего дерева, как
    // у оригинала (1 — рядом с бревном, 7 — дальше шести). Без этого вся
    // листва считалась бы оторванной и осыпалась бы, как только появится
    // опадание. Считается по блокам самого дерева, поэтому чанки на стыке
    // решают одинаково.
    let distances = if mushroom { std::collections::HashMap::new() } else { leaf_distances(&blocks) };
    let leaf_state = |distance: i32, wet: bool| {
        crate::blocks::state_from_text(&format!("{}[distance={},waterlogged={}]", tree.leaves, distance, wet))
    };

    for (dx, dy, dz, part) in &blocks {
        if *part == terrain::Part::Leaves {
            let (bx, by, bz) = (local_x + dx, height + dy, local_z + dz);

            if !inside(bx, by, bz) {
                continue;
            }

            let here = chunk.block(bx, by, bz);
            let distance = distances.get(&(*dx, *dy, *dz)).copied().unwrap_or(7);
            let state = if mushroom {
                Some(cap(tree.leaves, (*dx, *dy, *dz)).unwrap_or(leaves))
            } else {
                leaf_state(distance, here == water).or(Some(leaves))
            };

            if let Some(state) = state {
                chunk.put_generated(bx, by, bz, state, here == water);
            }
        }
    }

    for (dx, dy, dz, part) in &blocks {
        let terrain::Part::Log(axis) = part else {
            continue;
        };
        let (bx, by, bz) = (local_x + dx, height + dy, local_z + dz);

        if !inside(bx, by, bz) || !ground.holds_log(chunk.block(bx, by, bz), *dy) {
            continue;
        }

        let state = logs[*axis as usize];

        chunk.put_generated(bx, by, bz, state, true);

        // Низ ствола на склоне: соседние столбцы толстого ствола ниже, и
        // под бревном пусто. Бревно тянется вниз до земли — сквозь воздух,
        // воду и растения, не дальше `TRUNK_DOWN`. Смотрим только свой
        // столбец, поэтому оба чанка на стыке решают одинаково.
        if *dy <= 1 && *axis == terrain::Axis::Y {
            for below in (by - TRUNK_DOWN..by).rev() {
                if below < MIN_Y {
                    break;
                }

                let here = chunk.block(bx, below, bz);

                // Дошли до земли: трава под бревном становится землёй, как
                // под любым стволом.
                if here != AIR && here != water && crate::blocks::conductive(here) {
                    if grass.is_some_and(|(low, high)| (low..=high).contains(&here)) {
                        chunk.put_generated(bx, below, bz, ground.dirt, true);
                    }

                    break;
                }

                chunk.put_generated(bx, below, bz, state, true);
            }
        }
    }

    // Лианы, какао, корни — последними и только на свободное
    // место: в воздух, а что бывает затоплено — и в воду. Висячая плеть
    // (лиана, бледный мох) записана подряд сверху вниз; не встал её блок —
    // не ставится и всё, что ниже: висеть ему не на чем.
    let moss = crate::blocks::state_from_text("pale_hanging_moss[tip=false]");
    let moss_tip = crate::blocks::state_from_text("pale_hanging_moss[tip=true]");
    let mut cut: Option<(i32, i32, i32)> = None;

    for (dx, dy, dz, part) in &blocks {
        let after_cut = cut.take();
        let terrain::Part::Block(text) = part else {
            continue;
        };
        let hanging = text.starts_with("vine") || text.starts_with("pale_hanging_moss");

        if hanging && after_cut == Some((*dx, *dy + 1, *dz)) {
            cut = Some((*dx, *dy, *dz));
            continue;
        }

        // Ковёр мха — прямо на найденную под деревом землю, а не на
        // высоту ствола: на склоне он иначе висел бы.
        let dy = if text.starts_with("pale_moss_carpet") {
            match found.iter().find(|((fx, fz), _)| (*fx, *fz) == (local_x + dx, local_z + dz)) {
                Some((_, y)) => y + 1 - height,
                None => continue,
            }
        } else {
            *dy
        };
        let (bx, by, bz) = (local_x + dx, height + dy, local_z + dz);

        if !inside(bx, by, bz) {
            continue;
        }

        let here = chunk.block(bx, by, bz);
        let state = if here == AIR && mushroom && text.contains("mushroom_block") {
            cap(text, (*dx, dy, *dz))
        } else if here == AIR {
            crate::blocks::state_from_text(text)
        } else if here == water {
            let wet = match text.strip_suffix(']') {
                Some(open) => format!("{},waterlogged=true]", open),
                None => format!("{}[waterlogged=true]", text),
            };

            crate::blocks::state_from_text(&wet)
        } else {
            None
        };

        if let Some(state) = state {
            chunk.put_generated(bx, by, bz, state, true);
        } else if hanging {
            cut = Some((*dx, dy, *dz));

            // Мох, оборванный на середине, кончается кончиком.
            if moss.is_some()
                && by + 1 < MIN_Y + WORLD_HEIGHT
                && Some(chunk.block(bx, by + 1, bz)) == moss
                && let Some(tip) = moss_tip
            {
                chunk.put_generated(bx, by + 1, bz, tip, true);
            }
        }
    }
}

/// Блок шляпки огромного гриба: грани из записи (что не указано — шляпка),
/// а где вплотную другой блок того же гриба — поры.
fn mushroom_cap(text: &str, neighbour: impl Fn(i32, i32, i32) -> bool) -> Option<i32> {
    const FACES: [(&str, i32, i32, i32); 6] =
        [("up", 0, 1, 0), ("down", 0, -1, 0), ("north", 0, 0, -1), ("south", 0, 0, 1), ("east", 1, 0, 0), ("west", -1, 0, 0)];

    let (name, given) = text.split_once('[').map_or((text, ""), |(name, rest)| (name, rest.trim_end_matches(']')));
    let faces: Vec<String> = FACES
        .iter()
        .map(|(face, dx, dy, dz)| {
            let open = !given.split(',').any(|pair| pair == format!("{}=false", face)) && !neighbour(*dx, *dy, *dz);
            format!("{}={}", face, open)
        })
        .collect();

    crate::blocks::state_from_text(&format!("{}[{}]", name, faces.join(",")))
}

/// Случайное семя для нового мира: берём его из часов.
fn random_seed() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as i64)
        .unwrap_or(0)
}

/// Имя файла с общими сведениями о мире — как у оригинального сервера.
/// Внутри — наш собственный простой вид, не формат игры.
const LEVEL: &str = "level.dat";

/// Файл тактов мира: что ждёт своего срока и начатые ходы поршней.
const TICKS: &str = "ticks.dat";

/// Файл проёмов порталов мира.
const PORTALS: &str = "portals.dat";

/// Читает список проёмов порталов. Нет файла — порталов нет.
fn read_portals(path: &Path) -> Vec<(i32, i32, i32)> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };

    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(name, _)| name.trim() == "portal")
        .filter_map(|(_, value)| {
            let mut parts = value.split_whitespace().map(|part| part.parse::<i32>().ok());
            Some((parts.next()??, parts.next()??, parts.next()??))
        })
        .collect()
}

/// Место из файла мира, ждущее своего такта: где, какой такт, через сколько
/// и с каким приоритетом.
type SavedTick = ((i32, i32, i32), TickKind, u64, i8);

/// Едущий блок из файла мира: чей поршень, куда встанет и что это за блок.
type SavedCargo = ((i32, i32, i32), (i32, i32, i32), i32);

/// Что прочиталось из файла мира.
struct Level {
    age: u64,
    time_offset: i64,

    /// Семя мира. `None` — в файле его нет: мир новый.
    seed: Option<i64>,

    /// Тип мира. `None` — в файле его нет: мир новый или записан прежним
    /// сервером, у которого типов не было.
    kind: Option<WorldKind>,

    /// Места, ждущие своего такта.
    waiting: Vec<SavedTick>,

    /// Начатые ходы поршней: где поршень, выдвигается ли и куда смотрит.
    moving: Vec<((i32, i32, i32), bool, u8)>,

    /// Что едет: у какого поршня, куда встанет и что это за блок.
    cargo: Vec<SavedCargo>,

    /// Найденная когда-то точка появления.
    spawn: Option<(f64, f64, f64)>,

    /// Бой с драконом (у Края).
    fight: crate::dragon_fight::Fight,

    /// Погода.
    weather: crate::weather::Weather,

    /// Файла мира не было вовсе: мир новый.
    new: bool,
}

/// Прежняя копия файла мира — как у оригинала: перед каждой записью
/// нынешний level.dat становится ею.
const LEVEL_OLD: &str = "level.dat_old";

/// Куда сперва пишется новый файл мира: целиком и со сбросом на диск, и
/// только потом он встаёт на место level.dat.
const LEVEL_NEW: &str = "level.dat_new";

/// Куда откладывается испорченный level.dat, если мир прочитан по копии.
const LEVEL_CORRUPT: &str = "level.dat_corrupt";

/// Записывает файл в папке мира целиком или никак: новое пишется рядом,
/// сбрасывается на диск и встаёт на место прежнего.
fn write_atomically(directory: &Path, name: &str, text: &str) -> io::Result<()> {
    let target = directory.join(name);
    let fresh = directory.join(format!("{}_new", name));

    {
        let mut file = File::create(&fresh)?;

        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }

    fs::rename(&fresh, &target)?;
    region::sync_directory(directory);

    Ok(())
}

/// Записывает файл мира так, чтобы при сбое посреди записи на диске
/// остался целый файл — новый или прежний.
///
/// Новое содержимое пишется рядом и сбрасывается на диск; нынешний level.dat
/// переименовывается в level.dat_old, новый — в level.dat, и сбрасывается
/// сам каталог. Прервись это между переименованиями — останется
/// level.dat_old, и мир прочитается по нему (`load_level`).
fn write_level(directory: &Path, text: &str) -> io::Result<()> {
    let target = directory.join(LEVEL);
    let fresh = directory.join(LEVEL_NEW);

    {
        let mut file = File::create(&fresh)?;

        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }

    if target.exists() {
        fs::rename(&target, directory.join(LEVEL_OLD))?;
    }

    fs::rename(&fresh, &target)?;
    region::sync_directory(directory);

    Ok(())
}

/// Читает файл мира, а если он испорчен или пропал — его прежнюю копию.
///
/// Если не читается ни то ни другое, а регионы в папке мира есть — это
/// ошибка, и сервер не должен стартовать: иначе мир получил бы новое
/// случайное семя, и новые чанки не сошлись бы со старыми.
fn load_level(directory: &Path) -> io::Result<Level> {
    let main = read_level(&directory.join(LEVEL));

    if let Ok(Some(level)) = main {
        return Ok(level);
    }

    let old = read_level(&directory.join(LEVEL_OLD));

    if let Ok(Some(level)) = old {
        match &main {
            Err(why) => {
                // Испорченный файл откладываем: следующая запись иначе
                // сделала бы его копией и затёрла бы целую.
                let aside = directory.join(LEVEL_CORRUPT);
                let _ = fs::rename(directory.join(LEVEL), &aside);

                log_warn!("{} не читается ({}), мир прочитан по {}; испорченный отложен в {}", LEVEL, why, LEVEL_OLD, aside.display());
            }
            _ => log_warn!("{} нет, мир прочитан по {}", LEVEL, LEVEL_OLD),
        }

        return Ok(level);
    }

    let broken = match (&main, &old) {
        (Err(why), _) | (_, Err(why)) => Some(why.clone()),
        _ => None,
    };

    if has_regions(&directory.join(REGIONS)) {
        return Err(io::Error::other(format!(
            "в {} есть регионы, но файл мира не читается ({}): ни {}, ни {}. \
             Сервер не запущен, чтобы не выдумать миру новое семя. Восстановите {} \
             из копии или запишите в него строку «seed = <семя мира>»",
            directory.display(),
            broken.as_deref().unwrap_or("его нет"),
            LEVEL,
            LEVEL_OLD,
            LEVEL
        )));
    }

    if let Some(why) = broken {
        log_warn!("{} не читается ({}), но регионов нет — мир начинается заново", LEVEL, why);
    }

    Ok(Level::fresh())
}

/// Лежат ли в папке регионов записанные чанки.
fn has_regions(regions: &Path) -> bool {
    let Ok(entries) = fs::read_dir(regions) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry.path().extension().is_some_and(|extension| extension == "mcs" || extension == "rcw")
            && entry.metadata().is_ok_and(|meta| meta.len() > 0)
    })
}

/// Берёт замок папки мира: файл `session.lock`, запертый `flock`. Замок
/// держится, пока открыт файл, и отпускается сам — даже если сервер упал.
#[cfg(unix)]
fn lock_session(directory: &Path) -> io::Result<File> {
    use std::os::unix::io::AsRawFd;

    let path = directory.join("session.lock");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;

    // SAFETY: дескриптор принадлежит открытому здесь файлу и жив, пока
    // жив `file`.
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

    if locked != 0 {
        let error = io::Error::last_os_error();

        if error.kind() == io::ErrorKind::WouldBlock {
            let holder = fs::read_to_string(&path).unwrap_or_default();

            return Err(io::Error::other(format!(
                "папка мира {} уже занята другим сервером (session.lock{}). Два сервера на одной папке испортили бы мир",
                directory.display(),
                match holder.trim() {
                    "" => String::new(),
                    pid => format!(", процесс {}", pid),
                }
            )));
        }

        return Err(error);
    }

    // Кто держит папку — для того, кто увидит отказ.
    file.set_len(0)?;
    file.write_all(std::process::id().to_string().as_bytes())?;

    Ok(file)
}

#[cfg(not(unix))]
fn lock_session(directory: &Path) -> io::Result<File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("session.lock"))
}

impl Level {
    /// Сведения нового мира: ему ещё ничего не досталось.
    fn fresh() -> Level {
        Level {
            age: 0,
            time_offset: 0,
            seed: None,
            kind: None,
            waiting: Vec::new(),
            moving: Vec::new(),
            cargo: Vec::new(),
            spawn: None,
            fight: crate::dragon_fight::Fight::default(),
            weather: crate::weather::Weather::default(),
            new: true,
        }
    }
}

/// Читает файл мира. `Ok(None)` — файла нет. Ошибка — он есть, но не
/// читается или в нём нет семени (так выглядит оборванная запись).
fn read_level(path: &Path) -> Result<Option<Level>, String> {
    let mut level = Level::fresh();

    level.new = false;

    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };

    for line in text.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };

        let value = value.trim();

        match name.trim() {
            "age" => level.age = value.parse().unwrap_or(0),
            "time-offset" => level.time_offset = value.parse().unwrap_or(0),
            "seed" => level.seed = value.parse().ok(),
            "type" => level.kind = Some(WorldKind::from_setting(value)),
            "spawn" => level.spawn = read_spawn(value),
            "dragon-fight" => level.fight.read_flags(value),
            "weather" => level.weather = crate::weather::Weather::read(value),
            "gateways" => level.fight.read_gateways(value),
            // Прежний вид: такты лежали в level.dat со сроком «сколько ждать».
            "tick" => {
                if let Some(waiting) = read_waiting(value) {
                    level.waiting.push(waiting);
                }
            }
            "moving" => {
                if let Some(moving) = read_moving(value) {
                    level.moving.push(moving);
                }
            }
            "cargo" => {
                if let Some(cargo) = read_cargo(value) {
                    level.cargo.push(cargo);
                }
            }
            _ => {}
        }
    }

    if level.seed.is_none() {
        return Err("в нём нет семени мира".to_string());
    }

    Ok(Some(level))
}

/// Дочитывает такты и ходы поршней из `ticks.dat`. Срок там — по возрасту
/// мира, в `Level` он кладётся как «сколько ждать» от записанного возраста.
fn read_ticks(path: &Path, level: &mut Level) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return,
        Err(e) => {
            log_error!("Не удалось прочитать {}: {} — ждущие такты потеряны", path.display(), e);
            return;
        }
    };

    for line in text.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };

        let value = value.trim();

        match name.trim() {
            "due" => {
                if let Some((place, kind, due, priority)) = read_waiting(value) {
                    level.waiting.push((place, kind, due.saturating_sub(level.age).max(1), priority));
                }
            }
            "moving" => {
                if let Some(moving) = read_moving(value) {
                    level.moving.push(moving);
                }
            }
            "cargo" => {
                if let Some(cargo) = read_cargo(value) {
                    level.cargo.push(cargo);
                }
            }
            _ => {}
        }
    }
}

/// Разбирает строку «x y z» точки появления.
fn read_spawn(value: &str) -> Option<(f64, f64, f64)> {
    let mut parts = value.split_whitespace().map(|part| part.parse::<f64>().ok().filter(|v| v.is_finite()));

    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// Разбирает строку «x y z выдвигается сторона».
fn read_moving(value: &str) -> Option<((i32, i32, i32), bool, u8)> {
    let mut parts = value.split_whitespace();

    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    let extending = parts.next()?.parse::<u8>().ok()? != 0;
    let facing = parts.next()?.parse().ok()?;

    Some(((x, y, z), extending, facing))
}

/// Разбирает строку «поршень x y z место x y z состояние».
fn read_cargo(value: &str) -> Option<SavedCargo> {
    let mut parts = value.split_whitespace();
    let mut number = || parts.next()?.parse::<i32>().ok();

    let piston = (number()?, number()?, number()?);
    let place = (number()?, number()?, number()?);
    let state = number()?;

    Some((piston, place, state))
}

/// Разбирает строку «x y z вид через-сколько приоритет».
fn read_waiting(value: &str) -> Option<SavedTick> {
    let mut parts = value.split_whitespace();

    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;

    let kind = match parts.next()? {
        "block" => TickKind::Block,
        "fluid" => TickKind::Fluid,
        _ => return None,
    };

    let delay = parts.next()?.parse().ok()?;
    let priority = parts.next().and_then(|value| value.parse().ok()).unwrap_or(0);

    Some(((x, y, z), kind, delay, priority))
}

/// Само место и шесть его соседей — в том порядке, в каком их всегда
/// пересчитывал `schedule_around`.
const AROUND: [(i32, i32, i32); 7] = [(0, 0, 0), (0, 1, 0), (0, -1, 0), (-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1)];

/// Какая жидкость в блоке — для такта жидкости: 0 — никакой, 1 — источник
/// воды (и залитый водой блок), 2 — текущая вода, 3 — источник лавы,
/// 4 — текущая лава.
fn fluid_type(state: i32) -> u8 {
    match fluids::fluid_at(state) {
        Some((fluids::Kind::Water, fluids::SOURCE)) => 1,
        Some((fluids::Kind::Water, _)) => 2,
        Some((fluids::Kind::Lava, fluids::SOURCE)) => 3,
        Some((fluids::Kind::Lava, _)) => 4,
        None if wet(state) => 1,
        None => 0,
    }
}

/// Есть ли в блоке жидкость: вода, лава или залитый водой блок.
fn wet(state: i32) -> bool {
    if state == AIR {
        return false;
    }

    fluids::fluid_at(state).is_some()
        || crate::blocks::orientation_of(state).and_then(|rule| rule.value(state, "waterlogged")) == Some("true")
}

/// Ключ чанка по мировым координатам блока.
fn chunk_key(x: i32, z: i32) -> (i32, i32) {
    (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
}

/// Попадает ли высота в границы мира.
fn in_bounds(y: i32) -> bool {
    (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y)
}

/// Читатель простых чисел из файла мира.
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// Забирает следующие N байт, сдвигая позицию чтения.
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let end = self.offset + N;

        let slice = self.bytes.get(self.offset..end).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "файл мира обрывается")
        })?;

        self.offset = end;

        Ok(slice.try_into().expect("длина среза уже проверена"))
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(self.take::<2>()?))
    }

    /// Следующий байт, не сдвигая позицию.
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    /// Забирает следующие `count` байт.
    fn slice(&mut self, count: usize) -> io::Result<&'a [u8]> {
        let end = self.offset.checked_add(count).ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "файл мира обрывается"))?;

        let slice = self.bytes.get(self.offset..end).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "файл мира обрывается")
        })?;

        self.offset = end;

        Ok(slice)
    }

    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(self.take::<4>()?))
    }

    /// Всё, что ещё не прочитано.
    fn rest(&self) -> &'a [u8] {
        self.bytes.get(self.offset..).unwrap_or_default()
    }
}

#[cfg(test)]
mod mushroom_tests {
    use super::mushroom_cap;

    /// Край стенки красного гриба: сосед только с юга и снизу пусто — поры
    /// там, где задано, и там, где сосед.
    #[test]
    fn mushroom_faces_follow_neighbours() {
        let state = mushroom_cap("red_mushroom_block[west=false,down=false]", |dx, dy, dz| (dx, dy, dz) == (0, 0, 1));
        let expected = crate::blocks::state_from_text(
            "red_mushroom_block[up=true,down=false,north=true,south=false,east=true,west=false]",
        );
        assert_eq!(state, expected);
    }

    /// Столбцы, которыми деревья решают, где им расти, — те же, что считает
    /// `column_at`: иначе соседние чанки решали бы про одно дерево по-разному
    /// и на стыке оставались бы обрубки.
    #[test]
    fn tree_columns_match_column_at() {
        use super::{Column, Generator, TreeColumns, CHUNK_SIZE};
        use crate::world::terrain::{self, Terrain};

        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let (chunk_x, chunk_z) = (-3, 5);
        let (from_x, from_z) = (chunk_x * CHUNK_SIZE - 1, chunk_z * CHUNK_SIZE - 1);
        let grid = generator.relief_grid(from_x, from_z, from_x + CHUNK_SIZE + 1, from_z + CHUNK_SIZE + 1);
        let mut columns: Vec<Column> = Vec::new();

        for x in from_x..from_x + CHUNK_SIZE + 2 {
            for z in from_z..from_z + CHUNK_SIZE + 2 {
                columns.push(generator.column_in(x, z, grid.as_ref()));
            }
        }

        let near = TreeColumns::new(&generator, chunk_x, chunk_z, &columns);
        let reach = terrain::TREE_REACH + 3;

        for x in chunk_x * CHUNK_SIZE - reach..(chunk_x + 1) * CHUNK_SIZE + reach {
            for z in chunk_z * CHUNK_SIZE - reach..(chunk_z + 1) * CHUNK_SIZE + reach {
                let (ours, alone) = (near.column(x, z), generator.column_at(x, z));

                assert_eq!((ours.height, ours.biome), (alone.height, alone.biome), "столбец ({}, {})", x, z);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wet_sponge_dries_only_in_the_nether() {
        for kind in [WorldKind::Flat, WorldKind::Nether, WorldKind::End] {
            let mut world = World::in_memory();
            world.kind = kind;
            let wet = crate::blocks::state_by_name("wet_sponge").unwrap();
            world.set_block(0, 10, 0, wet);
            crate::redstone::settle(&mut world);
            let expected = if kind == WorldKind::Nether { "sponge" } else { "wet_sponge" };
            assert_eq!(crate::blocks::block_at_state(world.get_block(0, 10, 0)), Some(expected));
        }
    }

    /// Чанк складывается сам, когда до него дошли: под ногами земля,
    /// над ней воздух.
    #[test]
    fn a_new_chunk_comes_with_ground() {
        let mut world = World::in_memory();

        // Пока чанка нет — там пусто.
        assert_eq!(world.get_block(100, flat::SURFACE - 1, 100), AIR);

        assert!(world.ensure(6, 6));

        let grass = crate::blocks::state_by_name("grass_block").unwrap();
        let bedrock = crate::blocks::state_by_name("bedrock").unwrap();

        // Верхний блок земли — трава, ходят по его верху.
        assert_eq!(world.get_block(100, flat::SURFACE - 1, 100), grass);
        assert_eq!(world.get_block(100, flat::SURFACE, 100), AIR);
        assert_eq!(world.get_block(100, flat::bottom(), 100), bedrock);

        // Земля есть по всей площади чанка, а не только в одном месте.
        assert_eq!(world.get_block(111, flat::SURFACE - 1, 97), grass);

        // Второй раз тот же чанк не складывается заново.
        assert!(!world.ensure(6, 6));
    }

    /// Упавший далеко под мир не роняет такт: дождь там не идёт (полный
    /// прогон чёрного ящика: игрок, падавший в пустоте под дождём, вызывал
    /// выход за границы чанка).
    #[test]
    fn rain_below_the_world_is_dry_and_does_not_panic() {
        let mut world = World::in_memory();
        assert!(world.ensure(0, 0));
        world.weather.rain_level = 1.0;

        assert!(!world.rain_at(1, -243_824, 1));
        assert!(!world.in_rain(1.5, -243_824.0, 1.5, 1.8));
    }

    /// Яйцо черепахи на песке: событие мира 2012/15 раньше самого блока
    /// (чёрный ящик turtle-egg-hatch); на камне события нет.
    #[test]
    fn a_turtle_egg_on_sand_announces_itself() {
        let mut world = World::in_memory();
        assert!(world.ensure(0, 0));
        let sand = crate::blocks::state_by_name("sand").unwrap();
        let egg = crate::blocks::state_from_text("turtle_egg[eggs=4,hatch=0]").unwrap();
        let y = flat::SURFACE;
        let start = world.watch_events(0);

        world.set_block(1, y, 1, sand);
        world.set_block(1, y + 1, 1, egg);
        world.set_block(3, y + 1, 3, egg);

        let (events, _) = world.events_since(start, 0);
        let levels: Vec<_> = events.iter().filter_map(|event| match event { WorldEvent::Level { x, y, z, effect, data, .. } => Some((*x, *y, *z, *effect, *data)), _ => None }).collect();

        assert_eq!(levels, vec![(1, y + 1, 1, 2012, 15)]);
    }

    /// Построенное игроком остаётся: земля складывается только там, где
    /// чанка ещё не было.
    #[test]
    fn building_survives_the_ground() {
        let mut world = World::in_memory();

        world.ensure(0, 0);
        world.set_block(1, flat::SURFACE, 1, 1);

        assert!(!world.ensure(0, 0));
        assert_eq!(world.get_block(1, flat::SURFACE, 1), 1);
    }

    /// Мир записывается по чанкам и читается обратно: поставленный блок
    /// переживает перезапуск, а нетронутая земля складывается заново.
    #[test]
    fn a_changed_chunk_is_read_back_from_its_region() {
        let directory = std::env::temp_dir().join("rustcraft_world_round_trip");
        let _ = fs::remove_dir_all(&directory);

        let stone = crate::blocks::state_by_name("stone").unwrap();

        {
            let mut world = World::open(&directory, Some(1), WorldKind::Flat).expect("открыть мир");

            world.ensure(0, 0);
            world.set_block(3, flat::SURFACE, 4, stone);
            world.save_if_needed();
        }

        let mut world =
            World::open(&directory, Some(1), WorldKind::Flat).expect("открыть мир снова");

        world.ensure(0, 0);
        assert_eq!(world.get_block(3, flat::SURFACE, 4), stone);

        // Земля на месте, и её не пришлось хранить целиком.
        let grass = crate::blocks::state_by_name("grass_block").unwrap();
        assert_eq!(world.get_block(3, flat::GROUND, 4), grass);
    }

    /// Чанк на диске занимает немного: земля записывается палитрой, а
    /// регион сжимает запись.
    #[test]
    fn a_flat_chunk_is_small_on_disk() {
        let bytes = region::compress(&Chunk::flat().to_bytes()).unwrap().len();

        assert!(bytes < 256, "чанк занял {} байт", bytes);

        // И читается обратно тем же.
        let read = Chunk::from_bytes(&Chunk::flat().to_bytes()).expect("прочитать");

        assert_eq!(read.block(5, flat::GROUND, 5), Chunk::flat().block(5, flat::GROUND, 5));
        assert_eq!(read.not_air(), Chunk::flat().not_air());
    }

    /// Особый динамит помнит свой вид и после записи чанка на диск; сменился
    /// блок — вид забыт. Чанк без хранилищ, но с особым динамитом, тоже
    /// читается.
    #[test]
    fn custom_tnt_survives_the_disk() {
        let tnt = crate::blocks::state_by_name("tnt").expect("динамит");
        let mega = crate::custom_tnt::by_name("mega").expect("mega").id;
        let mut chunk = Chunk::flat();

        chunk.set(3, 70, 4, tnt);
        chunk.custom_tnt.push(((3, 70, 4), mega));
        chunk.set(5, -60, 6, tnt);

        let read = Chunk::from_bytes(&chunk.to_bytes()).expect("прочитать");

        assert_eq!(read.custom_tnt, vec![((3, 70, 4), mega)]);
        assert!(read.containers.is_empty());

        // Сменился блок — вид забыт.
        let mut read = read;
        read.set(3, 70, 4, AIR);
        assert!(read.custom_tnt.is_empty());

        // Через мир: поставленный особый динамит виден по месту.
        let mut world = World::in_memory();
        world.set_block_silently(17, 64, -3, tnt);
        world.set_custom_tnt((17, 64, -3), Some(mega));
        assert_eq!(world.custom_tnt_at((17, 64, -3)), Some(mega));
        world.set_block(17, 64, -3, AIR);
        assert_eq!(world.custom_tnt_at((17, 64, -3)), None);
    }

    /// Объёмные биомы записываются и читаются обратно клетка в клетку, и
    /// после сжатия они почти ничего не стоят.
    #[test]
    fn volume_biomes_survive_the_disk() {
        let mut chunk = Chunk::flat();
        let plain = region::compress(&chunk.to_bytes()).unwrap().len();
        let lush = terrain::Biome::LushCaves.number() as BiomeId;
        let deep = terrain::Biome::DeepDark.number() as BiomeId;

        for layer in 0..BIOME_LAYERS {
            for cell in 0..BIOME_LAYER {
                chunk.biomes[layer * BIOME_LAYER + cell] = match layer {
                    0..=10 => deep,
                    11..=30 if cell % 3 == 0 => lush,
                    _ => cell as BiomeId,
                };
            }
        }

        let bytes = chunk.to_bytes();
        let read = Chunk::from_bytes(&bytes).expect("прочитать");

        assert_eq!(bytes[0], CHUNK_VERSION);
        assert_eq!(read.biomes, chunk.biomes);
        assert_eq!(read.not_air(), chunk.not_air());
        assert!(plain < 256, "ровный чанк занял {} байт", plain);
    }

    /// Чанки прежних версий читаются: их биом был один на столбец клетки,
    /// теперь он растягивается на всю высоту.
    #[test]
    fn older_chunks_are_still_read() {
        let stone = crate::blocks::state_by_name("stone").unwrap();

        for version in [CHUNK_VERSION_WIDE, CHUNK_VERSION_FLAT_BIOMES] {
            let mut bytes = vec![version];

            for section in 0..SECTIONS {
                if section != 0 {
                    bytes.push(0);
                    continue;
                }

                // Нижняя секция целиком из камня: одна запись.
                bytes.push(1);
                bytes.extend_from_slice(&1u16.to_be_bytes());

                if version == CHUNK_VERSION_WIDE {
                    bytes.extend_from_slice(&stone.to_be_bytes());
                } else {
                    bytes.extend_from_slice(&(stone as u16).to_be_bytes());
                }

                bytes.extend_from_slice(&(SECTION_BLOCKS as u16).to_be_bytes());
            }

            let without_biomes = bytes.clone();

            for cell in 0..BIOME_LAYER {
                bytes.extend_from_slice(&(cell as u16 + 3).to_be_bytes());
            }

            let chunk = Chunk::from_legacy(&bytes).expect("прочитать прежний чанк");

            assert_eq!(chunk.block(7, MIN_Y + 15, 9), stone);
            assert_eq!(chunk.block(7, MIN_Y + 16, 9), AIR);

            for layer in 0..BIOME_LAYERS {
                for cell in 0..BIOME_LAYER {
                    assert_eq!(chunk.biomes[layer * BIOME_LAYER + cell], cell as BiomeId + 3);
                }
            }

            // Совсем старый чанк без биомов — первый биом реестра.
            let chunk = Chunk::from_legacy(&without_biomes).expect("прочитать чанк без биомов");

            assert!(chunk.biomes.iter().all(|biome| *biome == 0));

            // Прочитанный старый чанк записывается уже новой версией.
            assert_eq!(chunk.to_bytes()[0], CHUNK_VERSION);
        }
    }

    /// Испорченная запись биомов — не повод читать чанк как попало.
    #[test]
    fn broken_biomes_are_refused() {
        let mut bytes = Chunk::flat().to_bytes();

        bytes.truncate(bytes.len() - 2);
        assert!(Chunk::from_bytes(&bytes).is_none());

        // Вид контейнера, которого нет.
        let mut bytes = Chunk::flat().to_bytes();
        let length = bytes.len();

        bytes[length - 3] = 9;
        assert!(Chunk::from_bytes(&bytes).is_none());
    }

    /// Воздушные секции не занимают памяти: пустой чанк почти ничего не стоит.
    #[test]
    fn air_costs_nothing() {
        let chunk = Chunk::new();

        assert!(chunk.sections.iter().all(|section| section.is_none()));

        // У ровной земли заняты две секции из двадцати четырёх: слои земли
        // приходятся на границу между ними.
        let flat = Chunk::flat();
        let filled = flat.sections.iter().flatten().count();

        assert_eq!(filled, 2);
    }

    /// Чанк, до которого никому нет дела, выгружается из памяти, но не
    /// теряется: изменённое перед этим записано, и читается обратно.
    #[test]
    fn far_chunks_are_unloaded_but_not_lost() {
        let directory = std::env::temp_dir().join("rustcraft_world_unload");
        let _ = fs::remove_dir_all(&directory);

        let stone = crate::blocks::state_by_name("stone").unwrap();

        let mut world = World::open(&directory, Some(1), WorldKind::Flat).expect("открыть мир");

        world.ensure(0, 0);
        world.ensure(10, 10);
        world.set_block(160, flat::SURFACE, 160, stone); // это чанк 10 10

        // Оставляем только начальный чанк.
        assert_eq!(world.unload_far(|x, z| (x, z) == (0, 0)), 1);

        // И убеждаемся, что дальний вернулся с диска целым.
        world.ensure(10, 10);
        assert_eq!(world.get_block(160, flat::SURFACE, 160), stone);
    }

    /// За такт разбирается не больше положенного, а лишнее ждёт следующего
    /// такта, а не пропадает.
    #[test]
    fn work_beyond_the_limit_waits_instead_of_being_lost() {
        let mut world = World::in_memory();

        // Такты срабатывают только в загруженном чанке.
        world.ensure(0, 0);

        for x in 0..5 {
            world.schedule_once(x, 1, 0, 1, 0);
        }

        let first = world.advance(2);
        assert_eq!(first.len(), 2, "за такт взято больше положенного");

        let second = world.advance(2);
        assert_eq!(second.len(), 2, "отложенное не вернулось");

        let third = world.advance(2);
        assert_eq!(third.len(), 1, "последнее место потерялось");

        // И больше ничего не осталось.
        assert!(world.advance(2).is_empty());
    }


    /// За границами мира блоков нет, и спрашивать о них можно спокойно:
    /// поршень у потолка обращается именно туда.
    #[test]
    fn asking_beyond_the_world_is_safe() {
        let mut world = World::in_memory();

        world.set_block(0, 1, 0, 1);

        assert_eq!(world.get_block(0, MIN_Y + WORLD_HEIGHT, 0), AIR);
        assert_eq!(world.get_block(0, MIN_Y - 1, 0), AIR);
        assert_eq!(world.get_block(0, 100_000, 0), AIR);
        assert_eq!(world.get_block(0, -100_000, 0), AIR);

        // И поставить туда ничего нельзя.
        assert!(!world.set_block(0, MIN_Y + WORLD_HEIGHT, 0, 1));
    }


    /// Часы мира переживают перезапуск: возраст и смещение записываются
    /// в файл мира и читаются обратно.
    #[test]
    fn the_clock_survives_a_restart() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-level-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);

        {
            let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

            world.advance(16);
            world.set_time_of_day(13_000);
            world.save_if_needed();

            assert_eq!(world.time_of_day(), 13_000);
        }

        // Открываем заново — время на месте.
        let again = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся снова");

        assert_eq!(again.time_of_day(), 13_000, "часы сбросились");
        assert_eq!(again.tick(), 1, "возраст мира потерялся");

        let _ = fs::remove_dir_all(&path);
    }


    /// Запланированные такты переживают перезапуск: без них после
    /// перезагрузки встали бы часы на факелах и повторители в середине
    /// задержки.
    #[test]
    fn waiting_places_survive_a_restart() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-ticks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);

        {
            let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

            world.ensure(0, 0);
            world.schedule_once(1, 2, 3, 4, -3);
            world.schedule(5, 6, 7, 2);
            world.save_if_needed();
        }

        let mut again = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся снова");
        again.ensure(0, 0);

        // Через четыре такта должно всплыть место повторителя, через два —
        // место жидкости.
        let mut seen = Vec::new();

        for _ in 0..5 {
            for (place, kind) in again.advance(crate::tick::PER_TICK) {
                seen.push((place, kind));
            }
        }

        assert!(
            seen.contains(&((1, 2, 3), TickKind::Block)),
            "блочный такт не вернулся: {seen:?}"
        );
        assert!(
            seen.contains(&((5, 6, 7), TickKind::Fluid)),
            "жидкостный такт не вернулся: {seen:?}"
        );

        let _ = fs::remove_dir_all(&path);
    }


    /// Такт в выгруженном чанке не пропадает: он ждёт, пока чанк прочитают
    /// снова, и тогда срабатывает. Иначе схема в ушедшем из виду чанке
    /// умирала бы, как в тикете MC-711.
    #[test]
    fn a_tick_waits_for_its_chunk_to_come_back() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-parked-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);

        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");
        world.ensure(0, 0);
        world.ensure(10, 10);

        // Такт в дальнем чанке, а сам чанк тут же выгружаем.
        world.schedule_once(165, 5, 165, 2, 0);
        world.unload_far(|x, z| (x, z) == (0, 0));

        // Срок прошёл, но чанка нет — такт не должен ни сработать, ни пропасть.
        let mut fired = Vec::new();

        for _ in 0..4 {
            fired.extend(world.advance(crate::tick::PER_TICK));
        }

        assert!(fired.is_empty(), "такт сработал в выгруженном чанке: {fired:?}");

        // Чанк вернулся — такт срабатывает на ближайшем такте.
        world.ensure(10, 10);
        fired.extend(world.advance(crate::tick::PER_TICK));
        fired.extend(world.advance(crate::tick::PER_TICK));

        assert!(
            fired.contains(&((165, 5, 165), TickKind::Block)),
            "такт так и не сработал после возвращения чанка: {fired:?}"
        );

        let _ = fs::remove_dir_all(&path);
    }

}

#[cfg(test)]
mod safety_tests {
    use super::*;

    /// Своя папка для проверки — в каталоге временных файлов.
    fn directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mcsheriffanya-safety-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&path);
        path
    }

    /// Запись в незагруженный чанк не заводит его пустым: иначе пустой чанк
    /// затёр бы настоящий на диске.
    #[test]
    fn writing_into_an_unloaded_chunk_does_not_create_it() {
        let mut world = World::in_memory();
        world.create_missing = false;

        let stone = crate::blocks::state_by_name("stone").unwrap();

        assert!(!world.set_block(5, 10, 5, stone));
        assert!(!world.set_block_silently(5, 10, 5, stone));
        assert!(!world.set_block_quietly(5, 10, 5, stone));
        assert!(!world.has_chunk(0, 0), "чанк завёлся от записи");
        assert_eq!(world.get_block_loaded(5, 10, 5), None);

        // В загруженный — пишется.
        world.ensure(0, 0);
        assert!(world.set_block(5, 10, 5, stone));
        assert_eq!(world.get_block_loaded(5, 10, 5), Some(stone));
    }

    /// Возраст мира сам по себе level.dat каждую секунду не переписывает:
    /// файл пишется, когда сменилось что-то ещё, и при остановке.
    #[test]
    fn the_level_file_is_not_rewritten_for_the_age_alone() {
        let path = directory("level-pause");
        let mut world = World::open(&path, Some(7), WorldKind::Flat).expect("мир открылся");

        world.save_if_needed();
        let written = fs::metadata(path.join(LEVEL)).and_then(|meta| meta.modified()).expect("файл есть");

        world.advance(1);
        world.save_if_needed();
        assert_eq!(fs::metadata(path.join(LEVEL)).and_then(|meta| meta.modified()).expect("файл есть"), written);

        world.set_time_of_day(13_000);
        world.save_if_needed();
        assert!(read_level(&path.join(LEVEL)).expect("читается").is_some_and(|level| level.time_offset != 0), "перевод часов не записан");
    }

    /// Испорченный level.dat не даёт миру новое семя: мир читается по
    /// level.dat_old, а испорченный откладывается.
    #[test]
    fn a_broken_level_file_falls_back_to_the_old_copy() {
        let path = directory("level-old");

        {
            let mut world = World::open(&path, Some(7), WorldKind::Flat).expect("мир открылся");

            world.ensure(0, 0);
            world.save_all();
            world.save_all();
        }

        assert!(path.join(LEVEL_OLD).exists(), "копии нет");

        // Оборванная запись: файл есть, семени в нём нет.
        fs::write(path.join(LEVEL), "age = 5\nti").unwrap();

        let world = World::open(&path, Some(99), WorldKind::Flat).expect("мир открылся по копии");

        assert_eq!(world.seed, 7, "семя подменено");
        assert!(path.join(LEVEL_CORRUPT).exists(), "испорченный не отложен");
        drop(world);

        // Нет ни файла, ни копии, а регионы есть — мир не открывается.
        let _ = fs::remove_file(path.join(LEVEL));
        let _ = fs::remove_file(path.join(LEVEL_OLD));
        fs::write(path.join(LEVEL), [0xFF, 0xFE, 0x00]).unwrap();

        assert!(World::open(&path, Some(99), WorldKind::Flat).is_err(), "мир открылся с выдуманным семенем");

        // Без регионов — это просто новый мир.
        fs::remove_dir_all(path.join(REGIONS)).unwrap();
        let world = World::open(&path, Some(99), WorldKind::Flat).expect("новый мир");
        assert_eq!(world.seed, 99);
        drop(world);

        let _ = fs::remove_dir_all(&path);
    }

    /// Чанк, который не удалось записать, не теряется: выгруженный, он
    /// читается из очереди записи, а на диск ложится, как только можно.
    #[test]
    fn unsaved_chunks_are_not_lost() {
        let path = directory("unload-dirty");
        let stone = crate::blocks::state_by_name("stone").unwrap();
        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        world.ensure(0, 0);
        world.ensure(10, 10);
        world.save_all();
        world.set_block(160, flat::SURFACE, 160, stone); // чанк 10 10

        // Файл региона подменён каталогом — записать его нельзя.
        let file = region::path_for(&path.join(REGIONS), 10, 10);
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();

        // Чанк выгружается, но его байты ждут у записи и читаются оттуда.
        world.unload_far(|_, _| false);
        assert!(!world.has_chunk(10, 10));
        world.ensure(10, 10);
        assert_eq!(world.get_block(160, flat::SURFACE, 160), stone, "изменение потерялось");

        // Запись снова возможна — застрявшее ложится на диск со следующей.
        fs::remove_dir(&file).unwrap();
        world.save_all();
        drop(world);

        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");
        world.ensure(10, 10);
        assert_eq!(world.get_block(160, flat::SURFACE, 160), stone, "не дошло до диска");

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }

    /// Рамка для предмета ложится в файл вместе с чанком и возвращается с ним,
    /// а убранная — нет; и в мировых координатах, а не в местных.
    #[test]
    fn frames_survive_the_disk() {
        let path = directory("frames");
        let stone_item = crate::blocks::item_named("stone").expect("камень");
        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        world.ensure(-1, 2);

        let mut kept = crate::frame::Frame::new(1, (-3, flat::SURFACE + 1, 40), crate::frame::EAST, true);

        kept.item = Some(crate::inventory::Stack::new(stone_item, 1));
        kept.rotation = 6;

        let gone = crate::frame::Frame::new(2, (-4, flat::SURFACE + 1, 41), crate::frame::UP, false);

        world.store_frame(&kept);
        world.store_frame(&gone);
        world.forget_frame(gone.pos, gone.facing);
        world.save_all();
        drop(world);

        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        assert!(world.take_loaded_frames().is_empty(), "чанк ещё не загружен");
        world.ensure(-1, 2);

        let loaded = world.take_loaded_frames();

        assert_eq!(loaded.len(), 1);
        assert_eq!((loaded[0].pos, loaded[0].facing, loaded[0].glow), (kept.pos, kept.facing, true));
        assert_eq!((loaded[0].item, loaded[0].rotation), (kept.item, 6));

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }

    /// Мобы ложатся в файл вместе со своим чанком (по тому месту, где стоят
    /// при записи) и возвращаются с ним; вышедшие из мира не возвращаются.
    /// Новый чанк один раз попадает в список «только что созданных».
    #[test]
    fn mobs_survive_the_disk() {
        let path = directory("mobs");
        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        world.ensure(0, 3);
        assert_eq!(world.take_new_chunks(), vec![(0, 3)], "созданный чанк отмечен");
        assert!(world.take_new_chunks().is_empty(), "отметка одноразовая");

        let mut cow = crate::mob::Mob::new(1, crate::mob::Kind::Cow, (5.5, f64::from(flat::SURFACE + 1), 50.5));

        cow.age = -900;
        cow.vitals.health = 4.0;

        let mut dead = crate::mob::Mob::new(2, crate::mob::Kind::Pig, (6.5, f64::from(flat::SURFACE + 1), 50.5));

        dead.die();

        // Третий стоит в чанке, которого в памяти нет: не записывается.
        let far = crate::mob::Mob::new(3, crate::mob::Kind::Sheep, (500.5, f64::from(flat::SURFACE + 1), 500.5));

        world.store_mobs(&[cow, dead, far]);
        world.save_all();
        drop(world);

        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        assert!(world.take_loaded_mobs().is_empty(), "чанк ещё не загружен");
        world.ensure(0, 3);

        let loaded = world.take_loaded_mobs();

        assert_eq!(loaded.len(), 1, "погибший и дальний не записались");
        assert_eq!(loaded[0].kind, crate::mob::Kind::Cow);
        assert_eq!((loaded[0].x, loaded[0].z, loaded[0].age, loaded[0].vitals.health), (5.5, 50.5, -900, 4.0));
        assert!(world.take_new_chunks().is_empty(), "прочитанный с диска чанк не новый");

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }

    /// Сложенный заново чанк записывается на диск, даже если его не трогали.
    #[test]
    fn generated_chunks_are_saved() {
        let path = directory("fresh");
        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        world.ensure(3, -4);
        world.save_if_needed();

        assert!(matches!(
            region::read_stored(&path.join(REGIONS), 3, -4),
            Ok(region::Stored::Found(_))
        ));

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }

    /// Чанк, приготовленный в стороне до выгрузки своего места, не ложится
    /// поверх того, что за это время изменили и записали.
    #[test]
    fn a_stale_ready_chunk_is_not_accepted() {
        let path = directory("stale");
        let stone = crate::blocks::state_by_name("stone").unwrap();
        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        let source = world.source();
        let ready = source.take(5, 5);

        world.ensure(5, 5);
        world.set_block(80, flat::SURFACE, 80, stone);
        world.unload_far(|_, _| false);
        assert!(!world.has_chunk(5, 5));

        world.accept(5, 5, ready);

        assert_eq!(world.get_block(80, flat::SURFACE, 80), stone, "устаревший чанк лёг поверх нового");

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }

    /// Испорченная запись чанка не пропадает: её байты откладываются, и
    /// только потом чанк складывается заново.
    #[test]
    fn a_corrupt_chunk_is_set_aside() {
        let path = directory("corrupt");
        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");
        let regions = path.join(REGIONS);

        region::write(&regions, 2, 2, &[CHUNK_VERSION, 1, 0xFF]).unwrap();

        assert!(world.ensure(2, 2), "чанк не сложился заново");

        let aside: Vec<_> = fs::read_dir(regions.join("corrupt")).expect("папка отложенного").flatten().collect();

        assert_eq!(aside.len(), 1);
        assert_eq!(fs::read(aside[0].path()).unwrap(), vec![CHUNK_VERSION, 1, 0xFF]);

        drop(world);
        let _ = fs::remove_dir_all(&path);
    }

    /// Испорченные номера в записи чанка не роняют сервер, а дают отказ.
    #[test]
    fn nonsense_in_a_chunk_is_refused() {
        // Состояния нет в таблице блоков.
        let mut unknown = vec![CHUNK_VERSION, 1];
        unknown.extend_from_slice(&u16::MAX.to_be_bytes());
        assert!(Chunk::from_bytes(&unknown).is_none());

        // Палитра из одного значения (для этого есть свой вид), из большего
        // числа значений, чем клеток, и с номером за пределами палитры.
        for count in [1u16, 5000] {
            let mut palette = vec![CHUNK_VERSION, 2];
            palette.extend_from_slice(&count.to_be_bytes());
            palette.extend(std::iter::repeat_n(0u8, 20_000));
            assert!(Chunk::from_bytes(&palette).is_none(), "палитра из {}", count);
        }

        let mut outside = vec![CHUNK_VERSION, 2];
        outside.extend_from_slice(&3u16.to_be_bytes());

        for state in [0u16, 1, 2] {
            outside.extend_from_slice(&state.to_be_bytes());
        }

        // Ширина номера — 2 бита; номер 3 в палитре из трёх значений — вне её.
        outside.extend(std::iter::repeat_n(0xFFu8, SECTION_BLOCKS / 4));
        assert!(Chunk::from_bytes(&outside).is_none());

        // Записи не хватает на секцию.
        let mut short = vec![CHUNK_VERSION, 2];
        short.extend_from_slice(&2u16.to_be_bytes());
        short.extend_from_slice(&[0, 0, 0, 1, 0, 0]);
        assert!(Chunk::from_bytes(&short).is_none());

        // Прежние версии записи через `from_bytes` не читаются.
        assert!(Chunk::from_bytes(&[3, 0]).is_none());

        // Биома нет в реестре.
        let mut bytes = Chunk::flat().to_bytes();
        let length = bytes.len();
        bytes[length - 2..].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(Chunk::from_bytes(&bytes).is_none());
    }

    /// Вторая копия сервера ту же папку мира не открывает.
    #[test]
    fn a_world_folder_is_held_by_one_server() {
        let path = directory("session");
        let first = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

        assert!(World::open(&path, Some(1), WorldKind::Flat).is_err(), "папку открыли дважды");

        drop(first);
        World::open(&path, Some(1), WorldKind::Flat).expect("после закрытия папка свободна");

        let _ = fs::remove_dir_all(&path);
    }

    /// Блочный и жидкостный такты одного места пишутся каждый со своим
    /// приоритетом, а точка появления запоминается в level.dat.
    #[test]
    fn saved_ticks_keep_their_own_priority() {
        let path = directory("priority");
        let spawn;

        {
            let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

            world.ensure(0, 0);
            world.schedule_block(1, 2, 3, 4, -3);
            world.schedule(1, 2, 3, 4);
            spawn = world.spawn_position();
            world.save_if_needed();
        }

        let text = fs::read_to_string(path.join(LEVEL)).unwrap();
        let ticks = fs::read_to_string(path.join(TICKS)).unwrap();

        // Срок — по возрасту мира: мир не прожил ни такта, срок равен задержке.
        assert!(ticks.contains("due = 1 2 3 block 4 -3"), "{}", ticks);
        assert!(ticks.contains("due = 1 2 3 fluid 4 1"), "{}", ticks);
        assert!(!text.contains("tick = "), "такты остались в level.dat: {}", text);
        assert!(text.contains("spawn = "), "{}", text);

        // Такты переживают перезапуск: снова открытый мир ждёт их же.
        {
            let world = World::open(&path, None, WorldKind::Flat).expect("мир открылся снова");
            assert!(world.waiting.contains_key(&((1, 2, 3), TickKind::Block)));
            assert!(world.waiting.contains_key(&((1, 2, 3), TickKind::Fluid)));
        }

        // Прежний вид (такты в level.dat) по-прежнему читается.
        fs::remove_file(path.join(TICKS)).unwrap();
        fs::write(path.join(LEVEL), format!("{}tick = 5 6 7 block 3 0\n", text)).unwrap();
        {
            let world = World::open(&path, None, WorldKind::Flat).expect("мир открылся по старому виду");
            assert!(world.waiting.contains_key(&((5, 6, 7), TickKind::Block)));
        }

        let level = read_level(&path.join(LEVEL)).unwrap().unwrap();
        assert_eq!(level.spawn, Some(spawn));

        let _ = fs::remove_dir_all(&path);
    }

    /// Блок, поставленный вдали от воды, не заводит жидкостных тактов, а
    /// рядом с водой — заводит, причём самой воде: пустому месту такт не
    /// нужен, его зальёт сосед.
    #[test]
    fn only_places_near_fluids_are_scheduled() {
        let mut world = World::in_memory();
        let stone = crate::blocks::state_by_name("stone").unwrap();
        let water = fluids::state_of(fluids::Kind::Water, fluids::SOURCE);

        world.ensure(0, 0);
        world.set_block(4, 10, 4, stone);
        assert_eq!(world.scheduled_count(), 0, "такты вдали от воды");

        world.set_block(8, 10, 8, water);
        assert_eq!(world.scheduled_count(), 1, "вода не пустилась в такт");

        // Блок рядом с водой будит её саму, а через клетку — никого.
        let mut world = World::in_memory();

        world.ensure(0, 0);
        world.set_block_silently(8, 10, 8, water);
        world.set_block(10, 10, 8, stone);
        assert_eq!(world.scheduled_count(), 0, "вода через клетку разбужена");

        world.set_block(9, 10, 8, stone);
        assert_eq!(world.scheduled_count(), 1, "вода рядом не разбужена");
    }
}

#[cfg(test)]
mod spawn_tests {
    use super::*;

    /// Точка появления — на суше и над водой: в обычном мире начало координат
    /// запросто оказывается морем.
    #[test]
    fn the_spawn_point_stands_on_land() {
        let directory = std::env::temp_dir().join("mc_spawn_point");
        let _ = fs::remove_dir_all(&directory);

        let world = World::open(
            &directory,
            Some(100_554_032_945_340),
            WorldKind::SuperSmooth,
        )
        .expect("мир");
        let (x, y, z) = world.spawn_position();

        let column = world.generator.column_at(x as i32, z as i32);

        assert!(!column.biome.is_ocean(), "появление в море: {:?}", column.biome);
        assert!(y as i32 > terrain::SEA, "появление под водой: y = {}", y);
        assert_eq!(y as i32, column.height + 1, "не на поверхности");
    }
}

#[cfg(test)]
mod speed {
    use super::*;

    /// Сколько времени уходит на чанк. Не проверка, а замер: запускается
    /// вручную, `cargo test chunk_speed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn chunk_speed() {
        for style in [terrain::Style::Vanilla, terrain::Style::Smooth] {
            let generator = Generator::Normal(Terrain::new(100_554_032_945_340, style));

            let started = std::time::Instant::now();
            let count = 64;

            for i in 0..count {
                Chunk::generated(&generator, i % 8, i / 8);
            }

            let each = started.elapsed() / count as u32;

            println!(
                "{:?} — чанк: {:?}, на 441 чанк ушло бы {:?}",
                style,
                each,
                each * 441
            );
        }
    }

    /// Сколько времени уходит на каждый этап генерации чанка «Обычного»
    /// мира: `cargo test --release chunk_phases -- --ignored --nocapture`.
    /// Этапы после рельефа прогоняются на готовом чанке ещё раз и меряются
    /// по отдельности; рельеф — это всё остальное время.
    #[test]
    #[ignore]
    fn chunk_phases() {
        use std::time::{Duration, Instant};

        let generator =
            Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let count = 64;
        let mut total = Duration::ZERO;
        let mut phases = [Duration::ZERO; 7];
        let mut nests = Duration::ZERO;
        let mut ore_biomes = Duration::ZERO;
        let mut ready = HashMap::new();

        for i in 0..count {
            let (chunk_x, chunk_z) = (i % 8, i / 8);
            let started = Instant::now();
            let mut chunk = Chunk::generated(&generator, chunk_x, chunk_z);

            total += started.elapsed();

            let highest = chunk.highest();
            let mut timed = |index: usize, work: &mut dyn FnMut(&mut Chunk)| {
                let started = Instant::now();

                work(&mut chunk);
                phases[index] += started.elapsed();
            };

            let (from_x, from_z) = (chunk_x * CHUNK_SIZE - 1, chunk_z * CHUNK_SIZE - 1);

            timed(0, &mut |_| {
                let grid = generator.relief_grid(
                    from_x,
                    from_z,
                    from_x + CHUNK_SIZE + 1,
                    from_z + CHUNK_SIZE + 1,
                );

                for x in from_x..from_x + CHUNK_SIZE + 2 {
                    for z in from_z..from_z + CHUNK_SIZE + 2 {
                        std::hint::black_box(generator.column_in(x, z, grid.as_ref()));
                    }
                }
            });
            timed(1, &mut |chunk| generator.lay_ores(chunk, chunk_x, chunk_z));

            if let Generator::Normal(terrain) = &generator {
                let started = Instant::now();

                std::hint::black_box(terrain.ore_blocks_in(chunk_x, chunk_z, highest));
                nests += started.elapsed();

                let started = Instant::now();

                for from_x in chunk_x - 1..=chunk_x + 1 {
                    for from_z in chunk_z - 1..=chunk_z + 1 {
                        std::hint::black_box(terrain.column_at(from_x * 16 + 8, from_z * 16 + 8));
                    }
                }

                ore_biomes += started.elapsed();
            }
            timed(2, &mut |chunk| {
                underground::dig(&generator, chunk, chunk_x, chunk_z)
            });
            let grid = generator.relief_grid(
                from_x,
                from_z,
                from_x + CHUNK_SIZE + 1,
                from_z + CHUNK_SIZE + 1,
            );
            let mut columns = Vec::new();

            for x in from_x..from_x + CHUNK_SIZE + 2 {
                for z in from_z..from_z + CHUNK_SIZE + 2 {
                    columns.push(generator.column_in(x, z, grid.as_ref()));
                }
            }

            timed(3, &mut |chunk| {
                generator.grow_trees(chunk, chunk_x, chunk_z, &columns)
            });

            timed(4, &mut |chunk| {
                vegetation::decorate(&generator, chunk, chunk_x, chunk_z, &columns);
                ocean::decorate(&generator, chunk, chunk_x, chunk_z, &columns)
            });
            timed(5, &mut |chunk| {
                cave_biomes::decorate(&generator, chunk, chunk_x, chunk_z, &columns, grid.as_ref())
            });
            timed(6, &mut |chunk| chunk.light_up());

            ready.insert((chunk_x, chunk_z), chunk);
        }

        // Сшивка света с соседями — у чанков, у которых все восемь соседей
        // сложены, — и сколько памяти занимает свет.
        let mut world = World::in_memory();
        let light_bytes: usize = ready
            .values()
            .map(|chunk| chunk.light.as_ref().map_or(0, |(_, light)| light.bytes()))
            .sum();

        world.chunks = ready.into_iter().collect();

        let mut stitched = 0;
        let started = Instant::now();

        for chunk_x in 1..7 {
            for chunk_z in 1..7 {
                let job = world.light_job(chunk_x, chunk_z).expect("чанк");

                std::hint::black_box(job.finish());
                stitched += 1;
            }
        }

        let stitch = started.elapsed() / stitched;

        let each = |time: Duration| time / count as u32;
        let rest: Duration = phases.iter().sum();

        println!("весь чанк: {:?}", each(total));
        println!("  сетка и столбцы: {:?}", each(phases[0]));
        println!(
            "  блоки (пещеры, водоёмы, поверхность): {:?}",
            each(total.saturating_sub(rest))
        );
        println!(
            "  руда: {:?} (из них расчёт гнёзд {:?}, биомы соседних чанков {:?})",
            each(phases[1]),
            each(nests),
            each(ore_biomes)
        );
        println!("  подземелье: {:?}", each(phases[2]));
        println!("  деревья: {:?}", each(phases[3]));
        println!("  растительность и море: {:?}", each(phases[4]));
        println!("  пещерные биомы: {:?}", each(phases[5]));
        println!("  свет чанка: {:?}", each(phases[6]));
        println!("сшивка света с соседями перед отправкой: {:?}", stitch);
        println!(
            "память на свет: {} КБ на чанк (блоки — до {} КБ)",
            light_bytes / count as usize / 1024,
            SECTIONS as usize * SECTION_BLOCKS * 2 / 1024
        );
    }

    /// Растения на чанк по биомам — как tools/measure/plants_census.py
    /// считает у оригинала (биом — по середине чанка на поверхности,
    /// двухблочные — по нижней половине):
    /// `cargo test --release plants_census -- --ignored --nocapture`.
    /// На чём стоят стволы: ни одного дерева на камне, песке, гравии.
    /// `cargo test --release tree_ground_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn tree_ground_census() {
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let mut under: HashMap<&str, u64> = HashMap::new();
        let mut wrong = Vec::new();
        let mut other: HashMap<&str, u64> = HashMap::new();

        for chunk_x in (-400..400).step_by(16) {
            for chunk_z in (-400..400).step_by(16) {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        for y in MIN_Y + 1..MIN_Y + WORLD_HEIGHT {
                            let name = crate::blocks::block_at_state(chunk.block(x, y, z)).unwrap_or("");
                            let below = crate::blocks::block_at_state(chunk.block(x, y - 1, z)).unwrap_or("");

                            // Прочее вокруг деревьев: затопленная листва, снег на
                            // кронах, бледный мох, висящий ни на чём.
                            let above = crate::blocks::block_at_state(chunk.block(x, (y + 1).min(MIN_Y + WORLD_HEIGHT - 1), z)).unwrap_or("");
                            let wet = crate::blocks::orientation(name)
                                .and_then(|rule| rule.value(chunk.block(x, y, z), "waterlogged"))
                                .is_some_and(|value| value == "true");

                            if name.ends_with("_leaves") && wet {
                                *other.entry("лист в воде").or_default() += 1;
                            }

                            if name == "snow" && below.ends_with("_leaves") {
                                *other.entry("снег на листве").or_default() += 1;
                            }

                            if name == "pale_hanging_moss" && above == "air" {
                                *other.entry("мох ни на чём").or_default() += 1;
                            }

                            // Низ вертикального ствола: бревно над не-бревном и не-листвой.
                            let trunk = name.ends_with("_log") || name == "mushroom_stem";
                            let upright = crate::blocks::orientation(name)
                                .and_then(|rule| rule.value(chunk.block(x, y, z), "axis"))
                                .is_none_or(|axis| axis == "y");

                            if trunk && upright && !below.ends_with("_log") && !below.contains("leaves") && below != "mushroom_stem" && below != "air" {
                                *under.entry(below).or_default() += 1;

                                // Первые случаи на камне, песке и гравии — с местом
                                // и биомом, чтобы было с чего начать разбор.
                                let bare = below == "stone" || below.ends_with("_ore") || below.contains("sand") || below == "gravel" || below == "mossy_cobblestone";

                                if bare && wrong.len() < 30 {
                                    let (wx, wz) = (chunk_x * CHUNK_SIZE + x, chunk_z * CHUNK_SIZE + z);
                                    let column = generator.column_at(wx, wz);

                                    wrong.push(format!("{} на {} в ({}, {}, {}), {:?}, верх {}", name, below, wx, y, wz, column.biome, column.height));
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut rows: Vec<_> = under.into_iter().collect();
        rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        println!("под стволами: {:?}", rows);
        println!("прочее: {:?}", other);

        for line in wrong {
            println!("  {}", line);
        }
    }

    /// Плотная перепись деревьев вокруг точки: `AT="x z"` (блоки), радиус 6
    /// чанков. Для каждого низа ствола — блок под ним и сколько камня вокруг
    /// корня (8 соседей на уровне земли).
    /// `AT="1234 -787" cargo test --release trees_around -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn trees_around() {
        let at = std::env::var("AT").unwrap_or_else(|_| "0 0".into());
        let mut parts = at.split_whitespace().map(|v| v.parse::<i32>().expect("число"));
        let (bx, bz) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        let seed: i64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(100_554_032_945_340);
        let generator = Generator::Normal(Terrain::new(seed, terrain::Style::Vanilla));
        let (cx0, cz0) = (bx.div_euclid(CHUNK_SIZE), bz.div_euclid(CHUNK_SIZE));
        let mut chunks = HashMap::new();

        // WORLD — папка мира: тогда берутся записанные на диск чанки (то, что
        // видит игрок), а несложенные — пропускаются.
        let saved = std::env::var("WORLD").ok().map(|world| Path::new(&world).join("region"));
        let mut from_disk = 0;

        for cx in cx0 - 7..=cx0 + 7 {
            for cz in cz0 - 7..=cz0 + 7 {
                match &saved {
                    Some(region) => {
                        if let Ok(Some(bytes)) = region::read(region, cx, cz)
                            && let Some(chunk) = Chunk::from_bytes(&bytes)
                        {
                            chunks.insert((cx, cz), chunk);
                            from_disk += 1;
                        }
                    }
                    None => {
                        chunks.insert((cx, cz), Chunk::generated(&generator, cx, cz));
                    }
                }
            }
        }

        if saved.is_some() {
            println!("чанков с диска: {}", from_disk);
        }

        let block = |wx: i32, y: i32, wz: i32| -> &str {
            chunks
                .get(&(wx.div_euclid(CHUNK_SIZE), wz.div_euclid(CHUNK_SIZE)))
                .map(|chunk| chunk.block(wx.rem_euclid(CHUNK_SIZE), y, wz.rem_euclid(CHUNK_SIZE)))
                .and_then(crate::blocks::block_at_state)
                .unwrap_or("")
        };
        let bare = |name: &str| {
            matches!(name, "stone" | "gravel" | "andesite" | "diorite" | "granite" | "tuff" | "calcite" | "deepslate" | "cobblestone" | "mossy_cobblestone")
                || name.ends_with("_ore")
                || name.contains("sand")
                || name.contains("terracotta")
        };

        let (mut trees, mut on_bare, mut stony) = (0, 0, 0);
        let mut examples = Vec::new();

        for wx in (cx0 - 6) * CHUNK_SIZE..(cx0 + 7) * CHUNK_SIZE {
            for wz in (cz0 - 6) * CHUNK_SIZE..(cz0 + 7) * CHUNK_SIZE {
                for y in MIN_Y + 1..MIN_Y + WORLD_HEIGHT - 1 {
                    let name = block(wx, y, wz);
                    if !(name.ends_with("_log") || name == "mushroom_stem") {
                        continue;
                    }
                    let below = block(wx, y - 1, wz);
                    if below.ends_with("_log") || below.contains("leaves") || below == "air" || below == "mushroom_stem" || below.ends_with("_wood") {
                        continue;
                    }
                    trees += 1;
                    let around = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)]
                        .iter()
                        .filter(|(dx, dz)| bare(block(wx + dx, y - 1, wz + dz)))
                        .count();
                    if bare(below) {
                        on_bare += 1;
                    }
                    if around >= 4 {
                        stony += 1;
                    }
                    if (bare(below) || around >= 4) && examples.len() < 25 {
                        let column = generator.column_at(wx, wz);
                        examples.push(format!(
                            "{} на {} в ({}, {}, {}), камня вокруг {}/8, {:?}, верх столбца {}",
                            name, below, wx, y, wz, around, column.biome, column.height
                        ));
                    }
                }
            }
        }

        println!("стволов {}, прямо на голом {}, среди голого (≥4 из 8) {}", trees, on_bare, stony);
        for line in examples {
            println!("  {}", line);
        }
    }

    /// Сколько листвы с каким расстоянием до бревна (у оригинала 7 почти
    /// не бывает: такая листва осыпается).
    /// `cargo test --release leaf_distance_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn leaf_distance_census() {
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let mut counts = [0u64; 8];

        for chunk_x in (-200..200).step_by(20) {
            for chunk_z in (-200..200).step_by(20) {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
                            let state = chunk.block(x, y, z);
                            let Some(name) = crate::blocks::block_at_state(state) else { continue };

                            if !name.ends_with("_leaves") {
                                continue;
                            }

                            let distance = crate::blocks::orientation(name)
                                .and_then(|rule| rule.value(state, "distance"))
                                .and_then(|value| value.parse::<usize>().ok())
                                .unwrap_or(0);

                            counts[distance.min(7)] += 1;
                        }
                    }
                }
            }
        }

        println!("листва по расстоянию 0..7: {:?}", counts);
    }

    /// Сходится ли высота столбца, посчитанная в одиночку (`column_at`), с
    /// тем, что лежит в сложенном чанке: на этом держатся деревья и стыки.
    /// `cargo test --release column_height_matches_chunk -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn column_height_matches_chunk() {
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let (mut total, mut off, mut worst) = (0u64, 0u64, 0i32);
        let mut examples = Vec::new();

        for chunk_x in (-300..300).step_by(25) {
            for chunk_z in (-300..300).step_by(25) {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        let (wx, wz) = (chunk_x * CHUNK_SIZE + x, chunk_z * CHUNK_SIZE + z);
                        let column = generator.column_at(wx, wz);

                        // Верх рельефа в чанке: первый сверху камень или земля,
                        // мимо растений, деревьев, снега и воды.
                        let top = (MIN_Y..MIN_Y + WORLD_HEIGHT).rev().find(|&y| {
                            let name = crate::blocks::block_at_state(chunk.block(x, y, z)).unwrap_or("");
                            crate::blocks::conductive(chunk.block(x, y, z))
                                && !name.ends_with("_log")
                                && !name.ends_with("_leaves")
                                && !name.contains("mushroom")
                                && name != "snow_block"
                                && !name.contains("ice")
                        });

                        let Some(top) = top else { continue };

                        // Над морем — сравнивать нечего, если столбец под водой.
                        if column.height < 50 {
                            continue;
                        }

                        total += 1;
                        let diff = (top - column.height).abs();

                        if diff > 2 {
                            off += 1;
                            worst = worst.max(diff);

                            if examples.len() < 10 {
                                examples.push(format!("({}, {}): чанк {} / столбец {} {:?}", wx, wz, top, column.height, column.biome));
                            }
                        }
                    }
                }
            }
        }

        println!("столбцов {}, расходится больше чем на 2: {} ({:.3}%), худшее {}", total, off, off as f64 * 100.0 / total.max(1) as f64, worst);
        for line in examples {
            println!("  {}", line);
        }
    }

    #[test]
    #[ignore]
    fn plants_census() {
        const WATCH: [&str; 40] = [
            "sunflower", "lilac", "rose_bush", "peony", "tall_grass", "large_fern", "pumpkin", "melon", "sugar_cane",
            "cactus", "sweet_berry_bush", "lily_pad", "dandelion", "poppy", "cornflower", "azure_bluet", "short_grass",
            "fern", "pink_petals", "wildflowers", "dead_bush", "lily_of_the_valley", "oxeye_daisy", "allium",
            "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "blue_orchid", "bush", "firefly_bush",
            "leaf_litter", "short_dry_grass", "tall_dry_grass", "brown_mushroom", "red_mushroom", "seagrass",
            "tall_seagrass", "kelp", "cactus_flower",
        ];
        // PLANTS_TSV=1 — все растения построчно, как `plants_census.py --tsv`.
        let tsv = std::env::var_os("PLANTS_TSV").is_some();
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let mut chunks: HashMap<&str, u64> = HashMap::new();
        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();
        // Чанки через восемь на квадрате 800×800 — по полосам, каждая в своём
        // потоке.
        let rows: Vec<i32> = (-400..400).step_by(8).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let parts: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = rows
                .chunks(rows.len().div_ceil(threads))
                .map(|part| {
                    let generator = &generator;

                    scope.spawn(move || {
                        let mut chunks: HashMap<&str, u64> = HashMap::new();
                        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();

                        for &chunk_x in part {
                            for chunk_z in (-400..400).step_by(8) {
                                let chunk = Chunk::generated(generator, chunk_x, chunk_z);
                                let biome =
                                    generator.column_at(chunk_x * CHUNK_SIZE + 8, chunk_z * CHUNK_SIZE + 8).biome.name();

                                *chunks.entry(biome).or_default() += 1;

                                for x in 0..CHUNK_SIZE {
                                    for z in 0..CHUNK_SIZE {
                                        for y in 32..208 {
                                            let state = chunk.block(x, y, z);

                                            if state == AIR {
                                                continue;
                                            }

                                            // Верхняя половина двухблочного стоит на нижней
                                            // того же растения — её не считаем.
                                            if let Some(name) = crate::blocks::block_at_state(state)
                                                && WATCH.contains(&name)
                                                && crate::blocks::block_at_state(chunk.block(x, y - 1, z)) != Some(name)
                                            {
                                                *counts.entry((biome, name)).or_default() += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        (chunks, counts)
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });

        for (part_chunks, part_counts) in parts {
            for (biome, n) in part_chunks {
                *chunks.entry(biome).or_default() += n;
            }

            for (key, n) in part_counts {
                *counts.entry(key).or_default() += n;
            }
        }

        let mut biomes: Vec<_> = chunks.iter().filter(|(_, n)| **n >= 20).collect();
        biomes.sort_by_key(|(_, n)| std::cmp::Reverse(**n));

        for (biome, n) in biomes {
            if tsv {
                let mut names = WATCH;
                names.sort_unstable();

                for name in names {
                    if let Some(count) = counts.get(&(*biome, name)) {
                        println!("{}\t{}\t{}\t{:.3}", biome, n, name, *count as f64 / *n as f64);
                    }
                }

                continue;
            }

            let mut line: Vec<(&str, f64)> =
                WATCH.iter().map(|name| (*name, *counts.get(&(*biome, *name)).unwrap_or(&0) as f64 / *n as f64)).collect();
            line.sort_by(|a, b| b.1.total_cmp(&a.1));
            let text: Vec<String> = line.iter().take(8).filter(|(_, v)| *v > 0.0).map(|(k, v)| format!("{} {:.1}", k, v)).collect();

            println!("{} ({} чанков): {}", biome, n, text.join(", "));
        }
    }

    /// Сколько воды и лавы в обычных чанках сразу ставится течь и сколько
    /// стоит их поиск.
    /// `cargo test --release fluids_wake_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fluids_wake_census() {
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let chunks: Vec<Chunk> = (0..64).map(|i| Chunk::generated(&generator, i % 8 * 7, i / 8 * 7)).collect();
        let started = std::time::Instant::now();
        let found: usize = chunks.iter().enumerate().map(|(i, chunk)| chunk.fluids_to_wake(i as i32 % 8 * 7, i as i32 / 8 * 7).len()).sum();

        println!("на чанк: мест {:.1}, поиск {:?}", found as f64 / 64.0, started.elapsed() / 64);
    }

    /// Разбор места, где вода не течёт: участок 5×5 чанков кладётся в мир
    /// так же, как при игре, прогоняются такты жидкостей, и в середине
    /// ищутся места, которые правило растекания ещё хотело бы изменить.
    /// `CHUNK="-13 -81" cargo test --release fluids_missed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fluids_missed() {
        let place = std::env::var("CHUNK").unwrap_or_else(|_| "-13 -81".into());
        let mut parts = place.split_whitespace().map(|v| v.parse::<i32>().unwrap());
        let (cx, cz) = (parts.next().unwrap(), parts.next().unwrap());
        let generator = Arc::new(Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla)));
        let mut world = World::in_memory();
        // WORLD=<папка мира> — брать сохранённые чанки оттуда (например, мир
        // тестового сервера), а не свежую генерацию.
        let path = std::env::var("WORLD").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("mcsheriffanya-no-such-world"));
        let source = ChunkSource { path, generator, epoch: 0 };

        for dx in -2..=2 {
            for dz in -2..=2 {
                world.accept(cx + dx, cz + dz, source.take(cx + dx, cz + dz));
            }
        }

        if let Ok(spot) = std::env::var("SPOT") {
            let v: Vec<i32> = spot.split_whitespace().map(|p| p.parse().unwrap()).collect();
            let Taken::Loaded { chunk: fresh, wake, .. } = source.take(v[0].div_euclid(16), v[2].div_euclid(16)).taken else {
                panic!("чанк не сложился");
            };
            let s = fresh.block(v[0].rem_euclid(16), v[1], v[2].rem_euclid(16));
            let column = generator_column(&source, v[0], v[2]);
            println!("свежий чанк в {:?}: {:?}; столбец: высота {}, биом {}", v, crate::blocks::block_at_state(s), column.0, column.1);
            println!("в списке разбуженных: {}", wake.iter().any(|((x, y, z), _)| (*x, *y, *z) == (v[0], v[1], v[2])));
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0)] {
                let (x, y, z) = (v[0] + dx, v[1] + dy, v[2] + dz);
                let Taken::Loaded { chunk, .. } = source.take(x.div_euclid(16), z.div_euclid(16)).taken else {
                    continue;
                };
                println!("  сосед {} {} {} при генерации: {:?}", x, y, z, crate::blocks::block_at_state(chunk.block(x.rem_euclid(16), y, z.rem_euclid(16))));
            }
        }

        for _ in 0..std::env::var("TICKS").ok().and_then(|t| t.parse().ok()).unwrap_or(300) {
            for ((x, y, z), kind) in world.advance(crate::tick::PER_TICK) {
                if kind == TickKind::Fluid {
                    crate::tick::flow(&mut world, x, y, z);
                }
            }
        }

        let block = |x: i32, y: i32, z: i32| world.get_block(x, y, z);
        let mut stuck = 0;

        for x in cx * 16 - 16..cx * 16 + 32 {
            for z in cz * 16 - 16..cz * 16 + 32 {
                for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
                    let state = world.get_block(x, y, z);

                    if state != AIR && fluids::fluid_at(state).is_none() {
                        continue;
                    }

                    if let Some(next) = fluids::update((x, y, z), &block)
                        && next != state
                    {
                        stuck += 1;

                        if stuck <= 8 {
                            let around: Vec<String> = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0)]
                                .iter()
                                .map(|(dx, dy, dz)| {
                                    let s = world.get_block(x + dx, y + dy, z + dz);
                                    format!("{}{}", crate::blocks::block_at_state(s).unwrap_or("?"),
                                        fluids::fluid_at(s).map(|(_, l)| format!(":{}", l)).unwrap_or_default())
                                })
                                .collect();
                            println!("стоит {} {} {}: {:?} → {:?}; +x -x +z -z +y -y: {:?}", x, y, z,
                                crate::blocks::block_at_state(state), crate::blocks::block_at_state(next), around);
                        }
                    }
                }
            }
        }

        println!("мест, где жидкость ещё должна измениться: {}", stuck);
    }

    fn generator_column(source: &ChunkSource, x: i32, z: i32) -> (i32, &'static str) {
        let column = source.generator.column_at(x, z);
        (column.height, column.biome.name())
    }

    /// Вода над пустотой, сложенная вместе с чанком, сразу ставится течь,
    /// а замкнутая камнем — нет: ей течь некуда.
    #[test]
    fn generated_water_starts_flowing() {
        let mut chunk = Chunk::new();
        let water = fluids::state_of(fluids::Kind::Water, fluids::SOURCE);
        let stone = crate::blocks::state_by_name("stone").expect("камень есть");

        chunk.set(3, 70, 3, water);
        chunk.set(8, 70, 8, water);

        for (x, y, z) in [(7, 70, 8), (9, 70, 8), (8, 70, 7), (8, 70, 9), (8, 69, 8)] {
            chunk.set(x, y, z, stone);
        }

        // Будится сама вода: своим тактом она растечётся в пустоту под собой.
        let delay = fluids::Kind::Water.delay();
        let woken = chunk.fluids_to_wake(2, -1);

        assert!(woken.contains(&((35, 70, -13), delay)), "{:?}", woken);
        assert!(!woken.iter().any(|((x, _, z), _)| (*x, *z) == (40, -8)), "замкнутая вода разбужена: {:?}", woken);
    }

    /// Дно под водой по биомам: глубина и блоки сверху дна и на 1..4 ниже —
    /// в том же виде, что `tools/measure/seabed_census.py` у мира оригинала.
    /// Сравнение: `tools/measure/seabed_compare.py`.
    /// `cargo test --release seabed_census -- --ignored --nocapture | grep -P '^(depth|layer)\t'`.
    #[test]
    #[ignore]
    fn seabed_census() {
        type Layers = HashMap<(&'static str, i32), HashMap<&'static str, u64>>;

        let name = |state| crate::blocks::block_at_state(state).unwrap_or("air");
        let wet = |block: &str| {
            matches!(block, "water" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" | "bubble_column" | "sea_pickle")
                || block.ends_with("coral")
                || block.ends_with("coral_fan")
        };
        let top = |block: &str| wet(block) || matches!(block, "ice" | "packed_ice" | "blue_ice" | "lily_pad");
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let rows: Vec<i32> = (-300..300).step_by(6).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let parts: Vec<(HashMap<&str, Vec<i32>>, Layers)> = std::thread::scope(|scope| {
            let handles: Vec<_> = rows
                .chunks(rows.len().div_ceil(threads))
                .map(|part| {
                    let generator = &generator;

                    scope.spawn(move || {
                        let mut depth: HashMap<&str, Vec<i32>> = HashMap::new();
                        let mut layers: Layers = HashMap::new();

                        for &chunk_x in part {
                            for chunk_z in (-300..300).step_by(6) {
                                let chunk = Chunk::generated(generator, chunk_x, chunk_z);

                                for x in 0..CHUNK_SIZE {
                                    for z in 0..CHUNK_SIZE {
                                        let mut y = terrain::SEA;

                                        if !top(name(chunk.block(x, y, z))) {
                                            continue;
                                        }

                                        while y > -60 && top(name(chunk.block(x, y, z))) {
                                            y -= 1;
                                        }

                                        let biome = generator
                                            .column_at(chunk_x * CHUNK_SIZE + x, chunk_z * CHUNK_SIZE + z)
                                            .biome
                                            .name();

                                        depth.entry(biome).or_default().push(terrain::SEA - y);

                                        for k in 0..5 {
                                            *layers
                                                .entry((biome, k))
                                                .or_default()
                                                .entry(name(chunk.block(x, y - k, z)))
                                                .or_default() += 1;
                                        }
                                    }
                                }
                            }
                        }

                        (depth, layers)
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });

        let mut depth: HashMap<&str, Vec<i32>> = HashMap::new();
        let mut layers: Layers = HashMap::new();

        for (part_depth, part_layers) in parts {
            for (biome, values) in part_depth {
                depth.entry(biome).or_default().extend(values);
            }

            for (key, blocks) in part_layers {
                for (block, n) in blocks {
                    *layers.entry(key).or_default().entry(block).or_default() += n;
                }
            }
        }

        for (biome, values) in &mut depth {
            if values.len() < 2000 {
                continue;
            }

            values.sort();
            let at = |part: f64| values[((values.len() - 1) as f64 * part) as usize];
            println!("depth\t{}\t{}\t{}\t{}\t{}", biome, at(0.1), at(0.5), at(0.9), values.len());

            for k in 0..5 {
                let blocks = &layers[&(*biome, k)];
                let total: u64 = blocks.values().sum();
                let mut sorted: Vec<_> = blocks.iter().collect();
                sorted.sort_by(|a, b| b.1.cmp(a.1));

                for (block, n) in sorted.into_iter().take(8) {
                    println!("layer\t{}\t{}\t{}\t{:.4}", biome, k, block, *n as f64 / total as f64);
                }
            }
        }
    }

    /// Подводное на чанк по биомам — как tools/measure/ocean_census.py
    /// считает у оригинала (биом — по середине чанка на поверхности):
    /// кораллы, морские огурцы, айсберги, лёд на воде, магма и пузырьковые
    /// колонны. Высоты айсбергов — от верхнего блока воды (у оригинала он
    /// на y=62, у нас — `terrain::SEA`).
    /// `cargo test --release ocean_census -- --ignored --nocapture`.
    ///
    /// Тёплых и замёрзших океанов мало, поэтому чанки выбираются по биому
    /// середины на квадрате 4000×4000 чанков через восемь: каждого биома —
    /// не больше 300 (OCEAN_CAP) первых по порядку.
    #[test]
    #[ignore]
    fn ocean_census() {
        const ROWS: [&str; 32] = [
            "coral_block", "coral", "coral_fan", "coral_wall_fan", "sea_pickle", "pickles", "sea_pickle_on_coral",
            "sea_pickle_on_other", "coral_chunks", "coral_kinds_sum", "packed_ice", "packed_ice_above",
            "packed_ice_below", "snow_block", "snow_block_above", "snow_block_below", "blue_ice", "blue_ice_above",
            "blue_ice_below", "berg_cols", "berg_chunks", "berg_max_sum", "berg_max_top", "sea_ice", "sea_open",
            "magma_block", "magma_wet_floor", "magma_wet_under", "magma_dry", "magma_floor_y_sum", "bubble_column",
            "bubble_bottoms",
        ];
        let sea = terrain::SEA;
        let cap: usize = std::env::var("OCEAN_CAP").ok().and_then(|text| text.parse().ok()).unwrap_or(300);
        // Имя блока по состоянию — заранее: иначе поиск на каждый блок.
        let names: Vec<&str> =
            (0..=u16::MAX as i32).map(|state| crate::blocks::block_at_state(state).unwrap_or("")).collect();
        let name = |state: i32| names[state as usize];
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        // Сперва биомы середин — это дёшево, потом чанки нужных.
        let lines: Vec<i32> = (-2000..2000).step_by(8).collect();
        let found: Vec<Vec<(i32, i32, &str)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = lines
                .chunks(lines.len().div_ceil(threads))
                .map(|part| {
                    let generator = &generator;

                    scope.spawn(move || {
                        let mut found = Vec::new();

                        for &chunk_x in part {
                            for chunk_z in (-2000..2000).step_by(8) {
                                let middle = generator.column_at(chunk_x * CHUNK_SIZE + 8, chunk_z * CHUNK_SIZE + 8);

                                found.push((chunk_x, chunk_z, middle.biome.name()));
                            }
                        }

                        found
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });
        let mut taken: HashMap<&str, usize> = HashMap::new();
        let picked: Vec<(i32, i32, &str)> = found
            .into_iter()
            .flatten()
            .filter(|(_, _, biome)| {
                let count = taken.entry(biome).or_default();

                *count += 1;
                *count <= cap
            })
            .collect();
        let parts: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = picked
                .chunks(picked.len().div_ceil(threads))
                .map(|part| {
                    let (generator, name) = (&generator, &name);

                    scope.spawn(move || {
                        let mut chunks: HashMap<&str, u64> = HashMap::new();
                        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();

                        for &(chunk_x, chunk_z, biome) in part {
                            {
                                let chunk = Chunk::generated(generator, chunk_x, chunk_z);
                                let mut add = |row: &'static str, n: u64| *counts.entry((biome, row)).or_default() += n;
                                let top = chunk.highest();
                                let mut kinds = std::collections::HashSet::new();
                                let mut berg_top = 0;

                                *chunks.entry(biome).or_default() += 1;

                                for x in 0..CHUNK_SIZE {
                                    for z in 0..CHUNK_SIZE {
                                        let mut surface = None;

                                        for y in (MIN_Y..=top).rev() {
                                            let state = chunk.block(x, y, z);

                                            if state == AIR {
                                                continue;
                                            }

                                            let here = name(state);

                                            if surface.is_none() {
                                                surface = Some(if here == "snow" { y - 1 } else { y });
                                            }

                                            let coral = |suffix: &str| !here.starts_with("dead_") && here.ends_with(suffix);

                                            if coral("_coral_block") {
                                                add("coral_block", 1);
                                            } else if coral("_coral_wall_fan") {
                                                add("coral_wall_fan", 1);
                                            } else if coral("_coral_fan") {
                                                add("coral_fan", 1);
                                            } else if coral("_coral") {
                                                add("coral", 1);
                                            } else if here == "sea_pickle" {
                                                let count = crate::blocks::orientation_of(state)
                                                    .and_then(|rule| rule.value(state, "pickles"))
                                                    .and_then(|text| text.parse().ok())
                                                    .unwrap_or(1);
                                                let under = name(chunk.block(x, y - 1, z));

                                                add("sea_pickle", 1);
                                                add("pickles", count);
                                                add(
                                                    if under.ends_with("_coral_block") {
                                                        "sea_pickle_on_coral"
                                                    } else {
                                                        "sea_pickle_on_other"
                                                    },
                                                    1,
                                                );
                                            } else if matches!(here, "packed_ice" | "snow_block" | "blue_ice") {
                                                add(here, 1);
                                                add(
                                                    match (here, y > sea) {
                                                        ("packed_ice", true) => "packed_ice_above",
                                                        ("packed_ice", false) => "packed_ice_below",
                                                        ("snow_block", true) => "snow_block_above",
                                                        ("snow_block", false) => "snow_block_below",
                                                        (_, true) => "blue_ice_above",
                                                        _ => "blue_ice_below",
                                                    },
                                                    1,
                                                );
                                            } else if here == "magma_block" {
                                                let above = name(chunk.block(x, y + 1, z));

                                                add("magma_block", 1);
                                                add(
                                                    if matches!(above, "water" | "bubble_column") {
                                                        "magma_wet_under"
                                                    } else {
                                                        "magma_dry"
                                                    },
                                                    1,
                                                );
                                            } else if here == "bubble_column" {
                                                add("bubble_column", 1);

                                                if name(chunk.block(x, y - 1, z)) == "magma_block" {
                                                    add("bubble_bottoms", 1);
                                                }
                                            }

                                            if !here.starts_with("dead_")
                                                && (here.ends_with("_coral_block")
                                                    || here.ends_with("_coral")
                                                    || here.ends_with("_coral_fan")
                                                    || here.ends_with("_coral_wall_fan"))
                                            {
                                                kinds.insert(here.split('_').next().unwrap_or(""));
                                            }
                                        }

                                        // Поверхность моря: лёд или открытая вода там, где над
                                        // ней ничего; айсберги — по столбцам.
                                        match surface {
                                            Some(y) if y == sea => match name(chunk.block(x, y, z)) {
                                                "ice" => add("sea_ice", 1),
                                                "water" => add("sea_open", 1),
                                                _ => {}
                                            },
                                            Some(y)
                                                if y > sea
                                                    && y <= sea + 60
                                                    && matches!(
                                                        name(chunk.block(x, y, z)),
                                                        "packed_ice" | "snow_block" | "blue_ice"
                                                    ) =>
                                            {
                                                add("berg_cols", 1);
                                                berg_top = berg_top.max(y - sea);
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                                if berg_top > 0 {
                                    add("berg_chunks", 1);
                                    add("berg_max_sum", berg_top as u64);
                                    let best = counts.entry((biome, "berg_max_top")).or_default();
                                    *best = (*best).max(berg_top as u64);
                                }

                                if !kinds.is_empty() {
                                    *counts.entry((biome, "coral_chunks")).or_default() += 1;
                                }

                                *counts.entry((biome, "coral_kinds_sum")).or_default() += kinds.len() as u64;
                            }
                        }

                        (chunks, counts)
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });

        let mut chunks: HashMap<&str, u64> = HashMap::new();
        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();

        for (part_chunks, part_counts) in parts {
            for (biome, n) in part_chunks {
                *chunks.entry(biome).or_default() += n;
            }

            for (key, n) in part_counts {
                let total = counts.entry(key).or_default();

                *total = if key.1 == "berg_max_top" { (*total).max(n) } else { *total + n };
            }
        }

        let mut biomes: Vec<_> = chunks.iter().filter(|(_, n)| **n >= 20).collect();
        biomes.sort_by_key(|(_, n)| std::cmp::Reverse(**n));

        for (biome, n) in biomes {
            let line: Vec<String> = ROWS
                .iter()
                .filter_map(|row| {
                    let count = *counts.get(&(*biome, *row))?;

                    (count > 0 && *row != "berg_max_top").then(|| format!("{} {:.2}", row, count as f64 / *n as f64))
                })
                .collect();

            println!("{} ({} чанков): {}", biome, n, line.join(", "));

            if let Some(top) = counts.get(&(*biome, "berg_max_top")) {
                println!("  самый высокий айсберг: {}", top);
            }
        }
    }

    /// Сколько в чанках блоков, у которых на клиенте своя объёмная модель
    /// (сундуки, спавнеры, ульи, скалк и прочее): клиент рисует их как
    /// сущности, и тысячи таких в кадре тормозят.
    /// `cargo test --release block_entity_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn block_entity_census() {
        const WITH_MODEL: [&str; 16] = [
            "chest", "trapped_chest", "spawner", "trial_spawner", "bee_nest", "beehive", "sculk_sensor",
            "calibrated_sculk_sensor", "sculk_shrieker", "sculk_catalyst", "bell", "decorated_pot", "creaking_heart",
            "suspicious_sand", "suspicious_gravel", "vault",
        ];

        for style in [terrain::Style::Smooth, terrain::Style::Vanilla] {
            let generator = Generator::Normal(Terrain::new(100_554_032_945_340, style));
            let mut counts: HashMap<&str, u64> = HashMap::new();
            let side = 21;

            for chunk_x in -side / 2..=side / 2 {
                for chunk_z in -side / 2..=side / 2 {
                    let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                    for x in 0..CHUNK_SIZE {
                        for z in 0..CHUNK_SIZE {
                            for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
                                let state = chunk.block(x, y, z);

                                if state == AIR {
                                    continue;
                                }

                                if let Some(name) = crate::blocks::block_at_state(state)
                                    && WITH_MODEL.contains(&name)
                                {
                                    *counts.entry(name).or_default() += 1;
                                }
                            }
                        }
                    }
                }
            }

            println!("{:?}, {} чанков: {:?}", style, side * side, counts);
        }
    }

    /// Разведка подземелья: из чего сложен камень под землёй в сложенных
    /// чанках — сколько пустот, сколько руды и какой. Запускается вручную:
    /// `cargo test underground -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn underground() {
        let generator = Generator::Normal(Terrain::smooth(100_554_032_945_340));
        let air = AIR;
        let water = terrain::block_named("water");
        let lava = terrain::block_named("lava");

        let mut counts: HashMap<String, u64> = HashMap::new();
        let mut hollow = 0u64;
        let mut solid = 0u64;

        for i in 0..64 {
            let (chunk_x, chunk_z) = (i % 8 * 3, i / 8 * 3);
            let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let column = generator.column_at(chunk_x * 16 + x, chunk_z * 16 + z);

                    // Только то, что под землёй: от дна до поверхности.
                    for y in (MIN_Y + 5)..column.height.min(terrain::SEA) {
                        let state = chunk.block(x, y, z);

                        if state == air || state == water || state == lava {
                            hollow += 1;
                            continue;
                        }

                        solid += 1;

                        let name = crate::blocks::block_at_state(state).unwrap_or("?");

                        if name.ends_with("_ore")
                            || matches!(
                                name,
                                "dirt" | "gravel" | "granite" | "diorite" | "andesite" | "tuff"
                            )
                        {
                            *counts
                                .entry(name.trim_start_matches("deepslate_").to_string())
                                .or_default() += 1;
                        }
                    }
                }
            }
        }

        let total = (hollow + solid) as f64;
        let ores: u64 = counts
            .iter()
            .filter(|(name, _)| name.ends_with("_ore"))
            .map(|(_, n)| n)
            .sum();

        println!(
            "под землёй: пустот {:.1}%, руды {:.2}%",
            100.0 * hollow as f64 / total,
            100.0 * ores as f64 / total
        );

        let mut rows: Vec<(String, u64)> = counts.into_iter().collect();
        rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

        for (name, n) in rows {
            println!("  {:>7.3}%  {}", 100.0 * n as f64 / total, name);
        }
    }

    /// Вода в пещерах сразу после складывания — те же замеры, что
    /// снимаются с region-файлов оригинала: текущая жидкость на чанк,
    /// источники рядом с воздухом, и чем заполнены пустоты под дном моря
    /// и под сушей по глубине от дна.
    /// `cargo test --release cave_water_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cave_water_census() {
        use crate::world::terrain::{self, Terrain};
        use std::collections::BTreeMap;

        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let name = |state: i32| crate::blocks::block_at_state(state).unwrap_or("?");
        let waterish = |state: i32| matches!(name(state), "water" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" | "bubble_column");
        let (mut flow, mut leaks) = (BTreeMap::new(), BTreeMap::new());
        let mut under: BTreeMap<(&str, &str, &str), u64> = BTreeMap::new();
        let mut count = 0;

        for chunk_z in 0..13 {
            for chunk_x in 0..32 {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);
                count += 1;

                for x in 0..16 {
                    for z in 0..16 {
                        for y in MIN_Y..200 {
                            let state = chunk.block(x, y, z);
                            let Some((kind, level)) = fluids::fluid_at(state) else { continue };
                            if level != fluids::SOURCE {
                                *flow.entry(format!("{:?}", kind)).or_insert(0u64) += 1;
                                continue;
                            }
                            for (dx, dy, dz) in [(0, -1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)] {
                                let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                                if (0..16).contains(&nx) && (0..16).contains(&nz) && chunk.block(nx, ny, nz) == AIR {
                                    *leaks.entry(format!("{:?} {}", kind, if dy < 0 { "вниз" } else { "вбок" })).or_insert(0u64) += 1;
                                }
                            }
                        }

                        let mut y = 200;
                        while y > MIN_Y && chunk.block(x, y, z) == AIR {
                            y -= 1;
                        }
                        let sea = waterish(chunk.block(x, y, z)) && y >= 55;
                        while y > MIN_Y && (waterish(chunk.block(x, y, z)) || chunk.block(x, y, z) == AIR || name(chunk.block(x, y, z)) == "ice") {
                            y -= 1;
                        }
                        let floor = y;
                        if floor < 40 {
                            continue;
                        }
                        for yy in (-53..floor).rev() {
                            let state = chunk.block(x, yy, z);
                            let depth = floor - yy;
                            let band = match depth {
                                ..=10 => "1-10",
                                11..=30 => "11-30",
                                31..=60 => "31-60",
                                _ => "61+",
                            };
                            let what = if state == AIR {
                                "воздух"
                            } else if waterish(state) {
                                "вода"
                            } else if name(state) == "lava" {
                                "лава"
                            } else {
                                "твёрдое"
                            };
                            *under.entry((if sea { "под морем" } else { "под сушей" }, band, what)).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        println!("чанков {}", count);
        println!("текущая на чанк: {:?}", flow.iter().map(|(k, v)| (k.clone(), *v as f64 / count as f64)).collect::<Vec<_>>());
        println!("утечки на чанк: {:?}", leaks.iter().map(|(k, v)| (k.clone(), *v as f64 / count as f64)).collect::<Vec<_>>());
        for place in ["под морем", "под сушей"] {
            println!("{}", place);
            for band in ["1-10", "11-30", "31-60", "61+"] {
                let get = |what| under.get(&(place, band, what)).copied().unwrap_or(0);
                let all = (get("воздух") + get("вода") + get("лава") + get("твёрдое")).max(1) as f64;
                println!(
                    "  {:6} воздух {:.4} вода {:.4} лава {:.4}  (блоков {})",
                    band, get("воздух") as f64 / all, get("вода") as f64 / all, get("лава") as f64 / all, all
                );
            }
        }

        // Те же замеры по первым 400 чанкам r.0.0 мира оригинала
        // (vanilla-big), снятые с region-файлов.
        println!("оригинал: текущей воды 0,08 на чанк, лавы 0,01; утечек 0,2 на чанк");
        println!("  под морем:  1-10 воздух 0,0001 вода 0,027; 11-30 — 0,014 / 0,040; 31-60 — 0,044 / 0,050; 61+ — 0,054 / 0,002");
        println!("  под сушей:  1-10 воздух 0,023 вода 0,007; 11-30 — 0,005 / 0,008; 31-60 — 0,069 / 0,010; 61+ — 0,093 / 0,002");
    }
}

/// Точка в общей рамке непустого набора частей строения.
fn in_structure_bounds(bounds: impl Iterator<Item = [i32; 6]>, at: (i32, i32, i32)) -> bool {
    let bounds = bounds.reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2]), a[3].max(b[3]), a[4].max(b[4]), a[5].max(b[5])]);
    bounds.is_some_and(|b| b[0] <= at.0 && at.0 <= b[3] && b[1] <= at.1 && at.1 <= b[4] && b[2] <= at.2 && at.2 <= b[5])
}
