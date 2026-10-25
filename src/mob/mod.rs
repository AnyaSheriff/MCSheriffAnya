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
pub mod armor_stand;
pub mod cushion;
pub mod sulfur_cube;
pub mod movement_enchantments;
pub mod trial_gear;
pub mod equipment;
pub mod combat_enchantments;
pub mod small;
pub mod small_world;
pub mod growth;
pub mod arthropod;
pub mod dragon;
pub mod breeze;
pub mod golem;
pub mod village_golems;
pub mod guardian;
pub mod wither;
pub mod water;
pub mod warden;
pub mod hostile;
pub mod illager;
pub mod horse;
pub mod leash;
pub mod land;
pub mod trader;
pub mod nether;
pub mod newmobs;
pub mod newblocks;
pub mod phantom;
pub mod path;
pub mod spawn;
pub mod slime;
pub mod skeleton;
mod save;
mod item_storage;
pub mod tame;
pub mod wildlife;
pub mod wolf_armor;

use crate::blocks;
use crate::inventory::Stack;
use crate::survival::{Hit, Source, Vitals};

/// Вид моба.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Rabbit,
    Fox,
    Bee,
    Endermite,
    Zoglin,
    Creaking,
    HappyGhast,
    CopperGolem,
    ArmorStand,
    /// Подушка 26.3: неподвижная сущность без ИИ, цвет — в `color`, сидеть на ней можно (mob/cushion.rs).
    Cushion,
    /// Куб серы 26.3: пассивный прыгающий моб, поглощает блоки (mob/sulfur_cube.rs).
    SulfurCube,
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
    Llama,
    TraderLlama,
    WanderingTrader,
    Camel,
    Goat,
    PolarBear,
    Panda,
    Ocelot,
    Mooshroom,
    /// Бриз (вики, Breeze): скользит, стреляет зарядами ветра, прыгает (mob/breeze.rs).
    Breeze,
    // Водные мобы (вики; minecraft-data 26.1).
    Cod,
    Salmon,
    TropicalFish,
    Pufferfish,
    Squid,
    GlowSquid,
    Dolphin,
    Turtle,
    Axolotl,
    Frog,
    Tadpole,
    /// Хранитель: появляется только от крикуна (mob/warden.rs).
    Warden,
    Horse,
    Donkey,
    Mule,
    SkeletonHorse,
    ZombieHorse,
    Armadillo,
    Sniffer,
    Parrot,
    Bat,
    Allay,
}

impl Kind {
    /// Все виды: для проверок.
    #[cfg(test)]
    pub const ALL: [Kind; 91] = [
        Kind::Rabbit, Kind::Fox, Kind::Bee,
        Kind::Endermite,
        Kind::Zoglin,
        Kind::Creaking,
        Kind::HappyGhast,
        Kind::CopperGolem,
        Kind::ArmorStand,
        Kind::Cushion,
        Kind::SulfurCube,
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
        Kind::Llama,
        Kind::TraderLlama,
        Kind::WanderingTrader,
        Kind::Camel,
        Kind::Goat,
        Kind::PolarBear,
        Kind::Panda,
        Kind::Ocelot,
        Kind::Mooshroom,
        Kind::Breeze,
        Kind::Cod,
        Kind::Salmon,
        Kind::TropicalFish,
        Kind::Pufferfish,
        Kind::Squid,
        Kind::GlowSquid,
        Kind::Dolphin,
        Kind::Turtle,
        Kind::Axolotl,
        Kind::Frog,
        Kind::Tadpole,
        Kind::Warden,
        Kind::Horse,
        Kind::Donkey,
        Kind::Mule,
        Kind::SkeletonHorse,
        Kind::ZombieHorse,
        Kind::Armadillo, Kind::Sniffer, Kind::Parrot, Kind::Bat, Kind::Allay,
    ];

    /// Враждебный ли вид (вики, Monster): у него свои цели (mob/hostile.rs).
    pub fn hostile(self) -> bool {
        matches!(
            self,
            Kind::Endermite | Kind::Zoglin | Kind::Creaking | Kind::Zombie
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
                | Kind::Breeze
                | Kind::Warden
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
            "llama" => Kind::Llama,
            "trader_llama" => Kind::TraderLlama,
            "wandering_trader" => Kind::WanderingTrader,
            "camel" => Kind::Camel,
            "goat" => Kind::Goat,
            "polar_bear" => Kind::PolarBear,
            "panda" => Kind::Panda,
            "ocelot" => Kind::Ocelot,
            "mooshroom" => Kind::Mooshroom,
            "rabbit" => Kind::Rabbit,
            "fox" => Kind::Fox,
            "bee" => Kind::Bee,
            "armadillo" => Kind::Armadillo,
            "sniffer" => Kind::Sniffer,
            "parrot" => Kind::Parrot,
            "bat" => Kind::Bat,
            "allay" => Kind::Allay,
            "endermite" => Kind::Endermite,
            "zoglin" => Kind::Zoglin,
            "creaking" => Kind::Creaking,
            "happy_ghast" => Kind::HappyGhast,
            "copper_golem" => Kind::CopperGolem,
            "armor_stand" => Kind::ArmorStand,
            "cushion" => Kind::Cushion,
            "sulfur_cube" => Kind::SulfurCube,
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
            "breeze" => Kind::Breeze,
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
            "cod" => Kind::Cod,
            "salmon" => Kind::Salmon,
            "tropical_fish" => Kind::TropicalFish,
            "pufferfish" => Kind::Pufferfish,
            "squid" => Kind::Squid,
            "glow_squid" => Kind::GlowSquid,
            "dolphin" => Kind::Dolphin,
            "turtle" => Kind::Turtle,
            "axolotl" => Kind::Axolotl,
            "frog" => Kind::Frog,
            "tadpole" => Kind::Tadpole,
            "warden" => Kind::Warden,
            "horse" => Kind::Horse,
            "donkey" => Kind::Donkey,
            "mule" => Kind::Mule,
            "skeleton_horse" => Kind::SkeletonHorse,
            "zombie_horse" => Kind::ZombieHorse,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Llama => "llama",
            Kind::TraderLlama => "trader_llama",
            Kind::WanderingTrader => "wandering_trader",
            Kind::Camel => "camel",
            Kind::Goat => "goat",
            Kind::PolarBear => "polar_bear",
            Kind::Panda => "panda",
            Kind::Ocelot => "ocelot",
            Kind::Mooshroom => "mooshroom",
            Kind::Rabbit => "rabbit",
            Kind::Fox => "fox",
            Kind::Bee => "bee",
            Kind::Armadillo => "armadillo",
            Kind::Sniffer => "sniffer",
            Kind::Parrot => "parrot",
            Kind::Bat => "bat",
            Kind::Allay => "allay",
            Kind::Endermite => "endermite",
            Kind::Zoglin => "zoglin",
            Kind::Creaking => "creaking",
            Kind::HappyGhast => "happy_ghast",
            Kind::CopperGolem => "copper_golem",
            Kind::ArmorStand => "armor_stand",
            Kind::Cushion => "cushion",
            Kind::SulfurCube => "sulfur_cube",
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
            Kind::Breeze => "breeze",
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
            Kind::Cod => "cod",
            Kind::Salmon => "salmon",
            Kind::TropicalFish => "tropical_fish",
            Kind::Pufferfish => "pufferfish",
            Kind::Squid => "squid",
            Kind::GlowSquid => "glow_squid",
            Kind::Dolphin => "dolphin",
            Kind::Turtle => "turtle",
            Kind::Axolotl => "axolotl",
            Kind::Frog => "frog",
            Kind::Tadpole => "tadpole",
            Kind::Warden => "warden",
            Kind::Horse => "horse",
            Kind::Donkey => "donkey",
            Kind::Mule => "mule",
            Kind::SkeletonHorse => "skeleton_horse",
            Kind::ZombieHorse => "zombie_horse",
        }
    }

    /// Номер вида сущности для клиента Java (minecraft-data 26.1, entities.json).
    pub fn java_type(self) -> i32 {
        match self {
            Kind::Llama => 78,
            Kind::TraderLlama => 134,
            Kind::WanderingTrader => 141,
            Kind::Camel => 19,
            Kind::Goat => 62,
            Kind::PolarBear => 104,
            Kind::Panda => 96,
            Kind::Ocelot => 91,
            Kind::Mooshroom => 86,
            Kind::Rabbit => 108,
            Kind::Fox => 54,
            Kind::Bee => 11,
            Kind::Armadillo => 4,
            Kind::Sniffer => 119,
            Kind::Parrot => 98,
            Kind::Bat => 10,
            Kind::Allay => 2,
            Kind::Endermite => 42,
            Kind::Zoglin => 149,
            Kind::Creaking => 31,
            Kind::HappyGhast => 58,
            Kind::CopperGolem => 28,
            Kind::ArmorStand => 5,
            // Внутренний номер расширения 26.3 (после 775: 157/158 — лодки тополя); в 777 это 33.
            Kind::Cushion => 159,
            // Следующий внутренний номер расширения 26.3; в 777 это 133.
            Kind::SulfurCube => 160,
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
            // minecraft-data 26.1: breeze 17.
            Kind::Breeze => 17,
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
            Kind::Cod => 27,
            Kind::Salmon => 110,
            Kind::TropicalFish => 136,
            Kind::Pufferfish => 107,
            Kind::Squid => 127,
            Kind::GlowSquid => 61,
            Kind::Dolphin => 35,
            Kind::Turtle => 137,
            Kind::Axolotl => 7,
            Kind::Frog => 55,
            Kind::Tadpole => 130,
            Kind::Warden => 142,
            Kind::Horse => 66,
            Kind::Donkey => 36,
            Kind::Mule => 87,
            Kind::SkeletonHorse => 116,
            Kind::ZombieHorse => 151,
        }
    }

    /// Имя вида для клиента Bedrock (AddActor).
    pub fn bedrock_name(self) -> &'static str {
        match self {
            // [Bedrock: потом] поведение и свойства новых наземных мобов.
            Kind::Llama => "minecraft:llama",
            Kind::TraderLlama => "minecraft:trader_llama",
            Kind::WanderingTrader => "minecraft:wandering_trader",
            Kind::Camel => "minecraft:camel",
            Kind::Goat => "minecraft:goat",
            Kind::PolarBear => "minecraft:polar_bear",
            Kind::Panda => "minecraft:panda",
            Kind::Ocelot => "minecraft:ocelot",
            Kind::Mooshroom => "minecraft:mooshroom",
            // [Bedrock: потом] особые свойства и анимации этих животных.
            Kind::Rabbit => "minecraft:rabbit",
            Kind::Fox => "minecraft:fox",
            Kind::Bee => "minecraft:bee",
            Kind::Armadillo => "minecraft:armadillo",
            Kind::Sniffer => "minecraft:sniffer",
            Kind::Parrot => "minecraft:parrot",
            Kind::Bat => "minecraft:bat",
            Kind::Allay => "minecraft:allay",
            Kind::Endermite => "minecraft:endermite", // [Bedrock: потом] свойства.
            Kind::Zoglin => "minecraft:zoglin", // [Bedrock: потом] свойства.
            Kind::Creaking => "minecraft:creaking", // [Bedrock: потом] свойства.
            Kind::HappyGhast => "minecraft:happy_ghast", // [Bedrock: потом] свойства.
            Kind::CopperGolem => "minecraft:copper_golem",
            Kind::ArmorStand => "minecraft:armor_stand", // [Bedrock: потом] свойства.
            Kind::Cushion => "minecraft:cushion", // [проверить] имя в Bedrock.
            Kind::SulfurCube => "minecraft:sulfur_cube", // [проверить] имя в Bedrock.
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
            // [Bedrock: потом] бриз для клиента Bedrock: имя есть, поз нет.
            Kind::Breeze => "minecraft:breeze",
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
            // [Bedrock: потом] свойства вариантов, ведра и анимации.
            Kind::Cod => "minecraft:cod",
            Kind::Salmon => "minecraft:salmon",
            Kind::TropicalFish => "minecraft:tropical_fish",
            Kind::Pufferfish => "minecraft:pufferfish",
            Kind::Squid => "minecraft:squid",
            Kind::GlowSquid => "minecraft:glow_squid",
            Kind::Dolphin => "minecraft:dolphin",
            Kind::Turtle => "minecraft:turtle",
            Kind::Axolotl => "minecraft:axolotl",
            Kind::Frog => "minecraft:frog",
            Kind::Tadpole => "minecraft:tadpole",
            Kind::Warden => "minecraft:warden", // [Bedrock: потом] свойства и позы.
            // [Bedrock: потом] седло, броня, окно и управление лошадьми.
            Kind::Horse => "minecraft:horse",
            Kind::Donkey => "minecraft:donkey",
            Kind::Mule => "minecraft:mule",
            Kind::SkeletonHorse => "minecraft:skeleton_horse",
            Kind::ZombieHorse => "minecraft:zombie_horse",
        }
    }

    /// Ширина ящика (minecraft-data, entities.json).
    pub fn width(self) -> f64 {
        match self {
            Kind::Llama => 0.9,
            Kind::TraderLlama => 0.9,
            Kind::WanderingTrader => 0.6,
            Kind::Camel => 1.7,
            Kind::Goat => 0.9,
            Kind::PolarBear => 1.4,
            Kind::Panda => 1.3,
            Kind::Ocelot => 0.6,
            Kind::Mooshroom => 0.9,
            Kind::Cod => 0.5,
            Kind::Salmon => 0.7,
            Kind::TropicalFish => 0.5,
            Kind::Pufferfish => 0.7,
            Kind::Squid => 0.8,
            Kind::GlowSquid => 0.8,
            Kind::Dolphin => 0.9,
            Kind::Turtle => 1.2,
            Kind::Axolotl => 0.75,
            Kind::Frog => 0.5,
            Kind::Tadpole => 0.4,
            Kind::Rabbit => 0.49,
            Kind::Fox => 0.6,
            Kind::Bee => 0.7,
            Kind::Armadillo => 0.7,
            Kind::Sniffer => 1.9,
            Kind::Parrot => 0.5,
            Kind::Bat => 0.5,
            Kind::Allay => 0.35,
            Kind::Endermite => 0.4,
            Kind::Zoglin => 1.3964844,
            Kind::Creaking => 0.9,
            Kind::HappyGhast => 4.0,
            Kind::CopperGolem => 0.49,
            Kind::ArmorStand => 0.5,
            Kind::Cushion => 0.999,
            Kind::SulfurCube => 0.49,
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
            // minecraft-data: breeze 0,6 × 1,77.
            Kind::Breeze => 0.6,
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
            Kind::Horse | Kind::Donkey | Kind::Mule | Kind::SkeletonHorse | Kind::ZombieHorse => 1.3964844,
            _ => 0.9,
        }
    }

    /// Высота ящика взрослого (minecraft-data, entities.json).
    pub fn height(self) -> f64 {
        match self {
            Kind::Llama => 1.87,
            Kind::TraderLlama => 1.87,
            Kind::WanderingTrader => 1.95,
            Kind::Camel => 2.375,
            Kind::Goat => 1.3,
            Kind::PolarBear => 1.4,
            Kind::Panda => 1.25,
            Kind::Ocelot => 0.7,
            Kind::Mooshroom => 1.4,
            Kind::Cod => 0.3,
            Kind::Salmon => 0.4,
            Kind::TropicalFish => 0.4,
            Kind::Pufferfish => 0.7,
            Kind::Squid => 0.8,
            Kind::GlowSquid => 0.8,
            Kind::Dolphin => 0.6,
            Kind::Turtle => 0.4,
            Kind::Axolotl => 0.42,
            Kind::Frog => 0.5,
            Kind::Tadpole => 0.3,
            Kind::Rabbit | Kind::Bee => 0.6,
            Kind::Fox => 0.7,
            Kind::Armadillo => 0.65,
            Kind::Sniffer => 1.75,
            Kind::Parrot => 0.9,
            Kind::Bat => 0.9,
            Kind::Allay => 0.6,
            Kind::Endermite => 0.3,
            Kind::Zoglin => 1.4,
            Kind::Creaking => 2.7,
            Kind::HappyGhast => 4.0,
            Kind::CopperGolem => 0.98,
            Kind::ArmorStand => 1.975,
            Kind::Cushion => 0.249,
            Kind::SulfurCube => 0.49,
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
            Kind::Breeze => 1.77,
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
            Kind::Warden => 2.9,
            Kind::Horse => 1.6,
            Kind::Donkey => 1.5,
            Kind::Mule => 1.6,
            Kind::SkeletonHorse => 1.6,
            Kind::ZombieHorse => 1.6,
        }
    }

    /// Здоровье (вики: корова и свинья 10, овца 8, курица 4).
    pub fn max_health(self) -> f32 {
        match self {
            Kind::Llama => 30.0,
            Kind::TraderLlama => 30.0,
            Kind::WanderingTrader => 20.0,
            Kind::Camel => 32.0,
            Kind::Goat => 10.0,
            Kind::PolarBear => 30.0,
            Kind::Panda => 20.0,
            Kind::Ocelot => 10.0,
            Kind::Mooshroom => 10.0,
            Kind::Cod => 3.0,
            Kind::Salmon => 3.0,
            Kind::TropicalFish => 3.0,
            Kind::Pufferfish => 3.0,
            Kind::Squid => 10.0,
            Kind::GlowSquid => 10.0,
            Kind::Dolphin => 10.0,
            Kind::Turtle => 30.0,
            Kind::Axolotl => 14.0,
            Kind::Frog => 10.0,
            Kind::Tadpole => 6.0,
            Kind::Rabbit => 3.0,
            Kind::Fox | Kind::Bee => 10.0,
            Kind::Armadillo => 12.0,
            Kind::Sniffer => 14.0,
            Kind::Parrot => 6.0,
            Kind::Bat => 6.0,
            Kind::Allay => 20.0,
            Kind::Endermite => 8.0,
            Kind::Zoglin => 40.0,
            Kind::Creaking => 1.0,
            Kind::HappyGhast => 20.0,
            Kind::CopperGolem => 12.0,
            Kind::ArmorStand => 5.0,
            Kind::Cushion => 1.0,
            Kind::SulfurCube => 4.0,
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
            Kind::Breeze => breeze::HEALTH,
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
            Kind::Warden => 500.0,
            Kind::Horse | Kind::Donkey | Kind::Mule => 22.0,
            Kind::SkeletonHorse | Kind::ZombieHorse => 15.0,
        }
    }

    /// Скорость шага, блоков за такт при множителе 1 (вики, Cow/Pig/Sheep/
    /// Chicken, «Attributes»: 0,2; 0,25; 0,23; 0,25).
    pub fn speed(self) -> f64 {
        match self {
            Kind::Llama => 0.175,
            Kind::TraderLlama => 0.175,
            Kind::WanderingTrader => 0.5,
            Kind::Camel => 0.09,
            Kind::Goat => 0.2,
            Kind::PolarBear => 0.25,
            Kind::Panda => 0.15,
            Kind::Ocelot => 0.3,
            Kind::Mooshroom => 0.2,
            Kind::Cod => 0.7,
            Kind::Salmon => 0.7,
            Kind::TropicalFish => 0.7,
            Kind::Pufferfish => 0.7,
            Kind::Squid => 0.7,
            Kind::GlowSquid => 0.7,
            Kind::Dolphin => 1.2,
            Kind::Turtle => 0.25,
            Kind::Axolotl => 1.0,
            Kind::Frog => 1.0,
            Kind::Tadpole => 1.0,
            Kind::Rabbit | Kind::Fox => 0.3,
            Kind::Bee => 0.6,
            Kind::Armadillo => 0.14,
            Kind::Sniffer => 0.1,
            Kind::Parrot => 0.2,
            Kind::Bat => 0.1,
            Kind::Allay => 0.1,
            Kind::Endermite => 0.25,
            Kind::Zoglin => 0.3,
            Kind::Creaking => 0.4,
            Kind::HappyGhast => 0.05,
            Kind::CopperGolem => 0.2,
            Kind::ArmorStand => 0.0,
            Kind::Cushion => 0.0,
            Kind::SulfurCube => 0.3,
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
            Kind::Breeze => breeze::SPEED,
            // [проверить] атрибут скорости: страж 0,5, древний 0,3.
            Kind::Guardian => 0.5,
            Kind::ElderGuardian => 0.3,
            // Вики, Wither: скорость 0,6.
            Kind::Wither => 0.6,
            Kind::Warden => 0.3,
            Kind::Horse => 0.225,
            Kind::Donkey | Kind::Mule => 0.175,
            Kind::SkeletonHorse | Kind::ZombieHorse => 0.2,
        }
    }

    /// Съедобное для разведения (вики, Breeding).
    pub fn likes(self, item: &str) -> bool {
        match self {
            Kind::Armadillo => item == "spider_eye",
            Kind::Sniffer => item == "torchflower_seeds",
            Kind::Cow | Kind::Sheep | Kind::Goat | Kind::Mooshroom => item == "wheat",
            Kind::Horse | Kind::Donkey => matches!(item, "golden_carrot" | "golden_apple" | "enchanted_golden_apple"),
            Kind::Llama | Kind::TraderLlama => item == "hay_block",
            Kind::Camel => item == "cactus",
            Kind::Panda => item == "bamboo",
            Kind::Ocelot => matches!(item, "cod" | "salmon"),
            Kind::Turtle => item == "seagrass",
            Kind::Axolotl => item == "tropical_fish_bucket",
            Kind::Frog | Kind::Tadpole => item == "slime_ball",
            Kind::Rabbit => matches!(item, "carrot" | "golden_carrot" | "dandelion"),
            Kind::Fox => matches!(item, "sweet_berries" | "glow_berries"),
            Kind::Bee => wildlife::flower(item),
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
            Kind::Llama => ("minecraft:entity.llama.ambient", "minecraft:entity.llama.hurt", "minecraft:entity.llama.death"),
            Kind::TraderLlama => ("minecraft:entity.llama.ambient", "minecraft:entity.llama.hurt", "minecraft:entity.llama.death"),
            Kind::WanderingTrader => ("minecraft:entity.wandering_trader.ambient", "minecraft:entity.wandering_trader.hurt", "minecraft:entity.wandering_trader.death"),
            Kind::Camel => ("minecraft:entity.camel.ambient", "minecraft:entity.camel.hurt", "minecraft:entity.camel.death"),
            Kind::Goat => ("minecraft:entity.goat.ambient", "minecraft:entity.goat.hurt", "minecraft:entity.goat.death"),
            Kind::PolarBear => ("minecraft:entity.polar_bear.ambient", "minecraft:entity.polar_bear.hurt", "minecraft:entity.polar_bear.death"),
            Kind::Panda => ("minecraft:entity.panda.ambient", "minecraft:entity.panda.hurt", "minecraft:entity.panda.death"),
            Kind::Ocelot => ("minecraft:entity.ocelot.ambient", "minecraft:entity.ocelot.hurt", "minecraft:entity.ocelot.death"),
            Kind::Mooshroom => ("minecraft:entity.cow.ambient", "minecraft:entity.cow.hurt", "minecraft:entity.cow.death"),
            Kind::Cod => ("minecraft:entity.cod.ambient", "minecraft:entity.cod.hurt", "minecraft:entity.cod.death"),
            Kind::Salmon => ("minecraft:entity.salmon.ambient", "minecraft:entity.salmon.hurt", "minecraft:entity.salmon.death"),
            Kind::TropicalFish => ("minecraft:entity.tropical_fish.ambient", "minecraft:entity.tropical_fish.hurt", "minecraft:entity.tropical_fish.death"),
            Kind::Pufferfish => ("minecraft:entity.puffer_fish.ambient", "minecraft:entity.puffer_fish.hurt", "minecraft:entity.puffer_fish.death"),
            Kind::Squid => ("minecraft:entity.squid.ambient", "minecraft:entity.squid.hurt", "minecraft:entity.squid.death"),
            Kind::GlowSquid => ("minecraft:entity.glow_squid.ambient", "minecraft:entity.glow_squid.hurt", "minecraft:entity.glow_squid.death"),
            Kind::Dolphin => ("minecraft:entity.dolphin.ambient", "minecraft:entity.dolphin.hurt", "minecraft:entity.dolphin.death"),
            Kind::Turtle => ("minecraft:entity.turtle.ambient_land", "minecraft:entity.turtle.hurt", "minecraft:entity.turtle.death"),
            Kind::Axolotl => ("minecraft:entity.axolotl.ambient", "minecraft:entity.axolotl.hurt", "minecraft:entity.axolotl.death"),
            Kind::Frog => ("minecraft:entity.frog.ambient", "minecraft:entity.frog.hurt", "minecraft:entity.frog.death"),
            Kind::Tadpole => ("minecraft:entity.tadpole.ambient", "minecraft:entity.tadpole.hurt", "minecraft:entity.tadpole.death"),
            Kind::Rabbit => ("minecraft:entity.rabbit.ambient", "minecraft:entity.rabbit.hurt", "minecraft:entity.rabbit.death"),
            Kind::Fox => ("minecraft:entity.fox.ambient", "minecraft:entity.fox.hurt", "minecraft:entity.fox.death"),
            Kind::Bee => ("", "minecraft:entity.bee.hurt", "minecraft:entity.bee.death"),
            Kind::Armadillo => ("minecraft:entity.armadillo.ambient", "minecraft:entity.armadillo.hurt", "minecraft:entity.armadillo.death"),
            Kind::Sniffer => ("minecraft:entity.sniffer.ambient", "minecraft:entity.sniffer.hurt", "minecraft:entity.sniffer.death"),
            Kind::Parrot => ("minecraft:entity.parrot.ambient", "minecraft:entity.parrot.hurt", "minecraft:entity.parrot.death"),
            Kind::Bat => ("minecraft:entity.bat.ambient", "minecraft:entity.bat.hurt", "minecraft:entity.bat.death"),
            Kind::Allay => ("minecraft:entity.allay.ambient_without_item", "minecraft:entity.allay.hurt", "minecraft:entity.allay.death"),
            Kind::Endermite => ("minecraft:entity.endermite.ambient", "minecraft:entity.endermite.hurt", "minecraft:entity.endermite.death"),
            Kind::Zoglin => ("minecraft:entity.zoglin.ambient", "minecraft:entity.zoglin.hurt", "minecraft:entity.zoglin.death"),
            Kind::Creaking => ("minecraft:entity.creaking.ambient", "minecraft:entity.creaking.hurt", "minecraft:entity.creaking.death"),
            Kind::HappyGhast => ("minecraft:entity.happy_ghast.ambient", "minecraft:entity.happy_ghast.hurt", "minecraft:entity.happy_ghast.death"),
            Kind::CopperGolem => ("", "minecraft:entity.copper_golem.hurt", "minecraft:entity.copper_golem.death"),
            Kind::ArmorStand => ("", "minecraft:entity.armor_stand.hit", "minecraft:entity.armor_stand.break"),
            Kind::Cushion => ("", "", ""),
            // [проверить] имена звуков куба серы (в прогоне снят только порядок пакетов).
            Kind::SulfurCube => ("", "minecraft:entity.sulfur_cube.hurt", "minecraft:entity.sulfur_cube.death"),
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
            // Вики, Breeze, «Sounds»: на земле — idle_ground [проверить: в воздухе idle_air].
            Kind::Breeze => ("minecraft:entity.breeze.idle_ground", "minecraft:entity.breeze.hurt", "minecraft:entity.breeze.death"),
            Kind::LeashKnot => ("", "", ""),
            // Вики, Iron Golem и Snow Golem, «Sounds»: у железного голоса в покое нет.
            Kind::IronGolem => ("", "minecraft:entity.iron_golem.hurt", "minecraft:entity.iron_golem.death"),
            Kind::SnowGolem => ("minecraft:entity.snow_golem.ambient", "minecraft:entity.snow_golem.hurt", "minecraft:entity.snow_golem.death"),
            // Голоса в воде; на суше — свои (guardian::sounds).
            Kind::Guardian => ("minecraft:entity.guardian.ambient", "minecraft:entity.guardian.hurt", "minecraft:entity.guardian.death"),
            Kind::ElderGuardian => ("minecraft:entity.elder_guardian.ambient", "minecraft:entity.elder_guardian.hurt", "minecraft:entity.elder_guardian.death"),
            Kind::Wither => ("minecraft:entity.wither.ambient", "minecraft:entity.wither.hurt", "minecraft:entity.wither.death"),
            Kind::Pillager | Kind::Vindicator | Kind::Evoker | Kind::Illusioner | Kind::Vex | Kind::Ravager | Kind::EvokerFangs => illager::sounds(self),
            Kind::Warden => ("minecraft:entity.warden.ambient", "minecraft:entity.warden.hurt", "minecraft:entity.warden.death"),
            Kind::Horse => ("minecraft:entity.horse.ambient", "minecraft:entity.horse.hurt", "minecraft:entity.horse.death"),
            Kind::Donkey => ("minecraft:entity.donkey.ambient", "minecraft:entity.donkey.hurt", "minecraft:entity.donkey.death"),
            Kind::Mule => ("minecraft:entity.mule.ambient", "minecraft:entity.mule.hurt", "minecraft:entity.mule.death"),
            Kind::SkeletonHorse => ("minecraft:entity.skeleton_horse.ambient", "minecraft:entity.skeleton_horse.hurt", "minecraft:entity.skeleton_horse.death"),
            Kind::ZombieHorse => ("minecraft:entity.zombie_horse.ambient", "minecraft:entity.zombie_horse.hurt", "minecraft:entity.zombie_horse.death"),
        }
    }

    /// Звук шага (вики, Sounds).
    pub fn step_sound(self) -> &'static str {
        match self {
            Kind::Turtle => "minecraft:entity.turtle.shamble",
            Kind::Frog => "minecraft:entity.frog.step",
            Kind::Horse | Kind::Donkey | Kind::Mule | Kind::SkeletonHorse | Kind::ZombieHorse => "minecraft:entity.horse.step",
            Kind::Cow => "minecraft:entity.cow.step",
            Kind::Warden => "minecraft:entity.warden.step",
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
            Kind::Endermite => "minecraft:entity.endermite.step",
            Kind::Zoglin => "minecraft:entity.zoglin.step",
            Kind::Creaking => "minecraft:entity.creaking.step",
            Kind::CopperGolem => "minecraft:entity.copper_golem.step",
            Kind::ArmorStand => "",
            Kind::Cushion => "",
            Kind::SulfurCube => "",
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

/// Память об уроне: вики, Java Edition 24w21a, «2 seconds».
/// https://minecraft.wiki/w/Java_Edition_24w21a
pub const PANIC_TICKS: i32 = 40;

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
    fn catalog(&self) -> crate::gameplay_catalog::Catalog { crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2) }
    fn block(&self, x: i32, y: i32, z: i32) -> i32;
    fn loaded(&self, chunk_x: i32, chunk_z: i32) -> bool;

    fn piglin_safe(&self) -> bool { false }
    fn clouds(&self) -> bool { false }

    /// Открыто ли небо над клеткой (для солнца). Без мира — нет.
    fn open_sky(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }

    /// Биом клетки (кролики, лисы, пчёлы, водные виды).
    fn biome(&self, _x: i32, _y: i32, _z: i32) -> &'static str { "plains" }

    /// Условия для кроликов, лис и пчёл.
    fn griefing(&self) -> bool { true }
    fn raining(&self) -> bool { false }
    fn always_day(&self) -> bool { false }

    /// Идёт ли в клетке дождь (World::rain_at). Без мира — нет.
    fn rain(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }

    fn thunder(&self) -> bool { false }

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
    fn catalog(&self) -> crate::gameplay_catalog::Catalog { self.0.catalog() }
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
    SquidInk,
    GlowSquidInk,
    SonicBoom,
    /// «item» с предметом — у волчьей брони (play/mobs.rs подставляет предмет).
    ItemBreak,
}

impl Particle {
    pub fn id(self) -> i32 {
        match self {
            Particle::DamageIndicator => 7,
            Particle::SweepAttack => 67,
            Particle::SquidInk => 66,
            Particle::GlowSquidInk => 99,
            Particle::SonicBoom => 28,
            Particle::ItemBreak => 47,
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
    CommandHurt { source: Source, origin: crate::combat::DamageOrigin },

    /// Смерть: клиенты играют падение набок.
    Death,

    /// Частицы приземления: блок под ногами и их число.
    Landing { block: i32, count: i32 },
    /// Отскок от опоры; только игровое событие, без отдельного сетевого пакета.
    Bounce,

    /// Частицы осколков блока: состояние, место, разброс, скорость, число (поломка стойки).
    BlockBurst { block: i32, at: (f64, f64, f64), offset: (f32, f32, f32), speed: f32, count: i32 },

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
    pub age_locked: bool,
    pub growth_cooldown: u64,

    /// Сколько тактов ещё влюблён (0 — нет).
    pub love: i32,
    /// Кто покормил родителя; сбрасывается после рождения [проверить: сохранение].
    pub love_player: Option<[u8; 16]>,

    /// Овца: остригнута ли, цвет шерсти (0 — белая).
    pub sheared: bool,
    /// Цвет овцы; у курицы вариант: 0=temperate, 1=cold, 2=warm (старый 0 сохранён).
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
    /// Каталог владельца и состояние транспорта; не являются данными самого моба.
    pub catalog: crate::gameplay_catalog::Catalog,
    pub riding: bool,

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
    /// Игрок, которому принадлежит недавний успешный удар (вики, Statistics).
    pub kill_credit: Option<[u8; 16]>,

    /// Опыт, который надо ронять шариками: ставит смерть, забирает
    /// `Items::flush_mob_events`.
    pub pending_xp: i32,
    /// Принятые шипы; переживают смерть носителя до следующего такта.
    pub enchant_retaliation: Vec<combat_enchantments::Retaliation>,

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
    /// Метки сущности (`Tags` данных; выбор `@e[tag=…]`).
    pub tags: Vec<String>,

    /// На поводке: у кого (mob/leash.rs).
    pub leash: Option<leash::Holder>,

    /// Приручение волка и кошки (mob/tame.rs).
    pub tame: tame::State,

    /// Големы: цель, «построен игроком», тыква снежного (mob/golem.rs).
    pub golem: golem::State,
    /// Куб серы: поглощённый блок, запал, пауза подбора (mob/sulfur_cube.rs).
    pub sulfur: sulfur_cube::State,
    /// Фантом: размер, якорь кружения, пике (mob/phantom.rs).
    pub phantom: phantom::State,
    /// Бриз: поза, выстрел, прыжок (mob/breeze.rs).
    pub breeze: breeze::State,

    /// Стражи: луч, шипы, проклятие (mob/guardian.rs).
    pub guardian: guardian::State,

    /// Иссушитель: зарядка, головы, черепа (mob/wither.rs).
    pub wither: wither::State,
    /// Разбойники: заклинания, арбалет, рёв, жизнь вредины и челюстей (mob/illager.rs).
    pub illager: illager::State,
    /// Наземные животные и странствующий торговец (mob/land.rs).
    pub land: land::State,
    pub water: water::State,
    pub warden: warden::State,

    /// Лошади, ослы, мулы и нежить-лошади (mob/horse.rs).
    pub horse: horse::State,

    pub newmobs: newmobs::State,
    pub stand: armor_stand::State,
    pub foot_enchants: movement_enchantments::State,
    pub equipment: Option<equipment::Gear>,
    pub equipment_drop_chances: equipment::Chances,

    /// Кролик, лиса и пчела (mob/wildlife.rs).
    pub wild: wildlife::State,

    /// Малые мирные мобы [Bedrock: потом — особые свойства].
    pub small: small::State,
}

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

        // Куб серы по умолчанию малый (наблюдение: призванный с NBT без Size — Size 0, 4 здоровья).
        if kind == Kind::SulfurCube {
            nether.size = sulfur_cube::SMALL;
            vitals.health = 4.0;
            vitals.max_health = 4.0;
        }

        // Шалкер без цвета — 16 (minecraft-data: color; замер мира оригинала:
        // у всех шалкеров городов Color 16).
        let h = hostile::State { bow: kind.skeleton(), shell_color: 16, hand_drop: DEFAULT_DROP, ..hostile::State::default() };

        let mut mob = Self {
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
            age_locked: false,
            growth_cooldown: 0,
            love: 0,
            love_player: None,
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
            catalog: crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2),
            riding: false,
            strafe: (0.0, 0.0),
            wall: false,
            h,
            by_player: false,
            late: false,
            spawn_poof: false,
            attr_changes: 0,
            teleports: 0,
            xp_until: 0,
            kill_credit: None,
            pending_xp: 0,
            enchant_retaliation: Vec::new(),
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
            tags: Vec::new(),
            leash: None,
            tame: tame::State { variant: tame::default_variant(kind), ..tame::State::default() },
            golem: golem::State::default(),
            sulfur: sulfur_cube::State::default(),
            phantom: phantom::State::default(),
            breeze: breeze::State::default(),
            guardian: guardian::State::default(),
            wither: wither::State::default(),
            illager: illager::State::default(),
            land: land::State::new(kind),
            water: water::State::new(kind, at),
            warden: warden::State::default(),
            horse: horse::State::default(),
            wild: wildlife::State::default(),
            small: small::State::new(kind, entity_id),
            newmobs: newmobs::State::default(),
            stand: armor_stand::State::default(),
            foot_enchants: movement_enchantments::State::default(),
            equipment: None,
            equipment_drop_chances: [DEFAULT_DROP; 6],
        };
        horse::defaults(&mut mob);
        mob
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

    pub fn credit_to(&mut self, uuid: [u8; 16]) {
        self.kill_credit = Some(uuid);
        self.credit_player();
    }

    pub fn credit_player(&mut self) {
        self.xp_until = self.ticks + XP_CREDIT_TICKS;
    }

    pub fn movement_speed(&self) -> f64 {
        let base = if horse::equine(self.kind) { self.horse.speed } else { self.kind.speed() };
        base + crate::soul::bonus(self.foot_enchants.soul)
    }

    pub fn is_baby(&self) -> bool {
        self.age < 0
    }

    pub fn is_dead(&self) -> bool {
        self.dead_for > 0 || self.vitals.dead
    }

    /// Ширина и высота ящика: у детёныша вдвое меньше (вики, Baby mobs).
    pub fn size(&self) -> (f64, f64) {
        if self.kind==Kind::Cushion {return (0.999, 0.249);}
        if self.kind==Kind::SulfurCube {let w=sulfur_cube::width(self.nether.size); return (w, w);}
        if self.kind==Kind::ArmorStand {return if self.stand.marker {(0.0,0.0)} else if self.stand.small {(0.25,0.9875)} else {(0.5,1.975)};}
        if let Some(size) = water::size(self) { return size; }

        if self.kind == Kind::Warden && self.warden.invulnerable() { return (0.9, 1.0); }
        if self.kind == Kind::HappyGhast && self.is_baby() { return (0.95, 0.95); }
        if self.catalog.changed_rules() && self.is_baby()
            && let Some(size) = crate::gameplay_catalog::changed_rules::values().baby_sizes.get(self.kind.name())
        { return (size[0], size[1]); }
        if self.catalog.changed_rules() && self.is_baby()
            && matches!(self.kind, Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::ZombifiedPiglin | Kind::Villager | Kind::Piglin)
        {
            return (self.kind.width() * 0.5, crate::gameplay_catalog::changed_rules::values().baby_humanoid_height);
        }

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
        // [проверить] Индивидуальные глаза и точки посадки детёнышей 26.2.
        if self.catalog.changed_rules() && self.is_baby() { return self.size().1 * 0.85; }
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
        if self.kind==Kind::Goat && self.land.screaming {
            return ("minecraft:entity.goat.screaming.ambient","minecraft:entity.goat.screaming.hurt","minecraft:entity.goat.screaming.death");
        }
        if matches!(self.kind, Kind::Wolf | Kind::Cat) {
            return tame::sounds(self);
        }

        if guardian::guardian(self.kind) {
            return guardian::sounds(self);
        }

        if self.kind == Kind::Warden {
            let ambient = if self.warden.level() >= warden::ANGRY { "minecraft:entity.warden.angry" }
                else if self.warden.level() >= warden::AGITATED { "minecraft:entity.warden.agitated" }
                else { "minecraft:entity.warden.ambient" };
            return (ambient, "minecraft:entity.warden.hurt", "minecraft:entity.warden.death");
        }
        if self.kind == Kind::Fox && self.wild.sleeping {
            return ("minecraft:entity.fox.sleep", "minecraft:entity.fox.hurt", "minecraft:entity.fox.death");
        }
        if self.kind == Kind::Allay && self.small.held.is_some() {
            return ("minecraft:entity.allay.ambient_with_item", "minecraft:entity.allay.hurt", "minecraft:entity.allay.death");
        }
        if self.kind == Kind::HappyGhast && self.is_baby() {
            return ("minecraft:entity.ghastling.ambient", "minecraft:entity.ghastling.hurt", "minecraft:entity.ghastling.death");
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
        self.catalog = view.catalog();
        if self.wild.fresh {
            self.wild.fresh = false;
            return ai::Plan::default();
        }

        self.ticks += 1;
        ai::panic::tick(self);
        self.late = false;
        if self.kind == Kind::Warden && ctx.difficulty == crate::survival::PEACEFUL {
            self.h.despawn = true;
            return ai::Plan::default();
        }

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
        land::tick(self, rng);

        // Страж: в воде ли — и без ИИ (голоса суши и воды).
        if guardian::guardian(self.kind) {
            guardian::sense(self, view);
        }

        skeleton::snow_tick(self, view);

        // Кролик, лиса и пчела: свой такт (яд, гнев, смерть ужалившей пчелы).
        if wildlife::kind(self.kind) {
            wildlife::base_tick(self, view);
            if self.is_dead() { return ai::Plan::default(); }
        }

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

        if water::creature(self.kind) {
            water::sense(self, view);
            water::vitals(self, view);
            if self.is_dead() { return ai::Plan::default(); }
        }

        newmobs::lifecycle(self, view);
        if self.is_dead() || self.h.despawn { return ai::Plan::default(); }

        if self.kind == Kind::HappyGhast && self.vitals.health < self.vitals.max_health {
            let fast = (view.clouds() && (187.0..197.0).contains(&self.y)) || (!is_water(view.block(self.cell().0,self.cell().1,self.cell().2)) && view.rain(self.cell().0,(self.y+self.size().1).floor() as i32,self.cell().2));
            if self.ticks.is_multiple_of(if fast {20} else {600}) { self.vitals.health=(self.vitals.health+1.0).min(self.vitals.max_health); }
        }
        // Рост и пауза после разведения (вики, Breeding).
        // Детёныш-зомби не растёт (вики, Zombie: «baby zombies never grow up»).
        let was_baby = self.age < 0;
        growth::tick_age(self);
        if was_baby && self.age == 0 && self.kind == Kind::Turtle { water::scute(self); }
        if was_baby && self.age == 0 && self.kind == Kind::SulfurCube { sulfur_cube::grow(self); }

        if self.love > 0 {
            self.love -= 1;
            if self.love == 0 { self.love_player = None; }
        }

        if horse::equine(self.kind) { horse::tick(self, rng); }

        // NoAI: ни мысли, ни хода — и тяжести тоже: призванный в воздухе
        // висит, а толчок скорость копит, но не двигает (чёрный ящик,
        // cow-physics и cow-hit: корова NoAI висит на месте, скорость после
        // двух ударов 0,6).
        // Огонь: раз в 20 тактов на 1 (вики, Fire) — и у NoAI; а солнце
        // поджигает только мобов с ИИ (чёрный ящик, zombie-burn: зомби NoAI
        // в полдень цел).
        if !self.kind.object() && hostile::burn(self, view, ctx, rng, !self.no_ai) && !self.vitals.has_effect(crate::survival::EffectKind::FireResistance) {
            self.hurt(1.0, Source::OnFire, None);
        }

        let fireproof = self.kind.object() || self.fire_immune();
        if fireproof { self.vitals.extinguish(); }
        if !fireproof && !self.vitals.has_effect(crate::survival::EffectKind::FireResistance) {
            let (x, y, z) = self.cell();
            let state = view.block(x, y, z);
            let name = blocks::block_at_state(state);
            let campfire = crate::campfire::damage(state).max(crate::campfire::damage(view.block(x, (self.y - 0.01).floor() as i32, z)));
            if campfire > 0.0 { self.hurt(campfire, Source::Campfire, None); }
            if self.on_ground && blocks::block_at_state(view.block(x, (self.y - 0.2).floor() as i32, z)) == Some("magma_block") {
                self.hurt(1.0, Source::HotFloor, None);
            }
            if matches!(name, Some("fire" | "soul_fire")) {
                self.vitals.ignite(160);
                self.hurt(if name == Some("soul_fire") { 2.0 } else { 1.0 }, Source::InFire, None);
            }
            if name == Some("lava") { self.vitals.ignite(300); self.hurt(4.0, Source::Lava, None); }
        }

        // Подушка: без ИИ; пропала опора — ломается и роняет предмет (mob/cushion.rs).
        if self.kind==Kind::Cushion && !self.is_dead() {
            if !cushion::supported(view, self) { self.vitals.health=0.0; self.vitals.dead=true; self.die(); }
            return ai::Plan::default();
        }
        if self.kind==Kind::ArmorStand && !self.is_dead() {
            if !self.stand.marker && !self.stand.no_gravity {self.physics(view,0.0,false);}
            return ai::Plan::default();
        }

        // Кристалл Края стоит, где поставлен: ни тяжести, ни мыслей, ни голоса
        // (вики, End Crystal; чёрный ящик, end-crystal-explode: висит без сверок скорости).
        if self.kind == Kind::LeashKnot {
            let (x,y,z) = leash::knot_block(self);
            if !blocks::block_at_state(view.block(x,y,z)).is_some_and(|n| n.ends_with("_fence")) { self.h.despawn = true; }
        }
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

        // Иммунитет нужен и для прямого /effect, не только доставки зельем.
        if self.kind.spider() || crate::combat::mob_traits(self.kind.name()).0 {
            self.vitals.remove_effects(Some(crate::survival::EffectKind::Poison));
        }
        if crate::combat::mob_traits(self.kind.name()).0 {
            self.vitals.remove_effects(Some(crate::survival::EffectKind::Regeneration));
        }

        // Вики, Effect: каждый эффект тикает один раз; прежде raider терял
        // два такта, а witch/villager теряли периодический урон [проверить].
        // Кролик/лиса/пчела, лошади, водные и новые мобы тикают эффекты в своём
        // модуле (wildlife, horse, water, newmobs) — здесь их не дублируем.
        if !self.vitals.effects.is_empty() && !wildlife::kind(self.kind) && !horse::equine(self.kind) && !water::creature(self.kind)
            && !(newmobs::new_kind(self.kind) || self.kind == Kind::ZombifiedPiglin) {
            let effects = self.vitals.effects_tick_for(crate::combat::mob_traits(self.kind.name()).0);
            for (amount, source) in effects.hurt { self.hurt(amount, source, None); }
            if self.is_dead() { return ai::Plan::default(); }
        }

        if self.kind == Kind::Witch {
            hostile::witch_drink(self, view, ctx, rng);
        }
        if self.kind == Kind::Villager && self.no_ai { ai::voice(self, rng); }

        // Иссушитель без ИИ тоже подаёт голос (чёрный ящик, wither-half-armor:
        // entity.wither.ambient у NoAI).
        if self.kind == Kind::Wither && self.no_ai {
            ai::voice(self, rng);
        }

        if small::kind(self.kind) {
            small::tick(self, rng);
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

        if self.kind == Kind::HappyGhast {
            return newmobs::happy_tick(self, view, ctx, rng);
        }
        if horse::equine(self.kind) && self.horse.rider.is_some() && self.horse.tamed && self.horse.saddled() {
            self.brain.stop();
            ai::voice(self, rng);
            return ai::Plan::default();
        }
        if self.kind == Kind::Creaking && newmobs::freeze(self, view, ctx) {
            return ai::Plan::default();
        }

        // Лежащий в кровати житель стоит на месте (village_golems.rs).
        if self.kind == Kind::Villager && self.villager.summon.bed.is_some() {
            return ai::Plan::default();
        }

        ai::panic::burning(self);
        let was_on_ground = self.on_ground;
        let mut plan = if small::kind(self.kind) {
            small::think(self, view, ctx, rng)
        } else if land::animal(self.kind) || self.kind == Kind::WanderingTrader {
            land::think(self, view, ctx, rng)
        } else if water::creature(self.kind) {
            water::think(self, view, ctx, rng)
        } else if wildlife::kind(self.kind) {
            wildlife::think(self, view, ctx, rng)
        } else if matches!(self.kind, Kind::Wolf | Kind::Cat) {
            tame::think(self, view, ctx, rng)
        } else if self.kind == Kind::CopperGolem && self.newmobs.chest.is_some() {
            ai::walk(self, ai::Plan::default())
        } else if golem::golem(self.kind) {
            golem::think(self, view, ctx, rng)
        } else if self.kind == Kind::Warden {
            warden::think(self, view, ctx, rng)
        } else if guardian::guardian(self.kind) {
            guardian::think(self, view, ctx, rng)
        } else if self.kind == Kind::Breeze {
            breeze::think(self, view, ctx, rng)
        } else {
            ai::think(self, view, ctx, rng)
        };

        if let Some((x,_,z)) = self.water.egg_target.filter(|_| self.h.target.is_none()) {
            let dx = f64::from(x)+0.5-self.x;
            let dz = f64::from(z)+0.5-self.z;
            if dx*dx+dz*dz > 0.3 {
                self.face((-dx.atan2(dz).to_degrees()) as f32);
                plan.speed = 1.0;
                plan.jump = self.wall;
            }
        }

        // Пьющая ведьма стоит: модификатор «drinking» −0,25 к скорости 0,25
        // (чёрный ящик, witch-stand).
        if self.kind == Kind::Witch && self.h.drinking > 0 {
            plan.speed = 0.0;
        }

        // Шалкер сидит на грани: ни тяжести, ни шагов (вики, Shulker).
        if small::flying(self.kind) && !self.is_dead() {
            small::fly(self, view);
        } else if water::creature(self.kind) && !self.is_dead() {
            water::physics(self, view, &plan, rng);
        // Пчела, ещё не пошедшая в полёт, падает и стоит как всякий моб (чёрный ящик,
        // bee-sting: упала с 1 до 0, откинута ударом по земле, взлетела на +11).
        } else if self.kind == Kind::Bee && !self.is_dead() && self.wild.flying {
            wildlife::fly(self, view);
        } else if self.kind == Kind::Ghast && !self.is_dead() {
            nether::drift(self, view);
        } else if self.kind == Kind::Phantom && !self.is_dead() {
            phantom::fly(self, view);
        } else if guardian::guardian(self.kind) && !self.is_dead() {
            guardian::physics(self, view, rng);
        } else if self.kind == Kind::Vex && !self.is_dead() {
            illager::vex_fly(self);
        } else if self.kind == Kind::Breeze && !self.is_dead() && breeze::leap(self, view) {
            // Длинный прыжок бриза — своим ходом (mob/breeze.rs).
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
        if self.kind == Kind::SulfurCube && self.on_ground && !was_on_ground { sulfur_cube::landed(self); }

        plan
    }

    /// Движение живой сущности за такт (вики, Entity, «Motion»; чёрный
    /// ящик, cow-physics): сперва сдвиг на нынешнюю скорость (в первом такте
    /// падения она нулевая), потом тяжесть и сопротивление
    /// `v.y = (v.y − 0,08)·0,98`, трение `v.xz *= f·0,91`, где f —
    /// скользкость блока под ногами (в воздухе 1). Разгон по ходу — до сдвига:
    /// на земле `speed²·(0,6/f)³` (чёрный ящик, probe-cow-plain: у коровы
    /// 0,2 даёт 0,04 за такт — шаг 0,088 блока), в воздухе 0,02·speed.
    pub fn physics(&mut self, view: &dyn View, speed: f64, jump: bool) {
        let effect_speed = (1.0 + 0.2 * f64::from(self.vitals.effect_level(crate::survival::EffectKind::Speed)))
            * (1.0 - 0.15 * f64::from(self.vitals.effect_level(crate::survival::EffectKind::Slowness))).max(0.0);
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
            self.vy += 0.1 * f64::from(self.vitals.effect_level(crate::survival::EffectKind::JumpBoost));
        }

        // Утопленник плывёт к цели в воде: тяга `SWIM_THRUST · скорость` вперёд и
        // `скорость · подъём · 0,1` по вертикали, сопротивление 0,9, тяжести нет
        // (вики, Drowned; чёрный ящик, drowned-attack: скорость 0,02–0,04) [проверить].
        let swimming = self.kind == Kind::Drowned && in_water && self.h.swim && speed != 0.0;

        let slip = if self.on_ground { slipperiness(view.block(cell_x, (self.y - 0.5000001).floor() as i32, cell_z)) } else { 1.0 };
        let friction = if self.on_ground { slip * 0.91 } else { 0.91 };

        if swimming {
            let effective = speed * self.movement_speed();
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
            let effective = speed * if slime::cube(self.kind) { slime::speed(self.nether.size) } else { self.movement_speed() };
            // Куб: в воздухе 0,01·ввод — замер slime-hop (оригинал, три записи): прыжок 0,95 блока [проверить].
            let accel = if wades {
                0.02 * effective
            } else if self.on_ground {
                effective * effective * (0.6 / slip).powi(3)
            } else if slime::cube(self.kind) {
                0.01 * effective
            } else {
                // Ввод наземного моба равен attribute × goal modifier и в
                // воздухе. Фиксированные 0,02 давали полный разгон при
                // отдаче (чёрный ящик, cow-panic-run, panic-v1/v2).
                0.02 * effective
            };
            let rad = f64::from(self.yaw).to_radians();

            self.vx += -rad.sin() * accel;
            self.vz += rad.cos() * accel;
        } else if self.strafe != (0.0, 0.0) {
            // Шаг вбок и вперёд-назад: тело смотрит на цель, идёт боком.
            let (forward, left) = self.strafe;
            let length = (forward * forward + left * left).sqrt();
            let effective = length * self.movement_speed() * effect_speed;
            let accel = if self.on_ground { effective * effective * (0.6 / slip).powi(3) } else { 0.02 * effective };
            let rad = f64::from(self.yaw).to_radians();
            let (fx, fz) = (-rad.sin(), rad.cos());
            let (lx, lz) = (fz, -fx);

            self.vx += (fx * forward + lx * left) / length * accel;
            self.vz += (fz * forward + lz * left) / length * accel;
        }

        let before = (self.x, self.z);

        if (in_water || in_lava) && self.kind != Kind::Warden {
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
                    let volume = if matches!(self.kind, Kind::IronGolem | Kind::CopperGolem) { 1.0 } else { 0.15 };

                    self.events.push(Event::Sound { name, volume, pitch: 1.0 });
                }
            }
        }

        if !(self.kind.spider() && self.h.climbing) {
            let levitation = self.vitals.effect_level(crate::survival::EffectKind::Levitation);
            if levitation > 0 {
                self.vy += (0.05 * f64::from(levitation) - self.vy) * 0.2;
                self.vitals.fall = 0.0;
            } else {
                let gravity = if self.vy <= 0.0 && self.vitals.has_effect(crate::survival::EffectKind::SlowFalling) { 0.01 } else { GRAVITY };
                self.vy -= gravity;
            }
            self.vy *= VERTICAL_DRAG;
        }

        // Курица падает медленно (вики, Chicken: «slow falling»): в воздухе
        // падение после тяжести замедляется ещё в 0,6 — предельная скорость
        // −0,114 (чёрный ящик, chicken-fall).
        if self.kind == Kind::Chicken && !self.on_ground && self.vy < 0.0 {
            self.vy *= 0.6;
        }

        let floor_drag = movement_enchantments::floor_drag(self, view);
        self.vx *= friction * floor_drag;
        self.vz *= friction * floor_drag;

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
            let lift = clip_y(view, start, half, height, if self.kind == Kind::Camel { 1.5 } else if horse::equine(self.kind) { 1.0 } else { STEP_HEIGHT });
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
            let below = view.block(self.x.floor() as i32, (self.y - 0.001).floor() as i32, self.z.floor() as i32);
            let bed = blocks::block_at_state(below).is_some_and(|n| n.ends_with("_bed") && n != "straw_bed");
            let slime = blocks::block_at_state(below) == Some("slime_block");
            if view.catalog().changed_rules() && (bed || slime) {
                self.vy = -dy * if bed { crate::gameplay_catalog::changed_rules::values().bed_restitution } else { 1.0 };
                self.events.push(Event::Bounce);
            } else { self.vy = 0.0; }
        } else {
            self.on_ground = false;

            if dy > 0.0 && (moved_y - dy).abs() > 1.0e-9 {
                self.vy = 0.0;
            }
        }

        // Высота падения копится по пройденному вниз, и такт приземления
        // в счёт урона (чёрный ящик, cow-physics: с шести блоков — урон 3)
        // (вики, Fall damage); для частиц он не в счёте — берётся высота до него.
        let before = self.vitals.fall as f32;

        if moved_y < 0.0 {
            self.vitals.fall = f64::from(self.vitals.fall as f32 + (-moved_y) as f32);
        }

        if !self.on_ground {
            return;
        }

        let fall = self.vitals.fall as f32;

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
        // Вики, Frog: «5 ... less fall damage».
        let safe_fall=if self.kind==Kind::Goat { 10.0 } else if self.kind==Kind::Camel { 6.0 } else if self.kind==Kind::Frog { crate::survival::SAFE_FALL+5.0 } else { crate::survival::SAFE_FALL };
        let safe_fall = safe_fall + self.vitals.effect_level(crate::survival::EffectKind::JumpBoost) as f32;
        if fall > safe_fall && !self.vitals.has_effect(crate::survival::EffectKind::SlowFalling) && !matches!(self.kind, Kind::Chicken | Kind::Bee) && self.kind != Kind::Breeze && !golem::golem(self.kind) {
            let damage = if horse::equine(self.kind) { horse::fall_damage(fall) } else { ((fall - safe_fall - 1.0e-4) * if self.kind==Kind::Camel { 0.5 } else { 1.0 }).ceil() };
            if horse::equine(self.kind) && self.horse.rider.is_some() { self.horse.rider_fall += damage; }

            let damage = if view.catalog().changed_rules()
                && blocks::block_at_state(below).is_some_and(|n| n.ends_with("_bed") && n != "straw_bed") { damage * 0.5 } else { damage };
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

    /// Вики, Fire Aspect/Flame: огонь должен быть виден loot уже при
    /// смертельном ударе. Неудавшийся удар не оставляет поджог [проверить].
    pub fn hurt_burning(&mut self, amount: f32, source: Source, push: Option<Push>, breach: f32, ticks: i32) -> Option<Hit> {
        let before = self.vitals.fire;
        if !self.fire_immune() { self.vitals.ignite(ticks); }
        let hit = self.hurt_breached(amount, source, push, breach);
        if hit.is_none() { self.vitals.fire = before; }
        hit
    }

    /// Вики, Drops/Experience: удар игрока и его прирученного волка даёт
    /// кредит уже смертельному попаданию. die() рассчитывает добычу внутри
    /// hurt(), поэтому назначать владельца после него слишком поздно.
    /// Непринятый удар не обновляет ни владельца, ни «добычу».
    pub fn hurt_credited(&mut self, amount: f32, source: Source, push: Option<Push>, owner: [u8; 16], looting: i32) -> Option<Hit> {
        self.hurt_credited_burning(amount, source, push, owner, looting, 0)
    }

    pub fn hurt_credited_burning(&mut self, amount: f32, source: Source, push: Option<Push>, owner: [u8; 16], looting: i32, fire: i32) -> Option<Hit> {
        let before = (self.kill_credit, self.xp_until, self.looting);
        self.credit_to(owner);
        self.looting = looting.max(0);
        let hit = self.hurt_burning(amount, source, push, 1.0, fire);
        if hit.is_none() { (self.kill_credit, self.xp_until, self.looting) = before; }
        hit
    }

    fn fire_immune(&self) -> bool {
        matches!(self.kind, Kind::Blaze | Kind::Ghast | Kind::MagmaCube | Kind::Strider | Kind::WitherSkeleton | Kind::ZombifiedPiglin | Kind::Wither | Kind::EnderDragon | Kind::Zoglin | Kind::Warden)
    }

    pub fn effect_melee_damage(&self, base: f32) -> f32 {
        use crate::survival::EffectKind;
        (base + 3.0 * self.vitals.effect_level(EffectKind::Strength) as f32
            - 4.0 * self.vitals.effect_level(EffectKind::Weakness) as f32).max(0.0)
    }

    /// Вики, Effect/Undead: единая проверка для splash/cloud/tipped arrow.
    /// Кристалл, узел, челюсти и снаряды не живые [проверить].
    pub fn accepts_effect(&self, effect: crate::survival::EffectKind) -> bool {
        use crate::survival::EffectKind;
        !self.is_dead()
            && !matches!(self.kind, Kind::EnderDragon | Kind::EndCrystal | Kind::LeashKnot | Kind::EvokerFangs | Kind::ShulkerBullet)
            && (self.kind != Kind::Wither || effect.instant())
            && !(crate::combat::mob_traits(self.kind.name()).0 && matches!(effect, EffectKind::Poison | EffectKind::Regeneration))
            && !(self.kind.spider() && effect == EffectKind::Poison)
            && !(self.kind == Kind::Slime && effect == EffectKind::Oozing)
            && !(self.kind == Kind::Silverfish && effect == EffectKind::Infested)
    }

    /// Удар. `push` — кто и как толкнул (None — урон без толчка: падение,
    /// команда). Возвращает исход или None, если удар не прошёл (вики,
    /// Damage: «Invulnerability frames»).
    pub fn hurt(&mut self, amount: f32, source: Source, push: Option<Push>) -> Option<Hit> {
        self.hurt_breached(amount, source, push, 1.0)
    }

    /// Пробивание действует и на естественную броню моба (вики, Breach:
    /// «natural armor points»). Остальной путь урона общий [проверить].
    pub fn hurt_breached(&mut self, amount: f32, source: Source, push: Option<Push>, breach: f32) -> Option<Hit> {
        if crate::frost::protects_boots(equipment::slot(self, 2), source) { return None; }
        if self.kind==Kind::ArmorStand {return armor_stand::hurt(self,amount,source);}
        if self.kind==Kind::Cushion {return cushion::hurt(self,amount,source);}
        if self.kind==Kind::SulfurCube && sulfur_cube::resists(self, source) {
            sulfur_cube::launch(self, amount, push);
            return None;
        }
        if horse::undead(self.kind) && source == Source::Drown { return None; }
        if self.is_dead() || (self.fire_immune() && matches!(source, Source::InFire | Source::Campfire | Source::HotFloor | Source::OnFire | Source::Lava | Source::Fireball)) {
            return None;
        }
        if self.kind == Kind::Zoglin && matches!(source, Source::InFire | Source::OnFire | Source::Lava | Source::Drown) { return None; }
        if matches!(self.kind,Kind::CopperGolem|Kind::HappyGhast) && source == Source::Fall { return None; }
        if self.kind == Kind::CopperGolem && source == Source::Drown { return None; }
        if self.kind == Kind::HappyGhast && self.is_baby() && source == Source::Drown { return None; }
        if self.kind == Kind::Creaking && self.newmobs.heart.is_some() && source != Source::GenericKill {
            self.newmobs.resin_hit |= matches!(source,Source::PlayerAttack|Source::MaceSmash|Source::Arrow|Source::Thrown|Source::PlayerExplosion);
            self.events.push(Event::Sound { name: "minecraft:entity.creaking.sway", volume: 1.0, pitch: 1.0 });
            return None;
        }

        if self.kind == Kind::Warden && (matches!(source, Source::InFire | Source::OnFire | Source::Lava)
            || (self.warden.invulnerable() && source != Source::GenericKill)) { return None; }
        if self.kind == Kind::Allay && self.small.held.is_some() && push.is_some_and(|p| p.cause == self.tame.owner_id && p.cause != 0) {
            return None;
        }
        if small::flying(self.kind) && source == Source::Fall {
            return None;
        }
        let amount = if self.kind == Kind::Armadillo && self.small.roll != 0 && !matches!(source, Source::GenericKill | Source::OutOfWorld) {
            (amount - 1.0).max(0.0) / 2.0
        } else { amount };
        if self.kind == Kind::Bat { self.small.resting = false; }
        if self.kind == Kind::Armadillo && !self.no_ai { self.small.threat = 100; }

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
        if self.kind == Kind::EnderDragon && !matches!(source, Source::Spear | Source::PlayerAttack | Source::Arrow | Source::Explosion | Source::PlayerExplosion | Source::GenericKill) {
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

        if matches!(source, Source::InFire | Source::Campfire | Source::HotFloor | Source::OnFire | Source::Lava)
            && self.vitals.has_effect(crate::survival::EffectKind::FireResistance) {
            return None;
        }

        // Ведьма: магия на 85 % слабее (вики, Witch).
        let amount = if self.kind == Kind::Witch && matches!(source, Source::Magic | Source::IndirectMagic) { amount * 0.15 } else { amount };
        let before = self.vitals.health;
        let amount = if armored(source) {
            if breach != 1.0 {
                let toughness = equipment::bonus(self,crate::attributes::ARMOR_TOUGHNESS) + if horse::equine(self.kind) { horse::toughness(self) } else { 0.0 };
                crate::combat::after_armor_breached(amount, self.armor(), toughness, breach)
            }
            else if self.equipment.is_some() || self.h.trial_gear.is_some() { crate::combat::after_armor(amount,self.armor(),equipment::bonus(self,crate::attributes::ARMOR_TOUGHNESS)) }
            else if horse::equine(self.kind) { crate::combat::after_armor(amount, self.armor(), horse::toughness(self)) } else { after_armor(amount, self.armor()) }
        } else { amount };
        let points=equipment::all(self)[2..].iter().flatten().map(|s|crate::combat::protection_points(&s.extras(),source)).sum();
        let amount=crate::combat::after_protection(amount,points);
        let armor_absorbs = wolf_armor::protects(self, source);
        let armored_wolf = self.kind == Kind::Wolf && self.tame.armor.is_some();
        let absorption_before = self.vitals.absorption;
        let mut hit = self.vitals.hurt(amount, source, false)?;
        if armor_absorbs {
            wolf_armor::damage(self, hit.taken);
            self.vitals.health = before;
            self.vitals.absorption = absorption_before;
            self.vitals.dead = false;
            hit.taken = 0.0; hit.absorbed = 0.0; hit.died = false;
        }

        // Снято на самом деле — не больше оставшегося здоровья.
        hit.taken = hit.taken.min(before + hit.absorbed);
        self.water.last_attacker = push.map(|p| p.cause).filter(|id| *id != 0);

        if self.kind == Kind::Phantom && !self.no_ai { phantom::retreat(self); }

        if self.kind == Kind::Wither {
            wither::on_hurt(self);
        }

        if water::creature(self.kind) { water::on_hurt(self, source, push, hit.taken, hit.died); }

        if self.kind == Kind::EnderDragon {
            let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0xD4A);

            dragon::on_hurt(self, hit.taken, push.map(|push| push.cause).filter(|cause| *cause != 0), &mut rng);
        }

        if self.kind == Kind::Silverfish && !hit.died && self.h.silverfish_call == 0
            && (arthropod::calls_for_help(source) || push.is_some_and(|push| push.cause != 0))
        {
            self.h.silverfish_call = arthropod::CALL_DELAY;
        }

        combat_enchantments::on_hurt(self, source, push);
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

            if self.kind == Kind::Warden && !self.no_ai { warden::on_hurt(self, push.cause); }

            // Волк мстит и встаёт, кошка вскакивает (mob/tame.rs).
            tame::on_hurt(self, push.cause);
            land::on_hurt(self, push.cause);
            wildlife::on_hurt(self, push.cause);

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
                Kind::Enderman if !(self.catalog.changed_rules() && self.riding) => self.h.want_teleport = true,
                Kind::Shulker if self.vitals.health < self.kind.max_health() / 2.0 => {
                    self.h.want_teleport = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0x5B).below(4) == 0;
                }
                _ => {}
            }
        }

        if hit.died && wildlife::totem(self,source) {hit.died=false;}
        if hit.died {
            self.die();
            golem::crack_sound(self, before);
        } else {
            let (_, hurt, _) = self.sounds();
            let hurt = if self.kind == Kind::Slime { slime::sound(self.nether.size, "hurt") } else { hurt };
            let hurt = if self.kind == Kind::Armadillo && self.small.roll != 0 { "minecraft:entity.armadillo.hurt_reduced" } else { hurt };
            let hurt = if self.kind == Kind::Shulker && self.h.peek == 0 { "minecraft:entity.shulker.hurt_closed" } else { hurt };

            // Волк в броне стонет тише (0,4; gaps2-wolf-armor), в том числе когда броня лопнула.
            if !armor_absorbs || self.tame.armor.is_none() {
                let volume = if armored_wolf { 0.4 } else { self.sound_volume() };
                self.events.push(Event::Sound { name: hurt, volume, pitch: self.voice_pitch() });
            }
            golem::crack_sound(self, before);

            // Вики, 24w21a: панику вызывает вид урона, а не наличие
            // толчка/нападающего; падение и generic в список не входят.
            if ai::panic::on_hurt(self, source) {
                if let Some(push) = push {
                    self.fright = Some((self.x - push.dir.0 * 8.0, self.z - push.dir.1 * 8.0));
                }
                self.brain.panic(PANIC_TICKS);
            }
        }

        Some(hit)
    }

    /// Броня: у шалкера закрытого — 20, открытого — 0 (вики, Shulker). Броню
    /// даёт закрытие после открытия (`covered`): призванный закрытым без брони
    /// (чёрный ящик, shulker-stand: NoAI-шалкер теряет от меча 7 из 30).
    pub fn armor(&self) -> f32 {
        let extra=equipment::bonus(self,crate::attributes::ARMOR);
        extra + match self.kind {
            Kind::Rabbit if self.wild.variant == 99 => 8.0,
            Kind::Shulker if self.h.covered => 20.0,
            Kind::MagmaCube => nether::cube_armor(self.nether.size),
            Kind::Horse => self.horse.slots[1].and_then(|s| blocks::item_name(s.item)).map_or(0.0, horse::armor),
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
        if matches!(self.kind, Kind::Shulker | Kind::IronGolem | Kind::Warden | Kind::ArmorStand | Kind::Cushion) || (self.kind == Kind::Creaking && self.newmobs.frozen) {
            return;
        }

        // Хоглин держит отдачу: чёрный ящик, hoglin-stand — скорость после
        // удара 0,16 вместо 0,4, то есть сопротивление 0,6.
        let strength = if matches!(self.kind, Kind::Hoglin | Kind::Zoglin) { strength * 0.4 } else { strength };

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
            Kind::Cow | Kind::Mooshroom | Kind::Bee => 0.4,
            Kind::Warden => 4.0,
            // Вики, Bat, Sounds: «0.1».
            Kind::Bat => 0.1,
            kind if horse::equine(kind) => 0.8,
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

        if self.kind == Kind::Bat { (1.0 + spread) * 0.95 }
        else if self.is_baby() { 1.5 + spread } else { 1.0 + spread }
    }

    /// Вики, Sculk Catalyst: опыт без требования последнего удара игрока.
    pub fn sculk_xp(&self, rng: &mut Rng) -> i32 {
        if self.is_baby() || self.kind.object() || matches!(self.kind, Kind::Villager | Kind::IronGolem | Kind::SnowGolem) { return 0; }
        if self.kind == Kind::SulfurCube { return sulfur_cube::experience(self, rng); }
        if slime::cube(self.kind) { return self.nether.size; }
        if guardian::guardian(self.kind) { return guardian::XP; }
        if self.kind == Kind::EnderDragon { return 0; }
        if self.kind.hostile() {
            let worn = u32::from(self.h.bow) + u32::from(self.h.helmet);
            return 5 + (0..worn).map(|_| crate::survival::passive_mob_xp(rng.unit() as f32)).sum::<i32>();
        }
        crate::survival::passive_mob_xp(rng.unit() as f32)
    }

    /// Смерть: клиентам — событие, миру — добыча.
    pub fn die(&mut self) {
        if self.kind == Kind::Wolf && let Some(armor) = wolf_armor::stack(self) {
            self.events.push(Event::Loot(vec![armor])); self.tame.armor = None;
        }

        if self.dead_for > 0 {
            return;
        }

        // Подушка: только предмет, без звука смерти и опыта [проверить звук и частицы].
        if self.kind == Kind::Cushion {
            self.dead_for = 1;
            let drops = loot(self, &mut Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0x5EED));
            if !drops.is_empty() { self.events.push(Event::Loot(drops)); }
            return;
        }

        // Стойка: добыча, звук и частицы поломки — без события смерти и опыта
        // (чёрный ящик armorstand-equip-break).
        if self.kind == Kind::ArmorStand {
            self.dead_for = 1;
            let drops = loot(self, &mut Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0x5EED));
            if !drops.is_empty() { self.events.push(Event::Loot(drops)); }
            let (width, height) = self.size();
            self.events.push(Event::Sound { name: "minecraft:entity.armor_stand.break", volume: 1.0, pitch: 1.0 });
            // Частицы блока «доски» над серединой тела; номер состояния — [проверить].
            if let Some(block) = blocks::state_from_text("minecraft:oak_planks") {
                self.events.push(Event::BlockBurst { block, at: (self.x, self.y + height * 2.0 / 3.0, self.z), offset: ((width / 4.0) as f32, (height / 4.0) as f32, (width / 4.0) as f32), speed: 0.05, count: 10 });
            }
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
        } else if self.xp_until > self.ticks && (!self.is_baby() || self.kind == Kind::Zoglin) && self.kind != Kind::WanderingTrader {
            let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0xE4B);

            self.pending_xp = if matches!(self.kind,Kind::Axolotl | Kind::Tadpole | Kind::Bat | Kind::Allay) {
                0
            } else if self.kind == Kind::SulfurCube {
                sulfur_cube::experience(self, &mut rng)
            } else if slime::cube(self.kind) {
                // Вики, Magma Cube: опыт равен размеру (чёрный ящик, magma-cube-stand).
                self.nether.size
            } else if guardian::guardian(self.kind) {
                guardian::XP
            } else if self.kind == Kind::Wither {
                wither::XP
            } else if self.kind == Kind::Endermite {
                3
            } else if matches!(self.kind, Kind::CopperGolem | Kind::Creaking) {
                0
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
        self.dead_for > if self.kind == Kind::EnderDragon { DRAGON_DEATH_TICKS } else if matches!(self.kind, Kind::ArmorStand | Kind::Cushion) {0} else { DEATH_TICKS }
    }

    /// Проверяет длины и полную представимость предметного дополнения до записи.
    pub fn validate_item_storage(&self) -> Result<(), &'static str> {
        item_storage::validate(self)
    }

    /// Запись собственного формата; вызывающий проверяет данные до очереди снимков.
    pub fn write(&self, out: &mut Vec<u8>) {
        self.validate_item_storage().expect("предметы моба проверены до записи");
        let id = if self.kind == Kind::Skeleton && (self.h.water_ticks != 0 || self.h.converting != 0) {
            save::SNOW_SKELETON
        } else {
            self.kind.save_id()
        };
        save::write_head(out, id, self.name.is_some() || self.leash.is_some() || self.persistent_tag || self.h.trial_gear.is_some() || self.equipment.is_some() || matches!(self.kind, Kind::Hoglin | Kind::Piglin | Kind::PiglinBrute) || self.age_locked || leash::village_uuid(self).is_some() || item_storage::needed(self));
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
            Kind::SulfurCube => {
                out.extend_from_slice(&self.nether.size.to_be_bytes());
                out.push(u8::from(self.silent) | (u8::from(self.sulfur.from_bucket) << 1));
                out.extend_from_slice(&self.sulfur.body.unwrap_or(-1).to_be_bytes());
                out.extend_from_slice(&self.sulfur.fuse.to_be_bytes());
                out.extend_from_slice(&self.sulfur.pickup_timer.to_be_bytes());
            }
            Kind::Enderman => out.extend_from_slice(&self.h.carried.unwrap_or(-1).to_be_bytes()),
            Kind::Shulker => out.extend_from_slice(&[self.h.face, self.h.shell_color]),
            Kind::Villager | Kind::ZombieVillager => crate::villager::write(&self.villager, out),
            Kind::Wolf | Kind::Cat => tame::write(&self.tame, out),
            Kind::Rabbit | Kind::Fox | Kind::Bee => {
                let mut state=self.wild.clone(); state.silent=self.silent;
                wildlife::write(&state, out);
            }
            Kind::Armadillo | Kind::Sniffer | Kind::Parrot | Kind::Bat | Kind::Allay => small::write(self, out),
            Kind::IronGolem | Kind::SnowGolem => golem::write(&self.golem, out),
            kind if horse::equine(kind) => horse::write(self, out),
            _ => {}
        }

        if self.kind == Kind::Skeleton && (self.h.water_ticks != 0 || self.h.converting != 0) {
            out.extend_from_slice(&self.h.water_ticks.to_be_bytes());
            out.extend_from_slice(&self.h.converting.to_be_bytes());
        }

        if land::animal(self.kind) || self.kind == Kind::WanderingTrader {
            land::write(self, out);
        }
        if water::creature(self.kind) { water::write(&self.water, out); }
        if newmobs::new_kind(self.kind) { newmobs::write(&self.newmobs, out); }
        if self.kind==Kind::ArmorStand {self.stand.write(out);}

        // Общий хвост (вид с битом `EXTENDED`): имя с бирки и поводок (mob/leash.rs).
        if self.name.is_some() || self.leash.is_some() || self.persistent_tag || self.h.trial_gear.is_some() || self.equipment.is_some() || matches!(self.kind, Kind::Hoglin | Kind::Piglin | Kind::PiglinBrute) || self.age_locked || leash::village_uuid(self).is_some() || item_storage::needed(self) {
            leash::write_tail(self, out);
        }
    }

    /// Читает запись `write`; None — она испорчена.
    pub fn read(bytes: &mut &[u8]) -> Option<Mob> {
        let (id, extended) = save::read_head(bytes)?;
        let kind = if id == save::SNOW_SKELETON {
            Kind::Skeleton
        } else {
            Kind::from_save_id(id)?
        };
        let mut take = |count: usize| -> Option<&[u8]> {
            let (head, rest) = bytes.split_at_checked(count)?;
            *bytes = rest;
            Some(head)
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
        let carried = if kind == Kind::Enderman {
            match i32::from_be_bytes(take(4)?.try_into().ok()?) {
                -1 => None, state if state >= 0 => Some(state), _ => return None,
            }
        } else { None };
        let shell = if kind == Kind::Shulker { take(2)?.to_vec() } else { vec![0, 16] };
        let cube_data = if kind == Kind::SulfurCube {
            Some((i32::from_be_bytes(take(4)?.try_into().ok()?), take(1)?[0], i32::from_be_bytes(take(4)?.try_into().ok()?),
                i32::from_be_bytes(take(4)?.try_into().ok()?), i32::from_be_bytes(take(4)?.try_into().ok()?)))
        } else { None };
        let slime_data = if kind == Kind::Slime { Some((i32::from_be_bytes(take(4)?.try_into().ok()?), take(1)?[0])) } else { None };
        let villager = if matches!(kind, Kind::Villager | Kind::ZombieVillager) { Some(crate::villager::read(bytes)?) } else { None };
        let tamed = if matches!(kind, Kind::Wolf | Kind::Cat) { Some(tame::read(bytes)?) } else { None };
        let golem_state = if golem::golem(kind) { Some(golem::read(bytes)?) } else { None };
        let wild = if wildlife::kind(kind) { Some(wildlife::read(bytes)?) } else { None };
        let snow_state = if id == save::SNOW_SKELETON { Some(skeleton::read_snow(bytes)?) } else { None };
        let land_state = if land::animal(kind) || kind == Kind::WanderingTrader { Some(land::read(bytes)?) } else { None };
        let water_state = if water::creature(kind) { Some(water::read(bytes)?) } else { None };
        let mut horse_mob = Mob::new(0, kind, (x, y, z));
        if horse::equine(kind) { horse::read(&mut horse_mob, bytes)?; }
        let small_state = if small::kind(kind) { Some(small::read(bytes)?) } else { None };
        let stand_state = if kind==Kind::ArmorStand {Some(armor_stand::State::read(bytes)?)} else {None};
        let new_state = if newmobs::new_kind(kind) { Some(newmobs::read(bytes)?) } else { None };
        let tail = if extended { Some(leash::read_tail(bytes)?) } else { None };

        if !(x.is_finite() && y.is_finite() && z.is_finite() && health.is_finite()) || color > 15 {
            return None;
        }

        let mut mob = horse_mob;

        if let Some((state, tame)) = small_state {
            if let Some(uuid) = state.saved_uuid { mob.uuid = uuid; }
            mob.silent = state.silent;
            mob.small = state; mob.tame = tame;
        }
        if let Some(state) = new_state { mob.newmobs=state; }
        if let Some(state) = stand_state {mob.stand=state;}
        mob.yaw = yaw;
        mob.head_yaw = yaw;
        if let Some((size, flags)) = slime_data {
            slime::set_size(&mut mob, size);
            mob.silent = flags & 1 != 0;
            mob.nether.persistence_required = flags & 2 != 0;
        }
        if let Some((size, flags, body, fuse, pause)) = cube_data {
            sulfur_cube::set_size(&mut mob, size);
            mob.silent = flags & 1 != 0;
            mob.sulfur = sulfur_cube::State { body: (body >= 0).then_some(body), fuse, pickup_timer: pause, from_bucket: flags & 2 != 0, ..sulfur_cube::State::default() };
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

        if let Some((state, trader)) = land_state {
            mob.land = state;
            if let Some(uuid)=mob.land.saved_uuid { mob.uuid=uuid; }
            mob.villager = trader;
            land::health(&mut mob);
            mob.vitals.health = health.clamp(0.5, mob.vitals.max_health);
        }
        if let Some(state) = water_state { mob.water = state; }
        if let Some(wild) = wild {
            mob.silent = wild.silent;
            mob.wild = wild;
        }

        if let Some(tail) = tail {
            leash::apply_tail_checked(&mut mob, tail)?;
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
            && (!wildlife::kind(self.kind) || self.wild == other.wild)
    }
}

/// Выпадающее после смерти (вики: Cow, Pig, Sheep, Chicken, «Drops»):
/// корова — 0–2 кожи и 1–3 говядины, свинья — 1–3 свинины, овца — шерсть
/// своего цвета (если не острижена) и 1–2 баранины, курица — 0–2 пера и
/// курятина. Детёныши ничего не роняют. Горящие роняют жареное.
pub fn loot(mob: &Mob, rng: &mut Rng) -> Vec<Stack> {
    if mob.kind.object() {
        return Vec::new();
    }
    if mob.is_baby() && !mob.kind.hostile() && mob.kind != Kind::Fox { return equipment::death_drops(mob, rng); }

    // Вики, Drops: добыча от убийства игроком учитывает и его снаряды.
    // credit_player ставится до смертельного попадания; то же условие уже
    // используется skeleton::loot. by_player отдельно нужен прямому удару.
    let credited = mob.by_player || mob.xp_until > mob.ticks;
    let mut drops = Vec::new();
    let mut give = |name: &str, low: i32, high: i32, rng: &mut Rng| {
        let count = low + rng.below((high - low + 1) as u32) as i32;
        // Вики, Item modifier/enchanted_count_increase: дробный бонус
        // «rounded after multiplying by the looting level». Как у скелета,
        // бросок прибавки отдельный: распределение не равномерно 0..max+L.
        // Шерсть овцы, блок эндермена и железо голема «добыча» не умножает.
        // [проверить] Распределение и последовательность RNG на vanilla 26.1.2.
        let common = matches!((mob.kind, name),
            (Kind::Cow | Kind::Mooshroom, "leather" | "beef")
            | (Kind::Pig, "porkchop") | (Kind::Sheep, "mutton")
            | (Kind::Chicken, "feather" | "chicken")
            | (Kind::Zombie | Kind::Husk | Kind::ZombieVillager, "rotten_flesh")
            | (Kind::Creeper, "gunpowder") | (Kind::Spider, "string")
            | (Kind::Cat, "string") | (Kind::Enderman, "ender_pearl")
            | (Kind::Llama | Kind::TraderLlama | Kind::Horse | Kind::Donkey | Kind::Mule, "leather")
            | (Kind::Rabbit, "rabbit_hide" | "rabbit" | "cooked_rabbit")
            | (Kind::PolarBear, "cod" | "salmon")
            | (Kind::SkeletonHorse | Kind::WitherSkeleton, "bone")
            | (Kind::ZombieHorse | Kind::Drowned | Kind::Zoglin, "rotten_flesh")
            | (Kind::CaveSpider | Kind::Strider, "string")
            | (Kind::Blaze, "blaze_rod") | (Kind::Phantom, "phantom_membrane")
            | (Kind::Ghast, "ghast_tear" | "gunpowder")
            | (Kind::ZombifiedPiglin, "rotten_flesh" | "gold_nugget")
            | (Kind::Hoglin, "porkchop" | "leather") | (Kind::Parrot, "feather")
            | (Kind::Squid, "ink_sac") | (Kind::GlowSquid, "glow_ink_sac")
            | (Kind::Dolphin, "cod" | "cooked_cod") | (Kind::Turtle, "seagrass"));
        let count = if common && mob.looting > 0 {
            count + (rng.unit() * f64::from(mob.looting)).round() as i32
        } else { count };

        // Зловещие флаг и бутылка — с довесками (mob/illager.rs, captain_loot).
        let (name, data) = match name.split_once(':') {
            Some(("ominous_bottle", level)) => ("ominous_bottle", crate::stackdata::Data { ominous: level.parse().ok(), ..Default::default() }),
            _ if name == "ominous_banner" => ("white_banner", crate::stackdata::Data { ominous_banner: true, ..Default::default() }),
            _ => (name, crate::stackdata::Data::default()),
        };

        // Вики, Fire Aspect: горящая добыча готовится и при смертельном ударе.
        let name = if mob.vitals.fire > 0 { match name {
            "beef" => "cooked_beef", "porkchop" => "cooked_porkchop", "mutton" => "cooked_mutton",
            "chicken" => "cooked_chicken", "rabbit" => "cooked_rabbit", "cod" => "cooked_cod", "salmon" => "cooked_salmon", other => other,
        } } else { name };
        if let (Some(item), true) = (blocks::item_named(name), count > 0) {
            let mut stack = Stack::new(item, count);

            stack.data = crate::stackdata::intern(data);
            drops.push(stack);
        }
    };

    match mob.kind {
        Kind::Endermite | Kind::Creaking => {},
        Kind::HappyGhast => { if let Some(c) = mob.newmobs.harness { give(&format!("{}_harness", newmobs::COLOURS[c as usize]), 1, 1, rng); } },
        Kind::Zoglin => give("rotten_flesh", 1, 3, rng),
        Kind::CopperGolem => give("copper_ingot", 1, 3 + mob.looting, rng),
        Kind::ArmorStand => {if !mob.stand.creative_break {give("armor_stand",1,1,rng);}},
        Kind::Cushion => give(&cushion::item_name(mob.color), 1, 1, rng),
        Kind::Rabbit => {
            give("rabbit_hide", 0, 1, rng);
            give("rabbit", 0, 1, rng);
            if credited && rng.unit() < 0.1 + 0.03 * f64::from(mob.looting.max(0)) {
                give("rabbit_foot", 1, 1, rng);
            }
        }
        Kind::Fox => {
            if mob.equipment.is_none() && let Some(held) = mob.wild.mouth { drops.push(held); }
        }
        Kind::Bee => {}
        Kind::Cow | Kind::Mooshroom => {
            give("leather", 0, 2, rng);
            give("beef", 1, 3, rng);
        }
        Kind::Llama | Kind::TraderLlama => give("leather", 0, 2, rng),
        Kind::PolarBear => give(if rng.below(4) < 3 { "cod" } else { "salmon" }, 0, 2, rng),
        Kind::Panda => give("bamboo", 1, 1, rng),
        Kind::WanderingTrader | Kind::Camel | Kind::Goat | Kind::Ocelot => {},
        Kind::Pig => give("porkchop", 1, 3, rng),
        Kind::Horse | Kind::Donkey | Kind::Mule => give("leather", 0, 2, rng),
        Kind::SkeletonHorse => give("bone", 0, 2, rng),
        Kind::ZombieHorse => give("rotten_flesh", 0, 2, rng),
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
            give("rotten_flesh", 0, 2, rng);

            if credited && rng.below(100) < 11 + 2 * mob.looting as u32 {
                give("copper_ingot", 1, 1, rng);
            }

            if mob.equipment.is_none() && let Some(item) = hostile::hand_item(mob.h.hand).and_then(blocks::item_named)
                && rng.unit() < f64::from(mob.h.hand_drop)
            {
                drops.push(Stack::new(item, 1));
            }

            if mob.equipment.is_none() && mob.h.offhand_shell && let Some(item) = blocks::item_named("nautilus_shell") {
                drops.push(Stack::new(item, 1));
            }
        }
        // Вики, Zombie Villager: гнилая плоть и редко железо, морковь, картофель
        // [проверить: шансы редкого].
        Kind::ZombieVillager => give("rotten_flesh", 0, 2, rng),
        Kind::Skeleton | Kind::Stray | Kind::Bogged => drops.extend(skeleton::loot(mob, rng)),
        Kind::Creeper => give("gunpowder", 0, 2, rng),
        Kind::CaveSpider => {
            give("string", 0, 2, rng);

            if credited {
                // Вики, Cave Spider: «-1–1», с Looting верхняя граница растёт.
                give("spider_eye", -1, 1 + mob.looting, rng);
            }
        }
        Kind::Silverfish => {}
        Kind::Spider => {
            give("string", 0, 2, rng);

            if credited {
                give("spider_eye", 0, 1, rng);
            }
        }
        // Вики, Blaze: 0–1 стержня ифрита (только от игрока; «Looting» +1 за уровень).
        Kind::Blaze => {
            if credited {
                give("blaze_rod", 0, 1, rng);
            }
        }
        // Вики, Wither Skeleton: 0–2 костей, 0–1 угля (+ «Looting»); череп 2,5 %
        // (+1 % за уровень) только от игрока.
        Kind::WitherSkeleton => {
            give("bone", 0, 2, rng);
            give("coal", 0, 1 + mob.looting, rng);

            if credited && rng.below(1000) < 25 + 10 * mob.looting as u32 {
                give("wither_skeleton_skull", 1, 1, rng);
            }
        }
        // Куб серы: поглощённый блок (малый — ничего); опыт считает `die`.
        Kind::SulfurCube => {
            for stack in sulfur_cube::loot(mob) { drops.push(stack); }
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
            give("ghast_tear", 0, 1, rng);
            give("gunpowder", 0, 2, rng);
        }
        // Вики, Zombified Piglin: 0–1 гнилой плоти, 0–1 золотого самородка,
        // золотой слиток 2,5 % (только от игрока).
        Kind::ZombifiedPiglin => {
            give("rotten_flesh", 0, 1, rng);
            give("gold_nugget", 0, 1, rng);

            if credited && rng.below(1000) < 25 + 10 * mob.looting as u32 {
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
            give("porkchop", 2, 4, rng);
            give("leather", 0, 1, rng);
        }
        // Вики, Strider: 2–5 нитей.
        Kind::Strider => give("string", 2, 5, rng),
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
            if credited {
                give("phantom_membrane", 0, 1, rng);
            }
        }
        // Вики, Breeze: 1–2 стержня бриза (+1 за уровень «Looting» к верху и
        // низу: 2–4, 3–6, 4–8), только если убил игрок или прирученный волк.
        Kind::Breeze => {
            if credited {
                give("breeze_rod", 1 + mob.looting, 2 + 2 * mob.looting, rng);
            }
        }
        Kind::Guardian | Kind::ElderGuardian => guardian::loot(mob, rng, &mut give),
        // Вики, Wither: звезда Незера всегда (и убитому командой).
        Kind::Wither => give("nether_star", 1, 1, rng),
        Kind::Cod | Kind::Salmon | Kind::TropicalFish | Kind::Pufferfish | Kind::Squid | Kind::GlowSquid | Kind::Dolphin | Kind::Turtle | Kind::Axolotl | Kind::Frog | Kind::Tadpole => water::loot(mob, rng, &mut give),
        Kind::Warden => give("sculk_catalyst", 1, 1, rng),
        Kind::Armadillo | Kind::Sniffer | Kind::Bat => {}
        Kind::Parrot => give("feather", 1, 2, rng),
        Kind::Allay => { if mob.equipment.is_none() { drops.extend(mob.small.held); } drops.extend(mob.small.cargo); }
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

    if land::llama(mob.kind) {
        drops.extend(mob.land.cargo.iter().flatten().copied());
        if mob.land.chest && let Some(id)=blocks::item_named("chest") { drops.push(Stack::new(id,1)); }
        if let Some(c)=mob.land.decor && let Some(id)=blocks::item_named(&WOOL[usize::from(c)].replace("_wool","_carpet")) { drops.push(Stack::new(id,1)); }
    }
    if mob.kind==Kind::Camel && mob.land.saddle && let Some(id)=blocks::item_named("saddle") { drops.push(Stack::new(id,1)); }
    if horse::equine(mob.kind) {
        // Снаряжение, заданное данными призыва, выпадает с шансом 8,5 % на место;
        // надетое игроком — всегда (чёрный ящик, probe-ride: седло из `equipment`
        // у убитой лошади не выпало) [проверить: шанс].
        for (index, slot) in mob.horse.slots.iter().enumerate() {
            if let Some(stack) = slot && (index >= 2 || !mob.horse.gear_by_data || rng.below(1000) < 85) { drops.push(*stack); }
        }
        if mob.horse.chest && let Some(item) = blocks::item_named("chest") { drops.push(Stack::new(item, 1)); }
    }

    if mob.equipment.is_none() && let Some(stack) = mob.newmobs.cargo { drops.push(stack); }
    if mob.kind==Kind::ArmorStand && !mob.stand.creative_break {
        // Vanilla enchant-mob-equipment-drops: при двух ударах стойка
        // выпадает, а boots с Vanishing исчезают; обычные boots остаются.
        drops.extend(mob.stand.equipment.into_iter().flatten().filter(|stack| stack.extras().level("vanishing_curse") == 0));
    }
    drops.extend(equipment::death_drops(mob, rng));
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

// Форма первого нового блока отделена от прежней физики775.
// [проверить] Геометрия половин по свойству type; обычная физика26.3 ещё
// не сравнена с официальным сервером. Старые плиты пока сохраняют прежний долг.
pub(crate) fn catalog_clip(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, delta: f64, axis: usize) -> Option<f64> {
    use crate::gameplay_catalog::{Profile, SlabHalf};
    if view.catalog().profile() == Profile::Java26_1_2 { return None; }
    let low = [min.0, min.1, min.2];
    let high = [min.0 + 2.0 * half, min.1 + height, min.2 + 2.0 * half];
    let mut from = low;
    let mut to = high;
    from[axis] += delta.min(0.0);
    to[axis] += delta.max(0.0);
    let mut allowed = delta;
    for x in (from[0] - EPS).floor() as i32..=(to[0] + EPS).floor() as i32 {
        for y in (from[1] - EPS).floor() as i32..=(to[1] + EPS).floor() as i32 {
            for z in (from[2] - EPS).floor() as i32..=(to[2] + EPS).floor() as i32 {
                let state = view.block(x, y, z);
                let origin = [x as f64, y as f64, z as f64];
                let mut a = origin;
                let mut b = [a[0] + 1.0, a[1] + 1.0, a[2] + 1.0];
                // Ящики клетки: ступени — несколько ящиков, прочее — один.
                let mut cell: Vec<([f64; 3], [f64; 3])> = Vec::new();
                if let Ok(Some((half, _))) = view.catalog().slab_properties(state) {
                    match half { SlabHalf::Top => a[1] += 0.5, SlabHalf::Bottom => b[1] -= 0.5, SlabHalf::Double => {} }
                    cell.push((a, b));
                } else if let Ok(Some(stair)) = view.catalog().stair_properties(state) {
                    cell.extend(stair.boxes().into_iter().map(|(from, to)| ([origin[0] + from[0], origin[1] + from[1], origin[2] + from[2]], [origin[0] + to[0], origin[1] + to[1], origin[2] + to[2]])));
                } else if !solid(state) { continue; } else { cell.push((a, b)); }
                for (a, b) in cell {
                    if (0..3).any(|other| other != axis && (high[other] <= a[other] + EPS || low[other] >= b[other] - EPS)) { continue; }
                    if delta > 0.0 && high[axis] <= a[axis] + EPS {
                        allowed = allowed.min((a[axis] - high[axis]).max(0.0));
                    } else if delta < 0.0 && low[axis] >= b[axis] - EPS {
                        allowed = allowed.max((b[axis] - low[axis]).min(0.0));
                    }
                }
            }
        }
    }
    Some(allowed)
}

/// Свободный ход по оси вверх/вниз: насколько можно сдвинуться, не входя в блоки.
pub(crate) fn clip_y(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dy: f64) -> f64 {
    if let Some(allowed) = catalog_clip(view, min, half, height, dy, 1) { return allowed; }
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
    if let Some(allowed) = catalog_clip(view, min, half, height, dx, 0) { return allowed; }
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
    if let Some(allowed) = catalog_clip(view, min, half, height, dz, 2) { return allowed; }
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

/// Общий тег bypasses_armor для игроков и мобов (вики, Damage type tag).
fn armored(source: Source) -> bool {
    !crate::combat::bypasses_armor(source)
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
    pub progress: Vec<([u8; 16], crate::progress::Event)>,
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
    pub spits: Vec<(i32, golem::Throw)>,
    /// Заряды ветра бризов (mob/breeze.rs).
    pub wind_charges: Vec<(i32, golem::Throw)>,

    /// Клетки, куда снежные големы кладут снег (если разрешено mobGriefing).
    pub snow: Vec<(i32, i32, i32)>,

    /// Проклятия древних стражей: игрок и тихий ли страж (mob/guardian.rs).
    pub curses: Vec<(i32, bool)>,

    /// Заказы иссушителей: кто и что (mob/wither.rs; tick.rs, wither_deeds).
    pub withers: Vec<(i32, wither::Deed)>,
    pub water_effects: Vec<(i32,crate::survival::Effect)>,
    pub stings: Vec<water::Sting>,
    pub axolotl_assists: Vec<i32>,
    pub darkness: Vec<i32>,
    pub sonic_booms: Vec<warden::Boom>,
    pub enchant_retaliation: Vec<combat_enchantments::Retaliation>,
}

/// Удар моба в ближнем бою.
#[derive(Debug)]
pub struct Strike {
    pub attacker: i32,
    pub at: (f64, f64, f64),
    pub kind: Kind,
    pub melee: hostile::Melee,
    pub enchantments: crate::world::ProjectileEnchantments,
}

/// Выстрел скелета.
#[derive(Debug)]
pub struct Fired {
    pub kind: Kind,
    pub shooter: i32,
    pub shot: hostile::Shot,
    /// Оружие до tick: последний выстрел сохраняет чары сломанного арбалета.
    pub weapon: Option<Stack>,
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

    /// Возраст мира в тактах: память жителей «спал» хранит его, а не часы суток.
    pub tick: i64,
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
            ready: mob.age == 0 && (!horse::equine(mob.kind) || horse::can_parent(mob)),
            tamed: mob.tame.tamed(),
        })
        .collect();

    let ctx = ai::Ctx { players, mates: &mates, difficulty: env.difficulty, sky: env.sky, sun_up: env.sun_up };
    let guardian_targets = water::predator_targets(items.mobs(),players,true);
    let drowned_targets = water::predator_targets(items.mobs(),players,false);
    let guardian_ctx = ai::Ctx { players: &guardian_targets, ..ctx };
    let drowned_ctx = ai::Ctx { players: &drowned_targets, ..ctx };
    let mut mobs = std::mem::take(items.mobs_vec());
    let mut breeds: Vec<(i32, i32)> = Vec::new();
    let mut born = village_golems::step(&mut mobs, view, env.tick, env.clock.rem_euclid(24_000), rng, next_id);

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
    land::herd(&mut mobs, players, view);

    for (index, mob) in mobs.iter_mut().enumerate() {
        mob.tame.clock = env.clock;

        if mob.kind == Kind::Evoker {
            mob.illager.sheep = blue_sheep.iter().filter(|(_, at)| hostile::dist2(mob, at.0, at.1, at.2) <= illager::WOLOLO_RANGE * illager::WOLOLO_RANGE).map(|(id, _)| *id).next();
        }

        let sense = if guardian::guardian(mob.kind) { &guardian_ctx } else if mob.kind==Kind::Drowned { &drowned_ctx } else { &ctx };
        let shot_weapon = equipment::slot(mob, 0);
        let mut plan = mob.tick(view, sense, rng);
        // Вики, Strength/Weakness: +3L / −4L к melee; луч стража — магия.
        if !guardian::guardian(mob.kind) && let Some(melee) = &mut plan.melee {
            let target = mates.iter().find(|target| target.id == melee.target).map(|target| target.kind);
            melee.damage = mob.effect_melee_damage(melee.damage) + combat_enchantments::bonus(mob, target);
        }
        if let Some((target, damage, _)) = &mut plan.bite {
            let kind = mates.iter().find(|mate| mate.id == *target).map(|mate| mate.kind);
            *damage = mob.effect_melee_damage(*damage) + if mob.kind == Kind::Fox { 0.0 } else { combat_enchantments::bonus(mob, kind) };
        }

        // Снежный голем: след и таяние — и без ИИ (snow-golem-shear).
        outcome.snow.extend(golem::snow_tick(mob, view, true));

        if let Some((target, damage, push)) = plan.bite {
            bites.push((mob.entity_id, target, damage, push, plan.fling.then_some((mob.x, mob.y, mob.z))));
        }

        if let Some(spit) = plan.spit {
            outcome.spits.push((mob.entity_id, spit));
        }

        if let Some(throw) = plan.wind_charge {
            outcome.wind_charges.push((mob.entity_id, throw));
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
                visible: true, show_icon: true,
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
                    outcome.strikes.push(Strike { attacker: mob.entity_id, at: (mob.x, mob.y, mob.z), kind: mob.kind, melee, enchantments: combat_enchantments::effects(mob) });
                }
            }
        }

        outcome.darkness.extend(std::mem::take(&mut mob.warden.darkness));
        if let Some((target, to)) = mob.warden.boom.take() {
            outcome.sonic_booms.push(warden::Boom { attacker: mob.entity_id, target, from: (mob.x, mob.y + 1.6, mob.z), to });
        }
        for player in std::mem::take(&mut mob.guardian.cursed) {
            if players.iter().any(|p| p.id==player) { outcome.curses.push((player, mob.silent)); }
        }

        for deed in std::mem::take(&mut mob.wither.deeds) {
            outcome.withers.push((mob.entity_id, deed));
        }

        if let Some(melee) = plan.melee {
            // Укус фантома: событие мира 1039 в его клетке (чёрный ящик, phantom-swoop).
            if mob.kind == Kind::Phantom {
                outcome.level_events.push((mob.cell(), 1039));
            }

            outcome.strikes.push(Strike { attacker: mob.entity_id, at: (mob.x, mob.y, mob.z), kind: mob.kind, melee, enchantments: combat_enchantments::effects(mob) });
        }

        if let Some(shot) = plan.shot {
            outcome.shots.push(Fired { kind: mob.kind, shooter: mob.entity_id, shot, weapon: shot_weapon });
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

        if water::creature(mob.kind) && !matches!(mob.kind,Kind::Turtle | Kind::Frog | Kind::Tadpole) && !mob.persistent_tag && !mob.water.from_bucket && !mob.is_dead() {
            hostile::despawn_check(mob,players,rng,false);
        }
        if mob.kind == Kind::Bat && !mob.is_dead() && !mob.persistent_tag {
            hostile::despawn_check(mob, players, rng, false);
        }
        if mob.kind.hostile() && !matches!(mob.kind, Kind::Shulker | Kind::Warden) && ((!mob.is_dead() && !mob.persistent_tag) || (peaceful && mob.is_dead())) {
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
        if mob.kind==Kind::Bee && let Some((at,state))=plan.put {outcome.puts.extend(wildlife::crop_upper(at,state));}
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
        let owner = mobs.iter().find(|m| m.entity_id == attacker && m.kind == Kind::Wolf).and_then(|m| m.tame.owner);
        let goat = mobs.iter().any(|mob| mob.entity_id == attacker && mob.kind == Kind::Goat);
        let warden = mobs.iter().any(|mob| mob.entity_id == attacker && mob.kind == Kind::Warden);
        let bee_attack = mobs.iter().any(|m| m.entity_id == attacker && m.kind == Kind::Bee && !m.wild.stung);
        let weapon = mobs.iter().find(|m| m.entity_id == attacker).and_then(combat_enchantments::weapon);
        let mut stung = false;
        if let Some(victim) = mobs.iter_mut().find(|mob| mob.entity_id == target) {
            let source = if goat { Source::MobAttackNoAggro } else { Source::MobAttack };
            let push = Some(Push { dir, strength: BASE_KNOCKBACK, cause: attacker });
            let hit = if let Some(uuid) = owner {
                // Чары меча хозяина не переходят волку. Команда может выдать
                // оружие самому волку: его чары учитываются до смерти цели.
                let looting = weapon.map_or(0, |stack| stack.extras().level("looting"));
                let fire = weapon.map_or(0, |stack| stack.extras().level("fire_aspect").saturating_mul(80));
                victim.hurt_credited_burning(damage, source, push, uuid, looting, fire)
            } else {
                let before = victim.looting;
                victim.looting = weapon.map_or(0, |stack| stack.extras().level("looting"));
                let fire = weapon.map_or(0, |stack| 80 * stack.extras().level("fire_aspect"));
                let hit = victim.hurt_burning(damage, source, push, 1.0, fire);
                if hit.is_none() { victim.looting = before; }
                hit
            };
            if bee_attack && hit.is_some() {
                stung = true;
                if !victim.is_dead() && !crate::combat::mob_traits(victim.kind.name()).0 && !victim.kind.spider()
                    && let Some(effect) = wildlife::poison(env.difficulty) { victim.vitals.give_effect_shown(effect); }
            }

            if hit.is_some() && let Some(stack) = weapon {
                let data = stack.extras();
                let knockback = data.level("knockback");
                if knockback > 0 { victim.knockback(dir.0, dir.1, f64::from(knockback) * 0.5); }
                if let Some(effect) = crate::combat::bane_effect(&data, victim.kind.name(), rng.unit() as f32) { victim.vitals.give_effect_shown(effect); }
            }
            if hit.is_some() && warden { victim.knockback(dir.0, dir.1, 0.75); }
            if let Some(at) = golem_at {
                if hit.is_some() {
                    victim.vy += golem::FLING;
                }

                victim.events.push(Event::SoundAt { name: "minecraft:entity.iron_golem.attack", volume: 1.0, pitch: 1.0, at });
            }
        }
        if stung && let Some(bee)=mobs.iter_mut().find(|m|m.entity_id==attacker) {wildlife::sting(bee);}
    }

    water::predator_strikes(&mut mobs,&mut outcome);
    water::interactions(&mut mobs,view,players,&mut outcome,rng);

    // Волки хозяев: кого хозяин ударил и кто ударил хозяина; дикая стая;
    // поводки (mob/leash.rs).
    outcome.enchant_retaliation = combat_enchantments::reflect(&mut mobs);

    let owner_hits: Vec<(i32, i32)> = outcome.strikes.iter().map(|strike| (strike.attacker, strike.melee.target)).collect();

    tame::orders(&mut mobs, &owner_hits);
    tame::pack_anger(&mut mobs);
    wildlife::pack_anger(&mut mobs);
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
        let mut mob = convert(&mobs[index], kind, next_id());
        if kind == Kind::Frog {
            let (x,y,z) = mob.cell();
            water::initialize(&mut mob,view.biome(x,y,z),rng);
            mob.age = 0;
            mob.persistent_tag = true;
            mob.name = mobs[index].name.clone();
        }

        if matches!(kind, Kind::Zoglin | Kind::ZombifiedPiglin) && !mobs[index].silent {
            mob.events.push(Event::Sound { name: if kind == Kind::Zoglin { "minecraft:entity.hoglin.converted_to_zombified" } else { "minecraft:entity.piglin.converted_to_zombified" }, volume: 1.0, pitch: 1.0 });
        } else if !matches!(kind, Kind::Zoglin | Kind::ZombifiedPiglin) && !mobs[index].silent && kind != Kind::Frog {
            outcome.level_events.push((mobs[index].cell(), if kind == Kind::Stray { 1048 } else if kind == Kind::Drowned { 1040 } else { 1041 }));
        }

        mobs[index].h.despawn = true;
        born.push(mob);
    }

    for index in cured {
        if let Some(uuid) = mobs[index].villager.converter {
            outcome.progress.push((uuid, crate::progress::Event::Trigger("story/cure_zombie_villager")));
        }
        let villager = cure(&mut mobs[index], next_id());

        mobs[index].h.despawn = true;
        born.push(villager);
    }

    // Разведение: оба родителя в паузу, детёныш рождается у первого. Пара,
    // из которой один уже занят, пропускается [проверить].
    let mut used: Vec<i32> = Vec::new();
    let mut birth_xp: Vec<(f64, f64, f64)> = Vec::new();

    for (a, b) in breeds {
        if used.contains(&a) || used.contains(&b) {
            continue;
        }

        let (Some(ia), Some(ib)) = (mobs.iter().position(|m| m.entity_id == a), mobs.iter().position(|m| m.entity_id == b)) else {
            continue;
        };

        used.extend([a, b]);

        let mut baby = if horse::compatible(mobs[ia].kind, mobs[ib].kind) {
            horse::child(&mobs[ia], &mobs[ib], next_id(), rng)
        } else { Mob::new(next_id(), mobs[ia].kind, (mobs[ia].x, mobs[ia].y, mobs[ia].z)) };

        land::inherit(&mut baby, &mobs[ia], &mobs[ib], rng);
        baby.age = BABY_AGE;
        if baby.kind == Kind::Axolotl { baby.water.variant=water::child_variant(mobs[ia].water.variant,mobs[ib].water.variant,rng); }
        baby.yaw = mobs[ia].yaw;
        baby.head_yaw = mobs[ia].yaw;
        baby.color = if mobs[ia].kind == Kind::Sheep { if rng.below(2) == 0 { mobs[ia].color } else { mobs[ib].color } } else { WHITE };
        baby.egg_timer = 6000 + rng.below(6001) as i32;
        wildlife::inherit(&mut baby, &mobs[ia], &mobs[ib], view, rng);

        if let Some(uuid) = mobs[ia].love_player.or(mobs[ib].love_player) {
            outcome.progress.push((uuid, crate::progress::Event::Bred(baby.kind.name())));
        }
        for index in [ia, ib] {
            mobs[index].love_player = None;
            mobs[index].age = BREED_COOLDOWN;
            mobs[index].love = 0;
            mobs[index].brain.stop();
        }

        // Сердечки — событием сущности 18 у первого родителя (чёрный ящик, cow-breed).
        mobs[ia].events.push(Event::Hearts);
        if matches!(baby.kind,Kind::Turtle | Kind::Axolotl | Kind::Frog) {
            // Вики, Breeding: «1–7 experience»; выдача через общий склад.
            items.drop_experience(1+rng.below(7) as i32,(mobs[ia].x,mobs[ia].y,mobs[ia].z),next_id,rng);
        } else {
            // Вики, Breeding: «1–7 experience»; шарик — после сердечек (чёрный ящик, rabbit-breed).
            birth_xp.push((mobs[ia].x, mobs[ia].y, mobs[ia].z));
        }
        if matches!(baby.kind,Kind::Turtle | Kind::Frog) {
            mobs[ia].water.has_egg = true;
        } else if mobs[ia].kind == Kind::Sniffer {
            small::drop(&mut mobs[ia], "sniffer_egg");
        } else { born.push(baby); }
    }

    *items.mobs_vec() = mobs;
    wildlife::pickup(items,view.griefing());
    sulfur_cube::absorb_items(items,view.griefing());

    // Тела, пролежавшие своё, уходят облачком.
    for mob in items.mobs_mut() {
        if mob.gone() && !matches!(mob.kind, Kind::ArmorStand | Kind::Cushion) {
            mob.events.push(Event::Poof);
        }
    }

    // Эффекты «при смерти» (Oozing, Weaving, Wind Charged) срабатывают, когда
    // труп убирается, через 20 тактов (чёрный ящик, tails-effect-weaving и
    // -wind_charged: событие мира на +19 рядом с уходом коровы).
    let due: Vec<_> = items.mobs_mut().iter_mut().filter(|mob| mob.gone()).map(|mob| {
        let height = mob.size().1;
        crate::effect_reactions::take(&mut mob.vitals, true, (mob.x, mob.y, mob.z), height, mob.yaw, false)
    }).collect();
    items.death_reactions.extend(due);

    items.flush_mob_events(next_id, rng);
    items.remove_mobs(|mob| mob.gone() || mob.h.despawn);

    // Вики, Breeding: при рождении детёныша падает 1–7 очков опыта; шарик
    // появляется после сердечек и перед детёнышем (чёрный ящик, rabbit-breed).
    for at in birth_xp {
        let points = 1 + rng.below(7) as i32;
        // Один шарик на всё значение (не делится на 1/3/7, как за убийство).
        let velocity = ((rng.unit() * 0.2 - 0.1) * 2.0, rng.unit() * 0.2 * 2.0, (rng.unit() * 0.2 - 0.1) * 2.0);

        items.add_orb(next_id(), points, at, velocity);
    }

    for mut baby in born {
        baby.late = true;

        // Вылеченный житель: появление — без эффектов, на следующем такте в
        // слежении тошнота и сила, ещё через такт — одна тошнота (чёрный ящик,
        // zombie-villager-cured): эффекты ложатся уже после снимка.
        let cured = baby.kind == Kind::Villager && std::mem::take(&mut baby.villager.fresh_cure);

        items.add_mob_late(baby);

        if let Some(mob) = items.mobs_mut().last_mut().filter(|_| cured) {
            mob.vitals.give_effect_shown(crate::survival::Effect { kind: crate::survival::EffectKind::Nausea, amplifier: 0, ticks_left: CURED_NAUSEA, ambient: false, visible: true, show_icon: true });
            mob.vitals.give_effect_shown(crate::survival::Effect { kind: crate::survival::EffectKind::Strength, amplifier: 0, ticks_left: 1, ambient: false, visible: true, show_icon: true });
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
    new.leash = old.leash;
    new.h.bow = old.h.bow;
    new.h.helmet = old.h.helmet;
    new.h.trial_gear = old.h.trial_gear;
    new.equipment = old.equipment;
    new.equipment_drop_chances = old.equipment_drop_chances;
    new.h.hand = old.h.hand;
    new.h.offhand_shell = old.h.offhand_shell;
    new.h.hand_drop = old.h.hand_drop;
    if matches!(old.kind, Kind::Hoglin | Kind::Piglin | Kind::PiglinBrute) {
        // Вики, Hoglin / Piglin: "Nausea ... for 10 seconds".
        new.vitals.give_effect_shown(crate::survival::Effect {kind:crate::survival::EffectKind::Nausea,amplifier:0,ticks_left:200,ambient:false,visible:true, show_icon:true});
    }
    new
}

/// Сколько нельзя поднимать выпавшее из моба: полсекунды, как у выпавшего
/// из блока (вики, Item: «pickup delay 10 ticks»).
pub const DROP_DELAY: u64 = 10;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod changed_rules_tests;

#[cfg(test)]
mod itemaudit_tests;

#[cfg(test)]
#[path = "enchant_loot_tests.rs"]
mod enchant_loot_tests;

#[cfg(test)]
mod catalog_collision_tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile, SlabHalf};
    struct SlabView { catalog: Catalog, state: i32 }
    impl View for SlabView {
        fn catalog(&self) -> Catalog { self.catalog }
        fn block(&self, x: i32, y: i32, z: i32) -> i32 { if (x,y,z) == (0,0,0) { self.state } else { crate::world::AIR } }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }
    #[test]
    fn preview_slabs_clip_floor_ceiling_and_sides_for_both_water_states() {
        let catalog = Catalog::new(Profile::Preview26_3);
        for water in [false, true] {
            for (half, bottom, top) in [(SlabHalf::Bottom, 0.0, 0.5), (SlabHalf::Top, 0.5, 1.0), (SlabHalf::Double, 0.0, 1.0)] {
                let view = SlabView { catalog, state: catalog.slab_state(half,water).unwrap() };
                assert!((clip_y(&view,(0.2,2.0,0.2),0.2,0.2,-3.0) - (top-2.0)).abs() < EPS);
                assert!((clip_y(&view,(0.2,-1.0,0.2),0.2,0.2,3.0) - (bottom+0.8)).abs() < EPS);
                assert!((clip_x(&view,(-1.0,bottom+0.05,0.2),0.2,0.2,2.0)-0.6).abs() < EPS);
                assert!((clip_z(&view,(0.2,bottom+0.05,-1.0),0.2,0.2,2.0)-0.6).abs() < EPS);
                if half == SlabHalf::Bottom { assert_eq!(clip_x(&view,(-1.0,0.6,0.2),0.2,0.2,2.0),2.0); }
                if half == SlabHalf::Top { assert_eq!(clip_x(&view,(-1.0,0.1,0.2),0.2,0.2,2.0),2.0); }
            }
        }
        let base = SlabView { catalog: Catalog::new(Profile::Java26_1_2), state: catalog.slab_state(SlabHalf::Bottom,false).unwrap() };
        assert_eq!(catalog_clip(&base,(0.2,2.0,0.2),0.2,0.2,-3.0,1),None);
    }
}
