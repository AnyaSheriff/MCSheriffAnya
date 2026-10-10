// Мобы: живые существа мира — корова, свинья, овца, курица.
//
// Кода игры и чужих ядер здесь нет: правила взяты со страниц minecraft.wiki
// (Entity, Cow, Pig, Sheep, Chicken, Knockback, Damage, Breeding, Mob AI),
// размеры и номера видов — из minecraft-data (entities.json, 26.1), а числа,
// которых на вики нет, сняты с официального сервера чёрным ящиком —
// сценарии tools/blackbox/scenarios/cow-physics.json, sheep-shear.json,
// mob-plate.json, chicken-fall.json.
//
// Устройство такое же, как у прочих сущностей (items.rs, minecart.rs, frame.rs):
// склад мобов лежит в `Items` рядом с предметами, там же общий журнал, по
// которому подключения узнают о появлении, ударах и смерти; движение же
// клиентам рассылает конец такта (network/play.rs, `send_mob_ticks`).
//
// Что не сверено, помечено «[проверить]».

pub mod ai;
pub mod arthropod;
pub mod dragon;
pub mod golem;
pub mod guardian;
pub mod wither;
pub mod hostile;
pub mod illager;
pub mod leash;
pub mod nether;
pub mod phantom;
pub mod path;
pub mod spawn;
pub mod slime;
pub mod skeleton;
pub mod tame;

use crate::blocks;
use crate::inventory::Stack;
use crate::survival::{Hit, Source, Vitals};

/// Вид моба.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Cow,
    Pig,
    Sheep,
    Chicken,
    Zombie,
    Skeleton,
    /// Зимогор и болотник: общий ИИ скелета (вики, Stray, Bogged).
    Stray,
    Bogged,
    Creeper,
    Spider,
    CaveSpider,
    Silverfish,
    /// Кристалл Края: не живое существо, а «сущность-другое» (minecraft-data:
    /// end_crystal), но живёт в списке мобов ради слежения и ударов.
    EndCrystal,
    /// Дракон Края (вики, Ender Dragon).
    EnderDragon,
    /// Эндермен (вики, Enderman).
    Enderman,
    /// Шалкер (вики, Shulker): сидит на грани блока, не ходит.
    Shulker,
    /// Снаряд шалкера: не живое, как кристалл, но в списке мобов — ради
    /// слежения и самонаведения (вики, Shulker Bullet).
    ShulkerBullet,
    /// Ведьма (вики, Witch).
    Witch,
    /// Житель (вики, Villager): торговля — src/villager.rs.
    Villager,
    /// Зомби-житель (вики, Zombie Villager): зомби, что помнит профессию,
    /// сделки и опыт жителя; лечится золотым яблоком под слабостью.
    ZombieVillager,
    /// Ифрит (вики, Blaze): парит, стреляет тройкой малых огненных шаров.
    Blaze,
    /// Скелет-иссушитель (вики, Wither Skeleton): каменный меч, иссушение.
    WitherSkeleton,
    /// Магмовый куб (вики, Magma Cube): прыгает, делится при смерти.
    MagmaCube,
    /// Слизень (вики, Slime).
    Slime,
    /// Гаст (вики, Ghast): летает, огненные шары силой 1.
    Ghast,
    /// Зомби-пиглин (вики, Zombified Piglin): нейтрален, злится стаей.
    ZombifiedPiglin,
    /// Пиглин (вики, Piglin): меняет золото на добычу, нейтрален к золотой броне.
    Piglin,
    /// Брут пиглинов (вики, Piglin Brute): стерегёт бастион, броня не усмиряет.
    PiglinBrute,
    /// Хоглин (вики, Hoglin): бодает вверх, боится искажённого гриба.
    Hoglin,
    /// Страйдер (вики, Strider): ходит по лаве, мёрзнет вне её.
    Strider,
    /// Утопленник (вики, Drowned): зомби воды — плавает, кидает трезубец,
    /// горит на солнце вне воды.
    Drowned,
    /// Кадавр (вики, Husk): зомби пустыни — не горит, бьёт голодом.
    Husk,
    /// Волк (вики, Wolf): приручается костью, защищает хозяина (mob/tame.rs).
    Wolf,
    /// Кошка (вики, Cat): приручается сырой рыбой, пугает криперов (mob/tame.rs).
    Cat,
    /// Фантом (вики, Phantom): летает над игроком, что давно не спал, и пикирует (mob/phantom.rs).
    Phantom,
    /// Узел поводка на заборе (вики, Lead, «Leash knot»): не живое, как
    /// кристалл, но в списке мобов — ради слежения (mob/leash.rs).
    LeashKnot,
    /// Железный голем (вики, Iron Golem): строится из железа и тыквы (mob/golem.rs).
    IronGolem,
    /// Снежный голем (вики, Snow Golem): кидает снежки во врагов (mob/golem.rs).
    SnowGolem,
    /// Страж и древний страж (вики, Guardian, Elder Guardian): луч, шипы,
    /// проклятие (mob/guardian.rs).
    Guardian,
    ElderGuardian,
    /// Иссушитель (вики, Wither): босс из черепов и песка душ (mob/wither.rs).
    Wither,
    /// Разбойники (вики, Illager) и разоритель, вредина, челюсти заклинателя (mob/illager.rs).
    Pillager,
    Vindicator,
    Evoker,
    Vex,
    Illusioner,
    Ravager,
    /// Челюсти заклинателя: не живое, как снаряд шалкера (вики, Evoker Fangs).
    EvokerFangs,
}

impl Kind {
    /// Все виды: для проверок.
    #[cfg(test)]
    pub const ALL: [Kind; 48] = [
        Kind::Cow,
        Kind::Pig,
        Kind::Sheep,
        Kind::Chicken,
        Kind::Zombie,
        Kind::Skeleton,
        Kind::Stray,
        Kind::Bogged,
        Kind::Creeper,
        Kind::Spider,
        Kind::CaveSpider,
        Kind::Silverfish,
        Kind::EndCrystal,
        Kind::EnderDragon,
        Kind::Enderman,
        Kind::Shulker,
        Kind::ShulkerBullet,
        Kind::Witch,
        Kind::Villager,
        Kind::ZombieVillager,
        Kind::Blaze,
        Kind::WitherSkeleton,
        Kind::MagmaCube,
        Kind::Slime,
        Kind::Ghast,
        Kind::ZombifiedPiglin,
        Kind::Piglin,
        Kind::PiglinBrute,
        Kind::Hoglin,
        Kind::Strider,
        Kind::Drowned,
        Kind::Husk,
        Kind::Wolf,
        Kind::Cat,
        Kind::Phantom,
        Kind::LeashKnot,
        Kind::IronGolem,
        Kind::SnowGolem,
        Kind::Guardian,
        Kind::ElderGuardian,
        Kind::Wither,
        Kind::Pillager,
        Kind::Vindicator,
        Kind::Evoker,
        Kind::Vex,
        Kind::Illusioner,
        Kind::Ravager,
        Kind::EvokerFangs,
    ];

    /// Враждебный ли вид (вики, Monster): у него свои цели (mob/hostile.rs).
    pub fn hostile(self) -> bool {
        matches!(
            self,
            Kind::Zombie
                | Kind::ZombieVillager
                | Kind::Drowned
                | Kind::Husk
                | Kind::Skeleton
                | Kind::Stray
                | Kind::Bogged
                | Kind::Creeper
                | Kind::Spider
                | Kind::CaveSpider
                | Kind::Silverfish
                | Kind::Enderman
                | Kind::Shulker
                | Kind::Witch
                | Kind::Blaze
                | Kind::WitherSkeleton
                | Kind::MagmaCube
                | Kind::Slime
                | Kind::Ghast
                | Kind::ZombifiedPiglin
                | Kind::Piglin
                | Kind::PiglinBrute
                | Kind::Hoglin
                | Kind::Phantom
                | Kind::Guardian
                | Kind::ElderGuardian
                | Kind::Wither
                | Kind::Pillager
                | Kind::Vindicator
                | Kind::Evoker
                | Kind::Vex
                | Kind::Illusioner
                | Kind::Ravager
        )
    }

    /// Скелеты с луком используют один ИИ и одно снаряжение.
    pub fn skeleton(self) -> bool {
        matches!(self, Kind::Skeleton | Kind::Stray | Kind::Bogged)
    }

    /// Оба паука используют лазание и охоту (вики, Cave Spider: «inherit»).
    pub fn spider(self) -> bool {
        matches!(self, Kind::Spider | Kind::CaveSpider)
    }

    /// Моб Незера (mob/nether.rs): у них своя мысль.
    pub fn nether(self) -> bool {
        matches!(
            self,
            Kind::Blaze | Kind::WitherSkeleton | Kind::MagmaCube | Kind::Ghast | Kind::ZombifiedPiglin | Kind::Piglin | Kind::PiglinBrute | Kind::Hoglin
        )
    }

    /// Не живое существо, а «сущность-другое»: кристалл и снаряд шалкера.
    pub fn object(self) -> bool {
        matches!(self, Kind::EndCrystal | Kind::ShulkerBullet | Kind::LeashKnot | Kind::EvokerFangs)
    }

    /// По имени сущности, как в командах.
    pub fn from_name(name: &str) -> Option<Kind> {
        Some(match name.strip_prefix("minecraft:").unwrap_or(name) {
            "cow" => Kind::Cow,
            "pig" => Kind::Pig,
            "sheep" => Kind::Sheep,
            "chicken" => Kind::Chicken,
            "zombie" => Kind::Zombie,
            "skeleton" => Kind::Skeleton,
            "stray" => Kind::Stray,
            "bogged" => Kind::Bogged,
            "creeper" => Kind::Creeper,
            "spider" => Kind::Spider,
            "cave_spider" => Kind::CaveSpider,
            "silverfish" => Kind::Silverfish,
            "end_crystal" => Kind::EndCrystal,
            "ender_dragon" => Kind::EnderDragon,
            "enderman" => Kind::Enderman,
            "shulker" => Kind::Shulker,
            "shulker_bullet" => Kind::ShulkerBullet,
            "witch" => Kind::Witch,
            "villager" => Kind::Villager,
            "zombie_villager" => Kind::ZombieVillager,
            "blaze" => Kind::Blaze,
            "wither_skeleton" => Kind::WitherSkeleton,
            "magma_cube" => Kind::MagmaCube,
            "slime" => Kind::Slime,
            "ghast" => Kind::Ghast,
            "zombified_piglin" => Kind::ZombifiedPiglin,
            "piglin" => Kind::Piglin,
            "piglin_brute" => Kind::PiglinBrute,
            "hoglin" => Kind::Hoglin,
            "strider" => Kind::Strider,
            "drowned" => Kind::Drowned,
            "husk" => Kind::Husk,
            "wolf" => Kind::Wolf,
            "cat" => Kind::Cat,
            "phantom" => Kind::Phantom,
            "leash_knot" => Kind::LeashKnot,
            "iron_golem" => Kind::IronGolem,
            "snow_golem" => Kind::SnowGolem,
            "guardian" => Kind::Guardian,
            "elder_guardian" => Kind::ElderGuardian,
            "wither" => Kind::Wither,
            "pillager" => Kind::Pillager,
            "vindicator" => Kind::Vindicator,
            "evoker" => Kind::Evoker,
            "vex" => Kind::Vex,
            "illusioner" => Kind::Illusioner,
            "ravager" => Kind::Ravager,
            "evoker_fangs" => Kind::EvokerFangs,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Cow => "cow",
            Kind::Pig => "pig",
            Kind::Sheep => "sheep",
            Kind::Chicken => "chicken",
            Kind::Zombie => "zombie",
            Kind::Skeleton => "skeleton",
            Kind::Stray => "stray",
            Kind::Bogged => "bogged",
            Kind::Creeper => "creeper",
            Kind::Spider => "spider",
            Kind::CaveSpider => "cave_spider",
            Kind::Silverfish => "silverfish",
            Kind::EndCrystal => "end_crystal",
            Kind::EnderDragon => "ender_dragon",
            Kind::Enderman => "enderman",
            Kind::Shulker => "shulker",
            Kind::ShulkerBullet => "shulker_bullet",
            Kind::Witch => "witch",
            Kind::Villager => "villager",
            Kind::ZombieVillager => "zombie_villager",
            Kind::Blaze => "blaze",
            Kind::WitherSkeleton => "wither_skeleton",
            Kind::MagmaCube => "magma_cube",
            Kind::Slime => "slime",
            Kind::Ghast => "ghast",
            Kind::ZombifiedPiglin => "zombified_piglin",
            Kind::Piglin => "piglin",
            Kind::PiglinBrute => "piglin_brute",
            Kind::Hoglin => "hoglin",
            Kind::Strider => "strider",
            Kind::Drowned => "drowned",
            Kind::Husk => "husk",
            Kind::Wolf => "wolf",
            Kind::Cat => "cat",
            Kind::Phantom => "phantom",
            Kind::LeashKnot => "leash_knot",
            Kind::IronGolem => "iron_golem",
            Kind::SnowGolem => "snow_golem",
            Kind::Guardian => "guardian",
            Kind::ElderGuardian => "elder_guardian",
            Kind::Wither => "wither",
            Kind::Pillager => "pillager",
            Kind::Vindicator => "vindicator",
            Kind::Evoker => "evoker",
            Kind::Vex => "vex",
            Kind::Illusioner => "illusioner",
            Kind::Ravager => "ravager",
            Kind::EvokerFangs => "evoker_fangs",
        }
    }

    /// Номер вида сущности для клиента Java (minecraft-data 26.1, entities.json).
    pub fn java_type(self) -> i32 {
        match self {
            Kind::Chicken => 26,
            Kind::Creeper => 32,
            Kind::Skeleton => 115,
            Kind::Stray => 128,
            Kind::Bogged => 16,
            Kind::Spider => 124,
            Kind::CaveSpider => 22,
            Kind::Silverfish => 114,
            Kind::Zombie => 150,
            Kind::Cow => 30,
            Kind::Pig => 100,
            Kind::Sheep => 111,
            Kind::EnderDragon => 43,
            Kind::EndCrystal => 45,
            Kind::Enderman => 41,
            Kind::Shulker => 112,
            Kind::ShulkerBullet => 113,
            Kind::Witch => 144,
            Kind::Villager => 139,
            // minecraft-data 26.1: zombie_villager 153 (чёрный ящик, zombie-villager-stand).
            Kind::ZombieVillager => 153,
            // minecraft-data 26.1, entities.json.
            Kind::Blaze => 14,
            Kind::WitherSkeleton => 146,
            Kind::MagmaCube => 80,
            Kind::Slime => 117,
            Kind::Ghast => 57,
            Kind::ZombifiedPiglin => 154,
            Kind::Piglin => 101,
            Kind::PiglinBrute => 102,
            Kind::Hoglin => 64,
            Kind::Strider => 129,
            // minecraft-data 26.1, entities.json: drowned 38, husk 67 (чёрный ящик,
            // drowned-attack и husk-hunger: тип в пакете появления).
            Kind::Drowned => 38,
            Kind::Husk => 67,
            // minecraft-data 26.1, entities.json: wolf 148, cat 21, leash_knot 76
            // (чёрный ящик, wolf-tame, cat-tame, lead-attach).
            Kind::Wolf => 148,
            Kind::Cat => 21,
            // minecraft-data 26.1: phantom 99.
            Kind::Phantom => 99,
            Kind::LeashKnot => 76,
            // minecraft-data 26.1, entities.json: iron_golem 70, snow_golem 121.
            Kind::IronGolem => 70,
            Kind::SnowGolem => 121,
            // minecraft-data 26.1, entities.json.
            Kind::Guardian => 63,
            Kind::ElderGuardian => 40,
            Kind::Wither => 145,
            Kind::Pillager => 103,
            Kind::Vindicator => 140,
            Kind::Evoker => 46,
            Kind::Vex => 138,
            Kind::Illusioner => 68,
            Kind::Ravager => 109,
            Kind::EvokerFangs => 47,
        }
    }

    /// Имя вида для клиента Bedrock (AddActor).
    pub fn bedrock_name(self) -> &'static str {
        match self {
            Kind::Cow => "minecraft:cow",
            Kind::Pig => "minecraft:pig",
            Kind::Sheep => "minecraft:sheep",
            Kind::Chicken => "minecraft:chicken",
            Kind::Zombie => "minecraft:zombie",
            Kind::Skeleton => "minecraft:skeleton",
            // [Bedrock: потом] Свойства и эффекты новых вариантов ещё не сверены.
            Kind::Stray => "minecraft:stray",
            Kind::Bogged => "minecraft:bogged",
            Kind::Creeper => "minecraft:creeper",
            Kind::Spider => "minecraft:spider",
            // [Bedrock: потом] отдельная проверка метаданных и поведения.
            Kind::CaveSpider => "minecraft:cave_spider",
            Kind::Silverfish => "minecraft:silverfish",
            Kind::EndCrystal => "minecraft:ender_crystal",
            Kind::EnderDragon => "minecraft:ender_dragon",
            Kind::Enderman => "minecraft:enderman",
            Kind::Shulker => "minecraft:shulker",
            Kind::ShulkerBullet => "minecraft:shulker_bullet",
            Kind::Witch => "minecraft:witch",
            Kind::Villager => "minecraft:villager_v2",
            Kind::ZombieVillager => "minecraft:zombie_villager_v2",
            // [Bedrock: потом] имена видов Незера для клиента Bedrock.
            Kind::Blaze => "minecraft:blaze",
            Kind::WitherSkeleton => "minecraft:wither_skeleton",
            Kind::MagmaCube => "minecraft:magma_cube",
            // [Bedrock: потом] размер и анимация слизня.
            Kind::Slime => "minecraft:slime",
            Kind::Ghast => "minecraft:ghast",
            Kind::ZombifiedPiglin => "minecraft:zombie_pigman",
            Kind::Piglin => "minecraft:piglin",
            Kind::PiglinBrute => "minecraft:piglin_brute",
            Kind::Hoglin => "minecraft:hoglin",
            Kind::Strider => "minecraft:strider",
            // [Bedrock: потом] утопленник и кадавр для клиента Bedrock: имена есть, плавания и трезубца нет.
            Kind::Drowned => "minecraft:drowned",
            Kind::Husk => "minecraft:husk",
            // [Bedrock: потом] приручение, ошейник и узел поводка для клиента Bedrock.
            Kind::Wolf => "minecraft:wolf",
            Kind::Cat => "minecraft:cat",
            Kind::Phantom => "minecraft:phantom",
            Kind::LeashKnot => "minecraft:leash_knot",
            // [Bedrock: потом] големы для клиента Bedrock: имена есть, свойств нет.
            Kind::IronGolem => "minecraft:iron_golem",
            Kind::SnowGolem => "minecraft:snow_golem",
            Kind::Guardian => "minecraft:guardian",
            Kind::ElderGuardian => "minecraft:elder_guardian",
            Kind::Wither => "minecraft:wither",
            // [Bedrock: потом] разбойники для клиента Bedrock: имена есть, свойств нет.
            Kind::Pillager => "minecraft:pillager",
            Kind::Vindicator => "minecraft:vindicator",
            Kind::Evoker => "minecraft:evoker",
            Kind::Vex => "minecraft:vex",
            Kind::Illusioner => "minecraft:illusioner",
            Kind::Ravager => "minecraft:ravager",
            Kind::EvokerFangs => "minecraft:evoker_fangs",
        }
    }

    /// Ширина ящика (minecraft-data, entities.json).
    pub fn width(self) -> f64 {
        match self {
            Kind::Chicken => 0.4,
            Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::Skeleton | Kind::Stray | Kind::Bogged | Kind::Creeper | Kind::Enderman | Kind::Witch | Kind::Villager => 0.6,
            Kind::Spider => 1.4,
            Kind::CaveSpider => 0.7,
            Kind::Silverfish => 0.4,
            Kind::Blaze | Kind::ZombifiedPiglin | Kind::Piglin | Kind::PiglinBrute => 0.6,
            Kind::WitherSkeleton => 0.7,
            Kind::Ghast => 4.0,
            Kind::Hoglin => 1.3964844,
            Kind::Strider => 0.9,
            // Размер куба: 0,52 на единицу размера (minecraft-data: 0,52 при размере 1);
            // настоящую ширину даёт `Mob::size`.
            Kind::MagmaCube | Kind::Slime => 0.52,
            Kind::Shulker => 1.0,
            Kind::ShulkerBullet => 0.3125,
            Kind::EndCrystal => 2.0,
            Kind::EnderDragon => 16.0,
            // minecraft-data: wolf 0,6 × 0,85, cat 0,6 × 0,7, leash_knot 0,375 × 0,5.
            Kind::Wolf | Kind::Cat => 0.6,
            Kind::Phantom => 0.9,
            Kind::LeashKnot => 0.375,
            // minecraft-data: iron_golem 1,4 × 2,7, snow_golem 0,7 × 1,9.
            Kind::IronGolem => 1.4,
            Kind::SnowGolem => 0.7,
            // minecraft-data 26.1: 0,85 и 1,9975.
            Kind::Guardian => 0.85,
            Kind::ElderGuardian => 1.9975,
            // minecraft-data 26.1: 0,9 × 3,5.
            Kind::Wither => 0.9,
            // minecraft-data: разбойники 0,6 × 1,95, вредина 0,4 × 0,8, разоритель 1,95 × 2,2, челюсти 0,5 × 0,8.
            Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner => 0.6,
            Kind::Vex => 0.4,
            Kind::Ravager => 1.95,
            Kind::EvokerFangs => 0.5,
            _ => 0.9,
        }
    }

    /// Высота ящика взрослого (minecraft-data, entities.json).
    pub fn height(self) -> f64 {
        match self {
            Kind::Chicken => 0.7,
            Kind::Cow => 1.4,
            Kind::Pig => 0.9,
            Kind::Sheep => 1.3,
            Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk => 1.95,
            Kind::Skeleton | Kind::Stray | Kind::Bogged => 1.99,
            Kind::Creeper => 1.7,
            Kind::Spider => 0.9,
            Kind::CaveSpider => 0.5,
            Kind::Silverfish => 0.3,
            Kind::EndCrystal => 2.0,
            Kind::EnderDragon => 8.0,
            Kind::Enderman => 2.9,
            Kind::Shulker => 1.0,
            Kind::ShulkerBullet => 0.3125,
            Kind::Witch | Kind::Villager => 1.95,
            Kind::Blaze => 1.8,
            Kind::WitherSkeleton => 2.4,
            Kind::MagmaCube | Kind::Slime => 0.52,
            Kind::Ghast => 4.0,
            Kind::ZombifiedPiglin | Kind::Piglin | Kind::PiglinBrute => 1.95,
            Kind::Hoglin => 1.4,
            Kind::Strider => 1.7,
            Kind::Wolf => 0.85,
            Kind::Cat => 0.7,
            Kind::Phantom => 0.5,
            Kind::LeashKnot => 0.5,
            Kind::IronGolem => 2.7,
            Kind::SnowGolem => 1.9,
            Kind::Guardian => 0.85,
            Kind::ElderGuardian => 1.9975,
            // minecraft-data 26.1: 0,9 × 3,5.
            Kind::Wither => 3.5,
            Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner => 1.95,
            Kind::Vex | Kind::EvokerFangs => 0.8,
            Kind::Ravager => 2.2,
        }
    }

    /// Здоровье (вики: корова и свинья 10, овца 8, курица 4).
    pub fn max_health(self) -> f32 {
        match self {
            Kind::Cow | Kind::Pig => 10.0,
            Kind::Sheep => 8.0,
            Kind::Chicken => 4.0,
            Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::Skeleton | Kind::Stray | Kind::Creeper => 20.0,
            Kind::Bogged => 16.0,
            Kind::Spider => 16.0,
            Kind::CaveSpider => 12.0,
            Kind::Silverfish => 8.0,
            // У кристалла здоровья нет: любой удар его взрывает (вики, End Crystal).
            Kind::EndCrystal => 1.0,
            // Вики, Ender Dragon: «Health: 200»; чёрный ящик, dragon-summon: 9=200.
            Kind::EnderDragon => 200.0,
            // Вики: эндермен 40, шалкер 30, ведьма 26; снаряд гибнет от удара.
            Kind::Enderman => 40.0,
            Kind::Shulker => 30.0,
            Kind::ShulkerBullet => 1.0,
            Kind::Witch => 26.0,
            // Вики, Villager: «Health points 20»; чёрный ящик, villager-trade: 9=20.
            Kind::Villager => 20.0,
            // Вики: ифрит 20, иссушитель 20, гаст 10, зомби-пиглин 20, пиглин 16,
            // брут 50, хоглин 40, страйдер 20; куб — `Mob::new` по размеру.
            Kind::Blaze | Kind::WitherSkeleton | Kind::ZombifiedPiglin | Kind::Strider => 20.0,
            Kind::Ghast => 10.0,
            Kind::Piglin => 16.0,
            Kind::PiglinBrute => 50.0,
            Kind::Hoglin => 40.0,
            Kind::MagmaCube | Kind::Slime => 1.0,
            // Вики, Wolf: дикий 8, приручённый 40 (`tame::TAME_HEALTH`); чёрный ящик,
            // wolf-tame: 9=8, после приручения 9=40. Cat: 10 (cat-tame: 9=10).
            Kind::Wolf => 8.0,
            Kind::Cat => 10.0,
            Kind::Phantom => 20.0,
            Kind::LeashKnot => 1.0,
            // Вики, Iron Golem: «Health points 100»; Snow Golem: «4».
            Kind::IronGolem => golem::IRON_HEALTH,
            Kind::SnowGolem => golem::SNOW_HEALTH,
            // Вики: страж 30, древний 80.
            Kind::Guardian => 30.0,
            Kind::ElderGuardian => 80.0,
            // Вики, Wither: 300 на любой сложности.
            Kind::Wither => wither::HEALTH,
            Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner | Kind::Vex | Kind::Ravager | Kind::EvokerFangs => illager::max_health(self),
        }
    }

    /// Скорость шага, блоков за такт при множителе 1 (вики, Cow/Pig/Sheep/
    /// Chicken, «Attributes»: 0,2; 0,25; 0,23; 0,25).
    pub fn speed(self) -> f64 {
        match self {
            Kind::Cow => 0.2,
            Kind::Pig | Kind::Chicken => 0.25,
            Kind::Sheep | Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk => 0.23,
            Kind::Skeleton | Kind::Stray | Kind::Bogged | Kind::Creeper => 0.25,
            Kind::Spider => 0.3,
            Kind::CaveSpider => 0.3,
            Kind::Silverfish => 0.25,
            Kind::EndCrystal => 0.0,
            // Дракон летает (вики, Ender Dragon); ходьбы у него нет.
            Kind::EnderDragon => 0.0,
            // Вики, Enderman: 0,3; Witch: 0,25; шалкер не ходит.
            Kind::Enderman => 0.3,
            Kind::Witch => 0.25,
            // Чёрный ящик, villager-trade: атрибут скорости 0,5.
            Kind::Villager => 0.5,
            // Вики, «Attributes»: ифрит 0,23; иссушитель 0,25; куб 0,2 (с размером
            // `Mob::new` не меняет); зомби-пиглин 0,23; пиглин 0,35; брут 0,35;
            // хоглин 0,3; страйдер 0,175. Гаст и ифрит летают сами (nether.rs).
            Kind::Blaze => 0.23,
            Kind::WitherSkeleton => 0.25,
            Kind::MagmaCube => 0.2,
            Kind::Slime => slime::speed(1),
            Kind::Ghast => 0.0,
            Kind::ZombifiedPiglin => 0.23,
            Kind::Piglin | Kind::PiglinBrute => 0.35,
            Kind::Hoglin => 0.3,
            Kind::Strider => 0.175,
            Kind::Shulker | Kind::ShulkerBullet | Kind::LeashKnot => 0.0,
            // Чёрный ящик, wolf-tame и cat-tame: атрибут скорости 0,3.
            Kind::Wolf | Kind::Cat => 0.3,
            // Вики, «Attributes»: железный голем 0,25, снежный 0,2.
            Kind::IronGolem => 0.25,
            Kind::SnowGolem => 0.2,
            Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner | Kind::Vex | Kind::Ravager | Kind::EvokerFangs => illager::speed(self),
            Kind::Phantom => 0.0,
            // [проверить] атрибут скорости: страж 0,5, древний 0,3.
            Kind::Guardian => 0.5,
            Kind::ElderGuardian => 0.3,
            // Вики, Wither: скорость 0,6.
            Kind::Wither => 0.6,
        }
    }

    /// Съедобное для разведения (вики, Breeding).
    pub fn likes(self, item: &str) -> bool {
        match self {
            Kind::Cow | Kind::Sheep => item == "wheat",
            Kind::Pig => matches!(item, "carrot" | "potato" | "beetroot"),
            Kind::Chicken => {
                matches!(item, "wheat_seeds" | "beetroot_seeds" | "melon_seeds" | "pumpkin_seeds" | "torchflower_seeds" | "pitcher_pod")
            }
            _ => false,
        }
    }

    /// Броня (чёрный ящик, zombie-hit: удар мечом 7 снимает 6,888 — как при
    /// 2 очках брони: `урон · (1 − max(брони/5, брони − урон/2) / 25)`).
    pub fn armor(self) -> f32 {
        match self {
            Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::ZombifiedPiglin => 2.0,
            // Вики, Wither: броня 4.
            Kind::Wither => 4.0,
            _ => 0.0,
        }
    }

    /// Звук в покое, при ударе и при смерти (вики, Sounds).
    pub fn sounds(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Kind::Cow => ("minecraft:entity.cow.ambient", "minecraft:entity.cow.hurt", "minecraft:entity.cow.death"),
            Kind::Pig => ("minecraft:entity.pig.ambient", "minecraft:entity.pig.hurt", "minecraft:entity.pig.death"),
            Kind::Sheep => ("minecraft:entity.sheep.ambient", "minecraft:entity.sheep.hurt", "minecraft:entity.sheep.death"),
            Kind::Chicken => ("minecraft:entity.chicken.ambient", "minecraft:entity.chicken.hurt", "minecraft:entity.chicken.death"),
            Kind::Zombie => ("minecraft:entity.zombie.ambient", "minecraft:entity.zombie.hurt", "minecraft:entity.zombie.death"),
            // Чёрный ящик, zombie-villager-cure: ambient и hurt — zombie_villager.
            Kind::ZombieVillager => ("minecraft:entity.zombie_villager.ambient", "minecraft:entity.zombie_villager.hurt", "minecraft:entity.zombie_villager.death"),
            Kind::Skeleton => ("minecraft:entity.skeleton.ambient", "minecraft:entity.skeleton.hurt", "minecraft:entity.skeleton.death"),
            Kind::Bogged => ("minecraft:entity.bogged.ambient", "minecraft:entity.bogged.hurt", "minecraft:entity.bogged.death"),
            Kind::Stray => ("minecraft:entity.stray.ambient", "minecraft:entity.stray.hurt", "minecraft:entity.stray.death"),
            // У крипера голоса в покое нет (вики, Creeper, «Sounds»).
            Kind::Creeper => ("", "minecraft:entity.creeper.hurt", "minecraft:entity.creeper.death"),
            Kind::Spider | Kind::CaveSpider => ("minecraft:entity.spider.ambient", "minecraft:entity.spider.hurt", "minecraft:entity.spider.death"),
            Kind::Silverfish => ("minecraft:entity.silverfish.ambient", "minecraft:entity.silverfish.hurt", "minecraft:entity.silverfish.death"),
            // Кристалл молчит: его единственный звук — взрыв.
            Kind::EndCrystal => ("", "", ""),
            // Чёрный ящик, dragon-summon: голос в покое — entity.ender_dragon.ambient, громкость 5.
            Kind::EnderDragon => ("minecraft:entity.ender_dragon.ambient", "minecraft:entity.ender_dragon.hurt", "minecraft:entity.ender_dragon.death"),
            // Вики, Enderman/Shulker/Witch, «Sounds»; у закрытого шалкера удар
            // звучит hurt_closed (`Mob::hurt_sound`).
            Kind::Enderman => ("minecraft:entity.enderman.ambient", "minecraft:entity.enderman.hurt", "minecraft:entity.enderman.death"),
            Kind::Shulker => ("minecraft:entity.shulker.ambient", "minecraft:entity.shulker.hurt", "minecraft:entity.shulker.death"),
            Kind::ShulkerBullet => ("", "minecraft:entity.shulker_bullet.hurt", ""),
            Kind::Witch => ("minecraft:entity.witch.ambient", "minecraft:entity.witch.hurt", "minecraft:entity.witch.death"),
            // Чёрный ящик, villager-shake: голос в покое и смерть; удар — hurt [проверить].
            Kind::Villager => ("minecraft:entity.villager.ambient", "minecraft:entity.villager.hurt", "minecraft:entity.villager.death"),
            // Вики, «Sounds» страниц мобов Незера.
            Kind::Blaze => ("minecraft:entity.blaze.ambient", "minecraft:entity.blaze.hurt", "minecraft:entity.blaze.death"),
            Kind::WitherSkeleton => ("minecraft:entity.wither_skeleton.ambient", "minecraft:entity.wither_skeleton.hurt", "minecraft:entity.wither_skeleton.death"),
            Kind::Slime => ("", "minecraft:entity.slime.hurt", "minecraft:entity.slime.death"),
            Kind::MagmaCube => ("", "minecraft:entity.magma_cube.hurt", "minecraft:entity.magma_cube.death"),
            Kind::Ghast => ("minecraft:entity.ghast.ambient", "minecraft:entity.ghast.hurt", "minecraft:entity.ghast.death"),
            Kind::ZombifiedPiglin => ("minecraft:entity.zombified_piglin.ambient", "minecraft:entity.zombified_piglin.hurt", "minecraft:entity.zombified_piglin.death"),
            Kind::Piglin => ("minecraft:entity.piglin.ambient", "minecraft:entity.piglin.hurt", "minecraft:entity.piglin.death"),
            Kind::PiglinBrute => ("minecraft:entity.piglin_brute.ambient", "minecraft:entity.piglin_brute.hurt", "minecraft:entity.piglin_brute.death"),
            Kind::Hoglin => ("minecraft:entity.hoglin.ambient", "minecraft:entity.hoglin.hurt", "minecraft:entity.hoglin.death"),
            Kind::Strider => ("minecraft:entity.strider.ambient", "minecraft:entity.strider.hurt", "minecraft:entity.strider.death"),
            // Вики, Drowned и Husk, «Sounds»: у утопленника на суше — entity.drowned.*,
            // в воде — `Mob::sounds`; у кадавра свои.
            Kind::Drowned => ("minecraft:entity.drowned.ambient", "minecraft:entity.drowned.hurt", "minecraft:entity.drowned.death"),
            Kind::Husk => ("minecraft:entity.husk.ambient", "minecraft:entity.husk.hurt", "minecraft:entity.husk.death"),
            // Вики, Wolf и Cat, «Sounds»: голос в покое у волка и кошки зависит от
            // состояния (`Mob::sounds`: злой рычит, раненый скулит, приручённая мурлычет).
            Kind::Wolf => ("minecraft:entity.wolf.ambient", "minecraft:entity.wolf.hurt", "minecraft:entity.wolf.death"),
            Kind::Cat => ("minecraft:entity.cat.stray_ambient", "minecraft:entity.cat.hurt", "minecraft:entity.cat.death"),
            Kind::Phantom => ("minecraft:entity.phantom.ambient", "minecraft:entity.phantom.hurt", "minecraft:entity.phantom.death"),
            Kind::LeashKnot => ("", "", ""),
            // Вики, Iron Golem и Snow Golem, «Sounds»: у железного голоса в покое нет.
            Kind::IronGolem => ("", "minecraft:entity.iron_golem.hurt", "minecraft:entity.iron_golem.death"),
            Kind::SnowGolem => ("minecraft:entity.snow_golem.ambient", "minecraft:entity.snow_golem.hurt", "minecraft:entity.snow_golem.death"),
            // Голоса в воде; на суше — свои (guardian::sounds).
            Kind::Guardian => ("minecraft:entity.guardian.ambient", "minecraft:entity.guardian.hurt", "minecraft:entity.guardian.death"),
            Kind::ElderGuardian => ("minecraft:entity.elder_guardian.ambient", "minecraft:entity.elder_guardian.hurt", "minecraft:entity.elder_guardian.death"),
            Kind::Wither => ("minecraft:entity.wither.ambient", "minecraft:entity.wither.hurt", "minecraft:entity.wither.death"),
            Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner | Kind::Vex | Kind::Ravager | Kind::EvokerFangs => illager::sounds(self),
        }
    }

    /// Звук шага (вики, Sounds).
    pub fn step_sound(self) -> &'static str {
        match self {
            Kind::Cow => "minecraft:entity.cow.step",
            Kind::Pig => "minecraft:entity.pig.step",
            Kind::Sheep => "minecraft:entity.sheep.step",
            Kind::Chicken => "minecraft:entity.chicken.step",
            Kind::Zombie => "minecraft:entity.zombie.step",
            Kind::ZombieVillager => "minecraft:entity.zombie_villager.step",
            Kind::Skeleton => "minecraft:entity.skeleton.step",
            Kind::Bogged => "minecraft:entity.bogged.step",
            Kind::Stray => "minecraft:entity.stray.step",
            // Шаги крипера играются звуком блока [проверить]; молча.
            Kind::Creeper => "",
            Kind::Spider | Kind::CaveSpider => "minecraft:entity.spider.step",
            Kind::Silverfish => "minecraft:entity.silverfish.step",
            Kind::WitherSkeleton => "minecraft:entity.wither_skeleton.step",
            Kind::ZombifiedPiglin => "minecraft:entity.zombified_piglin.step",
            Kind::Piglin => "minecraft:entity.piglin.step",
            Kind::PiglinBrute => "minecraft:entity.piglin_brute.step",
            Kind::Hoglin => "minecraft:entity.hoglin.step",
            Kind::Strider => "minecraft:entity.strider.step",
            Kind::Drowned => "minecraft:entity.drowned.step",
            Kind::Husk => "minecraft:entity.husk.step",
            Kind::Wolf => "minecraft:entity.wolf.step",
            Kind::IronGolem => "minecraft:entity.iron_golem.step",
            // Чёрный ящик, ravager-roar: entity.ravager.step.
            Kind::Ravager => "minecraft:entity.ravager.step",
            // У эндермена и ведьмы шаги — звуком блока [проверить]; молча.
            _ => "",
        }
    }
}

/// Тяга плывущего утопленника на единицу скорости шага: по замеру (чёрный ящик,
/// drowned-swim: к цели в воде 0,0365 блока за такт при сопротивлении 0,9)
/// [проверить: у оригинала разгон быстрее — сопротивление, видимо, меньше].
pub const SWIM_THRUST: f64 = 0.0159;

/// Шанс выпасть вещи из руки по умолчанию (вики, Drops: «8.5%»).
pub const DEFAULT_DROP: f32 = 0.085;

/// Сколько тактов детёныш растёт: 20 минут (вики, Breeding: «-24000»).
pub const BABY_AGE: i32 = -24000;

/// Пауза взрослого после разведения: 5 минут (вики, Breeding).
pub const BREED_COOLDOWN: i32 = 6000;

/// Сколько тактов животное «влюблено» после еды: 30 секунд (вики, Breeding).
pub const LOVE_TICKS: i32 = 600;

/// Цвета овец: белая — по умолчанию у призванной командой (вики, Sheep).
pub const WHITE: u8 = 0;

/// Сила обычного удара рукой и оружием без чар (вики, Knockback: 0,4).
pub const BASE_KNOCKBACK: f64 = 0.4;

/// Тяжесть живой сущности за такт и сопротивление воздуха по вертикали
/// (вики, Entity, «Motion»: gravity 0.08, drag 0.98).
pub const GRAVITY: f64 = 0.08;
pub const VERTICAL_DRAG: f64 = 0.98;

/// Сколько тактов длится паника после удара (вики, Mob AI, «Panic»: ~5 с).
pub const PANIC_TICKS: i32 = 100;

/// Сколько тактов тело лежит после смерти, пока не пропадёт: 20 (вики,
/// Entity, «Death»: анимация смерти — секунда).
pub const DEATH_TICKS: i32 = 20;

/// Сколько частей у дракона: голова, шея, тело, три хвоста, два крыла
/// (вики, Ender Dragon, «Hitbox»); их номера идут следом за номером дракона,
/// голова — первая [проверить].
pub const DRAGON_PARTS: i32 = 8;

/// Сколько тактов дракон умирает (вики, Ender Dragon: «death animation … 10
/// seconds (200 game ticks)»).
pub const DRAGON_DEATH_TICKS: i32 = 200;

/// Опыт за дракона: 12000 за первого в бою Края, 500 за всякого другого
/// (вики, Ender Dragon: «12,000 … 500 for subsequent dragons»).
pub const DRAGON_XP_FIRST: i32 = 12000;
pub const DRAGON_XP: i32 = 500;

/// Сколько тактов после удара игрока смерть моба ещё даёт опыт (вики,
/// Experience, «Sources»: «within five seconds (100 game ticks)»).
pub const XP_CREDIT_TICKS: u64 = 100;

/// Насколько животное поднимается из клетки на ступеньку без прыжка.
pub const STEP_HEIGHT: f64 = 0.6;

/// Мелочь, что ставится между ящиком и стеной, чтобы не входить в неё.
const EPS: f64 = 1.0e-7;

/// Откуда моб видит мир: блоки и загруженные чанки.
pub trait View {
    fn block(&self, x: i32, y: i32, z: i32) -> i32;
    fn loaded(&self, chunk_x: i32, chunk_z: i32) -> bool;

    /// Открыто ли небо над клеткой (для солнца). Без мира — нет.
    fn open_sky(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }

    /// Идёт ли в клетке дождь (World::rain_at). Без мира — нет.
    fn rain(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }

    /// Температура биома (снежный голем: снег и таяние); по умолчанию — равнины.
    fn temperature(&self, _x: i32, _y: i32, _z: i32) -> f32 {
        0.8
    }

    /// Небесный (с поправкой на час: `sky` — свет под открытым небом сейчас)
    /// и блочный свет в клетке.
    fn light(&self, _x: i32, _y: i32, _z: i32, _sky: i32) -> (u8, u8) {
        (0, 0)
    }
}

/// Взгляд страйдера: поверхность лавы — твёрдый пол.
pub struct LavaFloor<'a>(pub &'a dyn View);

impl View for LavaFloor<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        let state = self.0.block(x, y, z);

        if is_lava(state) && !is_lava(self.0.block(x, y + 1, z)) {
            return blocks::state_by_name("stone").unwrap_or(state);
        }

        state
    }

    fn loaded(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.0.loaded(chunk_x, chunk_z)
    }

    fn open_sky(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.open_sky(x, y, z)
    }

    fn rain(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.rain(x, y, z)
    }

    fn temperature(&self, x: i32, y: i32, z: i32) -> f32 {
        self.0.temperature(x, y, z)
    }

    fn light(&self, x: i32, y: i32, z: i32, sky: i32) -> (u8, u8) {
        self.0.light(x, y, z, sky)
    }
}

/// Упирается ли моб в блок: у всего, у чего есть ящик, — стена или пол
/// (как у выпавшего предмета; ступени и плиты считаются целыми [проверить]).
pub fn solid(state: i32) -> bool {
    blocks::stops_items(state)
}

/// Скользкость блока (вики, Block Slipperiness): обычный 0,6, лёд 0,98,
/// слизь 0,8, синий лёд 0,989.
pub fn slipperiness(state: i32) -> f64 {
    match blocks::block_at_state(state) {
        Some("ice" | "packed_ice" | "frosted_ice") => 0.98,
        Some("blue_ice") => 0.989,
        Some("slime_block") => 0.8,
        _ => 0.6,
    }
}

/// Вода ли в этом состоянии блока.
pub fn is_water(state: i32) -> bool {
    matches!(blocks::block_at_state(state), Some("water" | "bubble_column" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant"))
}

/// Лава ли.
pub fn is_lava(state: i32) -> bool {
    blocks::block_at_state(state) == Some("lava")
}

/// Частицы удара (minecraft-data 26.1, particles.json).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Particle {
    DamageIndicator,
    SweepAttack,
}

impl Particle {
    pub fn id(self) -> i32 {
        match self {
            Particle::DamageIndicator => 7,
            Particle::SweepAttack => 67,
        }
    }
}

/// Удар со стороны: куда толкает (единичное направление по горизонтали),
/// с какой силой и кто ударил (номер сущности, 0 — никто).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Push {
    pub dir: (f64, f64),
    pub strength: f64,
    pub cause: i32,
}

/// Что случилось с мобом — запись, которую подключения перескажут клиентам
/// в порядке, в каком оно произошло.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Урон: вид, номер ударившей сущности (0 — никто) и место.
    Hurt { source: Source, cause: i32, at: (f64, f64, f64) },

    /// Смерть: клиенты играют падение набок.
    Death,

    /// Частицы приземления: блок под ногами и их число.
    Landing { block: i32, count: i32 },

    /// Звук в месте моба: имя, громкость, высота.
    Sound { name: &'static str, volume: f32, pitch: f32 },

    /// То же, но в точке, взятой до сдвига такта (прыжок куба: чёрный ящик,
    /// slime-hop — звук прыжка играет там, где куб стоял).
    SoundAt { name: &'static str, volume: f32, pitch: f32, at: (f64, f64, f64) },

    /// Звук игрока в его точке — удар по животному (раздел «игроки»): к
    /// моб-журналу привязан ради порядка пакетов (чёрный ящик, cow-hit).
    PlayerSound { name: &'static str, at: (f64, f64, f64) },

    /// Частицы: вид, место, разброс, скорость, число.
    Particle { kind: Particle, at: (f64, f64, f64), offset: (f32, f32, f32), speed: f32, count: i32 },

    /// Звук, привязанный к сущности (Entity Sound Effect): стрижка овцы —
    /// раздел «игроки» (чёрный ящик, sheep-shear). Не глушится `Silent`.
    EntitySound { name: &'static str, category: i32, volume: f32, pitch: f32 },

    /// Добыча погибшего: падает в его месте, в порядке журнала — между звуком
    /// смерти и событием смерти (чёрный ящик, cow-physics).
    Loot(Vec<Stack>),

    /// Анимация сущности (Entity Animation): 4 — критический удар (частицы
    /// вокруг раненого; чёрный ящик, cow-crit).
    Animation(u8),

    /// Тело убрано: облачко (событие сущности 60) перед исчезновением.
    Poof,

    /// Сердечки над головой (событие сущности 18) — влюблён.
    Hearts,

    /// Овца щиплет траву (событие сущности 10).
    Eating,

    /// Взмах рукой (Entity Animation 0): удар зомби, выстрел скелета.
    Swing,

    /// Прочее событие сущности (Entity Event): 46 — частицы прыжка
    /// эндермена и шалкера [проверить].
    Status(u8),

    /// Поводок: держатель (номер сущности, 0 — снят) — пакет Link Entities
    /// в такте события (чёрный ящик, lead-attach).
    Leash(i32),

}

/// Моб в мире.
#[derive(Clone, Debug)]
pub struct Mob {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub kind: Kind,

    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Скорость, блоков за такт.
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,

    /// Поворот тела и наклон головы в градусах, поворот головы.
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,

    pub on_ground: bool,

    /// Здоровье, неуязвимость после удара, высота падения — та же
    /// модель, что у игрока (survival.rs).
    pub vitals: Vitals,

    /// `NoAI:1b`: ни мысли, ни движения (вики, Mob: «NoAI»).
    pub no_ai: bool,

    /// `Silent:1b`: моб не издаёт звуков.
    pub silent: bool,

    /// Возраст: отрицательный — детёныш, растёт к нулю; положительный у
    /// взрослого — пауза после разведения.
    pub age: i32,

    /// Сколько тактов ещё влюблён (0 — нет).
    pub love: i32,

    /// Овца: остригнута ли, цвет шерсти (0 — белая).
    pub sheared: bool,
    pub color: u8,

    /// Курица: сколько тактов до яйца.
    pub egg_timer: i32,

    /// Такты со смерти (0 — жив): на 20-м моб пропадает.
    pub dead_for: i32,

    /// Сколько тактов моб живёт.
    pub ticks: u64,

    /// Пройденное по земле для звука шагов и когда играть следующий.
    pub move_dist: f64,
    pub next_step: f64,

    /// Сколько раз моба толкнуло (удар): клиентам после толчка скорость
    /// уходит сразу, вне очереди.
    pub impulses: u32,

    /// Стрел в теле (свойство 12) и тактов до того, как выпадет одна (как у
    /// игрока: 20·(30 − число); wither-half-armor).
    pub arrows: i32,
    pub arrow_timer: i32,

    /// Что произошло за такт — забирает `Items`.
    pub events: Vec<Event>,

    /// Мысли и цель ходьбы.
    pub brain: ai::Brain,

    /// Тот, на кого сердится (для паники): куда бежать от него.
    pub fright: Option<(f64, f64)>,

    /// Сохраняется ли с чанком.
    pub persistent: bool,

    /// Шаг с оглядкой на цель (скелет): вперёд и влево, в долях скорости;
    /// действует, пока нет пути.
    pub strafe: (f64, f64),

    /// Последний сдвиг упёрся в стену: паук по ней лезет.
    pub wall: bool,

    /// Состояние охоты у враждебных (mob/hostile.rs).
    pub h: hostile::State,

    /// Последний удар нанёс игрок: без него у паука и прочих часть добычи
    /// не падает (вики, Spider: «spider eye — только если убит игроком»).
    pub by_player: bool,

    /// Родился во время хода мобов: слежение покажет его в следующем такте.
    pub late: bool,

    /// При показе клиентам сразу за появлением идёт событие 20 (облачко
    /// появления): чешуйница из заражённого блока (чёрный ящик, silverfish-infested).
    pub spawn_poof: bool,

    /// Сколько раз ему правили данные (`summon … {…}`, `data merge`):
    /// клиентам после этого уходит пакет атрибутов.
    pub attr_changes: u32,

    /// Сколько раз моба переносили командой: клиентам нужен перенос
    /// целиком, а не сдвиг.
    pub teleports: u32,

    /// До какого такта жизни смерть моба даёт опыт: игрок ударил его не
    /// раньше чем за 100 тактов (вики, Experience, «Sources»); 0 — не бил.
    pub xp_until: u64,

    /// Опыт, который надо ронять шариками: ставит смерть, забирает
    /// `Items::flush_mob_events`.
    pub pending_xp: i32,

    /// Дракон боя Края, ещё не побеждённого: за него 12000 опыта.
    pub first_dragon: bool,

    /// Кристалл: куда идёт луч (цель исцеления); дракон: номер кристалла,
    /// что его лечит (0 — никто).
    pub beam_target: Option<(i32, i32, i32)>,
    pub healer: i32,

    /// Кристалл ударили: взрывается в этот же такт и исчезает.
    pub exploding: bool,

    /// Дракон: фаза полёта и память боя (mob/dragon.rs).
    pub dragon: dragon::State,

    /// Житель: профессия, сделки, опыт (src/villager.rs).
    pub villager: crate::villager::State,

    /// Мобы Незера: размер куба, гнев, торг пиглина и пр. (mob/nether.rs).
    pub nether: nether::State,

    /// Уровень «Добычи» оружия убившего: число добавляется к верхней границе
    /// выпадения (вики, Looting).
    pub looting: i32,

    /// Имя с бирки или из команды (`CustomName`, простой текст): видно над
    /// головой, такой моб не исчезает (вики, Name Tag).
    pub name: Option<String>,

    /// Пометка «не исчезать» из бирки или `PersistenceRequired` (вики, Name Tag:
    /// «named mobs do not despawn»).
    pub persistent_tag: bool,

    /// На поводке: у кого (mob/leash.rs).
    pub leash: Option<leash::Holder>,

    /// Приручение волка и кошки (mob/tame.rs).
    pub tame: tame::State,

    /// Големы: цель, «построен игроком», тыква снежного (mob/golem.rs).
    pub golem: golem::State,
    /// Фантом: размер, якорь кружения, пике (mob/phantom.rs).
    pub phantom: phantom::State,

    /// Стражи: луч, шипы, проклятие (mob/guardian.rs).
    pub guardian: guardian::State,

    /// Иссушитель: зарядка, головы, черепа (mob/wither.rs).
    pub wither: wither::State,
    /// Разбойники: заклинания, арбалет, рёв, жизнь вредины и челюстей (mob/illager.rs).
    pub illager: illager::State,
}

/// Бит в байте вида записи моба: за записью идёт общий хвост (имя, поводок).
/// Старые записи его не имеют — читаются как раньше.
const EXTENDED: u8 = 0x80;

impl Mob {
    pub fn new(entity_id: i32, kind: Kind, at: (f64, f64, f64)) -> Self {
        let mut vitals = Vitals::new();

        vitals.health = kind.max_health();
        vitals.max_health = kind.max_health();

        // Магмовый куб рождается размера 2 (у призванного без `Size` в оригинале
        // размер случайный 1/2/4 при естественном появлении — `nether::roll_size`).
        let mut nether = nether::State::default();

        if kind == Kind::MagmaCube {
            nether.size = 2;
            vitals.health = 4.0;
            vitals.max_health = 4.0;
        }

        if kind == Kind::Slime {
            nether.size = 1;
        }

        // Шалкер без цвета — 16 (minecraft-data: color; замер мира оригинала:
        // у всех шалкеров городов Color 16).
        let h = hostile::State { bow: kind.skeleton(), shell_color: 16, hand_drop: DEFAULT_DROP, ..hostile::State::default() };

        Self {
            entity_id,
            uuid: uuid_for(entity_id),
            kind,
            x: at.0,
            y: at.1,
            z: at.2,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            vitals,
            no_ai: false,
            silent: false,
            age: 0,
            love: 0,
            sheared: false,
            color: WHITE,
            egg_timer: 0,
            dead_for: 0,
            ticks: 0,
            move_dist: 0.0,
            next_step: 1.0,
            impulses: 0,
            arrows: 0,
            arrow_timer: 0,
            events: Vec::new(),
            brain: ai::Brain::new(entity_id),
            fright: None,
            persistent: true,
            strafe: (0.0, 0.0),
            wall: false,
            h,
            by_player: false,
            late: false,
            spawn_poof: false,
            attr_changes: 0,
            teleports: 0,
            xp_until: 0,
            pending_xp: 0,
            first_dragon: false,
            beam_target: None,
            healer: 0,
            exploding: false,
            dragon: dragon::State::default(),
            villager: crate::villager::State::new(),
            nether,
            looting: 0,
            name: None,
            persistent_tag: false,
            leash: None,
            tame: tame::State { variant: tame::default_variant(kind), ..tame::State::default() },
            golem: golem::State::default(),
            phantom: phantom::State::default(),
            guardian: guardian::State::default(),
            wither: wither::State::default(),
            illager: illager::State::default(),
        }
    }

    /// Лечение кристаллом: ближайший живой кристалл не дальше 32 блоков по
    /// каждой оси светит в дракона лучом и даёт 1 здоровья раз в 10 тактов
    /// (вики, Ender Dragon: «1HP every 10 game ticks»; End Crystal: «cuboid
    /// extending 32 blocks»). Кто лечит — `healer`; луч ставит такт боя.
    fn crystal_heal(&mut self, mates: &[ai::Mate]) {
        let nearest = mates
            .iter()
            .filter(|mate| mate.kind == Kind::EndCrystal)
            .filter(|mate| (mate.x - self.x).abs() <= dragon::HEAL_RANGE && (mate.y - self.y).abs() <= dragon::HEAL_RANGE && (mate.z - self.z).abs() <= dragon::HEAL_RANGE)
            .min_by(|a, b| {
                let da = (a.x - self.x).powi(2) + (a.y - self.y).powi(2) + (a.z - self.z).powi(2);
                let db = (b.x - self.x).powi(2) + (b.y - self.y).powi(2) + (b.z - self.z).powi(2);
                da.total_cmp(&db)
            })
            .map(|mate| mate.id);

        self.healer = nearest.unwrap_or(0);

        if self.healer != 0 && self.ticks.is_multiple_of(dragon::HEAL_EVERY) {
            self.vitals.health = (self.vitals.health + 1.0).min(self.kind.max_health());
        }
    }

    /// Ударил игрок: смерть в ближайшие 100 тактов даст опыт.
    /// Стрела застряла в теле.
    pub fn stick_arrow(&mut self) {
        self.arrows += 1;
        self.arrow_timer = 20 * (30 - self.arrows);
    }

    pub fn credit_player(&mut self) {
        self.xp_until = self.ticks + XP_CREDIT_TICKS;
    }

    pub fn is_baby(&self) -> bool {
        self.age < 0
    }

    pub fn is_dead(&self) -> bool {
        self.dead_for > 0 || self.vitals.dead
    }

    /// Ширина и высота ящика: у детёныша вдвое меньше (вики, Baby mobs).
    pub fn size(&self) -> (f64, f64) {
        let scale = if self.is_baby() { 0.5 } else { 1.0 };

        // Куб: 0,52 · размер (вики, Magma Cube, «Hitbox»).
        if slime::cube(self.kind) {
            let cube = 0.52 * f64::from(self.nether.size.max(1));

            return (cube, cube);
        }

        (self.kind.width() * scale, self.kind.height() * scale)
    }

    /// Высота глаз: 0,85 роста (вики, Entity: «eye height»); у враждебных —
    /// свои: зомби и скелет 1,74 (чёрный ящик, skeleton-arrow: стрела вылетает
    /// на 1,64 = глаза − 0,1), крипер 1,445, паук 0,65 [проверить].
    pub fn eye(&self) -> f64 {
        match self.kind {
            Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::Skeleton | Kind::Stray | Kind::Bogged | Kind::ZombifiedPiglin | Kind::Piglin | Kind::PiglinBrute => 1.74,
            // [проверить] высота глаз: ифрит 1,53, иссушитель 2,1, гаст 2,6.
            Kind::Blaze => 1.53,
            Kind::WitherSkeleton => 2.1,
            Kind::Ghast => 2.6,
            // [проверить] высота глаз фантома.
            Kind::Phantom => 0.175,
            Kind::Slime => self.size().1 * 0.625,
            Kind::Creeper => 1.445,
            Kind::Spider => 0.65,
            // [проверить] глаза малых членистоногих.
            Kind::CaveSpider => 0.45,
            Kind::Silverfish => 0.13,
            Kind::Enderman => hostile::ENDERMAN_EYE,
            // Ведьма — как игрок; шалкер смотрит из середины [проверить].
            Kind::Witch => 1.62,
            Kind::Shulker => 0.5,
            // Страж смотрит из середины (вики, Guardian: луч из глаза) [проверить].
            Kind::Guardian | Kind::ElderGuardian => self.size().1 * 0.5,
            _ => self.size().1 * 0.85,
        }
    }

    /// Клетка, в которой стоит моб.
    pub fn cell(&self) -> (i32, i32, i32) {
        (self.x.floor() as i32, (self.y + 1.0e-3).floor() as i32, self.z.floor() as i32)
    }

    /// Чанк, в котором стоит моб.
    pub fn chunk(&self) -> (i32, i32) {
        ((self.x.floor() as i32).div_euclid(16), (self.z.floor() as i32).div_euclid(16))
    }

    /// Голоса моба: у утопленника в воде — свои (вики, Drowned, «Sounds»:
    /// ambient_water, hurt_water, death_water).
    pub fn sounds(&self) -> (&'static str, &'static str, &'static str) {
        if matches!(self.kind, Kind::Wolf | Kind::Cat) {
            return tame::sounds(self);
        }

        if guardian::guardian(self.kind) {
            return guardian::sounds(self);
        }

        if self.kind == Kind::Drowned && self.h.wet {
            return ("minecraft:entity.drowned.ambient_water", "minecraft:entity.drowned.hurt_water", "minecraft:entity.drowned.death_water");
        }

        self.kind.sounds()
    }

    /// Ставит поворот тела по направлению взгляда.
    pub fn face(&mut self, yaw: f32) {
        self.yaw = yaw;
        self.head_yaw = yaw;
    }

    /// Такт моба: рост, мысли, движение, урон от падения. Возвращает то, что
    /// случилось вовне (детёныш, съеденная трава, яйцо), — исполняет вызывающий.
    pub fn tick(&mut self, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
        self.ticks += 1;
        self.late = false;

        // Застрявшие стрелы выпадают по одной [проверить: у мобов тот же счёт,
        // что у игрока].
        if self.arrows > 0 {
            self.arrow_timer -= 1;

            if self.arrow_timer <= 0 {
                self.arrows -= 1;
                self.arrow_timer = 20 * (30 - self.arrows);
            }
        }

        // Снаряд шалкера — не живое: ни возраста, ни урона, только полёт.
        if self.kind == Kind::ShulkerBullet {
            return hostile::bullet(self, view, ctx);
        }

        // Челюсти заклинателя: ждут, кусают, исчезают (mob/illager.rs).
        if self.kind == Kind::EvokerFangs {
            return illager::fangs(self, ctx);
        }

        // Житель мотает головой и мёртвым: счёт идёт каждый такт (чёрный
        // ящик, villager-shake: 18 = 20 в такте смерти и вниз дальше).
        if self.kind == Kind::Villager && self.villager.unhappy > 0 {
            self.villager.unhappy -= 1;
        }

        // Мёртвый лежит и падает набок; тело пропадает через секунду.
        if self.is_dead() {
            self.dead_for += 1;

            if self.kind == Kind::EnderDragon {
                dragon::die_step(self);
            } else if !self.no_ai {
                self.physics(view, 0.0, false);
            }

            return ai::Plan::default();
        }

        let loaded = {
            let (cx, cz) = self.chunk();
            view.loaded(cx, cz)
        };

        // В выгруженном чанке мира нет: моб стоит, как и всё там.
        if !loaded {
            return ai::Plan::default();
        }

        self.vitals.tick_invulnerable();

        // Страж: в воде ли — и без ИИ (голоса суши и воды).
        if guardian::guardian(self.kind) {
            guardian::sense(self, view);
        }

        skeleton::snow_tick(self, view);

        // Зомби и кадавр: голова 30 секунд в воде — 15 секунд трясётся — и они
        // утопленник и зомби (вики, Drowned, «Spawning»; Husk; чёрный ящик,
        // zombie-to-drowned). Без ИИ не превращаются.
        if matches!(self.kind, Kind::Zombie | Kind::Husk) && !self.no_ai {
            if self.h.converting > 0 {
                self.h.converting -= 1;

                if self.h.converting == 0 {
                    self.h.converted = Some(if self.kind == Kind::Husk { Kind::Zombie } else { Kind::Drowned });
                }
            } else {
                let head = ((self.x).floor() as i32, (self.y + self.eye()).floor() as i32, (self.z).floor() as i32);

                if self.ticks <= hostile::CONVERT_FROM_TICK {
                    // счёт ещё не начался
                } else if is_water(view.block(head.0, head.1, head.2)) {
                    self.h.water_ticks += 1;

                    if self.h.water_ticks >= hostile::CONVERT_AFTER {
                        self.h.converting = hostile::CONVERT_SHAKE;
                    }
                } else {
                    self.h.water_ticks = -1;
                }
            }
        }

        // Рост и пауза после разведения (вики, Breeding).
        // Детёныш-зомби не растёт (вики, Zombie: «baby zombies never grow up»).
        if self.age < 0 && !self.kind.hostile() {
            self.age += 1;
        } else if self.age > 0 {
            self.age -= 1;
        }

        if self.love > 0 {
            self.love -= 1;
        }

        // NoAI: ни мысли, ни хода — и тяжести тоже: призванный в воздухе
        // висит, а толчок скорость копит, но не двигает (чёрный ящик,
        // cow-physics и cow-hit: корова NoAI висит на месте, скорость после
        // двух ударов 0,6).
        // Огонь: раз в 20 тактов на 1 (вики, Fire) — и у NoAI; а солнце
        // поджигает только мобов с ИИ (чёрный ящик, zombie-burn: зомби NoAI
        // в полдень цел).
        if self.kind.hostile() && hostile::burn(self, view, ctx, rng, !self.no_ai) {
            self.hurt(1.0, Source::OnFire, None);
        }

        // Кристалл Края стоит, где поставлен: ни тяжести, ни мыслей, ни голоса
        // (вики, End Crystal; чёрный ящик, end-crystal-explode: висит без сверок скорости).
        if matches!(self.kind, Kind::EndCrystal | Kind::LeashKnot) {
            return ai::Plan::default();
        }

        // Дракон: полёта по кругу у нас нет — висит на месте (фаза 10,
        // «hovering»), как призванный [проверить: ИИ боя только по вики]. Голос
        // в покое — и у NoAI (чёрный ящик, dragon-summon: NoAI рычит).
        if self.kind == Kind::EnderDragon {
            ai::voice(self, rng);
            dragon::think(self, ctx.players, rng);

            // Без ИИ кристаллы его не лечат (чёрный ящик, dragon-crystal-heal:
            // у NoAI-дракона ни луча, ни здоровья).
            if !self.no_ai {
                self.crystal_heal(ctx.mates);
            }

            return ai::Plan::default();
        }

        // Эффекты наконечников на прочих мобах; отдельные такты ниже не дублируем.
        if !self.vitals.effects.is_empty() && !matches!(self.kind, Kind::Witch | Kind::Villager | Kind::ZombieVillager | Kind::CaveSpider | Kind::Silverfish) {
            let effects = self.vitals.effects_tick();
            for (amount, source) in effects.hurt {
                self.hurt(amount, source, None);
            }
            if self.is_dead() {
                return ai::Plan::default();
            }
        }

        // Ведьма пьёт зелья и без ИИ: это её такт, а не мысль (чёрный ящик,
        // witch-stand: раненая NoAI-ведьма пьёт лечение). Эффекты — до питья:
        // выпитое лечение действует на следующем такте.
        if self.kind == Kind::Witch {
            self.vitals.effects_tick();
            hostile::witch_drink(self, view, ctx, rng);
        }

        // Вики, Silverfish: «Poison damage» будит соседей; пауки яд не носят.
        if matches!(self.kind, Kind::CaveSpider | Kind::Silverfish) {
            if self.kind.spider() {
                self.vitals.effects.retain(|effect| effect.kind != crate::survival::EffectKind::Poison);
            }

            for (amount, source) in self.vitals.effects_tick().hurt {
                self.hurt(amount, source, None);
            }

            if self.is_dead() {
                return ai::Plan::default();
            }
        }

        // Житель: эффекты (регенерация после нового уровня) и голос в покое —
        // и без ИИ (чёрный ящик, villager-trade: NoAI-житель подаёт голос).
        if self.kind == Kind::ZombieVillager {
            self.vitals.effects_tick();
        }

        // Разбойники: свечение от колокола проходит (raid.rs).
        if illager::raider(self.kind) && !self.vitals.effects.is_empty() {
            self.vitals.effects_tick();
        }

        if self.kind == Kind::Villager {
            self.vitals.effects_tick();

            if self.no_ai {
                ai::voice(self, rng);
            }
        }

        // Иссушитель без ИИ тоже подаёт голос (чёрный ящик, wither-half-armor:
        // entity.wither.ambient у NoAI).
        if self.kind == Kind::Wither && self.no_ai {
            ai::voice(self, rng);
        }

        if self.no_ai {
            // Запал крипера — не мысль, а его такт: подожжённый NoAI взрывается
            // (чёрный ящик, creeper-explode).
            let mut plan = ai::Plan::default();

            if self.kind == Kind::Creeper && (self.h.lit || self.h.fuse > 0) {
                hostile::swell(self, None, &mut plan);
            }

            return plan;
        }

        // Иссушитель: зарядка, головы, полёт (mob/wither.rs).
        if self.kind == Kind::Wither {
            return wither::tick(self, view, ctx, rng);
        }

        // Страйдер ходит по лаве, как по полу (вики, Strider): для него верх
        // лавового блока твёрд.
        if self.kind == Kind::Strider {
            nether::strider_tick(self, view);
        }

        let lava_floor = LavaFloor(view);
        let view: &dyn View = if self.kind == Kind::Strider { &lava_floor } else { view };

        let was_on_ground = self.on_ground;
        let mut plan = if matches!(self.kind, Kind::Wolf | Kind::Cat) {
            tame::think(self, view, ctx, rng)
        } else if golem::golem(self.kind) {
            golem::think(self, view, ctx, rng)
        } else if guardian::guardian(self.kind) {
            guardian::think(self, view, ctx, rng)
        } else {
            ai::think(self, view, ctx, rng)
        };

        // Пьющая ведьма стоит: модификатор «drinking» −0,25 к скорости 0,25
        // (чёрный ящик, witch-stand).
        if self.kind == Kind::Witch && self.h.drinking > 0 {
            plan.speed = 0.0;
        }

        // Шалкер сидит на грани: ни тяжести, ни шагов (вики, Shulker).
        if self.kind == Kind::Ghast && !self.is_dead() {
            nether::drift(self, view);
        } else if self.kind == Kind::Phantom && !self.is_dead() {
            phantom::fly(self, view);
        } else if guardian::guardian(self.kind) && !self.is_dead() {
            guardian::physics(self, view, rng);
        } else if self.kind == Kind::Vex && !self.is_dead() {
            illager::vex_fly(self);
        } else if self.kind != Kind::Shulker && !self.is_dead() {
            if self.kind == Kind::Blaze {
                let eye = self.h.target.and_then(|id| ctx.players.iter().find(|player| player.id == id)).map(|player| player.y + player.eye);

                nether::blaze_lift(self, eye);
            }

            self.physics(view, plan.speed, plan.jump);
        }

        if slime::cube(self.kind) && self.on_ground && !was_on_ground {
            slime::landed(self, rng);
        }

        plan
    }

    /// Движение живой сущности за такт (вики, Entity, «Motion»; чёрный
    /// ящик, cow-physics): сперва сдвиг на нынешнюю скорость (в первом такте
    /// падения она нулевая), потом тяжесть и сопротивление
    /// `v.y = (v.y − 0,08)·0,98`, трение `v.xz *= f·0,91`, где f —
    /// скользкость блока под ногами (в воздухе 1). Разгон по ходу — до сдвига:
    /// на земле `speed²·(0,6/f)³` (чёрный ящик, probe-cow-plain: у коровы
    /// 0,2 даёт 0,04 за такт — шаг 0,088 блока), в воздухе 0,02.
    pub fn physics(&mut self, view: &dyn View, speed: f64, jump: bool) {
        let effect_speed = (1.0 - 0.15 * f64::from(self.vitals.effect_level(crate::survival::EffectKind::Slowness))).max(0.0);
        let speed = speed * effect_speed;
        let (cell_x, cell_y, cell_z) = self.cell();
        let here = view.block(cell_x, cell_y, cell_z);
        let in_water = is_water(here);
        let in_lava = is_lava(here);

        // Паук лезет вверх, пока упирается в стену: 0,2 за такт (вики, Spider:
        // «can climb any solid block»; скорость лестницы) [проверить].
        if self.kind.spider() {
            self.h.climbing = self.wall && speed != 0.0;

            // Лезет: перед сдвигом скорость — 0,2 после тяжести и сопротивления,
            // то есть (0,2 − 0,08)·0,98 = 0,1176 за такт, и клиентам уходит та
            // же (чёрный ящик, spider-climb: сдвиг 1445/4096 за три такта).
            if self.h.climbing {
                self.vy = (0.2 - GRAVITY) * VERTICAL_DRAG;
            }
        }

        if jump && self.on_ground {
            // Прыжок: 0,42 вверх (вики, Jumping: «0.42»).
            self.vy = if self.kind == Kind::MagmaCube { 0.42 + 0.1 * f64::from(self.nether.size) } else { 0.42 };
        }

        // Утопленник плывёт к цели в воде: тяга `SWIM_THRUST · скорость` вперёд и
        // `скорость · подъём · 0,1` по вертикали, сопротивление 0,9, тяжести нет
        // (вики, Drowned; чёрный ящик, drowned-attack: скорость 0,02–0,04) [проверить].
        let swimming = self.kind == Kind::Drowned && in_water && self.h.swim && speed != 0.0;

        let slip = if self.on_ground { slipperiness(view.block(cell_x, (self.y - 0.5000001).floor() as i32, cell_z)) } else { 1.0 };
        let friction = if self.on_ground { slip * 0.91 } else { 0.91 };

        if swimming {
            let effective = speed * self.kind.speed();
            let rad = f64::from(self.yaw).to_radians();

            self.vx += -rad.sin() * SWIM_THRUST * effective;
            self.vz += rad.cos() * SWIM_THRUST * effective;
            self.vy += effective * self.h.swim_rise * 0.1;
            self.vitals.fall = 0.0;
            self.step(view, self.vx, self.vy, self.vz);
            self.vx *= 0.9;
            self.vy *= 0.9;
            self.vz *= 0.9;

            return;
        }

        // Зомби и родня идут по воде с тягой `0,02 · скорость` — около 0,023
        // блока за такт на дне (чёрный ящик, drowned-attack: сдвиг 282/4096 за 3 такта).
        let wades = in_water && matches!(self.kind, Kind::Zombie | Kind::ZombieVillager | Kind::Husk | Kind::Drowned);

        if speed != 0.0 {
            let effective = speed * if slime::cube(self.kind) { slime::speed(self.nether.size) } else { self.kind.speed() };
            // Куб: в воздухе 0,01·ввод — замер slime-hop (оригинал, три записи): прыжок 0,95 блока [проверить].
            let accel = if wades {
                0.02 * effective
            } else if self.on_ground {
                effective * effective * (0.6 / slip).powi(3)
            } else if slime::cube(self.kind) {
                0.01 * effective
            } else {
                0.02
            };
            let rad = f64::from(self.yaw).to_radians();

            self.vx += -rad.sin() * accel;
            self.vz += rad.cos() * accel;
        } else if self.strafe != (0.0, 0.0) {
            // Шаг вбок и вперёд-назад: тело смотрит на цель, идёт боком.
            let (forward, left) = self.strafe;
            let length = (forward * forward + left * left).sqrt();
            let effective = length * self.kind.speed() * effect_speed;
            let accel = if self.on_ground { effective * effective * (0.6 / slip).powi(3) } else { 0.02 };
            let rad = f64::from(self.yaw).to_radians();
            let (fx, fz) = (-rad.sin(), rad.cos());
            let (lx, lz) = (fz, -fx);

            self.vx += (fx * forward + lx * left) / length * accel;
            self.vz += (fz * forward + lz * left) / length * accel;
        }

        let before = (self.x, self.z);

        if in_water || in_lava {
            self.vitals.fall = 0.0;
            self.step(view, self.vx, self.vy, self.vz);

            let drag = if in_lava { 0.5 } else { 0.8 };

            self.vx *= drag;
            self.vz *= drag;
            self.vy = self.vy * drag - 0.02;

            // «Float»: животное всплывает (вики, Mob AI: «Float» — плавать);
            // зомби, кадавр и утопленник тонут и ходят по дну (вики, Drowned; чёрный ящик,
            // zombie-to-drowned: зомби в воде стоит на дне).
            if !in_lava && self.vy < 0.04 && !matches!(self.kind, Kind::Zombie | Kind::ZombieVillager | Kind::Husk | Kind::Drowned) {
                self.vy += 0.04;
            }

            return;
        }

        self.step(view, self.vx, self.vy, self.vz);

        // Шаги: звук раз в 1/0,6 пройденного по земле (чёрный ящик,
        // probe-cow-plain: громкость 0,15, шаг раз в 1,6 блока).
        // Куб считает и полёт: порог набегает в прыжке, а звук играет при
        // приземлении, перед «squish» (чёрный ящик, slime-hop: звук блока
        // камня 0,15 на 59-м, 111-м, 137-м тактах — примерно через раз).
        let cube = slime::cube(self.kind);

        if self.on_ground || cube {
            self.move_dist += ((self.x - before.0).powi(2) + (self.z - before.1).powi(2)).sqrt() * 0.6;

            if self.on_ground && self.move_dist > self.next_step {
                self.next_step = self.move_dist.floor() + 1.0;

                // [проверить] У куба звук шага берётся у блока под ногами; пока только камень.
                let name = if cube { "minecraft:block.stone.step" } else { self.kind.step_sound() };

                if !name.is_empty() {
                    // Железный голем топает громко: 1,0 (чёрный ящик, iron-golem-attack).
                    let volume = if self.kind == Kind::IronGolem { 1.0 } else { 0.15 };

                    self.events.push(Event::Sound { name, volume, pitch: 1.0 });
                }
            }
        }

        if !(self.kind.spider() && self.h.climbing) {
            self.vy = (self.vy - GRAVITY) * VERTICAL_DRAG;
        }

        // Курица падает медленно (вики, Chicken: «slow falling»): в воздухе
        // падение после тяжести замедляется ещё в 0,6 — предельная скорость
        // −0,114 (чёрный ящик, chicken-fall).
        if self.kind == Kind::Chicken && !self.on_ground && self.vy < 0.0 {
            self.vy *= 0.6;
        }

        self.vx *= friction;
        self.vz *= friction;

        if self.vx.abs() < 0.003 {
            self.vx = 0.0;
        }

        if self.vz.abs() < 0.003 {
            self.vz = 0.0;
        }
    }

    /// Сдвиг с упором в блоки: сперва по вертикали, потом по большей из
    /// горизонтальных осей (вики, Entity: порядок осей y, x, z). Сдвиг вбок,
    /// упёршийся в ступеньку ниже 0,6, поднимает ящик на неё.
    pub fn step(&mut self, view: &dyn View, dx: f64, dy: f64, dz: f64) {
        let was_on_ground = self.on_ground;
        let (width, height) = self.size();
        let half = width / 2.0;
        let start = (self.x - half, self.y, self.z - half);

        // Вертикаль, потом горизонталь: по большей оси первой.
        let mut at = start;
        let mut moved_y = clip_y(view, at, half, height, dy);

        at.1 += moved_y;

        let (mut moved_x, mut moved_z) = slide(view, &mut at, half, height, dx, dz);
        let landed = dy < 0.0 && (moved_y - dy).abs() > 1.0e-9;
        let bumped = (moved_x - dx).abs() > 1.0e-9 || (moved_z - dz).abs() > 1.0e-9;

        // Ступенька: упёрлись, стоя на земле, — поднимаемся на STEP_HEIGHT и
        // проходим тот же путь, потом опускаемся до опоры [проверить: у
        // оригинала так же].
        if bumped && (was_on_ground || landed) {
            let lift = clip_y(view, start, half, height, STEP_HEIGHT);
            let mut up = (start.0, start.1 + lift, start.2);
            let (ux, uz) = slide(view, &mut up, half, height, dx, dz);
            let down = clip_y(view, up, half, height, dy.min(0.0) - lift);

            if ux * ux + uz * uz > moved_x * moved_x + moved_z * moved_z + 1.0e-12 {
                at = (up.0, up.1 + down, up.2);
                moved_x = ux;
                moved_z = uz;
                moved_y = lift + down;
            }
        }

        self.x = at.0 + half;
        self.y = at.1;
        self.z = at.2 + half;

        self.wall = (moved_x - dx).abs() > 1.0e-9 || (moved_z - dz).abs() > 1.0e-9;

        // Упёрлись в стену — по этой оси скорость гасится.
        if (moved_x - dx).abs() > 1.0e-9 {
            self.vx = 0.0;
        }

        if (moved_z - dz).abs() > 1.0e-9 {
            self.vz = 0.0;
        }

        // Упор вниз — на земле; вверх — головой.
        let down_hit = dy < 0.0 && (moved_y - dy).abs() > 1.0e-9;

        if down_hit {
            self.on_ground = true;
            self.vy = 0.0;
        } else {
            self.on_ground = false;

            if dy > 0.0 && (moved_y - dy).abs() > 1.0e-9 {
                self.vy = 0.0;
            }
        }

        // Высота падения копится по пройденному вниз, и такт приземления
        // в счёт урона (чёрный ящик, cow-physics: с шести блоков — урон 3)
        // (вики, Fall damage); для частиц он не в счёте — берётся высота до него.
        let before = self.vitals.fall;

        if moved_y < 0.0 {
            self.vitals.fall += (-moved_y) as f32;
        }

        if !self.on_ground {
            return;
        }

        let fall = self.vitals.fall;

        self.vitals.fall = 0.0;

        if fall <= 0.0 {
            return;
        }

        // Частицы приземления: та же формула, что у игрока (survival::
        // landing_particles; чёрный ящик probe-land-cow, probe-land-chicken).
        let below = view.block(self.x.floor() as i32, (self.y - 0.001).floor() as i32, self.z.floor() as i32);
        let count = crate::survival::landing_particles(before);

        if count > 0 && blocks::block_at_state(below).is_some_and(|name| !name.ends_with("air")) {
            self.events.push(Event::Landing { block: below, count });
        }

        // Урон падения: ceil(высота − 3) (вики, Fall damage); курица не
        // разбивается (вики, Chicken).
        // Големы падения не боятся (вики, Iron Golem, Snow Golem).
        if fall > crate::survival::SAFE_FALL && self.kind != Kind::Chicken && !golem::golem(self.kind) {
            let damage = (fall - crate::survival::SAFE_FALL - 1.0e-4).ceil();

            self.hurt(damage, Source::Fall, None);
        }
    }

    /// Удар по части дракона: в голову — полный, в прочие части — четверть
    /// и ещё до одного очка (вики, Ender Dragon: «reduces all incoming damage
    /// to it by ~75% unless the attack is hitting its head»; формула
    /// `damage/4 + min(1, damage)`) [проверить: у оригинала NoAI-дракона
    /// ударить нельзя — части стоят в нуле мира, сверить не удалось].
    pub fn hurt_part(&mut self, amount: f32, head: bool, source: Source, push: Option<Push>) -> Option<Hit> {
        let amount = if head { amount } else { amount / 4.0 + amount.min(1.0) };

        self.hurt(amount, source, push)
    }

    /// Удар. `push` — кто и как толкнул (None — урон без толчка: падение,
    /// команда). Возвращает исход или None, если удар не прошёл (вики,
    /// Damage: «Invulnerability frames»).
    pub fn hurt(&mut self, amount: f32, source: Source, push: Option<Push>) -> Option<Hit> {
        if self.is_dead() {
            return None;
        }

        // Кристалл: любой урон — взрыв, даже нулевой (вики, End Crystal:
        // «explode when attacked or damaged in most ways, even by attacks that
        // normally do zero damage»). Самого взрыва здесь нет: его делает
        // ударивший (strike_mob) или такт (tick::detonate_crystals), по `exploding`.
        if self.kind == Kind::EndCrystal {
            self.exploding = true;
            return None;
        }

        // Дракону вредят только игроки и взрывы (вики, Ender Dragon: «Only
        // explosions and players deal damage»; чёрный ящик, dragon-die:
        // `damage … minecraft:generic` ему нипочём).
        if self.kind == Kind::EnderDragon && !matches!(source, Source::PlayerAttack | Source::Arrow | Source::Explosion | Source::PlayerExplosion | Source::GenericKill) {
            return None;
        }

        // Снаряд шалкера гаснет от любого удара (вики, Shulker Bullet:
        // «destroyed when it is hit with any item»).
        if self.kind == Kind::ShulkerBullet {
            self.h.despawn = true;
            self.events.push(Event::Sound { name: "minecraft:entity.shulker_bullet.hurt", volume: 1.0, pitch: 1.0 });
            return None;
        }

        // Иссушитель: в зарядке неуязвим, иссушению и воде не поддаётся (mob/wither.rs).
        if self.kind == Kind::Wither && wither::ignores(self, source) {
            return None;
        }

        // Ведьма: магия на 85 % слабее (вики, Witch).
        let amount = if self.kind == Kind::Witch && source == Source::Magic { amount * 0.15 } else { amount };
        let before = self.vitals.health;
        let amount = if armored(source) { after_armor(amount, self.armor()) } else { amount };
        let mut hit = self.vitals.hurt(amount, source, false)?;

        // Снято на самом деле — не больше оставшегося здоровья.
        hit.taken = hit.taken.min(before);

        if self.kind == Kind::Wither {
            wither::on_hurt(self);
        }

        if self.kind == Kind::EnderDragon {
            let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0xD4A);

            dragon::on_hurt(self, hit.taken, push.map(|push| push.cause).filter(|cause| *cause != 0), &mut rng);
        }

        if self.kind == Kind::Silverfish && !hit.died && self.h.silverfish_call == 0
            && (arthropod::calls_for_help(source) || push.is_some_and(|push| push.cause != 0))
        {
            self.h.silverfish_call = arthropod::CALL_DELAY;
        }

        self.events.push(Event::Hurt { source, cause: push.map_or(0, |push| push.cause), at: (self.x, self.y, self.z) });

        // Ударили зомби-пиглина: злится и зовёт стаю (вики, Zombified Piglin).
        if self.kind == Kind::ZombifiedPiglin && push.is_some_and(|push| push.cause != 0) && !self.no_ai {
            let mut anger = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks.wrapping_mul(0x51));

            self.nether.anger = nether::anger_ticks(&mut anger);
            self.nether.alert = true;
        }

        // Ударил игрок: враждебный мстит ему (паук нападает и при свете).
        if let Some(push) = push.filter(|push| push.cause != 0) {
            self.by_player = matches!(source, Source::PlayerAttack | Source::MaceSmash);

            // Кто ударил разбойника — для героя деревни (raid.rs).
            if self.by_player {
                self.illager.hit_by = push.cause;
            }

            if self.kind.hostile() && !self.no_ai {
                self.h.target = Some(push.cause);
                self.h.provoked = true;
            }

            // Волк мстит и встаёт, кошка вскакивает (mob/tame.rs).
            tame::on_hurt(self, push.cause);

            // Железный голем мстит (mob/golem.rs).
            golem::on_hurt(self, push.cause, self.by_player);
        }

        // Всякий урон — «толчок» для слежения: скорость уходит клиентам вне
        // очереди, даже нулевая (чёрный ящик, cow-physics: падение).
        self.impulses += 1;

        if let Some(push) = push {
            self.knockback(push.dir.0, push.dir.1, push.strength);
        }

        // Эндермен от всякого урона прыгает прочь (вики, Enderman); шалкер —
        // ниже половины здоровья с шансом 25 % (вики, Shulker).
        if !self.no_ai && !hit.died {
            match self.kind {
                Kind::Enderman => self.h.want_teleport = true,
                Kind::Shulker if self.vitals.health < self.kind.max_health() / 2.0 => {
                    self.h.want_teleport = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0x5B).below(4) == 0;
                }
                _ => {}
            }
        }

        if hit.died {
            self.die();
            golem::crack_sound(self, before);
        } else {
            let (_, hurt, _) = self.sounds();
            let hurt = if self.kind == Kind::Slime { slime::sound(self.nether.size, "hurt") } else { hurt };
            let hurt = if self.kind == Kind::Shulker && self.h.peek == 0 { "minecraft:entity.shulker.hurt_closed" } else { hurt };

            self.events.push(Event::Sound { name: hurt, volume: self.sound_volume(), pitch: self.voice_pitch() });
            golem::crack_sound(self, before);

            // Паника — только от удара существа: падение и команда не пугают
            // (чёрный ящик, cow-physics: упавшая корова стоит).
            if let Some(push) = push.filter(|push| push.cause != 0 && !self.kind.hostile() && self.kind != Kind::Wolf && !golem::golem(self.kind)) {
                self.fright = Some((self.x - push.dir.0 * 8.0, self.z - push.dir.1 * 8.0));
                self.brain.panic(PANIC_TICKS);
            }
        }

        Some(hit)
    }

    /// Броня: у шалкера закрытого — 20, открытого — 0 (вики, Shulker). Броню
    /// даёт закрытие после открытия (`covered`): призванный закрытым без брони
    /// (чёрный ящик, shulker-stand: NoAI-шалкер теряет от меча 7 из 30).
    pub fn armor(&self) -> f32 {
        match self.kind {
            Kind::Shulker if self.h.covered => 20.0,
            Kind::MagmaCube => nether::cube_armor(self.nether.size),
            kind => kind.armor(),
        }
    }

    /// Толчок: скорость делится пополам и складывается с ударом; вверх
    /// не больше 0,4 (вики, Knockback).
    pub fn knockback(&mut self, ux: f64, uz: f64, strength: f64) {
        // Шалкер не сдвигается (чёрный ящик, shulker-stand: скорость после
        // удара мечом — 0).
        // Железный голем не сдвигается: сопротивление отдаче 100 % (вики, Iron
        // Golem; чёрный ящик, iron-golem-heal: скорость после удара 0).
        if matches!(self.kind, Kind::Shulker | Kind::IronGolem) {
            return;
        }

        // Хоглин держит отдачу: чёрный ящик, hoglin-stand — скорость после
        // удара 0,16 вместо 0,4, то есть сопротивление 0,6.
        let strength = if self.kind == Kind::Hoglin { strength * 0.4 } else { strength };

        self.vx = self.vx / 2.0 + ux * strength;
        self.vz = self.vz / 2.0 + uz * strength;

        if self.on_ground {
            self.vy = (self.vy / 2.0 + strength).min(0.4);
        }

    }

    /// Громкость голоса: у коровы 0,4, у прочих 1 (чёрный ящик: голос
    /// коровы при смерти — 0,4).
    pub fn sound_volume(&self) -> f32 {
        match self.kind {
            Kind::Cow => 0.4,
            Kind::Slime => 0.4 * self.nether.size as f32,
            // Чёрный ящик, dragon-summon: голос дракона громкостью 5,00.
            Kind::EnderDragon => 5.0,
            _ => 1.0,
        }
    }

    /// Высота голоса: `1 + (случайное − случайное)·0,2`, у детёныша на 0,5
    /// выше (вики, Sounds: «babies have higher-pitched sounds»; чёрный
    /// ящик, cow-hit: голос коровы 0,97–1,13).
    pub fn voice_pitch(&self) -> f32 {
        let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks.wrapping_mul(0x9E37) ^ ((self.events.len() as u64) << 40));
        let spread = ((rng.unit() - rng.unit()) * 0.2) as f32;

        if self.is_baby() { 1.5 + spread } else { 1.0 + spread }
    }

    /// Смерть: клиентам — событие, миру — добыча.
    pub fn die(&mut self) {
        if self.dead_for > 0 {
            return;
        }

        self.dead_for = 1;
        let (_, _, death) = self.sounds();
        let death = if self.kind == Kind::Slime { slime::sound(self.nether.size, "death") } else { death };
        let drops = loot(self, &mut Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0x5EED));

        self.events.push(Event::Sound { name: death, volume: self.sound_volume(), pitch: self.voice_pitch() });

        // Опыт: только убитый игроком, не детёныш; корова, свинья, овца,
        // курица — 1–3 (вики, «Drops»); враждебный — 5 и ещё 1–3 за каждую
        // вещь снаряжения (вики, Experience, «Killing mobs»; чёрный ящик:
        // зомби — шарики 3+1+1, скелет с луком — 3+3).
        // Дракон даёт опыт всегда — и убитый командой [проверить]; за первого
        // в бою Края больше (ставит бой, tick.rs).
        if self.kind == Kind::EnderDragon {
            // Шарики — по ходу анимации смерти (tick.rs, dragon_xp).
            self.dragon.xp = if self.first_dragon { DRAGON_XP_FIRST } else { DRAGON_XP };
        } else if self.xp_until > self.ticks && !self.is_baby() {
            let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0xE4B);

            self.pending_xp = if slime::cube(self.kind) {
                // Вики, Magma Cube: опыт равен размеру (чёрный ящик, magma-cube-stand).
                self.nether.size
            } else if guardian::guardian(self.kind) {
                guardian::XP
            } else if self.kind == Kind::Wither {
                wither::XP
            } else if self.kind.hostile() {
                let worn = u32::from(self.h.bow) + u32::from(self.h.helmet);

                5 + (0..worn).map(|_| crate::survival::passive_mob_xp(rng.unit() as f32)).sum::<i32>()
            } else {
                crate::survival::passive_mob_xp(rng.unit() as f32)
            };
        }

        if !drops.is_empty() {
            self.events.push(Event::Loot(drops));
        }

        self.events.push(Event::Death);
    }

    /// Пропал ли труп: 20 тактов после смерти.
    pub fn gone(&self) -> bool {
        self.dead_for > if self.kind == Kind::EnderDragon { DRAGON_DEATH_TICKS } else { DEATH_TICKS }
    }

    /// Запись для файла региона: вид, место, скорость не пишется, здоровье,
    /// возраст, любовь, признаки, цвет, яйцо.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.push(match self.kind {
            Kind::Cow => 0,
            Kind::Pig => 1,
            Kind::Sheep => 2,
            Kind::Chicken => 3,
            Kind::Zombie => 4,
            Kind::Skeleton => if self.h.water_ticks != 0 || self.h.converting != 0 { 63 } else { 5 },
            Kind::Stray => 61,
            Kind::Bogged => 62,
            Kind::Creeper => 6,
            Kind::Spider => 7,
            Kind::CaveSpider => 31,
            Kind::Silverfish => 32,
            Kind::EndCrystal => 8,
            Kind::EnderDragon => 9,
            Kind::Enderman => 10,
            Kind::Shulker => 11,
            Kind::ShulkerBullet => 12,
            Kind::Witch => 13,
            Kind::Villager => 14,
            Kind::ZombieVillager => 15,
            Kind::Blaze => 16,
            Kind::WitherSkeleton => 17,
            Kind::MagmaCube => 18,
            Kind::Ghast => 19,
            Kind::ZombifiedPiglin => 20,
            Kind::Piglin => 21,
            Kind::PiglinBrute => 22,
            Kind::Hoglin => 23,
            Kind::Strider => 24,
            Kind::Drowned => 25,
            Kind::Husk => 26,
            Kind::Slime => 27,
            Kind::Wolf => 28,
            Kind::Cat => 29,
            Kind::LeashKnot => 30,
            Kind::IronGolem => 33,
            Kind::SnowGolem => 34,
            Kind::Pillager => 64,
            Kind::Vindicator => 65,
            Kind::Evoker => 66,
            Kind::Vex => 67,
            Kind::Illusioner => 68,
            Kind::Ravager => 69,
            Kind::EvokerFangs => 70,
            Kind::Phantom => 35,
            Kind::Guardian => 36,
            Kind::ElderGuardian => 37,
            // Номер с запасом: другие новые мобы берут 38 и дальше.
            Kind::Wither => 60,
            // Зимогор, болотник и скелет-замерзающий (хвост с таймерами) — следом.
        } | if self.name.is_some() || self.leash.is_some() || self.persistent_tag { EXTENDED } else { 0 });
        out.extend_from_slice(&self.x.to_be_bytes());
        out.extend_from_slice(&self.y.to_be_bytes());
        out.extend_from_slice(&self.z.to_be_bytes());
        out.extend_from_slice(&self.yaw.to_be_bytes());
        out.extend_from_slice(&self.vitals.health.to_be_bytes());
        out.extend_from_slice(&self.age.to_be_bytes());
        out.extend_from_slice(&self.love.to_be_bytes());
        out.push(u8::from(self.no_ai) | (u8::from(self.sheared) << 1) | (u8::from(self.h.helmet) << 2) | (u8::from(self.h.lit) << 3) | (u8::from(self.h.bow) << 4) | (self.h.hand << 5) | (u8::from(self.h.offhand_shell) << 7));
        out.push(self.color);
        out.extend_from_slice(&self.egg_timer.to_be_bytes());

        // Хвост по виду: блок в руках эндермена (−1 — пусто), грань и цвет шалкера.
        match self.kind {
            Kind::Slime => {
                out.extend_from_slice(&self.nether.size.to_be_bytes());
                out.push(u8::from(self.silent) | (u8::from(self.nether.persistence_required) << 1));
            }
            Kind::Enderman => out.extend_from_slice(&self.h.carried.unwrap_or(-1).to_be_bytes()),
            Kind::Shulker => out.extend_from_slice(&[self.h.face, self.h.shell_color]),
            Kind::Villager | Kind::ZombieVillager => crate::villager::write(&self.villager, out),
            Kind::Wolf | Kind::Cat => tame::write(&self.tame, out),
            Kind::IronGolem | Kind::SnowGolem => golem::write(&self.golem, out),
            _ => {}
        }

        if self.kind == Kind::Skeleton && (self.h.water_ticks != 0 || self.h.converting != 0) {
            out.extend_from_slice(&self.h.water_ticks.to_be_bytes());
            out.extend_from_slice(&self.h.converting.to_be_bytes());
        }

        // Общий хвост (вид с битом `EXTENDED`): имя с бирки и поводок (mob/leash.rs).
        if self.name.is_some() || self.leash.is_some() || self.persistent_tag {
            leash::write_tail(self, out);
        }
    }

    /// Читает запись `write`; None — она испорчена.
    pub fn read(bytes: &mut &[u8]) -> Option<Mob> {
        let mut take = |count: usize| -> Option<&[u8]> {
            let (head, rest) = bytes.split_at_checked(count)?;
            *bytes = rest;
            Some(head)
        };

        let head = take(1)?[0];
        let extended = head & EXTENDED != 0;
        let kind = match head & !EXTENDED {
            0 => Kind::Cow,
            1 => Kind::Pig,
            2 => Kind::Sheep,
            3 => Kind::Chicken,
            4 => Kind::Zombie,
            5 | 63 => Kind::Skeleton,
            61 => Kind::Stray,
            62 => Kind::Bogged,
            6 => Kind::Creeper,
            7 => Kind::Spider,
            31 => Kind::CaveSpider,
            32 => Kind::Silverfish,
            8 => Kind::EndCrystal,
            9 => Kind::EnderDragon,
            10 => Kind::Enderman,
            11 => Kind::Shulker,
            12 => Kind::ShulkerBullet,
            13 => Kind::Witch,
            14 => Kind::Villager,
            15 => Kind::ZombieVillager,
            16 => Kind::Blaze,
            17 => Kind::WitherSkeleton,
            18 => Kind::MagmaCube,
            19 => Kind::Ghast,
            20 => Kind::ZombifiedPiglin,
            21 => Kind::Piglin,
            22 => Kind::PiglinBrute,
            23 => Kind::Hoglin,
            24 => Kind::Strider,
            25 => Kind::Drowned,
            26 => Kind::Husk,
            27 => Kind::Slime,
            28 => Kind::Wolf,
            29 => Kind::Cat,
            30 => Kind::LeashKnot,
            33 => Kind::IronGolem,
            34 => Kind::SnowGolem,
            64 => Kind::Pillager,
            65 => Kind::Vindicator,
            66 => Kind::Evoker,
            67 => Kind::Vex,
            68 => Kind::Illusioner,
            69 => Kind::Ravager,
            70 => Kind::EvokerFangs,
            35 => Kind::Phantom,
            36 => Kind::Guardian,
            37 => Kind::ElderGuardian,
            60 => Kind::Wither,
            _ => return None,
        };
        let x = f64::from_be_bytes(take(8)?.try_into().ok()?);
        let y = f64::from_be_bytes(take(8)?.try_into().ok()?);
        let z = f64::from_be_bytes(take(8)?.try_into().ok()?);
        let yaw = f32::from_be_bytes(take(4)?.try_into().ok()?);
        let health = f32::from_be_bytes(take(4)?.try_into().ok()?);
        let age = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let love = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let flags = take(1)?[0];
        let color = take(1)?[0];
        let egg_timer = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let carried = if kind == Kind::Enderman { Some(i32::from_be_bytes(take(4)?.try_into().ok()?)).filter(|state| *state >= 0) } else { None };
        let shell = if kind == Kind::Shulker { take(2)?.to_vec() } else { vec![0, 16] };
        let slime_data = if kind == Kind::Slime { Some((i32::from_be_bytes(take(4)?.try_into().ok()?), take(1)?[0])) } else { None };
        let villager = if matches!(kind, Kind::Villager | Kind::ZombieVillager) { Some(crate::villager::read(bytes)?) } else { None };
        let tamed = if matches!(kind, Kind::Wolf | Kind::Cat) { Some(tame::read(bytes)?) } else { None };
        let golem_state = if golem::golem(kind) { Some(golem::read(bytes)?) } else { None };
        let snow_state = if head & !EXTENDED == 63 { Some(skeleton::read_snow(bytes)?) } else { None };
        let tail = if extended { Some(leash::read_tail(bytes)?) } else { None };

        if !(x.is_finite() && y.is_finite() && z.is_finite() && health.is_finite()) || color > 15 {
            return None;
        }

        let mut mob = Mob::new(0, kind, (x, y, z));

        mob.yaw = yaw;
        mob.head_yaw = yaw;
        if let Some((size, flags)) = slime_data {
            slime::set_size(&mut mob, size);
            mob.silent = flags & 1 != 0;
            mob.nether.persistence_required = flags & 2 != 0;
        }
        mob.vitals.health = health.clamp(0.5, if kind == Kind::Wolf { tame::TAME_HEALTH } else { mob.vitals.max_health });
        mob.age = age;
        mob.love = love.max(0);
        mob.no_ai = flags & 1 != 0;
        mob.sheared = flags & 2 != 0;
        mob.h.helmet = flags & 4 != 0;
        mob.h.lit = flags & 8 != 0;
        mob.h.bow = flags & 16 != 0;
        mob.h.hand = (flags >> 5) & 3;
        mob.h.offhand_shell = flags & 128 != 0;
        mob.color = color;
        mob.egg_timer = egg_timer;
        if let Some((ticks, converting)) = snow_state {
            mob.h.water_ticks = ticks;
            mob.h.converting = converting;
        }
        mob.h.carried = carried;
        mob.h.face = shell[0].min(5);
        mob.h.shell_color = shell[1].min(16);

        if let Some(villager) = villager {
            mob.villager = villager;
        }

        if let Some(tamed) = tamed {
            tame::apply_read(&mut mob, tamed);
        }

        if let Some(state) = golem_state {
            mob.golem = state;
        }

        if let Some(tail) = tail {
            leash::apply_tail(&mut mob, tail);
        }

        // Снаряд без цели не живёт: прочитанный из файла гаснет.
        if kind == Kind::ShulkerBullet {
            mob.persistent = false;
        }

        Some(mob)
    }
}

impl PartialEq for Mob {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.x == other.x
            && self.y == other.y
            && self.z == other.z
            && self.vitals.health == other.vitals.health
            && self.age == other.age
            && self.no_ai == other.no_ai
            && self.sheared == other.sheared
            && self.color == other.color
    }
}

/// Выпадающее после смерти (вики: Cow, Pig, Sheep, Chicken, «Drops»):
/// корова — 0–2 кожи и 1–3 говядины, свинья — 1–3 свинины, овца — шерсть
/// своего цвета (если не острижена) и 1–2 баранины, курица — 0–2 пера и
/// курятина. Детёныши ничего не роняют. Горящие роняют жареное [проверить:
/// огня у мобов ещё нет].
pub fn loot(mob: &Mob, rng: &mut Rng) -> Vec<Stack> {
    if (mob.is_baby() && !mob.kind.hostile()) || mob.kind.object() {
        return Vec::new();
    }

    let mut drops = Vec::new();
    let mut give = |name: &str, low: i32, high: i32, rng: &mut Rng| {
        let count = low + rng.below((high - low + 1) as u32) as i32;

        // Зловещие флаг и бутылка — с довесками (mob/illager.rs, captain_loot).
        let (name, data) = match name.split_once(':') {
            Some(("ominous_bottle", level)) => ("ominous_bottle", crate::stackdata::Data { ominous: level.parse().ok(), ..Default::default() }),
            _ if name == "ominous_banner" => ("white_banner", crate::stackdata::Data { ominous_banner: true, ..Default::default() }),
            _ => (name, crate::stackdata::Data::default()),
        };

        if let (Some(item), true) = (blocks::item_named(name), count > 0) {
            let mut stack = Stack::new(item, count);

            stack.data = crate::stackdata::intern(data);
            drops.push(stack);
        }
    };

    match mob.kind {
        Kind::Cow => {
            give("leather", 0, 2, rng);
            give("beef", 1, 3, rng);
        }
        Kind::Pig => give("porkchop", 1, 3, rng),
        Kind::Sheep => {
            if !mob.sheared {
                give(WOOL[usize::from(mob.color) % 16], 1, 1, rng);
            }

            give("mutton", 1, 2, rng);
        }
        Kind::Chicken => {
            give("feather", 0, 2, rng);
            give("chicken", 1, 1, rng);
        }
        // Вики: зомби — 0–2 гнилой плоти, скелет — 0–2 костей и 0–2 стрел,
        // крипер — 0–2 пороха, паук — 0–2 нити и 0–1 паучий глаз (глаз — только
        // если убил игрок).
        Kind::Zombie | Kind::Husk => give("rotten_flesh", 0, 2, rng),
        // Вики, Drowned: 0–2 гнилой плоти, медный слиток (11 %, +2 % за уровень
        // «Добычи», только от игрока) [проверить: шанс], вещь из основной руки по
        // её шансу и ракушка из второй всегда.
        Kind::Drowned => {
            give("rotten_flesh", 0, 2 + mob.looting, rng);

            if mob.by_player && rng.below(100) < 11 + 2 * mob.looting as u32 {
                give("copper_ingot", 1, 1, rng);
            }

            if let Some(item) = hostile::hand_item(mob.h.hand).and_then(blocks::item_named)
                && rng.unit() < f64::from(mob.h.hand_drop)
            {
                drops.push(Stack::new(item, 1));
            }

            if mob.h.offhand_shell && let Some(item) = blocks::item_named("nautilus_shell") {
                drops.push(Stack::new(item, 1));
            }
        }
        // Вики, Zombie Villager: гнилая плоть и редко железо, морковь, картофель
        // [проверить: шансы редкого].
        Kind::ZombieVillager => give("rotten_flesh", 0, 2, rng),
        Kind::Skeleton | Kind::Stray | Kind::Bogged => return skeleton::loot(mob, rng),
        Kind::Creeper => give("gunpowder", 0, 2, rng),
        Kind::CaveSpider => {
            give("string", 0, 2 + mob.looting, rng);

            if mob.by_player || mob.xp_until > mob.ticks {
                // Вики, Cave Spider: «-1–1», с Looting верхняя граница растёт.
                give("spider_eye", -1, 1 + mob.looting, rng);
            }
        }
        Kind::Silverfish => {}
        Kind::Spider => {
            give("string", 0, 2, rng);

            if mob.by_player {
                give("spider_eye", 0, 1, rng);
            }
        }
        // Вики, Blaze: 0–1 стержня ифрита (только от игрока; «Looting» +1 за уровень).
        Kind::Blaze => {
            if mob.by_player {
                give("blaze_rod", 0, 1 + mob.looting, rng);
            }
        }
        // Вики, Wither Skeleton: 0–2 костей, 0–1 угля (+ «Looting»); череп 2,5 %
        // (+1 % за уровень) только от игрока.
        Kind::WitherSkeleton => {
            give("bone", 0, 2 + mob.looting, rng);
            give("coal", 0, 1 + mob.looting, rng);

            if mob.by_player && rng.below(1000) < 25 + 10 * mob.looting as u32 {
                give("wither_skeleton_skull", 1, 1, rng);
            }
        }
        Kind::Slime => {
            if mob.nether.size == 1 {
                let count = slime::slimeballs(mob.looting, rng);
                give("slime_ball", count, count, rng);
            }
        }
        // Вики, Magma Cube: 0–1 магмового сгустка у кубов размера 2 и больше (+ «Looting»).
        Kind::MagmaCube => {
            if mob.nether.size > 1 {
                give("magma_cream", 0, 1 + mob.looting, rng);
            }
        }
        // Вики, Ghast: 0–1 слезы гаста и 0–2 пороха (+ «Looting»).
        Kind::Ghast => {
            give("ghast_tear", 0, 1 + mob.looting, rng);
            give("gunpowder", 0, 2 + mob.looting, rng);
        }
        // Вики, Zombified Piglin: 0–1 гнилой плоти, 0–1 золотого самородка,
        // золотой слиток 2,5 % (только от игрока).
        Kind::ZombifiedPiglin => {
            give("rotten_flesh", 0, 1 + mob.looting, rng);
            give("gold_nugget", 0, 1 + mob.looting, rng);

            if mob.by_player && rng.below(1000) < 25 + 10 * mob.looting as u32 {
                give("gold_ingot", 1, 1, rng);
            }
        }
        // Вики, Piglin/Piglin Brute: сами ничего не роняют, только то, что держат.
        Kind::Piglin | Kind::PiglinBrute => {
            for stack in &mob.nether.held {
                drops.push(*stack);
            }
        }
        // Вики, Hoglin: 2–4 сырой свинины, 0–1 кожи.
        Kind::Hoglin => {
            give("porkchop", 2, 4 + mob.looting, rng);
            give("leather", 0, 1 + mob.looting, rng);
        }
        // Вики, Strider: 2–5 нитей.
        Kind::Strider => give("string", 2, 5 + mob.looting, rng),
        // Кристалл и дракон предметов не роняют (вики: у дракона только опыт,
        // яйцо — блок на портале).
        // Житель ничего не роняет (вики, Villager: «Drops: none»).
        // Волк ничего не роняет, кошка — 0–2 нити (вики, Wolf и Cat, «Drops»).
        Kind::Cat => give("string", 0, 2, rng),
        // Вики, Iron Golem: 3–5 железных слитков и 0–2 мака; Snow Golem: 0–15 снежков
        // («Looting» не влияет).
        Kind::IronGolem => {
            give("poppy", 0, 2, rng);
            give("iron_ingot", 3, 5, rng);
        }
        Kind::SnowGolem => give("snowball", 0, 15, rng),
        Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner | Kind::Vex | Kind::Ravager | Kind::EvokerFangs => illager::loot(mob, &mut give, rng),
        // Вики, Phantom: мембрана — 0–1 (+ «Looting»), только если убил игрок или прирученный волк.
        Kind::Phantom => {
            if mob.by_player {
                give("phantom_membrane", 0, 1 + mob.looting, rng);
            }
        }
        Kind::Guardian | Kind::ElderGuardian => guardian::loot(mob, rng, &mut give),
        // Вики, Wither: звезда Незера всегда (и убитому командой).
        Kind::Wither => give("nether_star", 1, 1, rng),
        Kind::EndCrystal | Kind::EnderDragon | Kind::ShulkerBullet | Kind::Villager | Kind::Wolf | Kind::LeashKnot => {}
        // Вики, Enderman: «0–1 ender pearl» и всегда блок из рук.
        Kind::Enderman => {
            give("ender_pearl", 0, 1, rng);

            if let Some(name) = mob.h.carried.and_then(blocks::block_at_state) {
                give(name, 1, 1, rng);
            }
        }
        // Вики, Shulker: панцирь с шансом 50 %.
        Kind::Shulker => {
            if rng.below(2) == 0 {
                give("shulker_shell", 1, 1, rng);
            }
        }
        // Вики, Witch: 1–3 броска, в каждом — одно из семи (палка вдвое
        // чаще) по 0–2 штуки [проверить: веса].
        Kind::Witch => {
            const WITCH: [&str; 8] = ["glass_bottle", "glowstone_dust", "gunpowder", "redstone", "spider_eye", "sugar", "stick", "stick"];

            for _ in 0..1 + rng.below(3) {
                let name = WITCH[rng.below(WITCH.len() as u32) as usize];

                give(name, 0, 2, rng);
            }
        }
    }

    drops
}

/// Шерсть по цветам, по порядку цветов красителей (вики, Wool).
pub const WOOL: [&str; 16] = [
    "white_wool",
    "orange_wool",
    "magenta_wool",
    "light_blue_wool",
    "yellow_wool",
    "lime_wool",
    "pink_wool",
    "gray_wool",
    "light_gray_wool",
    "cyan_wool",
    "purple_wool",
    "blue_wool",
    "brown_wool",
    "green_wool",
    "red_wool",
    "black_wool",
];

/// Урон оружия в руке и перезарядка (вики, Damage, «Weapon damage»): кулак 1,
/// мечи 4–8. Возвращает (урон, тактов перезарядки).
#[cfg(test)]
pub fn weapon(item: Option<&str>) -> (f32, f32) {
    match item {
        Some("wooden_sword" | "golden_sword") => (4.0, 12.5),
        Some("stone_sword") => (5.0, 12.5),
        Some("iron_sword") => (6.0, 12.5),
        Some("diamond_sword") => (7.0, 12.5),
        Some("netherite_sword") => (8.0, 12.5),
        _ => (1.0, 5.0),
    }
}

/// Доля урона при неполном заряде руки (вики, Damage: `0.2 + charge²·0.8`).
#[cfg(test)]
pub fn charged(damage: f32, since_attack: u32, cooldown: f32) -> f32 {
    let charge = ((since_attack as f32 + 0.5) / cooldown).clamp(0.0, 1.0);

    damage * (0.2 + charge * charge * 0.8)
}

/// Критический удар (вики, Critical hit): рука заряжена больше чем на 0,9,
/// игрок падает (высота падения накоплена), не стоит на земле, не бежит и не
/// в воде; урон тогда ×1,5 (чёрный ящик, cow-crit, cow-crit-fist).
#[cfg(test)]
pub fn is_critical(charge: f32, fall: f32, on_ground: bool, sprinting: bool, in_water: bool) -> bool {
    charge > 0.9 && fall > 0.0 && !on_ground && !sprinting && !in_water
}

/// Кто рядом мог бы толкнуть: единичное направление взгляда игрока по
/// горизонтали (вперёд по yaw) — им и толкает удар (вики, Knockback).
pub fn look_direction(yaw_degrees: f32) -> (f64, f64) {
    let rad = f64::from(yaw_degrees).to_radians();

    (-rad.sin(), rad.cos())
}

/// Сдвиг по горизонтали с упором: по большей оси первой. Возвращает, на
/// сколько сдвинулись по x и z; `at` — угол ящика, он обновляется.
fn slide(view: &dyn View, at: &mut (f64, f64, f64), half: f64, height: f64, dx: f64, dz: f64) -> (f64, f64) {
    if dx.abs() >= dz.abs() {
        let mx = clip_x(view, *at, half, height, dx);

        at.0 += mx;

        let mz = clip_z(view, *at, half, height, dz);

        at.2 += mz;
        (mx, mz)
    } else {
        let mz = clip_z(view, *at, half, height, dz);

        at.2 += mz;

        let mx = clip_x(view, *at, half, height, dx);

        at.0 += mx;
        (mx, mz)
    }
}

/// Свободный ход по оси вверх/вниз: насколько можно сдвинуться, не входя в блоки.
pub(crate) fn clip_y(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dy: f64) -> f64 {
    if dy == 0.0 {
        return 0.0;
    }

    let x0 = (min.0 + EPS).floor() as i32;
    let x1 = (min.0 + 2.0 * half - EPS).floor() as i32;
    let z0 = (min.2 + EPS).floor() as i32;
    let z1 = (min.2 + 2.0 * half - EPS).floor() as i32;

    if dy > 0.0 {
        let top = min.1 + height;
        let from = (top - EPS).floor() as i32 + 1;
        let to = (top + dy).floor() as i32;
        let mut allowed = dy;

        for y in from..=to.max(from - 1) {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.min((f64::from(y) - top).max(0.0));
                    }
                }
            }
        }

        allowed
    } else {
        let bottom = min.1;
        let from = (bottom + EPS).floor() as i32 - 1;
        let to = (bottom + dy).floor() as i32;
        let mut allowed = dy;
        let mut y = from;

        while y >= to {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.max(f64::from(y + 1) - bottom);
                    }
                }
            }

            y -= 1;
        }

        allowed
    }
}

pub(crate) fn clip_x(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dx: f64) -> f64 {
    if dx == 0.0 {
        return 0.0;
    }

    let y0 = (min.1 + EPS).floor() as i32;
    let y1 = (min.1 + height - EPS).floor() as i32;
    let z0 = (min.2 + EPS).floor() as i32;
    let z1 = (min.2 + 2.0 * half - EPS).floor() as i32;
    let mut allowed = dx;

    if dx > 0.0 {
        let edge = min.0 + 2.0 * half;
        let from = (edge - EPS).floor() as i32 + 1;
        let to = (edge + dx).floor() as i32;

        for x in from..=to.max(from - 1) {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.min((f64::from(x) - edge).max(0.0));
                    }
                }
            }
        }
    } else {
        let edge = min.0;
        let from = (edge + EPS).floor() as i32 - 1;
        let to = (edge + dx).floor() as i32;
        let mut x = from;

        while x >= to {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.max(f64::from(x + 1) - edge);
                    }
                }
            }

            x -= 1;
        }
    }

    allowed
}

pub(crate) fn clip_z(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dz: f64) -> f64 {
    if dz == 0.0 {
        return 0.0;
    }

    let y0 = (min.1 + EPS).floor() as i32;
    let y1 = (min.1 + height - EPS).floor() as i32;
    let x0 = (min.0 + EPS).floor() as i32;
    let x1 = (min.0 + 2.0 * half - EPS).floor() as i32;
    let mut allowed = dz;

    if dz > 0.0 {
        let edge = min.2 + 2.0 * half;
        let from = (edge - EPS).floor() as i32 + 1;
        let to = (edge + dz).floor() as i32;

        for z in from..=to.max(from - 1) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.min((f64::from(z) - edge).max(0.0));
                    }
                }
            }
        }
    } else {
        let edge = min.2;
        let from = (edge + EPS).floor() as i32 - 1;
        let to = (edge + dz).floor() as i32;
        let mut z = from;

        while z >= to {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.max(f64::from(z + 1) - edge);
                    }
                }
            }

            z -= 1;
        }
    }

    allowed
}

/// Действует ли броня на урон: не действует на падение, огонь, лаву,
/// удушье и пустоту (вики, Armor: «bypasses armor»).
fn armored(source: Source) -> bool {
    matches!(source, Source::PlayerAttack | Source::MaceSmash | Source::MobAttack | Source::Arrow | Source::Explosion | Source::PlayerExplosion)
}

/// Урон после брони (вики, Armor, «Damage protection»).
pub fn after_armor(amount: f32, armor: f32) -> f32 {
    if armor <= 0.0 {
        return amount;
    }

    let reduction = (armor / 5.0).max(armor - amount / 2.0).min(20.0);

    amount * (1.0 - reduction / 25.0)
}

/// Опознаватель сущности.
pub fn uuid_for(entity_id: i32) -> [u8; 16] {
    let mut uuid = crate::items::uuid_for(entity_id);

    uuid[12..].copy_from_slice(b"mob!");
    uuid
}

/// Небольшой счётчик случайных чисел (xorshift): мобам нужна случайность
/// сама по себе, но не криптографическая; своё зерно позволяет проверкам
/// быть повторяемыми.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;

        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Число от 0 (включительно) до `n` (нет).
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { ((self.next_u64() >> 32) % u64::from(n)) as u32 }
    }

    /// Дробь от 0 до 1.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Что мобы просят у мира за такт: съеденная овцами трава и снесённые яйца.
#[derive(Default, Debug)]
pub struct Outcome {
    pub eaten: Vec<ai::Eat>,
    pub eggs: Vec<(f64, f64, f64)>,

    /// Удары враждебных по игрокам, выстрелы скелетов, взрывы крипера.
    pub strikes: Vec<Strike>,
    pub shots: Vec<Fired>,
    pub blasts: Vec<Blast>,

    /// Попадания снарядов шалкера: кто выпустил, в кого, где.
    pub bullet_hits: Vec<(i32, i32, (f64, f64, f64))>,

    /// Блоки, взятые эндерменами, и поставленные ими (клетка, состояние).
    pub grabs: Vec<(i32, i32, i32)>,
    pub puts: Vec<((i32, i32, i32), i32)>,

    /// Огненные шары ифритов и гастов: кто пустил и какой.
    pub launches: Vec<(i32, nether::Launch)>,

    /// События мира: клетка и номер (1040 — зомби стал утопленником, 1041 —
    /// кадавр стал зомби).
    pub level_events: Vec<((i32, i32, i32), i32)>,

    /// Снежки снежных големов: кто бросил, откуда, скорость (mob/golem.rs).
    pub snowballs: Vec<(i32, golem::Throw)>,

    /// Клетки, куда снежные големы кладут снег (если разрешено mobGriefing).
    pub snow: Vec<(i32, i32, i32)>,

    /// Проклятия древних стражей: игрок и тихий ли страж (mob/guardian.rs).
    pub curses: Vec<(i32, bool)>,

    /// Заказы иссушителей: кто и что (mob/wither.rs; tick.rs, wither_deeds).
    pub withers: Vec<(i32, wither::Deed)>,
}

/// Удар моба в ближнем бою.
#[derive(Debug)]
pub struct Strike {
    pub attacker: i32,
    pub at: (f64, f64, f64),
    pub kind: Kind,
    pub melee: hostile::Melee,
}

/// Выстрел скелета.
#[derive(Debug)]
pub struct Fired {
    pub kind: Kind,
    pub shooter: i32,
    pub shot: hostile::Shot,
}

/// Взрыв крипера: центр, сила, кто.
#[derive(Debug)]
pub struct Blast {
    pub shooter: i32,
    pub at: (f64, f64, f64),
    pub power: f32,
}

/// Среда, что мобы чувствуют за такт: сложность, свет неба, солнце.
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub difficulty: u8,
    pub sky: i32,
    pub sun_up: bool,

    /// Часы мира от начала: день × 24000 + час (пополнения жителей).
    pub clock: i64,
}

/// Такт всех мобов склада: у каждого мысли и движение, потом разведение,
/// добыча погибших и уборка тел. События уходят в журнал в порядке, в каком
/// произошли. `next_id` выдаёт номера новым сущностям.
pub fn step_all(
    items: &mut crate::items::Items,
    view: &dyn View,
    players: &[ai::Watcher],
    env: Env,
    rng: &mut Rng,
    next_id: &mut dyn FnMut() -> i32,
) -> Outcome {
    let mut outcome = Outcome::default();

    if items.mobs().is_empty() {
        return outcome;
    }

    let mates: Vec<ai::Mate> = items
        .mobs()
        .iter()
        .filter(|mob| !mob.is_dead())
        .map(|mob| ai::Mate {
            id: mob.entity_id,
            kind: mob.kind,
            x: mob.x,
            y: mob.y,
            z: mob.z,
            love: mob.love,
            baby: mob.is_baby(),
            ready: mob.age == 0,
        })
        .collect();

    let ctx = ai::Ctx { players, mates: &mates, difficulty: env.difficulty, sky: env.sky, sun_up: env.sun_up };
    let mut mobs = std::mem::take(items.mobs_vec());
    let mut breeds: Vec<(i32, i32)> = Vec::new();
    let mut born = Vec::new();

    // Занятые жителями рабочие блоки: второй их не займёт.
    let mut claimed: Vec<(i32, i32, i32)> = mobs.iter().filter_map(|mob| mob.villager.job_site).collect();

    let mut cured: Vec<usize> = Vec::new();
    let mut converted: Vec<(usize, Kind)> = Vec::new();
    // Укус: кто, кого, урон, толчок и место железного голема (его удар подбрасывает).
    type Bite = (i32, i32, f32, (f64, f64), Option<(f64, f64, f64)>);
    let mut bites: Vec<Bite> = Vec::new();

    // Синие овцы — для «вололо» заклинателя (mob/illager.rs).
    let blue_sheep: Vec<(i32, (f64, f64, f64))> = mobs.iter().filter(|mob| mob.kind == Kind::Sheep && mob.color == illager::WOLOLO_FROM && !mob.is_dead()).map(|mob| (mob.entity_id, (mob.x, mob.y, mob.z))).collect();
    let mut recolor: Vec<i32> = Vec::new();

    for (index, mob) in mobs.iter_mut().enumerate() {
        mob.tame.clock = env.clock;

        if mob.kind == Kind::Evoker {
            mob.illager.sheep = blue_sheep.iter().filter(|(_, at)| hostile::dist2(mob, at.0, at.1, at.2) <= illager::WOLOLO_RANGE * illager::WOLOLO_RANGE).map(|(id, _)| *id).next();
        }

        let plan = mob.tick(view, &ctx, rng);

        // Снежный голем: след и таяние — и без ИИ (snow-golem-shear).
        outcome.snow.extend(golem::snow_tick(mob, view, true));

        if let Some((target, damage, push)) = plan.bite {
            bites.push((mob.entity_id, target, damage, push, plan.fling.then_some((mob.x, mob.y, mob.z))));
        }

        if let Some((from, velocity)) = plan.snowball {
            outcome.snowballs.push((mob.entity_id, (from, velocity)));
        }

        if let Some(kind) = mob.h.converted.take() {
            converted.push((index, kind));
        }

        // Призванный уже лечащимся (`ConversionTime`): сила от лечения ложится на
        // следующем такте (чёрный ящик, zombie-villager-cured) [проверить: уровень].
        if mob.villager.fresh_cure && mob.kind == Kind::ZombieVillager {
            mob.villager.fresh_cure = false;
            mob.vitals.give_effect_shown(crate::survival::Effect {
                kind: crate::survival::EffectKind::Strength,
                amplifier: i32::from(env.difficulty.saturating_sub(1)).min(2),
                ticks_left: mob.villager.conversion,
                ambient: false,
                visible: true,
            });
        }

        // Лечение зомби-жителя: счёт вниз, на нуле он снова житель (вики,
        // Zombie Villager, «Curing»; чёрный ящик, zombie-villager-cured).
        if mob.kind == Kind::ZombieVillager && mob.villager.conversion > 0 && !mob.is_dead() {
            mob.villager.conversion -= 1;

            if mob.villager.conversion == 0 {
                cured.push(index);
            }
        }

        if mob.kind == Kind::Villager && !mob.no_ai && !mob.is_dead() {
            crate::villager::step(mob, view, env.clock, &claimed, rng);
            claimed.extend(mob.villager.job_site);
        }

        if slime::cube(mob.kind) && env.difficulty != crate::survival::PEACEFUL {
            for player in players {
                if let Some(melee) = slime::contact(mob, view, player) {
                    outcome.strikes.push(Strike { attacker: mob.entity_id, at: (mob.x, mob.y, mob.z), kind: mob.kind, melee });
                }
            }
        }

        for player in std::mem::take(&mut mob.guardian.cursed) {
            outcome.curses.push((player, mob.silent));
        }

        for deed in std::mem::take(&mut mob.wither.deeds) {
            outcome.withers.push((mob.entity_id, deed));
        }

        if let Some(melee) = plan.melee {
            // Укус фантома: событие мира 1039 в его клетке (чёрный ящик, phantom-swoop).
            if mob.kind == Kind::Phantom {
                outcome.level_events.push((mob.cell(), 1039));
            }

            outcome.strikes.push(Strike { attacker: mob.entity_id, at: (mob.x, mob.y, mob.z), kind: mob.kind, melee });
        }

        if let Some(shot) = plan.shot {
            outcome.shots.push(Fired { kind: mob.kind, shooter: mob.entity_id, shot });
        }

        // Вредины и челюсти заклинателя (mob/illager.rs): вредина со сроком
        // 30–119 секунд (вики, Vex: «start taking damage after 30 to 119
        // seconds») [проверить].
        recolor.extend(mob.illager.recolor.take());

        for (kind, at, warmup, yaw) in std::mem::take(&mut mob.illager.summon) {
            let mut summoned = Mob::new(next_id(), kind, at);

            summoned.yaw = yaw;
            summoned.head_yaw = yaw;
            summoned.h.owner = mob.entity_id;
            summoned.h.target = mob.h.target;
            summoned.illager.warmup = warmup;
            summoned.silent = mob.silent && kind == Kind::EvokerFangs;

            if kind == Kind::Vex {
                summoned.illager.life = 20 * (30 + rng.below(90) as i32);
                // Не все сразу в рывок: у оригинала из трёх рванулась одна (evoker-fangs) [проверить].
                summoned.illager.wander = rng.below(80) as i32;
            }

            born.push(summoned);
        }

        if let Some(launch) = plan.launch {
            outcome.launches.push((mob.entity_id, launch));
        }

        if let Some(power) = plan.blast {
            outcome.blasts.push(Blast { shooter: mob.entity_id, at: (mob.x, mob.y, mob.z), power });
            mob.h.despawn = true;
        }

        // Шалкер не исчезает вдали от игроков (вики, Shulker: «does not despawn»)
        // [проверить: на мирной].
        // Труп враждебного исчезает на мирной тоже, без облачка: у оригинала
        // убитая чешуйница пропала в такт команды `difficulty peaceful`
        // (чёрный ящик, silverfish-infested) [проверить: остальные мобы].
        // Живой с именной биркой не исчезает (persistent_tag).
        let peaceful = env.difficulty == crate::survival::PEACEFUL;

        if mob.kind.hostile() && mob.kind != Kind::Shulker && ((!mob.is_dead() && !mob.persistent_tag) || (peaceful && mob.is_dead())) {
            hostile::despawn_check(mob, players, rng, peaceful);
        }

        if let Some(target) = plan.bullet {
            // Снаряд появляется над шалкером: y + 0,99692 (чёрный ящик, shulker-ai).
            let mut shot = Mob::new(next_id(), Kind::ShulkerBullet, (mob.x, mob.y + 0.99692, mob.z));

            shot.h.target = Some(target);
            shot.h.owner = mob.entity_id;
            shot.persistent = false;
            shot.h.quiet = mob.silent;
            born.push(shot);
        }

        if let Some(target) = plan.bullet_hit {
            outcome.bullet_hits.push((mob.h.owner, target, (mob.x, mob.y, mob.z)));
        }

        outcome.grabs.extend(plan.take);
        outcome.puts.extend(plan.put);

        if let Some(partner) = plan.breed {
            breeds.push((mob.entity_id, partner));
        }

        if let Some(eat) = plan.eat {
            outcome.eaten.push(eat);

            if mob.kind == Kind::Sheep {
                mob.sheared = false;

                if mob.is_baby() {
                    mob.age = (mob.age + 60).min(0);
                }
            }
        }

        if plan.lay_egg {
            outcome.eggs.push((mob.x, mob.y, mob.z));
            mob.events.push(Event::Sound { name: "minecraft:entity.chicken.egg", volume: 1.0, pitch: (rng.unit() - rng.unit()) as f32 * 0.2 + 1.0 });
        }
    }

    // Укусы волков по мобам (mob/tame.rs): урон от моба с толчком.
    // Удар железного голема подбрасывает цель на 0,4, звук удара — после
    // урона и смерти цели (чёрный ящик, iron-golem-attack).
    for (attacker, target, damage, dir, golem_at) in bites {
        if let Some(victim) = mobs.iter_mut().find(|mob| mob.entity_id == target) {
            let hit = victim.hurt(damage, Source::MobAttack, Some(Push { dir, strength: BASE_KNOCKBACK, cause: attacker }));

            if let Some(at) = golem_at {
                if hit.is_some() {
                    victim.vy += golem::FLING;
                }

                victim.events.push(Event::SoundAt { name: "minecraft:entity.iron_golem.attack", volume: 1.0, pitch: 1.0, at });
            }
        }
    }

    // Волки хозяев: кого хозяин ударил и кто ударил хозяина; дикая стая;
    // поводки (mob/leash.rs).
    let owner_hits: Vec<(i32, i32)> = outcome.strikes.iter().map(|strike| (strike.attacker, strike.melee.target)).collect();

    tame::orders(&mut mobs, &owner_hits);
    tame::pack_anger(&mut mobs);
    leash::bind_knots(&mut mobs);
    leash::step_all(&mut mobs, &players.iter().map(|player| (player.id, (player.x, player.y, player.z))).collect::<Vec<_>>());

    // Зомби-пиглины зовут стаю: все в 32 блоках злятся на обидчика (вики,
    // Zombified Piglin: «all zombified piglins within … become angry»).
    let alerts: Vec<(usize, Option<i32>)> = mobs.iter().enumerate().filter(|(_, mob)| mob.nether.alert).map(|(index, mob)| (index, mob.h.target)).collect();

    for (index, target) in alerts {
        let (x, y, z) = (mobs[index].x, mobs[index].y, mobs[index].z);

        mobs[index].nether.alert = false;

        for mate in mobs.iter_mut().filter(|mate| mate.kind == Kind::ZombifiedPiglin && !mate.is_dead() && !mate.no_ai) {
            if (mate.x - x).powi(2) + (mate.y - y).powi(2) + (mate.z - z).powi(2) <= nether::CALL_RANGE * nether::CALL_RANGE {
                if mate.nether.anger == 0 {
                    mate.nether.anger = nether::anger_ticks(rng);
                }

                mate.h.target = target;
            }
        }
    }

    // Оба куба делятся при удалении трупа; исчезновение на мирной не делит.
    for parent in mobs.iter().filter(|mob| slime::cube(mob.kind) && mob.gone()) {
        born.extend(slime::split(parent, rng, next_id));
    }

    // Зомби стал утопленником, кадавр — зомби: тот же моб, новый номер (вики,
    // Drowned; чёрный ящик, zombie-to-drowned).
    // «Вололо»: синяя овца становится красной (вики, Evoker).
    for id in recolor {
        if let Some(sheep) = mobs.iter_mut().find(|mob| mob.entity_id == id && mob.kind == Kind::Sheep && mob.color == illager::WOLOLO_FROM) {
            sheep.color = illager::WOLOLO_TO;
        }
    }

    for (index, kind) in converted {
        let mob = convert(&mobs[index], kind, next_id());

        if !mobs[index].silent {
            outcome.level_events.push((mobs[index].cell(), if kind == Kind::Stray { 1048 } else if kind == Kind::Drowned { 1040 } else { 1041 }));
        }

        mobs[index].h.despawn = true;
        born.push(mob);
    }

    for index in cured {
        let villager = cure(&mut mobs[index], next_id());

        mobs[index].h.despawn = true;
        born.push(villager);
    }

    // Разведение: оба родителя в паузу, детёныш рождается у первого. Пара,
    // из которой один уже занят, пропускается [проверить].
    let mut used: Vec<i32> = Vec::new();

    for (a, b) in breeds {
        if used.contains(&a) || used.contains(&b) {
            continue;
        }

        let (Some(ia), Some(ib)) = (mobs.iter().position(|m| m.entity_id == a), mobs.iter().position(|m| m.entity_id == b)) else {
            continue;
        };

        used.extend([a, b]);

        let mut baby = Mob::new(next_id(), mobs[ia].kind, (mobs[ia].x, mobs[ia].y, mobs[ia].z));

        baby.age = BABY_AGE;
        baby.yaw = mobs[ia].yaw;
        baby.head_yaw = mobs[ia].yaw;
        baby.color = if mobs[ia].kind == Kind::Sheep { if rng.below(2) == 0 { mobs[ia].color } else { mobs[ib].color } } else { WHITE };
        baby.egg_timer = 6000 + rng.below(6001) as i32;

        for index in [ia, ib] {
            mobs[index].age = BREED_COOLDOWN;
            mobs[index].love = 0;
            mobs[index].brain.stop();
        }

        // Сердечки — событием сущности 18 у первого родителя (чёрный ящик, cow-breed).
        mobs[ia].events.push(Event::Hearts);
        born.push(baby);
    }

    *items.mobs_vec() = mobs;

    // Тела, пролежавшие своё, уходят облачком.
    for mob in items.mobs_mut() {
        if mob.gone() {
            mob.events.push(Event::Poof);
        }
    }

    items.flush_mob_events(next_id, rng);
    items.remove_mobs(|mob| mob.gone() || mob.h.despawn);

    for mut baby in born {
        baby.late = true;

        // Вылеченный житель: появление — без эффектов, на следующем такте в
        // слежении тошнота и сила, ещё через такт — одна тошнота (чёрный ящик,
        // zombie-villager-cured): эффекты ложатся уже после снимка.
        let cured = baby.kind == Kind::Villager && std::mem::take(&mut baby.villager.fresh_cure);

        items.add_mob_late(baby);

        if let Some(mob) = items.mobs_mut().last_mut().filter(|_| cured) {
            mob.vitals.give_effect_shown(crate::survival::Effect { kind: crate::survival::EffectKind::Nausea, amplifier: 0, ticks_left: CURED_NAUSEA, ambient: false, visible: true });
            mob.vitals.give_effect_shown(crate::survival::Effect { kind: crate::survival::EffectKind::Strength, amplifier: 0, ticks_left: 1, ambient: false, visible: true });
        }
    }

    outcome
}

/// Сколько тактов тошнит вылеченного жителя (чёрный ящик, zombie-villager-cured:
/// тошнота в частицах сразу после превращения) [проверить: срок].
pub const CURED_NAUSEA: i32 = 200;

/// Репутация вылечившего: чёрный ящик, zombie-villager-cured-discount: цена
/// ниже на 6 при множителе 0,05 — `⌊репутация · 0,05⌋ = 6`, то есть от 120 до
/// 139; вики, Trading, «Gossip»: major_positive 20 · вес 5 и minor_positive
/// 25 · вес 1 дают 125 [проверить: части].
pub const CURE_REPUTATION: i32 = 125;

/// Из вылеченного зомби-жителя — житель с теми же профессией, опытом и
/// сделками. Вид — по биому (у нас плоский мир — равнинный: у оригинала при
/// таёжном зомби-жителе вышел равнинный) [проверить]. Вылечивший получает
/// молву «major_positive» 20 (вики, Trading, «Gossip»).
pub fn cure(zombie: &mut Mob, entity_id: i32) -> Mob {
    let mut villager = Mob::new(entity_id, Kind::Villager, (zombie.x, zombie.y, zombie.z));

    villager.yaw = zombie.yaw;
    villager.head_yaw = zombie.head_yaw;
    villager.pitch = zombie.pitch;
    villager.age = zombie.age;
    villager.no_ai = zombie.no_ai;
    villager.silent = zombie.silent;
    villager.on_ground = zombie.on_ground;
    villager.villager = std::mem::take(&mut zombie.villager);
    villager.villager.kind = crate::villager::PLAINS;
    villager.villager.finalized = true;
    villager.villager.conversion = 0;

    if let Some(uuid) = villager.villager.converter.take() {
        villager.villager.gossip.push((uuid, CURE_REPUTATION));
    }

    // Эффекты — на следующем такте (чёрный ящик, zombie-villager-cured: при
    // появлении частиц нет, через такт — тошнота и сила, ещё через такт — одна
    // тошнота): `fresh_cure`, см. `step_all`.
    villager.villager.fresh_cure = true;
    villager
}

/// Превращение зомби в утопленника или кадавра в зомби: место, поворот,
/// скорость, возраст, снаряжение и признаки остаются (здоровье — полное).
pub fn convert(old: &Mob, kind: Kind, entity_id: i32) -> Mob {
    let mut new = Mob::new(entity_id, kind, (old.x, old.y, old.z));

    (new.vx, new.vy, new.vz) = (old.vx, old.vy, old.vz);
    new.yaw = old.yaw;
    new.head_yaw = old.head_yaw;
    new.pitch = old.pitch;
    new.age = old.age;
    new.no_ai = old.no_ai;
    new.silent = old.silent;
    new.on_ground = old.on_ground;
    new.persistent = old.persistent;
    new.name = old.name.clone();
    new.persistent_tag = old.persistent_tag;
    new.h.bow = old.h.bow;
    new.h.helmet = old.h.helmet;
    new.h.hand = old.h.hand;
    new.h.offhand_shell = old.h.offhand_shell;
    new.h.hand_drop = old.h.hand_drop;
    new
}

/// Сколько нельзя поднимать выпавшее из моба: полсекунды, как у выпавшего
/// из блока (вики, Item: «pickup delay 10 ticks»).
pub const DROP_DELAY: u64 = 10;

#[cfg(test)]
mod tests;
